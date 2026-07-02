//! LLM 응답으로부터 Policy를 생성하는 모듈.
//!
//! 느린 루프의 핵심 파이프라인: intent + 관측 상태 → LLM 프롬프트 → JSON 응답 →
//! [`DiscretePolicy`] 또는 [`ContinuousPolicy`] 객체.

use std::io;

use serde_json::Value;

use crate::llm_interface::{ChatMessage, LlmClient};
use crate::policy::Policy;
use crate::policy_continuous::ContinuousPolicy;
use crate::policy_discrete::DiscretePolicy;

/// 게임의 액션 공간 정의. 어떤 종류의 policy를 생성할지 결정한다.
pub enum ActionSpace {
    Discrete {
        available_actions: Vec<Value>,
    },
    Continuous {
        dims: usize,
        bounds: Vec<(f64, f64)>,
    },
}

/// LLM에게 프롬프트를 보내고 응답을 파싱해 Policy를 생성한다.
///
/// `rules`는 게임 규칙 컨텍스트(룰북 하네스가 숙지시킨 브리핑)다. 비어 있으면 규칙
/// 없이 생성하며, 있으면 시스템 프롬프트 앞에 붙여 규칙에 부합하는 policy를 유도한다.
pub fn generate_policy(
    llm: &LlmClient,
    intent: &str,
    state_sample: &Value,
    action_space: &ActionSpace,
    rules: &str,
) -> io::Result<Box<dyn Policy>> {
    let system = build_system_prompt(action_space, rules);
    let user = build_user_prompt(intent, state_sample);

    let response = llm.chat(&[
        ChatMessage::system(system),
        ChatMessage::user(user),
    ])?;

    parse_policy_response(&response, action_space)
}

/// LLM 응답 텍스트를 Policy 객체로 파싱한다 (LLM 호출 없이 단독 사용 가능).
pub fn parse_policy_response(
    response: &str,
    action_space: &ActionSpace,
) -> io::Result<Box<dyn Policy>> {
    let json_str = extract_json(response).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("no JSON found in LLM response: {response}"),
        )
    })?;

    match action_space {
        ActionSpace::Discrete { .. } => {
            let policy: DiscretePolicy = serde_json::from_str(json_str)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(Box::new(policy))
        }
        ActionSpace::Continuous { .. } => {
            let policy: ContinuousPolicy = serde_json::from_str(json_str)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(Box::new(policy))
        }
    }
}

fn build_system_prompt(action_space: &ActionSpace, rules: &str) -> String {
    let base = build_schema_prompt(action_space);
    if rules.trim().is_empty() {
        base
    } else {
        format!("GAME RULES (you have studied these before playing):\n{rules}\n\n{base}")
    }
}

fn build_schema_prompt(action_space: &ActionSpace) -> String {
    match action_space {
        ActionSpace::Discrete { available_actions } => {
            let actions_json = serde_json::to_string_pretty(available_actions)
                .unwrap_or_else(|_| "[]".into());
            format!(
r#"You are a game-playing policy generator. Given the player's intent and the current game state, produce a JSON policy that maps observations to actions.

Available actions:
{actions_json}

Output a JSON object with this exact schema (no extra text outside the JSON):
{{
  "rules": [
    {{
      "name": "human-readable rule name",
      "conditions": [
        {{ "path": "dot.notation.path", "op": "lt"|"le"|"gt"|"ge"|"eq"|"ne", "value": <number|string|bool> }}
      ],
      "head": {{
        "choices": [
          {{ "command": <action>, "weight": <positive number> }}
        ]
      }}
    }}
  ],
  "fallback": {{
    "choices": [
      {{ "command": <action>, "weight": <positive number> }}
    ]
  }}
}}

Rules are evaluated top-to-bottom; the first matching rule wins. Conditions within a rule are AND-combined. Use "path" with dot notation to access nested state fields (e.g. "enemy.dist", "pos.0")."#
            )
        }
        ActionSpace::Continuous { dims, bounds } => {
            let bounds_str = bounds
                .iter()
                .enumerate()
                .map(|(i, (lo, hi))| format!("  dim {i}: [{lo}, {hi}]"))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
r#"You are a game-playing policy generator. Given the player's intent and the current game state, produce a JSON policy that maps observations to continuous action vectors.

Action space: {dims} dimensions
Bounds:
{bounds_str}

Output a JSON object with this exact schema (no extra text outside the JSON):
{{
  "dims": {dims},
  "bounds": {bounds:?},
  "rules": [
    {{
      "name": "human-readable rule name",
      "conditions": [
        {{ "path": "dot.notation.path", "op": "lt"|"le"|"gt"|"ge"|"eq"|"ne", "value": <number|string|bool> }}
      ],
      "head": {{
        "mean": [<f64 per dim>],
        "std": [<f64 per dim, 0 for deterministic>]
      }}
    }}
  ],
  "fallback": {{
    "mean": [<f64 per dim>],
    "std": [<f64 per dim>]
  }}
}}

Rules are evaluated top-to-bottom; the first matching rule wins. Conditions within a rule are AND-combined. Use "path" with dot notation to access nested state fields. Values are clamped to bounds after sampling."#
            )
        }
    }
}

fn build_user_prompt(intent: &str, state_sample: &Value) -> String {
    let state_str = serde_json::to_string_pretty(state_sample)
        .unwrap_or_else(|_| state_sample.to_string());
    format!(
        "Intent: {intent}\n\nCurrent game state sample:\n{state_str}\n\nGenerate a policy JSON."
    )
}

