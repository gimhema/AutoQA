//! 우로보로스 에이전트의 최상위 오케스트레이터.
//!
//! 두 루프를 조율한다:
//! - **빠른 루프** (~60Hz): [`GameInterface`]에서 최신 관측을 받아 [`Actor`]로
//!   액션을 만들어 게임에 전송. LLM을 기다리지 않는다.
//! - **느린 루프** (별도 스레드): [`Critics`]가 intent 부합을 평가하고,
//!   필요시 [`policy_gen`]으로 policy를 재생성해 [`Actor`]에 주입한다.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::actor::Actor;
use crate::critics::{self, Verdict};
use crate::game_interface::GameInterface;
use crate::llm_interface::LlmClient;
use crate::policy::Policy;
use crate::policy_gen::{self, ActionSpace};
use crate::status_observer::StatusObserver;

pub struct Agent {
    game: GameInterface,
    actor: Actor,
    intent: String,
    tick: Duration,
    llm: Option<LlmClient>,
    action_space: ActionSpace,
}

impl Agent {
    pub fn connect<A: std::net::ToSocketAddrs>(addr: A, intent: impl Into<String>) -> io::Result<Self> {
        let game = GameInterface::connect(addr)?;
        Ok(Self {
            game,
            actor: Actor::new(0),
            intent: intent.into(),
            tick: Duration::from_millis(16),
            llm: None,
            action_space: ActionSpace::Discrete {
                available_actions: vec![],
            },
        })
    }

    pub fn set_llm(&mut self, llm: LlmClient) {
        self.llm = Some(llm);
    }

    pub fn set_action_space(&mut self, action_space: ActionSpace) {
        self.action_space = action_space;
    }

    pub fn actor_mut(&mut self) -> &mut Actor {
        &mut self.actor
    }

    /// 빠른 루프 + 느린 루프를 실행한다. 게임 연결이 끊기면 종료.
    pub fn run(&mut self) -> io::Result<()> {
        eprintln!("[Agent] intent = {:?}", self.intent);

        let observer = Arc::new(Mutex::new(StatusObserver::new(64)));
        let alive = Arc::new(AtomicBool::new(true));

        let policy_rx = self.spawn_slow_loop(Arc::clone(&observer), Arc::clone(&alive));

        while self.game.is_alive() {
            if let Some(obs) = self.game.take_latest_observation() {
                observer.lock().unwrap().push(&obs);
                self.actor.act(&obs.state, &mut self.game)?;
            }

            if let Some(rx) = &policy_rx {
                if let Ok(new_policy) = rx.try_recv() {
                    eprintln!("[Agent] 느린 루프로부터 새 policy 수신, 교체");
                    self.actor.set_policy(new_policy);
                }
            }

            std::thread::sleep(self.tick);
        }

        alive.store(false, Ordering::Release);
        eprintln!("[Agent] 게임 연결 종료, 루프 중단");
        Ok(())
    }

    /// LLM이 설정돼 있으면 느린 루프 스레드를 시작하고 policy 수신 채널을 반환한다.
    fn spawn_slow_loop(
        &mut self,
        observer: Arc<Mutex<StatusObserver>>,
        alive: Arc<AtomicBool>,
    ) -> Option<mpsc::Receiver<Box<dyn Policy>>> {
        let llm = self.llm.take()?;
        let (tx, rx) = mpsc::channel::<Box<dyn Policy>>();
        let intent = self.intent.clone();
        let action_space = std::mem::replace(
            &mut self.action_space,
            ActionSpace::Discrete { available_actions: vec![] },
        );
        let slow_tick = Duration::from_secs(5);

        thread::spawn(move || {
            slow_loop(llm, intent, action_space, observer, alive, tx, slow_tick);
        });

        Some(rx)
    }
}

fn slow_loop(
    llm: LlmClient,
    intent: String,
    action_space: ActionSpace,
    observer: Arc<Mutex<StatusObserver>>,
    alive: Arc<AtomicBool>,
    policy_tx: mpsc::Sender<Box<dyn Policy>>,
    tick: Duration,
) {
    eprintln!("[SlowLoop] 시작, 간격 = {:?}", tick);

    // 첫 관측이 도착할 때까지 대기.
    while alive.load(Ordering::Acquire) {
        if !observer.lock().unwrap().is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    // 초기 policy 생성.
    if alive.load(Ordering::Acquire) {
        eprintln!("[SlowLoop] 초기 policy 생성 중…");
        let state = observer.lock().unwrap().latest_state().cloned();
        if let Some(state) = state {
            match policy_gen::generate_policy(&llm, &intent, &state, &action_space) {
                Ok(policy) => {
                    let _ = policy_tx.send(policy);
                    eprintln!("[SlowLoop] 초기 policy 전송 완료");
                }
                Err(e) => eprintln!("[SlowLoop] 초기 policy 생성 실패: {e}"),
            }
        }
    }

    // 주기적 평가 루프.
    while alive.load(Ordering::Acquire) {
        thread::sleep(tick);
        if !alive.load(Ordering::Acquire) {
            break;
        }

        let summary = observer.lock().unwrap().summarize();
        let state = observer.lock().unwrap().latest_state().cloned();

        match critics::evaluate(&llm, &intent, &summary) {
            Ok(Verdict::Keep) => {
                eprintln!("[SlowLoop] Critics: KEEP");
            }
            Ok(Verdict::Regenerate { reason }) => {
                eprintln!("[SlowLoop] Critics: REGENERATE — {reason}");
                if let Some(state) = state {
                    match policy_gen::generate_policy(&llm, &intent, &state, &action_space) {
                        Ok(policy) => {
                            if policy_tx.send(policy).is_err() {
                                break;
                            }
                            eprintln!("[SlowLoop] 새 policy 전송 완료");
                        }
                        Err(e) => eprintln!("[SlowLoop] policy 생성 실패: {e}"),
                    }
                }
            }
            Err(e) => eprintln!("[SlowLoop] Critics 평가 실패: {e}"),
        }
    }

    eprintln!("[SlowLoop] 종료");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conn::Listener;
    use crate::conn_message::Message;
    use crate::policy_discrete::{Categorical, DiscretePolicy, WeightedAction};
    use serde_json::json;

    #[test]
    fn fast_loop_runs_without_llm() {
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let game_side = thread::spawn(move || {
            let _conn = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(50));
        });

        let mut agent = Agent::connect(addr, "test intent").unwrap();
        // LLM 미설정 → 빠른 루프만 구동, 느린 루프 없음
        let result = agent.run();
        assert!(result.is_ok());
        game_side.join().unwrap();
    }

    #[test]
    fn policy_injection_via_channel() {
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let game_side = thread::spawn(move || {
            let mut conn = listener.accept().unwrap();
            // 관측 하나 보내기
            let obs = crate::conn_message::Observation::new(0, json!({"hp": 100}));
            conn.send(&Message::from(obs).to_bytes().unwrap()).unwrap();
            // 액션 하나 받기
            let bytes = conn.recv().unwrap().unwrap();
            match Message::from_bytes(&bytes).unwrap() {
                Message::Action(a) => a.command,
                _ => panic!("expected action"),
            }
        });

        let mut agent = Agent::connect(addr, "test").unwrap();
        agent.actor_mut().set_policy(Box::new(DiscretePolicy {
            rules: vec![],
            fallback: Categorical {
                choices: vec![WeightedAction {
                    command: json!({"key": "idle"}),
                    weight: 1.0,
                }],
            },
        }));

        let result = agent.run();
        assert!(result.is_ok());
        let cmd = game_side.join().unwrap();
        assert_eq!(cmd, json!({"key": "idle"}));
    }
}
