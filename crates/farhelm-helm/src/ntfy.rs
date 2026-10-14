//! Helm-owned ntfy alerts: connection-local waiting evidence, one bounded fleet
//! queue, and explicit configuration routes. Neither secrets nor peer titles
//! belong in diagnostics; HTTP responses from ntfy are never read as text.

use crate::{AppState, store::HelmStore};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use farhelm_proto::{
    SessionInfo, SessionStatus,
    ntfy::{SaveSettings, Settings},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::{sync::mpsc, time::Instant};

pub(crate) const SETTLE: Duration = Duration::from_secs(10);
const SEND_TIMEOUT: Duration = Duration::from_secs(5);
const QUEUE_CAPACITY: usize = 32;
const PER_MINUTE: usize = 5;

/// Stored configuration never crosses the API boundary as this type. The
/// token lives in helm.db with the account's other helm configuration.
#[derive(Clone, Default, Deserialize, Serialize)]
pub(crate) struct Config {
    pub settings: Settings,
    pub token: Option<String>,
}

impl Config {
    /// Resolve a write against the previous secret, refusing values with fixed
    /// messages so a bad URL, topic or token cannot escape in an error string.
    fn saved(previous: Self, save: SaveSettings) -> Result<Self, &'static str> {
        let server =
            reqwest::Url::parse(&save.server_url).map_err(|_| "enter a valid server URL")?;
        if save.server_url.len() > 2048
            || !matches!(server.scheme(), "http" | "https")
            || server.host_str().is_none()
            || !server.username().is_empty()
            || server.password().is_some()
            || server.query().is_some()
            || server.fragment().is_some()
        {
            return Err("use an http or https server URL without credentials, query or fragment");
        }
        if save.topic.len() > 64
            || !save
                .topic
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err("topic must contain at most 64 letters, numbers, underscores or dashes");
        }
        if save.enabled && save.topic.is_empty() {
            return Err("enter a topic before enabling ntfy alerts");
        }
        let token = match save.token {
            None => previous.token,
            Some(value) if value.is_empty() => None,
            Some(value) => {
                if value.len() > 4096 || !value.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
                    return Err("access token must contain at most 4096 visible ASCII characters");
                }
                Some(value)
            }
        };
        Ok(Self {
            settings: Settings {
                enabled: save.enabled,
                server_url: server.as_str().trim_end_matches('/').to_string(),
                topic: save.topic,
                priority: save.priority,
                token_set: token.is_some(),
            },
            token,
        })
    }
}

/// A list observation contains only the fields allowed into an alert. Keep the
/// raw status: the display cache may retain Waiting over an Unknown reading,
/// but that retained value is not fresh evidence for settling a spell.
pub(crate) struct Observation {
    pub(crate) id: String,
    title: String,
    status: SessionStatus,
}

impl From<&SessionInfo> for Observation {
    fn from(session: &SessionInfo) -> Self {
        Self {
            id: session.id.clone(),
            title: clean_text(&session.title),
            status: session.status.clone(),
        }
    }
}

/// One waiting spell's only state: pending since a real Waiting reading, or
/// already handled (including baseline and dropped sends). Unknown adds no evidence.
enum Spell {
    Pending(Instant),
    Handled,
}

/// Connection-local evidence starts empty after every reconnect, rather than
/// consulting a durable cache that survives helm restarts. A first successful
/// list is the baseline; removals and resolved non-waiting readings end spells.
#[derive(Default)]
pub(crate) struct Policy {
    initialized: bool,
    spells: HashMap<String, Spell>,
}

impl Policy {
    /// Consume one accepted listing at an injected monotonic time. Returned
    /// titles have settled once; queue failures must not turn into later retries.
    pub(crate) fn observe(&mut self, list: &[Observation], now: Instant) -> Vec<String> {
        let ids: HashSet<_> = list.iter().map(|s| s.id.as_str()).collect();
        self.spells.retain(|id, _| ids.contains(id.as_str()));
        let mut alerts = Vec::new();
        for session in list {
            match session.status {
                SessionStatus::Unknown => {}
                SessionStatus::Waiting => {
                    let spell = self.spells.entry(session.id.clone()).or_insert_with(|| {
                        if self.initialized {
                            Spell::Pending(now)
                        } else {
                            Spell::Handled
                        }
                    });
                    if let Spell::Pending(since) = spell
                        && now.saturating_duration_since(*since) >= SETTLE
                    {
                        alerts.push(session.title.clone());
                        *spell = Spell::Handled;
                    }
                }
                _ => {
                    self.spells.remove(&session.id);
                }
            }
        }
        self.initialized = true;
        alerts
    }
}

/// Strip characters that must never be shown as themselves and cap each
/// untrusted display field before it enters the bounded queue. The filter is
/// the repository's shared presentation list, not just controls: a title
/// carrying a bidirectional override could otherwise reorder the alert's text
/// on the phone so it appears to name a different host. A title generated from
/// a folder is still the session's title.
fn clean_text(value: &str) -> String {
    value
        .chars()
        .filter(|c| !farhelm_proto::text::is_presentation_unsafe(*c))
        .take(200)
        .collect()
}

