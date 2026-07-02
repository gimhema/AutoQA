//! 룰북 하네스 — 게임 시작 전 에이전트에게 룰을 숙지시킨다.
//!
//! 에이전트는 intent("이기도록 플레이해봐")만으로는 게임의 규칙을 모른다. 이 하네스는
//! 게임이 제공하는 룰북(markdown)을 로드하고, **게임 시작 전에 LLM에게 룰을 읽혀
//! 이해를 정리하게 한다(숙지 단계)**. 그 정리된 이해는 이후 policy 생성/평가 프롬프트에
//! "게임 규칙 컨텍스트"로 주입되어, LLM이 규칙에 부합하는 policy를 만들도록 돕는다.
//!
//! 흐름: [`Rulebook::load`] → [`Rulebook::study`](느린 루프 시작 시 1회) →
//! [`Rulebook::context`](이후 프롬프트에 반복 삽입).

use std::fs;
use std::io;

use crate::llm_interface::{ChatMessage, LlmClient};

const STUDY_SYSTEM_PROMPT: &str = r#"You are about to play a game you have never seen before. Read the rulebook carefully and produce a concise, structured briefing of the rules that will guide your play. Cover:
- Objective and win/lose conditions
- The pieces/entities and their roles
- Allowed actions and movement/legality constraints
- Turn structure
- Any strategic implications that follow directly from the rules

Be precise and factual. Do not invent rules that are not stated. Output only the briefing."#;

pub struct Rulebook {
    /// 원문 룰북 (markdown).
    text: String,
    /// 숙지 단계에서 LLM이 정리한 이해 요약. 아직 숙지 전이면 `None`.
    briefing: Option<String>,
}

impl Rulebook {
    /// 파일에서 룰북을 로드한다.
    pub fn load(path: &str) -> io::Result<Self> {
        let text = fs::read_to_string(path)?;
        Ok(Self { text, briefing: None })
    }

    /// 텍스트로부터 직접 만든다 (테스트/임베드용).
    pub fn from_text(text: impl Into<String>) -> Self {
        Self { text: text.into(), briefing: None }
    }

    /// 룰북 원문이 비어 있는지.
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// **숙지 단계**: LLM에게 룰을 읽혀 핵심을 정리하게 한다 (게임 시작 전 1회).
    ///
    /// 성공하면 정리된 브리핑을 내부에 저장하고 그 참조를 반환한다. 이후
    /// [`context`](Self::context)는 원문 대신 이 브리핑을 반환한다.
    pub fn study(&mut self, llm: &LlmClient) -> io::Result<&str> {
        if self.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "rulebook is empty; nothing to study",
            ));
        }
        let briefing = llm.chat(&[
            ChatMessage::system(STUDY_SYSTEM_PROMPT),
            ChatMessage::user(format!("RULEBOOK:\n\n{}", self.text)),
        ])?;
        self.briefing = Some(briefing);
        Ok(self.briefing.as_deref().unwrap_or(""))
    }

    /// 숙지 여부.
    pub fn is_studied(&self) -> bool {
        self.briefing.is_some()
    }

    /// 프롬프트에 삽입할 게임 규칙 컨텍스트.
    ///
    /// 숙지된 브리핑이 있으면 그것을, 없으면 원문 룰북을 반환한다.
    pub fn context(&self) -> &str {
        self.briefing.as_deref().unwrap_or(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_falls_back_to_raw_text_before_study() {
        let rb = Rulebook::from_text("King capture wins.");
        assert!(!rb.is_studied());
        assert_eq!(rb.context(), "King capture wins.");
    }

    #[test]
    fn empty_rulebook_detected() {
        assert!(Rulebook::from_text("   \n  ").is_empty());
        assert!(!Rulebook::from_text("rule").is_empty());
    }

    #[test]
    fn study_empty_rulebook_errors() {
        let mut rb = Rulebook::from_text("");
        // LLM 없이도 빈 룰북은 즉시 거부되어야 한다. (더미 클라이언트로 호출)
        let llm = LlmClient::new("http://127.0.0.1:1", "dummy");
        let err = rb.study(&llm).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn load_from_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("ouroboros_test_rulebook.md");
        std::fs::write(&path, "# Rules\nCapture the king.").unwrap();
        let rb = Rulebook::load(path.to_str().unwrap()).unwrap();
        assert!(rb.context().contains("Capture the king"));
        let _ = std::fs::remove_file(&path);
    }
}
