use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn sse(events: &[Value]) -> Response {
    let text = events
        .iter()
        .map(|v| format!("event: {}\ndata: {v}\n\n", v["type"].as_str().unwrap()))
        .collect::<String>();
    Response::builder()
        .header("content-type", "text/event-stream")
        .body(Body::from(text))
        .unwrap()
}

async fn text(response: Response) -> String {
    String::from_utf8(
        axum::body::to_bytes(response.into_body(), LIMIT)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

#[test]
fn filters_history_without_damaging_tools_or_current_effort() {
    let mut body = json!({"reasoning":{"effort":"high"}, "store":true,
        "include":["reasoning.encrypted_content","other"], "input":[
        {"type":"reasoning","id":"rs_old","encrypted_content":"opaque"},
        {"role":"user","id":"msg_old","content":[{"type":"input_image","file_id":"file_keep"}]},
        {"type":"function_call","id":"fc_old","call_id":"pair","name":"read","arguments":"{}"},
        {"type":"function_call_output","id":"out_old","call_id":"pair","output":"content"},
        {"type":"web_search_call","id":"resource_keep"}
    ]});
    assert_eq!(filter_request(&mut body).unwrap(), (1, 3));
    assert_eq!(body["reasoning"]["effort"], "high");
    assert_eq!(body["store"], false);
    assert_eq!(body["include"], json!(["other"]));
    assert_eq!(body["input"][0]["content"][0]["file_id"], "file_keep");
    assert_eq!(body["input"][1]["call_id"], body["input"][2]["call_id"]);
    assert_eq!(body["input"][2]["output"], "content");
    assert_eq!(body["input"][3]["id"], "resource_keep");
    assert_eq!(filter_request(&mut body).unwrap(), (0, 0));
}

#[test]
fn rejects_context_references_before_mutating_history() {
    for extra in [
        json!({"previous_response_id":"resp_old"}),
        json!({"conversation":"conv_old"}),
        json!({"input":[{"type":"reasoning"},{"type":"compaction","encrypted_content":"opaque"}]}),
        json!({"input":[{"type":"item_reference","id":"x"}]}),
        json!({"context_management":[{"type":"compaction"}]}),
        json!({"input":[{"type":"compaction_trigger","encrypted_content":"opaque"}]}),
    ] {
        let original = extra.clone();
        let mut body = extra;
        assert!(filter_request(&mut body).is_err());
        assert_eq!(body, original);
    }
}

#[test]
fn preserves_plain_summary_and_compaction_trigger() {
    let input = json!([
        codex_compaction::user_message_item("已完成迁移；下一步验证工具结果。"),
        {"type":"compaction_trigger"}
    ]);
    let mut body = json!({"input": input, "tools":[{"type":"function","name":"read"}]});
    assert_eq!(filter_request(&mut body).unwrap(), (0, 0));
    assert_eq!(body["input"], input);
    assert!(codex_compaction::is_compaction_request(&body));
    assert_eq!(body["tools"][0]["name"], "read");
}

#[test]
fn restores_own_compaction_summaries_and_aliases_without_losing_history() {
    let summary = "进度：已修改文件\n约束：保留数据\n下一步：运行测试";
    for kind in ["compaction", "compaction_summary", "context_compaction"] {
        let mut body = json!({"input":[
            {"type":"reasoning","encrypted_content":"old reasoning"},
            {"type":kind,"id":"cmp_old","encrypted_content":codex_compaction::encode_compaction_summary(summary)},
            {"type":"function_call","id":"fc_old","call_id":"pair","name":"read","arguments":"{}"},
            {"type":"function_call_output","call_id":"pair","output":"actual result"},
            {"role":"user","content":"continue"}
        ]});
        assert_eq!(filter_request(&mut body).unwrap(), (1, 1));
        let expected = codex_compaction::user_message_item(&format!(
            "{}\n{summary}",
            codex_compaction::SUMMARY_PREFIX
        ));
        assert_eq!(body["input"][0], expected);
        assert_eq!(body["input"][1]["call_id"], body["input"][2]["call_id"]);
        assert_eq!(body["input"][2]["output"], "actual result");
        assert_eq!(body["input"][3]["content"], "continue");
        let once = body.clone();
        assert_eq!(filter_request(&mut body).unwrap(), (0, 0));
        assert_eq!(body, once);
    }
}

#[test]
fn rejects_foreign_damaged_or_empty_summaries_before_any_mutation() {
    for kind in ["compaction", "compaction_summary", "context_compaction"] {
        for payload in [
            Value::Null,
            json!("gAAAA-foreign"),
            json!("ccswitch-compaction-v1:!broken!"),
            json!("ccswitch-compaction-v1:_w"), // invalid UTF-8
            json!(codex_compaction::encode_compaction_summary("  \n")),
        ] {
            let mut body = json!({"store":true,"input":[
                {"type":"reasoning","id":"keep_until_validated"},
                codex_compaction::compaction_output_item("valid earlier summary"),
                {"type":kind,"encrypted_content":payload}
            ]});
            let original = body.clone();
            assert!(filter_request(&mut body).is_err(), "{kind}: {payload}");
            assert_eq!(body, original);
        }
    }
}

#[test]
fn provider_setting_is_opt_in_and_roundtrips() {
    let mut p = crate::provider::Provider::with_id("test".into(), "test".into(), json!({}), None);
    assert!(!enabled(&crate::app_config::AppType::Codex, &p));
    p.meta = Some(serde_json::from_value(json!({"codexV9Filter":true})).unwrap());
    assert!(enabled(&crate::app_config::AppType::Codex, &p));
    assert!(!enabled(&crate::app_config::AppType::Claude, &p));
    assert_eq!(serde_json::to_value(p.meta).unwrap()["codexV9Filter"], true);
}

#[test]
fn only_retries_target_errors_and_honors_server_delay() {
    for code in [
        "invalid_api_key",
        "permission_denied",
        "insufficient_quota",
        "invalid_request_error",
    ] {
        assert_eq!(
            classify(&json!({"error":{"code":code,"message":"bad request"}})),
            None
        );
    }
    assert_eq!(
        classify(
            &json!({"error":{"code":"rate_limit_exceeded","message":"account quota exhausted"}})
        ),
        None
    );
    assert_eq!(
        classify(&json!({"type":"response.failed","response":{"error":null}})),
        Some(Kind::Transport)
    );
    assert_eq!(
        classify(&json!({"error":{"code":"rate_limit_exceeded"}})),
        Some(Kind::Rate)
    );
    let alb =
        b"<html><title>400 Bad Request</title><h1>400 Bad Request</h1><center>alb</center></html>";
    assert_eq!(
        http_kind(StatusCode::BAD_REQUEST, alb),
        Some(Kind::Transport)
    );
    assert_eq!(
        http_kind(
            StatusCode::BAD_REQUEST,
            b"<html><h1>400 Bad Request</h1></html>"
        ),
        None
    );
    let mut headers = HeaderMap::new();
    headers.insert("retry-after", "120".parse().unwrap());
    headers.insert("retry-after-ms", "130000".parse().unwrap());
    assert_eq!(
        retry_after(&headers, SystemTime::now()),
        Duration::from_secs(130)
    );
    headers.clear();
    headers.insert(
        "retry-after",
        "Wed, 21 Oct 2015 07:28:00 GMT".parse().unwrap(),
    );
    assert!(retry_after(&headers, SystemTime::UNIX_EPOCH) > Duration::from_secs(1));
}

#[tokio::test]
async fn gateway_concurrency_retries_http_and_sse_without_failed_output() {
    for mode in ["http400", "http429", "error", "response.failed"] {
        let mut calls = 0;
        let response = run(
            || {
                calls += 1;
                let attempt = calls;
                async move {
                    let error = json!({"code":"gateway_concurrency_limit","message":"Gateway concurrency limit reached"});
                    Ok(if attempt > 2 {
                        sse(&[json!({"type":"response.completed","response":{"output":[{"text":"recovered"}]}})])
                    } else if mode.starts_with("http") {
                        Response::builder()
                            .status(if mode == "http400" { 400 } else { 429 })
                            .header("content-type", "application/json")
                            .body(Body::from(json!({"error":error}).to_string()))
                            .unwrap()
                    } else {
                        sse(&[
                            json!({"type":"response.output_item.added","item":{"type":"function_call","name":"never_execute"}}),
                            if mode == "error" { json!({"type":"error","code":error["code"],"message":error["message"]}) }
                            else { json!({"type":"response.failed","response":{"error":error}}) }
                        ])
                    })
                }
            },
            Duration::from_secs(5), IDLE, LIMIT, false,
        ).await;
        let body = text(response).await;
        assert_eq!(calls, 3, "{mode}");
        assert!(body.contains("response.completed"), "{mode}");
        assert!(body.contains("recovered"), "{mode}");
        for discarded in [
            "never_execute",
            "gateway_concurrency_limit",
            "response.failed",
            "event: error",
        ] {
            assert!(!body.contains(discarded), "{mode}: {discarded}");
        }
    }
}

#[tokio::test]
async fn gateway_concurrency_shares_rate_budget_and_preserves_permanent_errors() {
    let mut calls = 0;
    let response = run(
        || {
            calls += 1;
            let code = if calls % 2 == 1 {
                "gateway_concurrency_limit"
            } else {
                "rate_limit_exceeded"
            };
            async move { Ok(sse(&[json!({"type":"error","code":code})])) }
        },
        Duration::from_secs(5),
        IDLE,
        LIMIT,
        false,
    )
    .await;
    let body = text(response).await;
    assert_eq!(calls, 21);
    assert_eq!(body.matches("event: error").count(), 1);
    assert!(body.contains("gateway_concurrency_limit"));
    assert!(!body.contains("response.completed"));

    for error in [
        json!({"code":"gateway_concurrency_limit","message":"Account quota exhausted"}),
        json!({"code":"invalid_api_key","message":"Gateway concurrency limit reached"}),
        json!({"code":"server_error","message":"Gateway concurrency limit reached"}),
    ] {
        let mut calls = 0;
        let response = run(
            || {
                calls += 1;
                let error = error.clone();
                async move {
                    Ok(sse(&[
                        json!({"type":"response.failed","response":{"error":error}}),
                    ]))
                }
            },
            Duration::from_secs(5),
            IDLE,
            LIMIT,
            false,
        )
        .await;
        assert!(text(response)
            .await
            .contains(error["code"].as_str().unwrap()));
        assert_eq!(calls, 1);
    }
}

#[tokio::test]
async fn failed_partial_tool_calls_are_discarded_before_retry() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let response = run(move || {
        let n = calls.fetch_add(1,Ordering::SeqCst);
        async move { Ok(if n == 0 {sse(&[
            json!({"type":"response.output_item.added","item":{"type":"function_call","name":"must_not_run"}}),
            json!({"type":"response.failed","response":{"error":{"code":"rate_limit_exceeded"}}})
        ])} else {sse(&[json!({"type":"response.completed","response":{"output":[{"text":"success"}]}})])}) }
    },Duration::from_secs(5),IDLE,LIMIT,false).await;
    let body = text(response).await;
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert!(body.contains("success"));
    assert!(!body.contains("must_not_run"));
}

#[tokio::test]
async fn transport_budget_is_ten_and_total_budget_is_twenty() {
    for mixed in [false, true] {
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let response = run(move || {
            let n = calls.fetch_add(1,Ordering::SeqCst);
            async move { Ok(sse(&[
                json!({"type":"response.output_item.added","item":{"name":"never_execute"}}),
                json!({"type":"response.failed","response":{"error":{"code":if mixed && n % 2 == 0 {"rate_limit_exceeded"} else {"stream_incomplete"}}}})
            ])) }
        },Duration::from_secs(5),IDLE,LIMIT,false).await;
        assert_eq!(count.load(Ordering::SeqCst), if mixed { 21 } else { 11 });
        let body = text(response).await;
        assert!(body.contains("response.failed"));
        assert!(!body.contains("never_execute"));
    }
}

#[tokio::test]
async fn incomplete_and_auth_failure_are_not_retried() {
    for event in [
        json!({"type":"response.incomplete","response":{"status":"incomplete"}}),
        json!({"type":"response.failed","response":{"error":{"code":"invalid_api_key"}}}),
    ] {
        let mut count = 0;
        let response = run(
            || {
                count += 1;
                let event = event.clone();
                async move { Ok(sse(&[event])) }
            },
            Duration::from_secs(5),
            IDLE,
            LIMIT,
            false,
        )
        .await;
        assert_eq!(count, 1);
        assert!(text(response)
            .await
            .contains(event["type"].as_str().unwrap()));
    }
}

#[tokio::test]
async fn incremental_crlf_unicode_and_compressed_streams() {
    use std::io::Write;
    let raw = "event: response.output_text.delta\r\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"中文\"}\r\n\r\nevent: response.completed\r\ndata: {\"type\":\"response.completed\"}\r\n\r\n";
    for encoding in ["identity", "gzip", "deflate", "br", "zstd"] {
        let bytes = match encoding {
            "gzip" => {
                let mut w =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                w.write_all(raw.as_bytes()).unwrap();
                w.finish().unwrap()
            }
            "deflate" => {
                let mut w =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                w.write_all(raw.as_bytes()).unwrap();
                w.finish().unwrap()
            }
            "br" => {
                let mut w = brotli::CompressorWriter::new(Vec::new(), 4096, 3, 20);
                w.write_all(raw.as_bytes()).unwrap();
                w.flush().unwrap();
                w.into_inner()
            }
            "zstd" => zstd::encode_all(raw.as_bytes(), 1).unwrap(),
            _ => raw.as_bytes().to_vec(),
        };
        let chunks = bytes
            .into_iter()
            .map(|b| Ok::<_, std::io::Error>(Bytes::from(vec![b])));
        let response = Response::builder()
            .header("content-type", "text/event-stream")
            .header("content-encoding", encoding)
            .body(Body::from_stream(futures::stream::iter(chunks)))
            .unwrap();
        let attempt = collect(response, LIMIT, IDLE).await;
        assert_eq!(attempt.retry, None, "{encoding}");
        assert_eq!(text(attempt.response).await, raw, "{encoding}");
    }
}

#[tokio::test]
async fn truncation_and_size_limit_do_not_release_partial_output() {
    let truncated = sse(&[json!({"type":"response.output_text.delta","delta":"partial"})]);
    let attempt = collect(truncated, LIMIT, IDLE).await;
    assert_eq!(attempt.retry, Some(Kind::Transport));
    assert!(!text(attempt.response).await.contains("partial"));
    let large = sse(&[json!({"type":"response.completed","payload":"too large"})]);
    let attempt = collect(large, 8, IDLE).await;
    assert_eq!(attempt.retry, None);
    assert!(text(attempt.response).await.contains("response_too_large"));
}

#[tokio::test(start_paused = true)]
async fn deadline_and_client_cancellation_drop_inflight_work() {
    struct Guard(Arc<AtomicUsize>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicUsize::new(0));
    let signal = dropped.clone();
    let response = handle(true, move || {
        let guard = Guard(signal.clone());
        async move {
            let _guard = guard;
            futures::future::pending::<Result<Response, ProxyError>>().await
        }
    })
    .await
    .unwrap();
    let mut body = response.into_body().into_data_stream();
    let heartbeat = body.next().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&heartbeat).contains("response.in_progress"));
    drop(body);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    let result = run(
        || async { futures::future::pending().await },
        Duration::from_secs(1),
        IDLE,
        LIMIT,
        false,
    )
    .await;
    assert!(text(result).await.contains("request_deadline"));
}
