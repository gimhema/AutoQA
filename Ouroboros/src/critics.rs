//! Intent 부합 평가 — 느린 루프의 판단 핵심.
//!
//! LLM에게 "intent + 최근 관측 이력"을 보내고, 현재 policy가 적절한지 평가받는다.
//! 판정(`Verdict`)에 따라 느린 루프는 policy를 유지하거나 재생성한다.

use std::io;

use crate::llm_interface::{ChatMessage, LlmClient};

/// LLM의 평가 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Keep,
    Regenerate { reason: String },
}

const SYSTEM_PROMPT: &str = r#"You are a QA evaluator for a game-playing agent. You will be given:
1. The agent's intent (what it is trying to accomplish)
2. A summary of recent game observations

Evaluate whether the agent's current behavior is aligned with the intent.

Respond with EXACTLY one of:
- KEEP (if the agent is behaving appropriately for the intent)
- REGENERATE: <reason> (if the agent's behavior does not match the intent and the policy should be regenerated)

Do not include any other text."#;

/// intent와 최근 관측 이력을 기반으로 현재 policy를 평가한다.
pub fn evaluate(
    llm: &LlmClient,
    intent: &str,
    observation_summary: &str,
) -> io::Result<Verdict> {
    let user_msg = format!(
        "Intent: {intent}\n\nRecent observations:\n{observation_summary}"
    );

    let response = llm.chat(&[
        ChatMessage::system(SYSTEM_PROMPT),
        ChatMessage::user(user_msg),
    ])?;

    Ok(parse_verdict(&response))
}

/// LLM 응답 텍스트를 Verdict로 파싱한다.
fn parse_verdict(response: &str) -> Verdict {
    let trimmed = response.trim();

    if trimmed.eq_ignore_ascii_case("KEEP") {
        return Verdict::Keep;
    }

    let upper = trimmed.to_uppercase();
    if let Some(rest) = upper.strip_prefix("REGENERATE:") {
        let reason = trimmed[trimmed.len() - rest.len()..].trim().to_string();
        return Verdict::Regenerate {
            reason: if reason.is_empty() {
                "LLM requested policy regeneration".into()
            } else {
                reason
            },
        };
    }

    if upper.contains("REGENERATE") {
        return Verdict::Regenerate {
            reason: trimmed.to_string(),
        };
    }

    Verdict::Keep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_keep() {
        assert_eq!(parse_verdict("KEEP"), Verdict::Keep);
        assert_eq!(parse_verdict("  keep  "), Verdict::Keep);
        assert_eq!(parse_verdict("Keep"), Verdict::Keep);
    }

    #[test]
    fn parse_regenerate_with_reason() {
        match parse_verdict("REGENERATE: agent is stuck in a corner") {
            Verdict::Regenerate { reason } => {
                assert_eq!(reason, "agent is stuck in a corner");
            }
            _ => panic!("expected Regenerate"),
        }
    }

    #[test]
    fn parse_regenerate_case_insensitive() {
        match parse_verdict("Regenerate: not moving") {
            Verdict::Regenerate { reason } => {
                assert_eq!(reason, "not moving");
            }
            _ => panic!("expected Regenerate"),
        }
    }

    #[test]
    fn parse_regenerate_without_colon() {
        match parse_verdict("The agent should REGENERATE its policy") {
            Verdict::Regenerate { reason } => {
                assert!(reason.contains("REGENERATE"));
            }
            _ => panic!("expected Regenerate"),
        }
    }

    #[test]
    fn parse_regenerate_empty_reason() {
        match parse_verdict("REGENERATE:") {
            Verdict::Regenerate { reason } => {
                assert_eq!(reason, "LLM requested policy regeneration");
            }
            _ => panic!("expected Regenerate"),
        }
    }

    #[test]
    fn parse_unknown_defaults_to_keep() {
        assert_eq!(parse_verdict("I think everything is fine"), Verdict::Keep);
        assert_eq!(parse_verdict(""), Verdict::Keep);
    }
}
