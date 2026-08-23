//! Ouroboros QA 에이전트와의 연동 (`ai` 서브커맨드).
//!
//! Minion은 실시간 게임이라 매 프레임 관측을 보내고, 그 시점에 도착해 있는
//! 최신 액션을 그대로 적용한다 (Invader와 동일한 패턴). Ouroboros가 게임 전체를
//! 담당하며, 창은 관전용으로만 쓰인다 (Esc/Q로 조기 종료만 가능).
//!
//! # 관측 포맷
//! ```json
//! {
//!   "flow": "playing" | "choosing_perk" | "victory",
//!   "player": {"x":.., "y":.., "hp":.., "aim_angle":..},
//!   "enemies": [{"id":.., "x":.., "y":.., "hp":.., "is_boss":bool}, ...],
//!   "stage": {"number":.., "kills":.., "boss_threshold":.., "boss_spawned":bool, "cleared":bool},
//!   "perk_choices": [{"index":1, "type":"INC_ATTACK", "amount":5}, ...],
//!   "valid_actions": [
//!     {"action":"move", "dx":-1, "dy":0},
//!     {"action":"shoot", "target_x":.., "target_y":.., "target_id":.., "dist":.., "is_boss":bool, "target_hp":..},
//!     {"action":"choose_perk", "index":1, "type":"INC_ATTACK", "amount":5}
//!   ]
//! }
//! ```
//! `perk_choices`는 `flow == "choosing_perk"`일 때만 채워진다.
//!
//! `valid_actions`는 이번 관측 시점 기준으로 이미 좌표까지 계산된, 그대로 돌려보내면
//! 되는 액션 후보 목록이다 (`--action-space dynamic`용). `shoot`의 `target_x`/`target_y`는
//! 실시간으로 움직이는 좌표라 정책 생성 시점에 값을 고정할 수 없다 — LLM이 직접
//! 좌표를 계산/추정하게 하는 대신, 매 프레임 게임이 계산한 후보를 조건으로 고르게
//! 한다. `flow == "playing"`일 땐 이동(8방향+제자리) + 살아있는 적마다 하나씩의
//! `shoot`, `flow == "choosing_perk"`일 땐 `choose_perk` 후보만 채워진다.
//!
//! # 액션 포맷 (Ouroboros → 게임)
//! `valid_actions`의 항목 하나를 그대로 반환하면 되지만, 형식은 다음과 같다:
//! - `{"action":"move", "dx":-1..1, "dy":-1..1}` — WASD와 동일한 의미
//! - `{"action":"shoot", "target_x":.., "target_y":..}` — 해당 좌표로 조준 후 발사
//! - `{"action":"choose_perk", "index":1..}` — `flow == "choosing_perk"`일 때만 유효
//! - `{"action":"stay"}` 또는 그 외 — 무시

use macroquad::prelude::*;
use ouroboros_link::OuroborosLink;
use serde_json::{json, Value};

use crate::game::{GameFlowState, GameSession};

pub async fn run(port : u16) {
    println!("[Minion] 포트 {port}에서 Ouroboros 접속 대기 중…");
    let mut link = match OuroborosLink::accept(("0.0.0.0", port)) {
        Ok(link) => link,
        Err(e) => {
            eprintln!("[Minion] Ouroboros 접속 대기 실패: {e}");
            return;
        }
    };
    println!("[Minion] Ouroboros 연결됨! 게임 시작 (관전 모드, Esc/Q로 종료)");

    let mut session = GameSession::New();

    loop {
        if !link.is_connected() {
            println!("[Minion] Ouroboros 연결이 끊겨 게임을 종료합니다.");
            break;
        }
        if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Q) {
            println!("[Minion] 관전 모드를 종료합니다.");
            break;
        }

        if let Some(action) = link.poll_action() {
            ApplyAction(&mut session, &action.command);
        }

        session.Tick();
        session.Render();
        draw_text("(관전 모드, Ouroboros 조종 중 · Esc 종료)", 10.0, screen_height() - 16.0, 18.0, GRAY);

        if let Err(e) = link.send_observation(Observation(&session)) {
            eprintln!("[Minion] 관측 전송 실패: {e}");
            break;
        }

        next_frame().await;
    }
}

