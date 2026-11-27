use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn provider(id: &str) -> Provider {
    Provider::with_id(id.into(), id.into(), json!({"env": {"key": "test"}}), None)
}

#[test]
fn website_check_toggle_preserves_other_fields_and_roundtrips() {
    let db = Database::memory().unwrap();
    db.save_provider(&AppType::Claude, &provider("p")).unwrap();
    db.save_provider(&AppType::Codex, &provider("p")).unwrap();
    let original = json!({"futureField":{"nested":"keep"},"usage_script":{"enabled":false,"language":"javascript","code":""}});
    db.conn
        .lock()
        .unwrap()
        .execute("UPDATE providers SET meta = ?1", [original.to_string()])
        .unwrap();
    for enabled in [true, false] {
        db.set_website_check_enabled(&AppType::Claude, "p", enabled)
            .unwrap();
        let loaded = db
            .get_provider_by_id("p", &AppType::Claude)
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.meta.as_ref().unwrap().website_check_enabled,
            Some(enabled)
        );
        assert_eq!(loaded.settings_config, provider("p").settings_config);
        let roundtrip: Provider =
            serde_json::from_value(serde_json::to_value(&loaded).unwrap()).unwrap();
        assert_eq!(roundtrip.meta.unwrap().website_check_enabled, Some(enabled));
        let raw: String = db
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT meta FROM providers WHERE app_type='claude' AND id='p'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let mut meta: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            meta.as_object_mut().unwrap().remove("websiteCheckEnabled"),
            Some(json!(enabled))
        );
        assert_eq!(meta, original);
        assert_eq!(
            db.get_provider_by_id("p", &AppType::Codex)
                .unwrap()
                .unwrap()
                .meta
                .unwrap()
                .website_check_enabled,
            None
        );
    }
    assert!(db
        .set_website_check_enabled(&AppType::Claude, "missing", true)
        .is_err());
}

#[test]
fn website_check_opt_in_survives_database_backup_and_restart() {
    let _home = crate::test_support::TestHome::new();
    let db = Database::memory().unwrap();
    let mut saved = provider("monitored");
    saved.website_url = Some("https://example.com".into());
    db.save_provider(&AppType::Claude, &saved).unwrap();
    db.set_website_check_enabled(&AppType::Claude, &saved.id, true)
        .unwrap();
    let backup = db.export_sql_string().unwrap();
    let restored = Database::memory().unwrap();
    restored.import_sql_string(&backup).unwrap();
    let loaded = restored
        .get_provider_by_id(&saved.id, &AppType::Claude)
        .unwrap()
        .unwrap();
    assert_eq!(loaded.meta.unwrap().website_check_enabled, Some(true));
    assert_eq!(loaded.website_url, saved.website_url);
}

#[test]
fn website_check_targets_include_only_opted_in_providers_across_apps() {
    let db = Database::memory().unwrap();
    for (app, id) in [
        (AppType::Claude, "on"),
        (AppType::Claude, "off"),
        (AppType::Codex, "on"),
        (AppType::Gemini, "broken"),
    ] {
        let mut p = provider(id);
        p.website_url = Some("https://example.com".into());
        db.save_provider(&app, &p).unwrap();
    }
    assert!(db.website_check_targets().unwrap().is_empty());
    db.set_website_check_enabled(&AppType::Claude, "on", true)
        .unwrap();
    db.set_website_check_enabled(&AppType::Codex, "on", true)
        .unwrap();
    db.conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE providers SET meta = 'broken' WHERE id = 'broken'",
            [],
        )
        .unwrap();
    let mut targets = db.website_check_targets().unwrap();
    targets.sort();
    assert_eq!(
        targets,
        vec![
            ("claude".into(), "on".into(), "https://example.com".into()),
            ("codex".into(), "on".into(), "https://example.com".into())
        ]
    );
    db.set_website_check_enabled(&AppType::Claude, "on", false)
        .unwrap();
    db.delete_provider(&AppType::Codex, "on").unwrap();
    assert!(db.website_check_targets().unwrap().is_empty());
}

