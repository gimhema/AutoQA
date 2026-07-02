//! MiniChess — TCP 2인 대전 미니 체스.
//!
//! 기물은 King/Pawn뿐이고, King이 잡히면 승부가 갈린다. 두 기물 모두 매 턴 상하좌우
//! 1칸 이동한다. 필드 크기와 Pawn 개수는 host가 정한다.
//!
//! 실행:
//!   host: `minichess host [--width W] [--height H] [--pawns N] [--port P]`
//!   guest: `minichess join <host:port>`
//!
//! host는 White(선공), guest는 Black. 입력은 `<col> <row> <방향>` 형식이며 방향은
//! WASD(w=위, a=좌, s=아래, d=우)다. 예: `2 4 w` = (2,4) 기물을 위로 한 칸.

mod game;
mod net;
mod render;

use std::io::{self, Write};

use game::{Board, Config, Player, Pos};
use net::{Msg, Peer};

const DEFAULT_PORT: u16 = 9500;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(|s| s.as_str()) {
        Some("host") => run_host(&args[2..]),
        Some("join") => run_join(&args[2..]),
        _ => {
            print_usage(&args[0]);
            std::process::exit(1);
        }
    };
    if let Err(e) = result {
        eprintln!("오류: {e}");
        std::process::exit(1);
    }
}

fn print_usage(prog: &str) {
    eprintln!(
        "MiniChess — TCP 2인 대전\n\n\
         사용법:\n  \
           {prog} host [--width W] [--height H] [--pawns N] [--port P]\n  \
           {prog} join <host:port>\n\n\
         입력: <col> <row> <방향>  (방향: w=위 a=좌 s=아래 d=우)\n  \
         예: `2 4 w` = (2,4) 기물을 위로 한 칸\n"
    );
}

/// host: 설정을 파싱하고 guest 접속을 기다린 뒤 White로 플레이한다.
fn run_host(args: &[String]) -> io::Result<()> {
    let mut width = 6i32;
    let mut height = 6i32;
    let mut pawns = 4i32;
    let mut port = DEFAULT_PORT;

    let mut i = 0;
    while i < args.len() {
        let need = |i: usize| -> io::Result<&String> {
            args.get(i + 1)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{} 값이 필요합니다", args[i])))
        };
        let parse_i32 = |i: usize| -> io::Result<i32> {
            need(i)?
                .parse()
                .map_err(|_| invalid(format!("{} 값은 정수여야 합니다", args[i])))
        };
        match args[i].as_str() {
            "--width" => width = parse_i32(i)?,
            "--height" => height = parse_i32(i)?,
            "--pawns" => pawns = parse_i32(i)?,
            "--port" => {
                port = need(i)?
                    .parse()
                    .map_err(|_| invalid(format!("{} 값은 포트 번호여야 합니다", args[i])))?
            }
            other => return Err(invalid(format!("알 수 없는 옵션: {other}"))),
        }
        i += 2;
    }

    let config = Config { width, height, pawns };
    if let Err(e) = config.validate() {
        return Err(invalid(e));
    }

    println!("[MiniChess] 포트 {port}에서 상대 접속 대기 중… (필드 {width}x{height}, Pawn {pawns})");
    let mut peer = Peer::host(("0.0.0.0", port))?;
    println!("[MiniChess] 상대 연결됨! 게임 시작 (당신: White, 선공)");

    // guest에게 설정을 알린다.
    peer.send(&Msg::Config(config))?;

    let board = Board::new(config);
    play(board, peer, Player::White)
}

/// guest: host에 접속해 설정을 받고 Black으로 플레이한다.
fn run_join(args: &[String]) -> io::Result<()> {
    let addr = args
        .first()
        .ok_or_else(|| invalid("host:port 를 지정하세요"))?;

    println!("[MiniChess] {addr} 에 접속 중…");
    let mut peer = Peer::join(addr.as_str())?;
    println!("[MiniChess] 연결됨! 설정 수신 대기…");

    let config = match peer.recv()? {
        Some(Msg::Config(c)) => c,
        Some(other) => return Err(invalid(format!("설정 대신 {other:?} 수신"))),
        None => return Err(invalid("설정 수신 전 연결이 끊겼습니다")),
    };
    println!(
        "[MiniChess] 설정 수신: 필드 {}x{}, Pawn {} (당신: Black, 후공)",
        config.width, config.height, config.pawns
    );

    let board = Board::new(config);
    play(board, peer, Player::Black)
}

