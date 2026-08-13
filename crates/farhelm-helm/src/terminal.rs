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
//! Terminal bytes are binary frames in both directions. Text frames are the
//! control channel (`WsClientMsg`): resize, and the browser's own
//! watermark pause/resume, which is forwarded to the supervisor rather than
//! interpreted here. Two answers are synthesized on this side —
//! `PONG_TEXT_MESSAGE` for the heartbeat and `REPLAY_COMPLETE_TEXT_MESSAGE`
//! for the marker that separates replayed scrollback from live output.

use crate::auth::AuthenticatedSocket;
use crate::sessions::{default_cols, default_rows, route_session};
use crate::{AppState, SupervisorClient, SupervisorError, TermEvent, TermStream};
use anyhow::Context;
use axum::Extension;
use axum::extract::{Path as AxPath, Query, State, WebSocketUpgrade, ws};
use axum::response::IntoResponse;
use farhelm_proto::{ErrorKind, TerminalSelector};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{error, info};

/// Initial terminal size, carried as query parameters because a WebSocket
/// handshake is a GET with no body. Sizing the pane at attach time rather
/// than waiting for the first `resize` message is what gets live output
/// to the right width immediately instead of reflowing a moment later.
///
/// It does not shape the replay directly: the supervisor resizes the
/// window after takeover and just before its replacement client captures
/// (in that order deliberately — resizing during prep would leave an
/// incumbent's terminal reflowed by an attach that may still fail; see
/// the attach handler), so the replay is content tmux reflowed to the
/// NEW geometry, possibly while the application is still repainting for
/// it. Full-screen apps finish repainting on the SIGWINCH they already
/// received; normal-screen sessions wear tmux's reflow until the next
/// output. For a freshly opened tab that resize is NORMALLY a no-op —
/// the open path pre-sizes the tab window to the agent window's geometry,
/// best-effort — which is what keeps a new tab's first replay from racing
/// its shell's resize repaint in the common case (a failed pre-size, or a
/// client that resized between open and attach, degrades to the ordinary
/// resize-at-attach).
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
/// input, resize, pause/resume, detach. Re-routing per message would let a
/// mid-session reconnect silently move a live terminal's writes to a
/// different connection than the one its attachment lives on.
async fn attach_from_query(
    state: &AppState,
    session_id: &str,
    q: &TermQuery,
    if_unowned: bool,
) -> anyhow::Result<(Arc<SupervisorClient>, u32, TermStream)> {
    let (terminal, lease) = resolve_attach_request(q)?;
    let (_claim, client) = route_session(state, session_id).await?;
    // Which of the two attach contracts this socket wants, decided by the
    // query the browser sent: an unattended reconnect refuses rather than
    // displaces (see `TermQuery::if_unowned`), everything else is the
    // ordinary last-attach-wins attach.
    let (channel, stream) = if if_unowned {
        client
            .attach_terminal_if_unowned(session_id, q.cols, q.rows, terminal, lease)
            .await?
    } else {
        client
            .attach_terminal(session_id, q.cols, q.rows, terminal, lease)
            .await?
    };
    Ok((client, channel, stream))
}

/// Terminal WebSocket: binary frames are terminal bytes in both
/// directions; text frames are small JSON control messages (client →
/// resize/pause/resume; server → detached notice, replay-complete marker).
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
/// The client → server text messages, all `{"type": ...}`:
/// - `{"type":"resize","cols":N,"rows":N}` — the pane's new geometry.
/// - `{"type":"pause"}` — this terminal's unflushed `term.write()`
///   backlog crossed its high-water mark; stop sending output.
/// - `{"type":"resume"}` — the backlog drained below the low-water mark;
///   output may flow again.
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
/// Pause and resume carry no payload because the channel is implicit:
/// one socket is one attachment. They are the browser end of
/// PLAN_M2_5.md's watermark flow control and travel straight through to
/// the supervisor as `ControlMsg::PauseOutput`/`ResumeOutput` — the helm
/// keeps no pause state of its own (see `SupervisorClient::pause_output`).
/// The browser side that SENDS them lands with the UI work in
/// PLAN_M2_5.md step 4; the server accepts them now.
///
/// Server → client, both text: `{"type":"detached","reason":...}` and, as
/// of PLAN_M5.md item 4, `{"type":"replay_complete"}` — the attach's
/// catch-up boundary, forwarded on this SAME socket after the binary
/// replay bytes it follows and before any binary live bytes, because
/// [`TermEvent::ReplayComplete`] rides the terminal's data queue rather
/// than jumping ahead of it (see that variant's own docs for why the
/// ordering is the whole point). Consumers must treat it as pure
/// presentation, never as a signal for session or lifecycle behavior —
/// the same restriction `ControlMsg::ReplayComplete`'s docs place on every
/// consumer of the wire message it forwards.
/// The fixed wire text for the replay-complete marker (PLAN_M5.md item 4;
/// see `term_ws`'s docs for where it sits in the server→client message
/// set). A plain constant rather than a `serde_json::json!` value built
/// fresh per marker: the shape never varies — no fields, ever — so there
/// is nothing for a JSON builder to add over a literal, and every marker
/// this socket ever sends is this exact string.
const REPLAY_COMPLETE_TEXT_MESSAGE: &str = r#"{"type":"replay_complete"}"#;

