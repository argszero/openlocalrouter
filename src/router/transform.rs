//! 协议格式转换（非流式）
//!
//! `Anthropic Messages` ↔ `OpenAI Chat Completions` 双向转换。
//! 支持 system prompts、tools、tool calls、images、thinking blocks。

use serde_json::{json, Value};

/// `Anthropic Messages` 请求 → `OpenAI Chat Completions` 请求
pub fn anthropic_to_openai_chat(body: &Value) -> Value {
    let mut result = json!({});

    if let Some(model) = body.get("model").and_then(|m| m.as_str()) {
        result["model"] = json!(model);
    }

    let mut messages = Vec::new();

    // System prompt
    if let Some(system) = body.get("system") {
        if let Some(text) = system.as_str() {
            let text = strip_billing_header(text);
            if !text.is_empty() {
                messages.push(json!({"role": "system", "content": text}));
            }
        } else if let Some(arr) = system.as_array() {
            for msg in arr {
                if let Some(text) = msg.get("text").and_then(|t| t.as_str()) {
                    let text = strip_billing_header(text);
                    if !text.is_empty() {
                        messages.push(json!({"role": "system", "content": text}));
                    }
                }
            }
        }
    }

    // Messages
    if let Some(msgs) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = msg.get("content");
            messages.extend(convert_message_to_openai(role, content));
        }
    }

    // Merge multiple system messages into one at position 0
    normalize_system_messages(&mut messages);
    result["messages"] = json!(messages);

    // Parameters
    if let Some(v) = body.get("max_tokens") {
        result["max_tokens"] = v.clone();
    }
    if let Some(v) = body.get("temperature") {
        result["temperature"] = v.clone();
    }
    if let Some(v) = body.get("top_p") {
        result["top_p"] = v.clone();
    }
    if let Some(v) = body.get("stop_sequences") {
        result["stop"] = v.clone();
    }
    if let Some(v) = body.get("stream") {
        result["stream"] = v.clone();
        // Inject stream_options.include_usage for usage reporting
        if v.as_bool().unwrap_or(false) {
            result["stream_options"] = json!({"include_usage": true});
        }
    }

    // Tools (skip BatchTool)
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        let openai_tools: Vec<Value> = tools
            .iter()
            .filter(|t| t.get("type").and_then(|v| v.as_str()) != Some("BatchTool"))
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                        "description": t.get("description"),
                        "parameters": t.get("input_schema").cloned().unwrap_or(json!({}))
                    }
                })
            })
            .collect();
        if !openai_tools.is_empty() {
            result["tools"] = json!(openai_tools);
        }
    }

    if let Some(v) = body.get("tool_choice") {
        result["tool_choice"] = map_tool_choice(v);
    }

    result
}

