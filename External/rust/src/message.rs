use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub state: Value,
}

impl Observation {
    pub fn new(seq: u64, state: Value) -> Self {
        Self {
            seq,
            timestamp_ms: now_millis(),
            state,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub command: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Message {
    Observation(Observation),
    Action(Action),
}

impl Message {
    pub fn to_bytes(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }

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
        let obs = Observation::new(7, json!({"hp": 100, "pos": [1.0, 2.5]}));
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
        let act = Action {
            seq: 3,
            timestamp_ms: now_millis(),
            command: json!({"move": "forward", "fire": true}),
        };
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
    fn kind_tag_present() {
        let obs_bytes = Message::from(Observation::new(1, json!({}))).to_bytes().unwrap();
        let s = String::from_utf8(obs_bytes).unwrap();
        assert!(s.contains("\"kind\":\"observation\""));
    }
}
