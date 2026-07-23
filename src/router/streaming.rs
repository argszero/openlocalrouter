//! 流式响应转换
//!
//! `OpenAI` `SSE` → `Anthropic` `SSE` 格式转换。
//! 支持工具调用、thinking (reasoning) blocks、usage 映射。

use crate::router::sse::{append_utf8_safe, strip_sse_field, take_sse_block};
use bytes::Bytes;
use futures::stream::Stream;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// `OpenAI` 流式响应 chunk
#[derive(Debug, Deserialize)]
struct OpenAIStreamChunk {
    #[serde(default)]
    id: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<StreamUsage>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Delta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default, alias = "reasoning_content")]
    reasoning: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<DeltaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct DeltaToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type", default)]
    #[allow(dead_code)]
    call_type: Option<String>,
    #[serde(default)]
    function: Option<DeltaFunction>,
}

#[derive(Debug, Deserialize)]
struct DeltaFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct PromptTokensDetails {
    #[serde(default)]
    cached_tokens: u32,
}

/// Tool block state tracker
#[derive(Debug, Clone)]
struct ToolBlockState {
    anthropic_index: u32,
    id: String,
    name: String,
    started: bool,
    pending_args: String,
}

/// Create an Anthropic SSE stream from an `OpenAI` SSE stream.
/// Supports passthrough + transform of tool calls, thinking blocks, and usage.
pub fn openai_sse_to_anthropic<E: std::error::Error + Send + 'static>(
    stream: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send {
    async_stream::stream! {
        let mut buffer = String::new();
        let mut utf8_remainder: Vec<u8> = Vec::new();
        let mut message_id = None;
        let mut current_model = None;
        let mut next_content_index: u32 = 0;
        let mut has_sent_message_start = false;
        let mut has_emitted_message_delta = false;
        let mut pending_message_delta: Option<(Option<String>, Option<Value>)> = None;
        let mut has_sent_message_stop = false;
        let mut stream_ended_with_error = false;
        let mut latest_usage: Option<Value> = None;
        let mut current_non_tool_block_type: Option<&'static str> = None;
        let mut current_non_tool_block_index: Option<u32> = None;
        let mut tool_blocks_by_index: HashMap<usize, ToolBlockState> = HashMap::new();
        let mut open_tool_block_indices: Vec<u32> = Vec::new();

        tokio::pin!(stream);

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    append_utf8_safe(&mut buffer, &mut utf8_remainder, &bytes);

                    while let Some(line) = take_sse_block(&mut buffer) {
                        if line.trim().is_empty() {
                            continue;
                        }

                        for l in line.lines() {
                            if let Some(data) = strip_sse_field(l, "data") {
                                if data.trim() == "[DONE]" {
                                    log::debug!("[Proxy] <<< OpenAI SSE: [DONE]");

                                    if let Some((stop_reason, usage_json)) = pending_message_delta.take() {
                                        let event = build_message_delta_event(stop_reason, usage_json);
                                        let sse_data = format!(
                                            "event: message_delta\ndata: {}\n\n",
                                            serde_json::to_string(&event).unwrap_or_default()
                                        );
                                        yield Ok(Bytes::from(sse_data));
                                    }

                                    let event = json!({"type": "message_stop"});
                                    let sse_data = format!(
                                        "event: message_stop\ndata: {}\n\n",
                                        serde_json::to_string(&event).unwrap_or_default()
                                    );
                                    yield Ok(Bytes::from(sse_data));
                                    has_sent_message_stop = true;
                                    continue;
                                }

                                if let Ok(chunk) = serde_json::from_str::<OpenAIStreamChunk>(data) {
                                    if message_id.is_none() && !chunk.id.is_empty() {
                                        message_id = Some(chunk.id.clone());
                                    }
                                    if current_model.is_none() && !chunk.model.is_empty() {
                                        current_model = Some(chunk.model.clone());
                                    }

                                    let chunk_usage_json =
                                        chunk.usage.as_ref().map(build_anthropic_usage_json);
                                    if let Some(ref usage_json) = chunk_usage_json {
                                        latest_usage = Some(usage_json.clone());
                                        if let Some((_, ref mut pending_usage)) = pending_message_delta {
                                            *pending_usage = Some(usage_json.clone());
                                        }
                                    }

                                    if let Some(choice) = chunk.choices.first() {
                                        // Send message_start on first chunk
                                        if !has_sent_message_start {
                                            let event = json!({
                                                "type": "message_start",
                                                "message": {
                                                    "id": message_id.clone().unwrap_or_default(),
                                                    "type": "message",
                                                    "role": "assistant",
                                                    "model": current_model.clone().unwrap_or_default(),
                                                    "usage": { "input_tokens": 0, "output_tokens": 0 }
                                                }
                                            });
                                            let sse_data = format!(
                                                "event: message_start\ndata: {}\n\n",
                                                serde_json::to_string(&event).unwrap_or_default()
                                            );
                                            yield Ok(Bytes::from(sse_data));
                                            has_sent_message_start = true;
                                        }

                                        // Handle reasoning (thinking)
                                        if let Some(reasoning) = &choice.delta.reasoning {
                                            if current_non_tool_block_type != Some("thinking") {
                                                if let Some(index) = current_non_tool_block_index.take() {
                                                    let event = json!({
                                                        "type": "content_block_stop", "index": index
                                                    });
                                                    let sse_data = format!(
                                                        "event: content_block_stop\ndata: {}\n\n",
                                                        serde_json::to_string(&event).unwrap_or_default()
                                                    );
                                                    yield Ok(Bytes::from(sse_data));
                                                }
                                                let index = next_content_index;
                                                next_content_index += 1;
                                                let event = json!({
                                                    "type": "content_block_start",
                                                    "index": index,
                                                    "content_block": { "type": "thinking", "thinking": "" }
                                                });
                                                let sse_data = format!(
                                                    "event: content_block_start\ndata: {}\n\n",
                                                    serde_json::to_string(&event).unwrap_or_default()
                                                );
                                                yield Ok(Bytes::from(sse_data));
                                                current_non_tool_block_type = Some("thinking");
                                                current_non_tool_block_index = Some(index);
                                            }
                                            if let Some(index) = current_non_tool_block_index {
                                                let event = json!({
                                                    "type": "content_block_delta",
                                                    "index": index,
                                                    "delta": { "type": "thinking_delta", "thinking": reasoning }
                                                });
                                                let sse_data = format!(
                                                    "event: content_block_delta\ndata: {}\n\n",
                                                    serde_json::to_string(&event).unwrap_or_default()
                                                );
                                                yield Ok(Bytes::from(sse_data));
                                            }
                                        }

                                        // Handle text content
                                        if let Some(content) = &choice.delta.content {
                                            if !content.is_empty() {
                                                if current_non_tool_block_type != Some("text") {
                                                    if let Some(index) = current_non_tool_block_index.take() {
                                                        let event = json!({
                                                            "type": "content_block_stop", "index": index
                                                        });
                                                        let sse_data = format!(
                                                            "event: content_block_stop\ndata: {}\n\n",
                                                            serde_json::to_string(&event).unwrap_or_default()
                                                        );
                                                        yield Ok(Bytes::from(sse_data));
                                                    }
                                                    let index = next_content_index;
                                                    next_content_index += 1;
                                                    let event = json!({
                                                        "type": "content_block_start",
                                                        "index": index,
                                                        "content_block": { "type": "text", "text": "" }
                                                    });
                                                    let sse_data = format!(
                                                        "event: content_block_start\ndata: {}\n\n",
                                                        serde_json::to_string(&event).unwrap_or_default()
                                                    );
                                                    yield Ok(Bytes::from(sse_data));
                                                    current_non_tool_block_type = Some("text");
                                                    current_non_tool_block_index = Some(index);
                                                }
                                                if let Some(index) = current_non_tool_block_index {
                                                    let event = json!({
                                                        "type": "content_block_delta",
                                                        "index": index,
                                                        "delta": { "type": "text_delta", "text": content }
                                                    });
                                                    let sse_data = format!(
                                                        "event: content_block_delta\ndata: {}\n\n",
                                                        serde_json::to_string(&event).unwrap_or_default()
                                                    );
                                                    yield Ok(Bytes::from(sse_data));
                                                }
                                            }
                                        }

                                        // Handle tool calls
                                        if let Some(tool_calls) = &choice.delta.tool_calls {
                                            if !tool_calls.is_empty() {
                                                if let Some(index) = current_non_tool_block_index.take() {
                                                    let event = json!({
                                                        "type": "content_block_stop", "index": index
                                                    });
                                                    let sse_data = format!(
                                                        "event: content_block_stop\ndata: {}\n\n",
                                                        serde_json::to_string(&event).unwrap_or_default()
                                                    );
                                                    yield Ok(Bytes::from(sse_data));
                                                }
                                                current_non_tool_block_type = None;

                                                for tool_call in tool_calls {
                                                    let (anthropic_index, should_start, pending_after_start, immediate_delta) = {
                                                        let state = tool_blocks_by_index
                                                            .entry(tool_call.index)
                                                            .or_insert_with(|| {
                                                                let index = next_content_index;
                                                                next_content_index += 1;
                                                                ToolBlockState {
                                                                    anthropic_index: index,
                                                                    id: String::new(),
                                                                    name: String::new(),
                                                                    started: false,
                                                                    pending_args: String::new(),
                                                                }
                                                            });

                                                        if let Some(id) = &tool_call.id {
                                                            state.id.clone_from(id);
                                                        }
                                                        if let Some(function) = &tool_call.function {
                                                            if let Some(name) = &function.name {
                                                                state.name.clone_from(name);
                                                            }
                                                        }

                                                        let should_start =
                                                            !state.started && !state.id.is_empty() && !state.name.is_empty();
                                                        if should_start {
                                                            state.started = true;
                                                        }
                                                        let pending_after_start = if should_start && !state.pending_args.is_empty() {
                                                            Some(std::mem::take(&mut state.pending_args))
                                                        } else {
                                                            None
                                                        };
                                                        let args_delta = tool_call.function.as_ref().and_then(|f| f.arguments.clone());
                                                        let immediate_delta = args_delta.and_then(|args| {
                                                            if state.started {
                                                                Some(args)
                                                            } else {
                                                                state.pending_args.push_str(&args);
                                                                None
                                                            }
                                                        });
                                                        (state.anthropic_index, should_start, pending_after_start, immediate_delta)
                                                    };

                                                    if should_start {
                                                        let state = &tool_blocks_by_index[&tool_call.index];
                                                        let event = json!({
                                                            "type": "content_block_start",
                                                            "index": anthropic_index,
                                                            "content_block": {
                                                                "type": "tool_use",
                                                                "id": state.id,
                                                                "name": state.name
                                                            }
                                                        });
                                                        let sse_data = format!(
                                                            "event: content_block_start\ndata: {}\n\n",
                                                            serde_json::to_string(&event).unwrap_or_default()
                                                        );
                                                        yield Ok(Bytes::from(sse_data));
                                                        open_tool_block_indices.push(anthropic_index);
                                                    }

                                                    for args in [pending_after_start, immediate_delta].iter().flatten() {
                                                        let event = json!({
                                                            "type": "content_block_delta",
                                                            "index": anthropic_index,
                                                            "delta": { "type": "input_json_delta", "partial_json": args }
                                                        });
                                                        let sse_data = format!(
                                                            "event: content_block_delta\ndata: {}\n\n",
                                                            serde_json::to_string(&event).unwrap_or_default()
                                                        );
                                                        yield Ok(Bytes::from(sse_data));
                                                    }
                                                }
                                            }
                                        }

                                        // Handle finish_reason — defer to [DONE]
                                        if let Some(finish_reason) = &choice.finish_reason {
                                            let stop_reason = map_stop_reason(Some(finish_reason));
                                            let usage_json = chunk_usage_json.clone().or_else(|| latest_usage.clone());

                                            if has_emitted_message_delta {
                                                if let (Some((_, ref mut usage)), Some(uj)) = (&mut pending_message_delta, usage_json) {
                                                    *usage = Some(uj);
                                                }
                                                continue;
                                            }
                                            has_emitted_message_delta = true;

                                            // Close current non-tool block
                                            if let Some(index) = current_non_tool_block_index.take() {
                                                let event = json!({
                                                    "type": "content_block_stop", "index": index
                                                });
                                                let sse_data = format!(
                                                    "event: content_block_stop\ndata: {}\n\n",
                                                    serde_json::to_string(&event).unwrap_or_default()
                                                );
                                                yield Ok(Bytes::from(sse_data));
                                            }
                                            current_non_tool_block_type = None;

                                            // Close all tool blocks
                                            for &index in &open_tool_block_indices {
                                                let event = json!({
                                                    "type": "content_block_stop", "index": index
                                                });
                                                let sse_data = format!(
                                                    "event: content_block_stop\ndata: {}\n\n",
                                                    serde_json::to_string(&event).unwrap_or_default()
                                                );
                                                yield Ok(Bytes::from(sse_data));
                                            }
                                            open_tool_block_indices.clear();

                                            pending_message_delta = Some((stop_reason, usage_json));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Stream error: {e}");
                    stream_ended_with_error = true;
                    let error_event = json!({
                        "type": "error",
                        "error": { "type": "stream_error", "message": format!("Stream error: {e}") }
                    });
                    let sse_data = format!(
                        "event: error\ndata: {}\n\n",
                        serde_json::to_string(&error_event).unwrap_or_default()
                    );
                    yield Ok(Bytes::from(sse_data));
                    break;
                }
            }
        }

        // Stream ended without [DONE] — emit pending events
        if !stream_ended_with_error {
            if let Some((stop_reason, usage_json)) = pending_message_delta.take() {
                let event = build_message_delta_event(stop_reason, usage_json);
                let sse_data = format!(
                    "event: message_delta\ndata: {}\n\n",
                    serde_json::to_string(&event).unwrap_or_default()
                );
                yield Ok(Bytes::from(sse_data));

                if !has_sent_message_stop {
                    let event = json!({"type": "message_stop"});
                    let sse_data = format!(
                        "event: message_stop\ndata: {}\n\n",
                        serde_json::to_string(&event).unwrap_or_default()
                    );
                    yield Ok(Bytes::from(sse_data));
                }
            }
        }
    }
}

fn build_anthropic_usage_json(usage: &StreamUsage) -> Value {
    let cached = extract_cache_read_tokens(usage).unwrap_or(0);
    let cache_creation = usage.cache_creation_input_tokens.unwrap_or(0);
    let input_tokens = usage
        .prompt_tokens
        .saturating_sub(cached)
        .saturating_sub(cache_creation);
    let mut usage_json = json!({
        "input_tokens": input_tokens,
        "output_tokens": usage.completion_tokens
    });
    if cached > 0 {
        usage_json["cache_read_input_tokens"] = json!(cached);
    }
    if cache_creation > 0 {
        usage_json["cache_creation_input_tokens"] = json!(cache_creation);
    }
    usage_json
}

fn extract_cache_read_tokens(usage: &StreamUsage) -> Option<u32> {
    if let Some(v) = usage.cache_read_input_tokens {
        return Some(v);
    }
    usage
        .prompt_tokens_details
        .as_ref()
        .map(|d| d.cached_tokens)
        .filter(|&v| v > 0)
}

fn map_stop_reason(finish_reason: Option<&str>) -> Option<String> {
    finish_reason.map(|r| {
        match r {
            "tool_calls" | "function_call" => "tool_use",
            "length" => "max_tokens",
            _ => "end_turn",
        }
        .to_string()
    })
}

fn build_message_delta_event(stop_reason: Option<String>, usage_json: Option<Value>) -> Value {
    let usage = usage_json.unwrap_or(json!({"input_tokens": 0, "output_tokens": 0}));
    json!({
        "type": "message_delta",
        "delta": { "stop_reason": stop_reason, "stop_sequence": null },
        "usage": usage
    })
}

// ── OpenAI Chat SSE → OpenAI Responses SSE ──

/// 将 `OpenAI Chat Completions` `SSE` 流转换为 `OpenAI Responses` `SSE` 流
pub fn openai_sse_to_openai_responses<S>(
    input_stream: S,
) -> impl Stream<Item = Result<Bytes, std::io::Error>>
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
{
    let mut state = ResponsesStreamState::default();

    input_stream.map(move |result| {
        let chunk = match result {
            Ok(b) => b,
            Err(e) => return Err(std::io::Error::other(e)),
        };

        let chunk_str = if let Ok(s) = std::str::from_utf8(&chunk) {
            s.to_string()
        } else {
            let sse = format!("data: {}\n\n", String::from_utf8_lossy(&chunk));
            return Ok(Bytes::from(sse));
        };

        let mut events = String::new();
        let lines: Vec<&str> = chunk_str.lines().collect();

        for line in &lines {
            if line.is_empty() {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    events.push_str("event: response.completed\ndata: {}\n\n");
                    continue;
                }

                let v: Value = if let Ok(v) = serde_json::from_str(data) {
                    v
                } else {
                    // Preserve non-JSON data lines
                    events.push_str(line);
                    events.push('\n');
                    continue;
                };

                let id = v
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("")
                    .to_string();
                let model = v
                    .get("model")
                    .and_then(|m| m.as_str())
                    .unwrap_or("")
                    .to_string();
                let response_id = format!("resp_{id}");

                let choices = v.get("choices").and_then(|c| c.as_array());

                if let Some(choices) = choices {
                    let first = match choices.first() {
                        Some(c) => c,
                        None => continue,
                    };

                    let delta = first.get("delta");
                    let finish_reason = first.get("finish_reason").and_then(|f| f.as_str());

                    let delta_role = delta.and_then(|d| d.get("role")).and_then(|r| r.as_str());

                    // Emit initial events on first content-bearing chunk
                    if !state.initialized && delta_role != Some("assistant") {
                        state.initialized = true;

                        events.push_str(&format!(
                            "event: response.created\ndata: {}\n\n",
                            serde_json::to_string(&json!({
                                "type": "response.created",
                                "response": {
                                    "id": response_id,
                                    "object": "response",
                                    "model": model,
                                    "output": [],
                                    "status": "in_progress"
                                }
                            }))
                            .unwrap_or_default()
                        ));

                        events.push_str(&format!(
                            "event: response.output_item.added\ndata: {}\n\n",
                            serde_json::to_string(&json!({
                                "type": "response.output_item.added",
                                "output_index": 0,
                                "item": {
                                    "id": format!("msg_{id}"),
                                    "type": "message",
                                    "role": "assistant",
                                    "content": []
                                }
                            }))
                            .unwrap_or_default()
                        ));

                        events.push_str(&format!(
                            "event: response.content_part.added\ndata: {}\n\n",
                            serde_json::to_string(&json!({
                                "type": "response.content_part.added",
                                "item_id": format!("msg_{id}"),
                                "output_index": 0,
                                "content_index": 0,
                                "part": {"type": "output_text", "text": "", "annotations": []}
                            }))
                            .unwrap_or_default()
                        ));
                    }

                    // Content delta
                    if state.initialized {
                        if let Some(text) = delta
                            .and_then(|d| d.get("content"))
                            .and_then(|c| c.as_str())
                        {
                            if !text.is_empty() {
                                events.push_str(&format!(
                                    "event: response.output_text.delta\ndata: {}\n\n",
                                    serde_json::to_string(&json!({
                                        "type": "response.output_text.delta",
                                        "item_id": format!("msg_{id}"),
                                        "output_index": 0,
                                        "content_index": 0,
                                        "delta": text
                                    }))
                                    .unwrap_or_default()
                                ));
                            }
                        }
                    }

                    // Tool calls
                    if let Some(tool_calls) = delta
                        .and_then(|d| d.get("tool_calls"))
                        .and_then(|t| t.as_array())
                    {
                        if !state.initialized {
                            state.initialized = true;
                            // Emit setup events for tool-call response
                            events.push_str(&format!(
                                "event: response.created\ndata: {}\n\n",
                                serde_json::to_string(&json!({
                                    "type": "response.created",
                                    "response": {
                                        "id": response_id,
                                        "object": "response",
                                        "model": model,
                                        "output": [],
                                        "status": "in_progress"
                                    }
                                }))
                                .unwrap_or_default()
                            ));
                        }

                        for tc in tool_calls {
                            let tc_id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
                            let tc_name = tc
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                                .unwrap_or("");
                            let tc_args = tc
                                .get("function")
                                .and_then(|f| f.get("arguments"))
                                .and_then(|a| a.as_str())
                                .unwrap_or("");

                            // Track if we've seen this tool call before
                            let key = tc_id.to_string();
                            if !state.emitted_tool_calls.contains(&key) {
                                state.emitted_tool_calls.insert(key);
                                let output_index = state.tool_output_index;
                                state.tool_output_index += 1;

                                let item_id = format!("fc_{tc_id}");
                                events.push_str(&format!(
                                    "event: response.output_item.added\ndata: {}\n\n",
                                    serde_json::to_string(&json!({
                                        "type": "response.output_item.added",
                                        "output_index": output_index,
                                        "item": {
                                            "id": item_id,
                                            "type": "function_call",
                                            "call_id": tc_id,
                                            "name": tc_name,
                                            "arguments": tc_args
                                        }
                                    }))
                                    .unwrap_or_default()
                                ));
                            }

                            // Append argument delta
                            let item_id = format!("fc_{tc_id}");
                            events.push_str(&format!(
                                "event: response.function_call_arguments.delta\ndata: {}\n\n",
                                serde_json::to_string(&json!({
                                    "type": "response.function_call_arguments.delta",
                                    "item_id": item_id,
                                    "output_index": state.tool_output_index - 1,
                                    "delta": tc_args
                                }))
                                .unwrap_or_default()
                            ));
                        }
                    }

                    // Finish reason → response completed
                    if let Some(fr_val) = finish_reason {
                        if !fr_val.is_empty() {
                            events.push_str(&format!(
                                "event: response.completed\ndata: {}\n\n",
                                serde_json::to_string(&json!({
                                    "type": "response.completed",
                                    "response": {
                                        "id": response_id,
                                        "object": "response",
                                        "model": model,
                                        "output": [],
                                        "status": "completed"
                                    }
                                }))
                                .unwrap_or_default()
                            ));
                        }
                    }
                }
            }
        }

        if events.is_empty() {
            // Preserve raw data lines that weren't matched
            let mut raw = String::new();
            for line in &lines {
                if !line.is_empty() {
                    raw.push_str(line);
                    raw.push('\n');
                }
            }
            Ok(Bytes::from(raw))
        } else {
            Ok(Bytes::from(events))
        }
    })
}

#[derive(Default)]
struct ResponsesStreamState {
    initialized: bool,
    tool_output_index: usize,
    emitted_tool_calls: std::collections::HashSet<String>,
}
