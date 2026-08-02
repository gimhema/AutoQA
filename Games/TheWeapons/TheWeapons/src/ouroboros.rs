//! Ouroboros QA 에이전트와의 연동 (`ai` 서브커맨드).
//!
//! TheWeapons는 host/join과 달리 네트워크로 두 사람을 잇지 않는다 — 한 프로세스 안에서
//! 사람(터미널)과 Ouroboros(TCP)가 각각 한 진영을 맡는다. 이 게임은 **동시 공개**이므로
//! 매 턴 순서가 중요하다: 사람의 입력을 먼저 받아두더라도, Ouroboros에게는 그 입력을
//! 절대 보여주지 않는다 — Ouroboros의 관측은 이번 턴 시작 시점의 상태(HP·손패)만 담고,
//! 사람이 이번 턴에 무엇을 냈는지는 판정 전까지 알 수 없다. 그래서 프로그램 순서상
//! 사람 입력을 먼저 읽어도 정보 공정성은 깨지지 않는다.
//!
//! # 관측 포맷 (Ouroboros Dynamic policy용, README에 사전 정의된 스키마)
//! ```json
//! {
//!   "my_hp": 7, "opponent_hp": 5,
//!   "my_hand": {"sword": 3, "shield": 2, "spear": 1},
//!   "socket_count": 3,
//!   "valid_actions": [
//!     {"slots": ["sword", "shield", "empty"]},
//!     {"slots": ["spear", "empty", "empty"]}
//!   ]
//! }
//! ```
//! `valid_actions`는 현재 손패로 감당 가능한 소켓 배치 조합 전체다(형식 검증은 게임이
//! 미리 다 해준다 — LLM은 그중 하나를 그대로 고르기만 하면 된다).
//!
//! # 액션 포맷 (Ouroboros → 게임)
//! `valid_actions` 항목을 그대로 반환한다: `{"slots": [...]}`.

use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use ouroboros_link::OuroborosLink;
use serde_json::{json, Value};

use crate::cards::{Card, Hand};
use crate::game::{Config, Match};
use crate::render;

/// `valid_actions` 열거가 지나치게 커지는 것을 막는 안전판. 소켓 수가 크면
/// 조합(최대 4^socket_count)이 기하급수적으로 늘어나므로, 이 개수에서 잘라낸다.
const MAX_VALID_ACTIONS: usize = 2048;

pub struct AiConfig {
    pub game_config: Config,
    pub ouroboros_port: u16,
}

/// AI 모드 메인 루프. 사람은 터미널로, Ouroboros는 TCP로 각각 한 진영을 맡는다.
pub fn run(config: AiConfig) -> io::Result<()> {
    eprintln!(
        "[TheWeapons] 포트 {}에서 Ouroboros 접속 대기 중…",
        config.ouroboros_port
    );
    let mut link = OuroborosLink::accept(("0.0.0.0", config.ouroboros_port))?;
    eprintln!("[TheWeapons] Ouroboros 연결됨! 게임 시작 (당신 vs Ouroboros)");

    let mut m = Match::new(config.game_config);

    loop {
        print!("{}{}", render::CLEAR, render::render(&m));
        io::stdout().flush()?;

        if m.is_over() {
            return Ok(());
        }

        let human_play = loop {
            match crate::read_play(&m) {
                Ok(Some(play)) => break play,
                Ok(None) => {
                    println!("게임을 종료합니다.");
                    return Ok(());
                }
                Err(e) => println!("입력 오류: {e}. 다시 입력하세요."),
            }
        };

        let ouroboros_play = ai_turn(&m, &mut link)?;
        m.apply_turn(human_play, ouroboros_play);
    }
}

