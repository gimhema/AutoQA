//! MiniChess 코어 룰과 게임 상태.
//!
//! 규칙:
//! - 기물은 King(진영당 1개)과 Pawn(개수 설정 가능)뿐이다.
//! - 두 기물의 이동 방식은 동일: 매 턴 상하좌우 중 한 방향으로 1칸.
//! - 상대 기물이 있는 칸으로 이동하면 그 기물을 잡는다(capture).
//! - **King이 잡히면 즉시 승패가 갈린다.** 그 외 승리 조건은 없다.
//! - 필드 크기와 Pawn 개수는 게임 시작 전에 정한다.

use std::fmt;

/// 두 플레이어.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Player {
    White,
    Black,
}

impl Player {
    pub fn other(self) -> Player {
        match self {
            Player::White => Player::Black,
            Player::Black => Player::White,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Player::White => "White",
            Player::Black => "Black",
        }
    }
}

/// 기물 종류. 이동은 동일하고, 승패는 King 여부로만 갈린다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Pawn,
    King,
}

/// 한 칸에 놓인 기물.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Piece {
    pub owner: Player,
    pub kind: Kind,
}

/// 보드 좌표. `(0,0)`은 좌상단, x는 열(→), y는 행(↓).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// 게임 시작 설정. 두 진영에 동일하게 적용된다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub width: i32,
    pub height: i32,
    /// 진영당 Pawn 개수 (King 1개는 별도).
    pub pawns: i32,
}

impl Config {
    /// 설정이 유효한지 검사한다.
    ///
    /// 각 진영은 자기 쪽 절반(`height/2`행)에만 배치되므로, 진영당 기물 수
    /// `pawns + 1`이 그 영역 안에 들어가야 한다.
    pub fn validate(&self) -> Result<(), String> {
        if self.width < 2 || self.height < 2 {
            return Err(format!(
                "필드가 너무 작습니다: {}x{} (최소 2x2)",
                self.width, self.height
            ));
        }
        if self.pawns < 0 {
            return Err("Pawn 개수는 0 이상이어야 합니다".into());
        }
        let per_side = self.pawns + 1; // + King
        let side_capacity = self.width * (self.height / 2);
        if per_side > side_capacity {
            return Err(format!(
                "기물이 너무 많습니다: 진영당 {}개인데 진영 수용량은 {}칸입니다 \
                 (필드를 키우거나 Pawn을 줄이세요)",
                per_side, side_capacity
            ));
        }
        Ok(())
    }
}

/// 이동 시도의 실패 사유.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MoveError {
    GameOver,
    OutOfBounds,
    NoPieceAtFrom,
    NotYourPiece,
    NotAdjacent,
    OwnPieceAtTarget,
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            MoveError::GameOver => "게임이 이미 끝났습니다",
            MoveError::OutOfBounds => "보드 밖입니다",
            MoveError::NoPieceAtFrom => "출발 칸에 기물이 없습니다",
            MoveError::NotYourPiece => "자신의 기물이 아닙니다",
            MoveError::NotAdjacent => "상하좌우 1칸만 이동할 수 있습니다",
            MoveError::OwnPieceAtTarget => "아군 기물이 있는 칸입니다",
        };
        f.write_str(s)
    }
}

/// 이동 성공 결과.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MoveOutcome {
    /// 이 이동으로 잡힌 상대 기물 (없으면 `None`).
    pub captured: Option<Piece>,
    /// 이 이동으로 게임이 끝났다면 승자.
    pub winner: Option<Player>,
}

/// 보드와 진행 상태.
#[derive(Clone)]
pub struct Board {
    width: i32,
    height: i32,
    cells: Vec<Option<Piece>>,
    turn: Player,
    winner: Option<Player>,
}

impl Board {
    /// 설정에 따라 초기 배치된 보드를 만든다.
    ///
    /// White는 하단(아래 행부터), Black은 상단(위 행부터) 자기 영역을 채운다.
    /// King은 각 진영 뒷줄(가장 바깥 행) 중앙에 놓이고 나머지는 Pawn으로 채운다.
    pub fn new(config: Config) -> Self {
        let mut board = Board {
            width: config.width,
            height: config.height,
            cells: vec![None; (config.width * config.height) as usize],
            turn: Player::White,
            winner: None,
        };
        board.place_side(Player::White, config.pawns);
        board.place_side(Player::Black, config.pawns);
        board
    }

