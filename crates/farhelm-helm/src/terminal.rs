//! `/api/sessions/{id}/term` — the terminal WebSocket, and the pump behind
//! it.
//!
//! One browser socket on this side, one multiplexed channel to a supervisor
//! on the other, and a translation between them that has to survive either
//! end going away at any moment. This is the only route in the helm that
//! holds a connection open for the life of a user's attention, which is why
//! nearly everything here is about teardown rather than about bytes.
//!
//! ## Auth is refused before upgrade; attach failures are delivered on-socket
//!
//! The API authentication boundary runs before `serve_term_upgrade`, so an
//! unauthenticated request receives HTTP 401 and never becomes a WebSocket.
//! After that boundary, `serve_term_upgrade` accepts the upgrade and does everything else
//! afterwards: `serve_term` resolves the query (`resolve_attach_request`),
//! routes to the owning host, and only then sends the `Attach`. So by the
//! time anything can be refused — an
//! unparseable `?lease=`, an unknown session or tab, a non-connected host,
//! another client already holding the session — the WebSocket is open.
//!
//! Every one of those refusals therefore leaves the same way: a
//! `{"type":"detached","reason":...}` text frame, then a close. That
//! uniformity is the point. A browser cannot read a status code off a
//! failed upgrade in any useful way, so the alternative is a socket that
//! opens and closes with nothing said — which the user experiences as a
//! terminal that flickered and vanished, and which the browser blames on
//! the network rather than on the request it just made.
//!
//! ## Teardown is the hard part
//!
//! A wedged browser must not be able to hold anything on this side open,
//! and a socket that closes normally must not be reported as an error.
//! `settle_outbound` is where that lands: the outbound drain is given
//! `WS_TEARDOWN_GRACE` to finish and is then abandoned, and a handle that
//! has ALREADY completed is never polled again — a real bug once, and
//! invisible from the browser, since the socket was closing either way.
//!
//! ## What crosses the socket
//!
//! Binary frames are the attachment's grid stream (`farhelm_proto::grid`),
//! one grid message per frame in each direction, relayed without being read.
//! Text frames are the control channel, typed once for both ends in
//! `farhelm_proto::terminal_socket`: resize and the heartbeat's ping from the
//! client, and from this side the pong, the marker that separates the
//! catch-up from live updates, and the detach notice.
//!
//! ## Bounds and flow control
//!
//! The helm reads no grid message; what it enforces is per frame and per
//! queue:
//!
//! - Outbound frames can be up to `GRID_MAX_MESSAGE` (1 MiB), and the
//!   per-terminal queue holds up to 4,096 of them, bounded by its 16 MiB of
//!   bytes (the client's `GRID_EVENT_QUEUE` and `TERM_QUEUE_BYTES`).
//! - Each inbound binary message is one grid input message and goes up as
//!   one data frame (`SupervisorClient::send_grid_input`); one over
//!   `GRID_MAX_INPUT_MESSAGE` ends the attachment.
//! - The grid client sends no pause or resume, so the helm pauses and
//!   resumes the supervisor from its own queue's fill (the client's
//!   `GRID_PAUSE_BYTES`, which also works out how slow a browser that still
//!   reads may be). A pause that outlives the supervisor's stall timeout,
//!   from a browser that stopped reading, ends the attachment as stalled.
//!
//! Version skew needs nothing of its own here: the protocol version must
//! match, so a supervisor from before protocol 77, which could still stream
//! an old terminal as bytes, fails the handshake as Needs update.

use crate::sessions::{default_cols, default_rows, resolve_session_ref, route_active_session};
use crate::{AppState, SupervisorClient, SupervisorError, TermEvent, TermStream};
use axum::extract::{Path as AxPath, Query, State, WebSocketUpgrade, ws};
use axum::response::IntoResponse;
use farhelm_proto::terminal_socket::{TerminalClientMessage, TerminalServerMessage};
use farhelm_proto::{ErrorKind, TerminalSelector};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{error, info};

// ---- The attachment trace -----------------------------------------------

/// The tracing target of the attachment trace: one debug event when an
/// admitted terminal attachment starts relaying and one when its socket's
/// handler is done with it.
///
/// A dedicated target so that a log can turn exactly these on
/// (`RUST_LOG=…,farhelm_helm::terminal_attachments=debug`) without the rest
/// of this module's debug output. It shows which session a window attached
/// and whether that attachment stayed open, which nothing outside the
/// process shows, since a terminal host serves only its supervisor. The old
/// desktop smoke test read these events; it went with the Dioxus app (GPUI
/// migration, F1), so nothing parses them today.
const ATTACHMENT_TRACE: &str = "farhelm_helm::terminal_attachments";

/// One admitted attachment's trace: `terminal attached` when made, and
/// `terminal ended` when dropped, so every way out of the socket's handler
/// (an early return and a panic's unwinding included) reports the end
/// exactly once.
struct TracedAttachment {
    session: String,
    /// `agent`, or `tab:` followed by a bounded, escaped peer identifier.
    terminal: String,
}

impl TracedAttachment {
    /// Trace the admitted attachment without giving a host's tab ID control of
    /// the operator's log terminal. Keep `agent` plain for the desktop smoke.
    fn start(session: &str, tab: Option<&str>) -> Self {
        let traced = Self {
            session: session.to_string(),
            terminal: match tab {
                None => "agent".to_string(),
                Some(tab) => format!("tab:{}", crate::manager::peer_text(tab)),
            },
        };
        tracing::debug!(
            target: ATTACHMENT_TRACE,
            session = %traced.session,
            terminal = %traced.terminal,
            "terminal attached"
        );
        traced
    }
}

impl Drop for TracedAttachment {
    fn drop(&mut self) {
        tracing::debug!(
            target: ATTACHMENT_TRACE,
            session = %self.session,
            terminal = %self.terminal,
            "terminal ended"
        );
    }
}

/// Initial terminal size, carried as query parameters because a WebSocket
/// handshake is a GET with no body. Sizing the terminal at attach time
/// rather than waiting for the first `resize` message is what gets live
/// output to the right width immediately instead of reflowing a moment
/// later.
///
/// What the size does to the catch-up is the supervisor's business, not
/// this route's: the attach waits briefly for the size to take effect in
/// the terminal host before it reads the screen and history it sends, so
/// the grid catch-up is drawn at the new width rather than sent twice, and
/// a full-screen app finishes repainting on the SIGWINCH it already
/// received. A freshly
/// opened tab normally already has the agent terminal's size, so its
/// attach-time resize is a no-op (a failed pre-size of an old tab, or a
/// client that resized between open and attach, degrades to the ordinary
/// resize-at-attach).
///
/// A `grid` parameter, which browser bundles before protocol 77 sent, is
/// ignored like any other unknown one: every attachment streams a grid.
///
/// `tab` and `lease` are PLAN_M4.md item 5's terminal-selector plumbing,
/// and BOTH are additive by construction, not just by `Option`: a request
/// carrying neither must reach the supervisor as the exact pre-M4 `Attach`
/// shape — `TerminalSelector::Agent` and an empty lease — because every
/// caller that predates tabs (an older UI build, a bookmarked URL, a
/// script) still means "attach the agent terminal as my one and only
/// terminal" when it says nothing. `resolve_attach_request` below is
/// where that legacy-absent reading and the new query-parsing both live,
/// deliberately as one function, so the two cannot silently diverge.
#[derive(Deserialize)]
pub(crate) struct TermQuery {
    #[serde(default = "default_cols")]
    cols: u16,
    #[serde(default = "default_rows")]
    rows: u16,
    /// The tab to attach, echoing a `TabInfo::id` this session's
    /// `SessionInfo.tabs` already handed the client. Absent means the
    /// agent terminal — see this struct's own docs. Deliberately NOT
    /// validated for shape (contrast `lease` below): every value,
    /// including an empty string, has an unambiguous supervisor-side
    /// reading, so there is nothing for the helm to reject here.
    tab: Option<String>,
    /// This client's session-scoped attach identity (PLAN_M4.md item 3),
    /// forwarded verbatim to `attach_terminal`. Absent means the empty,
    /// un-leased pre-M4 reading — see this struct's own docs. Kept as
    /// `Option<String>` rather than defaulted straight to `String`
    /// (`#[serde(default)]`) specifically so a PRESENT empty value stays
    /// distinguishable from an ABSENT one at the type level:
    /// `resolve_attach_request` needs to refuse the former while still
    /// reading the latter as empty, and collapsing them early would
    /// destroy exactly the distinction that refusal depends on.
    lease: Option<String>,
}

/// Turn a `?tab=`/`?lease=` query pair into what `attach_terminal` wants,
/// rejecting only the one shape neither the wire nor the supervisor CAN
/// reject on the helm's behalf.
///
/// `tab` needs no local validation at all (PLAN_M4.md item 5): an ABSENT
/// value is the agent terminal (the legacy reading `TermQuery` documents),
/// and every PRESENT value — including an empty string — becomes
/// `TerminalSelector::Tab { id }` and is left entirely to the supervisor's
/// own attach handling, which answers `NotFound` for an id no `TabInfo`
/// ever carried. That is the same visible failure every other unknown tab
/// produces, so there is no separate "shape" rejection to keep in sync
/// with it — one canonical path instead of two.
///
/// `lease` is asymmetric, and deliberately so. An ABSENT lease is the
/// pre-M4 un-leased singleton reading (`ControlMsg::Attach::lease`'s own
/// docs) — which IS the empty string on the wire, because that is what
/// every caller written before leases existed sends. A PRESENT but
/// EXPLICITLY EMPTY `?lease=` cannot be forwarded as that same empty
/// string: once it reaches the supervisor there is no way to tell "this
/// caller said nothing" apart from "this caller said lease is empty" —
/// the wire has only one empty-string value, not two — and the supervisor
/// already treats an empty lease as legal legacy content, so it has no
/// hook to refuse it either. Collapsing the two would let a client that
/// explicitly opted into the un-leased singleton reading (a stale
/// bookmark, a hand-written URL) silently join — and be joined by —
/// every OTHER un-leased attachment on the session: the one outcome
/// PLAN_M4.md item 3's per-session takeover exists to prevent. So this is
/// refused HERE, before it becomes indistinguishable from absence, which
/// is the only point in the whole path where the distinction still
/// exists to check.
fn resolve_attach_request(q: &TermQuery) -> anyhow::Result<(TerminalSelector, &str)> {
    let terminal = match &q.tab {
        None => TerminalSelector::Agent,
        Some(id) => TerminalSelector::Tab { id: id.clone() },
    };
    let lease = match q.lease.as_deref() {
        None => "",
        Some("") => {
            return Err(anyhow::anyhow!(
                "terminal websocket's ?lease= must not be empty"
            ));
        }
        Some(lease) => lease,
    };
    Ok((terminal, lease))
}

