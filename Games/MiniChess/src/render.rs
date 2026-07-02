//! 보드의 CLI 렌더링. ANSI 색상 + 유니코드 기물로 시각화한다.
//!
//! - White 기물은 밝게, Black 기물은 청록으로 구분.
//! - King(♚/♔)과 Pawn(♟/♙)을 유니코드로 표기.
//! - 체커보드 배경으로 칸 경계를 시각화하고, 열/행 좌표를 함께 출력한다.

use crate::game::{Board, Kind, Piece, Player, Pos};

// ANSI escape codes.
const RESET: &str = "\x1b[0m";
const WHITE_FG: &str = "\x1b[97;1m"; // 밝은 흰색 굵게
const BLACK_FG: &str = "\x1b[96;1m"; // 밝은 청록 굵게
const LIGHT_BG: &str = "\x1b[48;5;180m"; // 밝은 칸
const DARK_BG: &str = "\x1b[48;5;94m"; // 어두운 칸
const COORD: &str = "\x1b[90m"; // 좌표 라벨(회색)

fn glyph(piece: Piece) -> char {
    match (piece.owner, piece.kind) {
        (Player::White, Kind::King) => '♔',
        (Player::White, Kind::Pawn) => '♙',
        (Player::Black, Kind::King) => '♚',
        (Player::Black, Kind::Pawn) => '♟',
    }
}

fn fg(owner: Player) -> &'static str {
    match owner {
        Player::White => WHITE_FG,
        Player::Black => BLACK_FG,
    }
}

/// 보드를 색상 문자열로 렌더링한다.
pub fn render(board: &Board) -> String {
    let mut out = String::new();
    let w = board.width();
    let h = board.height();

    // 상단 열 좌표.
    out.push_str("   ");
    for x in 0..w {
        out.push_str(&format!("{COORD} {x:>2}{RESET}"));
    }
    out.push('\n');

    for y in 0..h {
        // 행 좌표.
        out.push_str(&format!("{COORD}{y:>2} {RESET}"));
        for x in 0..w {
            let bg = if (x + y) % 2 == 0 { LIGHT_BG } else { DARK_BG };
            let cell = match board.get(Pos::new(x, y)) {
                Some(p) => format!("{bg}{} {} {RESET}", fg(p.owner), glyph(p)),
                None => format!("{bg}   {RESET}"),
            };
            out.push_str(&cell);
        }
        out.push('\n');
    }
    out
}

/// 보드 + 상태 한 줄(턴/승자)을 함께 렌더링한다.
pub fn render_with_status(board: &Board, me: Player) -> String {
    let mut out = render(board);
    out.push('\n');
    match board.winner() {
        Some(winner) => {
            let verdict = if winner == me { "당신의 승리! 🎉" } else { "패배…" };
            out.push_str(&format!(
                "게임 종료 — {}{}{RESET} 승. {verdict}\n",
                fg(winner),
                winner.name()
            ));
        }
        None => {
            let whose = if board.turn() == me { "당신 차례" } else { "상대 차례" };
            out.push_str(&format!(
                "당신: {}{}{RESET}  |  현재 턴: {}{}{RESET} ({whose})\n",
                fg(me),
                me.name(),
                fg(board.turn()),
                board.turn().name()
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Config;

    #[test]
    fn render_contains_all_cells_and_coords() {
        let board = Board::new(Config { width: 3, height: 3, pawns: 0 });
        let s = render(&board);
        // 유니코드 King 글리프가 양측 다 들어가야 한다.
        assert!(s.contains('♔'));
        assert!(s.contains('♚'));
        // 열 좌표 라벨.
        assert!(s.contains('0'));
        assert!(s.contains('2'));
    }

    #[test]
    fn status_shows_winner() {
        let mut board = Board::new(Config { width: 3, height: 3, pawns: 0 });
        board.apply_move(Pos::new(1, 2), Pos::new(1, 1)).unwrap();
        board.apply_move(Pos::new(1, 0), Pos::new(1, 1)).unwrap();
        let s = render_with_status(&board, Player::Black);
        assert!(s.contains("게임 종료"));
        assert!(s.contains("당신의 승리"));
    }

    #[test]
    fn status_shows_turn() {
        let board = Board::new(Config { width: 3, height: 3, pawns: 0 });
        let s = render_with_status(&board, Player::White);
        assert!(s.contains("당신 차례"));
    }
}
