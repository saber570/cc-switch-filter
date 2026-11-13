//! V9 compatibility mode. Ported from AgentRouter-Codex-Filter (MIT);
//! see THIRD_PARTY_NOTICES.md. No background process or config-file rewriting.

use super::providers::codex_compaction;
use super::sse::{strip_sse_field, SseBuffer};
use super::ProxyError;
use axum::{
    body::Body,
    response::{IntoResponse, Response},
};
use bytes::{Bytes, BytesMut};
use futures::{Stream, StreamExt};
use http::{HeaderMap, StatusCode};
use serde_json::{json, Value};
use std::{
    future::Future,
    pin::Pin,
    time::{Duration, SystemTime},
};
use tokio::time::Instant;

pub(crate) const LIMIT: usize = 64 * 1024 * 1024;
const ERROR_LIMIT: usize = 64 * 1024;
const MAX_RETRIES: usize = 20;
const DEADLINE: Duration = Duration::from_secs(1800);
const IDLE: Duration = Duration::from_secs(120);

pub(crate) fn enabled(
    app: &crate::app_config::AppType,
    provider: &crate::provider::Provider,
) -> bool {
    matches!(app, crate::app_config::AppType::Codex)
        && !super::providers::is_codex_official_provider(provider)
}

fn rejection(code: &str, message: &str) -> ProxyError {
    ProxyError::UpstreamError {
        status: 400,
        body: Some(
            json!({"error": {"type": "invalid_request_error", "code": code, "message": message}})
                .to_string(),
        ),
    }
}

pub(crate) fn reject_compaction() -> ProxyError {
    ProxyError::UpstreamError { status: 409, body: Some(json!({"error": {
        "type": "invalid_request_error", "code": "encrypted_compaction_disabled",
        "message": "V9 filtering supports text summaries and CC Switch summary compaction, but not the encrypted compact endpoint or server-managed compaction."
    }}).to_string()) }
}

pub(crate) fn filter_request(body: &mut Value) -> Result<(usize, usize), ProxyError> {
    let Some(obj) = body.as_object_mut() else {
        return Err(rejection(
            "invalid_request_json",
            "Responses request must be a JSON object.",
        ));
    };
    if ["previous_response_id", "conversation"].iter().any(|key| {
        obj.get(*key)
            .is_some_and(|v| !v.is_null() && v != "" && v != false)
    }) {
        return Err(rejection("response_reference_not_portable", "V9 filter requires complete message/tool history, not server-side references. Save a text summary and start a new session."));
    }
    if obj.get("include").is_some_and(|v| !v.is_array()) {
        return Err(rejection("invalid_include", "include must be an array."));
    }
    if obj
        .get("input")
        .is_some_and(|v| !v.is_string() && !v.is_array())
    {
        return Err(rejection(
            "invalid_input",
            "input must be a string or an array.",
        ));
    }
    if obj
        .get("context_management")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|v| v["type"] == "compaction"))
    {
        return Err(reject_compaction());
    }
    // Unlike the standalone V9 proxy, CC Switch can decode its own summary
    // envelopes. Validate ALL items before replacing any history, and retain
    // compaction_trigger so the existing protocol adapters handle the turn.
    let mut summaries = Vec::new();
    if let Some(items) = obj.get("input").and_then(Value::as_array) {
        for (index, item) in items.iter().enumerate() {
            if item["type"] == "reasoning" {
                continue;
            }
            if codex_compaction::is_compaction_item(item) {
                let summary = item["encrypted_content"]
                    .as_str()
                    .and_then(codex_compaction::decode_compaction_summary)
                    .filter(|text| !text.trim().is_empty())
                    .ok_or_else(|| rejection("encrypted_context_not_portable", "V9 filter cannot read this compaction item. Only intact CC Switch text summaries can cross providers. History has not been discarded."))?;
                summaries.push((index, summary));
            } else if item["type"] == "item_reference"
                || item
                    .get("encrypted_content")
                    .is_some_and(|v| !v.is_null() && v != "")
            {
                return Err(rejection("encrypted_context_not_portable", "V9 filter cannot transfer opaque encrypted context or context references. History has not been discarded. Save a text summary and start a new session."));
            }
        }
    }
    obj.insert("store".into(), Value::Bool(false));
    if let Some(include) = obj.get_mut("include").and_then(Value::as_array_mut) {
        include.retain(|v| v != "reasoning.encrypted_content");
    }
    let (mut reasoning, mut ids) = (0, 0);
    if let Some(items) = obj.get_mut("input").and_then(Value::as_array_mut) {
        for (index, summary) in summaries {
            items[index] = codex_compaction::user_message_item(&format!(
                "{}\n{summary}",
                codex_compaction::SUMMARY_PREFIX
            ));
        }
        items.retain(|item| {
            let remove = item["type"] == "reasoning";
            reasoning += usize::from(remove);
            !remove
        });
        for item in items {
            // Native web-search records require the reasoning item that issued
            // them. Reasoning is deliberately not replayed across resources,
            // so replay the readable search history as assistant text instead.
            // Keep the successful response unchanged for the client's UI.
            if item["type"] == "web_search_call" {
                let mut record = item.clone();
                if let Some(record) = record.as_object_mut() {
                    ids += usize::from(record.remove("id").is_some());
                    record.remove("type");
                }
                *item = json!({
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": format!("[Previous web search]\n{record}")
                    }]
                });
            }
            let portable = matches!(
                item["type"].as_str(),
                Some(
                    "message"
                        | "function_call"
                        | "function_call_output"
                        | "custom_tool_call"
                        | "custom_tool_call_output"
                )
            ) || (item.get("type").is_none()
                && item["role"].is_string()
                && item.get("content").is_some());
            if portable
                && item
                    .as_object_mut()
                    .is_some_and(|o| o.remove("id").is_some())
            {
                ids += 1;
            }
        }
    }
    Ok((reasoning, ids))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Resource,
    Rate,
    Transport,
    Availability,
}
impl Kind {
    fn index(self) -> usize {
        self as usize
    }
    fn limit(self) -> usize {
        match self {
            Self::Resource => 5,
            Self::Rate => 20,
            _ => 10,
        }
    }
    fn delay(self, count: usize) -> Duration {
        let steps = match self {
            Self::Rate => [10, 20, 40, 60, 60],
            Self::Availability => [5, 10, 20, 30, 60],
            _ => [1, 2, 4, 8, 15],
        };
        Duration::from_secs(steps[count.min(4)])
    }
}

