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

/// How long a session must read Waiting before its alert goes out. The listing that first reads Waiting starts the
/// spell and sends nothing; the host loop then refreshes again when this is up ([`Policy::next_deadline_after`]), and
/// that listing sends the alert if the session still reads Waiting. The plan `agent-blocked-notifications` shortened it
/// from 10 seconds, when the backstop poll still ran every 3 seconds and supplied the second reading.
pub(crate) const SETTLE: Duration = Duration::from_secs(3);
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
    /// Preserve an omitted secret only for the same server origin. Resolve this
    /// against stored settings inside the save operation: a stale form must not
    /// move a stored token to a new server. Refusals use fixed messages
    /// so a bad URL, topic or token cannot escape in an error string.
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
        let same_origin = reqwest::Url::parse(&previous.settings.server_url)
            .is_ok_and(|previous| previous.origin() == server.origin());
        let token = match save.token {
            None if same_origin => previous.token,
            None => None,
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
                // An older client's save omits the switch and keeps it as stored.
                notify_waiting: save
                    .notify_waiting
                    .unwrap_or(previous.settings.notify_waiting),
                notify_blocked: save
                    .notify_blocked
                    .unwrap_or(previous.settings.notify_blocked),
                custom_blocked_criteria: match save.custom_blocked_criteria {
                    // An older client's save omits it and keeps the stored choice.
                    None => previous.settings.custom_blocked_criteria,
                    Some(text) => farhelm_proto::ntfy::normalize_blocked_criteria(&text)?,
                },
                // An older client's save omits it too, keeping the stored choice, or its absence, which keeps reading
                // as the ntfy switches did before the choice existed.
                ask_agents_when_blocked: save
                    .ask_agents_when_blocked
                    .or(previous.settings.ask_agents_when_blocked),
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

impl Observation {
    /// Whether the listing reported this session ended. The host refresh reads it to clear the blocked mark of an
    /// agent that exited (`manager`'s `clear_finished_blocks`), from the same fresh readings the alerts take.
    pub(crate) fn has_ended(&self) -> bool {
        self.status.has_ended()
    }
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
///
/// While waiting alerts are off the policy keeps no evidence at all, and the
/// first listing after they come back on is a fresh baseline: a session already
/// waiting then alerts only for its next wait, the same rule as after a
/// reconnect. Spells used to keep settling while alerts were off, each marked
/// handled while the sender dropped its alert; keeping nothing makes "off" mean
/// the policy does no work and holds no state that outlives the switch.
#[derive(Default)]
pub(crate) struct Policy {
    initialized: bool,
    spells: HashMap<String, Spell>,
}

impl Policy {
    /// Consume one accepted listing at an injected monotonic time, with
    /// `enabled` saying whether waiting alerts would be sent now
    /// ([`Settings::sends_waiting_alerts`], read from the store's cache so the
    /// refresh path never waits on the database). Returned titles have settled
    /// once; queue failures must not turn into later retries.
    pub(crate) fn observe(
        &mut self,
        list: &[Observation],
        now: Instant,
        enabled: bool,
    ) -> Vec<String> {
        if !enabled {
            if self.initialized {
                tracing::debug!("ntfy waiting alerts are off; dropping waiting evidence");
            }
            self.spells.clear();
            self.initialized = false;
            return Vec::new();
        }
        let ids: HashSet<_> = list.iter().map(|s| s.id.as_str()).collect();
        self.spells.retain(|id, _| ids.contains(id.as_str()));
        let mut alerts = Vec::new();
        for session in list {
            match session.status {
                SessionStatus::Unknown => {}
                SessionStatus::Waiting => {
                    let spell = self.spells.entry(session.id.clone()).or_insert_with(|| {
                        if self.initialized {
                            tracing::debug!(session = %session.id, "waiting spell started");
                            Spell::Pending(now)
                        } else {
                            tracing::debug!(
                                session = %session.id,
                                "session already waiting at the baseline; no alert for this wait"
                            );
                            Spell::Handled
                        }
                    });
                    if let Spell::Pending(since) = spell
                        && now.saturating_duration_since(*since) >= SETTLE
                    {
                        tracing::info!(session = %session.id, "waiting spell settled; queueing an ntfy alert");
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

    /// When the earliest pending waiting spell becomes due to alert, or
    /// `None` with nothing pending.
    ///
    /// The policy decides only when a listing is observed, and a session that
    /// stays waiting sends no further change hint, so without a refresh at
    /// this moment the alert would wait for the next backstop poll, which is
    /// thirty seconds apart (`manager::REFRESH_INTERVAL`) and would break
    /// SPEC.md's three seconds. The host loop sleeps until this deadline and
    /// then refreshes like the poll does; the refresh's observation is still
    /// what sends the alert.
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.pending_deadlines().min()
    }

    /// The earliest pending deadline strictly after `after`, skipping overdue
    /// ones.
    ///
    /// This is what the host loop arms its wake on, with `after` the start of
    /// the refresh it just ran. An overdue deadline cannot be armed (it would
    /// fire at once and spin a failing host), and it must not hide a later
    /// one either: a spell whose session went on to read `Unknown` stays
    /// pending past its deadline, because `Unknown` adds no evidence, and
    /// taking the minimum before skipping it would leave every other
    /// session's alert waiting for the backstop poll.
    pub(crate) fn next_deadline_after(&self, after: Instant) -> Option<Instant> {
        self.pending_deadlines()
            .filter(|deadline| *deadline > after)
            .min()
    }

    fn pending_deadlines(&self) -> impl Iterator<Item = Instant> + '_ {
        self.spells.values().filter_map(|spell| match spell {
            Spell::Pending(since) => Some(*since + SETTLE),
            Spell::Handled => None,
        })
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
    kind: AlertKind,
}

/// Which event an alert reports. Each has its own switch under the master one,
/// and both share the queue, the rate window and the sender.
enum AlertKind {
    /// A session settled into waiting for the user ([`Policy`]).
    Waiting,
    /// An agent said it is blocked (`farhelm agent notify`), with its reason,
    /// already cleaned and capped by the helm.
    Blocked { reason: String },
}

impl AlertKind {
    /// Whether `settings` send this kind of alert now.
    fn sends(&self, settings: &Settings) -> bool {
        match self {
            Self::Waiting => settings.sends_waiting_alerts(),
            Self::Blocked { .. } => settings.sends_blocked_alerts(),
        }
    }

    /// The alert's text for a session titled `title` on the host named `host`.
    fn body(&self, title: &str, host: &str) -> String {
        match self {
            Self::Waiting => format!("{title} on {host} is waiting for you"),
            Self::Blocked { reason } => format!("{title} on {host} is blocked: {reason}"),
        }
    }
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
                // The policy stops tracking spells once alerts are off, but an
                // alert queued just before a save turned them off still lands
                // here, and a blocked alert is queued whatever the switches say.
                if !alert.kind.sends(&config.settings) {
                    tracing::debug!("this kind of ntfy alert is off; dropping a queued alert");
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
                    tracing::debug!("ntfy alert's host is gone; dropping alert");
                    continue;
                };
                let name = crate::aggregate::host_display_name(
                    host.kind,
                    host.destination.as_deref(),
                    host.alias.as_deref(),
                );
                let mut body = alert.kind.body(&alert.title, &clean_text(&name));
                if dropped > 0 {
                    body.push_str(&format!(" (and {dropped} more)"));
                }
                match post(task_client.as_ref(), &config, body).await {
                    Ok(()) => tracing::info!("ntfy alert sent"),
                    Err(reason) => tracing::warn!(%reason, "ntfy alert failed; dropping alert"),
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
        self.send(Alert {
            title: clean_text(&title),
            host,
            kind: AlertKind::Waiting,
        });
    }

    /// Queue an alert that the agent in a session titled `title` on `host`
    /// says it is blocked, for `reason` (already cleaned and capped by the
    /// caller). The sender drops it unless blocked alerts are on.
    pub(crate) fn enqueue_blocked(
        &self,
        title: String,
        host: crate::store::HostId,
        reason: String,
    ) {
        self.send(Alert {
            title: clean_text(&title),
            host,
            kind: AlertKind::Blocked { reason },
        });
    }

    /// Note that the user has just shown they are attending to `key`'s session: they switched to it, sent input to
    /// its terminal, or interacted with it while it was open in an active window (`crate::attention`). It is the one
    /// place that per-session evidence reaches the alerts; window focus and activity elsewhere never do (SPEC.md,
    /// "Ntfy alerts").
    ///
    /// The alerts do not act on it yet: every alert still goes out as soon as its own rules allow, so this records
    /// nothing. Its callers run it on every terminal input frame, so whatever it comes to do must stay cheap and must
    /// not wait.
    pub(crate) fn note_attention(&self, _key: &crate::store::SessionKey) {}

    /// Try-send one alert, dropping it with a warning when the queue is full or gone.
    fn send(&self, alert: Alert) {
        if self.0.queue.try_send(alert).is_err() {
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
    match state.store.save_ntfy_config_noting_switch_on(save).await {
        Ok((settings, switched_on)) => {
            // The first listing after waiting alerts come on is each host's
            // fresh baseline, so take it now rather than at the next backstop
            // poll: a session that started waiting in between would be
            // baselined and miss its alert.
            if switched_on {
                state.manager.refresh_all_now();
            }
            axum::Json(settings).into_response()
        }
        Err(SaveError::Invalid(reason)) => (StatusCode::BAD_REQUEST, reason).into_response(),
        Err(SaveError::Storage) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not save ntfy alert settings",
        )
            .into_response(),
    }
}

/// One user-initiated send waits under the short HTTP timeout, independently of
/// background work. A client going away does not retract its accepted request.
/// The authenticated router owns the saved-settings read and the send together.
pub(crate) async fn test(State(state): State<Arc<AppState>>) -> Response {
    match state.ntfy.test().await {
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

/// Whether the stored configuration sends waiting alerts, read straight from a
/// connection inside a [`HelmStore`] open's blocking task, to seed its cache. A
/// database without the table yet (an older schema an offline open accepts)
/// sends none; any other failure is an error for [`seed_waiting_alerts`].
pub(crate) fn sends_waiting_alerts_in(conn: &rusqlite::Connection) -> anyhow::Result<bool> {
    let table: bool = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'ntfy_alerts'",
        [],
        |row| row.get(0),
    )?;
    if !table {
        return Ok(false);
    }
    let json: String = conn.query_row(
        "SELECT config FROM ntfy_alerts WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    Ok(serde_json::from_str::<Config>(&json)?
        .settings
        .sends_waiting_alerts())
}

/// [`sends_waiting_alerts_in`] for every store open: a failure is logged and
/// counts as off. A row this build cannot read (one a newer build wrote, say)
/// costs the waiting alerts, as it always cost every ntfy alert, and must not
/// stop the helm from starting; saving the settings again replaces it.
pub(crate) fn seed_waiting_alerts(conn: &rusqlite::Connection) -> bool {
    sends_waiting_alerts_in(conn).unwrap_or_else(|error| {
        tracing::warn!(%error, "could not read whether ntfy waiting alerts are on; treating them as off");
        false
    })
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

    /// Bind token preservation to the stored server inside one database operation.
    /// A stale client supplies no secret, so its origin change must clear the
    /// current token even if that token was saved after the form loaded.
    ///
    /// The store's cache of whether waiting alerts go out
    /// ([`HelmStore::waiting_alerts_enabled`]) is updated inside the same
    /// database call, so two saves racing each other leave it agreeing with
    /// whichever row the database kept.
    pub(crate) async fn save_ntfy_config(&self, save: SaveSettings) -> Result<Settings, SaveError> {
        self.save_ntfy_config_noting_switch_on(save)
            .await
            .map(|(settings, _)| settings)
    }

    /// [`Self::save_ntfy_config`], also saying whether this save is the one
    /// that turned waiting alerts on. The answer comes from swapping the cache
    /// inside the database call, where saves are serialized, so of two racing
    /// saves exactly the one the database applied last over an "off" sees the
    /// switch; a read of the cache before the save could miss an off-then-on
    /// pair and skip the refresh that takes the new baseline.
    pub(crate) async fn save_ntfy_config_noting_switch_on(
        &self,
        save: SaveSettings,
    ) -> Result<(Settings, bool), SaveError> {
        let waiting = self.ntfy_waiting_cache();
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
                let now_on = config.settings.sends_waiting_alerts();
                let was_on = waiting.swap(now_on, std::sync::atomic::Ordering::Relaxed);
                Ok(Ok((config.settings, now_on && !was_on)))
            })
            .await
            .map_err(|_| SaveError::Storage)?;
        result.map_err(SaveError::Invalid)
    }
}

#[cfg(test)]
#[path = "ntfy_tests.rs"]
mod tests;
