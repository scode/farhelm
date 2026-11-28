//! Helm-owned ntfy alerts: connection-local waiting evidence, one bounded fleet
//! queue, and explicit configuration routes. Neither secrets nor peer titles
//! belong in diagnostics; HTTP responses from ntfy are never read as text.
//!
//! Two waits stand between an event and its alert, and they are separate on
//! purpose. [`SETTLE`] is the waiting policy's: a session must read Waiting for
//! three seconds before its spell counts at all, which filters passing readings.
//! The grace period is the sender's ([`Attention`]): every alert, waiting or
//! blocked, is then held for the user's chosen time and withdrawn if the user
//! attends to that session, or what it reports ends, before it is due.
//!
//! Quiet hours come last, at the moment an alert is sent ([`deliver`]): the local
//! hour then decides its priority, or drops it when quiet hours send none. They
//! are decided before the rate window counts the attempt, so a silenced alert
//! takes no slot from one sent after quiet hours end.

use crate::AppState;
use crate::store::{HelmStore, HostId, SessionKey};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use farhelm_proto::{
    SessionInfo, SessionStatus,
    ntfy::{Priority, SaveSettings, Settings, TestOutcome},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::mpsc, time::Instant};

/// How long a session must read Waiting before its alert is queued, to be held for the grace period. The listing that
/// first reads Waiting starts the spell and sends nothing; the host loop then refreshes again when this is up
/// ([`Policy::next_deadline_after`]), and that listing queues the alert if the session still reads Waiting. The plan
/// `agent-blocked-notifications` shortened it from 10 seconds, when the backstop poll still ran every 3 seconds and
/// supplied the second reading.
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
        // An older client's save omits it and keeps the stored value.
        let grace_period_secs = save
            .grace_period_secs
            .unwrap_or(previous.settings.grace_period_secs);
        if grace_period_secs > farhelm_proto::ntfy::MAX_GRACE_PERIOD_SECS {
            return Err("the grace period must be at most 600 seconds");
        }
        // An older client's save omits them too. Checked whether or not they are on, so that turning them on later
        // never meets a range saved broken.
        let quiet_hours = save.quiet_hours.unwrap_or(previous.settings.quiet_hours);
        if quiet_hours.start_hour > 23 || quiet_hours.end_hour > 23 {
            return Err("quiet hours must start and end on an hour from 00:00 to 23:00");
        }
        if quiet_hours.start_hour == quiet_hours.end_hour {
            return Err("quiet hours must start and end at different hours");
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
                grace_period_secs,
                quiet_hours,
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

/// A waiting spell that settled in this observation: the session's bare id, its cleaned title, and when the spell
/// started (the first Waiting reading), which [`Alerts::enqueue`] needs so that the user's attention to the session
/// after that moment, even within the settling time, withdraws the alert.
#[derive(Debug)]
pub(crate) struct Settled {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) since: Instant,
}

/// Lets the policy's tests compare what settled by title alone, as they did when [`Policy::observe`] returned titles.
#[cfg(test)]
impl PartialEq<&str> for Settled {
    fn eq(&self, title: &&str) -> bool {
        self.title == *title
    }
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
    /// The sessions whose spells ended since the last [`Policy::take_ended`]: they left Waiting, left the listing, or
    /// alerts were turned off. An alert held for one of them is withdrawn ([`Alerts::cancel_waiting`]).
    ended: Vec<String>,
}