fn reported_error(value: &Value) -> Option<&Value> {
    value
        .get("error")
        .filter(|v| !v.is_null())
        .or_else(|| value.pointer("/response/error").filter(|v| !v.is_null()))
}

fn non_retryable_business(value: &Value) -> bool {
    let error = reported_error(value).unwrap_or(value);
    let codes = ["code", "type"].map(|key| error[key].as_str().unwrap_or("").to_ascii_lowercase());
    let lower = format!(
        "{} {} {}",
        codes[0],
        codes[1],
        error["message"].as_str().unwrap_or("")
    )
    .to_lowercase();
    ["quota", "budget", "billing", "balance", "credit", "spend"]
        .iter()
        .any(|s| lower.contains(s))
        || codes.iter().any(|code| {
            matches!(
                code.as_str(),
                "invalid_api_key"
                    | "authentication_error"
                    | "permission_denied"
                    | "unauthorized"
                    | "forbidden"
            )
        })
}

fn high_demand(message: &str) -> bool {
    message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
        .contains("currently experiencing high demand")
}

fn classify(value: &Value) -> Option<Kind> {
    classify_event(value, value["type"].as_str().unwrap_or(""))
}

fn classify_event(value: &Value, event: &str) -> Option<Kind> {
    // Check both original business fields before treating `type` as an SSE event.
    if non_retryable_business(value) {
        return None;
    }
    let error = reported_error(value).unwrap_or(value);
    let code = match &error["code"] {
        Value::String(code) if !code.is_empty() => code.to_ascii_lowercase(),
        Value::Number(code) => code.to_string(),
        _ => error["type"].as_str().unwrap_or("").to_ascii_lowercase(),
    };
    let message = error["message"].as_str().unwrap_or("");
    if event == "response.failed"
        && (code.is_empty() || code == "response.failed")
        && message.is_empty()
    {
        return Some(Kind::Transport);
    }
    let lower = message.to_lowercase();
    if message.contains("The requested item was created under a different Azure OpenAI resource.")
        && message.contains("Use the same resource that created the item to access it.")
    {
        return Some(Kind::Resource);
    }
    // Gateway concurrency throttling shares the rate-limit backoff and budget.
    if matches!(
        code.as_str(),
        "429"
            | "rate_limit"
            | "rate_limit_exceeded"
            | "rate_limit_error"
            | "too_many_requests"
            | "gateway_concurrency_limit"
    ) || lower.contains("exceeded token rate limit")
        || (lower.contains("rate limit exceeded")
            && ["your requests", "token", "requests per minute"]
                .iter()
                .any(|s| lower.contains(s)))
    {
        return Some(Kind::Rate);
    }
    if matches!(
        code.as_str(),
        "stream_incomplete"
            | "stream_disconnected"
            | "upstream_idle_timeout"
            | "upstream_aborted"
            | "cc_switch_timeout"
            | "cc_switch_stream_idle_timeout"
            | "cc_switch_forward_failed"
    ) || lower.contains("stream closed before response.completed")
        || lower.contains("idle timeout waiting for sse")
        || lower.contains("stream disconnected before completion")
        || high_demand(message)
    {
        return Some(Kind::Transport);
    }
    if code == "no_available_channel"
        || lower.contains("无可用渠道")
        || lower.contains("没有可用渠道")
        || lower.contains("no available channel")
    {
        return Some(Kind::Availability);
    }
    if matches!(code.as_str(), "bad_response" | "server_error") {
        return Some(Kind::Transport);
    }
    None
}

