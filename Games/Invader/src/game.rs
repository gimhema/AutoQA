//! Invader 코어 시뮬레이션. 플레이어 이동, 발사 쿨다운, 탄환/블록 충돌을 담당한다.
//!
//! 렌더링(`render.rs`)과 조작 방식(사람 키보드 / Ouroboros 액션, `main.rs` /
//! `ouroboros.rs`)은 이 모듈에 의존하지 않는다 — 이 모듈은 순수 게임 상태다.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use rand::seq::SliceRandom;
use rand::thread_rng;

pub const WIDTH: i32 = 30;
pub const HEIGHT: i32 = 20;
pub const BLOCK_ROWS: i32 = 5;
pub const TICK_MS: u64 = 50;
pub const SHOOT_COOLDOWN: Duration = Duration::from_millis(400);

pub struct Bullet {
    pub x: i32,
    pub y: i32,
}

pub struct Config {
    pub time_limit: Duration,
    pub block_count: usize,
}

impl Config {
    pub fn max_blocks() -> usize {
        (WIDTH * BLOCK_ROWS) as usize
    }
}

pub enum Outcome {
    Ongoing,
    Win,
    Lose,
}

pub struct Game {
    pub player_x: i32,
    pub player_y: i32,
    pub blocks: HashSet<(i32, i32)>,
    pub bullets: Vec<Bullet>,
    pub total_blocks: usize,
    last_shot: Instant,
    start: Instant,
    time_limit: Duration,
}

impl Game {
    pub fn new(config: &Config) -> Self {
        let mut rng = thread_rng();
        let mut positions: Vec<(i32, i32)> = (0..WIDTH)
            .flat_map(|x| (0..BLOCK_ROWS).map(move |y| (x, y)))
            .collect();
        positions.shuffle(&mut rng);
        let blocks: HashSet<(i32, i32)> =
            positions.into_iter().take(config.block_count).collect();
        let total_blocks = blocks.len();

        Game {
            player_x: WIDTH / 2,
            player_y: HEIGHT - 1,
            blocks,
            bullets: Vec::new(),
            total_blocks,
            last_shot: Instant::now()
                .checked_sub(SHOOT_COOLDOWN)
                .unwrap_or_else(Instant::now),
            start: Instant::now(),
            time_limit: config.time_limit,
        }
    }

    pub fn move_left(&mut self) {
        self.player_x = (self.player_x - 1).max(0);
    }

    pub fn move_right(&mut self) {
        self.player_x = (self.player_x + 1).min(WIDTH - 1);
    }

    pub fn can_shoot(&self) -> bool {
        self.last_shot.elapsed() >= SHOOT_COOLDOWN
    }

    /// 발사를 시도한다. 쿨다운 중이면 아무 일도 하지 않고 `false`를 반환한다.
    pub fn shoot(&mut self) -> bool {
        if !self.can_shoot() {
            return false;
        }
        self.bullets.push(Bullet {
            x: self.player_x,
            y: self.player_y - 1,
        });
        self.last_shot = Instant::now();
        true
    }

    /// 주어진 열(x)에 남아있는 블록 수.
    pub fn blocks_in_column(&self, x: i32) -> usize {
        self.blocks.iter().filter(|b| b.0 == x).count()
    }

    pub fn remaining(&self) -> Duration {
        self.time_limit.saturating_sub(self.start.elapsed())
    }

    /// 탄환 이동과 충돌을 한 틱만큼 갱신하고, 승패가 갈렸으면 결과를 반환한다.
    pub fn tick(&mut self) -> Outcome {
        let mut i = 0;
        while i < self.bullets.len() {
            self.bullets[i].y -= 1;
            let pos = (self.bullets[i].x, self.bullets[i].y);
            if self.bullets[i].y < 0 {
                self.bullets.remove(i);
            } else if self.blocks.remove(&pos) {
                self.bullets.remove(i);
            } else {
                i += 1;
            }
        }

        if self.blocks.is_empty() {
            return Outcome::Win;
        }
        if self.remaining() == Duration::ZERO {
            return Outcome::Lose;
        }
        Outcome::Ongoing
    }
}
