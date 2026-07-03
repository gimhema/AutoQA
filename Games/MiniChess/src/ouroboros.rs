//! Ouroboros QA 에이전트와의 연동.
//!
//! `ai` 서브커맨드에서 사용한다. 인간이 한 진영을, Ouroboros가 나머지 진영을
//! 담당한다. 게임은 매 AI 턴마다 보드 상태를 관측(observation)으로 전송하고,
//! Ouroboros가 보내는 액션을 이동으로 파싱해 적용한다.
//!
//! # 관측 포맷 (Ouroboros Dynamic policy용)
//! ```json
//! {
//!   "width": 6, "height": 6,
//!   "turn": "black",
//!   "ai_color": "black",
//!   "my_king":    {"x": 3, "y": 0},
//!   "my_pawns":   [{"x": 0, "y": 0}, {"x": 1, "y": 0}],
//!   "enemy_king": {"x": 3, "y": 5},
//!   "enemy_pawns":[{"x": 0, "y": 5}],
//!   "valid_actions": [
//!     {
//!       "from_x": 3, "from_y": 0, "to_x": 3, "to_y": 1,
//!       "piece": "king",
//!       "is_capture": false,
//!       "captured_kind": null,
//!       "dist_to_enemy_king_delta": -1
//!     }
//!   ]
//! }
//! ```
//!
//! # 액션 포맷 (Ouroboros → 게임)
//! `valid_actions` 항목을 그대로 반환한다:
//! ```json
//! {"from_x": 3, "from_y": 0, "to_x": 3, "to_y": 1, "piece": "king", ...}
//! ```
//! `parse_action`은 `from_x/from_y/to_x/to_y`만 읽고 나머지는 무시한다.

use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use ouroboros_link::OuroborosLink;
use serde_json::{json, Value};

use crate::game::{Board, Kind, Player, Pos};
use crate::render;

pub struct AiConfig {
    pub board_config: crate::game::Config,
    /// Ouroboros가 조종하는 진영. 인간은 반대 진영.
    pub ai_player: Player,
    pub ouroboros_port: u16,
}

/// AI 모드 메인 루프.
pub fn run(config: AiConfig) -> io::Result<()> {
    let me = config.ai_player.other();

    eprintln!(
        "[MiniChess] 포트 {}에서 Ouroboros 접속 대기 중…",
        config.ouroboros_port
    );
    let mut link = OuroborosLink::accept(("0.0.0.0", config.ouroboros_port))?;
    eprintln!("[MiniChess] Ouroboros 연결됨! 게임 시작");
    eprintln!(
        "[MiniChess] 당신: {} ({}) / Ouroboros: {} ({})",
        me.name(),
        if me == Player::White { "선공" } else { "후공" },
        config.ai_player.name(),
        if config.ai_player == Player::White { "선공" } else { "후공" },
    );

    let mut board = Board::new(config.board_config);

    loop {
        print!("{}{}", render::CLEAR, render::render_with_status(&board, me));
        io::stdout().flush()?;

        if board.is_over() {
            return Ok(());
        }

        if board.turn() == me {
            human_turn(&mut board, &mut link, config.ai_player)?;
        } else {
            ai_turn(&mut board, &mut link, config.ai_player)?;
        }
    }
}

/// 인간 턴: stdin 입력을 받아 이동을 적용하고 관측을 전송한다.
fn human_turn(board: &mut Board, link: &mut OuroborosLink, ai_color: Player) -> io::Result<()> {
    // valid_actions: [] → Ouroboros policy가 None 반환 → 액션 전송 자동 억제
    let _ = link.send_observation(board_to_observation(board, ai_color, false));

    loop {
        match read_move(board) {
            Ok(Some((from, to))) => match board.apply_move(from, to) {
                Ok(_) => return Ok(()),
                Err(e) => println!("잘못된 이동: {e}. 다시 입력하세요."),
            },
            Ok(None) => {
                println!("게임을 종료합니다.");
                return Err(io::Error::new(io::ErrorKind::Interrupted, "user quit"));
            }
            Err(e) => println!("입력 오류: {e}. 다시 입력하세요."),
        }
    }
}

