use cc_switch_lib::{
    get_settings, identity, parse_deeplink_url, update_settings, AppSettings, Database,
    MultiAppConfig,
};
use serial_test::serial;
use std::{fs, path::PathBuf};

#[test]
#[serial]
fn switchplus_owns_its_state_and_rejects_upstream_identity() {
    let sandbox = tempfile::tempdir().unwrap();
    let original = std::env::var_os("SWITCHPLUS_TEST_HOME");
    std::env::set_var("SWITCHPLUS_TEST_HOME", sandbox.path());
    let legacy = sandbox.path().join(".cc-switch");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(
        legacy.join("settings.json"),
        r#"{"language":"legacy-sentinel"}"#,
    )
    .unwrap();
    fs::write(legacy.join("cc-switch.db"), b"upstream-database-sentinel").unwrap();
    fs::write(legacy.join("config.json"), b"upstream-config-sentinel").unwrap();
    assert_eq!(identity::APP_IDENTIFIER, "io.github.saber570.switchplus");
    assert_eq!(identity::DEFAULT_PROXY_PORT, 15731);
    let ours = sandbox.path().join(".switchplus");
    assert_eq!(identity::device_dir(), ours);
    assert_eq!(identity::data_dir(), ours);
    assert!(identity::is_cc_switch_path(&legacy));
    assert!(identity::is_cc_switch_path(
        &legacy.join("nested/new/settings.json")
    ));
    assert!(identity::is_cc_switch_path(
        &sandbox.path().join("com.ccswitch.desktop/app_paths.json")
    ));
    assert!(!identity::is_cc_switch_path(&ours));
    assert!(identity::ensure_write_allowed(&legacy.join("new.json")).is_err());
    assert!(identity::ensure_read_allowed(&legacy.join("settings.json")).is_err());
    assert!(
        cc_switch_lib::read_json_file::<serde_json::Value>(&legacy.join("settings.json")).is_err()
    );
    assert!(cc_switch_lib::live::engine::read_current(&legacy.join("config.json")).is_err());
    assert!(
        parse_deeplink_url("ccswitch://v1/import?resource=provider&app=claude&name=X").is_err()
    );
    assert!(parse_deeplink_url("switchplus://v1/import?resource=provider&app=claude&name=X&endpoint=https%3A%2F%2Fexample.com&apiKey=test").is_ok());
    let state = MultiAppConfig::load().unwrap();
    let _ = state;
    let db = Database::init().unwrap();
    drop(db);
    assert!(ours.join("cc-switch.db").is_file());
    assert!(!ours.join("switchplus.db").exists());
    let initial_settings = futures::executor::block_on(get_settings()).unwrap();
    assert_ne!(
        initial_settings.language.as_deref(),
        Some("legacy-sentinel")
    );
    update_settings(AppSettings {
        language: Some("zh".into()),
        ..Default::default()
    })
    .unwrap();
    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(ours.join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["language"], "zh");
    assert_eq!(
        fs::read(legacy.join("settings.json")).unwrap(),
        br#"{"language":"legacy-sentinel"}"#
    );
    assert_eq!(
        fs::read(legacy.join("cc-switch.db")).unwrap(),
        b"upstream-database-sentinel"
    );
    assert_eq!(
        fs::read(legacy.join("config.json")).unwrap(),
        b"upstream-config-sentinel"
    );
    let upstream_running = identity::cc_switch_running().unwrap();
    if upstream_running {
        assert!(identity::ensure_no_cc_switch().is_err());
        assert!(
            identity::ensure_write_allowed(&PathBuf::from(r"C:\Users\zpw\.codex\config.toml"))
                .is_err()
        );
    }
    println!("isolated database/settings: passed; old sentinel files unchanged; ccswitch deep link rejected; CC Switch running: {upstream_running}");
    match original {
        Some(value) => std::env::set_var("SWITCHPLUS_TEST_HOME", value),
        None => std::env::remove_var("SWITCHPLUS_TEST_HOME"),
    }
}

