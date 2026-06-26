//! Actor — 빠른 루프의 실행자.
//!
//! 역할은 의도적으로 얇다: **현재 policy에서 액션을 샘플링해 게임에 전송**할 뿐이다.
//! 무거운 추론(LLM)은 여기 절대 끼어들지 않는다 — 이 얇음이 "빠른 루프에 LLM 해석이
//! 들어오지 않는다"는 원칙을 코드 구조로 강제한다.
//!
//! Actor는 [`PolicyManager`]를 소유한다. 느린 루프(LLM/Critics)는 [`set_policy`](Actor::set_policy)
//! 로 policy를 통째 교체하고, 빠른 루프는 [`act`](Actor::act)로 매 틱 액션을 낸다.

use std::io;

use serde_json::Value;

use crate::game_interface::GameInterface;
use crate::policy::{Policy, PolicyManager};

pub struct Actor {
    policy: PolicyManager,
}

impl Actor {
    /// 빈 Actor를 만든다 (아직 policy 없음). `seed`는 샘플링 난수 시드.
    pub fn new(seed: u64) -> Self {
        Self {
            policy: PolicyManager::new(seed),
        }
    }

    /// 현재 policy를 새 것으로 교체한다 (느린 루프가 LLM 생성 결과를 주입).
    pub fn set_policy(&mut self, policy: Box<dyn Policy>) {
        self.policy.set_policy(policy);
    }

    /// policy가 설정돼 있는지.
    pub fn has_policy(&self) -> bool {
        self.policy.has_policy()
    }

    /// 빠른 루프 한 스텝: 관측 상태로부터 액션을 결정해 게임에 전송한다.
    ///
    /// 보낼 액션이 결정되면 `true`, policy가 없거나 분포가 비어 액션이 없으면 `false`.
    pub fn act(&mut self, state: &Value, game: &mut GameInterface) -> io::Result<bool> {
        match self.policy.decide(state) {
            Some(command) => {
                game.send_action(command)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conn::Listener;
    use crate::conn_message::Message;
    use crate::policy_discrete::{Categorical, DiscretePolicy, WeightedAction};
    use serde_json::json;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn act_without_policy_sends_nothing() {
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let game_side = thread::spawn(move || {
            let _conn = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(50));
        });

        let mut game = GameInterface::connect(addr).unwrap();
        let mut actor = Actor::new(1);
        assert!(!actor.has_policy());
        assert_eq!(actor.act(&json!({ "hp": 100 }), &mut game).unwrap(), false);

        drop(game);
        game_side.join().unwrap();
    }

    #[test]
    fn act_samples_and_sends() {
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        // 게임 측: 액션 하나를 받아 command를 돌려준다.
        let game_side = thread::spawn(move || {
            let mut conn = listener.accept().unwrap();
            let bytes = conn.recv().unwrap().unwrap();
            match Message::from_bytes(&bytes).unwrap() {
                Message::Action(a) => a.command,
                _ => panic!("expected action"),
            }
        });

        let mut game = GameInterface::connect(addr).unwrap();
        let mut actor = Actor::new(1);
        actor.set_policy(Box::new(DiscretePolicy {
            rules: vec![],
            fallback: Categorical {
                choices: vec![WeightedAction { command: json!({ "key": "jump" }), weight: 1.0 }],
            },
        }));

        assert_eq!(actor.act(&json!({ "hp": 100 }), &mut game).unwrap(), true);
        let received = game_side.join().unwrap();
        assert_eq!(received, json!({ "key": "jump" }));
    }
}