fn alb_page(text: &str) -> bool {
    use std::sync::LazyLock;
    static PAGE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?is)<html\b[^>]*>.*<title>\s*400\s+Bad\s+Request\s*</title>.*<h1>\s*400\s+Bad\s+Request\s*</h1>.*<center>\s*alb\s*</center>.*</html>").unwrap()
    });
    PAGE.is_match(text)
}

fn http_kind(status: StatusCode, data: &[u8]) -> Option<Kind> {
    if data.len() > ERROR_LIMIT {
        return None;
    }
    let text = String::from_utf8_lossy(data);
    let value = serde_json::from_slice::<Value>(data).unwrap_or(Value::Null);
    if matches!(status.as_u16(), 401 | 403) || non_retryable_business(&value) {
        return None;
    }
    let message = if value.is_null() {
        text.as_ref()
    } else if value["error"]["code"] == "cc_switch_upstream_error" {
        value["error"]["message"].as_str().unwrap_or("")
    } else {
        ""
    };
    if status == StatusCode::BAD_REQUEST && alb_page(message) {
        return Some(Kind::Transport);
    }
    match (classify(&value), status.as_u16()) {
        (Some(k @ Kind::Resource), 400)
        | (Some(k @ Kind::Rate), 400 | 429)
        | (Some(k @ Kind::Availability), 503)
        | (Some(k @ Kind::Transport), 400..=599) => Some(k),
        _ if status.is_server_error() || (value.is_null() && high_demand(&text)) => {
            Some(Kind::Transport)
        }
        _ => None,
    }
}

// Parse all legal SSE line endings without changing the bytes delivered to the
// client or the shared parser used by unrelated protocol adapters.
#[derive(Default)]
struct FilterSseBuffer {
    frames: SseBuffer,
    previous_cr: bool,
}

impl FilterSseBuffer {
    fn push_bytes(&mut self, chunk: &[u8]) {
        if !self.previous_cr && !chunk.contains(&b'\r') {
            self.frames.push_bytes(chunk);
            return;
        }
        let mut normalized = Vec::with_capacity(chunk.len());
        for &byte in chunk {
            if self.previous_cr {
                normalized.push(b'\n');
                self.previous_cr = false;
                if byte == b'\n' {
                    continue;
                }
            }
            if byte == b'\r' {
                self.previous_cr = true;
            } else {
                normalized.push(byte);
            }
        }
        self.frames.push_bytes(&normalized);
    }

    fn finish(&mut self) {
        if self.previous_cr {
            self.frames.push_bytes(b"\n");
            self.previous_cr = false;
        }
    }

    fn next_block(&mut self) -> Option<&str> {
        self.frames.next_block()
    }
}

