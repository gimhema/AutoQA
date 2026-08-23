//! Minion — WASD 이동 + 마우스 조준의 실시간 슈팅 게임.
//!
//! 실행:
//!   play (기본): `cargo run` 또는 `cargo run -- play` — 사람이 직접 조작.
//!   ai:          `cargo run -- ai [--ouroboros-port P]` — Ouroboros 에이전트가 대신 플레이.

mod common;
mod common_logic;
mod controller;
mod enemy;
mod minion;
mod config;
mod perks;
mod world;
mod attack;
mod object;
mod stage;
mod game;
mod ouroboros;

use game::{GameFlowState, GameSession};
use macroquad::prelude::*;

const DEFAULT_OUROBOROS_PORT : u16 = 9000;

#[macroquad::main("Minion")]
async fn main() {
    let args : Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("ai") => {
            let port = ParsePortArg(&args).unwrap_or(DEFAULT_OUROBOROS_PORT);
            ouroboros::run(port).await;
        }
        _ => RunPlay().await
    }
}

fn ParsePortArg(args : &[String]) -> Option<u16> {
    let idx = args.iter().position(|a| a == "--ouroboros-port")?;
    args.get(idx + 1)?.parse().ok()
}

async fn RunPlay() {
    let mut session = GameSession::New();
    let perk_keys = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4];

    loop {
        if session.flow == GameFlowState::Playing {
            session.player.Update(&mut session.world);
        } else if session.flow == GameFlowState::ChoosingPerk {
            for (i, key) in perk_keys.iter().enumerate() {
                if is_key_pressed(*key) {
                    session.ChoosePerk(i + 1);
                }
            }
        }

        session.Tick();
        session.Render();

        next_frame().await;
    }
}