fn ApplyAction(session : &mut GameSession, command : &Value) {
    match command.get("action").and_then(|v| v.as_str()) {
        Some("move") => {
            let dx = command.get("dx").and_then(|v| v.as_i64()).unwrap_or(0).clamp(-1, 1) as i32;
            let dy = command.get("dy").and_then(|v| v.as_i64()).unwrap_or(0).clamp(-1, 1) as i32;
            session.player.Move(&mut session.world, dx, dy);
        }
        Some("shoot") => {
            if let (Some(tx), Some(ty)) = (
                command.get("target_x").and_then(|v| v.as_f64()),
                command.get("target_y").and_then(|v| v.as_f64())
            ) {
                session.player.AimAt(&mut session.world, tx as f32, ty as f32);
            }
            session.player.Shoot(&mut session.world);
        }
        Some("choose_perk") => {
            if let Some(index) = command.get("index").and_then(|v| v.as_u64()) {
                session.ChoosePerk(index as usize);
            }
        }
        _ => {}
    }
}

fn Observation(session : &GameSession) -> Value {
    let boss_id = session.manager.Peek().and_then(|s| s.boss_id);

    let enemies : Vec<Value> = session.world.minions.iter()
        .filter(|m| m.id != session.player_id)
        .map(|m| json!({
            "id": m.id,
            "x": m.actorInfo.geometry.x,
            "y": m.actorInfo.geometry.y,
            "hp": m.actorInfo.status.health,
            "is_boss": Some(m.id) == boss_id
        }))
        .collect();

    let player = session.world.GetMinion(session.player_id).map(|p| json!({
        "x": p.actorInfo.geometry.x,
        "y": p.actorInfo.geometry.y,
        "hp": p.actorInfo.status.health,
        "aim_angle": session.player.aim_angle
    }));

    let stage = session.manager.Peek().map(|s| json!({
        "number": session.manager.current_index + 1,
        "kills": s.kill_count,
        "boss_threshold": s.boss_threshold,
        "boss_spawned": s.boss_spawned,
        "cleared": s.cleared
    }));

    let perk_choices : Vec<Value> = if session.flow == GameFlowState::ChoosingPerk {
        session.perk_choices.iter().enumerate()
            .map(|(i, p)| json!({ "index": i + 1, "type": format!("{:?}", p.perk_type), "amount": p.amount }))
            .collect()
    } else {
        Vec::new()
    };

    json!({
        "flow": match session.flow {
            GameFlowState::Playing => "playing",
            GameFlowState::ChoosingPerk => "choosing_perk",
            GameFlowState::Victory => "victory"
        },
        "player": player,
        "enemies": enemies,
        "stage": stage,
        "perk_choices": perk_choices,
        "valid_actions": ValidActions(session, boss_id)
    })
}

// 이번 관측 시점 기준으로 좌표까지 이미 계산된 액션 후보 목록.
// `--action-space dynamic` policy가 계산 없이 조건만으로 고를 수 있도록 하기 위함.
const MOVE_DIRECTIONS : [(i64, i64); 9] = [
    (-1, -1), (0, -1), (1, -1),
    (-1, 0),           (1, 0),
    (-1, 1),  (0, 1),  (1, 1),
    (0, 0)
];

fn ValidActions(session : &GameSession, boss_id : Option<usize>) -> Vec<Value> {
    match session.flow {
        GameFlowState::Playing => {
            let mut actions : Vec<Value> = MOVE_DIRECTIONS.iter()
                .map(|(dx, dy)| json!({ "action": "move", "dx": dx, "dy": dy }))
                .collect();

            if let Some(player) = session.world.GetMinion(session.player_id) {
                let px = player.actorInfo.geometry.x;
                let py = player.actorInfo.geometry.y;

                for enemy in session.world.minions.iter().filter(|m| m.id != session.player_id) {
                    let dx = (enemy.actorInfo.geometry.x - px) as f32;
                    let dy = (enemy.actorInfo.geometry.y - py) as f32;
                    actions.push(json!({
                        "action": "shoot",
                        "target_x": enemy.actorInfo.geometry.x,
                        "target_y": enemy.actorInfo.geometry.y,
                        "target_id": enemy.id,
                        "dist": (dx * dx + dy * dy).sqrt(),
                        "is_boss": Some(enemy.id) == boss_id,
                        "target_hp": enemy.actorInfo.status.health
                    }));
                }
            }

            actions
        }
        GameFlowState::ChoosingPerk => {
            session.perk_choices.iter().enumerate()
                .map(|(i, p)| json!({
                    "action": "choose_perk",
                    "index": i + 1,
                    "type": format!("{:?}", p.perk_type),
                    "amount": p.amount
                }))
                .collect()
        }
        GameFlowState::Victory => Vec::new()
    }
}