fn retry_after(headers: &HeaderMap, now: SystemTime) -> Duration {
    let mut delay = Duration::ZERO;
    for name in ["retry-after-ms", "x-ms-retry-after-ms"] {
        if let Some(ms) = headers
            .get(name)
            .and_then(|h| h.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
        {
            delay = delay.max(Duration::from_millis(ms));
        }
    }
    if let Some(value) = headers.get("retry-after").and_then(|h| h.to_str().ok()) {
        if let Ok(seconds) = value.parse::<u64>() {
            delay = delay.max(Duration::from_secs(seconds));
        } else if let Ok(date) = chrono::DateTime::parse_from_rfc2822(value) {
            let then: SystemTime = date.into();
            delay = delay.max(then.duration_since(now).unwrap_or_default());
        }
    }
    delay
}

struct Attempt {
    response: Response,
    retry: Option<Kind>,
}

fn error_response(code: &str, message: &str) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        axum::Json(json!({"error": {"type": "upstream_error", "code": code, "message": message}})),
    )
        .into_response()
}

fn transport_failure() -> Attempt {
    Attempt {
        response: error_response(
            "stream_incomplete",
            "Upstream stream disconnected or timed out before response.completed.",
        ),
        retry: Some(Kind::Transport),
    }
}

fn heartbeat(id: &str) -> Bytes {
    // Match V9: SSE comments alone do not reset every Codex client's event
    // timeout. No text, reasoning or tool output is included in this keepalive.
    super::providers::codex_responses_sse::response_in_progress(&json!({
        "id": id, "object": "response", "status": "in_progress", "output": []
    }))
}

/// Never exposes a failed attempt's text or tool calls to the downstream client.
async fn collect(response: Response, limit: usize, idle: Duration) -> Attempt {
    let (mut parts, body) = response.into_parts();
    let sse = parts
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.to_ascii_lowercase().starts_with("text/event-stream"));
    let encoding = super::content_encoding::get_content_encoding(&parts.headers);
    let mut stream: Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>> = Box::pin(
        body.into_data_stream()
            .map(|r| r.map_err(std::io::Error::other)),
    );
    if let Some(encoding) = encoding {
        use async_compression::tokio::bufread::{
            BrotliDecoder, GzipDecoder, ZlibDecoder, ZstdDecoder,
        };
        use tokio::io::{AsyncRead, BufReader};
        use tokio_util::io::{ReaderStream, StreamReader};
        let reader = BufReader::new(StreamReader::new(stream));
        let decoded: Pin<Box<dyn AsyncRead + Send>> = match encoding.as_str() {
            "identity" => Box::pin(reader),
            "gzip" | "x-gzip" => Box::pin(GzipDecoder::new(reader)),
            "deflate" => Box::pin(ZlibDecoder::new(reader)),
            "br" => Box::pin(BrotliDecoder::new(reader)),
            "zstd" | "zst" => Box::pin(ZstdDecoder::new(reader)),
            _ => {
                return Attempt {
                    response: error_response(
                        "unsupported_content_encoding",
                        "Unsupported compressed response encoding.",
                    ),
                    retry: None,
                }
            }
        };
        stream = Box::pin(ReaderStream::new(decoded));
    }
    for header in [
        "content-encoding",
        "content-length",
        "transfer-encoding",
        "connection",
    ] {
        parts.headers.remove(header);
    }
    let mut bytes = BytesMut::new();
    let mut frames = FilterSseBuffer::default();
    loop {
        let (chunk, ended) = match tokio::time::timeout(idle, stream.next()).await {
            Ok(Some(Ok(chunk))) => (chunk, false),
            Ok(None) if !sse => break,
            Ok(None) => (Bytes::new(), true),
            _ => return transport_failure(),
        };
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Attempt {
                response: error_response(
                    "response_too_large",
                    "V9 buffered response exceeds 64 MiB.",
                ),
                retry: None,
            };
        }
        bytes.extend_from_slice(&chunk);
        if !sse {
            continue;
        }
        frames.push_bytes(&chunk);
        if ended {
            frames.finish();
        }
        while let Some(frame) = frames.next_block() {
            let data = frame
                .lines()
                .filter_map(|l| strip_sse_field(l, "data"))
                .collect::<Vec<_>>()
                .join("\n");
            let Ok(mut value) = serde_json::from_str::<Value>(&data) else {
                continue;
            };
            let event = frame
                .lines()
                .find_map(|l| strip_sse_field(l, "event"))
                .map(str::trim)
                .or_else(|| value["type"].as_str())
                .unwrap_or("")
                .to_owned();
            match event.as_str() {
                "error" | "response.failed" => {
                    let retry = if matches!(parts.status.as_u16(), 401 | 403) {
                        None
                    } else {
                        classify_event(&value, &event)
                    };
                    if let Some(obj) = value.as_object_mut() {
                        // Preserve a top-level business type when the protocol
                        // requires the outgoing `type` to name the SSE event.
                        if !obj.contains_key("code") {
                            if let Some(kind) = obj.get("type").and_then(Value::as_str) {
                                if kind != "error" && kind != "response.failed" {
                                    obj.insert("code".into(), json!(kind));
                                }
                            }
                        }
                        obj.insert("type".into(), event.into());
                    }
                    // Even when retries are exhausted, do not release partial tool
                    // calls from a failed attempt. Only forward its failure event.
                    let terminal = format!(
                        "event: {}\ndata: {}\n\n",
                        value["type"].as_str().unwrap_or("error"),
                        value
                    );
                    return Attempt {
                        response: Response::from_parts(parts, Body::from(terminal)),
                        retry,
                    };
                }
                "response.completed" | "response.incomplete" => {
                    return Attempt {
                        response: Response::from_parts(parts, Body::from(bytes.freeze())),
                        retry: None,
                    }
                }
                _ => {}
            }
        }
        if ended {
            return transport_failure();
        }
    }
    if parts.status.is_success() {
        let value = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
        let textual_error = value.is_null() && high_demand(&String::from_utf8_lossy(&bytes));
        let failed = reported_error(&value).is_some()
            || value["status"] == "failed"
            || value["response"]["status"] == "failed"
            || value["type"] == "error"
            || value["type"] == "response.failed"
            || textual_error;
        if failed {
            let retry = if textual_error {
                Some(Kind::Transport)
            } else {
                classify_event(
                    &value,
                    if value["status"] == "failed" || value["response"]["status"] == "failed" {
                        "response.failed"
                    } else {
                        value["type"].as_str().unwrap_or("")
                    },
                )
            };
            let detail = reported_error(&value).unwrap_or(&value);
            // An HTTP 200 error envelope can contain partial output. Deliver
            // only its error fields, including when retry limits are exhausted.
            let mut error = serde_json::Map::new();
            for key in ["code", "type", "message", "param"] {
                if let Some(field) = detail.get(key) {
                    error.insert(key.into(), field.clone());
                }
            }
            if error.is_empty() {
                error.insert("code".into(), json!("upstream_failed_without_details"));
                error.insert(
                    "message".into(),
                    json!("Upstream returned a failed response without error details."),
                );
            }
            parts.status = StatusCode::BAD_GATEWAY;
            parts
                .headers
                .insert("content-type", "application/json".parse().unwrap());
            return Attempt {
                response: Response::from_parts(
                    parts,
                    Body::from(json!({"error":error}).to_string()),
                ),
                retry,
            };
        }
    }
    let retry = if parts.status.is_client_error() || parts.status.is_server_error() {
        http_kind(parts.status, &bytes)
    } else {
        None
    };
    Attempt {
        response: Response::from_parts(parts, Body::from(bytes.freeze())),
        retry,
    }
}

