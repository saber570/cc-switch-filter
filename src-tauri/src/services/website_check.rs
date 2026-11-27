//! Opt-in website checks. Runs independently of windows, balances and routing health.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use reqwest::Url;
use serde::Serialize;
use tokio::sync::{Mutex, Notify, Semaphore};
use tokio::time::Instant;

use crate::{app_config::AppType, database::Database, provider::Provider};

const CHECK_INTERVAL: Duration = Duration::from_secs(10 * 60);
const CHECK_TIMEOUT: Duration = Duration::from_secs(5);
const RECONCILE_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteCheckStatus {
    pub url: String,
    pub checking: bool,
    pub available: Option<bool>,
    pub checked_at: Option<i64>,
    pub http_status: Option<u16>,
    pub error: Option<String>,
}

struct Entry {
    generation: u64,
    status: WebsiteCheckStatus,
    finished_at: Option<Instant>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for Entry {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

pub struct WebsiteMonitor {
    entries: Mutex<HashMap<(String, String), Entry>>,
    wake: Notify,
    slots: Arc<Semaphore>,
    revision: AtomicU64,
    next_generation: AtomicU64,
}

impl Default for WebsiteMonitor {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            wake: Notify::new(),
            slots: Arc::new(Semaphore::new(4)),
            revision: AtomicU64::new(0),
            next_generation: AtomicU64::new(0),
        }
    }
}

/// Only the saved website is checked, never a guessed API/model endpoint.
pub fn website_url(provider: &Provider) -> Result<String, String> {
    let raw = provider.website_url.as_deref().unwrap_or_default().trim();
    normalize_url(raw)
}

fn normalize_url(raw: &str) -> Result<String, String> {
    let mut url = parse_url(raw.trim())?;
    url.set_fragment(None);
    Ok(url.to_string())
}

fn parse_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|_| "Invalid website URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Website must be an HTTP(S) URL without credentials".into());
    }
    Ok(url)
}

impl WebsiteMonitor {
    pub fn start(self: &Arc<Self>, db: Arc<Database>) {
        let monitor = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                let revision = monitor.revision.load(Ordering::SeqCst);
                let db = db.clone();
                // Database reads may wait for imports/session sync. Never hold a lock over HTTP.
                let targets = tauri::async_runtime::spawn_blocking(move || {
                    let mut targets = HashMap::new();
                    for (app, id, url) in db.website_check_targets().map_err(|e| e.to_string())? {
                        targets.insert((app, id), normalize_url(&url));
                    }
                    Ok::<_, String>(targets)
                })
                .await;
                match targets {
                    Ok(Ok(targets)) => monitor.reconcile(targets, revision).await,
                    Ok(Err(error)) => log::warn!("Website monitor: {error}"),
                    Err(error) => log::warn!("Website monitor task: {error}"),
                }
                tokio::select! {
                    _ = monitor.wake.notified() => {},
                    _ = tokio::time::sleep(RECONCILE_INTERVAL) => {},
                }
            }
        });
    }

    /// Disabling, editing or manual refresh invalidates any in-flight result.
    pub async fn reset(&self, app: &AppType, id: &str) {
        let mut entries = self.entries.lock().await;
        self.revision.fetch_add(1, Ordering::SeqCst);
        entries.remove(&(app.to_string(), id.into()));
        self.wake.notify_one();
    }

    pub async fn status(&self, app: &AppType, provider: &Provider) -> Option<WebsiteCheckStatus> {
        if provider.meta.as_ref().and_then(|m| m.website_check_enabled) != Some(true) {
            return None;
        }
        let url = website_url(provider);
        self.entries
            .lock()
            .await
            .get(&(app.to_string(), provider.id.clone()))
            .filter(|entry| entry.status.url == url.clone().unwrap_or_default())
            .map(|entry| entry.status.clone())
    }

    async fn reconcile(
        self: &Arc<Self>,
        targets: HashMap<(String, String), Result<String, String>>,
        revision: u64,
    ) {
        let mut entries = self.entries.lock().await;
        // A toggle/manual refresh may have overtaken the database snapshot.
        if self.revision.load(Ordering::SeqCst) != revision {
            return;
        }
        entries.retain(|key, entry| {
            targets
                .get(key)
                .is_some_and(|url| entry.status.url == url.clone().unwrap_or_default())
        });
        for (key, target) in targets {
            if entries.get(&key).is_some_and(|entry| {
                entry.status.checking
                    || entry
                        .finished_at
                        .is_some_and(|at| at.elapsed() < CHECK_INTERVAL)
            }) {
                continue;
            }
            let status = WebsiteCheckStatus {
                url: target.clone().unwrap_or_default(),
                checking: true,
                available: None,
                checked_at: None,
                http_status: None,
                error: None,
            };
            let monitor = self.clone();
            let task_key = key.clone();
            let generation = self.next_generation.fetch_add(1, Ordering::SeqCst);
            let task = tokio::spawn(async move {
                let Ok(_permit) = monitor.slots.clone().acquire_owned().await else {
                    return;
                };
                let (http_status, error) = match target {
                    Ok(url) => probe(&url, CHECK_TIMEOUT).await,
                    Err(error) => (None, Some(error)),
                };
                let mut entries = monitor.entries.lock().await;
                if let Some(entry) = entries
                    .get_mut(&task_key)
                    .filter(|entry| entry.generation == generation)
                {
                    entry.status.checking = false;
                    entry.status.available = Some(error.is_none());
                    entry.status.checked_at = Some(chrono::Utc::now().timestamp_millis());
                    entry.status.http_status = http_status;
                    entry.status.error = error;
                    entry.finished_at = Some(Instant::now());
                    entry.task = None;
                }
            });
            entries.insert(
                key,
                Entry {
                    generation,
                    status,
                    finished_at: None,
                    task: Some(task),
                },
            );
        }
    }
}

