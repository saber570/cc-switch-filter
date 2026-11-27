use tauri::State;

use crate::services::website_check::{website_url, WebsiteCheckStatus};
use crate::{app_config::AppType, provider::Provider, store::AppState};

async fn provider(
    state: &State<'_, AppState>,
    app: AppType,
    id: String,
) -> Result<Provider, String> {
    super::run_db_task(state, move |state| {
        state
            .db
            .get_provider_by_id(&id, &app)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Provider not found".to_string())
    })
    .await
}

#[tauri::command]
pub async fn get_website_check_status(
    state: State<'_, AppState>,
    app: String,
    provider_id: String,
) -> Result<Option<WebsiteCheckStatus>, String> {
    let app: AppType = app.parse()?;
    let provider = provider(&state, app.clone(), provider_id).await?;
    Ok(state.website_monitor.status(&app, &provider).await)
}

#[tauri::command]
pub async fn set_website_check_enabled(
    state: State<'_, AppState>,
    app: String,
    provider_id: String,
    enabled: bool,
) -> Result<(), String> {
    let app: AppType = app.parse()?;
    if enabled {
        website_url(&provider(&state, app.clone(), provider_id.clone()).await?)?;
    }
    let id = provider_id.clone();
    let app_copy = app.clone();
    super::run_db_task(&state, move |state| {
        state.db.set_website_check_enabled(&app_copy, &id, enabled)
    })
    .await?;
    state.website_monitor.reset(&app, &provider_id).await;
    Ok(())
}

#[tauri::command]
pub async fn refresh_website_check(
    state: State<'_, AppState>,
    app: String,
    provider_id: String,
) -> Result<(), String> {
    let app: AppType = app.parse()?;
    let provider = provider(&state, app.clone(), provider_id.clone()).await?;
    if provider.meta.as_ref().and_then(|m| m.website_check_enabled) != Some(true) {
        return Err("Website checks are disabled".into());
    }
    state.website_monitor.reset(&app, &provider_id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use std::sync::Arc;
    use tauri::Manager;

    #[test]
    fn website_check_commands_gate_refresh_and_save_only_valid_opt_ins() {
        let _home = crate::test_support::TestHome::new();
        let db = Arc::new(Database::memory().unwrap());
        let mut provider = Provider::with_id("p".into(), "P".into(), serde_json::json!({}), None);
        db.save_provider(&AppType::Claude, &provider).unwrap();
        let app = tauri::test::mock_builder()
            .manage(AppState::new(db.clone()))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        tauri::async_runtime::block_on(async {
            assert!(
                get_website_check_status(app.state(), "claude".into(), "p".into())
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(
                refresh_website_check(app.state(), "claude".into(), "p".into())
                    .await
                    .is_err()
            );
            assert!(
                set_website_check_enabled(app.state(), "claude".into(), "p".into(), true)
                    .await
                    .is_err()
            );
            provider.website_url = Some("https://example.com".into());
            db.save_provider(&AppType::Claude, &provider).unwrap();
            set_website_check_enabled(app.state(), "claude".into(), "p".into(), true)
                .await
                .unwrap();
            assert_eq!(
                db.get_provider_by_id("p", &AppType::Claude)
                    .unwrap()
                    .unwrap()
                    .meta
                    .unwrap()
                    .website_check_enabled,
                Some(true)
            );
            refresh_website_check(app.state(), "claude".into(), "p".into())
                .await
                .unwrap();
            set_website_check_enabled(app.state(), "claude".into(), "p".into(), false)
                .await
                .unwrap();
            assert!(
                refresh_website_check(app.state(), "claude".into(), "p".into())
                    .await
                    .is_err()
            );
            assert!(
                get_website_check_status(app.state(), "claude".into(), "p".into())
                    .await
                    .unwrap()
                    .is_none()
            );
        });
    }
}
