//! Discrete 환경용 policy — 가중 범주형(categorical) head.
//!
//! 이산 액션 공간에서 native하게 동작한다. 규칙이 매칭되면 그 규칙의 가중 분포에서
//! 액션을 하나 샘플링한다 ("70% 점프, 30% 발사" 같은 범주형 분포).
//!
//! 동작:
//! 1. 규칙을 위에서부터 평가해 조건이 맞는 **첫 규칙**을 채택 (first-match 우선순위)
//! 2. 그 규칙의 가중 분포에서 액션 command를 샘플링
//! 3. 매칭되는 규칙이 없으면 `fallback`에서 샘플링

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::policy::{all_match, Cond, Policy, Rng};

/// 가중치가 붙은 하나의 이산 액션. `command`는 그대로 [`crate::conn_message::Action`]의
/// 본문이 된다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedAction {
    pub command: Value,
    pub weight: f64,
}

/// 가중 범주형 분포. 매칭된 규칙(또는 fallback)의 액션 출력.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Categorical {
    pub choices: Vec<WeightedAction>,
}

impl Categorical {
    /// 가중치에 비례해 command 하나를 샘플링한다. 비어 있거나 가중치 합이 0이면 `None`.
    fn sample(&self, rng: &mut Rng) -> Option<Value> {
        let total: f64 = self.choices.iter().map(|c| c.weight.max(0.0)).sum();
        if total <= 0.0 {
            return None;
        }
        let mut point = rng.next_f64() * total;
        for choice in &self.choices {
            point -= choice.weight.max(0.0);
            if point < 0.0 {
                return Some(choice.command.clone());
            }
        }
        // 부동소수 오차로 끝까지 못 고른 경우 마지막을 반환.
        self.choices.last().map(|c| c.command.clone())
    }
}

/// 조건 묶음(AND) → 범주형 액션 분포.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscreteRule {
    /// 사람·LLM 가독성용 이름 ("적 근접 교전"). 평가엔 쓰이지 않는다.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub conditions: Vec<Cond>,
    pub head: Categorical,
}

/// Discrete 환경용 policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscretePolicy {
    pub rules: Vec<DiscreteRule>,
    /// 어느 규칙도 안 맞을 때의 기본 분포.
    #[serde(default)]
    pub fallback: Categorical,
}

impl Policy for DiscretePolicy {
    fn decide(&self, state: &Value, rng: &mut Rng) -> Option<Value> {
        // first-match: 가장 위의 매칭 규칙이 이긴다.
        for rule in &self.rules {
            if all_match(&rule.conditions, state) {
                return rule.head.sample(rng);
            }
        }
        self.fallback.sample(rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::Op;
    use serde_json::json;

    fn wa(cmd: Value, w: f64) -> WeightedAction {
        WeightedAction { command: cmd, weight: w }
    }

    #[test]
    fn first_match_rule_wins() {
        let policy = DiscretePolicy {
            rules: vec![
                DiscreteRule {
                    name: "근접".into(),
                    conditions: vec![Cond { path: "dist".into(), op: Op::Lt, value: json!(5) }],
                    head: Categorical { choices: vec![wa(json!({ "key": "melee" }), 1.0)] },
                },
                DiscreteRule {
                    name: "원거리".into(),
                    conditions: vec![Cond { path: "dist".into(), op: Op::Ge, value: json!(5) }],
                    head: Categorical { choices: vec![wa(json!({ "key": "shoot" }), 1.0)] },
                },
            ],
            fallback: Categorical::default(),
        };
        let mut rng = Rng::new(1);
        assert_eq!(policy.decide(&json!({ "dist": 3 }), &mut rng), Some(json!({ "key": "melee" })));
        assert_eq!(policy.decide(&json!({ "dist": 9 }), &mut rng), Some(json!({ "key": "shoot" })));
    }

    #[test]
    fn falls_back_when_no_rule_matches() {
        let policy = DiscretePolicy {
            rules: vec![DiscreteRule {
                name: "x".into(),
                conditions: vec![Cond { path: "dist".into(), op: Op::Lt, value: json!(5) }],
                head: Categorical { choices: vec![wa(json!("melee"), 1.0)] },
            }],
            fallback: Categorical { choices: vec![wa(json!("idle"), 1.0)] },
        };
        let mut rng = Rng::new(1);
        assert_eq!(policy.decide(&json!({ "dist": 100 }), &mut rng), Some(json!("idle")));
    }

    #[test]
    fn weighted_sampling_respects_distribution() {
        let cat = Categorical {
            choices: vec![wa(json!("a"), 0.8), wa(json!("b"), 0.2)],
        };
        let mut rng = Rng::new(7);
        let n = 10_000;
        let a_count = (0..n).filter(|_| cat.sample(&mut rng) == Some(json!("a"))).count();
        let ratio = a_count as f64 / n as f64;
        assert!((ratio - 0.8).abs() < 0.03, "ratio off: {ratio}");
    }

    #[test]
    fn empty_distribution_yields_none() {
        let mut rng = Rng::new(1);
        assert_eq!(Categorical::default().sample(&mut rng), None);
    }
}