async fn run<F, Fut>(
    mut send: F,
    deadline: Duration,
    idle: Duration,
    limit: usize,
    delays: bool,
) -> Response
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Response, ProxyError>>,
{
    let deadline = Instant::now() + deadline;
    let request_id = uuid::Uuid::new_v4();
    let mut counts = [0usize; 4];
    let mut retries = 0;
    loop {
        let attempt = tokio::time::timeout_at(deadline, async {
            match tokio::time::timeout(idle, send()).await {
                Ok(Ok(response)) => collect(response, limit, idle).await,
                Err(_)
                | Ok(Err(
                    ProxyError::Timeout(_)
                    | ProxyError::StreamIdleTimeout(_)
                    | ProxyError::ForwardFailed(_),
                )) => transport_failure(),
                Ok(Err(error)) => collect(error.into_response(), limit, idle).await,
            }
        })
        .await;
        let Ok(attempt) = attempt else {
            log::warn!("[CodexV9] request={request_id} deadline exhausted retries={retries}");
            return error_response(
                "request_deadline",
                "V9 request exceeded its 30-minute deadline.",
            );
        };
        let Some(kind) = attempt.retry else {
            log::info!(
                "[CodexV9] request={request_id} finished retries={retries} http_status={}",
                attempt.response.status()
            );
            return attempt.response;
        };
        let index = kind.index();
        if retries >= MAX_RETRIES || counts[index] >= kind.limit() {
            log::warn!(
                "[CodexV9] request={request_id} retries exhausted kind={kind:?} retries={retries}"
            );
            return attempt.response;
        }
        let jitter = if kind == Kind::Rate {
            Duration::from_millis(u64::from(uuid::Uuid::new_v4().as_bytes()[0]) * 3)
        } else {
            Duration::ZERO
        };
        let delay = if delays {
            kind.delay(counts[index])
                .max(retry_after(attempt.response.headers(), SystemTime::now()))
                + jitter
        } else {
            Duration::ZERO
        };
        if Instant::now()
            .checked_add(delay)
            .is_none_or(|next| next >= deadline)
        {
            return attempt.response;
        }
        retries += 1;
        counts[index] += 1;
        log::info!(
            "[CodexV9] request={request_id} retry={retries}/20 kind={kind:?} delay_ms={}",
            delay.as_millis()
        );
        drop(attempt);
        tokio::time::sleep(delay).await;
    }
}

