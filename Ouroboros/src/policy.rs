//! Policy의 공통 전단(front-end)과 [`PolicyManager`].
//!
//! policy는 "조건부 규칙 + 확률 가중치"로, 관측값을 받아 액션을 즉시 낸다 (LLM 없이
//! 빠른 루프에서 평가). 조건 매칭은 discrete/continuous 환경에서 공통이며, 규칙이
//! 매칭됐을 때 무엇을 내놓느냐(액션 head)만 환경별로 갈린다:
//! - discrete : 가중 범주형(categorical) — [`crate::policy_discrete`]
//! - continuous : 차원별 가우시안(Gaussian) — [`crate::policy_continuous`]
//!
//! 이 모듈은 두 환경이 공유하는 것만 담는다:
//! - [`Op`]/[`Cond`] : 관측 상태를 영역으로 나누는 술어
//! - [`Policy`] 트레잇 : 환경별 policy가 구현하는 `decide`
//! - [`PolicyManager`] : 현재 policy를 들고 빠른 루프에 액션을 제공. 느린 루프(LLM)가
//!   `set_policy`로 통째 교체한다.
//! - [`Rng`] : 의존성 없는 경량 난수 (가중 샘플링/가우시안용)

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 비교 연산자. 연속값은 `Lt`~`Ge`(수치 비교), 이산/범주형은 `Eq`/`Ne`로 매칭.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

/// 관측 상태의 한 경로를 꺼내 비교하는 단일 조건.
///
/// `path`는 점 표기(`"enemy.dist"`)로 중첩 JSON을 탐색한다. 관측 스키마에 비종속.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cond {
    pub path: String,
    pub op: Op,
    pub value: Value,
}

impl Cond {
    /// 관측 상태에 대해 이 조건이 성립하는지 평가한다.
    ///
    /// 경로가 없거나 타입이 안 맞으면 `false` (매칭 실패).
    pub fn matches(&self, state: &Value) -> bool {
        let Some(actual) = lookup_path(state, &self.path) else {
            return false;
        };
        match self.op {
            Op::Eq => actual == &self.value,
            Op::Ne => actual != &self.value,
            // 순서 비교는 양쪽이 수치일 때만 의미를 가진다.
            Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                match (actual.as_f64(), self.value.as_f64()) {
                    (Some(a), Some(b)) => match self.op {
                        Op::Lt => a < b,
                        Op::Le => a <= b,
                        Op::Gt => a > b,
                        Op::Ge => a >= b,
                        _ => unreachable!(),
                    },
                    _ => false,
                }
            }
        }
    }
}

/// 조건 묶음을 AND로 평가한다. 빈 묶음은 "항상 참"(무조건 규칙).
pub fn all_match(conds: &[Cond], state: &Value) -> bool {
    conds.iter().all(|c| c.matches(state))
}