/// Resolve `q`, route to the session's owning host, and attach — as one
/// `Result` (PLAN_M4.md item 5; owner routing per PLAN_M6.md item 5).
///
/// Folding the local query-shape check, the owner lookup, and the
/// supervisor round trip into a single function is what lets `serve_term`
/// report every kind of failure through one notice-then-close arm instead
/// of three copies of the same three lines: a caller here cannot tell (and
/// does not need to) whether an `Err` came from `resolve_attach_request`
/// refusing the shape, from the session's host being unreachable, or from
/// the supervisor refusing the attach itself — all are, from the browser's
/// perspective, "this attach did not happen," and all deserve the identical
/// visible treatment.
///
/// That uniformity is also what gives the terminal socket its half of
/// SPEC.md's host-unreachable story for free: the refusal text names the
/// host's actual state, and it arrives as the same
/// `{"type":"detached","reason":...}` notice a takeover would, so nothing
/// on the browser side needs a new message shape to render it.
///
/// The client is returned alongside the attachment because `serve_term`
/// must keep talking to the SAME connection for the socket's whole life —
/// input, resize, detach. The resolved bare ID travels with it,
/// because later resize messages must never send the public full reference.
/// Re-routing per message would let a mid-session reconnect silently move a live
/// terminal's writes to a different connection than the one its attachment lives on.
async fn attach_from_query(
    state: &AppState,
    session_id: &str,
    q: &TermQuery,
    if_unowned: bool,
) -> anyhow::Result<(Arc<SupervisorClient>, String, u32, TermStream)> {
    let (terminal, lease) = resolve_attach_request(q)?;
    let key = resolve_session_ref(state, session_id).await?;
    let (_claim, client) = route_active_session(state, &key).await?;
    // Which of the two attach contracts this socket wants, decided by the
    // query the browser sent: an unattended reconnect refuses rather than
    // displaces (see `TermQuery::if_unowned`), everything else is the
    // ordinary last-attach-wins attach.
    let (channel, stream) = if if_unowned {
        client
            .attach_terminal_if_unowned(&key.session_id, q.cols, q.rows, terminal, lease)
            .await?
    } else {
        client
            .attach_terminal(&key.session_id, q.cols, q.rows, terminal, lease)
            .await?
    };
    Ok((client, key.session_id, channel, stream))
}

/// Whether this failure is the supervisor refusing a non-displacing attach
/// because another client holds the session.
///
/// `anyhow`'s `downcast_ref` searches the root cause and every context
/// layer above it, so this survives callers that annotate the error on the
/// way out — which is exactly what comparing rendered text does not.
fn refused_as_taken_over(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<SupervisorError>()
        .is_some_and(|supervised| supervised.kind() == ErrorKind::TakenOver)
}

/// Whether this failure says the TAB being attached does not exist, which
/// the browser must hear as `tab_closed` rather than as a failed attempt.
///
/// Every `NotFound` a tab attach can meet describes current state, not a
/// passing condition: the supervisor looks the tab up among the session's
/// live tabs at attach time (its tab hosts; an exited but unreaped tab
/// already counts as gone); and a session or host the helm cannot route has
/// taken its tabs with it.
/// Transport trouble (a lost supervisor, an unreachable host) arrives as
/// other kinds and keeps recovering. Reported as `other`, this refusal
/// sent the browser's reconnect ladder through six attempts per tab, each
/// rebuilding a terminal, which is what starved the page in the island-cap
/// test (FLAKES.md, 2026-09-29) and would do the same to a view listing
/// tabs the supervisor no longer has.
///
/// Tab attaches only. The agent terminal's `NotFound` carries the
/// supervisor's account of what happened to the session (a reboot, a
/// restart gap, what restart would do), and the browser keeps showing it
/// rather than hiding the terminal.
fn refused_as_missing_tab(error: &anyhow::Error, q: &TermQuery) -> bool {
    q.tab.is_some()
        && error
            .downcast_ref::<SupervisorError>()
            .is_some_and(|supervised| supervised.kind() == ErrorKind::NotFound)
}

/// The `{"type":"detached","reason":...,"code":...}` notice the GUI's terminal client receives.
///
/// `code` is the [`farhelm_proto::DetachCode`] a client decides from (latch a
/// takeover, hold a stall, hide a closed tab); `reason` is only ever shown.
/// One builder so every exit from this socket carries both.
fn detached_notice(detach: &crate::client::Detach) -> TerminalServerMessage {
    TerminalServerMessage::Detached {
        reason: detach.reason.clone(),
        code: Some(detach.code),
    }
}

/// One control message as the text frame that carries it ([`TerminalServerMessage::to_text`] is the one spelling).
fn control_frame(message: &TerminalServerMessage) -> ws::Message {
    ws::Message::Text(message.to_text().into())
}

/// Terminal WebSocket: binary frames are the attachment's grid messages in
/// both directions; text frames are small JSON control messages (client →
/// resize/ping; server → detached notice, replay-complete marker, pong).
/// This is the browser-facing twin of the proto data channel, kept equally
/// dumb.
///
/// `?cols=`/`?rows=` set the initial size (see `TermQuery`'s docs).
/// The sibling route `/api/sessions/{id}/term/unowned` (PLAN_M6.md item 7)
/// is this same socket attached NON-DISPLACINGLY, and its refusal arrives
/// here as an ordinary `{"type":"detached","reason":"another client
/// attached"}` notice — the same notice, and the same string, a client
/// displaced while attached receives. That identity is the point: a
/// browser that was away when it lost the session renders the state it is
/// actually in without a second vocabulary for it. See `term_ws_if_unowned`
/// for why it is a path rather than a parameter.
///
/// `?tab=<id>` and `?lease=<id>` (PLAN_M4.md item 5) select which of the
/// session's terminals this socket attaches and under which client
/// identity; BOTH default to the exact pre-M4 behavior when absent —
/// the agent terminal, un-leased — so a caller that predates tabs sees no
/// change at all. `resolve_attach_request` owns the one shape check the
/// helm makes locally (an explicitly empty `?lease=`, never `?tab=` —
/// see that function's own docs for why the two are asymmetric);
/// everything else, including an unknown tab id, is the supervisor's own
/// `NotFound` and reaches this socket exactly like any other attach
/// failure — a `{"type":"detached",...}` notice, then close, never a bare
/// disconnect the browser would blame on the network instead of the
/// session (see `serve_term`'s single attach-failure arm).
///
/// The client → server text messages, all `{"type": ...}` and typed as
/// [`TerminalClientMessage`] (the server → client ones as
/// [`TerminalServerMessage`]), one definition both ends share:
/// - `{"type":"resize","cols":N,"rows":N}` — the terminal's new size.
/// - `{"type":"ping"}` — the browser's idle-gated liveness probe
///   (PLAN_M6.md item 7), answered with `{"type":"pong"}` and forwarded to
///   nobody.
///
/// The ping is the one client message this helm ANSWERS itself rather than
/// relaying, and that is the whole design rather than an optimization. What
/// the browser needs to know is whether ITS socket is still carrying
/// anything — the laptop-wake case, where a NAT or sleep timeout killed the
/// connection and left both ends believing it is open — and that question
/// is answered entirely by this end replying. Round-tripping it to the
/// supervisor would answer a different question (is the HOST healthy),
/// which already has its own surfaces: the host's connection state, the
/// stall detach, and the detach notice this socket would receive anyway.
/// It would also put a periodic message on every idle session's control
/// path for no added signal.
///
/// The browser sends no flow control: the helm pauses and resumes the
/// supervisor from its own queue (this module's "Bounds and flow control").
/// A `pause` or `resume` from a page is an unknown message, ignored.
///
/// Server → client, all text: `{"type":"pong"}`,
/// `{"type":"detached","reason":...}` and, as of PLAN_M5.md item 4,
/// `{"type":"replay_complete"}` — the attach's
/// catch-up boundary, forwarded on this SAME socket after the binary
/// catch-up messages it follows and before any live ones, because
/// [`TermEvent::ReplayComplete`] rides the terminal's data queue rather
/// than jumping ahead of it (see that variant's own docs for why the
/// ordering is the whole point). Consumers must treat it as pure
/// presentation, never as a signal for session or lifecycle behavior —
/// the same restriction `ControlMsg::ReplayComplete`'s docs place on every
/// consumer of the wire message it forwards.
pub(crate) async fn term_ws(
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
    AxPath(id): AxPath<String>,
    Query(q): Query<TermQuery>,
) -> impl IntoResponse {
    serve_term_upgrade(state, id, q, upgrade, false)
}

/// The same socket, attached NON-DISPLACINGLY: refused rather than taking
/// the session from another client (`ControlMsg::Attach::if_unowned`).
///
/// A route of its own, served only by a helm that understands the contract
/// — see `build_router` for why the request has to be unaskable rather
/// than merely ignorable. Only the browser's automatic reconnect uses it;
/// a click, a reload and a take-control all carry user intent and go to
/// the ordinary path.
pub(crate) async fn term_ws_if_unowned(
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
    AxPath(id): AxPath<String>,
    Query(q): Query<TermQuery>,
) -> impl IntoResponse {
    serve_term_upgrade(state, id, q, upgrade, true)
}

/// The largest WebSocket message, and frame, a terminal socket accepts from
/// the browser: 8 MiB, the protocol's frame cap
/// ([`farhelm_proto::MAX_FRAME_LEN`]).
///
/// A browser message is at most `GRID_MAX_INPUT_MESSAGE` (64 KiB), and one
/// over that ends the attachment when it arrives
/// (`SupervisorClient::send_grid_input`), so this only bounds what the
/// socket layer reads before that check, and what it reserves for a frame's
/// declared length. Kept at the frame cap rather than tightened to the grid
/// bound: a message between the two is then refused by the grid bound, with
/// a detach notice naming it, instead of by the socket layer, whose error
/// closes the socket with nothing said.
const TERM_WS_MAX_MESSAGE: usize = farhelm_proto::MAX_FRAME_LEN as usize;

/// The upgrade both terminal routes share; `if_unowned` is the one thing
/// they differ in.
fn serve_term_upgrade(
    state: Arc<AppState>,
    id: String,
    q: TermQuery,
    upgrade: WebSocketUpgrade,
    if_unowned: bool,
) -> axum::response::Response {
    // Kept for the log line below, which runs after `id` has moved into
    // the handler.
    let id_for_log = id.clone();
    upgrade
        .protocols([crate::auth::WS_PROTOCOL])
        // Both bounds, not just the message: a peer can otherwise send one
        // enormous frame or a run of small ones assembled into one message,
        // and only one of those is what `max_message_size` alone stops. The
        // tungstenite 0.29 default frame bound is 16 MiB, and its frame
        // reader reserves the declared length before any payload arrives.
        .max_frame_size(TERM_WS_MAX_MESSAGE)
        .max_message_size(TERM_WS_MAX_MESSAGE)
        .on_upgrade(move |socket| async move {
            if let Err(e) = serve_term(state, id, q, socket, if_unowned).await {
                // A refused non-displacing attach is an ORDINARY outcome,
                // not a fault: a browser probing every thirty seconds for a
                // session someone else holds is the reconnect ladder
                // working exactly as designed (PLAN_M6.md item 7), and
                // logging each probe at ERROR would fill the helm's log
                // with the sound of nothing going wrong. It is still
                // logged, because SPEC.md lists reconnection among the
                // things that must be observable — just at the level the
                // event deserves.
                //
                // Classified by DOWNCAST, the same way `http_error` reads a
                // supervisor failure's kind, rather than by comparing the
                // rendered message: `{e:#}` folds in every `.context(...)`
                // layer, so a caller adding one anywhere above this would
                // silently promote a routine refusal back to ERROR. Only the
                // kind (`ErrorKind::TakenOver`) is checked: the message is
                // user-facing prose and decides nothing.
                if refused_as_taken_over(&e) {
                    info!(session = %id_for_log, "terminal reconnect refused: another client holds this session");
                } else {
                    // A refusal can carry host text into the operator's terminal.
                    error!(error = %crate::manager::peer_text(&format!("{e:#}")), "terminal websocket ended with error");
                }
            }
        })
}

/// How long a terminal WebSocket's outbound drain gets to deliver its
/// final detach notice once the handler is unwinding.
///
/// SPEC.md requires a takeover — and now a stall — to be visibly itself
/// rather than a bare connection close, and that notice is one small text
/// frame, so this only has to cover a socket that is working. A socket
/// that is NOT working is the case this bound exists for: the browser
/// that stopped reading is precisely the one whose detach is being
/// delivered, and waiting on it indefinitely would reinstate the pin the
/// detach just removed.
const WS_TEARDOWN_GRACE: std::time::Duration = std::time::Duration::from_secs(5);

