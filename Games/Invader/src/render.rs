//! 터미널 렌더링. raw mode / alternate screen 진입·복원과 매 틱 프레임 출력을 담당한다.

use std::io::{self, Write};

use crossterm::{
    cursor, execute,
    style::Print,
    terminal::{self, ClearType},
};

use crate::game::{Game, HEIGHT, WIDTH};

/// raw mode + alternate screen 진입을 보장하고, drop 시 반드시 복원한다.
pub struct TerminalGuard;

impl TerminalGuard {
    pub fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), terminal::EnterAlternateScreen, cursor::Hide)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

/// 현재 게임 상태를 한 프레임으로 그린다. `header_suffix`는 상태줄 끝에 붙는
/// 조작 안내(사람 모드) 또는 관전 안내(ai 모드) 문구다.
pub fn draw(out: &mut io::Stdout, game: &Game, header_suffix: &str) -> io::Result<()> {
    let remaining_secs = game.remaining().as_secs();
    let mut frame = String::new();
    frame.push_str(&format!(
        "[ Invader ]  남은 시간: {:>3}s   남은 블록: {:>3}/{:<3}   {header_suffix}\r\n",
        remaining_secs,
        game.blocks.len(),
        game.total_blocks
    ));
    frame.push_str(&"-".repeat(WIDTH as usize));
    frame.push_str("\r\n");

    for y in 0..HEIGHT {
        let mut line = String::with_capacity(WIDTH as usize);
        for x in 0..WIDTH {
            let ch = if y == game.player_y && x == game.player_x {
                'A'
            } else if game.blocks.contains(&(x, y)) {
                '#'
            } else if game.bullets.iter().any(|b| b.x == x && b.y == y) {
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