/// Whether this failure is the supervisor refusing a non-displacing attach
/// because another client holds the session.
///
/// `anyhow`'s `downcast_ref` searches the root cause and every context
/// layer above it, so this survives callers that annotate the error on the
/// way out — which is exactly what comparing rendered text does not.
fn refused_as_taken_over(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<SupervisorError>()
        .is_some_and(|supervised| {
            supervised.kind == ErrorKind::Conflict
                && supervised.message == farhelm_proto::ATTACH_REFUSED_TAKEN_OVER
        })
}

pub(crate) async fn term_ws(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedSocket>,
    AxPath(id): AxPath<String>,
    Query(q): Query<TermQuery>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    serve_term_upgrade(state, auth, id, q, upgrade, false)
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
    Extension(auth): Extension<AuthenticatedSocket>,
    AxPath(id): AxPath<String>,
    Query(q): Query<TermQuery>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    serve_term_upgrade(state, auth, id, q, upgrade, true)
}

/// The upgrade both terminal routes share; `if_unowned` is the one thing
/// they differ in.
fn serve_term_upgrade(
    state: Arc<AppState>,
    auth: AuthenticatedSocket,
    id: String,
    q: TermQuery,
    upgrade: WebSocketUpgrade,
    if_unowned: bool,
) -> axum::response::Response {
    // Sized to what the client can chunk onward (MAX_FRAME_LEN), not
    // smaller: xterm.js hands a bracketed paste to us as ONE message, so
    // a tighter cap would turn a large clipboard paste into a dropped
    // connection — the very failure chunking exists to prevent.
    // Kept for the log line below, which runs after `id` has moved into
    // the handler.
    let id_for_log = id.clone();
    upgrade
        .protocols([crate::auth::WS_PROTOCOL])
        .max_message_size(farhelm_proto::MAX_FRAME_LEN as usize)
        .on_upgrade(move |socket| async move {
            if let Err(e) = serve_term(state, auth, id, q, socket, if_unowned).await {
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
                // silently promote a routine refusal back to ERROR. The
                // kind is checked alongside the reason because the reason
                // is user-facing prose and the kind is what makes it a
                // refusal rather than a coincidence.
                if refused_as_taken_over(&e) {
                    info!(session = %id_for_log, "terminal reconnect refused: another client holds this session");
                } else {
                    error!(error = %e, "terminal websocket ended with error");
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

/// Client-to-helm control messages on a terminal socket. Text frames only
/// — binary is always terminal input — and an unparseable one is ignored
/// rather than fatal, so adding a message type does not break older
/// clients. See `term_ws`'s docs for the wire shapes.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsClientMsg {
    Resize {
        cols: u16,
        rows: u16,
    },
    /// Watermark flow control, PLAN_M2_5.md. No fields: one socket is one
    /// attachment, so the channel these apply to is never ambiguous, and
    /// letting the browser name a channel would only invite it to name
    /// somebody else's.
    Pause,
    Resume,
    /// The browser's liveness probe (PLAN_M6.md item 7), answered here with
    /// [`PONG_TEXT_MESSAGE`] and never forwarded — see `term_ws`'s docs for
    /// why the answer belongs to this end of the socket.
    Ping,
}

/// The fixed wire text for the heartbeat's answer.
///
/// A constant for the same reason the replay marker is one: the shape never
/// varies, so there is nothing for a JSON builder to add over a literal.
const PONG_TEXT_MESSAGE: &str = r#"{"type":"pong"}"#;

/// Pump one attached terminal between the browser and the supervisor.
///
/// The body of the terminal path, and deliberately the dumbest part of it:
/// bytes are never inspected, buffered, or transformed in either
/// direction. Every escape sequence a client sees comes from the pane, and
/// every keystroke reaches it unedited — that is what "full fidelity" in
/// SPEC.md costs here, and any parsing added at this layer would break it.
///
/// The socket always outlives its attachment by exactly one message: when
/// the supervisor ends the attachment (takeover, dead terminal), the
/// detach notice goes out *before* the close, because a bare close renders
/// as a generic "connection closed" and SPEC.md requires a takeover to be
/// visibly a takeover. `detach` runs on every exit path so the supervisor
/// never keeps an attachment alive for a browser that is gone.
///
/// # Why two tasks
///
/// Inbound (browser → supervisor) and outbound (supervisor → browser) run
/// as separate tasks rather than two arms of one `select!`. Since the
/// helm's outbound queue became bounded, every inbound forward — input,
/// resize, pause, resume — can park waiting for capacity, and in a single
/// loop that parking also stops draining terminal events. That is the
/// worst possible coupling: a big paste blocks output delivery, the
/// per-terminal queue backs up, and a perfectly healthy viewer trips the
/// stalled-terminal detach. Splitting them means a blocked inbound send
/// cannot starve the outbound drain.
///
/// Inbound ORDER is preserved regardless: all four inbound message kinds
/// originate from this one WebSocket read loop and are forwarded from it
/// in arrival order. Only the outbound drain moved.
async fn serve_term(
    state: Arc<AppState>,
    auth: AuthenticatedSocket,
    session_id: String,
    q: TermQuery,
    mut socket: ws::WebSocket,
    if_unowned: bool,
) -> anyhow::Result<()> {
    use futures_util::{SinkExt, StreamExt};

    if auth.is_revoked().await {
        return Ok(());
    }

    // One arm covers both failure sources `attach_from_query` can produce
    // — a locally-refused query shape (an explicit empty `?lease=`) and a
    // supervisor-side attach refusal (unknown session, unknown tab, tmux
    // trouble) — because both must reach the user identically: a
    // `{"type":"detached",...}` notice, then a closed socket, never a bare
    // disconnect the browser would blame on the network instead of the
    // request it just made. See `attach_from_query`'s own docs for why
    // folding them into one `Result` is what keeps this a single arm
    // instead of two copies of the same three lines.
    let attached = attach_from_query(&state, &session_id, &q, if_unowned);
    tokio::pin!(attached);
    let (client, channel, mut events) = tokio::select! {
        biased;
        _ = auth.revoked() => {
            // The request may already have crossed into the supervisor. Drop
            // the browser first, then wait only long enough to learn whether
            // admission completed and needs an explicit detach.
            drop(socket);
            if let Ok(Ok((client, channel, _events))) =
                tokio::time::timeout(WS_TEARDOWN_GRACE, &mut attached).await
            {
                detach_bounded(&client, channel).await;
            }
            return Ok(());
        }
        attached = &mut attached => {
            match attached {
            Ok(parts) => parts,
            Err(e) => {
                let notice = serde_json::json!({"type": "detached", "reason": format!("{e:#}")});
                let _ = socket
                    .send(ws::Message::Text(notice.to_string().into()))
                    .await;
                return Err(e);
            }
            }
        }
    };

    // Rotation may have committed after the attach future became ready but
    // before this task won the select. Do not start either I/O pump in that
    // admission gap.
    if auth.is_revoked().await {
        drop(socket);
        detach_bounded(&client, channel).await;
        return Ok(());
    }

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
                None => ws::Message::Text(PONG_TEXT_MESSAGE.into()),
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
                    ws::Message::Text(REPLAY_COMPLETE_TEXT_MESSAGE.into())
                }
                Some(TermEvent::Detached(reason)) => {
                    let notice = serde_json::json!({"type": "detached", "reason": reason});
                    // Best-effort and last: the socket closes right after,
                    // and a browser that cannot even take this notice is
                    // one the reason would not have reached anyway.
                    let _ = ws_tx
                        .send(ws::Message::Text(notice.to_string().into()))
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
                    let reason = reason.unwrap_or_else(|| "detached".to_string());
                    let notice = serde_json::json!({"type": "detached", "reason": reason});
                    let _ = ws_tx
                        .send(ws::Message::Text(notice.to_string().into()))
                        .await;
                    break;
                }
            }
        }
    });

    let client_inbound = Arc::clone(&client);
    let session_id_inbound = session_id.clone();
    let inbound = tokio::spawn(async move {
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(ws::Message::Binary(bytes)) => {
                    client_inbound.send_input(channel, bytes.to_vec()).await;
                }
                Ok(ws::Message::Text(text)) => match serde_json::from_str::<WsClientMsg>(&text) {
                    Ok(WsClientMsg::Resize { cols, rows }) => {
                        client_inbound
                            .resize(&session_id_inbound, channel, cols, rows)
                            .await;
                    }
                    Ok(WsClientMsg::Pause) => client_inbound.pause_output(channel).await,
                    Ok(WsClientMsg::Resume) => client_inbound.resume_output(channel).await,
                    // The one client message answered HERE rather than
                    // relayed (see `term_ws`'s docs). Notifying never parks,
                    // which is the property this loop needs: the browser
                    // that is not draining is exactly the one whose ping
                    // would otherwise block every keystroke queued behind
                    // it.
                    Ok(WsClientMsg::Ping) => pong.notify_one(),
                    // Unparseable or unknown: ignored on purpose, so a
                    // newer browser bundle talking to an older helm
                    // degrades rather than dropping the terminal.
                    Err(_) => {}
                },
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
        Revoked,
        Inbound(Result<anyhow::Result<()>, tokio::task::JoinError>),
        Outbound,
    }
    let end = tokio::select! {
        _ = auth.revoked() => SocketEnd::Revoked,
        result = &mut inbound => SocketEnd::Inbound(result),
        _ = &mut outbound => SocketEnd::Outbound,
    };

    if matches!(&end, SocketEnd::Revoked) {
        // Drop both WebSocket halves before asking the supervisor to clean
        // up. Cleanup can backpressure; revoked browser I/O must not remain
        // reachable while it does.
        inbound.abort();
        outbound.abort();
        let _ = inbound.await;
        let _ = outbound.await;
        detach_bounded(&client, channel).await;
        return Ok(());
    }

    let (result, outbound_finished, inbound_finished) = match end {
        SocketEnd::Inbound(result) => {
            let result = result.context("terminal websocket inbound task panicked")?;
            (result, false, true)
        }
        SocketEnd::Outbound => (Ok(()), true, false),
        SocketEnd::Revoked => unreachable!("handled above"),
    };
    if !inbound_finished {
        inbound.abort();
        let _ = inbound.await;
    }

    // Detaching is what ends the outbound task in the ORDINARY case (the
    // browser closed its socket): the supervisor drops the attachment,
    // the client signals detached, and the drain unwinds after sending
    // its notice. The grace period covers exactly that notice; past it
    // the task is abandoned, because by then it can only be blocked on
    // the same unreadable socket the detach was about.
    client.detach(channel).await;
    settle_outbound(outbound, outbound_finished, WS_TEARDOWN_GRACE).await;
    result
}

