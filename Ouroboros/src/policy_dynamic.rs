//! Dynamic discrete policy — 관측 안의 `valid_actions` 목록에서 액션을 선택한다.
//!
//! `DiscretePolicy`는 policy 생성 시점에 고정된 command 집합에서 샘플링하므로,
//! 기물 위치처럼 **매 턴 달라지는** 액션 공간에 쓸 수 없다. 이 policy는
//! 게임이 관측마다 `valid_actions` 배열을 계산해 제공하고, LLM이 생성한 규칙이
//! 그 배열에서 적합한 액션을 선택하는 방식으로 동작한다.
//!
//! ## 선택 흐름
//! 1. 관측에서 `valid_actions: [...]`를 꺼낸다.
//! 2. 규칙을 위→아래로 평가한다:
//!    a. **state_conditions**: 게임 상태 전체에 대해 AND 검사
//!    b. **action_conditions**: `valid_actions`를 순회하며, 모든 조건을 만족하는 **첫 액션** 반환
//! 3. 어떤 규칙도 매칭되지 않으면 `valid_actions` 중 무작위 선택 (fallback).
//!
//! ## 조건 경로
//! `state_conditions`의 `path`는 관측 JSON 최상위 키(`"my_king.x"` 등)를 참조하고,
//! `action_conditions`의 `path`는 각 action 객체의 키(`"piece"`, `"is_capture"` 등)를 참조한다.
//! 두 조건 모두 [`crate::policy::Cond`]를 재사용한다.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::policy::{all_match, Cond, Policy, Rng};

/// 하나의 선택 규칙.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicRule {
    /// 가독성용 이름 (평가에 미사용).
    #[serde(default)]
    pub name: String,
    /// 게임 상태 전체에 대한 조건 (AND). 빈 배열 = 항상 참.
    #[serde(default)]
    pub state_conditions: Vec<Cond>,
    /// 후보 액션 각각에 대한 조건 (AND). 빈 배열 = valid_actions 첫 번째 반환.
    #[serde(default)]
    pub action_conditions: Vec<Cond>,
}

/// `valid_actions` 기반 dynamic discrete policy.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DynamicDiscretePolicy {
    pub rules: Vec<DynamicRule>,
}

impl Policy for DynamicDiscretePolicy {
    fn decide(&self, state: &Value, rng: &mut Rng) -> Option<Value> {
        let valid_actions = state.get("valid_actions")?.as_array()?;
        if valid_actions.is_empty() {
            return None;
        }

        for rule in &self.rules {
            if !all_match(&rule.state_conditions, state) {
                continue;
            }
            for action in valid_actions {
                if all_match(&rule.action_conditions, action) {
                    return Some(action.clone());
                }
            }
        }

        // Fallback: valid_actions 중 무작위 선택
        let idx = (rng.next_u64() as usize) % valid_actions.len();
        Some(valid_actions[idx].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{Op, Rng};
    use serde_json::json;

    fn state(valid_actions: Value) -> Value {
        json!({
            "my_king": {"x": 3, "y": 0},
            "enemy_king": {"x": 3, "y": 5},
            "valid_actions": valid_actions,
        })
    }

    #[test]
    fn selects_matching_action_condition() {
        let policy = DynamicDiscretePolicy {
            rules: vec![DynamicRule {
                name: "King 잡기".into(),
                state_conditions: vec![],
                action_conditions: vec![Cond {
                    path: "captured_kind".into(),
                    op: Op::Eq,
                    value: json!("king"),
                }],
            }],
        };
        let actions = json!([
            {"from_x":3,"from_y":0,"to_x":3,"to_y":1,"piece":"king","is_capture":false,"captured_kind":null},
            {"from_x":0,"from_y":0,"to_x":0,"to_y":1,"piece":"pawn","is_capture":true,"captured_kind":"king"}
        ]);
        let mut rng = Rng::new(0);
        let cmd = policy.decide(&state(actions), &mut rng).unwrap();
        assert_eq!(cmd["piece"], "pawn");
        assert_eq!(cmd["captured_kind"], "king");
    }

    #[test]
    fn state_condition_filters_rule() {
        // 적 King이 가까울 때만 King을 전진시키는 규칙
        let policy = DynamicDiscretePolicy {
            rules: vec![DynamicRule {
                name: "근거리 King 전진".into(),
                state_conditions: vec![Cond {
                    path: "enemy_king.y".into(),
                    op: Op::Lt,
                    value: json!(3),
                }],
                action_conditions: vec![Cond {
                    path: "piece".into(),
                    op: Op::Eq,
                    value: json!("king"),
                }],
            }],
        };
        let actions = json!([
            {"from_x":3,"from_y":0,"to_x":3,"to_y":1,"piece":"king","is_capture":false,"captured_kind":null},
        ]);
        // enemy_king.y = 5 → state_condition 불충족 → fallback (random, 유일 액션)
        let mut rng = Rng::new(1);
        let cmd = policy.decide(&state(actions), &mut rng).unwrap();
        // fallback으로도 same action
        assert_eq!(cmd["piece"], "king");
    }

    #[test]
    fn falls_back_to_random_when_no_rule_matches() {
        let policy = DynamicDiscretePolicy {
            rules: vec![DynamicRule {
                name: "절대 안 맞는 규칙".into(),
                state_conditions: vec![],
                action_conditions: vec![Cond {
                    path: "piece".into(),
                    op: Op::Eq,
                    value: json!("bishop"), // MiniChess에 없는 기물
                }],
            }],
        };
        let actions = json!([
            {"from_x":0,"from_y":0,"to_x":0,"to_y":1,"piece":"pawn","captured_kind":null},
            {"from_x":3,"from_y":0,"to_x":3,"to_y":1,"piece":"king","captured_kind":null},
        ]);
        let mut rng = Rng::new(42);
        let cmd = policy.decide(&state(actions), &mut rng).unwrap();
        // 둘 중 하나여야 함
        assert!(cmd["piece"] == "pawn" || cmd["piece"] == "king");
    }

    #[test]
    fn empty_valid_actions_returns_none() {
        let policy = DynamicDiscretePolicy::default();
        let mut rng = Rng::new(0);
        assert!(policy.decide(&state(json!([])), &mut rng).is_none());
    }

    #[test]
    fn prefers_capture_over_advance() {
        let policy = DynamicDiscretePolicy {
            rules: vec![
                DynamicRule {
                    name: "포획 우선".into(),
                    state_conditions: vec![],
                    action_conditions: vec![Cond {
                        path: "is_capture".into(),
                        op: Op::Eq,
                        value: json!(true),
                    }],
                },
                DynamicRule {
                    name: "전진".into(),
                    state_conditions: vec![],
                    action_conditions: vec![Cond {
                        path: "dist_to_enemy_king_delta".into(),
                        op: Op::Lt,
                        value: json!(0),
                    }],
                },
            ],
        };
        let actions = json!([
            {"from_x":3,"from_y":0,"to_x":3,"to_y":1,"piece":"king","is_capture":false,"captured_kind":null,"dist_to_enemy_king_delta":-1},
            {"from_x":0,"from_y":0,"to_x":0,"to_y":1,"piece":"pawn","is_capture":true,"captured_kind":"pawn","dist_to_enemy_king_delta":1},
        ]);
        let mut rng = Rng::new(0);
        let cmd = policy.decide(&state(actions), &mut rng).unwrap();
        // 포획 규칙이 먼저 → pawn capture
        assert_eq!(cmd["is_capture"], true);
    }
}
