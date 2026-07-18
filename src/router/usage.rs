//! 用量提取和记录
//!
//! 从各协议（OpenAI Chat、OpenAI Responses、Anthropic Messages）的
//! 响应中提取 token 用量信息，用于写入 usage_records 表。

use serde_json::Value;

/// 从响应中提取的 token 用量
#[derive(Debug, Default, Clone)]
pub(crate) struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
}

impl TokenUsage {
    /// 是否为零用量（无法提取到有效数据）
    pub fn is_zero(&self) -> bool {
        self.input_tokens == 0 && self.output_tokens == 0 && self.cache_read_tokens == 0
    }
}

/// 用量记录上下文，在 proxy_request 认证后构建
#[derive(Debug, Clone)]
pub(crate) struct UsageContext {
    pub api_key_id: String,
    pub key_owner_id: String,
    pub endpoint_id: String,
    pub user_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model: String,
}

/// 从各协议的非流式响应 body 中提取 usage
///
/// 支持格式：
/// - OpenAI Chat:     `{ "usage": { "prompt_tokens": N, "completion_tokens": N } }`
/// - OpenAI Responses: `{ "usage": { "input_tokens": N, "output_tokens": N } }`
/// - Anthropic:       `{ "usage": { "input_tokens": N, "output_tokens": N } }`
pub(crate) fn extract_usage_from_body(data: &[u8]) -> Option<TokenUsage> {
    let v: Value = serde_json::from_slice(data).ok()?;
    let usage = v.get("usage")?;

    // OpenAI Chat: prompt_tokens / completion_tokens
    if let (Some(prompt), Some(completion)) = (
        usage.get("prompt_tokens").and_then(|v| v.as_u64()),
        usage.get("completion_tokens").and_then(|v| v.as_u64()),
    ) {
        let cached = usage
            .get("prompt_tokens_details")
            .and_then(|d| d.get("cached_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        return Some(TokenUsage {
            input_tokens: prompt.saturating_sub(cached) as u32,
            output_tokens: completion as u32,
            cache_read_tokens: cached as u32,
        });
    }

    // OpenAI Responses / Anthropic: input_tokens / output_tokens
    if let (Some(input), Some(output)) = (
        usage.get("input_tokens").and_then(|v| v.as_u64()),
        usage.get("output_tokens").and_then(|v| v.as_u64()),
    ) {
        let cached = usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        return Some(TokenUsage {
            input_tokens: input as u32,
            output_tokens: output as u32,
            cache_read_tokens: cached as u32,
        });
    }

    None
}