/// 공통 게임 루프. 자기 턴이면 stdin으로 이동을 받고, 아니면 상대 이동을 기다린다.
fn play(mut board: Board, mut peer: Peer, me: Player) -> io::Result<()> {
    loop {
        // 매 턴 보드 출력.
        print!("\n{}", render::render_with_status(&board, me));
        io::stdout().flush()?;

        if board.is_over() {
            return Ok(());
        }

        if board.turn() == me {
            // 내 턴: 유효한 이동을 받을 때까지 반복.
            let (from, to) = loop {
                match read_move(&board, me) {
                    Ok(Some((from, to))) => match board.apply_move(from, to) {
                        Ok(_) => break (from, to),
                        Err(e) => println!("잘못된 이동: {e}. 다시 입력하세요."),
                    },
                    Ok(None) => {
                        // 사용자 종료.
                        let _ = peer.send(&Msg::Quit);
                        println!("게임을 종료합니다.");
                        return Ok(());
                    }
                    Err(e) => println!("입력 오류: {e}. 다시 입력하세요."),
                }
            };
            // 상대에게 이동 통보.
            peer.send(&Msg::Move { from, to })?;
        } else {
            // 상대 턴: 이동 수신.
            println!("상대의 이동을 기다리는 중…");
            match peer.recv()? {
                Some(Msg::Move { from, to }) => {
                    if let Err(e) = board.apply_move(from, to) {
                        // 룰이 결정론적이므로 정상적으로는 도달 불가.
                        return Err(invalid(format!("상대의 이동이 규칙에 어긋납니다: {e}")));
                    }
                }
                Some(Msg::Quit) => {
                    println!("상대가 게임을 종료했습니다.");
                    return Ok(());
                }
                Some(other) => return Err(invalid(format!("예기치 않은 메시지: {other:?}"))),
                None => {
                    println!("상대와의 연결이 끊겼습니다.");
                    return Ok(());
                }
            }
        }
    }
}

/// stdin에서 한 줄을 읽어 이동으로 파싱한다.
///
/// - `Ok(Some((from, to)))`: 유효 형식의 이동 (룰 검증은 호출부에서)
/// - `Ok(None)`: 사용자가 종료(quit/q) 또는 EOF
/// - `Err`: 형식 오류
fn read_move(board: &Board, _me: Player) -> io::Result<Option<(Pos, Pos)>> {
    print!("이동 입력 (col row 방향[wasd], 종료=q) > ");
    io::stdout().flush()?;

    let mut line = String::new();
    let n = io::stdin().read_line(&mut line)?;
    if n == 0 {
        return Ok(None); // EOF
    }
    let line = line.trim();
    if line.eq_ignore_ascii_case("q") || line.eq_ignore_ascii_case("quit") {
        return Ok(None);
    }

    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() != 3 {
        return Err(invalid("형식: <col> <row> <방향wasd>"));
    }
    let x: i32 = tokens[0].parse().map_err(|_| invalid("col은 숫자여야 합니다"))?;
    let y: i32 = tokens[1].parse().map_err(|_| invalid("row는 숫자여야 합니다"))?;
    let (dx, dy) = match tokens[2].to_ascii_lowercase().as_str() {
        "w" => (0, -1),
        "s" => (0, 1),
        "a" => (-1, 0),
        "d" => (1, 0),
        _ => return Err(invalid("방향은 w/a/s/d 중 하나")),
    };

    let from = Pos::new(x, y);
    let to = Pos::new(x + dx, y + dy);
    if !board.in_bounds(from) {
        return Err(invalid("출발 좌표가 보드 밖입니다"));
    }
    Ok(Some((from, to)))
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}
