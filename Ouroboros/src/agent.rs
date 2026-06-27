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

use crate::actor::Actor;
use crate::game_interface::GameInterface;
use crate::llm_interface::LlmClient;

pub struct Agent {
    /// 게임과의 고수준 통신 핸들.
    game: GameInterface,
    /// 빠른 루프의 실행자. 액션 결정/전송을 위임받는다.
    actor: Actor,
    /// 자연어 의도(intent). 향후 느린 루프에서 LLM에게 전달된다.
    intent: String,
    /// 빠른 루프 한 틱의 간격.
    tick: Duration,
    /// 로컬 LLM 클라이언트. 없으면 빠른 루프만 구동.
    llm: Option<LlmClient>,
}

impl Agent {
    /// 게임에 접속하고 주어진 intent로 에이전트를 구성한다.
    pub fn connect<A: std::net::ToSocketAddrs>(addr: A, intent: impl Into<String>) -> io::Result<Self> {
        let game = GameInterface::connect(addr)?;
        Ok(Self {
            game,
            actor: Actor::new(0),
            intent: intent.into(),
            tick: Duration::from_millis(16), // 약 60Hz
            llm: None,
        })
    }

    /// 로컬 LLM 클라이언트를 설정한다.
    pub fn set_llm(&mut self, llm: LlmClient) {
        self.llm = Some(llm);
    }

    /// 액션 결정을 담당하는 Actor에 접근한다 (policy 주입 등).
    pub fn actor_mut(&mut self) -> &mut Actor {
        &mut self.actor
    }

    /// 빠른 루프를 실행한다. 게임 연결이 끊기면 종료.
    ///
    /// 코어는 두 루프의 조율만 맡고, 액션 결정/전송은 [`Actor`]에 위임한다. 느린
    /// 루프(LLM이 policy를 갱신)는 후속 작업에서 이 루프와 병행하도록 붙는다.
    pub fn run(&mut self) -> io::Result<()> {
        println!("[Agent] intent = {:?}", self.intent);

        while self.game.is_alive() {
            // 빠른 루프: 최신 관측만 사용하고 밀린 stale 관측은 버린다.
            if let Some(obs) = self.game.take_latest_observation() {
                self.actor.act(&obs.state, &mut self.game)?;
            }
            std::thread::sleep(self.tick);
        }

        println!("[Agent] 게임 연결 종료, 루프 중단");
        Ok(())
    }
}