/// AI 턴: 유효한 액션이 올 때까지 관측을 반복 전송한다.
///
/// 인간 턴 중 Ouroboros는 `valid_actions: []` 관측을 받아 self-throttle하므로
/// 낡은 액션이 누적되지 않는다. drain 불필요.
fn ai_turn(board: &mut Board, link: &mut OuroborosLink, ai_color: Player) -> io::Result<()> {
    println!("Ouroboros 생각 중…");

    loop {
        if !link.is_connected() {
            eprintln!("[MiniChess] Ouroboros 연결 끊김");
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Ouroboros disconnected",
            ));
        }

        link.send_observation(board_to_observation(board, ai_color, true))?;

        if let Some(action) = link.poll_action() {
            match parse_action(&action.command) {
                Some((from, to)) => match board.apply_move(from, to) {
                    Ok(_) => {
                        eprintln!(
                            "[Ouroboros] 이동: ({},{}) → ({},{})",
                            from.x, from.y, to.x, to.y
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("[Ouroboros] 규칙 위반 이동 ({e}), 재시도");
                    }
                },
                None => {
                    eprintln!("[Ouroboros] 액션 파싱 실패: {}", action.command);
                }
            }
        }

        thread::sleep(Duration::from_millis(100));
    }
}

/// 보드 상태를 Ouroboros Dynamic policy용 관측 JSON으로 직렬화한다.
///
/// - 기물은 역할별 고정 키(`my_king`, `enemy_king` 등)로 표현한다.
/// - `is_ai_turn = true`: `valid_actions`에 합법 이동과 전략 피처를 채운다.
/// - `is_ai_turn = false`: `valid_actions: []` → policy가 None 반환 → 액션 억제.
fn board_to_observation(board: &Board, ai_color: Player, is_ai_turn: bool) -> Value {
    let enemy = ai_color.other();

    let my_king = find_king(board, ai_color);
    let enemy_king = find_king(board, enemy);
    let my_pawns = find_pawns(board, ai_color);
    let enemy_pawns = find_pawns(board, enemy);

    let valid_actions = if is_ai_turn {
        compute_valid_actions(board, ai_color, &enemy_king)
    } else {
        Value::Array(vec![])
    };

    json!({
        "width":       board.width(),
        "height":      board.height(),
        "turn":        color_str(board.turn()),
        "ai_color":    color_str(ai_color),
        "my_king":     my_king,
        "enemy_king":  enemy_king,
        "my_pawns":    my_pawns,
        "enemy_pawns": enemy_pawns,
        "valid_actions": valid_actions,
    })
}

/// 해당 진영의 King 위치. King이 잡혔으면 `null`.
fn find_king(board: &Board, player: Player) -> Value {
    for y in 0..board.height() {
        for x in 0..board.width() {
            if let Some(p) = board.get(Pos::new(x, y)) {
                if p.owner == player && p.kind == Kind::King {
                    return json!({"x": x, "y": y});
                }
            }
        }
    }
    Value::Null
}

/// 해당 진영의 Pawn 위치 목록.
fn find_pawns(board: &Board, player: Player) -> Value {
    let mut pawns = Vec::new();
    for y in 0..board.height() {
        for x in 0..board.width() {
            if let Some(p) = board.get(Pos::new(x, y)) {
                if p.owner == player && p.kind == Kind::Pawn {
                    pawns.push(json!({"x": x, "y": y}));
                }
            }
        }
    }
    Value::Array(pawns)
}

