//! SwitchPlus product identity. Keep protocol envelopes and upstream schema
//! names unchanged so imported histories and future upstream merges still work.
pub const APP_NAME: &str = "SwitchPlus";
pub const DATA_DIR: &str = ".switchplus";
pub const APP_UPDATES_ENABLED: bool = false;

#[cfg(test)]
mod tests {
    #[test]
    fn switchplus_install_and_deep_links_are_separate_from_upstream() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["productName"], super::APP_NAME);
        assert_eq!(config["identifier"], "io.github.saber570.switchplus");
        assert_eq!(
            config["plugins"]["deep-link"]["desktop"]["schemes"],
            serde_json::json!(["switchplus"])
        );
        assert_eq!(
            config["plugins"]["updater"]["endpoints"],
            serde_json::json!([])
        );
        assert_eq!(config["bundle"]["createUpdaterArtifacts"], false);
        let input = "switchplus://v1/import?resource=provider&app=codex&name=Test&endpoint=https%3A%2F%2Fexample.com&apiKey=test";
        assert!(crate::deeplink::parse_deeplink_url(input).is_ok());
        assert!(crate::deeplink::parse_deeplink_url(
            &input.replace("switchplus://", "ccswitch://")
        )
        .is_ok());
    }

    #[tokio::test]
    async fn switchplus_new_database_uses_its_own_port_but_keeps_custom_ports() {
        let db = crate::database::Database::memory().unwrap();
        let mut config = db.get_global_proxy_config().await.unwrap();
        assert_eq!(config.listen_port, 15722);
        config.listen_port = 18000;
        db.update_global_proxy_config(config).await.unwrap();
        assert_eq!(
            db.get_global_proxy_config().await.unwrap().listen_port,
            18000
        );
    }
}