#[test]
fn provider_list_batches_endpoints_without_crossing_apps_or_changing_order() {
    let db = Database::memory().unwrap();
    let mut first = provider("z");
    first.sort_index = Some(0);
    let mut second = provider("a");
    second.sort_index = Some(1);
    db.save_provider(&AppType::Claude, &second).unwrap();
    db.save_provider(&AppType::Claude, &first).unwrap();
    db.save_provider(&AppType::Codex, &first).unwrap();
    {
        let conn = db.conn.lock().unwrap();
        // Old metadata is not authoritative, even for a provider with no endpoints.
        conn.execute(
            "UPDATE providers SET meta = ?1 WHERE app_type = 'claude' AND id = 'a'",
            [json!({"custom_endpoints": {"stale": {"url": "stale", "addedAt": 1}}}).to_string()],
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO provider_endpoints (provider_id, app_type, url, added_at) VALUES
             ('z', 'claude', 'https://shared', 20),
             ('z', 'claude', 'https://shared', 10),
             ('z', 'claude', 'https://null', NULL),
             ('z', 'codex', 'https://other-app', 30);",
        )
        .unwrap();
    }
    let providers = db.get_all_providers(&AppType::Claude).unwrap();
    assert_eq!(
        providers.keys().map(String::as_str).collect::<Vec<_>>(),
        ["z", "a"]
    );
    let endpoints = &providers["z"].meta.as_ref().unwrap().custom_endpoints;
    assert_eq!(endpoints.len(), 2);
    assert_eq!(endpoints["https://shared"].added_at, 20);
    assert_eq!(endpoints["https://shared"].url, "https://shared");
    assert_eq!(endpoints["https://null"].added_at, 0);
    assert_eq!(endpoints["https://shared"].last_used, None);
    assert!(providers["a"]
        .meta
        .as_ref()
        .unwrap()
        .custom_endpoints
        .is_empty());
    assert!(db.get_all_providers(&AppType::Gemini).unwrap().is_empty());
    assert_eq!(
        db.get_all_providers(&AppType::Codex).unwrap()["z"]
            .meta
            .as_ref()
            .unwrap()
            .custom_endpoints
            .len(),
        1
    );
}

#[test]
fn unchanged_provider_save_does_not_emit_a_database_change() {
    let db = Database::memory().unwrap();
    let mut provider = provider("same");
    db.save_provider(&AppType::Claude, &provider).unwrap();
    db.add_custom_endpoint(&AppType::Claude, "same", "https://endpoint")
        .unwrap();
    {
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "UPDATE providers SET is_current = 1, in_failover_queue = 1",
            [],
        )
        .unwrap();
    }
    let updates = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&updates);
    let cache = db.read_cache.clone();
    db.conn.lock().unwrap().update_hook(Some(
        move |action: rusqlite::hooks::Action, _: &str, table: &str, _: i64| {
            // Installing the counter replaces the production update hook.
            cache.invalidate_table(table);
            if action == rusqlite::hooks::Action::SQLITE_UPDATE && table == "providers" {
                count.fetch_add(1, Ordering::Relaxed);
            }
        },
    ));

    for _ in 0..5 {
        db.save_provider(&AppType::Claude, &provider).unwrap();
    }
    assert_eq!(updates.load(Ordering::Relaxed), 0);

    // NULL <-> value transitions and normal edits must still be persisted.
    provider.notes = Some("updated".into());
    db.save_provider(&AppType::Claude, &provider).unwrap();
    assert_eq!(
        db.get_provider_by_id("same", &AppType::Claude)
            .unwrap()
            .unwrap()
            .notes,
        provider.notes
    );
    provider.notes = None;
    provider.name = "renamed".into();
    provider.settings_config = json!({"env": {"key": "changed"}});
    db.save_provider(&AppType::Claude, &provider).unwrap();
    db.save_provider(&AppType::Claude, &provider).unwrap();
    assert_eq!(updates.load(Ordering::Relaxed), 2);
    let loaded = db
        .get_all_providers(&AppType::Claude)
        .unwrap()
        .swap_remove("same")
        .unwrap();
    assert_eq!(loaded.name, "renamed");
    assert_eq!(loaded.notes, None);
    assert_eq!(loaded.settings_config, provider.settings_config);
    assert!(loaded.in_failover_queue);
    assert_eq!(loaded.meta.unwrap().custom_endpoints.len(), 1);
    // Check the stored flag without consulting the device-local current pointer.
    let is_current: bool = db
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT is_current FROM providers WHERE id = 'same' AND app_type = 'claude'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(is_current);
}