/// 유효한 액션이 올 때까지 관측을 반복 전송한다.
///
/// 사람 턴 중에도 Ouroboros는 계속 관측을 받지만, 이 함수는 사람이 입력을 마친
/// "이번 턴" 시점에 호출되어 그 순간의 상태로 관측을 만든다 — 사람이 이번 턴 낸
/// 카드는 이 관측에 담기지 않는다(동시 공개 유지).
fn ai_turn(m: &Match, link: &mut OuroborosLink) -> io::Result<Vec<Option<Card>>> {
    println!("Ouroboros 생각 중…");

    loop {
        if !link.is_connected() {
            eprintln!("[TheWeapons] Ouroboros 연결 끊김");
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Ouroboros disconnected",
            ));
        }

        link.send_observation(observation(m))?;

        if let Some(action) = link.poll_action() {
            match parse_action(&action.command, m.config.socket_count) {
                Some(play) if m.opp_hand.can_afford(&play) => {
                    eprintln!("[Ouroboros] 배치: {}", describe(&play));
                    return Ok(play);
                }
                Some(_) => eprintln!("[Ouroboros] 손패를 초과한 배치, 재시도"),
                None => eprintln!("[Ouroboros] 액션 파싱 실패: {}", action.command),
            }
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn describe(play: &[Option<Card>]) -> String {
    play.iter().map(|c| slot_str(*c)).collect::<Vec<_>>().join(",")
}

/// 게임 상태를 Ouroboros 시점(자신=Ouroboros, 상대=사람)의 관측 JSON으로 직렬화한다.
fn observation(m: &Match) -> Value {
    json!({
        "my_hp": m.opp_hp,
        "opponent_hp": m.my_hp,
        "my_hand": {
            "sword": m.opp_hand.sword,
            "shield": m.opp_hand.shield,
            "spear": m.opp_hand.spear,
        },
        "socket_count": m.config.socket_count,
        "valid_actions": enumerate_valid_actions(m.config.socket_count, &m.opp_hand),
    })
}

fn slot_str(card: Option<Card>) -> &'static str {
    match card {
        None => "empty",
        Some(c) => c.as_str(),
    }
}

/// 현재 손패로 감당 가능한 소켓 배치 조합을 전부 나열한다.
fn enumerate_valid_actions(socket_count: usize, hand: &Hand) -> Vec<Value> {
    let mut out = Vec::new();
    let mut current = vec![None; socket_count];
    enumerate_rec(0, hand, &mut current, &mut out);
    out
}

fn enumerate_rec(idx: usize, hand: &Hand, current: &mut [Option<Card>], out: &mut Vec<Value>) {
    if out.len() >= MAX_VALID_ACTIONS {
        return;
    }
    if idx == current.len() {
        if hand.can_afford(current) {
            out.push(json!({
                "slots": current.iter().map(|c| slot_str(*c)).collect::<Vec<_>>(),
            }));
        }
        return;
    }
    for choice in [None, Some(Card::Sword), Some(Card::Shield), Some(Card::Spear)] {
        current[idx] = choice;
        enumerate_rec(idx + 1, hand, current, out);
        if out.len() >= MAX_VALID_ACTIONS {
            return;
        }
    }
}

/// `{"slots": [...]}`를 파싱한다. 슬롯 값은 `null`/`"empty"`(빈 소켓) 또는
/// `"sword"`/`"shield"`/`"spear"`. 소켓 수가 다르거나 알 수 없는 값이면 `None`.
fn parse_action(command: &Value, socket_count: usize) -> Option<Vec<Option<Card>>> {
    let slots = command.get("slots")?.as_array()?;
    if slots.len() != socket_count {
        return None;
    }
    let mut play = Vec::with_capacity(slots.len());
    for slot in slots {
        let card = match slot {
            Value::Null => None,
            Value::String(s) if s == "empty" => None,
            Value::String(s) => Some(Card::parse_str(s)?),
            _ => return None,
        };
        play.push(card);
    }
    Some(play)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_action_reads_slots() {
        let cmd = json!({"slots": ["sword", "empty", "spear"]});
        assert_eq!(
            parse_action(&cmd, 3),
            Some(vec![Some(Card::Sword), None, Some(Card::Spear)])
        );
    }

    #[test]
    fn parse_action_accepts_null_as_empty() {
        let cmd = json!({"slots": [null, "shield"]});
        assert_eq!(parse_action(&cmd, 2), Some(vec![None, Some(Card::Shield)]));
    }

    #[test]
    fn parse_action_rejects_wrong_socket_count() {
        let cmd = json!({"slots": ["sword"]});
        assert_eq!(parse_action(&cmd, 2), None);
    }

    #[test]
    fn parse_action_rejects_unknown_card() {
        let cmd = json!({"slots": ["axe"]});
        assert_eq!(parse_action(&cmd, 1), None);
    }

    #[test]
    fn enumerate_all_combinations_within_ample_hand() {
        let hand = Hand::new(5, 5, 5);
        let actions = enumerate_valid_actions(2, &hand);
        // 4 choices per socket (empty/sword/shield/spear), 2 sockets, ample hand → 16.
        assert_eq!(actions.len(), 16);
    }

    #[test]
    fn enumerate_filters_by_hand_limits() {
        // 검 0장이면 검을 포함한 조합은 전부 제외된다.
        let hand = Hand::new(0, 5, 5);
        let actions = enumerate_valid_actions(1, &hand);
        assert_eq!(actions.len(), 3); // empty/shield/spear
        for action in &actions {
            let slots = action["slots"].as_array().unwrap();
            assert_ne!(slots[0], json!("sword"));
        }
    }
}