    /// 한 진영의 기물을 배치한다.
    fn place_side(&mut self, player: Player, pawns: i32) {
        // 뒷줄(자기 영역의 가장 바깥 행)과 안쪽으로의 진행 방향.
        let (back_row, step) = match player {
            Player::White => (self.height - 1, -1), // 하단, 위로 채움
            Player::Black => (0, 1),                // 상단, 아래로 채움
        };
        let king_x = self.width / 2;

        // 채울 순서: 뒷줄부터 안쪽으로, 각 줄은 좌→우.
        let total = pawns + 1;
        let mut placed = 0;
        let mut row = back_row;
        // 먼저 King을 뒷줄 중앙에.
        self.set(Pos::new(king_x, back_row), Some(Piece { owner: player, kind: Kind::King }));
        placed += 1;

        let mut x = 0;
        while placed < total {
            if x >= self.width {
                x = 0;
                row += step;
            }
            let pos = Pos::new(x, row);
            if self.get(pos).is_none() {
                self.set(pos, Some(Piece { owner: player, kind: Kind::Pawn }));
                placed += 1;
            }
            x += 1;
        }
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn turn(&self) -> Player {
        self.turn
    }

    pub fn winner(&self) -> Option<Player> {
        self.winner
    }

    pub fn is_over(&self) -> bool {
        self.winner.is_some()
    }

    pub fn in_bounds(&self, pos: Pos) -> bool {
        pos.x >= 0 && pos.x < self.width && pos.y >= 0 && pos.y < self.height
    }

    /// 칸의 기물을 조회한다. 보드 밖이면 `None`.
    pub fn get(&self, pos: Pos) -> Option<Piece> {
        if !self.in_bounds(pos) {
            return None;
        }
        self.cells[(pos.y * self.width + pos.x) as usize]
    }

    fn set(&mut self, pos: Pos, piece: Option<Piece>) {
        let idx = (pos.y * self.width + pos.x) as usize;
        self.cells[idx] = piece;
    }

    /// `from`에서 `to`로 이동을 시도한다. 성공하면 상태를 갱신하고 결과를 돌려준다.
    ///
    /// 규칙 검증: 게임 진행 중 / 양 칸 보드 안 / 출발 칸에 현재 턴의 기물 /
    /// 상하좌우 1칸 인접 / 도착 칸이 아군이 아님. 상대 King을 잡으면 승리.
    pub fn apply_move(&mut self, from: Pos, to: Pos) -> Result<MoveOutcome, MoveError> {
        if self.winner.is_some() {
            return Err(MoveError::GameOver);
        }
        if !self.in_bounds(from) || !self.in_bounds(to) {
            return Err(MoveError::OutOfBounds);
        }
        let piece = self.get(from).ok_or(MoveError::NoPieceAtFrom)?;
        if piece.owner != self.turn {
            return Err(MoveError::NotYourPiece);
        }
        // 상하좌우 1칸: 맨해튼 거리 정확히 1.
        if (from.x - to.x).abs() + (from.y - to.y).abs() != 1 {
            return Err(MoveError::NotAdjacent);
        }
        let target = self.get(to);
        if let Some(t) = target {
            if t.owner == self.turn {
                return Err(MoveError::OwnPieceAtTarget);
            }
        }

        // 이동 적용.
        self.set(to, Some(piece));
        self.set(from, None);

        // 상대 King을 잡았으면 승리.
        let winner = match target {
            Some(t) if t.kind == Kind::King => {
                self.winner = Some(self.turn);
                Some(self.turn)
            }
            _ => None,
        };

        if winner.is_none() {
            self.turn = self.turn.other();
        }

        Ok(MoveOutcome {
            captured: target,
            winner,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_pieces(board: &Board, owner: Player, kind: Kind) -> usize {
        let mut n = 0;
        for y in 0..board.height() {
            for x in 0..board.width() {
                if let Some(p) = board.get(Pos::new(x, y)) {
                    if p.owner == owner && p.kind == kind {
                        n += 1;
                    }
                }
            }
        }
        n
    }

    #[test]
    fn config_validation() {
        assert!(Config { width: 5, height: 5, pawns: 3 }.validate().is_ok());
        assert!(Config { width: 1, height: 5, pawns: 0 }.validate().is_err()); // 너무 작음
        assert!(Config { width: 3, height: 2, pawns: 100 }.validate().is_err()); // 너무 많음
        assert!(Config { width: 3, height: 4, pawns: -1 }.validate().is_err()); // 음수
    }

    #[test]
    fn initial_placement_has_one_king_and_n_pawns_each() {
        let board = Board::new(Config { width: 5, height: 5, pawns: 3 });
        for player in [Player::White, Player::Black] {
            assert_eq!(count_pieces(&board, player, Kind::King), 1);
            assert_eq!(count_pieces(&board, player, Kind::Pawn), 3);
        }
        // White는 하단, Black은 상단.
        assert_eq!(board.get(Pos::new(2, 4)).unwrap().owner, Player::White);
        assert_eq!(board.get(Pos::new(2, 0)).unwrap().owner, Player::Black);
        // King 위치 (뒷줄 중앙).
        assert_eq!(board.get(Pos::new(2, 4)).unwrap().kind, Kind::King);
        assert_eq!(board.get(Pos::new(2, 0)).unwrap().kind, Kind::King);
    }

    #[test]
    fn white_moves_first_then_alternates() {
        let mut board = Board::new(Config { width: 3, height: 4, pawns: 0 });
        assert_eq!(board.turn(), Player::White);
        // White King at (1,3) → 위로.
        board.apply_move(Pos::new(1, 3), Pos::new(1, 2)).unwrap();
        assert_eq!(board.turn(), Player::Black);
    }

    #[test]
    fn rejects_non_adjacent_and_diagonal() {
        let mut board = Board::new(Config { width: 4, height: 4, pawns: 0 });
        // 대각선 이동 거부.
        assert_eq!(
            board.apply_move(Pos::new(2, 3), Pos::new(3, 2)),
            Err(MoveError::NotAdjacent)
        );
        // 2칸 이동 거부.
        assert_eq!(
            board.apply_move(Pos::new(2, 3), Pos::new(2, 1)),
            Err(MoveError::NotAdjacent)
        );
    }

    #[test]
    fn rejects_moving_opponent_piece() {
        let mut board = Board::new(Config { width: 3, height: 4, pawns: 0 });
        // White 턴에 Black King(1,0) 이동 시도.
        assert_eq!(
            board.apply_move(Pos::new(1, 0), Pos::new(1, 1)),
            Err(MoveError::NotYourPiece)
        );
    }

    #[test]
    fn capturing_king_wins() {
        // 3x3, pawn 없음. King끼리 붙여 잡기.
        let mut board = Board::new(Config { width: 3, height: 3, pawns: 0 });
        // White King (1,2), Black King (1,0). 중앙(1,1)에서 만나게 한다.
        board.apply_move(Pos::new(1, 2), Pos::new(1, 1)).unwrap(); // White 위로
        assert_eq!(board.turn(), Player::Black);
        board.apply_move(Pos::new(1, 0), Pos::new(1, 1)).unwrap(); // Black이 White King 잡음
        assert_eq!(board.winner(), Some(Player::Black));
        assert!(board.is_over());
        // 게임 종료 후 이동 거부.
        assert_eq!(
            board.apply_move(Pos::new(1, 1), Pos::new(0, 1)),
            Err(MoveError::GameOver)
        );
    }

    #[test]
    fn capture_returns_captured_piece() {
        let mut board = Board::new(Config { width: 3, height: 3, pawns: 0 });
        board.apply_move(Pos::new(1, 2), Pos::new(1, 1)).unwrap();
        let outcome = board.apply_move(Pos::new(1, 0), Pos::new(1, 1)).unwrap();
        assert_eq!(outcome.captured, Some(Piece { owner: Player::White, kind: Kind::King }));
        assert_eq!(outcome.winner, Some(Player::Black));
    }

    #[test]
    fn out_of_bounds_rejected() {
        let mut board = Board::new(Config { width: 3, height: 3, pawns: 0 });
        assert_eq!(
            board.apply_move(Pos::new(1, 2), Pos::new(1, 3)),
            Err(MoveError::OutOfBounds)
        );
    }
}