/// The test-only control message that panics the inbound task, for
/// exercising its panic cleanup. Checked before the shared parse, so the
/// shared [`TerminalClientMessage`] carries no test variant and malformed
/// client input stays harmless in shipped builds.
#[cfg(test)]
const TEST_PANIC_TEXT_MESSAGE: &str = r#"{"type":"panic"}"#;

/// Pump one attached terminal between the browser and the supervisor.
///
/// The body of the terminal path, and deliberately the dumbest part of it:
/// frames are never inspected, buffered, or transformed in either
/// direction. Everything a client draws comes from the supervisor's grid
/// messages, and every input message reaches it unedited — that is what "full fidelity" in SPEC.md
/// costs here, and any parsing added at this layer would break it.
///
/// The socket always outlives its attachment by exactly one message: when
/// the supervisor ends the attachment (takeover, dead terminal), the
/// detach notice goes out *before* the close, because a bare close renders
/// as a generic "connection closed" and SPEC.md requires a takeover to be
/// visibly a takeover. `detach` runs on every exit path, including an inbound
/// task panic, so the supervisor never keeps an attachment alive for a browser
/// that is gone.
///
/// # Why two tasks
///
/// Inbound (browser → supervisor) and outbound (supervisor → browser) run
/// as separate tasks rather than two arms of one `select!`. Since the
/// helm's outbound queue became bounded, every inbound forward — input and
/// resize — can park waiting for capacity, and in a single
/// loop that parking also stops draining terminal events. That is the
/// worst possible coupling: a big paste blocks output delivery, the
/// per-terminal queue backs up, and a perfectly healthy viewer trips the
/// stalled-terminal detach. Splitting them means a blocked inbound send
/// cannot starve the outbound drain.
///
/// Inbound ORDER is preserved regardless: every inbound message
/// originates from this one WebSocket read loop and are forwarded from it
/// in arrival order. Only the outbound drain moved.
async fn serve_term(
    state: Arc<AppState>,
    session_id: String,
    q: TermQuery,
    mut socket: ws::WebSocket,
    if_unowned: bool,
) -> anyhow::Result<()> {
    use futures_util::{SinkExt, StreamExt};

    // One arm covers both failure sources `attach_from_query` can produce
    // — a locally-refused query shape (an explicit empty `?lease=`) and a
    // supervisor-side attach refusal (unknown session, unknown tab, a
    // terminal that cannot be read) — because both must reach the user
    // identically: a `{"type":"detached",...}` notice, then a closed
    // socket, never a bare disconnect the browser would blame on the
    // network instead of the request it just made. See
    // `attach_from_query`'s own docs for why folding them into one
    // `Result` is what keeps this a single arm instead of two copies of
    // the same three lines.
    let (client, bare_session_id, channel, mut events) =
        match attach_from_query(&state, &session_id, &q, if_unowned).await {
            Ok(parts) => parts,
            Err(e) => {
                let code = if refused_as_taken_over(&e) {
                    farhelm_proto::DetachCode::TakenOver
                } else if refused_as_missing_tab(&e, &q) {
                    farhelm_proto::DetachCode::TabClosed
                } else {
                    farhelm_proto::DetachCode::Other
                };
                let notice = detached_notice(&crate::client::Detach {
                    reason: format!("{e:#}"),
                    code,
                });
                // Bound this notice because a peer that cannot accept a
                // hundred-byte notice within the grace period is one the
                // reason would never have reached anyway.
                let _ =
                    tokio::time::timeout(WS_TEARDOWN_GRACE, socket.send(control_frame(&notice)))
                        .await;
                return Err(e);
            }
        };

    let (mut ws_tx, mut ws_rx) = socket.split();

    // The detach signal, watched independently of the event queue. This is
    // the priority path that makes teardown always possible: a browser
    // that has stopped reading can block the `ws_tx.send` below
    // indefinitely, and without a way to abandon that send, a stall detach
    // would leave this handler, its queued frames, and the attachment
    // itself pinned for as long as the wedge lasted — the very leak the
    // stall detach exists to prevent.
    let mut detach_signal = events.detach_signal();
    // The heartbeat's return path (PLAN_M6.md item 7). The inbound half
    // reads the ping, but `ws_tx` belongs to the outbound task — and it has
    // to stay there: two halves writing to one sink is exactly the
    // interleaving that would let a pong land in the middle of a terminal
    // frame. So the ping crosses as a bare signal and the answer is written
    // where every other outbound message is written.
    //
    // `Notify` rather than a channel, because coalescing IS the wanted
    // behavior rather than a limit to configure: a ping arriving while an
    // earlier pong is still unsent means the outbound half is already
    // stuck, and a second answer to a socket that has not taken the first
    // one adds nothing. `notify_one` collapses those into the single
    // pending permit — the same drop, without a queue depth to justify or a
    // sender to keep alive so the receiver does not spin.
    let pong = Arc::new(tokio::sync::Notify::new());
    let pong_outbound = Arc::clone(&pong);
    // Held until this handler returns, so the attachment trace reports this
    // attachment starting here and ending exactly once, however it ends.
    let _traced = TracedAttachment::start(&session_id, q.tab.as_deref());
    let mut outbound = tokio::spawn(async move {
        loop {
            // A pong does not queue behind the terminal's events: a
            // heartbeat is only ever sent when that queue has been silent,
            // so parking the answer behind it would answer late precisely
            // when the answer is being timed. It still goes out through the
            // same detach-racing send below as everything else, which is
            // what keeps a wedged browser from pinning this handler on a
            // one-frame write — and through the same sink, since two halves
            // writing to one socket is exactly the interleaving that would
            // let a pong land in the middle of a terminal frame.
            let next = tokio::select! {
                event = events.recv() => match event {
                    Some(event) => Some(event),
                    None => break,
                },
                // `None` stands for the pong: the signal carries no
                // payload, because the only thing a ping conveys is that it
                // arrived.
                _ = pong_outbound.notified() => None,
            };
            let message = match next {
                None => control_frame(&TerminalServerMessage::Pong),
                Some(TermEvent::Data(bytes)) => ws::Message::Binary(bytes.into()),
                // The catch-up boundary (PLAN_M5.md item 4). Built as an
                // ordinary outbound `Message` rather than sent inline like
                // `Detached` below, deliberately: it must go through the
                // SAME `select!` — racing the browser's detach signal —
                // as a `Data` message would, so a viewer that vanished
                // between the marker and this send abandons it exactly
                // like abandoned data, instead of the marker getting a
                // priority path data never had.
                Some(TermEvent::ReplayComplete) => {
                    control_frame(&TerminalServerMessage::ReplayComplete)
                }
                Some(TermEvent::Detached(reason)) => {
                    let notice = detached_notice(&reason);
                    // Bound this notice because a peer that cannot accept a
                    // hundred-byte notice within the grace period is one the
                    // reason would never have reached anyway.
                    let _ =
                        tokio::time::timeout(WS_TEARDOWN_GRACE, ws_tx.send(control_frame(&notice)))
                            .await;
                    break;
                }
            };
            tokio::select! {
                sent = ws_tx.send(message) => {
                    if sent.is_err() {
                        break;
                    }
                }
                reason = detach_signal.detached() => {
                    // Abandon the in-flight send along with everything
                    // still queued behind it: this viewer is gone, and the
                    // backlog is exactly the data it already proved it was
                    // not reading.
                    let reason =
                        reason.unwrap_or_else(|| crate::client::Detach::other("detached"));
                    let notice = detached_notice(&reason);
                    // Bound this notice because the peer was just proven not
                    // to read, and a peer that cannot accept a hundred-byte
                    // notice within the grace period is one the reason would
                    // never have reached anyway.
                    let _ = tokio::time::timeout(
                        WS_TEARDOWN_GRACE,
                        ws_tx.send(control_frame(&notice)),
                    )
                    .await;
                    break;
                }
            }
        }
    });

    let client_inbound = Arc::clone(&client);
    // Resizes belong to this attachment's original connection and bare wire ID.
    let session_id_inbound = bare_session_id;
    let inbound = tokio::spawn(async move {
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(ws::Message::Binary(bytes)) => {
                    // The attachment is over: this message was over the
                    // bound and ended it (the reason is published for the
                    // outbound half to deliver), or it had already ended.
                    // Stop reading the browser. `send_grid_input` forwards
                    // nothing for an ended attachment either way.
                    if client_inbound
                        .send_grid_input(channel, bytes.to_vec())
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                #[cfg(test)]
                Ok(ws::Message::Text(text)) if text.as_str() == TEST_PANIC_TEXT_MESSAGE => {
                    panic!("test inbound terminal task panic")
                }
                Ok(ws::Message::Text(text)) => {
                    match serde_json::from_str::<TerminalClientMessage>(&text) {
                        Ok(TerminalClientMessage::Resize { cols, rows }) => {
                            client_inbound
                                .resize(&session_id_inbound, channel, cols, rows)
                                .await;
                        }
                        // The one client message answered HERE rather than
                        // relayed (see `term_ws`'s docs). Notifying never parks,
                        // which is the property this loop needs: the browser
                        // that is not draining is exactly the one whose ping
                        // would otherwise block every keystroke queued behind
                        // it.
                        Ok(TerminalClientMessage::Ping) => pong.notify_one(),
                        // Unparseable or unknown: ignored on purpose, so a
                        // newer browser bundle talking to an older helm
                        // degrades rather than dropping the terminal. That
                        // includes a `pause` or `resume`, which no page sends
                        // now: the supervisor keeps one paused flag per
                        // attachment, and the helm's own flow control
                        // (`SupervisorClient::spawn_grid_flow`) owns it.
                        Err(_) => {}
                    }
                }
                Ok(ws::Message::Close(_)) => break,
                Ok(_) => {} // ping/pong handled by axum
                // Surfaced, not swallowed: an oversized message or a
                // protocol error here is otherwise invisible to both the
                // user (generic "connection closed") and the log.
                Err(e) => {
                    return Err(anyhow::Error::new(e).context("terminal websocket receive failed"));
                }
            }
        }
        anyhow::Ok(())
    });
    let mut inbound = inbound;

    // Either half ending must end the whole handler, and the outbound arm
    // is the one that matters for teardown. A browser that stops reading
    // never closes its socket and never sends anything, so the inbound
    // loop alone would wait forever — pinning this handler, its socket,
    // and every frame queued for it for exactly as long as the wedge
    // lasts. That is the leak the stall detach exists to end, so the
    // detach has to be able to end this handler by itself.
    enum SocketEnd {
        Inbound(Result<anyhow::Result<()>, tokio::task::JoinError>),
        Outbound,
    }
    let end = tokio::select! {
        result = &mut inbound => SocketEnd::Inbound(result),
        _ = &mut outbound => SocketEnd::Outbound,
    };

    let (result, outbound_finished, inbound_finished) = match end {
        SocketEnd::Inbound(result) => {
            // Keep the panic as the returned result, but do not return before
            // the detach below: the supervisor attachment must be cleaned up
            // even when the browser-to-supervisor task panics.
            let result =
                match result {
                    Ok(result) => result,
                    Err(join) => Err(anyhow::Error::new(join)
                        .context("terminal websocket inbound task panicked")),
                };
            (result, false, true)
        }
        SocketEnd::Outbound => (Ok(()), true, false),
    };
    if !inbound_finished {
        inbound.abort();
        let _ = inbound.await;
    }

    // Let detach finish within the same grace as forced teardown. The
    // independently owned upstream send survives this wait, so timing out
    // bounds this handler without dropping the supervisor notification.
    detach_bounded(&client, channel).await;
    settle_outbound(outbound, outbound_finished, WS_TEARDOWN_GRACE).await;
    result
}

