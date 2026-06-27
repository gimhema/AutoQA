//! 관측 이력 축적 및 요약.
//!
//! 빠른 루프는 최신 관측 하나만 사용하고 나머지를 버리지만, 느린 루프(LLM/Critics)는
//! 최근 상태 변화를 알아야 한다. `StatusObserver`는 고정 크기 링 버퍼에 관측을 쌓고,
//! LLM 프롬프트에 포함할 수 있는 텍스트 요약을 생성한다.

use std::collections::VecDeque;

use serde_json::Value;

use crate::conn_message::Observation;

struct Snapshot {
    seq: u64,
    timestamp_ms: u64,
    state: Value,
}

pub struct StatusObserver {
    history: VecDeque<Snapshot>,
    capacity: usize,
}

impl StatusObserver {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self {
            history: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// 관측을 기록한다. 용량 초과 시 가장 오래된 것을 제거한다.
    pub fn push(&mut self, obs: &Observation) {
        if self.history.len() == self.capacity {
            self.history.pop_front();
        }
        self.history.push_back(Snapshot {
            seq: obs.seq,
            timestamp_ms: obs.timestamp_ms,
            state: obs.state.clone(),
        });
    }

    /// 최신 상태를 반환한다. `policy_gen`의 `state_sample`에 사용.
    pub fn latest_state(&self) -> Option<&Value> {
        self.history.back().map(|s| &s.state)
    }

    /// 마지막으로 기록된 시퀀스 번호.
    pub fn latest_seq(&self) -> Option<u64> {
        self.history.back().map(|s| s.seq)
    }

    /// 현재 이력 수.
    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// 최근 이력을 LLM 프롬프트에 넣을 텍스트로 요약한다.
    ///
    /// 각 스냅샷을 시간순으로 나열. 상태는 compact JSON으로 출력한다.
    pub fn summarize(&self) -> String {
        if self.history.is_empty() {
            return "(no observations yet)".into();
        }

        let mut buf = String::new();
        for (i, snap) in self.history.iter().enumerate() {
            let state_str = serde_json::to_string(&snap.state)
                .unwrap_or_else(|_| snap.state.to_string());
            buf.push_str(&format!(
                "[{i}] seq={} t={}ms state={}\n",
                snap.seq, snap.timestamp_ms, state_str
            ));
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obs(seq: u64, state: Value) -> Observation {
        Observation { seq, timestamp_ms: 1000 + seq * 16, state }
    }

    #[test]
    fn push_and_latest() {
        let mut so = StatusObserver::new(10);
        assert!(so.is_empty());
        assert!(so.latest_state().is_none());

        so.push(&obs(0, json!({"hp": 100})));
        assert_eq!(so.len(), 1);
        assert_eq!(so.latest_state().unwrap(), &json!({"hp": 100}));
        assert_eq!(so.latest_seq(), Some(0));

        so.push(&obs(1, json!({"hp": 90})));
        assert_eq!(so.len(), 2);
        assert_eq!(so.latest_state().unwrap(), &json!({"hp": 90}));
        assert_eq!(so.latest_seq(), Some(1));
    }

    #[test]
    fn evicts_oldest_when_full() {
        let mut so = StatusObserver::new(3);
        for i in 0..5 {
            so.push(&obs(i, json!({"seq": i})));
        }
        assert_eq!(so.len(), 3);
        assert_eq!(so.latest_seq(), Some(4));
        // 가장 오래된 것은 seq=2
        assert_eq!(so.history.front().unwrap().seq, 2);
    }

    #[test]
    fn summarize_empty() {
        let so = StatusObserver::new(5);
        assert_eq!(so.summarize(), "(no observations yet)");
    }

    #[test]
    fn summarize_contains_all_entries() {
        let mut so = StatusObserver::new(10);
        so.push(&obs(0, json!({"hp": 100})));
        so.push(&obs(1, json!({"hp": 80})));
        let summary = so.summarize();
        assert!(summary.contains("seq=0"));
        assert!(summary.contains("seq=1"));
        assert!(summary.contains("\"hp\":100"));
        assert!(summary.contains("\"hp\":80"));
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn zero_capacity_panics() {
        StatusObserver::new(0);
    }
}
