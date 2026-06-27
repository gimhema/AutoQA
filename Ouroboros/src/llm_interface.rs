//! 로컬 LLM 서버와의 HTTP 통신 계층.
//!
//! OpenAI 호환 `/v1/chat/completions` 엔드포인트를 사용한다.
//! llama.cpp server, Ollama, LM Studio 등 로컬 LLM 서버라면 어디든 연결 가능.
//! Ouroboros는 HTTP 클라이언트만 포함하며 추론은 별도 프로세스가 담당한다.

use std::io;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

pub struct LlmClient {
    endpoint: String,
    model: String,
    temperature: f64,
}

impl LlmClient {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            temperature: 0.7,
        }
    }

    pub fn set_temperature(&mut self, temperature: f64) {
        self.temperature = temperature;
    }

    /// 메시지 배열을 보내고 assistant 응답 텍스트를 반환한다.
    pub fn chat(&self, messages: &[ChatMessage]) -> io::Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": self.temperature,
        });

        let response: Value = ureq::post(&self.endpoint)
            .send_json(&body)
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e.to_string()))?
            .body_mut()
            .read_json()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        extract_content(&response)
    }
}

fn extract_content(response: &Value) -> io::Result<String> {
    response
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected response structure: {response}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn chat_message_constructors() {
        let sys = ChatMessage::system("you are a bot");
        assert_eq!(sys.role, "system");
        assert_eq!(sys.content, "you are a bot");

        let usr = ChatMessage::user("hello");
        assert_eq!(usr.role, "user");

        let asst = ChatMessage::assistant("hi");
        assert_eq!(asst.role, "assistant");
    }

    #[test]
    fn chat_message_serializes_to_openai_format() {
        let msg = ChatMessage::user("test");
        let v = serde_json::to_value(&msg).unwrap();
        assert_eq!(v, json!({"role": "user", "content": "test"}));
    }

    #[test]
    fn extract_content_from_valid_response() {
        let resp = json!({
            "choices": [{
                "message": { "role": "assistant", "content": "hello world" },
                "finish_reason": "stop"
            }]
        });
        assert_eq!(extract_content(&resp).unwrap(), "hello world");
    }

    #[test]
    fn extract_content_fails_on_missing_choices() {
        let resp = json!({"error": "bad request"});
        assert!(extract_content(&resp).is_err());
    }

    #[test]
    fn extract_content_fails_on_empty_choices() {
        let resp = json!({"choices": []});
        assert!(extract_content(&resp).is_err());
    }
}
