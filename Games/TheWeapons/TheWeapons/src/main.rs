//! TheWeapons — 무기 카드를 소켓에 배치해 상대 HP를 먼저 소모시키는 2인 턴제 카드 게임.
//! 매 턴 양측의 카드가 동시에 공개되어 효과가 적용된다.
//!
//! 실행:
//!   host: `theweapons host [--sockets N] [--hp N] [--sword N] [--shield N] [--spear N] [--port P]`
//!   join: `theweapons join <host:port>`
//!
//! 입력은 소켓 수만큼의 토큰(공백 구분)이며 각 토큰은 s(검)/d(방패)/p(창)/.(빈 소켓) 중 하나다.
//! 예: `s d .` = 소켓1 검, 소켓2 방패, 소켓3 비움.

mod cards;
mod game;
mod net;
mod render;

use std::io::{self, Write};

use cards::Card;
use game::{Config, Match};
use net::{Msg, Peer};

const DEFAULT_PORT: u16 = 9600;

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
        "TheWeapons — 2인 대전 카드 게임\n\n\
         사용법:\n  \
           {prog} host [--sockets N] [--hp N] [--sword N] [--shield N] [--spear N] [--port P]\n  \
           {prog} join <host:port>\n\n\
         입력: 소켓 수만큼 토큰(공백 구분). s=검 d=방패 p=창 .=빈 소켓\n  \
         예: `s d .` = 소켓1 검, 소켓2 방패, 소켓3 비움\n"
    );
}

/// host: 세팅을 파싱하고 guest 접속을 기다린 뒤 세팅을 알려준다.
fn run_host(args: &[String]) -> io::Result<()> {
    let mut socket_count = 3usize;
    let mut initial_hp = 10i32;
    let mut sword_count = 5u32;
    let mut shield_count = 5u32;
    let mut spear_count = 5u32;
    let mut port = DEFAULT_PORT;

    let mut i = 0;
    while i < args.len() {
        let need = |i: usize| -> io::Result<&String> {
            args.get(i + 1)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{} 값이 필요합니다", args[i])))
        };
        match args[i].as_str() {
            "--sockets" => {
                socket_count = need(i)?
                    .parse()
                    .map_err(|_| invalid("--sockets 값은 정수여야 합니다"))?;
            }
            "--hp" => {
                initial_hp = need(i)?
                    .parse()
                    .map_err(|_| invalid("--hp 값은 정수여야 합니다"))?;
            }
            "--sword" => {
                sword_count = need(i)?
                    .parse()
                    .map_err(|_| invalid("--sword 값은 정수여야 합니다"))?;
            }
            "--shield" => {
                shield_count = need(i)?
                    .parse()
                    .map_err(|_| invalid("--shield 값은 정수여야 합니다"))?;
            }
            "--spear" => {
                spear_count = need(i)?
                    .parse()
                    .map_err(|_| invalid("--spear 값은 정수여야 합니다"))?;
            }
            "--port" => {
                port = need(i)?
                    .parse()
                    .map_err(|_| invalid("--port 값은 포트 번호여야 합니다"))?;
            }
            other => return Err(invalid(format!("알 수 없는 옵션: {other}"))),
        }
        i += 2;
    }

    let config = Config {
        socket_count,
        initial_hp,
        sword_count,
        shield_count,
        spear_count,
    };
    config.validate().map_err(invalid)?;

    println!(
        "[TheWeapons] 포트 {port}에서 상대 접속 대기 중… (소켓 {socket_count}, HP {initial_hp}, 검{sword_count}/방패{shield_count}/창{spear_count})"
    );
    let mut peer = Peer::host(("0.0.0.0", port))?;
    println!("[TheWeapons] 상대 연결됨! 게임 시작");

    peer.send(&Msg::Config(config))?;

    play(Match::new(config), peer)
}

/// guest: host에 접속해 세팅을 받는다.
fn run_join(args: &[String]) -> io::Result<()> {
    let addr = args.first().ok_or_else(|| invalid("host:port 를 지정하세요"))?;

    println!("[TheWeapons] {addr} 에 접속 중…");
    let mut peer = Peer::join(addr.as_str())?;
    println!("[TheWeapons] 연결됨! 세팅 수신 대기…");

    let config = match peer.recv()? {
        Some(Msg::Config(c)) => c,
        Some(other) => return Err(invalid(format!("세팅 대신 {other:?} 수신"))),
        None => return Err(invalid("세팅 수신 전 연결이 끊겼습니다")),
    };
    println!(
        "[TheWeapons] 세팅 수신: 소켓 {} / HP {} / 검{} 방패{} 창{}",
        config.socket_count, config.initial_hp, config.sword_count, config.shield_count, config.spear_count
    );

    play(Match::new(config), peer)
}

/// 공통 게임 루프. 매 턴 내 배치를 받아 상대와 교환하고 동시에 판정한다.
fn play(mut m: Match, mut peer: Peer) -> io::Result<()> {
    loop {
        print!("{}{}", render::CLEAR, render::render(&m));
        io::stdout().flush()?;

        if m.is_over() {
            return Ok(());
        }

        let my_play = loop {
            match read_play(&m) {
                Ok(Some(play)) => break play,
                Ok(None) => {
                    let _ = peer.send(&Msg::Quit);
                    println!("게임을 종료합니다.");
                    return Ok(());
                }
                Err(e) => println!("입력 오류: {e}. 다시 입력하세요."),
            }
        };

        // 동시 공개: 서로 자기 배치를 먼저 보내고 나서 상대 것을 받는다.
        peer.send(&Msg::Play(my_play.clone()))?;

        match peer.recv()? {
            Some(Msg::Play(their_play)) => {
                if their_play.len() != m.config.socket_count {
                    return Err(invalid("상대 배치의 소켓 수가 세팅과 다릅니다"));
                }
                m.apply_turn(my_play, their_play);
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

/// stdin에서 한 줄을 읽어 소켓 배치로 파싱한다.
///
/// - `Ok(Some(play))`: 소켓 수·손패 범위를 통과한 배치
/// - `Ok(None)`: 사용자가 종료(quit/q) 또는 EOF
/// - `Err`: 형식 오류 또는 손패 초과
fn read_play(m: &Match) -> io::Result<Option<Vec<Option<Card>>>> {
    print!(
        "카드 배치 ({}개 토큰, s=검 d=방패 p=창 .=빈, 종료=q) > ",
        m.config.socket_count
    );
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
    if tokens.len() != m.config.socket_count {
        return Err(invalid(format!(
            "토큰 {}개가 필요합니다 (받은 개수: {})",
            m.config.socket_count,
            tokens.len()
        )));
    }

    let mut play = Vec::with_capacity(tokens.len());
    for tok in &tokens {
        if *tok == "." {
            play.push(None);
            continue;
        }
        let ch = tok
            .chars()
            .next()
            .ok_or_else(|| invalid("빈 토큰입니다"))?;
        match Card::from_short(ch) {
            Some(card) => play.push(Some(card)),
            None => return Err(invalid(format!("알 수 없는 토큰: {tok} (s/d/p/. 중 하나)"))),
        }
    }

    if !m.can_afford(&play) {
        return Err(invalid("보유한 카드 수를 초과했습니다"));
    }

    Ok(Some(play))
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}