#[test]
#[serial]
fn switchplus_upgrade_reopens_existing_data_with_either_filename() {
    let original = std::env::var_os("SWITCHPLUS_TEST_HOME");
    for filename in ["cc-switch.db", "switchplus.db"] {
        let sandbox = tempfile::tempdir().unwrap();
        std::env::set_var("SWITCHPLUS_TEST_HOME", sandbox.path());
        let ours = sandbox.path().join(".switchplus");
        let db = Database::init().unwrap();
        db.set_setting("upgrade_sentinel", "existing-data").unwrap();
        let mut config = futures::executor::block_on(db.get_global_proxy_config()).unwrap();
        config.listen_port = 15722;
        futures::executor::block_on(db.update_global_proxy_config(config)).unwrap();
        drop(db);
        if filename == "switchplus.db" {
            fs::rename(ours.join("cc-switch.db"), ours.join(filename)).unwrap();
        }
        let db = Database::init().unwrap();
        assert_eq!(
            db.get_setting("upgrade_sentinel").unwrap().as_deref(),
            Some("existing-data")
        );
        assert_eq!(
            futures::executor::block_on(db.get_global_proxy_config())
                .unwrap()
                .listen_port,
            15722
        );
        assert_eq!(identity::database_path(), ours.join(filename));
        drop(db);
        let other = if filename == "cc-switch.db" {
            "switchplus.db"
        } else {
            "cc-switch.db"
        };
        assert!(!ours.join(other).exists());
    }
    match original {
        Some(value) => std::env::set_var("SWITCHPLUS_TEST_HOME", value),
        None => std::env::remove_var("SWITCHPLUS_TEST_HOME"),
    }
}

#[test]
fn codex_patch_preserves_cc_switch_provider_table() {
    use cc_switch_lib::live::project::codex::{proxy_route_table, CodexConfigPatch, RouteWrite};
    let mut doc = r#"model_provider = "custom"
model = "existing-model"

[model_providers.custom]
name = "CC Switch"
base_url = "http://127.0.0.1:15721/v1"
experimental_bearer_token = "PROXY_MANAGED"
wire_api = "responses"

[profiles.previous]
model_provider = "custom"
"#
    .parse::<toml_edit::DocumentMut>()
    .unwrap();
    let previous = doc["model_providers"]["custom"].to_string();
    let patch = CodexConfigPatch {
        top: vec![],
        nested: vec![],
        exclusive: vec![],
        outgoing: vec![],
        route: RouteWrite::Custom(proxy_route_table(
            "SwitchPlus",
            "http://127.0.0.1:15731/v1",
            false,
        )),
        catalog: false,
        retired: vec![],
    };
    patch
        .apply_to(std::path::Path::new("isolated-config.toml"), &mut doc)
        .unwrap();
    assert_eq!(doc["model_provider"].as_str(), Some("switchplus"));
    assert_eq!(doc["model_providers"]["custom"].to_string(), previous);
    assert_eq!(
        doc["profiles"]["previous"]["model_provider"].as_str(),
        Some("custom")
    );
    assert_eq!(
        doc["model_providers"]["switchplus"]["base_url"].as_str(),
        Some("http://127.0.0.1:15731/v1")
    );
    assert_eq!(
        doc["model_providers"]["switchplus"]["experimental_bearer_token"].as_str(),
        Some("SWITCHPLUS_PROXY_MANAGED")
    );
}

#[cfg(windows)]
#[test]
fn cc_switch_junction_targets_are_rejected() {
    use std::os::windows::process::CommandExt;
    let sandbox = tempfile::tempdir().unwrap();
    let legacy = sandbox.path().join(".cc-switch");
    let alias = sandbox.path().join("alias");
    fs::create_dir_all(&legacy).unwrap();
    let output = std::process::Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&alias)
        .arg(&legacy)
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "junction creation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(identity::is_cc_switch_path(&alias));
    assert!(identity::ensure_read_allowed(&alias.join("settings.json")).is_err());
    assert!(identity::ensure_write_allowed(&alias.join("settings.json")).is_err());
    assert!(identity::is_cc_switch_path(
        &alias.join("not-created/settings.json")
    ));
    fs::remove_dir(&alias).unwrap();
}