/// `OpenAI Chat Completions` 响应 → `Anthropic Messages` 响应
pub fn openai_chat_to_anthropic(body: &Value) -> Value {
    let choices = body.get("choices").and_then(|c| c.as_array());
    let choice = choices.and_then(|c| c.first());

    let message = choice.and_then(|c| c.get("message"));
    if message.is_none() {
        return json!({
            "id": body.get("id").and_then(|i| i.as_str()).unwrap_or(""),
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": body.get("model").and_then(|m| m.as_str()).unwrap_or(""),
            "stop_reason": null,
            "stop_sequence": null,
            "usage": json!({"input_tokens": 0, "output_tokens": 0})
        });
    }

    let message = message.unwrap();
    let mut content = Vec::new();
    let mut has_tool_use = false;

    // Reasoning content
    if let Some(reasoning) = message.get("reasoning_content").and_then(|r| r.as_str()) {
        if !reasoning.is_empty() {
            content.push(json!({"type": "thinking", "thinking": reasoning}));
        }
    }

    // Text/refusal content
    if let Some(msg_content) = message.get("content") {
        if let Some(text) = msg_content.as_str() {
            if !text.is_empty() {
                content.push(json!({"type": "text", "text": text}));
            }
        } else if let Some(parts) = msg_content.as_array() {
            for part in parts {
                let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match part_type {
                    "text" | "output_text" => {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            if !text.is_empty() {
                                content.push(json!({"type": "text", "text": text}));
                            }
                        }
                    }
                    "refusal" => {
                        if let Some(refusal) = part.get("refusal").and_then(|r| r.as_str()) {
                            if !refusal.is_empty() {
                                content.push(json!({"type": "text", "text": refusal}));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Tool calls
    if let Some(tool_calls) = message.get("tool_calls").and_then(|t| t.as_array()) {
        if !tool_calls.is_empty() {
            has_tool_use = true;
        }
        for tc in tool_calls {
            let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let func = tc.get("function");
            let name = func
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let args_str = func
                .and_then(|f| f.get("arguments"))
                .and_then(|a| a.as_str())
                .unwrap_or("{}");
            let input: Value = serde_json::from_str(args_str).unwrap_or(json!({}));

            content.push(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input
            }));
        }
    }

    // Stop reason
    let stop_reason = choice
        .and_then(|c| c.get("finish_reason"))
        .and_then(|r| r.as_str())
        .map(|r| match r {
            "stop" => "end_turn",
            "length" => "max_tokens",
            "tool_calls" | "function_call" => "tool_use",
            "content_filter" => "end_turn",
            _ => "end_turn",
        })
        .or(if has_tool_use { Some("tool_use") } else { None });

    // Usage
    let usage = body.get("usage");
    let usage_json = build_usage(usage);

    json!({
        "id": body.get("id").and_then(|i| i.as_str()).unwrap_or(""),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": body.get("model").and_then(|m| m.as_str()).unwrap_or(""),
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": usage_json
    })
}

// ── helpers ──

const BILLING_HEADER_PREFIX: &str = "x-anthropic-billing-header:";

fn strip_billing_header(text: &str) -> &str {
    if !text.starts_with(BILLING_HEADER_PREFIX) {
        return text;
    }
    let line_end = match text
        .as_bytes()
        .iter()
        .position(|b| *b == b'\n' || *b == b'\r')
    {
        Some(p) => p,
        None => return "",
    };
    let bytes = text.as_bytes();
    let mut rest_start = line_end + 1;
    if bytes[line_end] == b'\r' && bytes.get(line_end + 1) == Some(&b'\n') {
        rest_start += 1;
    }
    let rest = &text[rest_start..];
    rest.strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .or_else(|| rest.strip_prefix('\r'))
        .unwrap_or(rest)
}

fn convert_message_to_openai(role: &str, content: Option<&Value>) -> Vec<Value> {
    let mut result = Vec::new();
    let content = match content {
        Some(c) => c,
        None => {
            result.push(json!({"role": role, "content": null}));
            return result;
        }
    };

    if let Some(text) = content.as_str() {
        result.push(json!({"role": role, "content": text}));
        return result;
    }

    if let Some(blocks) = content.as_array() {
        let mut text_parts = String::new();
        let mut content_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in blocks {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match block_type {
                "text" => {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        text_parts.push_str(text);
                        content_parts.push(json!({"type": "text", "text": text}));
                    }
                }
                "image" => {
                    if let Some(source) = block.get("source") {
                        let media_type = source
                            .get("media_type")
                            .and_then(|m| m.as_str())
                            .unwrap_or("image/png");
                        let data = source.get("data").and_then(|d| d.as_str()).unwrap_or("");
                        content_parts.push(json!({
                            "type": "image_url",
                            "image_url": {"url": format!("data:{};base64,{}", media_type, data)}
                        }));
                    }
                }
                "tool_use" => {
                    let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                    let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let input = block.get("input").cloned().unwrap_or(json!({}));
                    tool_calls.push(json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": serde_json::to_string(&input).unwrap_or_default()
                        }
                    }));
                }
                "tool_result" => {
                    let tool_use_id = block
                        .get("tool_use_id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("");
                    let content_val = block.get("content");
                    let content_str = match content_val {
                        Some(Value::String(s)) => s.clone(),
                        Some(v) => v.to_string(),
                        None => String::new(),
                    };
                    result.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_use_id,
                        "content": content_str
                    }));
                }
                _ => {}
            }
        }

        if !content_parts.is_empty() || !tool_calls.is_empty() {
            let mut msg = json!({"role": role});

            if text_parts.len() <= 50 && content_parts.len() == 1 {
                msg["content"] = json!(text_parts);
            } else if content_parts.is_empty() {
                msg["content"] = Value::Null;
            } else {
                msg["content"] = json!(content_parts);
            }

            if !tool_calls.is_empty() {
                msg["tool_calls"] = json!(tool_calls);
            }

            result.push(msg);
        }

        return result;
    }

    result.push(json!({"role": role, "content": content}));
    result
}