/// A fleet-wide fixed window counts attempts, including failures, so an outage
/// cannot turn the cap into a rapid series of failed requests. Dropped counts
/// carry to the next permitted alert; no timer creates a summary send.
struct RateWindow {
    start: Instant,
    sent: usize,
    dropped: usize,
    warned: bool,
}

impl RateWindow {
    fn new(now: Instant) -> Self {
        Self {
            start: now,
            sent: 0,
            dropped: 0,
            warned: false,
        }
    }
    /// None drops this attempt; Some returns how many earlier attempts to mention.
    fn admit(&mut self, now: Instant) -> Option<usize> {
        if now.saturating_duration_since(self.start) >= Duration::from_secs(60) {
            self.start = now;
            self.sent = 0;
            self.warned = false;
        }
        if self.sent == PER_MINUTE {
            if !self.warned {
                tracing::warn!("ntfy alert rate cap reached; dropping excess alerts");
                self.warned = true;
            }
            self.dropped = self.dropped.saturating_add(1);
            return None;
        }
        self.sent += 1;
        Some(std::mem::take(&mut self.dropped))
    }
}

/// Queue and direct-test client share settings, but the test request deliberately
/// bypasses transition policy, queue and rate cap. The last handle aborts the
/// sender so an embedded helm's shutdown leaves no detached outbound task.
#[derive(Clone)]
pub(crate) struct Alerts(Arc<Inner>);
/// Handles own the task, which holds only the store/client/receiver: no Arc
/// cycle keeps either an embedded helm or a test fixture alive after shutdown.
struct Inner {
    store: HelmStore,
    client: Option<reqwest::Client>,
    queue: mpsc::Sender<Alert>,
    task: tokio::task::JoinHandle<()>,
}
/// The host name is resolved by the sender, keeping database and network awaits
/// off the refresh path and reflecting an alias edit without reconnecting.
struct Alert {
    title: String,
    host: crate::store::HostId,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Alerts {
    /// Production uses a bounded no-redirect client. Test-built managers default
    /// offline; only tests that inject a loopback client can send anything.
    pub(crate) fn new(store: HelmStore) -> Self {
        #[cfg(not(test))]
        let client = client_builder()
            .build()
            // Without this line, a machine whose TLS setup cannot build a
            // client (no CA bundle, say) would only ever report the generic
            // "cannot make outbound connections" reason for every alert.
            .map_err(|error| tracing::warn!(%error, "the ntfy HTTP client could not be built"))
            .ok();
        #[cfg(test)]
        let client = None;
        Self::with_client(store, client)
    }

    /// Inject the outbound capability without touching process configuration.
    pub(crate) fn with_client(store: HelmStore, client: Option<reqwest::Client>) -> Self {
        let (queue, mut receiver) = mpsc::channel::<Alert>(QUEUE_CAPACITY);
        let task_store = store.clone();
        let task_client = client.clone();
        let task = tokio::spawn(async move {
            let mut window = RateWindow::new(Instant::now());
            while let Some(alert) = receiver.recv().await {
                let config = match task_store.ntfy_config().await {
                    Ok(config) => config,
                    Err(_) => {
                        tracing::warn!("could not read ntfy alert settings; dropping alert");
                        continue;
                    }
                };
                if !config.settings.enabled || config.settings.topic.is_empty() {
                    continue;
                }
                let Some(dropped) = window.admit(Instant::now()) else {
                    continue;
                };
                let host = match task_store.list_hosts().await {
                    Ok(hosts) => hosts.into_iter().find(|host| host.id == alert.host),
                    Err(_) => {
                        tracing::warn!("could not read ntfy alert host; dropping alert");
                        continue;
                    }
                };
                let Some(host) = host else {
                    continue;
                };
                let name = crate::aggregate::host_display_name(
                    host.kind,
                    host.destination.as_deref(),
                    host.alias.as_deref(),
                );
                let mut body = format!(
                    "{} on {} is waiting for you",
                    alert.title,
                    clean_text(&name)
                );
                if dropped > 0 {
                    body.push_str(&format!(" (and {dropped} more)"));
                }
                if let Err(reason) = post(task_client.as_ref(), &config, body).await {
                    tracing::warn!(%reason, "ntfy alert failed; dropping alert");
                }
            }
        });
        Self(Arc::new(Inner {
            store,
            client,
            queue,
            task,
        }))
    }

    /// A synchronous try-send keeps network and queue waits out of host refresh.
    pub(crate) fn enqueue(&self, title: String, host: crate::store::HostId) {
        if self
            .0
            .queue
            .try_send(Alert {
                title: clean_text(&title),
                host,
            })
            .is_err()
        {
            tracing::warn!("ntfy alert queue unavailable or full; dropping alert");
        }
    }

    /// Load the saved secret only inside the helm. A disabled switch does not
    /// prevent an explicit test, which is how users verify before enabling.
    async fn test(&self) -> Result<(), String> {
        let config = self
            .0
            .store
            .ntfy_config()
            .await
            .map_err(|_| "could not read ntfy alert settings")?;
        post(
            self.0.client.as_ref(),
            &config,
            "Farhelm ntfy alerts are working".into(),
        )
        .await
    }
}

/// No redirects or retries: a configured endpoint cannot forward a token or
/// session title elsewhere, and one failed alert remains one failed attempt.
pub(crate) fn client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .timeout(SEND_TIMEOUT)
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("farhelm/", env!("CARGO_PKG_VERSION")))
}