/// Bound local detach after browser I/O is gone. Once the local attachment
/// is removed, the client's independent task owns upstream notification;
/// this timeout cannot cancel a send waiting for writer capacity.
async fn detach_bounded(client: &SupervisorClient, channel: u32) {
    if tokio::time::timeout(WS_TEARDOWN_GRACE, client.detach(channel))
        .await
        .is_err()
    {
        tracing::warn!(
            channel,
            "supervisor detach timed out during terminal teardown"
        );
    }
}

/// Let a terminal socket's outbound drain finish, aborting it past
/// `grace` — and never polling its `JoinHandle` more than once past
/// completion.
///
/// That last clause is the entire reason this is a function rather than
/// three lines at the call site. `tokio::JoinHandle`'s documented contract
/// is that polling it after it has already returned `Ready` panics — it is
/// not a fused future — and the teardown it belongs to had two independent
/// ways to do exactly that: the `select!` above can be what drives the
/// handle to completion (the supervisor ended the attachment first), and so
/// can the timeout below (the ordinary case — the browser navigated away,
/// the drain sent its detach notice and stopped). Either one left the old
/// `timeout(...); handle.await` pair polling a spent handle, so a plain page
/// navigation printed "JoinHandle polled after completion" into the helm's
/// log on the way out.
///
/// `already_finished` is the caller's report of the first case; the second
/// is handled by only awaiting after an `abort`, which is the one path where
/// the handle is known to still be outstanding.
async fn settle_outbound(
    mut outbound: tokio::task::JoinHandle<()>,
    already_finished: bool,
    grace: std::time::Duration,
) {
    if already_finished {
        return;
    }
    if tokio::time::timeout(grace, &mut outbound).await.is_err() {
        outbound.abort();
        // Safe to await: the timeout expired, so nothing has taken this
        // handle's output yet, and the abort only makes it resolve sooner.
        let _ = outbound.await;
    }
}

#[cfg(test)]
mod tests {
    /// Keep a qualified URL alive through the connection future, including concurrent peer setup.
    async fn connect_session(
        harness: &crate::rest_harness::Harness,
        addr: std::net::SocketAddr,
        path: &str,
    ) -> WsTestClient {
        WsTestClient::connect(addr, &session_path(harness, path)).await
    }

    /// Build this fixture's public reference from its registered entry, keeping wire IDs bare.
    fn session_path(harness: &crate::rest_harness::Harness, path: &str) -> String {
        let prefix = "/api/sessions/";
        let Some(suffix) = path.strip_prefix(prefix) else {
            return path.to_string();
        };
        let entry = harness
            .manager
            .snapshots()
            .into_iter()
            .find(|host| host.kind == crate::store::HostKind::Local)
            .expect("the fixture has a local host")
            .entry_uuid;
        format!("{prefix}{entry}:{suffix}")
    }