/// 점 표기 경로로 중첩 JSON 값을 탐색한다. 배열 인덱스는 숫자 세그먼트로 접근.
fn lookup_path<'a>(state: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = state;
    for seg in path.split('.') {
        cur = match cur {
            Value::Object(map) => map.get(seg)?,
            Value::Array(arr) => arr.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// 환경별 policy가 구현하는 동작. 관측값 → 액션 command(JSON).
///
/// `None`이면 이 틱에 보낼 액션이 없음(매칭 규칙도 fallback도 없을 때).
pub trait Policy: Send {
    fn decide(&self, state: &Value, rng: &mut Rng) -> Option<Value>;
}

/// 현재 policy를 보유하고 빠른 루프에 액션을 제공한다.
///
/// 느린 루프(LLM)가 [`set_policy`](Self::set_policy)로 policy를 통째 교체한다.
/// (동시 접근 동기화는 상위 계층의 책임 — 지금은 단일 스레드 소유 가정.)
pub struct PolicyManager {
    current: Option<Box<dyn Policy>>,
    rng: Rng,
}

impl PolicyManager {
    /// 빈 매니저를 만든다 (아직 policy 없음). `seed`는 샘플링 난수 시드.
    pub fn new(seed: u64) -> Self {
        Self {
            current: None,
            rng: Rng::new(seed),
        }
    }

    /// 현재 policy를 새 것으로 교체한다 (LLM 생성 결과 주입).
    pub fn set_policy(&mut self, policy: Box<dyn Policy>) {
        self.current = Some(policy);
    }

    /// policy가 설정돼 있는지.
    pub fn has_policy(&self) -> bool {
        self.current.is_some()
    }

    /// 관측 상태로부터 액션을 결정한다. policy가 없으면 `None`.
    pub fn decide(&mut self, state: &Value) -> Option<Value> {
        let policy = self.current.as_ref()?;
        policy.decide(state, &mut self.rng)
    }
}

/// 의존성 없는 경량 PRNG (xorshift128+). QA 자동화의 샘플링 용도로 충분하며,
/// 암호학적 보안은 목표가 아니다.
pub struct Rng {
    s0: u64,
    s1: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        // splitmix64로 시드를 흩뿌려 0 시드 등의 약점을 피한다.
        let mut z = seed.wrapping_add(0x9E3779B97F4A7C15);
        let mut next = || {
            z = z.wrapping_add(0x9E3779B97F4A7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
            x ^ (x >> 31)
        };
        Self {
            s0: next(),
            s1: next(),
        }
    }

    /// 다음 u64.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.s0;
        let y = self.s1;
        self.s0 = y;
        x ^= x << 23;
        self.s1 = x ^ y ^ (x >> 17) ^ (y >> 26);
        self.s1.wrapping_add(y)
    }

    /// [0, 1) 범위의 f64.
    pub fn next_f64(&mut self) -> f64 {
        // 상위 53비트로 [0,1) 균등 분포.
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// 표준정규 N(0,1) 표본 (Box-Muller).
    pub fn next_gaussian(&mut self) -> f64 {
        // u1이 0이면 ln이 발산하므로 작은 하한을 둔다.
        let u1 = self.next_f64().max(1e-12);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numeric_and_categorical_conditions() {
        let state = json!({ "hp": 73.2, "boss": { "phase": 2 }, "weapon": "sword" });

        assert!(Cond { path: "hp".into(), op: Op::Lt, value: json!(100) }.matches(&state));
        assert!(!Cond { path: "hp".into(), op: Op::Gt, value: json!(100) }.matches(&state));
        assert!(Cond { path: "boss.phase".into(), op: Op::Eq, value: json!(2) }.matches(&state));
        assert!(Cond { path: "weapon".into(), op: Op::Eq, value: json!("sword") }.matches(&state));
        assert!(Cond { path: "weapon".into(), op: Op::Ne, value: json!("bow") }.matches(&state));
        // 없는 경로 → 매칭 실패.
        assert!(!Cond { path: "mana".into(), op: Op::Lt, value: json!(10) }.matches(&state));
    }

    #[test]
    fn array_path_lookup() {
        let state = json!({ "pos": [1.0, 2.5, 3.0] });
        assert!(Cond { path: "pos.1".into(), op: Op::Eq, value: json!(2.5) }.matches(&state));
    }

    #[test]
    fn all_match_is_and() {
        let state = json!({ "a": 1, "b": 2 });
        let conds = vec![
            Cond { path: "a".into(), op: Op::Eq, value: json!(1) },
            Cond { path: "b".into(), op: Op::Eq, value: json!(2) },
        ];
        assert!(all_match(&conds, &state));
        assert!(all_match(&[], &state)); // 빈 조건 = 항상 참
    }

    #[test]
    fn gaussian_mean_is_roughly_zero() {
        let mut rng = Rng::new(42);
        let n = 10_000;
        let mean: f64 = (0..n).map(|_| rng.next_gaussian()).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.1, "mean drifted: {mean}");
    }
}
