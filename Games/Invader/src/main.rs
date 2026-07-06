use std::collections::HashSet;
use std::io::{self, stdout, Write};
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::Print,
    terminal::{self, ClearType},
};
use rand::seq::SliceRandom;
use rand::thread_rng;

const WIDTH: i32 = 30;
const HEIGHT: i32 = 20;
const BLOCK_ROWS: i32 = 5;
const TICK_MS: u64 = 50;
const SHOOT_COOLDOWN: Duration = Duration::from_millis(400);
const DEFAULT_TIME_LIMIT_SECS: u64 = 60;
const DEFAULT_BLOCK_COUNT: usize = 20;

struct Bullet {
    x: i32,
    y: i32,
}

struct GameConfig {
    time_limit: Duration,
    block_count: usize,
}

enum Outcome {
    Win,
    Lose,
    Quit,
}

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), terminal::EnterAlternateScreen, cursor::Hide)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn main() -> io::Result<()> {
    let config = read_config();

    let guard = TerminalGuard::new()?;
    let outcome = run_game(&config)?;
    drop(guard);

    match outcome {
        Outcome::Win => println!("모든 블록을 파괴했습니다! 승리!"),
        Outcome::Lose => println!("제한시간 종료. 블록을 모두 파괴하지 못했습니다. 패배!"),
        Outcome::Quit => println!("게임을 종료합니다."),
    }
    Ok(())
}

fn read_config() -> GameConfig {
    println!("=== Invader 설정 ===");
    let time_limit_secs = prompt_u64("제한시간(초)", DEFAULT_TIME_LIMIT_SECS);
    let max_blocks = (WIDTH * BLOCK_ROWS) as usize;
    let block_count = prompt_usize("블록 갯수", DEFAULT_BLOCK_COUNT, max_blocks);
    GameConfig {
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

fn run_game(config: &GameConfig) -> io::Result<Outcome> {
    let mut rng = thread_rng();
    let mut positions: Vec<(i32, i32)> = (0..WIDTH)
        .flat_map(|x| (0..BLOCK_ROWS).map(move |y| (x, y)))
        .collect();
    positions.shuffle(&mut rng);
    let mut blocks: HashSet<(i32, i32)> = positions.into_iter().take(config.block_count).collect();
    let total_blocks = blocks.len();

    let mut player_x = WIDTH / 2;
    let player_y = HEIGHT - 1;
    let mut bullets: Vec<Bullet> = Vec::new();
    let mut last_shot = Instant::now()
        .checked_sub(SHOOT_COOLDOWN)
        .unwrap_or_else(Instant::now);

    let start = Instant::now();
    let mut out = stdout();

    loop {
        if event::poll(Duration::from_millis(TICK_MS))? {
            loop {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Release {
                        match key.code {
                            KeyCode::Char('a') | KeyCode::Char('A') => {
                                player_x = (player_x - 1).max(0);
                            }
                            KeyCode::Char('d') | KeyCode::Char('D') => {
                                player_x = (player_x + 1).min(WIDTH - 1);
                            }
                            KeyCode::Char('s') | KeyCode::Char('S') => {
                                if last_shot.elapsed() >= SHOOT_COOLDOWN {
                                    bullets.push(Bullet {
                                        x: player_x,
                                        y: player_y - 1,
                                    });
                                    last_shot = Instant::now();
                                }
                            }
                            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                                return Ok(Outcome::Quit);
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

        let mut i = 0;
        while i < bullets.len() {
            bullets[i].y -= 1;
            let pos = (bullets[i].x, bullets[i].y);
            if bullets[i].y < 0 {
                bullets.remove(i);
            } else if blocks.remove(&pos) {
                bullets.remove(i);
            } else {
                i += 1;
            }
        }

        if blocks.is_empty() {
            return Ok(Outcome::Win);
        }

        let elapsed = start.elapsed();
        if elapsed >= config.time_limit {
            return Ok(Outcome::Lose);
        }

        render(
            &mut out,
            player_x,
            player_y,
            &bullets,
            &blocks,
            total_blocks,
            config.time_limit,
            elapsed,
        )?;
    }
}

fn render(
    out: &mut io::Stdout,
    player_x: i32,
    player_y: i32,
    bullets: &[Bullet],
    blocks: &HashSet<(i32, i32)>,
    total_blocks: usize,
    time_limit: Duration,
    elapsed: Duration,
) -> io::Result<()> {
    let remaining = time_limit.saturating_sub(elapsed).as_secs();
    let mut frame = String::new();
    frame.push_str(&format!(
        "[ Invader ]  남은 시간: {:>3}s   남은 블록: {:>3}/{:<3}   (A/D 이동, S 발사, Q 종료)\r\n",
        remaining,
        blocks.len(),
        total_blocks
    ));
    frame.push_str(&"-".repeat(WIDTH as usize));
    frame.push_str("\r\n");

    for y in 0..HEIGHT {
        let mut line = String::with_capacity(WIDTH as usize);
        for x in 0..WIDTH {
            let ch = if y == player_y && x == player_x {
                'A'
            } else if blocks.contains(&(x, y)) {
                '#'
            } else if bullets.iter().any(|b| b.x == x && b.y == y) {
                '|'
            } else {
                '.'
            };
            line.push(ch);
        }
        frame.push_str(&line);
        frame.push_str("\r\n");
    }

    execute!(
        out,
        cursor::MoveTo(0, 0),
        terminal::Clear(ClearType::All),
        Print(frame)
    )?;
    out.flush()?;
    Ok(())
}