fn normalize_system_messages(messages: &mut Vec<Value>) {
    let system_count = messages
        .iter()
        .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("system"))
        .count();
    if system_count <= 1 {
        return;
    }

    let mut parts = Vec::new();
    messages.retain(|m| {
        if m.get("role").and_then(|v| v.as_str()) != Some("system") {
            return true;
        }
        match m.get("content") {
            Some(Value::String(text)) if !text.is_empty() => parts.push(text.clone()),
            _ => {}
        }
        false
    });

    if !parts.is_empty() {
        messages.insert(0, json!({"role": "system", "content": parts.join("\n")}));
    }
}

fn map_tool_choice(tool_choice: &Value) -> Value {
    match tool_choice {
        Value::String(s) => match s.as_str() {
            "any" => json!("required"),
            _ => json!(s),
        },
        Value::Object(obj) => match obj.get("type").and_then(|t| t.as_str()) {
            Some("any") => json!("required"),
            Some("auto") => json!("auto"),
            Some("none") => json!("none"),
            Some("tool") => {
                let name = obj.get("name").and_then(|n| n.as_str()).unwrap_or("");
                json!({"type": "function", "function": {"name": name}})
            }
            _ => tool_choice.clone(),
        },
        _ => tool_choice.clone(),
    }
}

fn build_usage(usage: Option<&Value>) -> Value {
    let usage = match usage {
        Some(u) => u,
        None => return json!({"input_tokens": 0, "output_tokens": 0}),
    };

    let cached = usage
        .get("cache_read_input_tokens")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| {
            usage
                .pointer("/prompt_tokens_details/cached_tokens")
                .and_then(serde_json::Value::as_u64)
        })
        .unwrap_or(0);
    let cache_creation = usage
        .get("cache_creation_input_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        .saturating_sub(cached)
        .saturating_sub(cache_creation);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    let mut usage_json = json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens
    });
    if cached > 0 {
        usage_json["cache_read_input_tokens"] = json!(cached);
    }
    if cache_creation > 0 {
        usage_json["cache_creation_input_tokens"] = json!(cache_creation);
    }
    usage_json
}

/// `OpenAI Responses` 请求 → `OpenAI Chat Completions` 请求
///
/// 将 Responses API 格式降级为 Chat Completions 格式：
/// - `input` (string 或 messages 数组) → `messages`
/// - `instructions` → 前置 system message
/// - `max_output_tokens` → `max_tokens`
pub fn openai_responses_to_openai_chat(body: &Value) -> Value {
    let mut result = json!({});

    if let Some(model) = body.get("model").and_then(|m| m.as_str()) {
        result["model"] = json!(model);
    }

    let mut messages = Vec::new();

    // instructions → system message
    if let Some(instructions) = body.get("instructions").and_then(|i| i.as_str()) {
        if !instructions.is_empty() {
            messages.push(json!({"role": "system", "content": instructions}));
        }
    }

    // input → messages
    if let Some(input) = body.get("input") {
        match input {
            Value::String(text) => {
                messages.push(json!({"role": "user", "content": text}));
            }
            Value::Array(arr) => {
                for item in arr {
                    messages.push(convert_responses_message(item));
                }
            }
            _ => {}
        }
    }

    // If no messages extracted, try the legacy "messages" field
    if messages.is_empty() {
        if let Some(msgs) = body.get("messages").and_then(|m| m.as_array()) {
            for msg in msgs {
                if let Some(role) = msg.get("role").and_then(|r| r.as_str()) {
                    let role = match role {
                        "developer" => "system",
                        other => other,
                    };
                    let content = msg.get("content").cloned().unwrap_or(Value::Null);
                    messages.push(json!({"role": role, "content": content}));
                }
            }
        }
    }

    result["messages"] = json!(messages);

    // Parameters
    if let Some(v) = body.get("max_output_tokens").or(body.get("max_tokens")) {
        result["max_tokens"] = v.clone();
    }
    if let Some(v) = body.get("temperature") {
        result["temperature"] = v.clone();
    }
    if let Some(v) = body.get("top_p") {
        result["top_p"] = v.clone();
    }
    if let Some(v) = body.get("stream") {
        result["stream"] = v.clone();
        if v.as_bool().unwrap_or(false) {
            result["stream_options"] = json!({"include_usage": true});
        }
    }

    result
}