/// 从 OpenAI Chat SSE chunk 提取 usage（last chunk before [DONE]）
///
/// 格式: `{ "usage": { "prompt_tokens": N, "completion_tokens": N, ... } }`
/// 注意：SSE chunk 可能不是纯 JSON，需要先处理 `data: ` 前缀
pub(crate) fn extract_usage_from_chat_sse(chunk: &Value) -> Option<TokenUsage> {
    chunk.get("usage").map(|u| {
        let prompt = u
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let completion = u
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cached = u
            .get("prompt_tokens_details")
            .and_then(|d| d.get("cached_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        TokenUsage {
            input_tokens: prompt.saturating_sub(cached) as u32,
            output_tokens: completion as u32,
            cache_read_tokens: cached as u32,
        }
    })
}

/// 从 Anthropic SSE message_delta 事件提取 usage
///
/// 格式: `{ "type": "message_delta", "usage": { "input_tokens": N, "output_tokens": N } }`
pub(crate) fn extract_usage_from_anthropic_sse(chunk: &Value) -> Option<TokenUsage> {
    if chunk.get("type")?.as_str()? != "message_delta" {
        return None;
    }
    let usage = chunk.get("usage")?;
    let cached = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    Some(TokenUsage {
        input_tokens: usage
            .get("input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: usage
            .get("output_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        cache_read_tokens: cached as u32,
    })
}

/// 从 OpenAI Responses SSE response.completed 事件提取 usage
///
/// 格式: `{ "type": "response.completed", "response": { "usage": { ... } } }`
#[allow(dead_code)]
pub(crate) fn extract_usage_from_responses_sse(chunk: &Value) -> Option<TokenUsage> {
    if chunk.get("type")?.as_str()? != "response.completed" {
        return None;
    }
    let usage = chunk.get("response")?.get("usage")?;

    let input = usage
        .get("input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cached = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    Some(TokenUsage {
        input_tokens: input as u32,
        output_tokens: output as u32,
        cache_read_tokens: cached as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_usage_openai_chat() {
        let body = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "model": "gpt-5.5",
            "choices": [],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "total_tokens": 150
            }
        });
        let data = serde_json::to_vec(&body).unwrap();
        let usage = extract_usage_from_body(&data).unwrap();
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
    }

    #[test]
    fn test_extract_usage_openai_chat_with_cached() {
        let body = json!({
            "usage": {
                "prompt_tokens": 200,
                "completion_tokens": 80,
                "prompt_tokens_details": {"cached_tokens": 150}
            }
        });
        let data = serde_json::to_vec(&body).unwrap();
        let usage = extract_usage_from_body(&data).unwrap();
        assert_eq!(usage.input_tokens, 50); // 200 - 150
        assert_eq!(usage.output_tokens, 80);
        assert_eq!(usage.cache_read_tokens, 150);
    }

    #[test]
    fn test_extract_usage_openai_responses() {
        let body = json!({
            "id": "resp_123",
            "object": "response",
            "usage": {
                "input_tokens": 300,
                "output_tokens": 120,
                "total_tokens": 420
            }
        });
        let data = serde_json::to_vec(&body).unwrap();
        let usage = extract_usage_from_body(&data).unwrap();
        assert_eq!(usage.input_tokens, 300);
        assert_eq!(usage.output_tokens, 120);
    }

    #[test]
    fn test_extract_usage_anthropic() {
        let body = json!({
            "id": "msg_123",
            "type": "message",
            "usage": {
                "input_tokens": 50,
                "output_tokens": 30
            }
        });
        let data = serde_json::to_vec(&body).unwrap();
        let usage = extract_usage_from_body(&data).unwrap();
        assert_eq!(usage.input_tokens, 50);
        assert_eq!(usage.output_tokens, 30);
    }

    #[test]
    fn test_extract_usage_no_usage_field() {
        let body = json!({"id": "123", "object": "chat.completion"});
        let data = serde_json::to_vec(&body).unwrap();
        assert!(extract_usage_from_body(&data).is_none());
    }

    #[test]
    fn test_extract_usage_invalid_json() {
        assert!(extract_usage_from_body(b"not json").is_none());
    }

    #[test]
    fn test_extract_usage_chat_sse() {
        let chunk = json!({
            "id": "chatcmpl-123",
            "choices": [{"finish_reason": "stop"}],
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": 25,
                "total_tokens": 75
            }
        });
        let usage = extract_usage_from_chat_sse(&chunk).unwrap();
        assert_eq!(usage.input_tokens, 50);
        assert_eq!(usage.output_tokens, 25);
    }

    #[test]
    fn test_extract_usage_anthropic_sse() {
        let chunk = json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {
                "input_tokens": 100,
                "output_tokens": 60
            }
        });
        let usage = extract_usage_from_anthropic_sse(&chunk).unwrap();
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 60);
    }

    #[test]
    fn test_extract_usage_anthropic_sse_not_delta() {
        let chunk = json!({"type": "content_block_start", "index": 0});
        assert!(extract_usage_from_anthropic_sse(&chunk).is_none());
    }

    #[test]
    fn test_extract_usage_responses_sse() {
        let chunk = json!({
            "type": "response.completed",
            "response": {
                "id": "resp_123",
                "usage": {
                    "input_tokens": 200,
                    "output_tokens": 80,
                    "total_tokens": 280
                }
            }
        });
        let usage = extract_usage_from_responses_sse(&chunk).unwrap();
        assert_eq!(usage.input_tokens, 200);
        assert_eq!(usage.output_tokens, 80);
    }

    #[test]
    fn test_token_usage_is_zero() {
        assert!(TokenUsage::default().is_zero());
        assert!(!TokenUsage {
            input_tokens: 1,
            ..Default::default()
        }
        .is_zero());
    }
}