#[test]
fn unchanged_metadata_maps_do_not_emit_database_changes_after_reload() {
    let db = Database::memory().unwrap();
    let mut provider = provider("metadata");
    provider.meta = Some(
        serde_json::from_value(json!({
            "localProxyRequestOverrides": {
                "headers": {"X-First": "one", "X-Second": "two", "X-Third": "three"},
                "body": {"reasoning": {"effort": "high"}}
            }
        }))
        .unwrap(),
    );
    db.save_provider(&AppType::Claude, &provider).unwrap();
    {
        let conn = db.conn.lock().unwrap();
        let stored: String = conn
            .query_row(
                "SELECT meta FROM providers WHERE id = 'metadata'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut reordered: serde_json::Value = serde_json::from_str(&stored).unwrap();
        let headers = reordered["localProxyRequestOverrides"]["headers"]
            .as_object_mut()
            .unwrap();
        let reversed = headers
            .iter()
            .rev()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        *headers = reversed;
        let reordered = reordered.to_string();
        assert_ne!(stored, reordered);
        // HashMap serialization order is unstable across reads/devices. Make
        // the first equivalent-but-differently-ordered save deterministic.
        conn.execute(
            "UPDATE providers SET meta = ?1 WHERE id = 'metadata'",
            [reordered],
        )
        .unwrap();
    }
    let updates = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&updates);
    let cache = db.read_cache.clone();
    db.conn.lock().unwrap().update_hook(Some(
        move |action: rusqlite::hooks::Action, _: &str, table: &str, _: i64| {
            // Installing the counter replaces the production update hook.
            cache.invalidate_table(table);
            if action == rusqlite::hooks::Action::SQLITE_UPDATE && table == "providers" {
                count.fetch_add(1, Ordering::Relaxed);
            }
        },
    ));
    db.save_provider(&AppType::Claude, &provider).unwrap();
    for _ in 0..5 {
        provider = db
            .get_provider_by_id("metadata", &AppType::Claude)
            .unwrap()
            .unwrap();
        db.save_provider(&AppType::Claude, &provider).unwrap();
    }
    assert_eq!(updates.load(Ordering::Relaxed), 0);

    provider
        .meta
        .as_mut()
        .unwrap()
        .local_proxy_request_overrides
        .as_mut()
        .unwrap()
        .headers
        .insert("X-First".into(), "changed".into());
    db.save_provider(&AppType::Claude, &provider).unwrap();
    assert_eq!(updates.load(Ordering::Relaxed), 1);
    let loaded = db
        .get_provider_by_id("metadata", &AppType::Claude)
        .unwrap()
        .unwrap();
    assert_eq!(
        loaded
            .meta
            .unwrap()
            .local_proxy_request_overrides
            .unwrap()
            .headers["X-First"],
        "changed"
    );
}

/// 新版本写入、本版本认不出的 meta（比如新增的枚举值）：读出来要报错点名，不能换成默认值，
/// 库里的原文一个字节都不能动；同应用里别的供应商单独读仍然正常。
#[test]
fn unreadable_stored_json_is_reported_and_left_untouched() {
    let db = Database::memory().unwrap();
    db.save_provider(&AppType::Claude, &provider("good"))
        .unwrap();
    db.save_provider(&AppType::Claude, &provider("newer"))
        .unwrap();
    db.save_provider(&AppType::Claude, &provider("broken"))
        .unwrap();
    let newer_meta = json!({
        "claudeDesktopMode": "mode-from-a-newer-version",
        "usageScript": {"enabled": true, "language": "javascript", "code": "return 1"}
    })
    .to_string();
    let raw = |id: &str| -> (String, String) {
        db.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT settings_config, meta FROM providers WHERE app_type = 'claude' AND id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    };
    {
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "UPDATE providers SET meta = ?1 WHERE app_type = 'claude' AND id = 'newer'",
            [&newer_meta],
        )
        .unwrap();
        conn.execute(
            "UPDATE providers SET settings_config = '{\"env\":' WHERE app_type = 'claude' AND id = 'broken'",
            [],
        )
        .unwrap();
    }
    let stored = (raw("newer"), raw("broken"));

    let error = db
        .get_all_providers(&AppType::Claude)
        .unwrap_err()
        .to_string();
    // 列表停在第一条读不出的行（按排序是 broken），并点名它。
    assert!(
        error.contains("broken") && error.contains("settings_config"),
        "{error}"
    );
    let error = db
        .get_provider_by_id("newer", &AppType::Claude)
        .unwrap_err()
        .to_string();
    assert!(error.contains("newer") && error.contains("meta"), "{error}");
    let error = db
        .get_provider_by_id("broken", &AppType::Claude)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("broken") && error.contains("settings_config"),
        "{error}"
    );

    assert!(db
        .get_provider_by_id("good", &AppType::Claude)
        .unwrap()
        .is_some());
    assert_eq!((raw("newer"), raw("broken")), stored);
}