/// Poll the retry future inside the response body, so dropping/canceling the
/// client body cancels upstream work and backoff. No detached task survives it.
pub(crate) async fn handle<F, Fut>(streaming: bool, send: F) -> Result<Response, ProxyError>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = Result<Response, ProxyError>> + Send + 'static,
{
    if !streaming {
        return Ok(run(send, DEADLINE, IDLE, LIMIT, true).await);
    }
    // Preserve the actual status, headers and JSON for attempts that complete
    // before the first heartbeat. Only commit SSE when we need to keep waiting.
    let mut work = Box::pin(run(send, DEADLINE, IDLE, LIMIT, true));
    if let Ok(response) = tokio::time::timeout(Duration::from_secs(15), &mut work).await {
        return Ok(response);
    }
    let heartbeat_id = format!("resp_filter_{}", uuid::Uuid::new_v4().simple());
    let stream = async_stream::stream! {
        yield Ok::<Bytes, std::io::Error>(heartbeat(&heartbeat_id));
        let mut heartbeat_timer = tokio::time::interval_at(Instant::now() + Duration::from_secs(15), Duration::from_secs(15));
        let response = loop {
            tokio::select! {
                biased;
                response = &mut work => break response,
                _ = heartbeat_timer.tick() => yield Ok::<Bytes, std::io::Error>(heartbeat(&heartbeat_id)),
            }
        };
        let status = response.status();
        let is_sse = response.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("text/event-stream"));
        if is_sse {
            let mut body = response.into_body().into_data_stream();
            while let Some(chunk) = body.next().await { yield chunk.map_err(std::io::Error::other); }
        } else {
            let raw = axum::body::to_bytes(response.into_body(), LIMIT).await.unwrap_or_default();
            let value = serde_json::from_slice::<Value>(&raw).unwrap_or(Value::Null);
            if status.is_success() && value["status"] == "completed" {
                // A gateway may return a full JSON Response despite stream=true.
                // Once SSE headers were committed, replay its items as SSE so
                // Codex receives tool calls as well as the completion envelope.
                use super::providers::codex_responses_sse as events;
                let mut initial = value.clone();
                initial["output"] = json!([]);
                initial["status"] = json!("in_progress");
                yield Ok(events::response_created(&initial));
                if let Some(items) = value["output"].as_array() {
                    for (index, item) in items.iter().enumerate() {
                        yield Ok(events::output_item_added(index as u32, item));
                        yield Ok(events::output_item_done(index as u32, item));
                    }
                }
                yield Ok(events::response_completed(&value));
            } else {
                let detail = value.get("error").cloned().unwrap_or_else(|| json!({"code": "upstream_error", "message": format!("Upstream returned HTTP {status}")}));
                let mut error = detail.as_object().cloned().unwrap_or_default();
                error.insert("type".into(), json!("error"));
                yield Ok(Bytes::from(format!("event: error\ndata: {}\n\n", Value::Object(error))));
            }
        }
    };
    Ok(Response::builder()
        .status(200)
        .header("content-type", "text/event-stream; charset=utf-8")
        .header("cache-control", "no-cache")
        .body(Body::from_stream(stream))
        .unwrap())
}

#[cfg(test)]
#[path = "codex_v9_filter_tests.rs"]
mod tests;
