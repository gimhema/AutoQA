//! Ouroboros QA 에이전트와의 연동 (`ai` 서브커맨드).
//!
//! MiniChess는 턴제라 인간 턴/AI 턴을 번갈아 기다리지만, Invader는 실시간
//! 1인 게임이라 그럴 필요가 없다. 이 모듈은 Ouroboros의 "빠른 루프" 개념을
//! 그대로 게임 쪽에 반영한다: 매 틱 관측을 보내고, 그 시점에 도착해 있는
//! 최신 액션을 그대로 적용한다. Ouroboros가 게임 전체를 담당하며, 터미널은
//! 관전용으로만 쓰인다(Q/Esc로 조기 종료만 가능).
//!
//! # 관측 포맷 (Ouroboros Dynamic policy용)
//! ```json
//! {
//!   "width": 30, "height": 20,
//!   "player_x": 15, "player_y": 19,
//!   "remaining_blocks": 12, "total_blocks": 20,
//!   "remaining_time_ms": 45230,
//!   "can_shoot": true,
//!   "blocks":  [{"x": 3, "y": 1}],
//!   "bullets": [{"x": 15, "y": 10}],
//!   "valid_actions": [
//!     {"action": "move_left",  "resulting_x": 14, "blocks_in_column": 1},
//!     {"action": "move_right", "resulting_x": 16, "blocks_in_column": 0},
//!     {"action": "shoot", "aligned_blocks": 1},
//!     {"action": "stay", "blocks_in_column": 0}
//!   ]
//! }
//! ```
//! `shoot`은 쿨다운 중이면 `valid_actions`에서 아예 빠진다.
//!
//! # 액션 포맷 (Ouroboros → 게임)
//! `valid_actions` 항목을 그대로 반환한다. `parse_action`은 `action` 필드만
//! 읽고 나머지 피처는 무시한다.

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ouroboros_link::OuroborosLink;
use serde_json::{json, Value};

use crate::game::{Config, Game, Outcome, HEIGHT, TICK_MS, WIDTH};
use crate::render;

pub struct AiConfig {
    pub game_config: Config,
    pub ouroboros_port: u16,
}

enum RunOutcome {
    Win,
    Lose,
    Quit,
    Disconnected,
}

/// AI 모드 메인 루프.
pub fn run(config: AiConfig) -> io::Result<()> {
    eprintln!(
        "[Invader] 포트 {}에서 Ouroboros 접속 대기 중…",
        config.ouroboros_port
    );
    let mut link = OuroborosLink::accept(("0.0.0.0", config.ouroboros_port))?;
    eprintln!("[Invader] Ouroboros 연결됨! 게임 시작 (관전 모드, Q로 종료)");

    let mut game = Game::new(&config.game_config);
    let guard = render::TerminalGuard::new()?;
    let mut out = io::stdout();

    let outcome = 'game_loop: loop {
        if !link.is_connected() {
            break 'game_loop RunOutcome::Disconnected;
        }

        // Q/Esc만 처리하는 관전 모드 입력. 나머지 키는 무시(조작권은 Ouroboros).
        if event::poll(Duration::from_millis(TICK_MS))? {
            loop {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Release
                        && matches!(key.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q'))
                    {
                        break 'game_loop RunOutcome::Quit;
                    }
                }
                if !event::poll(Duration::from_millis(0))? {
                    break;
                }
            }
        }

        if let Some(action) = link.poll_action() {
            apply_action(&mut game, &action.command);
        }

        match game.tick() {
            Outcome::Win => break 'game_loop RunOutcome::Win,
            Outcome::Lose => break 'game_loop RunOutcome::Lose,
            Outcome::Ongoing => {}
        }

        render::draw(&mut out, &game, "(관전 모드, Ouroboros 조종 중 · Q 종료)")?;
        link.send_observation(observation(&game))?;
    };

    drop(guard);
    match outcome {
        RunOutcome::Win => println!("Ouroboros가 모든 블록을 파괴했습니다! 승리!"),
        RunOutcome::Lose => println!("제한시간 종료. 블록을 모두 파괴하지 못했습니다. 패배!"),
        RunOutcome::Quit => println!("게임을 종료합니다."),
        RunOutcome::Disconnected => println!("Ouroboros 연결이 끊겨 게임을 종료합니다."),
    }
    Ok(())
}

/// `{"action": "..."}`만 읽는다. 나머지 피처 필드는 무시.
fn apply_action(game: &mut Game, command: &Value) {
    match command.get("action").and_then(|v| v.as_str()) {
        Some("move_left") => game.move_left(),
        Some("move_right") => game.move_right(),
        Some("shoot") => {
            game.shoot();
        }
        _ => {}
    }
}

/// 게임 상태를 Ouroboros Dynamic policy용 관측 JSON으로 직렬화한다.
fn observation(game: &Game) -> Value {
    let mut valid_actions = Vec::new();

    if game.player_x > 0 {
        let rx = game.player_x - 1;
        valid_actions.push(json!({
            "action": "move_left",
            "resulting_x": rx,
            "blocks_in_column": game.blocks_in_column(rx),
        }));
    }
    if game.player_x < WIDTH - 1 {
        let rx = game.player_x + 1;
        valid_actions.push(json!({
            "action": "move_right",
            "resulting_x": rx,
            "blocks_in_column": game.blocks_in_column(rx),
        }));
    }
    if game.can_shoot() {
        valid_actions.push(json!({
            "action": "shoot",
            "aligned_blocks": game.blocks_in_column(game.player_x),
        }));
    }
    valid_actions.push(json!({
        "action": "stay",
        "blocks_in_column": game.blocks_in_column(game.player_x),
    }));

    json!({
        "width": WIDTH,
        "height": HEIGHT,
        "player_x": game.player_x,
        "player_y": game.player_y,
        "remaining_blocks": game.blocks.len(),
        "total_blocks": game.total_blocks,
        "remaining_time_ms": game.remaining().as_millis(),
        "can_shoot": game.can_shoot(),
        "blocks": game.blocks.iter().map(|(x, y)| json!({"x": x, "y": y})).collect::<Vec<_>>(),
        "bullets": game.bullets.iter().map(|b| json!({"x": b.x, "y": b.y})).collect::<Vec<_>>(),
        "valid_actions": valid_actions,
    })
}
