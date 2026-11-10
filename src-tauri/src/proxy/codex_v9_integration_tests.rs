use super::*;
use crate::{
    database::Database,
    mode::stack::StackTarget,
    provider::{Provider, ProviderMeta},
};
use std::sync::Arc;
use tokio::sync::Mutex;

fn compaction_test_provider(addr: std::net::SocketAddr, format: &str) -> Provider {
    let mut provider = Provider::with_id(
        format!("v9-{format}"),
        format.into(),
        json!({
            "auth":{"OPENAI_API_KEY":"test-only"},
            "config":format!("model_provider = \"custom\"\nmodel = \"test-model\"\n[model_providers.custom]\nbase_url = \"http://{addr}/v1\"\nwire_api = \"responses\"\n")
        }),
        None,
    );
    provider.meta = Some(ProviderMeta {
        api_format: Some(format.into()),
        codex_v9_filter: Some(true),
        ..Default::default()
    });
    provider
}

async fn send_compaction_test_request(
    state: &ProxyState,
    provider: Provider,
    body: Value,
) -> Vec<Value> {
    let ctx = RequestContext::new(
        state,
        &body,
        &http::HeaderMap::new(),
        AppType::Codex,
        "Codex",
        "codex",
        Some(StackTarget {
            provider,
            upstream_model: "test-model".into(),
            original_model: "test-model".into(),
        }),
    )
    .await
    .unwrap();
    let response = dispatch_responses_request(
        state.clone(),
        ctx,
        AppType::Codex,
        http::Method::POST,
        "/responses".into(),
        body,
        http::HeaderMap::new(),
        http::Extensions::new(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|data| serde_json::from_str(data).ok())
        .collect()
}

#[tokio::test]
async fn codex_v9_compaction_roundtrips_and_replays_across_provider_protocols() {
    use crate::proxy::providers::codex_compaction::{decode_compaction_summary, COMPACT_PROMPT};
    const SUMMARY: &str = "已完成：修复路由；下一步：保留工具结果并运行测试。";
    let _ = rustls::crypto::ring::default_provider().install_default();
    let seen = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let records = seen.clone();
    let app = axum::Router::new().fallback(axum::routing::post(move |uri: http::Uri, Json(body): Json<Value>| {
        let records = records.clone();
        async move {
            records.lock().await.push((uri.path().to_owned(), body));
            let events = if uri.path().ends_with("/chat/completions") {
                vec![json!({"id":"chat-summary","model":"test-model","choices":[{"index":0,"delta":{"role":"assistant","content":SUMMARY},"finish_reason":null}]}),
                    json!({"id":"chat-summary","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]})]
            } else if uri.path().ends_with("/messages") {
                vec![json!({"type":"message_start","message":{"id":"msg-summary","type":"message","role":"assistant","model":"test-model","content":[],"usage":{"input_tokens":10,"output_tokens":0}}}),
                    json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
                    json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":SUMMARY}}),
                    json!({"type":"content_block_stop","index":0}),
                    json!({"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":10}}),
                    json!({"type":"message_stop"})]
            } else {
                assert!(uri.path().ends_with("/responses"), "unexpected upstream path: {uri}");
                vec![json!({"type":"response.completed","response":{"id":"resp-summary","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":SUMMARY}]}]}})]
            };
            let mut sse = events.iter().map(|event| {
                let name = event["type"].as_str().map(|name| format!("event: {name}\n")).unwrap_or_default();
                format!("{name}data: {event}\n\n")
            }).collect::<String>();
            if uri.path().ends_with("/chat/completions") { sse.push_str("data: [DONE]\n\n"); }
            ([("content-type", "text/event-stream")], sse)
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let state = ProxyState::for_test(Arc::new(Database::memory().unwrap()));
    for source in ["openai_responses", "openai_chat", "anthropic"] {
        let events = send_compaction_test_request(
            &state,
            compaction_test_provider(addr, source),
            json!({
                "model":"test-model","stream":true,"input":[
                    {"type":"reasoning","encrypted_content":"discard old reasoning"},
                    {"role":"user","content":"Please retain the work so far"},
                    {"type":"compaction_trigger"}
                ]
            }),
        )
        .await;
        let completed = events
            .iter()
            .find(|event| event["type"] == "response.completed")
            .unwrap_or_else(|| panic!("{source} compaction did not complete: {events:?}"));
        let items = completed["response"]["output"].as_array().unwrap();
        let compact: Vec<_> = items
            .iter()
            .filter(|item| item["type"] == "compaction")
            .collect();
        assert_eq!(compact.len(), 1, "{source}");
        assert_eq!(
            decode_compaction_summary(compact[0]["encrypted_content"].as_str().unwrap()).as_deref(),
            Some(SUMMARY)
        );
        let sent = seen.lock().await.last().unwrap().1.to_string();
        assert!(
            sent.contains(COMPACT_PROMPT.lines().next().unwrap()),
            "{source}: {sent}"
        );
        assert!(!sent.contains("compaction_trigger"));
        for destination in ["openai_responses", "openai_chat", "anthropic"] {
            let replay = json!({"model":"test-model","stream":true,"input":[
                compact[0], {"role":"user","content":"Continue from the summary"}
            ]});
            let events = send_compaction_test_request(
                &state,
                compaction_test_provider(addr, destination),
                replay,
            )
            .await;
            assert!(
                events
                    .iter()
                    .any(|event| event["type"] == "response.completed"),
                "{source} -> {destination}: {events:?}"
            );
            let sent = seen.lock().await.last().unwrap().1.to_string();
            assert!(sent.contains(SUMMARY), "{source} -> {destination}: {sent}");
            assert!(sent.contains("Continue from the summary"));
            assert!(!sent.contains("ccswitch-compaction-v1:"));
            assert!(!sent.contains("encrypted_content"));
        }
    }
    assert_eq!(seen.lock().await.len(), 12);
    server.abort();
}

#[tokio::test]
async fn codex_v9_setting_filters_and_retries_through_real_http_forwarder() {
    for (enabled, error_code) in [
        (false, "stream_incomplete"),
        (true, "stream_incomplete"),
        (true, "server_error"),
        (true, "bad_response"),
    ] {
        let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
        let records = seen.clone();
        let app = axum::Router::new().route("/v1/responses", axum::routing::post(move |Json(body): Json<Value>| {
            let records = records.clone();
            async move {
                let mut calls = records.lock().await;
                calls.push(body);
                let events = if calls.len() == 1 {
                    vec![json!({"type":"response.output_item.added","item":{"type":"function_call","name":"never_run"}}),
                        json!({"type":"response.failed","response":{"error":{"code":error_code}}})]
                } else { vec![json!({"type":"response.completed","response":{"id":"resp_ok","status":"completed","output":[]}})] };
                let text = events.into_iter().map(|event| format!("event: {}\ndata: {event}\n\n",event["type"].as_str().unwrap())).collect::<String>();
                ([("content-type","text/event-stream")],text)
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let state = ProxyState::for_test(Arc::new(Database::memory().unwrap()));
        let mut provider = Provider::with_id(
            "v9-test".into(),
            "V9 Test".into(),
            json!({
                "auth":{"OPENAI_API_KEY":"test-only"},
                "config":format!("model_provider = \"custom\"\nmodel = \"test-model\"\n[model_providers.custom]\nbase_url = \"http://{addr}/v1\"\nwire_api = \"responses\"\n")
            }),
            None,
        );
        provider.meta = Some(ProviderMeta {
            codex_v9_filter: Some(enabled),
            ..Default::default()
        });
        let body = json!({"model":"test-model","stream":true,"reasoning":{"effort":"high"},"input":[
            {"type":"reasoning","id":"rs_old","encrypted_content":"opaque"},
            {"role":"user","id":"msg_old","content":"hello"}
        ]});
        let ctx = RequestContext::new(
            &state,
            &body,
            &http::HeaderMap::new(),
            AppType::Codex,
            "Codex",
            "codex",
            Some(StackTarget {
                provider,
                upstream_model: "test-model".into(),
                original_model: "test-model".into(),
            }),
        )
        .await
        .unwrap();
        let response = dispatch_responses_request(
            state,
            ctx,
            AppType::Codex,
            http::Method::POST,
            "/v1/responses".into(),
            body,
            http::HeaderMap::new(),
            http::Extensions::new(),
        )
        .await
        .unwrap();
        let text = String::from_utf8(
            axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        let calls = seen.lock().await;
        assert_eq!(calls.len(), if enabled { 2 } else { 1 });
        if enabled {
            assert!(!text.contains("never_run"));
            assert!(text.contains("resp_ok"));
            for call in calls.iter() {
                assert_eq!(call["model"], "test-model");
                assert_eq!(call["input"].as_array().unwrap().len(), 1);
                assert!(call["input"][0].get("id").is_none());
                assert_eq!(call["reasoning"]["effort"], "high");
            }
        } else {
            assert!(text.contains("never_run"));
            assert_eq!(calls[0]["input"].as_array().unwrap().len(), 2);
        }
        server.abort();
    }
}