impl Policy {
    /// Consume one accepted listing at an injected monotonic time, with
    /// `enabled` saying whether waiting alerts would be sent now
    /// ([`Settings::sends_waiting_alerts`], read from the store's cache so the
    /// refresh path never waits on the database). Returned spells have settled
    /// once; queue failures must not turn into later retries.
    pub(crate) fn observe(
        &mut self,
        list: &[Observation],
        now: Instant,
        enabled: bool,
    ) -> Vec<Settled> {
        if !enabled {
            if self.initialized {
                tracing::debug!("ntfy waiting alerts are off; dropping waiting evidence");
            }
            self.ended.extend(self.spells.drain().map(|(id, _)| id));
            self.initialized = false;
            return Vec::new();
        }
        let ids: HashSet<_> = list.iter().map(|s| s.id.as_str()).collect();
        let ended = &mut self.ended;
        self.spells.retain(|id, _| {
            let listed = ids.contains(id.as_str());
            if !listed {
                ended.push(id.clone());
            }
            listed
        });
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
                        alerts.push(Settled {
                            id: session.id.clone(),
                            title: session.title.clone(),
                            since: *since,
                        });
                        *spell = Spell::Handled;
                    }
                }
                _ => {
                    if self.spells.remove(&session.id).is_some() {
                        self.ended.push(session.id.clone());
                    }
                }
            }
        }
        self.initialized = true;
        alerts
    }

    /// The sessions whose waiting spells ended since the last call, for the caller to withdraw their held alerts.
    pub(crate) fn take_ended(&mut self) -> Vec<String> {
        std::mem::take(&mut self.ended)
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
/// Handles own the task, which holds only the store/client/receiver and the
/// shared [`Attention`]: no Arc cycle keeps either an embedded helm or a test
/// fixture alive after shutdown.
struct Inner {
    store: HelmStore,
    client: Option<reqwest::Client>,
    queue: mpsc::Sender<Alert>,
    /// The alerts the sender holds through the grace period, and the awareness that withdraws them. Shared outside
    /// the queue, so awareness and cancellation never wait behind queued alerts or a full queue.
    attention: Arc<Mutex<Attention>>,
    /// Where quiet hours read the local hour, shared with the sender task.
    clock: LocalClock,
    /// How many queued alerts the sender has taken and decided (sent, dropped or held), for tests that must know an
    /// alert has reached the grace period's state before they act on it. Counted once the decision is carried out, so
    /// an alert sent at once has been sent, or dropped by quiet hours, by the time it counts.
    #[cfg(test)]
    taken: tokio::sync::watch::Receiver<u64>,
    task: tokio::task::JoinHandle<()>,
}
/// The host name is resolved by the sender, keeping database and network awaits
/// off the refresh path and reflecting an alias edit without reconnecting.
struct Alert {
    /// The session the alert is about; its host is the alert's host.
    key: SessionKey,
    title: String,
    kind: AlertKind,
    /// When what the alert reports began: the waiting spell's first Waiting reading, or the agent's report. Attention
    /// to the session at or after this moment withdraws the alert ([`Attention::admit`]).
    started_at: Instant,
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

/// [`AlertKind`] without its payload: what a held alert is filed under beside its session, so one kind can be
/// withdrawn without the other (a session leaving Waiting does not withdraw its blocked alert).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Waiting,
    Blocked,
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

    fn kind(&self) -> Kind {
        match self {
            Self::Waiting => Kind::Waiting,
            Self::Blocked { .. } => Kind::Blocked,
        }
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Where the sender and Send test read the local hour (0 to 23) that quiet hours compare against: the C library's
/// local time ([`crate::local_time::hour_now`]), except in this crate's unit tests, where it is a value the test sets
/// ([`Alerts::set_local_hour_for_test`], noon until then), so that no test depends on the machine's zone or the time it
/// runs.
#[derive(Clone, Default)]
struct LocalClock {
    #[cfg(test)]
    hour: Arc<std::sync::atomic::AtomicU8>,
}

impl LocalClock {
    /// The local hour now.
    fn hour(&self) -> u8 {
        #[cfg(test)]
        {
            self.hour.load(std::sync::atomic::Ordering::Relaxed)
        }
        #[cfg(not(test))]
        {
            crate::local_time::hour_now()
        }
    }

    /// A clock that reads noon until a test sets it.
    #[cfg(test)]
    fn new() -> Self {
        Self {
            hour: Arc::new(std::sync::atomic::AtomicU8::new(12)),
        }
    }

    #[cfg(not(test))]
    fn new() -> Self {
        Self::default()
    }
}

/// How long a withdrawal ([`Attention`]'s attention and cancellation records) is remembered. A record drops an alert
/// whose spell or report began at or before it and that reaches the sender after it; an alert normally reaches the
/// sender within the settling time and the queue's latency of its start, and a spell that stays pending longer
/// (through Unknown readings) is the one case where an older record could still matter. An hour is far past any of
/// those. Records older than this are dropped whenever the records outgrow [`Attention`]'s moving prune threshold.
const RECORD_HORIZON: Duration = Duration::from_secs(60 * 60);
/// The floor of [`Attention`]'s prune threshold: no prune happens until there are more records than this, and after a
/// prune the threshold is twice what is left, but never lower. Each record is a distinct session (or host and kind),
/// so the records are bounded by about twice those seen within [`RECORD_HORIZON`]; pruning keeps a long-running helm
/// from keeping one for every session it ever saw.
const RECORDS_PRUNE_AT: usize = 256;

/// The grace period's state, shared between the sender task and every caller that reports attention or withdraws an
/// alert (SPEC.md, "Ntfy alerts"): the alerts held until they are due, and when each session's alerts were last
/// withdrawn.
///
/// A pure state machine over injected times, so its rules are tested without a clock or a server; [`Alerts`] reads
/// the clock for it. The rules:
///
/// - **Admit** ([`Self::admit`]): an alert withdrawn at or after its start is dropped at once; with a grace period of
///   zero it goes out now; otherwise it is held until its grace period has passed.
/// - **Withdraw**: the user attending to a session ([`Self::note_aware`]) withdraws both its kinds of alert; the
///   session leaving Waiting withdraws its waiting alert and its blocked mark being cleared its blocked alert
///   ([`Self::cancel`]); a host's connection ending withdraws its waiting alerts, and its entry being removed every
///   alert of that host ([`Self::cancel_host`]). A blocked alert outlives a dropped connection: its mark is durable,
///   and when the reconnected refresh finds the session over it clears the mark, which withdraws the alert.
///
/// A withdrawal removes what is held and is also remembered with its time, because the alert it is meant for may still
/// be in the queue, not yet admitted: attention during the waiting spell's 3-second settling time comes before the
/// alert is even queued, and a session can leave Waiting between its alert being queued and the sender taking it.
/// Comparing a record with the alert's start keeps a withdrawal from reaching a later spell or report.
struct Attention {
    held: HashMap<(SessionKey, Kind), Held>,
    /// When the user last attended to each session; withdraws both kinds.
    last_aware: HashMap<SessionKey, Instant>,
    /// When each session's alert of one kind was last withdrawn by its spell ending or its mark clearing.
    withdrawn: HashMap<(SessionKey, Kind), Instant>,
    /// When each host's alerts of one kind were last withdrawn: waiting by its connection ending, both by its entry
    /// being removed.
    hosts_withdrawn: HashMap<(HostId, Kind), Instant>,
    /// How many records may build up before [`Self::prune`] scans them: [`RECORDS_PRUNE_AT`] at first, then twice
    /// what the previous prune left, so a fleet with many fresh records does not scan them on every keystroke.
    prune_at: usize,
}

impl Default for Attention {
    fn default() -> Self {
        Self {
            held: HashMap::new(),
            last_aware: HashMap::new(),
            withdrawn: HashMap::new(),
            hosts_withdrawn: HashMap::new(),
            prune_at: RECORDS_PRUNE_AT,
        }
    }
}

/// One alert held through its grace period, due to go out at `due`.
struct Held {
    alert: Alert,
    due: Instant,
}

impl Attention {
    /// Decide what happens to `alert`, arriving at `now` under a grace period of `grace`: `Some` to send it now, or
    /// `None` when it was dropped (withdrawn since it began) or is now held. A second alert of the same kind for a
    /// session already holding one replaces its content and keeps the earlier due time, so a repeated report neither
    /// sends twice nor pushes the alert later; under a zero grace period it goes out now and takes the held one's
    /// place.
    fn admit(&mut self, alert: Alert, now: Instant, grace: Duration) -> Option<Alert> {
        if self.withdrawn_since_start(&alert) {
            tracing::debug!("the alert was withdrawn since it began; dropping it");
            return None;
        }
        let slot = (alert.key.clone(), alert.kind.kind());
        if grace.is_zero() {
            // One held from before the grace period was turned off would repeat this one.
            self.held.remove(&slot);
            return Some(alert);
        }
        let due = self
            .held
            .get(&slot)
            .map_or(now + grace, |held| held.due.min(now + grace));
        self.held.insert(slot, Held { alert, due });
        None
    }

    /// Whether any record withdraws `alert`: one at or after the moment it began. At, not after, so the user switching
    /// to a session in the same instant its spell started still counts as having seen it.
    fn withdrawn_since_start(&self, alert: &Alert) -> bool {
        let records = [
            self.last_aware.get(&alert.key),
            self.withdrawn.get(&(alert.key.clone(), alert.kind.kind())),
            self.hosts_withdrawn
                .get(&(alert.key.host, alert.kind.kind())),
        ];
        records
            .into_iter()
            .flatten()
            .any(|at| *at >= alert.started_at)
    }

    /// The user attended to `key`'s session at `at`: withdraw both its kinds of alert that began by then. `at` can be
    /// in the past (a delayed signal), so an alert held since begun stays, and an earlier record never replaces a later
    /// one.
    fn note_aware(&mut self, key: &SessionKey, at: Instant) {
        self.held
            .retain(|(held, _), entry| held != key || entry.alert.started_at > at);
        let record = self.last_aware.entry(key.clone()).or_insert(at);
        *record = (*record).max(at);
        self.prune(at);
    }

    /// Withdraw `key`'s alert of `kind` at `now`.
    fn cancel(&mut self, key: &SessionKey, kind: Kind, now: Instant) {
        self.held.remove(&(key.clone(), kind));
        self.withdrawn.insert((key.clone(), kind), now);
        self.prune(now);
    }

    /// Withdraw the alerts of each kind in `kinds` for every session of `host`, at `now`.
    fn cancel_host(&mut self, host: HostId, kinds: &[Kind], now: Instant) {
        self.held
            .retain(|(key, kind), _| key.host != host || !kinds.contains(kind));
        for kind in kinds {
            self.hosts_withdrawn.insert((host, *kind), now);
        }
        self.prune(now);
    }

    /// Drop records older than [`RECORD_HORIZON`] once there are more than [`Self::prune_at`] of them, and move that
    /// threshold to twice what is left (never below [`RECORDS_PRUNE_AT`]), so a prune that frees little is not
    /// repeated on the next record.
    fn prune(&mut self, now: Instant) {
        if self.records() <= self.prune_at {
            return;
        }
        let fresh = |at: &Instant| now.saturating_duration_since(*at) < RECORD_HORIZON;
        self.last_aware.retain(|_, at| fresh(at));
        self.withdrawn.retain(|_, at| fresh(at));
        self.hosts_withdrawn.retain(|_, at| fresh(at));
        self.prune_at = RECORDS_PRUNE_AT.max(self.records() * 2);
    }

    /// How many withdrawal records are kept.
    fn records(&self) -> usize {
        self.last_aware.len() + self.withdrawn.len() + self.hosts_withdrawn.len()
    }

    /// When the earliest held alert is due, if any is held.
    fn next_due(&self) -> Option<Instant> {
        self.held.values().map(|held| held.due).min()
    }

    /// Take every held alert due at `now`, earliest first.
    fn take_due(&mut self, now: Instant) -> Vec<Alert> {
        let due: Vec<(SessionKey, Kind)> = self
            .held
            .iter()
            .filter(|(_, held)| held.due <= now)
            .map(|(slot, _)| slot.clone())
            .collect();
        let mut ready: Vec<Held> = due
            .into_iter()
            .filter_map(|slot| self.held.remove(&slot))
            .collect();
        ready.sort_by_key(|held| held.due);
        ready.into_iter().map(|held| held.alert).collect()
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
    ///
    /// The one sender task does all of an alert's waiting: for an alert to arrive through the queue, or for the
    /// earliest held one to fall due, whichever comes first. Held alerts are only ever added by this task, so the time
    /// it sleeps until can only move later behind its back (an alert withdrawn meanwhile), never earlier; a wake for an
    /// alert no longer held finds nothing due and sleeps again.
    pub(crate) fn with_client(store: HelmStore, client: Option<reqwest::Client>) -> Self {
        let (queue, mut receiver) = mpsc::channel::<Alert>(QUEUE_CAPACITY);
        let clock = LocalClock::new();
        let task_clock = clock.clone();
        let attention = Arc::new(Mutex::new(Attention::default()));
        let task_attention = Arc::clone(&attention);
        let task_store = store.clone();
        let task_client = client.clone();
        #[cfg(test)]
        let (taken_tx, taken) = tokio::sync::watch::channel(0_u64);
        let task = tokio::spawn(async move {
            let mut window = RateWindow::new(Instant::now());
            loop {
                let due = task_attention
                    .lock()
                    .expect("ntfy attention poisoned")
                    .next_due();
                // Whether this turn took an alert from the queue, which the test counter counts once it is decided.
                let (ready, from_queue) = tokio::select! {
                    received = receiver.recv() => {
                        let Some(alert) = received else {
                            break;
                        };
                        (admit(alert, &task_store, &task_attention).await, true)
                    }
                    () = sleep_until_due(due) => {
                        let ready = task_attention
                            .lock()
                            .expect("ntfy attention poisoned")
                            .take_due(Instant::now());
                        if ready.is_empty() {
                            continue;
                        }
                        // Read again at the due time: the switches may have changed while the alert was held.
                        let Some(config) = read_config(&task_store).await else {
                            continue;
                        };
                        (Some((ready, config)), false)
                    }
                };
                if let Some((ready, config)) = ready {
                    for alert in ready {
                        if alert.kind.sends(&config.settings) {
                            deliver(
                                alert,
                                &config,
                                &task_store,
                                task_client.as_ref(),
                                &mut window,
                                task_clock.hour(),
                            )
                            .await;
                        } else {
                            tracing::debug!(
                                "this kind of ntfy alert was turned off while it was held; dropping it"
                            );
                        }
                    }
                }
                #[cfg(test)]
                if from_queue {
                    taken_tx.send_modify(|taken| *taken += 1);
                }
                #[cfg(not(test))]
                let _ = from_queue;
            }
        });
        Self(Arc::new(Inner {
            store,
            client,
            queue,
            attention,
            clock,
            #[cfg(test)]
            taken,
            task,
        }))
    }

    /// Queue the waiting alert for `settled`, the spell [`Policy::observe`] settled for a session of `host`. A
    /// synchronous try-send keeps network and queue waits out of host refresh.
    pub(crate) fn enqueue(&self, host: HostId, settled: Settled) {
        self.send(Alert {
            key: SessionKey::new(host, settled.id),
            title: clean_text(&settled.title),
            kind: AlertKind::Waiting,
            started_at: settled.since,
        });
    }

    /// Queue an alert that the agent in `key`'s session, titled `title`, says it is blocked, for `reason` (already
    /// cleaned and capped by the caller). `reported` is its start, read before the session was marked: from the
    /// moment the mark can reach a client, a switch to the session, or the mark's clear, must withdraw the alert. The
    /// sender drops it unless blocked alerts are on.
    pub(crate) fn enqueue_blocked(
        &self,
        key: SessionKey,
        title: String,
        reason: String,
        reported: Instant,
    ) {
        self.send(Alert {
            key,
            title: clean_text(&title),
            kind: AlertKind::Blocked { reason },
            started_at: reported,
        });
    }

    /// Note that the user has just shown they are attending to `key`'s session: they switched to it, sent input to
    /// its terminal, or interacted with it while it was open in an active window (`crate::attention`). It is the one
    /// place that per-session evidence reaches the alerts; window focus and activity elsewhere never do (SPEC.md,
    /// "Ntfy alerts"). It withdraws the session's alerts, held or still in the queue, for anything that began up to
    /// now.
    ///
    /// Called on every counted terminal input frame, so it never waits: it takes a lock, touches two maps, and now and
    /// then prunes the withdrawal records, a scan of all of them that the threshold in [`Attention::prune`] keeps
    /// rare.
    pub(crate) fn note_attention(&self, key: &SessionKey) {
        self.note_attention_at(key, Instant::now());
    }

    /// [`Self::note_attention`] for attention that happened at `at`, before now: a delayed signal from the desktop
    /// app, which reports its age (`farhelm_proto::SessionAttentionRequest::age_ms`). It withdraws only what began up
    /// to `at`, so an alert that began between the interaction and the signal's arrival stays.
    pub(crate) fn note_attention_at(&self, key: &SessionKey, at: Instant) {
        self.attention().note_aware(key, at);
    }

    /// Withdraw `key`'s waiting alert, held or still queued: its session left Waiting, or left its host's listing.
    pub(crate) fn cancel_waiting(&self, key: &SessionKey) {
        self.attention().cancel(key, Kind::Waiting, Instant::now());
    }

    /// Withdraw the blocked alerts, held or still queued, of `keys`, whose blocked marks were just cleared
    /// (`attention::cleared_marks`).
    pub(crate) fn cancel_blocked(&self, keys: &[SessionKey]) {
        let now = Instant::now();
        let mut attention = self.attention();
        for key in keys {
            attention.cancel(key, Kind::Blocked, now);
        }
    }

    /// Withdraw the waiting alerts, held or still queued, of `host`'s sessions: its connection ended, and the waiting
    /// policy that would have ended their spells went with it. Blocked alerts stay: a blocked mark is durable, and the
    /// reconnected refresh clears it (withdrawing its alert) only if the session is really over. Its next
    /// connection's alerts start after this and are unaffected.
    pub(crate) fn cancel_host_waiting(&self, host: HostId) {
        self.attention()
            .cancel_host(host, &[Kind::Waiting], Instant::now());
    }

    /// Withdraw every alert, held or still queued, of `host`'s sessions: its entry was removed, and its sessions and
    /// marks with it.
    pub(crate) fn cancel_host(&self, host: HostId) {
        self.attention()
            .cancel_host(host, &[Kind::Waiting, Kind::Blocked], Instant::now());
    }

    fn attention(&self) -> std::sync::MutexGuard<'_, Attention> {
        self.0.attention.lock().expect("ntfy attention poisoned")
    }

    /// Try-send one alert, dropping it with a warning when the queue is full or gone.
    fn send(&self, alert: Alert) {
        if self.0.queue.try_send(alert).is_err() {
            tracing::warn!("ntfy alert queue unavailable or full; dropping alert");
        }
    }

    /// Load the saved secret only inside the helm. A disabled switch does not
    /// prevent an explicit test, which is how users verify before enabling.
    ///
    /// The test goes out at the priority alerts would have now, inside quiet
    /// hours too, and says which ([`Settings::test_outcome`]); quiet hours that
    /// silence alerts do not silence it, so the connection can be checked then.
    async fn test(&self) -> Result<TestOutcome, String> {
        let config = self
            .0
            .store
            .ntfy_config()
            .await
            .map_err(|_| "could not read ntfy alert settings")?;
        let outcome = config.settings.test_outcome(self.0.clock.hour());
        post(
            self.0.client.as_ref(),
            &config,
            outcome.priority,
            "Farhelm ntfy alerts are working".into(),
        )
        .await?;
        Ok(outcome)
    }

    /// Make the local hour quiet hours read `hour` from now on, for this sender and its Send test.
    #[cfg(test)]
    pub(crate) fn set_local_hour_for_test(&self, hour: u8) {
        self.0
            .clock
            .hour
            .store(hour, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Decide what happens to an alert the sender just took from the queue, under the settings stored now: `Some` with
/// those settings to send it at once, or `None` when it was dropped or is now held ([`Attention::admit`]).
async fn admit(
    alert: Alert,
    store: &HelmStore,
    attention: &Mutex<Attention>,
) -> Option<(Vec<Alert>, Config)> {
    let config = read_config(store).await?;
    // The policy stops tracking spells once alerts are off, but an alert queued just before a save turned them off
    // still lands here, and a blocked alert is queued whatever the switches say.
    if !alert.kind.sends(&config.settings) {
        tracing::debug!("this kind of ntfy alert is off; dropping a queued alert");
        return None;
    }
    let grace = Duration::from_secs(u64::from(config.settings.grace_period_secs));
    let admitted =
        attention
            .lock()
            .expect("ntfy attention poisoned")
            .admit(alert, Instant::now(), grace);
    admitted.map(|alert| (vec![alert], config))
}

/// Sleep until `due`, or forever when nothing is held, for the sender task's `select!`.
async fn sleep_until_due(due: Option<Instant>) {
    match due {
        Some(due) => tokio::time::sleep_until(due).await,
        None => std::future::pending().await,
    }
}

/// The stored settings, or `None` with a warning when they cannot be read, in which case the alert at hand is dropped.
async fn read_config(store: &HelmStore) -> Option<Config> {
    match store.ntfy_config().await {
        Ok(config) => Some(config),
        Err(_) => {
            tracing::warn!("could not read ntfy alert settings; dropping alert");
            None
        }
    }
}

/// Send one alert under the fleet-wide rate window, naming its host as the registry names it now, at the priority
/// quiet hours give it at `local_hour`. An alert that quiet hours silence is dropped before the rate window counts it,
/// so it neither takes a slot nor adds to the "and N more" count, which is about the rate cap alone.
async fn deliver(
    alert: Alert,
    config: &Config,
    store: &HelmStore,
    client: Option<&reqwest::Client>,
    window: &mut RateWindow,
    local_hour: u8,
) {
    let Some(priority) = config.settings.effective_priority(local_hour) else {
        tracing::debug!("quiet hours send no ntfy alerts now; dropping alert");
        return;
    };
    let Some(dropped) = window.admit(Instant::now()) else {
        return;
    };
    let host = match store.list_hosts().await {
        Ok(hosts) => hosts.into_iter().find(|host| host.id == alert.key.host),
        Err(_) => {
            tracing::warn!("could not read ntfy alert host; dropping alert");
            return;
        }
    };
    let Some(host) = host else {
        tracing::debug!("ntfy alert's host is gone; dropping alert");
        return;
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
    match post(client, config, priority, body).await {
        Ok(()) => tracing::info!("ntfy alert sent"),
        Err(reason) => tracing::warn!(%reason, "ntfy alert failed; dropping alert"),
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

/// Send exactly one body at `priority`, ignoring the response body entirely. Errors are
/// classified into fixed reasons; the topic-bearing request URL is stripped
/// even before classification and no source chain is displayed or logged.
async fn post(
    client: Option<&reqwest::Client>,
    config: &Config,
    priority: Priority,
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
        .header("Priority", priority.header())
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
/// A sent test answers with what it did ([`TestOutcome`]), so the form can say
/// which priority it went at and whether quiet hours decided that.
pub(crate) async fn test(State(state): State<Arc<AppState>>) -> Response {
    match state.ntfy.test().await {
        Ok(outcome) => axum::Json(outcome).into_response(),
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
