//! Invader — 좌우 이동과 탄환 발사로 상단 블록을 파괴하는 CLI 슈팅 게임.
//!
//! 실행:
//!   play (기본): `invader` 또는 `invader play` — 사람이 A/D/S로 직접 조작.
//!   ai:          `invader ai [--time-limit N] [--blocks N] [--ouroboros-port P]`
//!                Ouroboros 에이전트가 접속해 플레이를 대신한다.

mod game;
mod ouroboros;
mod render;

use std::io::{self, Write};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use game::{Config, Game, Outcome};

const DEFAULT_TIME_LIMIT_SECS: u64 = 60;
const DEFAULT_BLOCK_COUNT: usize = 20;
const DEFAULT_OUROBOROS_PORT: u16 = 9000;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(|s| s.as_str()) {
        Some("ai") => run_ai(&args[2..]),
        Some("play") | None => run_play(),
        Some(other) => {
            eprintln!("알 수 없는 서브커맨드: {other}");
            print_usage(&args[0]);
            std::process::exit(1);
        }
    };
    if let Err(e) = result {
        eprintln!("오류: {e}");
        std::process::exit(1);
    }
    Ok(())
}

fn print_usage(prog: &str) {
    eprintln!(
        "Invader — CLI 슈팅 게임 / Ouroboros AI 플레이\n\n\
         사용법:\n  \
           {prog} [play]                                                사람이 직접 조작\n  \
           {prog} ai [--time-limit N] [--blocks N] [--ouroboros-port P]  Ouroboros가 플레이\n\n\
         play 모드 조작: A/D 이동, S 발사, Q/Esc 종료\n"
    );
}

enum RunOutcome {
    Win,
    Lose,
    Quit,
}

/// 사람이 직접 조작하는 기본 모드.
fn run_play() -> io::Result<()> {
    let config = read_config_interactive();
    let mut game = Game::new(&config);
    let guard = render::TerminalGuard::new()?;
    let mut out = io::stdout();

    let outcome = 'game_loop: loop {
        if event::poll(Duration::from_millis(game::TICK_MS))? {
            loop {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Release {
                        match key.code {
                            KeyCode::Char('a') | KeyCode::Char('A') => game.move_left(),
                            KeyCode::Char('d') | KeyCode::Char('D') => game.move_right(),
                            KeyCode::Char('s') | KeyCode::Char('S') => {
                                game.shoot();
                            }
                            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                                break 'game_loop RunOutcome::Quit;
                            }
                            _ => {}
                        }
                    }
                }
                if !event::poll(Duration::from_millis(0))? {
                    break;
                }
            }
        }

        match game.tick() {
            Outcome::Win => break 'game_loop RunOutcome::Win,
            Outcome::Lose => break 'game_loop RunOutcome::Lose,
            Outcome::Ongoing => {}
        }

        render::draw(&mut out, &game, "(A/D 이동, S 발사, Q 종료)")?;
    };

    drop(guard);
    match outcome {
        RunOutcome::Win => println!("모든 블록을 파괴했습니다! 승리!"),
        RunOutcome::Lose => println!("제한시간 종료. 블록을 모두 파괴하지 못했습니다. 패배!"),
        RunOutcome::Quit => println!("게임을 종료합니다."),
    }
    Ok(())
}

fn read_config_interactive() -> Config {
    println!("=== Invader 설정 ===");
    let time_limit_secs = prompt_u64("제한시간(초)", DEFAULT_TIME_LIMIT_SECS);
    let max_blocks = Config::max_blocks();
    let block_count = prompt_usize("블록 갯수", DEFAULT_BLOCK_COUNT, max_blocks);
    Config {
        time_limit: Duration::from_secs(time_limit_secs),
        block_count,
    }
}

fn prompt_u64(label: &str, default: u64) -> u64 {
    print!("{label} (기본 {default}): ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_ok() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            if let Ok(v) = trimmed.parse::<u64>() {
                if v > 0 {
                    return v;
                }
            }
        }
    }
    default
}

fn prompt_usize(label: &str, default: usize, max: usize) -> usize {
    print!("{label} (기본 {default}, 최대 {max}): ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_ok() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            if let Ok(v) = trimmed.parse::<usize>() {
                if v > 0 {
                    return v.min(max);
                }
            }
        }
    }
    default.min(max)
}

/// ai: Ouroboros가 게임 전체를 담당하는 자동 플레이 모드.
fn run_ai(args: &[String]) -> io::Result<()> {
    let mut time_limit_secs = DEFAULT_TIME_LIMIT_SECS;
    let mut block_count = DEFAULT_BLOCK_COUNT;
    let mut ouroboros_port = DEFAULT_OUROBOROS_PORT;

    let mut i = 0;
    while i < args.len() {
        let need = |i: usize| -> io::Result<&String> {
            args.get(i + 1)
                .ok_or_else(|| invalid(format!("{} 값이 필요합니다", args[i])))
        };
        match args[i].as_str() {
            "--time-limit" => {
                time_limit_secs = need(i)?
                    .parse()
                    .map_err(|_| invalid("--time-limit 값은 정수(초)여야 합니다"))?;
            }
            "--blocks" => {
                block_count = need(i)?
                    .parse()
                    .map_err(|_| invalid("--blocks 값은 정수여야 합니다"))?;
            }
            "--ouroboros-port" => {
                ouroboros_port = need(i)?
                    .parse()
                    .map_err(|_| invalid("--ouroboros-port 값은 포트 번호여야 합니다"))?;
            }
            other => return Err(invalid(format!("알 수 없는 옵션: {other}"))),
        }
        i += 2;
    }

    let block_count = block_count.min(Config::max_blocks());
    let config = Config {
        time_limit: Duration::from_secs(time_limit_secs),
        block_count,
    };

    ouroboros::run(ouroboros::AiConfig {
        game_config: config,
        ouroboros_port,
    })
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}