/// One five-second budget, including redirects. No warmup, authentication, body or retries.
async fn probe(raw: &str, timeout: Duration) -> (Option<u16>, Option<String>) {
    let result = tokio::time::timeout(timeout, async {
        let mut url = parse_url(raw)?;
        for _ in 0..=5 {
            let client = crate::proxy::http_client::no_redirect_for_url(url.as_str())?;
            let response = client
                .get(url.clone())
                .timeout(timeout)
                .header("accept", "*/*")
                .header("accept-encoding", "identity")
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        "Request timed out"
                    } else if e.is_connect() {
                        "DNS, TLS or connection failed"
                    } else {
                        "Website request failed"
                    }
                    .to_string()
                })?;
            let status = response.status();
            if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or("Redirect has no valid location")?;
                let next = url.join(location).map_err(|_| "Invalid redirect URL")?;
                url = parse_url(next.as_str())?;
                continue;
            }
            return Ok((
                Some(status.as_u16()),
                (!status.is_success()).then(|| format!("HTTP {}", status.as_u16())),
            ));
        }
        Err::<_, String>("Too many redirects".into())
    })
    .await;
    match result {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => (None, Some(error)),
        Err(_) => (None, Some("Request timed out (5s)".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn read_headers(stream: &mut tokio::net::TcpStream) -> String {
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let byte = stream.read_u8().await.unwrap();
            request.push(byte);
            assert!(request.len() < 16_384);
        }
        String::from_utf8(request).unwrap().to_lowercase()
    }

    // All HTTP tests use ephemeral loopback ports. No real provider or user database.
    async fn server(
        responses: Vec<String>,
        delay: Duration,
    ) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let request = read_headers(&mut stream).await;
                assert!(request.starts_with("get "));
                assert!(!request.contains("authorization:") && !request.contains("cookie:"));
                let index = count
                    .fetch_add(1, Ordering::SeqCst)
                    .min(responses.len() - 1);
                tokio::time::sleep(delay).await;
                let _ = stream.write_all(responses[index].as_bytes()).await;
            }
        });
        (url, calls, task)
    }

    fn reply(code: u16) -> String {
        format!("HTTP/1.1 {code} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
    }

    fn targets(url: &str) -> HashMap<(String, String), Result<String, String>> {
        HashMap::from([(("claude".into(), "test".into()), Ok(url.into()))])
    }

    async fn finish(monitor: &WebsiteMonitor) -> WebsiteCheckStatus {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let status = monitor
                    .entries
                    .lock()
                    .await
                    .values()
                    .next()
                    .unwrap()
                    .status
                    .clone();
                if !status.checking {
                    return status;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }

    #[test]
    fn website_check_target_requires_an_explicit_http_website() {
        let mut provider = Provider::with_id(
            "p".into(),
            "P".into(),
            serde_json::json!({"base_url":"https://api.example.com"}),
            None,
        );
        assert!(website_url(&provider).is_err());
        for url in [
            "file:///tmp/a",
            "ftp://example.com",
            "https://user:secret@example.com",
            "bad",
        ] {
            provider.website_url = Some(url.into());
            assert!(website_url(&provider).is_err());
        }
        provider.website_url = Some(" https://example.com/pricing#cost ".into());
        assert_eq!(
            website_url(&provider).unwrap(),
            "https://example.com/pricing"
        );
    }

    #[tokio::test]
    async fn website_check_classifies_success_and_http_errors_without_retries() {
        for code in [200, 204, 401, 403, 404, 429, 500, 503] {
            let (url, calls, server) = server(vec![reply(code)], Duration::ZERO).await;
            let (status, error) = probe(&url, CHECK_TIMEOUT).await;
            assert_eq!(status, Some(code));
            assert_eq!(error.is_none(), code < 300);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            server.abort();
        }
    }

    #[tokio::test]
    async fn website_check_follows_redirects_and_bounds_loops() {
        let redirect = "HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string();
        let (url, calls, server) = server(vec![redirect.clone(), reply(200)], Duration::ZERO).await;
        assert_eq!(probe(&url, CHECK_TIMEOUT).await, (Some(200), None));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        server.abort();
        let (url, calls, server) = self::server(vec![redirect], Duration::ZERO).await;
        assert_eq!(
            probe(&url, CHECK_TIMEOUT).await.1.as_deref(),
            Some("Too many redirects")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 6);
        server.abort();
    }

    #[tokio::test]
    async fn website_check_times_out_once_and_reports_connection_failure() {
        let (url, calls, server) = server(vec![reply(200)], Duration::from_secs(1)).await;
        let start = Instant::now();
        assert!(probe(&url, Duration::from_millis(50)).await.1.is_some());
        assert!(start.elapsed() < Duration::from_millis(500));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        server.abort();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        assert!(probe(&url, CHECK_TIMEOUT).await.1.is_some());
    }

    #[tokio::test]
    async fn website_check_only_repeats_after_ten_minutes_and_recovers() {
        let monitor = Arc::new(WebsiteMonitor::default());
        let (url, calls, server) = server(vec![reply(503), reply(200)], Duration::ZERO).await;
        monitor.reconcile(HashMap::new(), 0).await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        monitor.reconcile(targets(&url), 0).await;
        assert_eq!(finish(&monitor).await.available, Some(false));
        tokio::time::pause();
        tokio::time::advance(CHECK_INTERVAL - Duration::from_millis(1)).await;
        monitor.reconcile(targets(&url), 0).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_millis(1)).await;
        monitor.reconcile(targets(&url), 0).await;
        tokio::time::resume();
        assert_eq!(finish(&monitor).await.available, Some(true));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        server.abort();
    }

    #[tokio::test]
    async fn website_check_disable_cancels_and_rejects_an_older_snapshot() {
        let monitor = Arc::new(WebsiteMonitor::default());
        let (url, calls, server) = server(vec![reply(200)], Duration::from_secs(1)).await;
        monitor.reconcile(targets(&url), 0).await;
        while calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        monitor.reset(&AppType::Claude, "test").await;
        monitor.reconcile(targets(&url), 0).await;
        assert!(monitor.entries.lock().await.is_empty());
        monitor.reconcile(HashMap::new(), 1).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // The permit is released by abort; no lingering HTTP request can publish a result.
        tokio::time::timeout(Duration::from_secs(1), async {
            while monitor.slots.available_permits() != 4 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(monitor.entries.lock().await.is_empty());
        server.abort();
    }

    #[tokio::test]
    async fn website_check_url_change_drops_old_result_and_manual_refresh_is_immediate() {
        let monitor = Arc::new(WebsiteMonitor::default());
        let (url, _, first) = server(vec![reply(200)], Duration::ZERO).await;
        monitor.reconcile(targets(&url), 0).await;
        assert_eq!(finish(&monitor).await.available, Some(true));
        let (new_url, calls, second) = server(vec![reply(503), reply(200)], Duration::ZERO).await;
        monitor.reconcile(targets(&new_url), 0).await;
        assert_eq!(finish(&monitor).await.available, Some(false));
        monitor.reset(&AppType::Claude, "test").await;
        monitor.reconcile(targets(&new_url), 1).await;
        assert_eq!(finish(&monitor).await.available, Some(true));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        first.abort();
        second.abort();
    }

    #[tokio::test]
    async fn website_check_limits_concurrency_and_cancels_queued_checks() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let server = tokio::spawn(async move {
            let mut streams = Vec::new();
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                read_headers(&mut stream).await;
                count.fetch_add(1, Ordering::SeqCst);
                streams.push(stream); // Do not send headers until the monitor cancels.
            }
        });
        let monitor = Arc::new(WebsiteMonitor::default());
        let targets = (0..8)
            .map(|i| (("claude".into(), i.to_string()), Ok(url.clone())))
            .collect();
        monitor.reconcile(targets, 0).await;
        tokio::time::timeout(Duration::from_secs(1), async {
            while calls.load(Ordering::SeqCst) < 4 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(monitor.slots.available_permits(), 0);
        monitor.reconcile(HashMap::new(), 0).await;
        tokio::time::timeout(Duration::from_secs(1), async {
            while monitor.slots.available_permits() != 4 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        assert!(monitor.entries.lock().await.is_empty());
        server.abort();
    }
}
