//! Continuous 환경용 policy — 차원별 가우시안(Gaussian) head.
//!
//! 연속 액션 공간(조준각, 이동 벡터 등)을 다룬다. 규칙이 매칭되면 각 차원의 평균과
//! 분산으로 정의된 가우시안에서 액션 벡터를 샘플링하고, 액션 공간 경계로 clamp한다.
//! `std=0`이면 결정론적(평균값 고정)이 되어, LLM이 탐색/결정론을 스스로 선택할 수 있다.
//!
//! 동작:
//! 1. 규칙을 위에서부터 평가해 조건이 맞는 **첫 규칙**을 채택 (first-match 우선순위)
//! 2. 차원별로 `mean + std * N(0,1)`을 샘플링하고 `bounds`로 clamp
//! 3. 매칭되는 규칙이 없으면 `fallback`에서 샘플링
//!
//! 출력 command는 연속값 배열(`[f64; dims]`)을 담은 JSON.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::policy::{all_match, Cond, Policy, Rng};

/// 차원별 가우시안 분포. 매칭된 규칙(또는 fallback)의 액션 출력.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Gaussian {
    /// 차원별 평균.
    pub mean: Vec<f64>,
    /// 차원별 표준편차. 0이면 해당 차원은 결정론적.
    #[serde(default)]
    pub std: Vec<f64>,
}

impl Gaussian {
    /// 액션 벡터를 샘플링해 `bounds`로 clamp한다. `mean`이 비어 있으면 `None`.
    fn sample(&self, bounds: &[(f64, f64)], rng: &mut Rng) -> Option<Vec<f64>> {
        if self.mean.is_empty() {
            return None;
        }
        let out = self
            .mean
            .iter()
            .enumerate()
            .map(|(i, &m)| {
                let s = self.std.get(i).copied().unwrap_or(0.0);
                let v = if s > 0.0 { m + s * rng.next_gaussian() } else { m };
                match bounds.get(i) {
                    Some(&(lo, hi)) => v.clamp(lo, hi),
                    None => v,
                }
            })
            .collect();
        Some(out)
    }
}

/// 조건 묶음(AND) → 가우시안 액션 분포.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousRule {
    /// 사람·LLM 가독성용 이름. 평가엔 쓰이지 않는다.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub conditions: Vec<Cond>,
    pub head: Gaussian,
}

/// Continuous 환경용 policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContinuousPolicy {
    /// 액션 공간 차원 수.
    pub dims: usize,
    /// 차원별 `(min, max)` 경계. 샘플 결과를 이 범위로 clamp.
    #[serde(default)]
    pub bounds: Vec<(f64, f64)>,
    pub rules: Vec<ContinuousRule>,
    /// 어느 규칙도 안 맞을 때의 기본 분포.
    #[serde(default)]
    pub fallback: Gaussian,
}

impl ContinuousPolicy {
    /// 샘플 벡터를 JSON command로 감싼다. `{ "action": [..] }` 형식.
    fn wrap(vec: Vec<f64>) -> Value {
        serde_json::json!({ "action": vec })
    }
}

impl Policy for ContinuousPolicy {
    fn decide(&self, state: &Value, rng: &mut Rng) -> Option<Value> {
        // first-match: 가장 위의 매칭 규칙이 이긴다.
        for rule in &self.rules {
            if all_match(&rule.conditions, state) {
                return rule.head.sample(&self.bounds, rng).map(Self::wrap);
            }
        }
        self.fallback.sample(&self.bounds, rng).map(Self::wrap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::Op;
    use serde_json::json;

    fn action_vec(v: &Value) -> Vec<f64> {
        v["action"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect()
    }

    #[test]
    fn deterministic_when_std_zero() {
        let policy = ContinuousPolicy {
            dims: 2,
            bounds: vec![(-180.0, 180.0), (0.0, 1.0)],
            rules: vec![ContinuousRule {
                name: "조준".into(),
                conditions: vec![Cond { path: "enemy.visible".into(), op: Op::Eq, value: json!(true) }],
                head: Gaussian { mean: vec![37.5, 0.8], std: vec![0.0, 0.0] },
            }],
            fallback: Gaussian::default(),
        };
        let mut rng = Rng::new(1);
        let state = json!({ "enemy": { "visible": true } });
        let a = policy.decide(&state, &mut rng).unwrap();
        assert_eq!(action_vec(&a), vec![37.5, 0.8]);
    }

    #[test]
    fn clamps_to_bounds() {
        // 평균이 경계를 넘고 std가 커도 결과는 bounds 안.
        let policy = ContinuousPolicy {
            dims: 1,
            bounds: vec![(-10.0, 10.0)],
            rules: vec![],
            fallback: Gaussian { mean: vec![100.0], std: vec![50.0] },
        };
        let mut rng = Rng::new(3);
        for _ in 0..1000 {
            let a = policy.decide(&json!({}), &mut rng).unwrap();
            let x = action_vec(&a)[0];
            assert!((-10.0..=10.0).contains(&x), "out of bounds: {x}");
        }
    }

    #[test]
    fn stochastic_mean_tracks_specified_mean() {
        let policy = ContinuousPolicy {
            dims: 1,
            bounds: vec![(-1000.0, 1000.0)],
            rules: vec![],
            fallback: Gaussian { mean: vec![5.0], std: vec![2.0] },
        };
        let mut rng = Rng::new(11);
        let n = 10_000;
        let avg: f64 = (0..n)
            .map(|_| action_vec(&policy.decide(&json!({}), &mut rng).unwrap())[0])
            .sum::<f64>()
            / n as f64;
        assert!((avg - 5.0).abs() < 0.1, "mean off: {avg}");
    }

    #[test]
    fn empty_mean_yields_none() {
        let policy = ContinuousPolicy::default();
        let mut rng = Rng::new(1);
        assert_eq!(policy.decide(&json!({}), &mut rng), None);
    }
}
