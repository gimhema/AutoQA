//! 우로보로스 에이전트의 최상위 오케스트레이터.
//!
//! 두 루프를 조율한다:
//! - **빠른 루프**: [`GameInterface`]에서 최신 관측을 받아 (향후 Actor/Policy로)
//!   액션을 만들어 게임에 전송. LLM을 기다리지 않는다.
//! - **느린 루프**: (향후) LLM이 intent 부합을 평가하고 policy/서브골을 갱신.
//!
//! 현재는 통신 계층([`crate::game_interface`])만 연동돼 있다. Policy/Actor/Critics는
//! 후속 작업에서 이 골격에 주입된다.

use std::io;
use std::time::Duration;

use serde_json::Value;

use crate::game_interface::GameInterface;

pub struct Agent {
    /// 게임과의 고수준 통신 핸들.
    game: GameInterface,
    /// 자연어 의도(intent). 향후 느린 루프에서 LLM에게 전달된다.
    intent: String,
    /// 빠른 루프 한 틱의 간격.
    tick: Duration,
}

impl Agent {
    /// 게임에 접속하고 주어진 intent로 에이전트를 구성한다.
    pub fn connect<A: std::net::ToSocketAddrs>(addr: A, intent: impl Into<String>) -> io::Result<Self> {
        let game = GameInterface::connect(addr)?;
        Ok(Self {
            game,
            intent: intent.into(),
            tick: Duration::from_millis(16), // 약 60Hz
        })
    }

    /// 빠른 루프를 실행한다. 게임 연결이 끊기면 종료.
    ///
    /// 아직 Policy/Actor가 없으므로 지금은 최신 관측을 가져와 (자리표시) 액션을
    /// 결정하는 골격만 돈다. Policy 연동 시 `decide_action`을 교체한다.
    pub fn run(&mut self) -> io::Result<()> {
        println!("[Agent] intent = {:?}", self.intent);

        while self.game.is_alive() {
            // 빠른 루프: 최신 관측만 사용하고 밀린 stale 관측은 버린다.
            if let Some(obs) = self.game.take_latest_observation() {
                if let Some(command) = self.decide_action(&obs.state) {
                    self.game.send_action(command)?;
                }
            }
            std::thread::sleep(self.tick);
        }

        println!("[Agent] 게임 연결 종료, 루프 중단");
        Ok(())
    }

    /// 관측 상태로부터 액션을 결정한다.
    ///
    /// **자리표시(placeholder)**: 현재는 아무 액션도 내지 않는다. 후속 작업에서
    /// 현재 policy에서 액션을 샘플링하는 Actor 호출로 대체된다.
    fn decide_action(&self, _state: &Value) -> Option<Value> {
        None
    }
}