/// Give supervisor cleanup a bounded opportunity after browser I/O is gone.
async fn detach_bounded(client: &SupervisorClient, channel: u32) {
    if tokio::time::timeout(WS_TEARDOWN_GRACE, client.detach(channel))
        .await
        .is_err()
    {
        tracing::warn!(
            channel,
            "supervisor detach timed out after socket revocation"
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
    use crate::BUILD_STAMP_HEADER;
    use crate::rest_harness::{self, WsTestClient};
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};
    use farhelm_proto::{ControlMsg, Frame};
    use std::time::Duration;
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
    #[tokio::test]
    async fn outbound_teardown_never_polls_a_finished_join_handle() {
        // Driven to completion by the caller, exactly as the `select!`
        // arm does before reporting `already_finished`.
        let mut handle = tokio::spawn(async {});
        (&mut handle).await.expect("the task cannot panic");
        super::settle_outbound(handle, true, Duration::from_secs(5)).await;

        // Finishes on its own inside the grace: the ordinary navigation
        // case, and the one the old code panicked on.
        let handle = tokio::spawn(async {
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

    /// Rotation during supervisor admission never starts browser I/O and
    /// still detaches an attachment whose late reply crossed the revocation.
    #[tokio::test]
    async fn rotation_during_terminal_admission_closes_before_cleanup() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let (attach_seen_tx, attach_seen_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (read, write) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(read);
            let mut writer = FrameWriter::new(write);
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
                panic!("expected Attach, got {request:?}");
            };
            attach_seen_tx.send(()).unwrap();
            release_rx.await.unwrap();
            writer
                .write_control(&ControlMsg::Attached { req_id, channel })
                .await
                .unwrap();
            loop {
                let frame = reader.read_frame().await.unwrap().unwrap();
                let message = farhelm_proto::io::parse_control(&frame).unwrap();
                if matches!(message, ControlMsg::Detach { channel: got } if got == channel) {
                    return;
                }
            }
        });

        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let mut ws = WsTestClient::connect(addr, "/api/sessions/sess-1/term").await;
        attach_seen_rx.await.unwrap();
        harness.state.auth.rotate().await.unwrap();
        release_tx.send(()).unwrap();

        tokio::time::timeout(Duration::from_secs(5), async {
            while ws.recv().await.is_some() {}
        })
        .await
        .expect("the revoked browser socket must close before cleanup can stall");
        tokio::time::timeout(Duration::from_secs(5), peer)
            .await
            .expect("the admitted supervisor channel must be cleaned up")
            .unwrap();
    }

    /// Browser pause/resume must reach the SUPERVISOR as
    /// `PauseOutput`/`ResumeOutput` for this terminal's channel.
    ///
    /// The WS half of PLAN_M2_5.md's watermark flow control had no
    /// coverage at all: only `SupervisorClient`'s methods were tested, so
    /// nothing pinned the JSON message shapes, the routing from a text
    /// frame to the right channel, or that the helm forwards rather than
    /// interpreting. Any of those silently breaking would leave the
    /// browser's watermark wired to nothing — a failure whose only symptom
    /// is memory growth under load.
    #[tokio::test]
    async fn browser_pause_and_resume_reach_the_supervisor_for_this_channel() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        // Concurrently, and this ordering is not incidental: the scripted
        // peer only ever sees an `Attach` because a browser connected, so
        // awaiting it first would be waiting on something this test has
        // not caused yet.
        let (mut ws, peer) = tokio::join!(
            WsTestClient::connect(addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (mut reader, _writer, channel) = peer.unwrap();

        ws.send_text(r#"{"type":"pause"}"#).await;
        ws.send_text(r#"{"type":"resume"}"#).await;

        for expected in [
            ControlMsg::PauseOutput { channel },
            ControlMsg::ResumeOutput { channel },
        ] {
            let frame = tokio::time::timeout(Duration::from_secs(5), reader.read_frame())
                .await
                .expect("the supervisor never saw the browser's flow-control message")
                .unwrap()
                .expect("connection closed");
            let got = farhelm_proto::io::parse_control(&frame).unwrap();
            assert_eq!(
                format!("{got:?}"),
                format!("{expected:?}"),
                "the browser's message must reach the supervisor unchanged, for its own channel"
            );
        }
    }

    /// For a completed initial attach catch-up, the marker's ordering
    /// property is: it must reach the browser AFTER the binary replay
    /// bytes it describes and BEFORE any binary live byte, on the exact
    /// same socket (PLAN_M5.md item 4) — not the marker's whole contract,
    /// which also covers attaches a takeover/detach/stall ends before a
    /// marker is owed, and the markerless `%pause` recovery replay (see
    /// `TermEvent::ReplayComplete`'s own docs); neither is this test's
    /// concern. This is the real-socket complement to
    /// `client::tests::replay_complete_marker_is_ordered_between_replay_and_live_data_in_the_queue`
    /// — that test pins the ordering inside `SupervisorClient`'s queue;
    /// this one pins that `serve_term`'s WS forwarding does not reorder or
    /// drop the marker on the way to a real `WsTestClient`, and that it
    /// arrives as the documented `{"type":"replay_complete"}` text frame
    /// rather than, say, folded into the binary stream.
    #[tokio::test]
    async fn term_ws_delivers_the_replay_complete_marker_between_replay_and_live_bytes() {
        use farhelm_proto::ControlMsg;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            WsTestClient::connect(addr, "/api/sessions/sess-1/term"),
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

    /// A browser that stops reading must not pin the WebSocket handler:
    /// the stall detach has to terminate it even while a send to that
    /// browser is blocked.
    ///
    /// This is the teardown half of the detach-not-block design, and
    /// until now it was only argued. The failure it guards is specific
    /// and quiet: `serve_term` parked in `ws_tx.send()` to a browser that
    /// stopped reading cannot observe a detach that arrives through the
    /// terminal's data queue, because that queue is full — which is
    /// precisely why the terminal was detached. Handler, socket, queued
    /// frames, and the notification task would then stay alive for as
    /// long as the wedge lasted, which is exactly the unbounded pin the
    /// stall detach exists to end.
    ///
    /// The assertion is the strongest one available from outside: the
    /// server CLOSES the connection. That can only happen after
    /// `serve_term` returned, which can only happen after the blocked send
    /// was abandoned — so a regression that restores the in-band-only
    /// detach hangs here instead of passing.
    #[tokio::test]
    async fn a_wedged_browser_is_torn_down_by_the_stall_detach() {
        let (client_side, peer_side) = tokio::io::duplex(1024 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        // Concurrently, for the same reason as the test above.
        let (mut ws, peer) = tokio::join!(
            WsTestClient::connect(addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (_reader, mut writer, channel) = peer.unwrap();

        // Read exactly one frame, proving the socket works, and then stop
        // reading forever — the wedged browser.
        writer
            .write_frame(&Frame::data(channel, b"first".to_vec()))
            .await
            .unwrap();
        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no first frame")
            .expect("socket closed early");
        assert_eq!((opcode, payload.as_slice()), (2, &b"first"[..]));

        // Flood until the kernel buffers, the WS sink, and the helm's
        // per-terminal queue are all full, so `serve_term` is genuinely
        // parked mid-send rather than idle.
        for _ in 0..2_000 {
            if writer
                .write_frame(&Frame::data(channel, vec![b'x'; 4096]))
                .await
                .is_err()
            {
                break;
            }
        }

        // The supervisor detaches it as stalled. This must tear the
        // handler down even though the send above cannot complete.
        writer
            .write_control(&ControlMsg::Detached {
                channel,
                reason: farhelm_proto::DETACH_REASON_STALLED.to_string(),
            })
            .await
            .unwrap();

        // Drain to EOF: the server must close. Whatever backlog is still
        // in flight is fine to receive — the contract is that the
        // connection ENDS, not that the backlog is discarded byte for
        // byte.
        let closed = tokio::time::timeout(Duration::from_secs(20), async {
            while ws.recv().await.is_some() {}
        })
        .await;
        assert!(
            closed.is_ok(),
            "the terminal WebSocket never closed after a stall detach — `serve_term` is still \
             pinned on a send to a browser that stopped reading"
        );
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
    /// absence: a `pause` follows the ping, and the first control frame the
    /// supervisor sees must be that pause. A relayed ping would be sitting
    /// in front of it, in the same order this one socket wrote them.
    #[tokio::test]
    async fn term_ws_answers_a_heartbeat_ping_without_relaying_it() {
        use farhelm_proto::ControlMsg;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            WsTestClient::connect(addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (mut reader, _writer, channel) = peer.unwrap();

        ws.send_text(r#"{"type":"ping"}"#).await;
        ws.send_text(r#"{"type":"pause"}"#).await;

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
            .expect("the supervisor never saw the pause that followed the ping")
            .unwrap()
            .expect("connection closed");
        let got = farhelm_proto::io::parse_control(&frame).unwrap();
        assert_eq!(
            format!("{got:?}"),
            format!("{:?}", ControlMsg::PauseOutput { channel }),
            "the ping must not reach the supervisor at all, so the pause is the FIRST control \
             frame it sees"
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
    #[tokio::test]
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
            let (_ws, asked) = tokio::join!(WsTestClient::connect(addr, path), peer);
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
    #[tokio::test]
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
                .uri(uri)
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
    #[tokio::test]
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
            let (_ws, peer) = tokio::join!(WsTestClient::connect(addr, &path), peer);
            peer.unwrap();
        }
    }

    /// `?tab=<id>&lease=<id>` together must carry BOTH fields onto the
    /// SAME `Attach` — the parameterized selector test above deliberately
    /// covers each field in isolation, which would not catch a
    /// regression where handling one query param clobbers the other
    /// (e.g. an extractor path that overwrites `terminal` and forgets to
    /// also thread `lease`, or vice versa).
    #[tokio::test]
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
            WsTestClient::connect(addr, "/api/sessions/sess-1/term?tab=tab-1&lease=client-abc"),
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
    /// somewhere in the WS plumbing this PR adds. Both the notice recv
    /// AND the close recv are wrapped in a bounded timeout: a regression
    /// that left either one pending must fail this test, not hang it.
    #[tokio::test]
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
            WsTestClient::connect(addr, "/api/sessions/sess-1/term?tab=no-such-tab"),
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

        assert!(
            tokio::time::timeout(Duration::from_secs(5), ws.recv())
                .await
                .expect("socket never closed after the failed attach's notice")
                .is_none(),
            "the socket must close once the failed attach's notice is sent"
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
    #[tokio::test]
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
        let mut ws = WsTestClient::connect(addr, "/api/sessions/sess-1/term?lease=").await;

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
