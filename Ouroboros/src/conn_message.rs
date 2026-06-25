//! 게임 ↔ 에이전트 간 메시지 스키마와 직렬화.
//!
//! 전송 계층([`crate::conn`])은 raw 바이트 프레임만 다룬다. 이 모듈은 그 프레임에
//! 실리는 페이로드의 의미를 정의한다. 직렬화 포맷은 JSON — 관측값을 그대로 LLM에
//! 넘길 수 있고, 게임이 정의하는 상태 스키마에 종속되지 않는다.
//!
//! 상태/액션 본문은 [`serde_json::Value`]로 둔다. 이 통신 계층은 스키마에 관여하지
//! 않으며, 구체적 형태는 상위 계층(StatusObserver / Actor)이 해석한다.
//!
//! 모든 메시지는 `seq`와 `timestamp_ms`를 가진다. 빠른 루프는 최신 관측만 사용하고
//! 밀린(stale) 관측은 폐기하기 위해 이 값으로 신선도를 판단한다 (HoL blocking 완화).

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 현재 시각을 Unix epoch 기준 밀리초로 반환한다.
pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 게임 → 에이전트: 구조화된 상태 관측값.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// 단조 증가 시퀀스 번호. stale 관측 폐기에 사용.
    pub seq: u64,
    /// 관측 시각 (Unix epoch ms).
    pub timestamp_ms: u64,
    /// 게임이 정의하는 상태값 (체력, 마나, 위치 등). 스키마는 게임에 종속.
    pub state: Value,
}

impl Observation {
    /// 현재 시각 타임스탬프로 관측값을 만든다.
    pub fn new(seq: u64, state: Value) -> Self {
        Self {
            seq,
            timestamp_ms: now_millis(),
            state,
        }
    }
}

/// 에이전트 → 게임: 수행할 액션.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// 단조 증가 시퀀스 번호.
    pub seq: u64,
    /// 전송 시각 (Unix epoch ms).
    pub timestamp_ms: u64,
    /// 액션 본문 (키 입력, 이동 등). 스키마는 게임에 종속.
    pub command: Value,
}

impl Action {
    /// 현재 시각 타임스탬프로 액션을 만든다.
    pub fn new(seq: u64, command: Value) -> Self {
        Self {
            seq,
            timestamp_ms: now_millis(),
            command,
        }
    }
}

/// 와이어 상의 메시지. `kind` 태그로 관측/액션을 구분한다 (internally tagged).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Message {
    Observation(Observation),
    Action(Action),
}

impl Message {
    /// JSON 바이트로 직렬화한다. [`crate::conn::Connection::send`]에 그대로 전달.
    pub fn to_bytes(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }

    /// [`crate::conn::Connection::recv`]가 돌려준 프레임 바이트를 역직렬화한다.
    pub fn from_bytes(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }
}

impl From<Observation> for Message {
    fn from(o: Observation) -> Self {
        Message::Observation(o)
    }
}

impl From<Action> for Message {
    fn from(a: Action) -> Self {
        Message::Action(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn observation_roundtrip() {
        let obs = Observation::new(7, json!({ "hp": 100, "mp": 30, "pos": [1.0, 2.5] }));
        let msg: Message = obs.into();
        let bytes = msg.to_bytes().unwrap();
        let decoded = Message::from_bytes(&bytes).unwrap();
        match decoded {
            Message::Observation(o) => {
                assert_eq!(o.seq, 7);
                assert_eq!(o.state["hp"], 100);
                assert_eq!(o.state["pos"][1], 2.5);
            }
            _ => panic!("expected observation"),
        }
    }

    #[test]
    fn action_roundtrip() {
        let act = Action::new(3, json!({ "move": "forward", "fire": true }));
        let bytes = Message::from(act).to_bytes().unwrap();
        let decoded = Message::from_bytes(&bytes).unwrap();
        match decoded {
            Message::Action(a) => {
                assert_eq!(a.seq, 3);
                assert_eq!(a.command["fire"], true);
            }
            _ => panic!("expected action"),
        }
    }

    #[test]
    fn kind_tag_distinguishes_variants() {
        let obs_bytes = Message::from(Observation::new(1, json!({}))).to_bytes().unwrap();
        let s = String::from_utf8(obs_bytes).unwrap();
        assert!(s.contains("\"kind\":\"observation\""));
    }
}