/// `OpenAI Chat Completions` 响应 → `OpenAI Responses` 响应
///
/// 将 Chat Completions 格式包装为 Responses API 格式：
/// - `choices[0].message.content` → `output[0].content[]`
/// - `usage` → `usage`
pub fn openai_chat_to_openai_responses(body: &Value) -> Value {
    let choices = body.get("choices").and_then(|c| c.as_array());
    let choice = choices.and_then(|c| c.first());

    let mut output = Vec::new();

    if let Some(message) = choice.and_then(|c| c.get("message")) {
        let mut content_parts = Vec::new();

        // Text content
        if let Some(text) = message.get("content").and_then(|c| c.as_str()) {
            if !text.is_empty() {
                content_parts.push(json!({"type": "output_text", "text": text, "annotations": []}));
            }
        } else if let Some(parts) = message.get("content").and_then(|c| c.as_array()) {
            for part in parts {
                let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match part_type {
                    "text" | "output_text" => {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            content_parts.push(
                                json!({"type": "output_text", "text": text, "annotations": []}),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        // Tool calls → function_call output
        if let Some(tool_calls) = message.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tool_calls {
                let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
                let func = tc.get("function");
                let name = func
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let args = func
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())
                    .unwrap_or("{}");
                let arguments: Value = serde_json::from_str(args).unwrap_or(json!({}));
                output.push(json!({
                    "id": format!("fc_{id}"),
                    "type": "function_call",
                    "call_id": id,
                    "name": name,
                    "arguments": serde_json::to_string(&arguments).unwrap_or_default()
                }));
            }
        }

        if !content_parts.is_empty() {
            output.push(json!({
                "id": body.get("id").and_then(|i| i.as_str()).unwrap_or(""),
                "type": "message",
                "role": "assistant",
                "content": content_parts
            }));
        }
    }

    // Usage
    let usage = body.get("usage");
    let usage_json = match usage {
        Some(u) => json!({
            "input_tokens": u.get("prompt_tokens").and_then(serde_json::Value::as_u64).unwrap_or(0),
            "output_tokens": u.get("completion_tokens").and_then(serde_json::Value::as_u64).unwrap_or(0),
            "total_tokens": u.get("total_tokens").and_then(serde_json::Value::as_u64).unwrap_or(0)
        }),
        None => json!({"input_tokens": 0, "output_tokens": 0, "total_tokens": 0}),
    };

    json!({
        "id": body.get("id").and_then(|i| i.as_str()).unwrap_or(""),
        "object": "response",
        "model": body.get("model").and_then(|m| m.as_str()).unwrap_or(""),
        "output": output,
        "usage": usage_json
    })
}

fn convert_responses_message(item: &Value) -> Value {
    let raw_role = item.get("role").and_then(|r| r.as_str()).unwrap_or("user");
    // Map OpenAI-specific roles that upstream providers may not support
    let role = match raw_role {
        "developer" => "system",
        other => other,
    };
    let content = item.get("content").cloned().unwrap_or(Value::Null);

    // OpenAI Responses content format: could be string or array
    if content.is_string() {
        return json!({"role": role, "content": content});
    }

    if let Some(arr) = content.as_array() {
        let mut texts = Vec::new();
        for part in arr {
            if let Some(part_type) = part.get("type").and_then(|t| t.as_str()) {
                match part_type {
                    "input_text" | "output_text" | "text" => {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            texts.push(text.to_string());
                        }
                    }
                    "input_image" => {
                        // Pass through images as content array
                        return json!({"role": role, "content": content});
                    }
                    _ => {}
                }
            }
        }
        if !texts.is_empty() {
            return json!({"role": role, "content": texts.join("\n")});
        }
    }

    json!({"role": role, "content": content})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_to_openai_basic() {
        let input = json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let result = anthropic_to_openai_chat(&input);
        assert_eq!(result["messages"][0]["role"], "user");
        assert_eq!(result["messages"][0]["content"], "Hello");
    }

    #[test]
    fn test_anthropic_to_openai_with_system() {
        let input = json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 1024,
            "system": "You are helpful.",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let result = anthropic_to_openai_chat(&input);
        assert_eq!(result["messages"][0]["role"], "system");
        assert_eq!(result["messages"][0]["content"], "You are helpful.");
    }

    #[test]
    fn test_anthropic_to_openai_strips_billing_header() {
        let input = json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 1024,
            "system": "x-anthropic-billing-header: cc_version=2.1\n\nYou are helpful.",
            "messages": [{"role": "user", "content": "Hi"}]
        });
        let result = anthropic_to_openai_chat(&input);
        assert_eq!(result["messages"][0]["content"], "You are helpful.");
    }

    #[test]
    fn test_anthropic_to_openai_with_tools() {
        let input = json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "weather?"}],
            "tools": [{
                "name": "get_weather",
                "description": "Get weather",
                "input_schema": {"type": "object", "properties": {"city": {"type": "string"}}}
            }]
        });
        let result = anthropic_to_openai_chat(&input);
        assert_eq!(result["tools"][0]["type"], "function");
        assert_eq!(result["tools"][0]["function"]["name"], "get_weather");
    }

    #[test]
    fn test_anthropic_to_openai_tool_use() {
        let input = json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 1024,
            "messages": [{
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "call_123",
                    "name": "get_weather",
                    "input": {"city": "Tokyo"}
                }]
            }]
        });
        let result = anthropic_to_openai_chat(&input);
        let msg = &result["messages"][0];
        assert!(msg.get("tool_calls").is_some());
        assert_eq!(msg["tool_calls"][0]["id"], "call_123");
        assert_eq!(msg["tool_calls"][0]["function"]["name"], "get_weather");
    }

    #[test]
    fn test_anthropic_to_openai_tool_result() {
        let input = json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 1024,
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "call_123",
                    "content": "Sunny, 25C"
                }]
            }]
        });
        let result = anthropic_to_openai_chat(&input);
        assert_eq!(result["messages"][0]["role"], "tool");
        assert_eq!(result["messages"][0]["tool_call_id"], "call_123");
    }

    #[test]
    fn test_openai_to_anthropic_basic() {
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hello!"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        });
        let result = openai_chat_to_anthropic(&input);
        assert_eq!(result["content"][0]["type"], "text");
        assert_eq!(result["content"][0]["text"], "Hello!");
        assert_eq!(result["stop_reason"], "end_turn");
        assert_eq!(result["usage"]["input_tokens"], 10);
        assert_eq!(result["usage"]["output_tokens"], 5);
    }

    #[test]
    fn test_openai_to_anthropic_with_tool_calls() {
        let input = json!({
            "id": "chatcmpl-456",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_789",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": "{\"city\":\"Tokyo\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        });
        let result = openai_chat_to_anthropic(&input);
        assert_eq!(result["content"][0]["type"], "tool_use");
        assert_eq!(result["content"][0]["id"], "call_789");
        assert_eq!(result["content"][0]["name"], "get_weather");
        assert_eq!(result["content"][0]["input"]["city"], "Tokyo");
        assert_eq!(result["stop_reason"], "tool_use");
    }

    #[test]
    fn test_openai_to_anthropic_with_cache_tokens() {
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hi"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "prompt_tokens_details": {"cached_tokens": 80}
            }
        });
        let result = openai_chat_to_anthropic(&input);
        assert_eq!(result["usage"]["input_tokens"], 20); // 100 - 80
        assert_eq!(result["usage"]["output_tokens"], 50);
        assert_eq!(result["usage"]["cache_read_input_tokens"], 80);
    }
}
