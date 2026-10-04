//! The helm asks the user before it carries out an acting `farhelm` command
//! from inside a session (SPEC.md, Agent-spawned sessions).
//!
//! # Where the decision lives, and why there
//!
//! Every acting agent verb reaches the helm through the agent relay
//! (`agent_requests`), and the helm is the one party the threat model lets
//! decide: the requesting host and its supervisor are untrusted, so the
//! per-host setting, the GUI check and the wait for the user's answer all
//! live here, keyed by the host connection the request arrived on
//! ([`crate::agent_requests::AgentOrigin`]) and never by anything the request
//! claims. The verb handlers call [`ask`] after they have resolved what they
//! are about to do and before they do any of it, so the card shows exactly
//! what an approval lets happen.
//!
//! # What a pending approval is
//!
//! A request waiting for the user is an entry in an in-memory table
//! ([`Approvals`]) holding what the card shows and the sender half of a
//! oneshot the verb handler is parked on. Nothing is persisted: a helm
//! restart drops the table, and the parked handlers go with the process, so
//! the asking CLI gets the relay's ordinary "helm went away" ending. The GUI
//! reads the table through `GET /api/approvals` and learns of changes from the
//! fleet invalidation feed, which every insert and removal bumps, so no event
//! channel of its own is needed.
//!
//! An entry leaves the table exactly once, by whichever comes first: the
//! user's answer (`POST /api/approvals/{id}`), the asking session's deletion
//! ([`Approvals::deny_session`]), the requesting host's connection going away
//! (the wait watches it, see [`ask`]), or the wait expiring. The last two, and
//! the parked handler being dropped at helm shutdown, all end in one drop
//! guard, so no ending can leave a card on screen for a request nobody is
//! waiting on.
//!
//! The connection has to be watched rather than left to cancellation: every
//! verb that asks is a mutation, and the helm deliberately never aborts a
//! started mutation's answer task when its connection is torn down
//! (`SupervisorClient::spawn_agent_answer`). Without the watch, a card would
//! outlive its connection by up to the whole wait, and "Always allow" on it
//! could grant the setting to whatever install held the host row by then.
//!
//! # Bounds
//!
//! There is no cap here. Each pending entry is one parked agent answer task on
//! a live connection, and the helm admits at most `client::AGENT_ANSWER_SLOTS`
//! of those per connection; because a wait ends when its connection does, a
//! host has at most that many cards up at once. While it does, every other
//! agent request from that host, listings included, is refused with the slots'
//! ordinary "too many in flight" message; SPEC_impl.md records that as
//! accepted.
//!
//! The gate that calls [`ask`] arrives in a later change of the same stack as
//! this table, after the GUI that answers it; until then only the tests
//! exercise it.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the agent request gate calling `ask` lands later in this stack"
    )
)]

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::time::Duration;

use axum::extract::{Path as AxPath, State};
use axum::response::IntoResponse;
use farhelm_proto::ErrorKind;
use farhelm_proto::approvals::{
    ApprovalAction, ApprovalAnswer, ApprovalAnswerRequest, ApprovalSession, PendingApproval,
    PendingApprovals,
};
use tokio::sync::oneshot;

use crate::agent_requests::AgentOrigin;
use crate::{AppState, http_error};

/// The refusal when no GUI is connected to ask in.
pub(crate) const NO_GUI_REFUSAL: &str = "no Farhelm window is open to ask the user for approval, \
     so nothing was done; open Farhelm and retry";

/// The refusal when the asking session is being deleted. Worded as the
/// request rather than the outcome: a guarded delete can still be refused by
/// the supervisor after this, and the session would then still exist.
pub(crate) const DELETING_REFUSAL: &str = "a delete of the asking session was requested while \
     this request waited for approval, so nothing was done";

/// The refusal when the requesting host's connection went away during the
/// wait.
pub(crate) const ORIGIN_GONE_REFUSAL: &str = "the requesting host's connection was replaced or \
     removed while this request waited for approval, so nothing was done; retry";

/// How often a waiting request rechecks its connection when the feed is
/// quiet. The feed is what normally wakes the check (a connection's phase and
/// incarnation are published through it); this only bounds how long a missed
/// wake-up could leave a dead connection's card up.
const ORIGIN_RECHECK: Duration = Duration::from_secs(5);

/// How an entry left the table, as its parked handler learns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Allow,
    Deny,
    /// The asking session is being deleted.
    SessionDeleted,
}

/// One waiting request.
struct Entry {
    view: PendingApproval,
    /// The connection the request arrived on: what "Always allow" checks is
    /// still live, and what the wait watches.
    origin: AgentOrigin,
    /// The install identity the registry held for the origin's row when the
    /// request arrived. "Always allow" writes the setting only while the row
    /// still holds it (see `HelmStore::allow_commands_for_identity`).
    identity: Option<String>,
    /// The asking session, for [`Approvals::deny_session`]. Kept beside the
    /// view rather than read back out of it so the lookup does not depend on
    /// the card's shape.
    asking_session: String,
    answer: oneshot::Sender<Verdict>,
}