/// LLM 응답 텍스트에서 JSON 부분을 추출한다.
///
/// 1. ```json ... ``` 마크다운 블록이 있으면 내부를 반환
/// 2. 없으면 첫 `{` ~ 마지막 `}` 범위를 반환
fn extract_json(text: &str) -> Option<&str> {
    if let Some(start) = text.find("```json") {
        let body_start = start + "```json".len();
        let rest = &text[body_start..];
        if let Some(end) = rest.find("```") {
            let json = rest[..end].trim();
            if !json.is_empty() {
                return Some(json);
            }
        }
    }

    if let Some(start) = text.find("```") {
        let body_start = text[start + 3..].find('\n').map(|n| start + 3 + n + 1)?;
        let rest = &text[body_start..];
        if let Some(end) = rest.find("```") {
            let json = rest[..end].trim();
            if json.starts_with('{') {
                return Some(json);
            }
        }
    }

    let first_brace = text.find('{')?;
    let last_brace = text.rfind('}')?;
    if first_brace < last_brace {
        Some(text[first_brace..=last_brace].trim())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_json_from_markdown_block() {
        let text = r#"Here is the policy:
```json
{"rules": [], "fallback": {"choices": []}}
```
Done."#;
        assert_eq!(
            extract_json(text).unwrap(),
            r#"{"rules": [], "fallback": {"choices": []}}"#
        );
    }

    #[test]
    fn extract_json_from_unlabeled_code_block() {
        let text = "Sure:\n```\n{\"rules\": []}\n```";
        assert_eq!(extract_json(text).unwrap(), r#"{"rules": []}"#);
    }

    #[test]
    fn extract_json_from_raw_text() {
        let text = r#"OK here: {"rules": [], "fallback": {"choices": []}} that's it"#;
        assert_eq!(
            extract_json(text).unwrap(),
            r#"{"rules": [], "fallback": {"choices": []}}"#
        );
    }

    #[test]
    fn extract_json_pure_json() {
        let text = r#"{"rules": []}"#;
        assert_eq!(extract_json(text).unwrap(), r#"{"rules": []}"#);
    }

    #[test]
    fn extract_json_returns_none_for_no_json() {
        assert!(extract_json("no json here").is_none());
    }

    #[test]
    fn parse_discrete_policy_from_llm_response() {
        let response = r#"```json
{
  "rules": [
    {
      "name": "low hp",
      "conditions": [{"path": "hp", "op": "lt", "value": 30}],
      "head": {
        "choices": [{"command": {"key": "heal"}, "weight": 1.0}]
      }
    }
  ],
  "fallback": {
    "choices": [{"command": {"key": "attack"}, "weight": 0.7}, {"command": {"key": "dodge"}, "weight": 0.3}]
  }
}
```"#;
        let action_space = ActionSpace::Discrete {
            available_actions: vec![json!({"key": "attack"}), json!({"key": "heal"}), json!({"key": "dodge"})],
        };
        let policy = parse_policy_response(response, &action_space).unwrap();

        let mut rng = crate::policy::Rng::new(42);
        let cmd = policy.decide(&json!({"hp": 20}), &mut rng);
        assert_eq!(cmd, Some(json!({"key": "heal"})));
    }

    #[test]
    fn parse_continuous_policy_from_llm_response() {
        let response = r#"{
  "dims": 2,
  "bounds": [[-1.0, 1.0], [-1.0, 1.0]],
  "rules": [],
  "fallback": {"mean": [0.5, -0.3], "std": [0.0, 0.0]}
}"#;
        let action_space = ActionSpace::Continuous {
            dims: 2,
            bounds: vec![(-1.0, 1.0), (-1.0, 1.0)],
        };
        let policy = parse_policy_response(response, &action_space).unwrap();

        let mut rng = crate::policy::Rng::new(1);
        let cmd = policy.decide(&json!({}), &mut rng).unwrap();
        assert_eq!(cmd["action"][0], 0.5);
        assert_eq!(cmd["action"][1], -0.3);
    }

    #[test]
    fn parse_fails_on_invalid_json() {
        let response = "not json at all";
        let action_space = ActionSpace::Discrete { available_actions: vec![] };
        assert!(parse_policy_response(response, &action_space).is_err());
    }

    #[test]
    fn parse_fails_on_wrong_schema() {
        let response = r#"{"wrong_field": true}"#;
        let action_space = ActionSpace::Continuous {
            dims: 1,
            bounds: vec![(-1.0, 1.0)],
        };
        assert!(parse_policy_response(response, &action_space).is_err());
    }

    #[test]
    fn system_prompt_includes_available_actions() {
        let action_space = ActionSpace::Discrete {
            available_actions: vec![json!("jump"), json!("fire")],
        };
        let prompt = build_system_prompt(&action_space, "");
        assert!(prompt.contains("jump"));
        assert!(prompt.contains("fire"));
        assert!(prompt.contains("choices"));
    }

    #[test]
    fn system_prompt_includes_rules_when_provided() {
        let action_space = ActionSpace::Discrete {
            available_actions: vec![json!("jump")],
        };
        let prompt = build_system_prompt(&action_space, "Capturing the King wins the game.");
        assert!(prompt.contains("GAME RULES"));
        assert!(prompt.contains("Capturing the King wins"));
    }

    #[test]
    fn system_prompt_includes_bounds() {
        let action_space = ActionSpace::Continuous {
            dims: 2,
            bounds: vec![(-180.0, 180.0), (0.0, 1.0)],
        };
        let prompt = build_system_prompt(&action_space, "");
        assert!(prompt.contains("-180"));
        assert!(prompt.contains("dims"));
        assert!(prompt.contains("mean"));
    }
}