    use crate::BUILD_STAMP_HEADER;
    use crate::client::SupervisorClient;
    use crate::rest_harness::{self, WsTestClient};
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};
    use farhelm_proto::{ControlMsg, Frame};
    use std::time::Duration;
    use tower::ServiceExt;

    /// A terminal from an old browser tab must explain its stale reference on the socket,
    /// without attaching the live session whose bare suffix happens to match.
    #[farhelm_testtrace::test]
    async fn bare_terminal_target_is_refused_before_supervisor_dispatch() {
        let (client, server) = tokio::io::duplex(64 * 1024);
        let (observe, peer) = rest_harness::silent_supervisor(server);
        let mut harness = rest_harness::spliced_helm(client).await;
        let host = harness
            .manager
            .snapshots()
            .into_iter()
            .find(|host| host.kind == crate::store::HostKind::Local)
            .unwrap()
            .id;
        crate::sessions::route_session(
            &harness.state,
            &crate::store::SessionKey::new(host, "sess-1"),
        )
        .await
        .expect("premise: the matching bare suffix is routable");
        let addr = harness.serve().await;
        let mut ws = WsTestClient::connect(addr, "/api/sessions/sess-1/term").await;
        let (opcode, bytes) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("a refused attach must explain why")
            .unwrap();
        assert_eq!(opcode, 1);
        let notice: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(notice["type"], "detached");
        assert!(
            notice["reason"]
                .as_str()
                .unwrap()
                .contains("full session ID")
        );
        observe.send(()).unwrap();
        peer.await.unwrap();
    }

    /// Public references stop at attachment admission. A later browser resize must use the
    /// same connection and bare ID, or an initially working terminal fails only after resizing.
    #[farhelm_testtrace::test]
    async fn qualified_terminal_attach_and_later_resize_keep_bare_wire_ids() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let mut peer = rest_harness::FakeSupervisor::accept(peer_side).await;
            let ControlMsg::Attach {
                req_id,
                session_id,
                channel,
                ..
            } = peer.recv().await
            else {
                panic!("expected terminal admission");
            };
            assert_eq!(session_id, "sess-1");
            peer.send(&ControlMsg::Attached { req_id, channel }).await;
            peer.send_frame(&Frame::data(channel, b"ready".to_vec()))
                .await;
            let ControlMsg::Resize {
                session_id,
                channel: resized,
                cols,
                rows,
            } = peer.recv().await
            else {
                panic!("expected resize on the attached connection");
            };
            assert_eq!(session_id, "sess-1");
            assert_eq!(resized, channel);
            assert_eq!((cols, rows), (101, 43));
        });
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let mut ws = connect_session(&harness, addr, "/api/sessions/sess-1/term").await;
        let (opcode, bytes) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("attached terminal must deliver its live bytes")
            .unwrap();
        assert_eq!(opcode, 2);
        assert_eq!(bytes, b"ready");
        ws.send_text(r#"{"type":"resize","cols":101,"rows":43}"#)
            .await;
        tokio::time::timeout(Duration::from_secs(5), peer)
            .await
            .expect("resize must reach the attached supervisor")
            .unwrap();
    }

    /// A client that forgets to request an upgrade must receive Axum's
    /// ordinary missing-upgrade response, not an internal error. The
    /// authenticated request matters: without it, middleware would reject the
    /// request before handler extraction.
    #[farhelm_testtrace::test]
    async fn an_authenticated_plain_terminal_get_is_not_an_internal_error() {
        let harness = rest_harness::idle_helm().await;
        let request = Request::builder()
            .uri(session_path(&harness, "/api/sessions/sess-1/term"))
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = harness.router().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = String::from_utf8(to_bytes(response.into_body(), 4096).await.unwrap().to_vec())
            .unwrap();
        assert!(
            !body.contains("AuthenticatedSocket"),
            "unexpected body: {body}"
        );
    }
    /// Every exit shape of a terminal socket's teardown must leave its
    /// outbound drain settled WITHOUT ever polling a spent `JoinHandle`.
    ///
    /// This is a regression test for a live bug, not a test of tokio: the
    /// old teardown polled the handle in the `select!` and then again in
    /// the timeout and again in a trailing `await`, so an ordinary page
    /// navigation — the drain finishing inside the grace period — panicked
    /// the connection task with "JoinHandle polled after completion". A
    /// panic there is invisible from the browser side (the socket is
    /// already closing either way), which is exactly why it survived until
    /// someone read the webserver log, and why the property is pinned here
    /// at the seam instead of end to end.
    ///
    /// All three shapes run, because each reaches the handle differently:
    /// already-driven-to-completion by the caller, completing inside the
    /// grace, and never completing at all (the wedged browser the grace
    /// exists for).
    #[farhelm_testtrace::test]
    async fn outbound_teardown_never_polls_a_finished_join_handle() {
        // Driven to completion by the caller, exactly as the `select!`
        // arm does before reporting `already_finished`.
        let mut handle = tokio::spawn(async {});
        (&mut handle).await.expect("the task cannot panic");
        super::settle_outbound(handle, true, Duration::from_secs(5)).await;

        // Finishes on its own inside the grace: the ordinary navigation
        // case, and the one the old code panicked on.
        let handle = tokio::spawn(async {
            // sleep-ok: make the drain initially pending, with completion intended to fall within teardown's grace.
            tokio::time::sleep(Duration::from_millis(20)).await;
        });
        super::settle_outbound(handle, false, Duration::from_secs(5)).await;

        // Never finishes: aborted past the grace, which must also not
        // leave this call hanging.
        let handle = tokio::spawn(async {
            std::future::pending::<()>().await;
        });
        let start = tokio::time::Instant::now();
        super::settle_outbound(handle, false, Duration::from_millis(50)).await;
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "a wedged drain must be abandoned at the grace, not waited on"
        );
    }

    /// Normal teardown bounds its wait while preserving the real client's
    /// independently owned upstream send under writer backpressure.
    ///
    /// The peer first completes a real attach, then stops reading while
    /// terminal input fills the client's bounded writer queue. The helper
    /// must return at the teardown grace; once the peer resumes reading, it
    /// must still receive the Detach that the timed-out caller no longer owns.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn ordinary_detach_wait_is_bounded_by_the_teardown_grace() {
        use farhelm_proto::TerminalSelector;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (read, write) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(read);
            let mut writer = FrameWriter::new(write);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .expect("peer handshake");
            let request = farhelm_proto::io::parse_control(
                &reader
                    .read_frame()
                    .await
                    .expect("attach read")
                    .expect("peer remains connected"),
            )
            .expect("attach control");
            let ControlMsg::Attach {
                req_id, channel, ..
            } = request
            else {
                panic!("expected Attach, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Attached { req_id, channel })
                .await
                .expect("attach reply");

            release_rx.await.expect("test releases the stalled peer");
            loop {
                let frame = reader
                    .read_frame()
                    .await
                    .expect("peer read")
                    .expect("peer remains connected");
                if frame.kind == farhelm_proto::FrameKind::Control
                    && matches!(
                        farhelm_proto::io::parse_control(&frame).expect("control frame"),
                        ControlMsg::Detach { channel: got } if got == channel
                    )
                {
                    return channel;
                }
            }
        });
        let (read, write) = tokio::io::split(client_side);
        let client = SupervisorClient::start(read, write)
            .await
            .expect("client handshake");
        let (channel, _events) = client
            .attach_terminal("sess-1", 80, 24, TerminalSelector::default(), "")
            .await
            .expect("terminal attach");

        // Poll until the real bounded writer has accepted all available
        // capacity and the remaining input is waiting for the stalled peer.
        let message = vec![b'x'; farhelm_proto::grid::GRID_MAX_INPUT_MESSAGE];
        let mut input = Box::pin(async {
            while client
                .send_grid_input(channel, message.clone())
                .await
                .is_ok()
            {}
        });
        let parked = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(std::future::Future::poll(input.as_mut(), cx).is_pending())
        })
        .await;
        assert!(parked, "the scripted peer must backpressure the writer");

        super::detach_bounded(&client, channel).await;
        drop(input);
        release_tx.send(()).expect("peer is waiting for release");
        let detached = tokio::time::timeout(Duration::from_secs(5), peer)
            .await
            .expect("the independent detach send was lost after caller timeout")
            .expect("peer task panicked");
        assert_eq!(detached, channel);
    }

    /// For a completed initial attach catch-up, the marker's ordering
    /// property is: it must reach the browser AFTER the binary replay
    /// bytes it describes and BEFORE any binary live byte, on the exact
    /// same socket (PLAN_M5.md item 4) — not the marker's whole contract,
    /// which also covers attaches a takeover/detach/stall ends before a
    /// marker is owed, and the markerless history resend after a reset (see
    /// `TermEvent::ReplayComplete`'s own docs); neither is this test's
    /// concern. This is the real-socket complement to
    /// `client::tests::replay_complete_marker_is_ordered_between_replay_and_live_data_in_the_queue`
    /// — that test pins the ordering inside `SupervisorClient`'s queue;
    /// this one pins that `serve_term`'s WS forwarding does not reorder or
    /// drop the marker on the way to a real `WsTestClient`, and that it
    /// arrives as the documented `{"type":"replay_complete"}` text frame
    /// rather than, say, folded into the binary stream.
    #[farhelm_testtrace::test]
    async fn term_ws_delivers_the_replay_complete_marker_between_replay_and_live_bytes() {
        use farhelm_proto::ControlMsg;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            connect_session(&harness, addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (_reader, mut writer, channel) = peer.unwrap();

        writer
            .write_frame(&farhelm_proto::Frame::data(channel, b"replay".to_vec()))
            .await
            .unwrap();
        writer
            .write_control(&ControlMsg::ReplayComplete { channel })
            .await
            .unwrap();
        writer
            .write_frame(&farhelm_proto::Frame::data(channel, b"live".to_vec()))
            .await
            .unwrap();

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no replay data arrived")
            .expect("socket closed before the replay data");
        assert_eq!(opcode, 2, "replay bytes are a binary frame");
        assert_eq!(payload, b"replay");

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no replay-complete marker arrived")
            .expect("socket closed before the marker");
        assert_eq!(opcode, 1, "the marker is a text frame");
        let notice: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(
            notice,
            serde_json::json!({"type": "replay_complete"}),
            "the marker must be exactly this fixed object — no stray fields (a channel, say, \
             which the socket has no need to name since it IS the channel) and no missing ones"
        );

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no live data arrived")
            .expect("socket closed before the live data");
        assert_eq!(opcode, 2, "live bytes are a binary frame");
        assert_eq!(
            payload, b"live",
            "live output must follow the marker, not race ahead of it"
        );
    }

    /// The attachment trace reports an admitted attachment starting, with the
    /// session as the browser named it and `terminal=agent`, and reports its
    /// end once when the browser closes the socket.
    ///
    /// Why: the desktop smoke test tells which session the relaunched window
    /// has open only through these two events, since a terminal in a terminal host shows no
    /// viewer anywhere outside the supervisor; a trace that missed the start,
    /// or reported the end twice or never, would pass or fail those checks
    /// for the wrong reason. Spec: after an attach of `<entry>:sess-1`'s
    /// agent terminal, exactly one `terminal attached` event on the
    /// `farhelm_helm::terminal_attachments` target names that session and
    /// `agent` by the time the attachment's first data frame arrives, and
    /// none says it ended; after the browser drops the socket, exactly one
    /// `terminal ended` event names it, within a bound.
    #[farhelm_testtrace::test]
    async fn the_attachment_trace_reports_an_attachment_start_and_end_once() {
        let events = crate::test_capture::current();
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let path = session_path(&harness, "/api/sessions/sess-1/term");
        let session = path
            .strip_prefix("/api/sessions/")
            .and_then(|rest| rest.strip_suffix("/term"))
            .expect("a session path")
            .to_string();
        let traced = |message: &str| {
            crate::test_capture::matching(&events, message)
                .into_iter()
                .filter(|event| {
                    event.target == super::ATTACHMENT_TRACE
                        && event.field("session") == Some(session.as_str())
                        && event.field("terminal") == Some("agent")
                })
                .count()
        };
        let (mut ws, peer) = tokio::join!(WsTestClient::connect(addr, &path), peer);
        let (_reader, mut writer, channel) = peer.unwrap();
        // The handler traces the start before it relays anything, so the
        // first frame reaching the browser is the point by which it has.
        writer
            .write_frame(&Frame::data(channel, b"first".to_vec()))
            .await
            .unwrap();
        let (opcode, _) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("the first frame never arrived")
            .expect("socket closed before the first frame");
        assert_eq!(opcode, 2);
        assert_eq!(traced("terminal attached"), 1);
        assert_eq!(traced("terminal ended"), 0);

        drop(ws);
        let ended = tokio::time::timeout(Duration::from_secs(10), async {
            while traced("terminal ended") == 0 {
                tokio::time::sleep(Duration::from_millis(20)).await; // sleep-ok: bounded polling for the handler's teardown, which has no other observable here
            }
        })
        .await;
        assert!(
            ended.is_ok(),
            "the trace never reported the attachment ending"
        );
        assert_eq!(traced("terminal ended"), 1);
        assert_eq!(traced("terminal attached"), 1);
    }

    /// Successful attachments can carry host-supplied tab IDs, so both trace
    /// events must escape controls even when no error or refusal occurred.
    #[farhelm_testtrace::test]
    async fn the_attachment_trace_escapes_peer_tab_controls() {
        let events = crate::test_capture::current();
        let traced = super::TracedAttachment::start("sess-1", Some("shell\n\u{1b}[2J\t\r\0"));
        drop(traced);
        for message in ["terminal attached", "terminal ended"] {
            let matches: Vec<_> = crate::test_capture::matching(&events, message)
                .into_iter()
                .filter(|event| event.target == super::ATTACHMENT_TRACE)
                .collect();
            assert_eq!(matches.len(), 1, "{message}: {matches:?}");
            assert_eq!(
                matches[0].field("terminal"),
                Some(r#"tab:"shell\n\u{1b}[2J\t\r\0""#),
                "{message}: peer controls must remain printable log data"
            );
        }
    }

    /// A terminal peer declaring an oversized frame is refused before its
    /// payload arrives, so the WebSocket cannot reserve the frame's declared
    /// length while waiting for bytes that never come.
    #[farhelm_testtrace::test]
    async fn an_oversized_terminal_frame_is_refused_before_its_bytes_arrive() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            connect_session(&harness, addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (_reader, _writer, _channel) = peer.unwrap();

        // The header declares more than the socket's cap, but no payload
        // bytes are sent. Only the frame bound can reject this declaration
        // before the reader waits for or reserves the missing payload.
        ws.send_frame_header(2, super::TERM_WS_MAX_MESSAGE + 1)
            .await;
        let result = tokio::time::timeout(super::WS_TEARDOWN_GRACE, ws.recv())
            .await
            .expect("the oversized frame must be refused within the teardown grace");
        assert!(
            result.is_none(),
            "an oversized terminal frame must close before any payload arrives: {result:?}"
        );
    }

    /// A browser that stops reading must not pin the WebSocket handler:
    /// the stall detach has to end it even while a send to that browser is
    /// blocked.
    ///
    /// This is the teardown half of the detach-not-block design. The
    /// failure it guards is specific and quiet: `serve_term` parked in
    /// `ws_tx.send()` to a browser that stopped reading cannot observe a
    /// detach that arrives through the terminal's data queue, because that
    /// queue is backed up behind the very send it is parked in. Handler,
    /// socket, queued frames, and the notification task would then stay
    /// alive for as long as the wedge lasted, which is exactly the unbounded
    /// pin the stall detach exists to end.
    ///
    /// The premise is a handler parked mid-send with its attachment still
    /// live. The browser socket's receive buffer is kept small, so the
    /// server's sends block after a few MiB, and the flood stops short of
    /// the helm's own overflow detach, which would end the attachment by
    /// another path. Both are observed: the stand-in stops at the helm's own
    /// `PauseOutput`, sent only once the attachment's queue reached its pause
    /// mark (so the handler is not draining it), and that pause must be the
    /// first thing it hears, so no overflow detach came first.
    ///
    /// The oracle is the handler's own end. The client acknowledges the
    /// supervisor's stalled `Detached` with one `Detach` of its own at once,
    /// in addition to the handler's; a second
    /// `Detach` comes only from `serve_term`'s teardown, after its outbound
    /// half got past the blocked send (racing the detach signal) and past
    /// the notice it then tries to send to the same browser (bounded by the
    /// grace). The browser is not read before that, so a regression in
    /// either hangs here instead of passing. Spec: after the stalled
    /// `Detached` the supervisor receives exactly two `Detach` frames for the
    /// channel and nothing else, within twice the teardown grace plus
    /// headroom, and the browser socket then closes.
    #[farhelm_testtrace::test]
    async fn a_wedged_browser_is_torn_down_by_the_stall_detach() {
        let (client_side, peer_side) = tokio::io::duplex(1024 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let path = session_path(&harness, "/api/sessions/sess-1/term");
        let (mut ws, peer) = tokio::join!(
            WsTestClient::connect_with_receive_buffer(addr, &path, SMALL_RECEIVE_BUFFER),
            peer
        );
        let (reader, mut writer, channel) = peer.unwrap();

        // Read exactly one frame, proving the socket works, and then stop
        // reading until the handler has ended — the wedged browser.
        writer
            .write_frame(&Frame::data(channel, b"first".to_vec()))
            .await
            .unwrap();
        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no first frame")
            .expect("socket closed early");
        assert_eq!((opcode, payload.as_slice()), (2, &b"first"[..]));

        let flood = flood_until_heard(reader, &mut writer, channel, 32 * 1024, 0).await;
        assert!(
            is_control(&flood.heard, &ControlMsg::PauseOutput { channel }),
            "fixture premise: the stream must be paused, and not detached, before the stall \
             detach is injected: {:?} after {} frames",
            flood.heard,
            flood.written
        );
        let mut reader = flood.reader;

        writer
            .write_control(&ControlMsg::Detached {
                channel,
                reason: farhelm_proto::DETACH_REASON_STALLED.to_string(),
                code: farhelm_proto::DetachCode::Stalled,
            })
            .await
            .unwrap();

        // The bound is deliberately wider than one grace period: a blocked
        // notice consumes one grace before the handler can detach upstream,
        // then the detach round trip still needs headroom on a loaded machine.
        //
        // Two, because the client acknowledges the supervisor's `Detached`
        // with a `Detach` of its own (`SupervisorClient::dispatch`) and the
        // handler's teardown sends another (`detach_bounded`), and only the
        // second shows the handler got past the blocked send. A cleanup that
        // drops either of them as a duplicate must change this oracle, or the
        // test would wait for a frame that never comes.
        let mut others = Vec::new();
        let ended = tokio::time::timeout(
            super::WS_TEARDOWN_GRACE * 2 + Duration::from_secs(5),
            async {
                let mut detaches = 0;
                while detaches < 2 {
                    let frame = reader
                        .read_frame()
                        .await
                        .expect("the supervisor connection must remain readable")
                        .expect("the supervisor connection closed before detach");
                    if is_control(&frame, &ControlMsg::Detach { channel }) {
                        detaches += 1;
                    } else {
                        others.push(frame);
                    }
                }
            },
        )
        .await;
        assert!(
            ended.is_ok(),
            "the handler never detached in teardown — `serve_term` is still pinned on a send to a \
             browser that stopped reading"
        );
        assert!(
            others.is_empty(),
            "nothing but the two detaches may follow the stall: {others:?}"
        );
        tokio::time::timeout(Duration::from_secs(10), async {
            while let Some((opcode, _)) = ws.recv().await {
                if opcode == 8 {
                    break;
                }
            }
        })
        .await
        .expect("the ended handler must close the browser socket");
    }

    /// A panic in the browser-to-supervisor task must still leave the
    /// supervisor-side attachment cleaned up. The `panic` message exists only
    /// in test builds because deployed clients' malformed messages are
    /// intentionally ignored rather than treated as a fault injection path.
    #[farhelm_testtrace::test]
    async fn a_panicking_inbound_task_is_torn_down_by_detach() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            connect_session(&harness, addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (mut reader, _writer, channel) = peer.unwrap();

        ws.send_text(r#"{"type":"panic"}"#).await;

        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let frame = reader
                    .read_frame()
                    .await
                    .expect("the supervisor connection must remain readable")
                    .expect("the supervisor connection closed before detach");
                let message = farhelm_proto::io::parse_control(&frame).unwrap();
                if matches!(message, ControlMsg::Detach { channel: got } if got == channel) {
                    return;
                }
            }
        })
        .await
        .expect("a panicking inbound task must still detach its supervisor attachment");
    }

    /// Drive a scripted supervisor peer through an attach, returning the
    /// reader/writer halves positioned right after the `Attached` reply.
    ///
    /// Every WebSocket test below needs the same preamble — handshake,
    /// answer one `Attach` — and none of them is about that preamble.
    async fn scripted_supervisor_attach(
        peer_side: tokio::io::DuplexStream,
    ) -> (
        FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
        FrameWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
        u32,
    ) {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request =
            farhelm_proto::io::parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::Attach {
            req_id, channel, ..
        } = request
        else {
            panic!("expected an Attach, got {request:?}");
        };
        writer
            .write_control(&ControlMsg::Attached { req_id, channel })
            .await
            .unwrap();
        (reader, writer, channel)
    }

    /// Data that reached the helm ahead of the supervisor's answer is the
    /// socket's first frame, unchanged.
    ///
    /// Why: the supervisor enqueues `Attached` before it starts the
    /// catch-up, but the catch-up's first frames can already be in the
    /// helm's queue when the caller awaiting the attach resumes, so the
    /// channel is registered before the request goes out. This peer sends
    /// its data even before the reply, a robustness case beyond what a
    /// supervisor does, which a helm that registered the channel only on
    /// the reply would drop. Spec: the supervisor sends a data frame and
    /// then `Attached`; the browser's first frame is that data frame, as a
    /// binary frame, unchanged.
    #[farhelm_testtrace::test]
    async fn data_that_beat_the_attach_reply_reaches_the_browser_first() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request =
                farhelm_proto::io::parse_control(&reader.read_frame().await.unwrap().unwrap())
                    .unwrap();
            let ControlMsg::Attach {
                req_id, channel, ..
            } = request
            else {
                panic!("expected an Attach, got {request:?}");
            };
            writer
                .write_frame(&Frame::data(channel, b"early".to_vec()))
                .await
                .unwrap();
            writer
                .write_control(&ControlMsg::Attached { req_id, channel })
                .await
                .unwrap();
            (reader, writer)
        });
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            connect_session(&harness, addr, "/api/sessions/sess-1/term"),
            peer
        );
        let _peer_halves = peer.unwrap();

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("the early data frame never arrived")
            .expect("socket closed before the early data frame");
        assert_eq!(opcode, 2, "attachment data travels as binary frames");
        assert_eq!(payload, b"early");
    }

    /// Read the supervisor side until `want` names a frame, returning every
    /// frame read before it; a bounded wait, so a frame that never comes
    /// fails the test instead of hanging it.
    async fn read_until(
        reader: &mut FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
        what: &str,
        want: impl Fn(&Frame) -> bool,
    ) -> Vec<Frame> {
        let mut before = Vec::new();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let frame = reader
                    .read_frame()
                    .await
                    .expect("the supervisor connection must stay readable")
                    .expect("the supervisor connection closed");
                if want(&frame) {
                    return;
                }
                before.push(frame);
            }
        })
        .await
        .unwrap_or_else(|_| panic!("the supervisor never received {what}; got {before:?}"));
        before
    }

    /// Whether `frame` is the control message `expected`.
    fn is_control(frame: &Frame, expected: &ControlMsg) -> bool {
        frame.kind == farhelm_proto::FrameKind::Control
            && farhelm_proto::io::parse_control(frame).ok().as_ref() == Some(expected)
    }

    /// A grid stream crosses the helm unread in both directions: large
    /// frames reach the browser whole, each browser message reaches the
    /// supervisor as one frame, and the control messages keep their meaning.
    ///
    /// Why: an attachment carries one grid message per data frame both ways
    /// (`farhelm_proto::grid`, "Framing"), so a frame the helm cut, joined or
    /// refused is a protocol error at the far end; a long row or a long paste
    /// part is ordinary traffic. The large frame is sent before the
    /// `Attached` reply, as a supervisor starts streaming before its reply is
    /// read, so the bound in force before the reply is the one under test.
    /// Spec: the browser receives a 200 KiB data frame byte for byte, then
    /// `replay_complete`; a binary message of exactly the 64 KiB input bound
    /// reaches the supervisor as one identical data frame, and of a resize, a
    /// pause, a resume and a second resize sent after it, only the two
    /// resizes follow, in order: flow control is the helm's own, so a
    /// browser's pause and resume are not forwarded (the second resize shows
    /// they were read and dropped, since the socket's messages are forwarded
    /// in arrival order).
    #[farhelm_testtrace::test]
    async fn a_grid_stream_is_relayed_unread_in_both_directions() {
        let early: Vec<u8> = (0..200 * 1024).map(|i| (i % 251) as u8).collect();
        let early_sent = early.clone();
        let (client_side, peer_side) = tokio::io::duplex(1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request =
                farhelm_proto::io::parse_control(&reader.read_frame().await.unwrap().unwrap())
                    .unwrap();
            let ControlMsg::Attach {
                req_id, channel, ..
            } = request
            else {
                panic!("expected an Attach, got {request:?}");
            };
            writer
                .write_frame(&Frame::data(channel, early_sent))
                .await
                .unwrap();
            writer
                .write_control(&ControlMsg::Attached { req_id, channel })
                .await
                .unwrap();
            writer
                .write_control(&ControlMsg::ReplayComplete { channel })
                .await
                .unwrap();
            (reader, writer, channel)
        });
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            connect_session(&harness, addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (mut reader, _writer, channel) = peer.unwrap();

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("the large grid frame never arrived")
            .expect("socket closed before the large grid frame");
        assert_eq!(opcode, 2, "grid messages travel as binary frames");
        assert!(
            payload == early,
            "the grid frame must arrive whole and unchanged"
        );
        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no replay-complete marker arrived")
            .expect("socket closed before the marker");
        assert_eq!(
            (
                opcode,
                serde_json::from_slice::<serde_json::Value>(&payload).unwrap()
            ),
            (1, serde_json::json!({"type": "replay_complete"}))
        );

        let input: Vec<u8> = (0..farhelm_proto::grid::GRID_MAX_INPUT_MESSAGE)
            .map(|i| b'a' + (i % 26) as u8)
            .collect();
        ws.send_frame(true, 2, &input).await;
        ws.send_text(r#"{"type":"resize","cols":90,"rows":30}"#)
            .await;
        ws.send_text(r#"{"type":"pause"}"#).await;
        ws.send_text(r#"{"type":"resume"}"#).await;
        ws.send_text(r#"{"type":"resize","cols":100,"rows":40}"#)
            .await;

        let frame = tokio::time::timeout(Duration::from_secs(5), reader.read_frame())
            .await
            .expect("the browser's grid input never reached the supervisor")
            .unwrap()
            .expect("connection closed");
        assert_eq!(
            (frame.kind, frame.channel, frame.body.len()),
            (farhelm_proto::FrameKind::Data, channel, input.len()),
            "one browser message must be one data frame on the attachment's channel"
        );
        assert!(frame.body == input, "the input must arrive unchanged");
        for expected in [
            ControlMsg::Resize {
                session_id: "sess-1".to_string(),
                channel,
                cols: 90,
                rows: 30,
            },
            ControlMsg::Resize {
                session_id: "sess-1".to_string(),
                channel,
                cols: 100,
                rows: 40,
            },
        ] {
            let frame = tokio::time::timeout(Duration::from_secs(5), reader.read_frame())
                .await
                .expect("a control message after the input never arrived")
                .unwrap()
                .expect("connection closed");
            assert!(
                is_control(&frame, &expected),
                "expected {expected:?}, got {frame:?}"
            );
        }
    }

    /// A grid input message over the input bound ends the attachment, as
    /// the supervisor would, and nothing the browser sends after it is
    /// forwarded.
    ///
    /// Why: the helm cannot cut a grid message into frames, and forwarding a
    /// browser message whole can exceed what the connection's writer can
    /// encode, which would end every attachment sharing the connection;
    /// dropping it and carrying on would deliver later input without what
    /// came before it. Spec: a binary message of 64 KiB + 1
    /// and then a small one, in one write; the browser gets a `detached`
    /// notice with code `other` whose reason begins "terminal input failed",
    /// then the close; the supervisor receives a `Detach` for the channel and
    /// no data frame.
    #[farhelm_testtrace::test]
    async fn an_oversized_grid_input_message_ends_the_attachment() {
        let (client_side, peer_side) = tokio::io::duplex(1024 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            connect_session(&harness, addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (mut reader, _writer, channel) = peer.unwrap();

        // One write, so the message after the oversized one is already
        // there when the helm reads the first: what is not forwarded is
        // decided by the helm, not by which of them arrived first.
        let oversized = vec![b'x'; farhelm_proto::grid::GRID_MAX_INPUT_MESSAGE + 1];
        ws.send_frames(&[(2, &oversized), (2, br#"{"t":"text","data":"after"}"#)])
            .await;

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no detach notice arrived")
            .expect("socket closed before the detach notice");
        assert_eq!(opcode, 1, "the detach notice is a text frame");
        let notice: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(notice["type"], "detached", "{notice}");
        assert_eq!(notice["code"], "other", "{notice}");
        assert!(
            notice["reason"]
                .as_str()
                .unwrap()
                .starts_with("terminal input failed"),
            "{notice}"
        );
        tokio::time::timeout(Duration::from_secs(5), async {
            while let Some((opcode, _)) = ws.recv().await {
                if opcode == 8 {
                    break;
                }
            }
        })
        .await
        .expect("the socket must close after the notice");

        let before = read_until(&mut reader, "the Detach", |frame| {
            is_control(frame, &ControlMsg::Detach { channel })
        })
        .await;
        assert!(
            before.is_empty(),
            "nothing may reach the supervisor before the Detach: {before:?}"
        );
        // The socket has closed, so `serve_term` has sent everything it ever
        // will; what is left to read is at most its own second `Detach`.
        while let Ok(frame) =
            tokio::time::timeout(Duration::from_millis(200), reader.read_frame()).await
        {
            let frame = frame.unwrap().expect("connection closed");
            assert_eq!(
                frame.kind,
                farhelm_proto::FrameKind::Control,
                "no input may be forwarded after the oversized message: {frame:?}"
            );
        }
    }

    /// The receive buffer a test's wedged or slow browser asks for: small,
    /// so the server's sends block after the kernel's own send buffer
    /// rather than after megabytes of loopback autotuning.
    const SMALL_RECEIVE_BUFFER: u32 = 16 * 1024;

    /// What [`flood_until_heard`] saw.
    struct Flood {
        reader: FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
        /// The first frame the stand-in heard from the helm.
        heard: Frame,
        /// Data frames written, numbered from 0 in their first four bytes.
        written: u32,
    }

    /// Write numbered data frames of `size` bytes on `channel` until the
    /// helm says anything, then `after` more, the way a supervisor's sender
    /// stops at a pause while what is already queued on its connection still
    /// goes out.
    ///
    /// The stand-in reads on a task of its own and the writer stops at its
    /// signal. The writer yields after each frame so the reader and the
    /// helm run between frames on this single-threaded runtime; a bound on
    /// the frames written turns a helm that never answers into a failure
    /// rather than a hang.
    async fn flood_until_heard(
        mut reader: FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
        writer: &mut FrameWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
        channel: u32,
        size: usize,
        after: u32,
    ) -> Flood {
        const MOST_FRAMES: u32 = 4096;
        let (heard_tx, heard_rx) = tokio::sync::watch::channel(false);
        let watcher = tokio::spawn(async move {
            let heard = tokio::time::timeout(Duration::from_secs(30), reader.read_frame())
                .await
                .expect("the supervisor never heard from the helm while the browser was behind")
                .unwrap()
                .expect("connection closed");
            heard_tx.send_replace(true);
            (reader, heard)
        });
        let mut written = 0u32;
        let mut stop_at = None;
        while stop_at.is_none_or(|stop| written < stop) {
            assert!(
                written < MOST_FRAMES,
                "the helm said nothing after {written} frames"
            );
            if stop_at.is_none() && *heard_rx.borrow() {
                stop_at = Some(written + after);
                continue;
            }
            let mut body = vec![0u8; size];
            body[..4].copy_from_slice(&written.to_be_bytes());
            writer
                .write_frame(&Frame::data(channel, body))
                .await
                .unwrap();
            written += 1;
            tokio::task::yield_now().await;
        }
        let (reader, heard) = watcher.await.unwrap();
        Flood {
            reader,
            heard,
            written,
        }
    }

    /// A grid stream whose browser falls behind is paused by the helm, and
    /// resumed once the browser catches up, without losing a frame, at the
    /// supervisor's message size and with its writer queue's worth still
    /// arriving after the pause.
    ///
    /// Why: the grid client sends no pause of its own, and the helm reads
    /// the supervisor connection without waiting on any one terminal, so
    /// without this pause a browser slower than the supervisor fills its
    /// terminal's queue and is detached as stalled at the first large
    /// catch-up or flood. What was already on its way when the pause went
    /// out still arrives, and must fit. Spec: a stand-in supervisor writes
    /// 32 KiB frames (its sender's packing) to a browser that is not
    /// reading, and 64 more (its writer queue) once it hears from the helm;
    /// what it hears first is `PauseOutput` for the channel, not a `Detach`.
    /// The browser then reads every frame written, in order, after which the
    /// supervisor receives `ResumeOutput` for the channel and nothing before
    /// it.
    #[farhelm_testtrace::test]
    async fn a_slow_browser_pauses_and_resumes_its_grid_stream() {
        slow_browser_pauses_and_resumes(32 * 1024, 64, false).await;
    }

    /// A browser's own resume and pause, sent while the
    /// helm holds the stream paused, neither lift the helm's pause nor
    /// reach the supervisor; the helm still resumes the stream itself once
    /// the browser catches up.
    ///
    /// Why: the supervisor keeps one paused flag per attachment. A
    /// forwarded browser resume would lift the pause the helm needs to keep
    /// its queue within bounds, and a forwarded pause the helm never sent
    /// would hold the stream paused until the stall detach, since the helm
    /// resumes only its own pauses. Spec: as the 32 KiB case, but once the
    /// helm's `PauseOutput` is heard the browser sends `resume`, `pause`
    /// and then a resize; the supervisor's next message is that `Resize`,
    /// with nothing before it, and after the browser reads every frame the
    /// helm's `ResumeOutput` still arrives with nothing before it.
    #[farhelm_testtrace::test]
    async fn a_browser_pause_or_resume_never_touches_a_grid_streams_flow() {
        slow_browser_pauses_and_resumes(32 * 1024, 64, true).await;
    }

    /// The same, with rows at the format's largest message and several of
    /// them still arriving after the pause.
    ///
    /// Why: a row that does not fit the supervisor's 32 KiB packing goes
    /// alone in a message of up to 1 MiB, so what follows a pause can be
    /// megabytes in a few frames, which is what the helm's byte budget has
    /// to absorb while its event count barely moves. Spec: as above with
    /// 1 MiB frames and 8 of them after the stand-in hears from the helm.
    #[farhelm_testtrace::test]
    async fn a_slow_browser_survives_large_rows_arriving_after_the_pause() {
        slow_browser_pauses_and_resumes(farhelm_proto::grid::GRID_MAX_MESSAGE, 8, false).await;
    }

    /// One run of the slow-browser tests: frames of `size`, and `after`
    /// more once the helm has spoken. With `browser_flow`, the browser sends
    /// its own `resume` and `pause` (then a resize as a marker) while the
    /// helm's pause holds, and the supervisor must hear only the resize.
    async fn slow_browser_pauses_and_resumes(size: usize, after: u32, browser_flow: bool) {
        let (client_side, peer_side) = tokio::io::duplex(1024 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let path = session_path(&harness, "/api/sessions/sess-1/term");
        let (mut ws, peer) = tokio::join!(
            WsTestClient::connect_with_receive_buffer(addr, &path, SMALL_RECEIVE_BUFFER),
            peer
        );
        let (reader, mut writer, channel) = peer.unwrap();

        let flood = flood_until_heard(reader, &mut writer, channel, size, after).await;
        let (mut reader, written) = (flood.reader, flood.written);
        assert!(
            is_control(&flood.heard, &ControlMsg::PauseOutput { channel }),
            "a browser falling behind must pause its grid stream before anything else: {:?} \
             after {written} frames",
            flood.heard
        );
        if browser_flow {
            // The socket's messages reach the supervisor in arrival order,
            // so the resize arriving first, alone, shows that the resume and
            // pause before it were read and dropped.
            ws.send_text(r#"{"type":"resume"}"#).await;
            ws.send_text(r#"{"type":"pause"}"#).await;
            ws.send_text(r#"{"type":"resize","cols":91,"rows":31}"#)
                .await;
            let resize = ControlMsg::Resize {
                session_id: "sess-1".to_string(),
                channel,
                cols: 91,
                rows: 31,
            };
            let before = read_until(&mut reader, "the marker resize", |frame| {
                is_control(frame, &resize)
            })
            .await;
            assert!(
                before.is_empty(),
                "a browser's pause and resume on a grid socket must not be forwarded: {before:?}"
            );
        }

        for expected in 0..written {
            let (opcode, payload) = tokio::time::timeout(Duration::from_secs(10), ws.recv())
                .await
                .unwrap_or_else(|_| panic!("frame {expected} of {written} never arrived"))
                .unwrap_or_else(|| panic!("socket closed before frame {expected} of {written}"));
            assert_eq!(opcode, 2, "frame {expected} is not a binary frame");
            assert_eq!(
                (
                    payload.len(),
                    u32::from_be_bytes(payload[..4].try_into().unwrap())
                ),
                (size, expected),
                "frames must arrive whole and in order"
            );
        }
        let before = read_until(&mut reader, "ResumeOutput", |frame| {
            is_control(frame, &ControlMsg::ResumeOutput { channel })
        })
        .await;
        assert!(
            before.is_empty(),
            "nothing but the resume may follow the pause here: {before:?}"
        );
    }

    /// The heartbeat's ping is answered BY THE HELM and never relayed
    /// (PLAN_M6.md item 7).
    ///
    /// Both halves are the contract, and each fails differently. Without
    /// the answer, a browser on a healthy-but-quiet socket would time its
    /// own probe out, tear a perfectly good terminal down, and reattach on
    /// a ladder — turning the check that exists to catch dead sockets into
    /// a generator of spurious reconnects. Without the non-relay, every
    /// idle terminal in a fleet would put a periodic message on its host's
    /// control path forever, and a probe about the BROWSER's socket would
    /// be answered by the health of something else entirely.
    ///
    /// The non-relay is proven positively rather than by waiting for an
    /// absence: a `resize` follows the ping, and the first control frame the
    /// supervisor sees must be that resize. A relayed ping would be sitting
    /// in front of it, in the same order this one socket wrote them.
    #[farhelm_testtrace::test]
    async fn term_ws_answers_a_heartbeat_ping_without_relaying_it() {
        use farhelm_proto::ControlMsg;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            connect_session(&harness, addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (mut reader, _writer, channel) = peer.unwrap();

        ws.send_text(r#"{"type":"ping"}"#).await;
        ws.send_text(r#"{"type":"resize","cols":81,"rows":25}"#)
            .await;

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("the helm never answered the heartbeat")
            .expect("socket closed before the pong");
        assert_eq!(opcode, 1, "the pong is a text frame");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&payload).unwrap(),
            serde_json::json!({"type": "pong"}),
            "the answer is exactly this fixed object — the browser matches on its type and \
             nothing else"
        );

        let frame = tokio::time::timeout(Duration::from_secs(5), reader.read_frame())
            .await
            .expect("the supervisor never saw the resize that followed the ping")
            .unwrap()
            .expect("connection closed");
        let resize = ControlMsg::Resize {
            session_id: "sess-1".to_string(),
            channel,
            cols: 81,
            rows: 25,
        };
        assert!(
            is_control(&frame, &resize),
            "the ping must not reach the supervisor at all, so the resize is the FIRST control \
             frame it sees: {frame:?}"
        );
    }

    /// The unattended attach route reaches the supervisor as a
    /// NON-DISPLACING attach, and the ordinary route does not (PLAN_M6.md
    /// item 7).
    ///
    /// Pinned at the wire because that is where the safety property lives:
    /// the browser's automatic reconnect asks for the refusal by CHOOSING
    /// A PATH, and what has to be true is that the path it chose becomes
    /// the flag the supervisor reads. A regression that dropped the flag
    /// would leave every automatic reconnect displacing again — silently,
    /// and only observably in a two-client race.
    #[farhelm_testtrace::test]
    async fn the_unowned_route_asks_the_supervisor_not_to_displace() {
        for (path, expected) in [
            ("/api/sessions/sess-1/term", false),
            ("/api/sessions/sess-1/term/unowned", true),
        ] {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request =
                    farhelm_proto::io::parse_control(&reader.read_frame().await.unwrap().unwrap())
                        .unwrap();
                let ControlMsg::Attach { if_unowned, .. } = request else {
                    panic!("expected an Attach, got {request:?}");
                };
                if_unowned
            });
            let mut harness = rest_harness::spliced_helm(client_side).await;
            let addr = harness.serve().await;
            let (_ws, asked) = tokio::join!(connect_session(&harness, addr, path), peer);
            assert_eq!(
                asked.unwrap(),
                expected,
                "{path} must attach with if_unowned={expected}"
            );
        }
    }

    /// Every reply carries this helm's build stamp (PLAN_M6.md item 6), so
    /// a browser tab left open across a helm upgrade can notice on whatever
    /// request it makes next.
    ///
    /// Asserted on the responses that do NOT come from a handler as well as
    /// the ones that do, and those are the interesting cases: the stamp is
    /// added by the OUTERMOST layer precisely so a mismatch surfaces even
    /// when it manifests as an inexplicable refusal, which is the shape a
    /// stale bundle's failures usually take.
    ///
    /// The rejected-origin leg is the one that caught a real hole: the
    /// origin guard answers 403 and returns before anything inside it runs,
    /// so a stamp inserted in that guard's own success path — where this
    /// started — was absent from exactly the reply a confused client is
    /// most likely to receive. A skewed UI whose requests are being refused
    /// would have been told nothing at all.
    #[farhelm_testtrace::test]
    async fn every_reply_carries_the_helms_build_stamp() {
        let harness = rest_harness::helm_listing(vec![]).await;
        for (uri, origin, expected) in [
            ("/api/sessions", None, axum::http::StatusCode::OK),
            (
                "/api/sessions/nope",
                None,
                axum::http::StatusCode::NOT_FOUND,
            ),
            (
                "/api/sessions",
                Some("http://attacker.example"),
                axum::http::StatusCode::FORBIDDEN,
            ),
        ] {
            let mut builder = axum::http::Request::builder()
                .method("GET")
                .uri(session_path(&harness, uri))
                .header("host", "127.0.0.1:7433");
            if let Some(origin) = origin {
                builder = builder.header("origin", origin);
            }
            let request = builder.body(axum::body::Body::empty()).unwrap();
            let response = tower::ServiceExt::oneshot(harness.router(), request)
                .await
                .unwrap();
            assert_eq!(response.status(), expected, "{uri}");
            assert_eq!(
                response
                    .headers()
                    .get(BUILD_STAMP_HEADER)
                    .and_then(|value| value.to_str().ok()),
                Some(farhelm_proto::BUILD_VERSION),
                "{uri} must carry the stamp the UI compares against its own"
            );
        }
    }

    /// The three ways a WS attach's selector/lease resolve to an `Attach`
    /// frame — neither param (the legacy pre-M4 reading), `?tab=` alone,
    /// `?lease=` alone — share one assertion shape (the resolved
    /// `terminal`/`lease` pair reaching the supervisor's `Attach`) and
    /// differ only in the query string and the expected pair, so one
    /// parameterized test replaces three near-identical ones.
    /// `term_ws_with_tab_and_lease_together_carries_both_on_one_attach`
    /// below is deliberately NOT folded in here: it is the one case whose
    /// entire point is that two fields combine on the SAME frame, which a
    /// shared loop body would only obscure.
    #[farhelm_testtrace::test]
    async fn term_ws_selector_and_lease_reach_the_attach_frame() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, TerminalSelector};

        let cases: [(&str, TerminalSelector, &str); 3] = [
            ("", TerminalSelector::Agent, ""),
            (
                "?tab=tab-1",
                TerminalSelector::Tab { id: "tab-1".into() },
                "",
            ),
            ("?lease=client-abc", TerminalSelector::Agent, "client-abc"),
        ];

        for (query, expected_terminal, expected_lease) in cases {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::Attach {
                    req_id,
                    channel,
                    terminal,
                    lease,
                    ..
                } = request
                else {
                    panic!("expected Attach, got {request:?}");
                };
                assert_eq!(terminal, expected_terminal, "for query {query:?}");
                assert_eq!(lease, expected_lease, "for query {query:?}");
                writer
                    .write_control(&ControlMsg::Attached { req_id, channel })
                    .await
                    .unwrap();
            });

            let mut harness = rest_harness::spliced_helm(client_side).await;
            let addr = harness.serve().await;
            let path = format!("/api/sessions/sess-1/term{query}");
            let (_ws, peer) = tokio::join!(connect_session(&harness, addr, &path), peer);
            peer.unwrap();
        }
    }

    /// `?tab=<id>&lease=<id>` together must carry BOTH fields onto the
    /// SAME `Attach` — the parameterized selector test above deliberately
    /// covers each field in isolation, which would not catch a
    /// regression where handling one query param clobbers the other
    /// (e.g. an extractor path that overwrites `terminal` and forgets to
    /// also thread `lease`, or vice versa).
    #[farhelm_testtrace::test]
    async fn term_ws_with_tab_and_lease_together_carries_both_on_one_attach() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, TerminalSelector};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::Attach {
                req_id,
                channel,
                terminal,
                lease,
                ..
            } = request
            else {
                panic!("expected Attach, got {request:?}");
            };
            assert_eq!(terminal, TerminalSelector::Tab { id: "tab-1".into() });
            assert_eq!(lease, "client-abc");
            writer
                .write_control(&ControlMsg::Attached { req_id, channel })
                .await
                .unwrap();
        });

        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (_ws, peer) = tokio::join!(
            connect_session(
                &harness,
                addr,
                "/api/sessions/sess-1/term?tab=tab-1&lease=client-abc"
            ),
            peer
        );
        peer.unwrap();
    }

    /// An unknown `?tab=` id must surface on the WebSocket exactly like
    /// any other attach failure (PLAN_M4.md item 5): a
    /// `{"type":"detached",...}` notice carrying the supervisor's own
    /// `NotFound` message, then the socket closes — never a bare
    /// disconnect a browser would blame on the network instead of the
    /// session. The supervisor owns the real "does this tab exist" check
    /// (see `resolve_attach_request`'s docs, including for why `?tab=`
    /// gets no local shape check at all); this test is what proves its
    /// `NotFound` reaches the client rather than being swallowed
    /// somewhere in the WS plumbing this PR adds. The notice's code is
    /// `tab_closed`, because a tab the supervisor cannot find is gone and
    /// the browser must stop trying to attach it (see
    /// `refused_as_missing_tab`). Both the notice recv
    /// AND the close recv are wrapped in a bounded timeout: a regression
    /// that left either one pending must fail this test, not hang it.
    #[farhelm_testtrace::test]
    async fn term_ws_with_unknown_tab_id_surfaces_the_supervisors_not_found_error() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind, TerminalSelector};

        const SENTINEL: &str = "SENTINEL-tab-attach-6e21: no such tab";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::Attach {
                req_id, terminal, ..
            } = request
            else {
                panic!("expected Attach, got {request:?}");
            };
            assert_eq!(
                terminal,
                TerminalSelector::Tab {
                    id: "no-such-tab".into()
                }
            );
            writer
                .write_control(&ControlMsg::Error {
                    create_unaccepted: false,
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::NotFound,
                })
                .await
                .unwrap();
        });

        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            connect_session(&harness, addr, "/api/sessions/sess-1/term?tab=no-such-tab"),
            peer
        );
        peer.unwrap();

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no detach notice arrived")
            .expect("socket closed before sending a notice");
        assert_eq!(opcode, 1, "the detach notice is a text frame");
        let notice: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(notice["type"], "detached");
        assert!(
            notice["reason"].as_str().unwrap().contains(SENTINEL),
            "reason must carry the supervisor's own message: {notice}"
        );
        assert_eq!(
            notice["code"], "tab_closed",
            "a tab the supervisor says does not exist is closed, not a failed attempt: {notice}"
        );

        assert!(
            tokio::time::timeout(Duration::from_secs(5), ws.recv())
                .await
                .expect("socket never closed after the failed attach's notice")
                .is_none(),
            "the socket must close once the failed attach's notice is sent"
        );
    }

    /// Attach `path` against a scripted supervisor that refuses with `kind`,
    /// and return the browser's detach notice.
    async fn detach_notice_for_refused_attach(
        path: &str,
        kind: farhelm_proto::ErrorKind,
    ) -> serde_json::Value {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::Attach { req_id, .. } = request else {
                panic!("expected Attach, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    create_unaccepted: false,
                    req_id,
                    message: "scripted refusal".to_string(),
                    kind,
                })
                .await
                .unwrap();
        });

        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(connect_session(&harness, addr, path), peer);
        peer.unwrap();
        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no detach notice arrived")
            .expect("socket closed before sending a notice");
        assert_eq!(opcode, 1, "the detach notice is a text frame");
        serde_json::from_slice(&payload).unwrap()
    }

    /// `tab_closed` is reserved for the one refusal that proves a TAB is
    /// gone. The same `NotFound` on the agent terminal carries the
    /// supervisor's account of what happened to the session, which the
    /// browser must keep showing rather than silently hiding, and a tab
    /// refused for any other reason, such as a host the helm cannot reach,
    /// is transport trouble that the browser's reconnect ladder exists to
    /// ride out. Both stay `other`.
    ///
    /// This pins the edges of the mapping that stopped phantom tabs from
    /// retrying (see `refused_as_missing_tab`): widened to the agent, it
    /// would hide a rebooted session's explanation; widened past
    /// `NotFound`, it would end recovery for tabs on a host that is only
    /// briefly away.
    #[farhelm_testtrace::test]
    async fn only_a_missing_tab_is_reported_as_tab_closed() {
        use farhelm_proto::ErrorKind;

        let agent =
            detach_notice_for_refused_attach("/api/sessions/sess-1/term", ErrorKind::NotFound)
                .await;
        assert_eq!(agent["type"], "detached");
        assert_eq!(
            agent["code"], "other",
            "the agent terminal's NotFound keeps its explanation visible: {agent}"
        );

        let unreachable = detach_notice_for_refused_attach(
            "/api/sessions/sess-1/term?tab=some-tab",
            ErrorKind::Unavailable,
        )
        .await;
        assert_eq!(unreachable["type"], "detached");
        assert_eq!(
            unreachable["code"], "other",
            "a tab refused for transport reasons must stay recoverable: {unreachable}"
        );
    }

    /// An explicit, empty `?lease=` (as opposed to no `?lease=` at all)
    /// must be REJECTED helm-side (`resolve_attach_request`'s asymmetry,
    /// item 5's own docs): the wire's empty lease IS the legal legacy
    /// meaning, so the supervisor cannot refuse it — accepting `?lease=`
    /// here would silently fold "this client explicitly opted into the
    /// un-leased singleton reading" back into "this client said nothing",
    /// which would make one session view's own terminal sockets take
    /// each other over. The failure path is the same detach-notice-then-
    /// close every other refusal in this file uses, and the scripted peer
    /// proves NO `Attach` ever left the helm for it.
    ///
    /// The no-`Attach` check runs AFTER the WS client has already
    /// observed both the notice and the socket's close — not a fixed
    /// timer racing the request (a flaw an earlier version of this class
    /// of test had for `?tab=`): by the time the client sees the close,
    /// `serve_term` has already returned, so anything it was ever going
    /// to send to the supervisor has already been sent, and checking for
    /// it needs no guess at how long "long enough" is.
    #[farhelm_testtrace::test]
    async fn term_ws_with_empty_lease_is_refused_locally_without_contacting_the_supervisor() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            reader
        });

        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let mut ws = WsTestClient::connect(
            addr,
            &session_path(&harness, "/api/sessions/sess-1/term?lease="),
        )
        .await;

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no detach notice arrived")
            .expect("socket closed before sending a notice");
        assert_eq!(opcode, 1, "the detach notice is a text frame");
        let notice: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(notice["type"], "detached");
        assert!(
            notice["reason"]
                .as_str()
                .unwrap()
                .contains("must not be empty"),
            "reason must name the empty-lease shape problem: {notice}"
        );

        // Bounded, not indefinite: a regression that left the socket open
        // must fail this test rather than hang it.
        assert!(
            tokio::time::timeout(Duration::from_secs(5), ws.recv())
                .await
                .expect("socket never closed after the local refusal's notice")
                .is_none(),
            "the socket must close once the locally-refused attach's notice is sent"
        );

        // Only NOW — after both the notice and the close are observed, so
        // `serve_term` has already returned and anything it would ever
        // send has already been sent — check that no `Attach` reached the
        // peer. A short timeout suffices: nothing further can arrive at
        // this point, so this is not a race against the request, only a
        // way to turn "nothing queued" into an assertion without blocking
        // forever on a connection this test keeps open indefinitely.
        let mut reader = peer.await.unwrap();
        let got = tokio::time::timeout(Duration::from_millis(200), reader.read_frame()).await;
        assert!(
            got.is_err(),
            "an Attach reached the supervisor for an explicitly empty ?lease=, which must be \
             refused locally instead"
        );
    }

    // ---- Multi-host aggregation and routing (PLAN_M6.md item 5) ------
    //
    // Everything below stands the real serving path up over a scripted
    // FLEET rather than one connection (see `rest_harness`), because the
    // properties are about more than one host at a time: which rows appear
    // together and in what order, which of them are stale, and which host
    // an operation reaches.
}