/// Send exactly one body, ignoring the response body entirely. Errors are
/// classified into fixed reasons; the topic-bearing request URL is stripped
/// even before classification and no source chain is displayed or logged.
async fn post(
    client: Option<&reqwest::Client>,
    config: &Config,
    body: String,
) -> Result<(), String> {
    if config.settings.topic.is_empty() {
        return Err("save a topic before sending a test".into());
    }
    let client = client.ok_or("this helm cannot make outbound connections")?;
    let url = format!("{}/{}", config.settings.server_url, config.settings.topic);
    let mut request = client
        .post(url)
        .header("Title", "Farhelm")
        .header("Priority", config.settings.priority.header())
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(body);
    if let Some(token) = &config.token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(|error| {
        let error = error.without_url();
        if error.is_timeout() {
            "the ntfy server did not answer in time".to_string()
        } else {
            "the ntfy server could not be reached".to_string()
        }
    })?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!(
            "the ntfy server refused it (HTTP {})",
            response.status().as_u16()
        ))
    }
}

/// GET never serializes the stored token, including after a replacement.
pub(crate) async fn get(State(state): State<Arc<AppState>>) -> Response {
    match state.store.ntfy_config().await {
        Ok(config) => axum::Json(config.settings).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not read ntfy alert settings",
        )
            .into_response(),
    }
}

/// Reject malformed JSON with our fixed body: serde/axum's usual diagnostic can
/// quote a submitted string, which is unsafe for a secret-bearing request.
pub(crate) async fn put(
    State(state): State<Arc<AppState>>,
    body: Result<axum::Json<SaveSettings>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(axum::Json(save)) = body else {
        return (StatusCode::BAD_REQUEST, "invalid ntfy alert settings").into_response();
    };
    match state.store.save_ntfy_config(save).await {
        Ok(settings) => axum::Json(settings).into_response(),
        Err(SaveError::Invalid(reason)) => (StatusCode::BAD_REQUEST, reason).into_response(),
        Err(SaveError::Storage) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not save ntfy alert settings",
        )
            .into_response(),
    }
}

/// One user-initiated send waits under the short HTTP timeout, independently of
/// background work. Closing the page does not retract its accepted request.
pub(crate) async fn test(State(state): State<Arc<AppState>>) -> Response {
    let alerts = state.ntfy.clone();
    match crate::run_owned(async move { alerts.test().await }).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(reason) => (StatusCode::BAD_GATEWAY, reason).into_response(),
    }
}

/// Validation and storage refusals stay distinct without retaining secret data
/// in error payloads. The route uses only these fixed messages.
pub(crate) enum SaveError {
    Invalid(&'static str),
    Storage,
}

impl HelmStore {
    /// Read dedicated helm configuration, never the shared preference snapshot.
    pub(crate) async fn ntfy_config(&self) -> anyhow::Result<Config> {
        self.conn()
            .call("ntfy settings read", |conn| {
                let json: String = conn.query_row(
                    "SELECT config FROM ntfy_alerts WHERE singleton = 1",
                    [],
                    |row| row.get(0),
                )?;
                Ok(serde_json::from_str(&json)?)
            })
            .await
    }

    /// Preserve/replace/clear the token inside one database operation so two
    /// simultaneous clients cannot restore an old token from a stale snapshot.
    pub(crate) async fn save_ntfy_config(&self, save: SaveSettings) -> Result<Settings, SaveError> {
        let result = self
            .conn()
            .call("ntfy settings save", move |conn| {
                let previous: String = conn.query_row(
                    "SELECT config FROM ntfy_alerts WHERE singleton = 1",
                    [],
                    |row| row.get(0),
                )?;
                let config = match Config::saved(serde_json::from_str(&previous)?, save) {
                    Ok(config) => config,
                    Err(reason) => return Ok(Err(reason)),
                };
                conn.execute(
                    "UPDATE ntfy_alerts SET config = ?1 WHERE singleton = 1",
                    [serde_json::to_string(&config)?],
                )?;
                Ok(Ok(config.settings))
            })
            .await
            .map_err(|_| SaveError::Storage)?;
        result.map_err(SaveError::Invalid)
    }
}

#[cfg(test)]
#[path = "ntfy_tests.rs"]
mod tests;