/// 현재 AI 진영의 모든 합법 이동을 전략 피처와 함께 계산한다.
///
/// 각 항목의 피처:
/// - `piece`: `"king"` | `"pawn"`
/// - `is_capture`: bool
/// - `captured_kind`: `"king"` | `"pawn"` | null
/// - `dist_to_enemy_king_delta`: 이동 후 적 King까지의 맨해튼 거리 변화량
///   (음수 = 가까워짐, 0 = 동일, 양수 = 멀어짐)
fn compute_valid_actions(board: &Board, player: Player, enemy_king: &Value) -> Value {
    let dirs: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
    let mut actions = Vec::new();

    let ek_x = enemy_king.get("x").and_then(|v| v.as_i64()).map(|v| v as i32);
    let ek_y = enemy_king.get("y").and_then(|v| v.as_i64()).map(|v| v as i32);

    for y in 0..board.height() {
        for x in 0..board.width() {
            let Some(piece) = board.get(Pos::new(x, y)) else { continue };
            if piece.owner != player { continue; }

            for (dx, dy) in dirs {
                let tx = x + dx;
                let ty = y + dy;
                let to = Pos::new(tx, ty);
                if !board.in_bounds(to) { continue; }

                let target = board.get(to);
                if target.is_some_and(|t| t.owner == player) { continue; }

                let is_capture = target.is_some();
                let captured_kind = target.map(|t| kind_str(t.kind));

                let dist_delta = match (ek_x, ek_y) {
                    (Some(ex), Some(ey)) => {
                        let before = (x - ex).abs() + (y - ey).abs();
                        let after  = (tx - ex).abs() + (ty - ey).abs();
                        after - before
                    }
                    _ => 0,
                };

                actions.push(json!({
                    "from_x": x,
                    "from_y": y,
                    "to_x":   tx,
                    "to_y":   ty,
                    "piece":  kind_str(piece.kind),
                    "is_capture":          is_capture,
                    "captured_kind":       captured_kind,
                    "dist_to_enemy_king_delta": dist_delta,
                }));
            }
        }
    }

    Value::Array(actions)
}

/// `{"from_x", "from_y", "to_x", "to_y"}`를 파싱한다. 나머지 피처 필드는 무시.
fn parse_action(command: &Value) -> Option<(Pos, Pos)> {
    let fx = command.get("from_x")?.as_i64()? as i32;
    let fy = command.get("from_y")?.as_i64()? as i32;
    let tx = command.get("to_x")?.as_i64()? as i32;
    let ty = command.get("to_y")?.as_i64()? as i32;
    Some((Pos::new(fx, fy), Pos::new(tx, ty)))
}

fn color_str(player: Player) -> &'static str {
    match player {
        Player::White => "white",
        Player::Black => "black",
    }
}

fn kind_str(kind: Kind) -> &'static str {
    match kind {
        Kind::King => "king",
        Kind::Pawn => "pawn",
    }
}

/// stdin에서 이동을 파싱한다.
fn read_move(board: &Board) -> io::Result<Option<(Pos, Pos)>> {
    print!("이동 입력 (col row 방향[wasd], 종료=q) > ");
    io::stdout().flush()?;

    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let line = line.trim();
    if line.eq_ignore_ascii_case("q") || line.eq_ignore_ascii_case("quit") {
        return Ok(None);
    }

    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "형식: <col> <row> <방향wasd>",
        ));
    }
    let x: i32 = tokens[0]
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "col은 숫자여야 합니다"))?;
    let y: i32 = tokens[1]
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "row는 숫자여야 합니다"))?;
    let (dx, dy) = match tokens[2].to_ascii_lowercase().as_str() {
        "w" => (0, -1),
        "s" => (0, 1),
        "a" => (-1, 0),
        "d" => (1, 0),
        _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "방향은 w/a/s/d 중 하나")),
    };

    let from = Pos::new(x, y);
    let to = Pos::new(x + dx, y + dy);
    if !board.in_bounds(from) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "출발 좌표가 보드 밖입니다"));
    }
    Ok(Some((from, to)))
}