/// The helm's table of requests waiting for the user. One per helm, in
/// [`AppState`].
pub(crate) struct Approvals {
    /// How long a request waits before it is refused as not answered:
    /// [`farhelm_proto::approvals::APPROVAL_WAIT`] in production, shorter in
    /// tests that observe the expiry (`rest_harness::FleetBuilder::approval_wait`).
    wait: Duration,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// Arrival order. Ids handed to clients are random (`Entry::view.id`), so
    /// an answer from a page that outlived a helm restart can never name a
    /// request of the new process; this only orders the listing.
    next_seq: u64,
    pending: BTreeMap<u64, Entry>,
    /// Sessions with a delete in flight, counted because two deletes of one
    /// session can overlap. A request from one of them is refused rather than
    /// carded (see [`Approvals::deny_session`]).
    deleting: HashMap<String, usize>,
}

impl Approvals {
    pub(crate) fn new(wait: Duration) -> Approvals {
        Approvals {
            wait,
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Every waiting request, oldest first.
    pub(crate) fn list(&self) -> Vec<PendingApproval> {
        self.lock()
            .pending
            .values()
            .map(|entry| entry.view.clone())
            .collect()
    }

    /// Take the entry whose client-facing id is `id` out of the table, if it
    /// is still waiting.
    fn take(&self, id: &str) -> Option<Entry> {
        let mut inner = self.lock();
        let seq = inner
            .pending
            .iter()
            .find(|(_, entry)| entry.view.id == id)
            .map(|(seq, _)| *seq)?;
        inner.pending.remove(&seq)
    }

    /// The origin and recorded identity of the entry `id`, if it is still
    /// waiting, without settling it.
    fn origin_of(&self, id: &str) -> Option<(AgentOrigin, Option<String>)> {
        self.lock()
            .pending
            .values()
            .find(|entry| entry.view.id == id)
            .map(|entry| (entry.origin, entry.identity.clone()))
    }

    /// Deny every request `session_id` has waiting, and refuse new ones, for
    /// as long as the returned mark lives: the caller holds it across the
    /// delete it is about to send.
    ///
    /// Called before a delete is forwarded to the supervisor. The supervisor
    /// holds the asking session's request fence for the whole of a mutating
    /// request (`agent_request_locks`), from before it reaches the helm, and its
    /// delete waits on that fence. Denying only the cards already up would
    /// miss a request still on its way into [`ask`] (resolving its launch,
    /// reading a clone's source), which would then put up a card and hold the
    /// delete for the whole wait; the mark is what makes that request refuse
    /// at once instead.
    pub(crate) fn deny_session(&self, session_id: &str) -> DeleteMark<'_> {
        let settled: Vec<Entry> = {
            let mut inner = self.lock();
            *inner.deleting.entry(session_id.to_string()).or_default() += 1;
            let seqs: Vec<u64> = inner
                .pending
                .iter()
                .filter(|(_, entry)| entry.asking_session == session_id)
                .map(|(seq, _)| *seq)
                .collect();
            seqs.into_iter()
                .filter_map(|seq| inner.pending.remove(&seq))
                .collect()
        };
        for entry in settled {
            let _ = entry.answer.send(Verdict::SessionDeleted);
        }
        DeleteMark {
            approvals: self,
            session_id: session_id.to_string(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // Nothing done while this lock is held can panic (map inserts,
        // removals and counter updates), so a poisoned lock cannot hide a
        // half-made change and is safe to keep using.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// A delete of one session in flight; see [`Approvals::deny_session`].
#[must_use = "the mark refuses the session's new requests only while it is held"]
pub(crate) struct DeleteMark<'a> {
    approvals: &'a Approvals,
    session_id: String,
}

impl Drop for DeleteMark<'_> {
    fn drop(&mut self) {
        let mut inner = self.approvals.lock();
        if let Some(count) = inner.deleting.get_mut(&self.session_id) {
            *count -= 1;
            if *count == 0 {
                inner.deleting.remove(&self.session_id);
            }
        }
    }
}

/// Removes an entry from the table when its parked handler stops waiting
/// for any reason, and announces the change.
///
/// The expiry, a dead connection and a dropped handler all end the wait by
/// leaving the scope that holds this, so all of them are covered here rather
/// than at each ending. An entry already settled by an answer or a deletion is
/// gone from the table, and the removal is a no-op apart from the feed bump,
/// which tells clients to drop the card.
struct PendingGuard<'a> {
    state: &'a AppState,
    seq: u64,
}

impl Drop for PendingGuard<'_> {
    fn drop(&mut self) {
        self.state.approvals.lock().pending.remove(&self.seq);
        self.state.manager.events().bump();
    }
}

/// Whether the GUI that would show a card is connected: at least one
/// subscriber to the fleet invalidation feed.
///
/// That feed is what every GUI holds open for its whole life, and what the
/// card relies on to appear without a reload. The desktop app's window
/// closing ends its process (dioxus-desktop's default, which Farhelm keeps),
/// so an app with no window holds no subscription. A browser whose feed
/// socket failed and fell back to polling does not count; that GUI is told
/// nothing and its request is refused as having no window to ask in.
fn gui_connected(state: &AppState) -> bool {
    state.manager.events().subscriber_count() > 0
}

/// Ask the user to approve `action`, requested by `asking_session` over the
/// host connection `origin`, and wait for the answer.
///
/// `Ok(())` means carry it out: the user allowed it (directly, or by turning
/// on the host's setting from the card), or the host's "run farhelm commands
/// from this host without asking" setting is on. Every `Err` means nothing
/// may be carried out, and its message is what the asking CLI shows:
///
/// - no GUI connected: `Unavailable`, at once (see [`gui_connected`]);
/// - the asking session being deleted: `Unauthorized`, at once or during the
///   wait;
/// - the requesting connection gone during the wait: `Unavailable`;
/// - not answered within the wait: `Unavailable`, nothing happened;
/// - denied: `Unauthorized`.
///
/// The caller must still recheck that `origin` is live after an `Ok`, before
/// acting: the connection can go between the answer and the action.
pub(crate) async fn ask(
    state: &AppState,
    origin: AgentOrigin,
    asking_session: &str,
    action: ApprovalAction,
) -> anyhow::Result<()> {
    let hosts = state.store.list_hosts().await?;
    let Some(row) = hosts.iter().find(|row| row.id == origin.host) else {
        return Err(crate::sessions::no_such_host(origin.host));
    };
    if row.commands_without_asking {
        return Ok(());
    }
    if !gui_connected(state) {
        return Err(refusal(ErrorKind::Unavailable, NO_GUI_REFUSAL.to_string()));
    }
    let host_name = crate::aggregate::host_display_name(
        row.kind,
        row.destination.as_deref(),
        row.alias.as_deref(),
    );
    let title = asking_session_title(state, origin, asking_session).await;
    let wait = state.approvals.wait;
    let expires_at_ms = unix_now_ms().saturating_add(wait.as_millis() as i64);
    let (tx, mut rx) = oneshot::channel();
    // Subscribed before the entry exists, so a connection change published
    // between the insert and the first wait is not missed.
    let mut changes = state.manager.events().subscribe();
    let seq = {
        let mut inner = state.approvals.lock();
        if inner.deleting.contains_key(asking_session) {
            return Err(refusal(
                ErrorKind::Unauthorized,
                DELETING_REFUSAL.to_string(),
            ));
        }
        inner.next_seq += 1;
        let seq = inner.next_seq;
        inner.pending.insert(
            seq,
            Entry {
                view: PendingApproval {
                    id: uuid::Uuid::new_v4().to_string(),
                    host_id: origin.host,
                    host_name: host_name.clone(),
                    session: ApprovalSession {
                        id: asking_session.to_string(),
                        title,
                        host_name: Some(host_name),
                    },
                    action,
                    expires_at_ms,
                },
                origin,
                identity: row.host_identity.clone(),
                asking_session: asking_session.to_string(),
                answer: tx,
            },
        );
        seq
    };
    let _guard = PendingGuard { state, seq };
    state.manager.events().bump();
    tracing::info!(
        approval = seq,
        host = origin.host,
        asking = crate::agent_requests::escape_for_log(asking_session).as_str(),
        "an agent request is waiting for the user's approval"
    );
    let deadline = tokio::time::Instant::now() + wait;
    let ending = loop {
        tokio::select! {
            verdict = &mut rx => break Ending::Answered(verdict.ok()),
            () = tokio::time::sleep_until(deadline) => break Ending::Expired,
            _ = changes.changed() => {}
            () = tokio::time::sleep(ORIGIN_RECHECK) => {}
        }
        if !crate::agent_requests::origin_is_live(state, origin) {
            break Ending::OriginGone;
        }
    };
    tracing::info!(approval = seq, outcome = ?ending, "an agent approval request ended");
    match ending {
        Ending::Answered(Some(Verdict::Allow)) => Ok(()),
        Ending::Answered(Some(Verdict::Deny)) => Err(refusal(
            ErrorKind::Unauthorized,
            "the user declined this request in Farhelm; nothing was done".to_string(),
        )),
        Ending::Answered(Some(Verdict::SessionDeleted)) => Err(refusal(
            ErrorKind::Unauthorized,
            DELETING_REFUSAL.to_string(),
        )),
        // The sender is dropped only with the entry, and every path that
        // removes an entry sends first; this arm is defensive.
        Ending::Answered(None) => Err(refusal(
            ErrorKind::Unavailable,
            "the approval request was withdrawn; nothing was done".to_string(),
        )),
        Ending::OriginGone => Err(refusal(
            ErrorKind::Unavailable,
            ORIGIN_GONE_REFUSAL.to_string(),
        )),
        Ending::Expired => Err(refusal(
            ErrorKind::Unavailable,
            format!(
                "nobody answered this request in Farhelm within {}, so nothing was done; retry \
                 when someone can approve it",
                describe_wait(wait)
            ),
        )),
    }
}

/// How a wait in [`ask`] ended.
#[derive(Debug)]
enum Ending {
    Answered(Option<Verdict>),
    OriginGone,
    Expired,
}

/// "9 minutes", or seconds for a wait under a minute (tests).
fn describe_wait(wait: Duration) -> String {
    match wait.as_secs() {
        seconds if seconds >= 60 => format!("{} minutes", seconds / 60),
        seconds => format!("{seconds} seconds"),
    }
}

/// The asking session's title as the ORIGIN host's cache knows it.
///
/// Looked up on that host only. The session id is the requesting
/// supervisor's claim, believed about its own host and nothing else
/// (`agent_requests`' module docs); resolving it fleet-wide would let a
/// compromised host present its request as coming from another host's session,
/// with that session's title on the card.
async fn asking_session_title(
    state: &AppState,
    origin: AgentOrigin,
    session_id: &str,
) -> Option<String> {
    state
        .store
        .cached_session(origin.host, session_id)
        .await
        .ok()
        .flatten()
        .map(|info| info.title)
}

fn refusal(kind: ErrorKind, message: String) -> anyhow::Error {
    anyhow::Error::new(crate::SupervisorError {
        origin: crate::client::ErrorOrigin::Helm,
        kind,
        message,
    })
}

/// Wall-clock now in Unix milliseconds, for the card's expiry stamp only;
/// the wait itself runs on tokio's monotonic clock.
fn unix_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// REST
// ---------------------------------------------------------------------------

/// `GET /api/approvals` — every request waiting for the user, oldest first.
pub(crate) async fn list_approvals(
    State(state): State<std::sync::Arc<AppState>>,
) -> impl IntoResponse {
    axum::Json(PendingApprovals {
        approvals: state.approvals.list(),
    })
}

/// `POST /api/approvals/{id}` — the user's answer to one card.
///
/// 204 when the answer reached a waiting request; 410 Gone when the request
/// is no longer waiting (answered elsewhere, expired, or withdrawn), which a
/// client answers by dropping the card. Gone has its own status so it cannot
/// be confused with a refusal the client must show.
///
/// "Always allow" turns the requesting host's setting on FIRST, and approves
/// only once that is stored. It is refused (409, the card keeps waiting with
/// the reason) when the request's connection is no longer live, or when the
/// host row no longer holds the install the request came from: the card was
/// raised by that install, and an adoption in between resets the setting so
/// that a new install is judged afresh (SPEC.md, Topology). One window stays
/// open: the request can expire in the instant between the setting being
/// stored and the approval, leaving the setting on with the request refused
/// as not answered. That is accepted; the user did ask for the setting.
pub(crate) async fn answer_approval(
    State(state): State<std::sync::Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(request): axum::Json<ApprovalAnswerRequest>,
) -> impl IntoResponse {
    crate::run_owned(async move {
        let verdict = match request.answer {
            ApprovalAnswer::Allow => Verdict::Allow,
            ApprovalAnswer::Deny => Verdict::Deny,
            ApprovalAnswer::AlwaysAllow => {
                let Some((origin, identity)) = state.approvals.origin_of(&id) else {
                    return gone(&id);
                };
                if !crate::agent_requests::origin_is_live(&state, origin) {
                    return http_error(refusal(
                        ErrorKind::Conflict,
                        ORIGIN_GONE_REFUSAL.to_string(),
                    ));
                }
                let serialized = state.manager.host_write_lock(origin.host).await;
                let allowed = state
                    .store
                    .allow_commands_for_identity(origin.host, identity)
                    .await;
                drop(serialized);
                match allowed {
                    Ok(true) => {
                        state.manager.events().bump();
                        tracing::info!(
                            host = origin.host,
                            "host commands-without-asking turned on from an approval card"
                        );
                    }
                    Ok(false) => {
                        return http_error(refusal(
                            ErrorKind::Conflict,
                            "the requesting host now reports a different install than the one \
                             that asked, so its setting was not changed; answer this request \
                             with allow or deny"
                                .to_string(),
                        ));
                    }
                    Err(error) => return http_error(error),
                }
                Verdict::Allow
            }
        };
        match state.approvals.take(&id) {
            Some(entry) => {
                // A failed send means the parked handler is already gone; its
                // guard announces the removal.
                let _ = entry.answer.send(verdict);
                axum::http::StatusCode::NO_CONTENT.into_response()
            }
            None => gone(&id),
        }
    })
    .await
}

/// The answer to an answer that arrived after its request stopped waiting.
fn gone(id: &str) -> axum::response::Response {
    (
        axum::http::StatusCode::GONE,
        format!("approval request {id} is no longer waiting"),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest_harness;
    use std::sync::Arc;
    use tower::ServiceExt;

    /// How long any one step of these tests may take before it fails with a
    /// diagnosis rather than hanging until the runner kills it.
    const STEP: Duration = Duration::from_secs(20);

    /// One request against the real router: its status and body as text.
    async fn request(
        harness: &rest_harness::Harness,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (axum::http::StatusCode, String) {
        let request = axum::http::Request::builder()
            .method(method)
            .uri(uri)
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                body.map(|body| body.to_string()).unwrap_or_default(),
            ))
            .unwrap();
        let response = harness.router().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    async fn answer(
        harness: &rest_harness::Harness,
        id: &str,
        answer: &str,
    ) -> (axum::http::StatusCode, String) {
        request(
            harness,
            "POST",
            &format!("/api/approvals/{id}"),
            Some(serde_json::json!({ "answer": answer })),
        )
        .await
    }

    /// The local host's LIVE connection as a request's origin, the way the
    /// relay hands it to a verb handler. The premise every waiting test
    /// rests on: `ask` ends a wait whose connection is not live.
    fn live_origin(harness: &rest_harness::Harness, host: crate::store::HostId) -> AgentOrigin {
        let client = harness
            .manager
            .status(host)
            .expect("an actor runs for the host")
            .client
            .expect("premise: the host is connected");
        let origin = AgentOrigin {
            host,
            connection: client.connection_id(),
        };
        assert!(
            crate::agent_requests::origin_is_live(&harness.state, origin),
            "premise: the origin is live"
        );
        origin
    }

    async fn local_origin(harness: &rest_harness::Harness) -> AgentOrigin {
        let host = rest_harness::local_id(&harness.store).await;
        live_origin(harness, host)
    }

    fn stop_action() -> ApprovalAction {
        ApprovalAction::Stop {
            target: ApprovalSession {
                id: "target".to_string(),
                title: None,
                host_name: None,
            },
        }
    }

    /// Start `ask` for a stop on a task, as a verb handler would, and wait
    /// (bounded) until a card from `asking` is listed. Returns the task and
    /// the card's id. A request refused before it put up a card fails here
    /// with that refusal, rather than as a timeout with no cause.
    async fn waiting(
        harness: &rest_harness::Harness,
        origin: AgentOrigin,
        asking: &str,
    ) -> (tokio::task::JoinHandle<anyhow::Result<()>>, String) {
        let state = Arc::clone(&harness.state);
        let asker = asking.to_string();
        let task = tokio::spawn(async move { ask(&state, origin, &asker, stop_action()).await });
        let mut changes = harness.state.manager.events().subscribe();
        let found = tokio::time::timeout(STEP, async {
            loop {
                if let Some(card) = harness
                    .state
                    .approvals
                    .list()
                    .into_iter()
                    .find(|card| card.session.id == asking)
                {
                    return card.id;
                }
                if task.is_finished() {
                    return String::new();
                }
                // The feed wakes this when a card appears; the tick only notices
                // a request that ended without one, which bumps nothing.
                tokio::select! {
                    _ = changes.changed() => {}
                    // sleep-ok: polling interval for the task's own ending, beside the feed wake-up.
                    () = tokio::time::sleep(Duration::from_millis(50)) => {}
                }
            }
        })
        .await;
        match found {
            Ok(id) if !id.is_empty() => (task, id),
            Ok(_) => panic!(
                "the request ended before it put up a card: {:?}",
                task.await.unwrap().map_err(|error| format!("{error:#}"))
            ),
            Err(_) => panic!(
                "no card within {STEP:?}; GUI seats: {}",
                harness.state.manager.events().subscriber_count()
            ),
        }
    }

    /// The ask task's result, bounded.
    async fn ended(task: tokio::task::JoinHandle<anyhow::Result<()>>) -> anyhow::Result<()> {
        tokio::time::timeout(STEP, task)
            .await
            .expect("the request ended within the step bound")
            .expect("the ask task did not panic")
    }

    /// The kind and message of a refusal `ask` made itself.
    struct Refusal {
        kind: ErrorKind,
        message: String,
    }

    fn refusal_of(result: anyhow::Result<()>) -> Refusal {
        let error = result.expect_err("refused");
        let refusal = error
            .downcast_ref::<crate::SupervisorError>()
            .unwrap_or_else(|| panic!("refused with the helm's own error, got {error:#}"));
        Refusal {
            kind: refusal.kind,
            message: refusal.message.clone(),
        }
    }

    async fn local_row(harness: &rest_harness::Harness) -> crate::store::HostRow {
        let local = rest_harness::local_id(&harness.store).await;
        harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == local)
            .unwrap()
    }

    /// Spec: with no GUI subscribed to the event feed, a request that needs
    /// approval is refused at once as Unavailable, saying there is no Farhelm
    /// window to ask in, and no card is created.
    ///
    /// Why: SPEC.md has the request fail fast rather than sit nine minutes
    /// waiting for an answer nobody can give; an agent that waited would hit
    /// its own tool timeout and learn nothing.
    #[farhelm_testtrace::test]
    async fn with_no_gui_a_request_is_refused_at_once() {
        let harness = rest_harness::idle_helm().await;
        assert_eq!(harness.state.manager.events().subscriber_count(), 0);
        let origin = local_origin(&harness).await;
        let refused = refusal_of(ask(&harness.state, origin, "asker", stop_action()).await);
        assert_eq!(refused.kind, ErrorKind::Unavailable);
        assert_eq!(refused.message, NO_GUI_REFUSAL);
        assert!(harness.state.approvals.list().is_empty());
    }

    /// Spec: a host whose "run farhelm commands without asking" setting is on
    /// is not asked about, with or without a GUI; turning the setting back off
    /// asks again.
    ///
    /// Why: the setting is SPEC.md's per-host way to stop asking, and it is
    /// the helm's to read from its own registry, never from the request.
    #[farhelm_testtrace::test]
    async fn a_host_set_to_run_commands_without_asking_is_not_asked() {
        let harness = rest_harness::idle_helm().await;
        let origin = local_origin(&harness).await;
        harness
            .store
            .set_commands_without_asking(origin.host, true)
            .await
            .unwrap();
        ask(&harness.state, origin, "asker", stop_action())
            .await
            .expect("allowed without asking");
        assert!(harness.state.approvals.list().is_empty());
        harness
            .store
            .set_commands_without_asking(origin.host, false)
            .await
            .unwrap();
        let refused = refusal_of(ask(&harness.state, origin, "asker", stop_action()).await);
        assert_eq!(refused.message, NO_GUI_REFUSAL, "asks again once it is off");
    }

    /// Spec: with a GUI connected, a request waits as a card listed by
    /// `GET /api/approvals`, naming the requesting host, the asking session
    /// with its title from that host's cache, and the action. Allow over
    /// `POST /api/approvals/{id}` lets the request go ahead, removes the card
    /// and bumps the event feed; a second answer to the same card is 410 Gone.
    ///
    /// Why: this is the round trip every acting agent verb depends on. The
    /// listing is the only way the GUI learns what it is approving, the bump
    /// is the only way an open GUI learns to drop the card, and Gone is how it
    /// tells a stale card from a refusal it must show.
    #[farhelm_testtrace::test]
    async fn an_allowed_card_lets_the_request_through_and_disappears() {
        let harness = rest_harness::idle_helm().await;
        let _gui = harness.state.manager.events().admit(1).expect("a GUI seat");
        let origin = local_origin(&harness).await;
        let (task, id) = waiting(&harness, origin, "sess-1").await;
        let (status, body) = request(&harness, "GET", "/api/approvals", None).await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        let listed: PendingApprovals = serde_json::from_str(&body).unwrap();
        assert_eq!(listed.approvals.len(), 1, "{body}");
        let card = &listed.approvals[0];
        assert_eq!(card.id, id);
        assert_eq!(card.host_id, origin.host);
        assert_eq!(card.session.id, "sess-1");
        assert!(
            card.session.title.is_some(),
            "the origin host caches sess-1: {body}"
        );
        assert!(matches!(card.action, ApprovalAction::Stop { .. }));

        let before = harness.state.manager.events().revision();
        let (status, body) = answer(&harness, &id, "allow").await;
        assert_eq!(status, axum::http::StatusCode::NO_CONTENT, "{body}");
        ended(task).await.expect("allowed");
        assert!(harness.state.approvals.list().is_empty());
        assert!(
            harness.state.manager.events().revision() > before,
            "the card's removal is announced"
        );
        let (status, _) = answer(&harness, &id, "allow").await;
        assert_eq!(status, axum::http::StatusCode::GONE);
    }

    /// Spec: the asking session on a card is looked up on the requesting
    /// host only: a request claiming the id of a session another host caches
    /// gets no title from that host.
    ///
    /// Why: the session id is the requesting supervisor's claim, believed
    /// about its own host only. A fleet-wide lookup would let a compromised
    /// host dress its request up as another machine's session.
    #[farhelm_testtrace::test]
    async fn a_card_does_not_borrow_another_hosts_session_title() {
        let harness = rest_harness::idle_helm().await;
        let _gui = harness.state.manager.events().admit(1).expect("a GUI seat");
        let origin = local_origin(&harness).await;
        let elsewhere = AgentOrigin {
            host: origin.host + 1_000,
            ..origin
        };
        assert_eq!(
            asking_session_title(&harness.state, elsewhere, "sess-1").await,
            None,
            "a host that does not own sess-1 learns nothing about it"
        );
        assert!(
            asking_session_title(&harness.state, origin, "sess-1")
                .await
                .is_some(),
            "premise: the owning host's cache has the title"
        );
    }

    /// Spec: Deny refuses the request as Unauthorized with a message saying
    /// the user declined it, and leaves the host's setting alone.
    ///
    /// Why: SPEC.md makes declined a distinct, readable refusal after which
    /// nothing is carried out.
    #[farhelm_testtrace::test]
    async fn a_denied_card_refuses_the_request() {
        let harness = rest_harness::idle_helm().await;
        let _gui = harness.state.manager.events().admit(1).expect("a GUI seat");
        let origin = local_origin(&harness).await;
        let (task, id) = waiting(&harness, origin, "asker").await;
        let (status, _) = answer(&harness, &id, "deny").await;
        assert_eq!(status, axum::http::StatusCode::NO_CONTENT);
        let refused = refusal_of(ended(task).await);
        assert_eq!(refused.kind, ErrorKind::Unauthorized);
        assert!(refused.message.contains("declined"), "{}", refused.message);
        assert!(!local_row(&harness).await.commands_without_asking);
    }

    /// Spec: Always allow turns the requesting host's setting on, then
    /// approves this request.
    ///
    /// Why: SPEC.md's "Always allow from <host>" is the setting change and
    /// the approval together; approving without storing the setting would
    /// keep asking, and storing it without approving would leave the agent
    /// waiting on a card the user already answered.
    #[farhelm_testtrace::test]
    async fn always_allow_turns_the_host_setting_on_and_approves() {
        let harness = rest_harness::idle_helm().await;
        let _gui = harness.state.manager.events().admit(1).expect("a GUI seat");
        let origin = local_origin(&harness).await;
        let (task, id) = waiting(&harness, origin, "asker").await;
        let (status, body) = answer(&harness, &id, "always_allow").await;
        assert_eq!(status, axum::http::StatusCode::NO_CONTENT, "{body}");
        ended(task).await.expect("allowed");
        assert!(local_row(&harness).await.commands_without_asking);
    }

    /// Spec: when the requesting host's connection goes away while a card
    /// waits, the request is refused as Unavailable saying so, its card goes,
    /// and an Always allow sent for it changes no setting.
    ///
    /// Why: the helm never cancels a started mutation when its connection
    /// dies, so without this watch a dead connection's card would stay up for
    /// the whole wait, and Always allow on it could grant the setting to a
    /// different install that took over the host row in the meantime.
    #[farhelm_testtrace::test]
    async fn a_card_goes_with_its_connection_and_grants_nothing() {
        let harness = rest_harness::idle_helm().await;
        let _gui = harness.state.manager.events().admit(1).expect("a GUI seat");
        let origin = local_origin(&harness).await;
        let (task, id) = waiting(&harness, origin, "asker").await;
        harness.fleet.take_down(origin.host);
        harness
            .await_state(origin.host, |state| state.phase() != "connected")
            .await;
        let (status, body) = answer(&harness, &id, "always_allow").await;
        assert!(
            status == axum::http::StatusCode::CONFLICT || status == axum::http::StatusCode::GONE,
            "a card from a dead connection cannot be approved: {status} {body}"
        );
        let refused = refusal_of(ended(task).await);
        assert_eq!(refused.kind, ErrorKind::Unavailable);
        assert_eq!(refused.message, ORIGIN_GONE_REFUSAL);
        assert!(harness.state.approvals.list().is_empty());
        assert!(!local_row(&harness).await.commands_without_asking);
    }

    /// Spec: a request nobody answers within the wait is refused as
    /// Unavailable, saying nobody answered, and its card is gone.
    ///
    /// Why: SPEC.md bounds the wait so an agent always gets an answer before
    /// its own tool timeout, and a card left behind would offer the user an
    /// approval that can no longer reach anything.
    #[farhelm_testtrace::test]
    async fn an_unanswered_request_expires_and_its_card_goes() {
        let harness = rest_harness::FleetBuilder::new()
            .await
            .approval_wait(Duration::from_millis(300))
            .local(rest_harness::HostScript {
                identity: Some("local-identity".to_string()),
                ..rest_harness::HostScript::default()
            })
            .await
            .start()
            .await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;
        let _gui = harness.state.manager.events().admit(1).expect("a GUI seat");
        let origin = live_origin(&harness, local);
        let refused = refusal_of(ask(&harness.state, origin, "asker", stop_action()).await);
        assert_eq!(refused.kind, ErrorKind::Unavailable);
        assert!(
            refused.message.contains("nobody answered"),
            "{}",
            refused.message
        );
        assert!(refused.message.contains("0 seconds"), "{}", refused.message);
        assert!(harness.state.approvals.list().is_empty());
    }

    /// Spec: a delete of the asking session denies its waiting requests with
    /// a message naming the delete, refuses new requests from it for as long
    /// as the delete is in flight, and leaves other sessions' cards alone;
    /// once the delete is over, the session may ask again.
    ///
    /// Why: the supervisor holds the asking session's request fence from
    /// before a request reaches the helm, and its delete waits on that fence.
    /// Denying only the cards already up would miss a request still on its
    /// way to one, which would then hold the delete for the whole wait.
    #[farhelm_testtrace::test]
    async fn a_delete_in_flight_denies_and_refuses_the_sessions_requests() {
        let harness = rest_harness::idle_helm().await;
        let _gui = harness.state.manager.events().admit(1).expect("a GUI seat");
        let origin = local_origin(&harness).await;
        let (doomed, _) = waiting(&harness, origin, "doomed").await;
        let (other, _) = waiting(&harness, origin, "other").await;
        let mark = harness.state.approvals.deny_session("doomed");
        let refused = refusal_of(ended(doomed).await);
        assert_eq!(refused.message, DELETING_REFUSAL);
        let refused = refusal_of(ask(&harness.state, origin, "doomed", stop_action()).await);
        assert_eq!(refused.kind, ErrorKind::Unauthorized);
        assert_eq!(
            refused.message, DELETING_REFUSAL,
            "no card while the delete runs"
        );
        let left = harness.state.approvals.list();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].session.id, "other");
        drop(mark);
        let (again, _) = waiting(&harness, origin, "doomed").await;
        again.abort();
        other.abort();
    }

    /// Spec: a card's id is a fresh random value, so it cannot name a request
    /// of another helm process.
    ///
    /// Why: a browser can keep a card across a helm restart and answer it
    /// before its next listing arrives. Ids counted from 1 in every process
    /// would let that answer approve whatever unrelated request the new
    /// process numbered the same.
    #[farhelm_testtrace::test]
    async fn card_ids_are_not_a_process_local_count() {
        let harness = rest_harness::idle_helm().await;
        let _gui = harness.state.manager.events().admit(1).expect("a GUI seat");
        let origin = local_origin(&harness).await;
        let (task, id) = waiting(&harness, origin, "asker").await;
        assert!(uuid::Uuid::parse_str(&id).is_ok(), "{id}");
        let (status, _) = answer(&harness, "1", "allow").await;
        assert_eq!(status, axum::http::StatusCode::GONE);
        assert_eq!(harness.state.approvals.list().len(), 1, "still waiting");
        task.abort();
    }

    /// Spec: when the task waiting on a card is dropped (the helm shutting
    /// down drops every answer task), the card disappears from the listing
    /// and the removal is announced.
    ///
    /// Why: nothing would ever answer or expire such a card, so the user
    /// would be left approving a request that can no longer happen.
    #[farhelm_testtrace::test]
    async fn a_dropped_wait_removes_its_card() {
        let harness = rest_harness::idle_helm().await;
        let _gui = harness.state.manager.events().admit(1).expect("a GUI seat");
        let origin = local_origin(&harness).await;
        let (task, _) = waiting(&harness, origin, "asker").await;
        let before = harness.state.manager.events().revision();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(harness.state.approvals.list().is_empty());
        assert!(harness.state.manager.events().revision() > before);
    }

    /// Spec: `DELETE /api/sessions/{id}` denies the session's waiting
    /// requests before the delete reaches the supervisor.
    ///
    /// Why: the supervisor parks a delete behind the asking session's
    /// in-flight request; if the helm forwarded the delete first, the user
    /// deleting a session would wait on a card for that same session.
    #[farhelm_testtrace::test]
    async fn deleting_a_session_over_rest_denies_its_cards_first() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let (delete_seen, delete_seen_rx) = oneshot::channel::<()>();
        let (reply, reply_rx) = oneshot::channel::<()>();
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::DeleteSession { req_id, .. } = request else {
                panic!("expected DeleteSession, got {request:?}");
            };
            delete_seen.send(()).unwrap();
            reply_rx.await.unwrap();
            writer
                .write_control(&ControlMsg::SessionDeleted {
                    req_id,
                    notice: None,
                })
                .await
                .unwrap();
        });
        let harness = rest_harness::spliced_helm(client_side).await;
        let _gui = harness.state.manager.events().admit(1).expect("a GUI seat");
        let origin = local_origin(&harness).await;
        let (task, _) = waiting(&harness, origin, "sess-1").await;
        let router = harness.router();
        let delete = tokio::spawn(async move {
            router
                .oneshot(
                    axum::http::Request::builder()
                        .method("DELETE")
                        .uri("/api/sessions/sess-1")
                        .header("host", "127.0.0.1:7433")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status()
        });
        tokio::time::timeout(STEP, delete_seen_rx)
            .await
            .expect("the delete reached the supervisor")
            .unwrap();
        let refused = refusal_of(ended(task).await);
        assert_eq!(refused.message, DELETING_REFUSAL);
        reply.send(()).unwrap();
        assert_eq!(delete.await.unwrap(), axum::http::StatusCode::OK);
        peer.await.unwrap();
    }
}
