//! The Farhelm UI: one Dioxus component tree, two targets.
//!
//! The same components render as the web app (wasm32, real DOM, served
//! by the helm at loopback) and the desktop app (wry webview). The
//! terminal itself is an xterm.js island (assets/terminal.js) whose byte
//! path bypasses Dioxus entirely — Dioxus owns the chrome around the
//! terminal, never its content (SPEC_impl.md, "Terminal widget").
//!
//! Data fetching uses reqwest, which works on both native (desktop) and
//! wasm (browser fetch) — one code path, no per-target HTTP client.
//!
//! ## Navigation (M2, PLAN_M2.md step 7)
//!
//! `App` holds `Signal<Option<Session>>` rather than pulling in a router
//! crate: `None` renders [`ListView`], `Some(session)` renders
//! [`SessionView`] plus a back control that clears the signal.
//! PLAN_M2.md names a premature router as a risk this milestone
//! deliberately avoids — two states and one signal cover everything M2
//! needs, and a router can still be introduced later if M4's terminal
//! tabs (or something else) actually demands one.

use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;
use serde::Deserialize;

/// Absolute origin of the helm's HTTP/WS API, e.g.
/// `http://127.0.0.1:7433`.
///
/// Both targets carry a real origin rather than a relative path: reqwest
/// requires absolute URLs even on wasm, so the web build reads the
/// page's own origin (it is served by the helm) and the desktop build
/// takes `FARHELM_URL`, since a wry webview's origin is not the helm.
#[derive(Clone, PartialEq)]
pub struct ApiBase(pub String);

/// Mirror of the helm's session status JSON (farhelm-proto
/// `SessionStatus`). Kept local for the same reason `Session` is — the UI
/// depends on the HTTP contract, not on proto internals.
///
/// `#[serde(default)]` on every `Session::status` field (below) is what
/// makes an old-shaped reply — one with no `status` at all — decode as
/// `Unknown` rather than fail; this mirrors `SessionStatus`'s own
/// wire-tolerance contract in farhelm-proto. `#[default]` on the
/// `Unknown` variant is what backs that: a reply that predates this
/// field must decode as "not yet known", never as a fabricated liveness
/// claim in either direction.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SessionStatus {
    /// Not yet known one way or the other — never a guess. See
    /// farhelm-proto's `SessionStatus::Unknown` for the full rationale;
    /// this mirror exists only to give the UI something to match on.
    #[default]
    Unknown,
    /// The agent's process is running.
    Alive,
    /// The agent's process has ended. `exit_code` is `None` when tmux
    /// could not reduce the death to a plain code (a signal, or no live
    /// pane to ask at all).
    Exited { exit_code: Option<i32> },
}

/// Mirror of the helm's session JSON (farhelm-proto `SessionInfo`). Kept
/// as a local type so the UI depends on the HTTP contract, not on proto
/// internals — the browser speaks JSON, not frames.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub invocation: String,
    #[serde(default)]
    pub status: SessionStatus,
}

const VENDOR_XTERM_CSS: Asset = asset!("/assets/vendor/xterm.css");
const VENDOR_XTERM_JS: Asset = asset!("/assets/vendor/xterm.js");
const VENDOR_FIT_JS: Asset = asset!("/assets/vendor/addon-fit.js");
const TERMINAL_JS: Asset = asset!("/assets/terminal.js");
const APP_CSS: Asset = asset!("/assets/app.css");

/// Root component: switches between the session list and one open
/// terminal. No router crate (see the module docs) — just a signal.
#[component]
pub fn App() -> Element {
    let mut current = use_signal(|| None::<Session>);

    rsx! {
        document::Link { rel: "stylesheet", href: VENDOR_XTERM_CSS }
        document::Link { rel: "stylesheet", href: APP_CSS }
        document::Script { src: VENDOR_XTERM_JS }
        document::Script { src: VENDOR_FIT_JS }
        document::Script { src: TERMINAL_JS }
        match &*current.read() {
            None => rsx! {
                ListView { on_open: move |session| current.set(Some(session)) }
            },
            Some(session) => rsx! {
                SessionView {
                    session: session.clone(),
                    on_back: move |_| current.set(None),
                }
            },
        }
    }
}

/// Mirror of the helm's `GET /api/sessions` response body (farhelm-helm's
/// `SessionListing`, PLAN_M2.md step 6): `{"sessions": [...], "total": N,
/// "truncated": bool}`. `total`/`truncated` are `#[serde(default)]` for
/// the same old-peer tolerance as `Session::status` above; a decoder that
/// only ever talks to this build's own helm will never hit that path, but
/// nothing here should assume that. `Deserialize` is the only derive this
/// needs: it is read once by reference out of the signal that holds it
/// (see `fetch_sessions`'s docs), never cloned or compared.
#[derive(Deserialize)]
struct SessionListing {
    sessions: Vec<Session>,
    #[serde(default)]
    total: u64,
    #[serde(default)]
    truncated: bool,
}

/// Fetch the session listing, flattening every failure into a displayable
/// string.
///
/// The message is a `String`, not `reqwest::Error`, because it is
/// rendered to the user directly (SPEC.md wants concrete errors) — the
/// URL and status are folded into the message here rather than logged
/// and dropped. That is the whole reason, not a Dioxus constraint:
/// `Signal<T>` only requires `T: 'static` to hold a value, nothing about
/// `Clone` or `PartialEq`.
async fn fetch_sessions(base: &str) -> Result<SessionListing, String> {
    let url = format!("{base}/api/sessions");
    let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp
            .text()
            .await
            .map_err(|error| format!("GET {url}: {status}: reading error response: {error}"))?;
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            format!("GET {url}: {status}")
        } else {
            format!("GET {url}: {status}: {detail}")
        });
    }
    let mut listing = resp
        .json::<SessionListing>()
        .await
        .map_err(|e| e.to_string())?;
    listing.sessions = sort_sessions(listing.sessions);
    Ok(listing)
}

/// Stabilize the list's display order across the protocol's undefined
/// wire order.
///
/// The wire order is a HashMap's ("no defined order" per the proto
/// docs), so without a sort the list would visibly reshuffle rows on
/// every poll even when nothing changed — distracting at best, and
/// actively confusing for a "did status flip?" glance. `id` sort was M1's
/// stopgap for picking a stable `first()`; it survives into M2 as the
/// list's actual display order (its doc previously called it
/// unnecessary once M2 landed a real list — that turned out to be wrong,
/// since a list still needs SOME deterministic order) and stays until
/// something better — most likely creation time — is wanted.
fn sort_sessions(mut sessions: Vec<Session>) -> Vec<Session> {
    sessions.sort_by(|a, b| a.id.cmp(&b.id));
    sessions
}

/// How often the list view refetches (PLAN_M2.md: "Polling for list
/// freshness" is the M2 mechanism; live push is out of scope until M5).
const POLL_INTERVAL_MS: u64 = 3_000;

/// POST the create endpoint, returning the decoded `Session` on success or
/// the response body's own text on failure.
///
/// The failure text is the helm's own message — `http_error` in
/// farhelm-helm renders the supervisor's `anyhow` error chain as
/// `text/plain` (see farhelm-helm's `http_error`), which is why this reads
/// `.text()` rather than trying to parse an error body as JSON. Surfacing
/// that string, rather than a generic "create failed", is the whole point
/// of PLAN_M2.md's create-dialog acceptance: a bad working directory must
/// fail with the supervisor's own "does not exist" message. It is
/// surface-level TRIMMED, not byte-for-byte verbatim: leading/trailing
/// whitespace is stripped before display (and before the empty-body
/// fallback check below), so "the helm's own message" means modulo
/// incidental surrounding whitespace, not an exact-bytes guarantee.
async fn create_session(
    base: &str,
    cwd: &str,
    invocation: &str,
    title: &str,
) -> Result<Session, String> {
    let url = format!("{base}/api/sessions");
    // `title` is the API's `Option<String>`, not a bare string: an empty
    // field means "auto-generate", per SPEC.md's "Title: optional;
    // auto-generated when omitted" — sending `Some("")` would instead ask
    // the supervisor to name the session the empty string.
    let title = (!title.trim().is_empty()).then_some(title);
    let body = serde_json::json!({ "cwd": cwd, "invocation": invocation, "title": title });
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp
            .text()
            .await
            .map_err(|error| format!("POST {url}: {status}: reading error response: {error}"))?;
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            format!("POST {url}: {status}")
        } else {
            detail.to_string()
        });
    }
    resp.json::<Session>().await.map_err(|e| e.to_string())
}

/// POST the stop endpoint for one session. The empty `Ok(())` on success
/// mirrors the helm's own reply (`{}` — see farhelm-helm's `stop_session`);
/// there is nothing in the body worth decoding.
///
/// A body-read failure (the connection dying mid-response, say) is
/// reported WITH the method/URL/status that were already known, the same
/// as `create_session` — swallowing it into an empty string via
/// `unwrap_or_default` would silently turn "we don't know why this
/// failed" into "it failed for no stated reason", which is a strictly
/// worse message for the per-session error line the caller renders. NOT
/// covered by an automated test: Playwright's `route.fulfill` cannot
/// truncate a body it is itself constructing, and this one straight-line
/// read-error path did not seem worth standing up a raw socket server in
/// the suite to reach — a regression here would show up as a generic
/// reqwest error string rather than a wrong one, not a silent failure.
async fn stop_session(base: &str, id: &str) -> Result<(), String> {
    let url = format!("{base}/api/sessions/{id}/stop");
    let resp = reqwest::Client::new()
        .post(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp
            .text()
            .await
            .map_err(|error| format!("POST {url}: {status}: reading error response: {error}"))?;
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            format!("POST {url}: {status}")
        } else {
            detail.to_string()
        });
    }
    Ok(())
}

/// DELETE a session. See `stop_session`'s docs — same error-surfacing
/// shape (including the body-read-failure context), different verb and
/// endpoint.
async fn delete_session(base: &str, id: &str) -> Result<(), String> {
    let url = format!("{base}/api/sessions/{id}");
    let resp = reqwest::Client::new()
        .delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp
            .text()
            .await
            .map_err(|error| format!("DELETE {url}: {status}: reading error response: {error}"))?;
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            format!("DELETE {url}: {status}")
        } else {
            detail.to_string()
        });
    }
    Ok(())
}

/// The subset of `Session` `on_delete` actually needs, in `ListView` below:
/// the id the API call targets, plus `status` to decide whether this click
/// deletes immediately or opens the inline confirm prompt. No `title` —
/// unlike before this file's eval-based `window.confirm()` was replaced
/// with `SessionRow`'s in-page one, `on_delete` itself never builds any
/// confirm wording anymore; `confirm_message` (near `SessionRow`) computes
/// that straight from the row's own live `session.title`/`session.status`
/// on every render instead, so a title never needs to travel through this
/// type at all. Deliberately narrower than the whole `Session` — `on_delete`
/// has no legitimate reason to depend on `cwd` or `invocation` either, and a
/// dedicated type is what makes that impossible rather than merely
/// unlikely, unlike `on_open` (which keeps taking the whole `Session`: it
/// needs every field to populate `SessionView`).
#[derive(Debug, Clone)]
struct DeleteTarget {
    id: String,
    status: SessionStatus,
}

/// The flat session list: title, cwd, invocation, and a truthful status
/// badge per row, refetched on a timer; the "new session" form and the
/// per-row stop/delete actions (PLAN_M2.md step 8) live here too, since
/// both need to reach into the same poll loop — a create or a stop should
/// be reflected as soon as the next poll runs, not held behind an
/// optimistic local edit.
///
/// The poll loop lives in a `use_future` scoped to this component, so it
/// is cancelled for free when `App` switches to `SessionView` and this
/// component unmounts — "polling stops while a terminal is open"
/// (PLAN_M2.md) falls out of Dioxus's own task lifecycle rather than
/// needing an explicit stop signal.
#[component]
fn ListView(on_open: EventHandler<Session>) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut listing = use_signal(|| None::<Result<SessionListing, String>>);
    // Per-session, not one shared slot: a stop failing on session A must
    // not blank out session B's still-fresh success (or vice versa), and
    // a LATER success on any session must not silently erase an EARLIER
    // failure on a different one — which a single `Option<String>` would
    // do on every write regardless of which session it was about. Keyed
    // by session id so each row renders only its own entry.
    let mut errors = use_signal(HashMap::<String, String>::new);
    // Which sessions have a stop/delete in flight right now (also keyed
    // by id): both disables that row's buttons (so a second click can't
    // race the first) and is the re-entry guard the click handlers check
    // before doing anything — belt-and-suspenders, since a disabled
    // button should already stop the click from firing, but the DOM
    // update disabling it is not synchronous with the click handler
    // itself.
    let mut pending = use_signal(HashSet::<String>::new);
    // Which sessions are showing the inline "confirm delete?" prompt in
    // place of their normal stop/delete buttons — see `on_delete` below.
    // Deliberately a plain client-side set with no timeout and no
    // poll-driven reset: a listing refresh must leave an in-progress
    // confirmation alone (the user is mid-decision, not mid-poll), so
    // this is intentionally NOT derived from `listing` on every render.
    // The one reconciliation that does happen is in the poll loop below,
    // which drops an entry once its session is no longer in the listing
    // at all (deleted from elsewhere, say) — there is no row left for a
    // dangling entry to ever affect, so this is tidiness, not correctness.
    let mut confirming = use_signal(HashSet::<String>::new);
    let mut show_create = use_signal(|| false);
    // Lifted out of `CreateSessionForm` rather than owned there: the
    // "new session" toggle button below needs to know whether a create is
    // in flight too, so it can refuse to unmount the form out from under
    // its own pending POST (see the toggle button's doc below).
    let submitting = use_signal(|| false);

    // Cloned once up front rather than moved into the poll loop below: a
    // `move ||` closure takes ownership of everything it captures, and
    // `on_stop`/`on_delete` need their own copy of `base` afterward.
    let poll_base = base.clone();
    use_future(move || {
        let base = poll_base.clone();
        async move {
            loop {
                let fetched = fetch_sessions(&base).await;
                // Drop any `confirming` entry whose session is gone from
                // this fetch entirely — the counterpart to the "a poll
                // refresh must not clear an in-progress confirmation"
                // rule just above: that rule protects a row that is
                // still LISTED, not one that has vanished (deleted from
                // another client while this one sat mid-confirmation, an
                // externally-imposed departure the `retain` below cannot
                // distinguish from the id simply never having existed).
                // Left off a failed fetch on purpose: an error reply
                // carries no session ids at all, and a transient fetch
                // failure is not evidence any session actually left.
                if let Ok(listing) = &fetched {
                    let live_ids: HashSet<&str> =
                        listing.sessions.iter().map(|s| s.id.as_str()).collect();
                    confirming
                        .write()
                        .retain(|id| live_ids.contains(id.as_str()));
                }
                listing.set(Some(fetched));
                // Inlined rather than a shared `sleep_ms` helper: this is
                // the only call site, and `tokio::time::sleep` is
                // unavailable on wasm32 (no reactor in the browser) while
                // `gloo-timers`' `TimeoutFuture` only works on wasm32 (a
                // `wasm-bindgen` binding to `setTimeout`) — each target
                // gets the idiom that already fits it. The desktop build
                // runs inside the tokio multi-thread runtime
                // `dioxus-desktop` itself constructs (see its
                // `launch.rs`), so `tokio::time::sleep` needs no extra
                // setup there.
                #[cfg(target_arch = "wasm32")]
                gloo_timers::future::TimeoutFuture::new(POLL_INTERVAL_MS as u32).await;
                #[cfg(not(target_arch = "wasm32"))]
                tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
            }
        }
    });

    let stop_base = base.clone();
    // Takes the id directly, not the whole `Session`: nothing past the
    // insert-into-`pending` check below reads any other field, so a
    // `Session` clone (and a second, redundant id clone off of it) would
    // only be dead weight — see `SessionRow`'s call site for the mirrored
    // simplification on the caller's side.
    let on_stop = move |id: String| {
        // Cross-guard against `confirming`, not just `pending`: the
        // stop/delete buttons are only ABSENT from the DOM once a
        // rerender following `confirming.insert` has actually landed
        // (see `SessionRow`'s doc), so a stop click queued just ahead of
        // that rerender — a rapid synthetic double-click, say — could
        // otherwise still reach this handler for a row that is, or is
        // about to be, showing the confirm prompt. Refusing here keeps
        // the row's two lifecycle handlers from ever racing each other
        // for the same id: without it, a stop could slip `id` into
        // `pending` WHILE a delete confirmation is open, and the eventual
        // "confirm delete" click would then silently no-op — NOT because
        // of anything in `confirm_delete` itself, but because `do_delete`
        // (which it calls after removing `confirming`) has its OWN
        // `pending`-insert re-entry guard, which would find the id
        // already occupied by that stop and bail with no error at all.
        if confirming.read().contains(&id) {
            return;
        }
        // Re-entry guard for the per-session in-flight set: a disabled
        // button should already stop this, but the click and the
        // re-render that disables it are not synchronous, so the handler
        // checks for itself too. `insert` returning `false` means an op
        // for this id was already running.
        if !pending.write().insert(id.clone()) {
            return;
        }
        let base = stop_base.clone();
        spawn(async move {
            // No optimistic flip (PLAN_M2.md design note): the row's
            // badge only ever reflects what the NEXT poll observes, so a
            // stop that silently failed can never leave the UI claiming a
            // session is exited when tmux still disagrees.
            let outcome = stop_session(&base, &id).await;
            match outcome.err() {
                Some(e) => {
                    errors.write().insert(id.clone(), format!("stop: {e}"));
                    pending.write().remove(&id);
                }
                None => {
                    errors.write().remove(&id);
                    // `pending` stays set across this extra fetch — not
                    // released until it completes: `on_delete`'s confirm
                    // wording is decided from the `status` the LATEST
                    // listing carries, and without this, an instant
                    // delete right after this stop would still see the
                    // stale pre-stop Alive (up to `POLL_INTERVAL_MS` old)
                    // and confirm with the wrong "is still running"
                    // wording for a session that just got stopped.
                    listing.set(Some(fetch_sessions(&base).await));
                    pending.write().remove(&id);
                }
            }
        });
    };

    let delete_base = base.clone();
    // The actual DELETE call, shared by both ways a delete can be
    // decided on: immediately for an Exited session, or after the user
    // hits "confirm delete" on the inline prompt for an Alive/Unknown
    // one (see `on_delete` and `confirm_delete` below, both of which
    // clone this closure rather than each reimplementing the request/
    // pending/error bookkeeping). Mirrors `on_stop`'s shape exactly,
    // `delete_session` and `errors`'/`pending`'s "delete:"-prefixed entry
    // in place of `on_stop`'s "stop:" one.
    let mut do_delete = move |id: String| {
        if !pending.write().insert(id.clone()) {
            return;
        }
        let base = delete_base.clone();
        spawn(async move {
            let outcome = delete_session(&base, &id).await;
            match outcome.err() {
                Some(e) => {
                    errors.write().insert(id.clone(), format!("delete: {e}"));
                    pending.write().remove(&id);
                }
                None => {
                    errors.write().remove(&id);
                    // Removed from the LOCAL listing immediately, before
                    // releasing `pending` — the one deliberate optimistic
                    // exception in this file (PLAN_M2.md's "no optimistic
                    // status flips" is about STATUS badges specifically;
                    // this is acting on a delete response already in
                    // hand, not guessing). Waiting for the next poll
                    // instead would leave the stale row's delete button
                    // re-enabled with nothing left server-side to delete
                    // — a second click in that window would 404 against
                    // an id that no longer exists, a confusing failure
                    // for an action that had already succeeded.
                    if let Some(Ok(current)) = listing.write().as_mut() {
                        current.sessions.retain(|s| s.id != id);
                    }
                    pending.write().remove(&id);
                }
            }
        });
    };

    // The delete button's initial click: decides whether this id needs
    // confirming at all, never itself calling the API.
    //
    // SPEC.md's "Lifecycle operations": delete confirms first only when
    // the agent might still be alive — an Exited session deletes
    // immediately (see its own arm below for the residual risk that
    // accepts). Alive and Unknown both confirm, entering the per-session
    // `confirming` state that `SessionRow` reads to swap its action area
    // (see that component's doc) — this closure itself does nothing more
    // than flip that flag; `confirm_delete` below is what a confirmed
    // click actually acts on.
    //
    // Refuses an id already in `pending` (the cross-guard mirroring
    // `on_stop`'s `confirming` check above): the delete button is only
    // disabled once a rerender following a `pending` insert lands, so a
    // rapid click queued just ahead of that rerender could otherwise
    // still reach this handler while, say, a stop is already in flight
    // for the same session. Refusing here is what keeps `confirming` from
    // ever being entered in that window — closing the door on the
    // opposite race `on_stop`'s guard closes. `do_delete`'s OWN
    // `pending`-insert guard (not `confirm_delete`'s `confirming`-removal
    // check, which exists for a different race entirely — see its own
    // doc below) would eventually refuse this same id too, but only AFTER
    // a confirm prompt had already opened for nothing; catching it here
    // means the prompt never opens in the first place.
    let mut do_delete_on_confirm = do_delete.clone();
    let on_delete = move |target: DeleteTarget| {
        if pending.read().contains(&target.id) {
            return;
        }
        match target.status {
            // Deliberately unconfirmed, a known residual: the AGENT
            // process has exited, but process-tree descendants it
            // spawned (a stray MCP server, a dev server) can outlive it,
            // and delete's process-tree sweep will kill whatever it
            // still finds. The UI has no way to know whether any such
            // descendant exists — only the supervisor's sweep does,
            // after the fact — so there is nothing concrete to report
            // here, and always confirming "just in case" would make
            // deleting routine, already-finished sessions needlessly
            // noisy. Revisit if M5's status work ever gives the UI a
            // basis for a sharper answer.
            SessionStatus::Exited { .. } => do_delete_on_confirm(target.id),
            // Unknown must not borrow Alive's "is still running" claim
            // it has no basis for — SPEC.md's no-guessing rule means an
            // unresolved status is presented as exactly that, uncertain,
            // never rounded up to a known-alive claim just because both
            // wordings end up confirming the same way. The DIFFERENT
            // wording itself lives in `SessionRow`, computed from
            // whatever `status` the row's own next render carries — not
            // captured here, since a status that changes while a
            // confirmation sits open (a session stopped from another
            // client, say) should be reflected in the prompt too.
            SessionStatus::Alive | SessionStatus::Unknown => {
                confirming.write().insert(target.id);
            }
        }
    };

    // The confirm-delete button's click, inside the inline prompt: the
    // exact same DELETE call an accepted `window.confirm()` used to
    // trigger before this rewrite, just reached from a different UI
    // widget. Clears `confirming` first so the row falls back to its
    // normal (busy/disabled) button layout the instant `do_delete`'s own
    // `pending` insert takes effect, rather than momentarily showing
    // both the prompt and a busy state.
    //
    // Proceeds ONLY when `remove` reports the id was actually present:
    // `HashSet::remove` returns `false` for an id already gone, which
    // happens whenever this confirmation was already resolved by
    // something else — `cancel_delete` running first (a queued confirm
    // click landing just after a cancel click, both fired in the same
    // burst), or a second confirm click racing the first's own removal.
    // Without this check, that second call would fall through to
    // `do_delete` regardless, which for the cancel-then-confirm race
    // would delete a session the user just told the UI to leave alone.
    let confirm_delete = move |id: String| {
        if !confirming.write().remove(&id) {
            return;
        }
        do_delete(id);
    };

    // The inline prompt's cancel button: just drops the flag. No API
    // call, no `pending` involvement — cancelling was never in flight to
    // begin with.
    let cancel_delete = move |id: String| {
        confirming.write().remove(&id);
    };

    // Whether ANY row's open button should be disabled right now.
    // Opening a row navigates `App` away from `ListView` entirely (see
    // the module docs) — that unmounts this whole component, and with it
    // every task it owns, `spawn`ed or not: a create or a stop/delete
    // still in flight would have its eventual result silently discarded
    // instead of ever being acted on. This has to be a single flag
    // covering every row rather than a per-row one, because it is
    // `ListView` ITSELF that would unmount — every row's open action is
    // equally unsafe while ANYTHING is in flight, not just the row whose
    // own operation is running. A finer-grained rule (only the busy row's
    // own open button disabled, say) would need operations to be owned by
    // something that outlives this component instead, which is what M5's
    // live-push channel could plausibly provide; M2 has nothing of the
    // kind, so the global lock is what today's ownership model can
    // actually promise.
    let nav_locked = submitting() || !pending.read().is_empty();

    rsx! {
        div { class: "list-toolbar",
            button {
                r#type: "button",
                class: "btn new-session-button",
                // Disabled while a create is in flight: this is the
                // form's only cancel/close affordance, and toggling
                // `show_create` off would unmount `CreateSessionForm`
                // mid-POST — dropping the component drops its `spawn`ed
                // task's ability to ever act on the response, silently
                // losing track of whether the create actually happened.
                // Disabling the one control that can cause that unmount
                // is simpler and more robust than trying to keep a
                // detached task's result meaningful after the fact.
                disabled: submitting(),
                onclick: move |_| {
                    // Signal-level re-entry check, not just the
                    // `disabled` attribute above: the attribute's DOM
                    // update from a rerender is not synchronous with a
                    // click event, so a second click landing in that gap
                    // would still reach this handler even though the
                    // button already looked disabled.
                    if submitting() {
                        return;
                    }
                    show_create.set(!show_create());
                },
                "new session"
            }
        }
        if show_create() {
            CreateSessionForm {
                submitting,
                on_created: move |session| {
                    show_create.set(false);
                    on_open.call(session);
                },
            }
        }
        match &*listing.read() {
            None => rsx! { div { class: "status", "loading sessions…" } },
            Some(Err(e)) => rsx! {
                div { class: "status error", "failed to load sessions: {e}" }
            },
            Some(Ok(listing)) => rsx! {
                if listing.sessions.is_empty() && listing.total == 0 {
                    div { class: "status", "no sessions" }
                } else {
                    if listing.truncated {
                        // PLAN_M2.md acceptance 5: the cap and truncated
                        // flag exist to be shown, not just plumbed —
                        // silently presenting a partial list would look
                        // like a complete one.
                        div { class: "banner truncation-banner",
                            "showing {listing.sessions.len()} of {listing.total} sessions"
                        }
                    }
                    div { class: "session-list",
                        for session in listing.sessions.iter() {
                            SessionRow {
                                key: "{session.id}",
                                session: session.clone(),
                                error: errors.read().get(&session.id).cloned(),
                                busy: pending.read().contains(&session.id),
                                confirming: confirming.read().contains(&session.id),
                                nav_disabled: nav_locked,
                                on_open,
                                on_stop: on_stop.clone(),
                                on_delete: on_delete.clone(),
                                on_confirm_delete: confirm_delete.clone(),
                                on_cancel_delete: cancel_delete,
                            }
                        }
                    }
                }
            },
        }
    }
}

/// Inline create form (PLAN_M2.md step 8's "not a modal library" design
/// choice): working directory and agent command are required, title is
/// optional. Lives entirely inside `ListView` — there is no route or
/// signal for it beyond the `show_create` toggle that mounts/unmounts it.
///
/// `submitting` is owned by the CALLER (`ListView`), not this component:
/// `ListView`'s own "new session" toggle button needs to see it too, so it
/// can refuse to unmount this form while a create is still in flight —
/// dropping this component mid-`spawn` would strand the POST's eventual
/// response with nothing left to act on it. Lifting the flag up is
/// simpler than trying to keep a detached task meaningful after the fact.
///
/// `on_created` fires only on a successful POST, with the newly created
/// `Session` from the response body; `ListView` uses that to both close
/// the form and navigate straight into the new session's terminal
/// (SPEC.md: "creation launches the agent; you type your first prompt
/// into its terminal"). On failure the form stays mounted with its values
/// untouched and the error text rendered next to it — the fields are
/// plain `use_signal<String>`s rather than being reset or lifted into
/// `ListView`, so "form contents preserved" falls out of simply not
/// clearing them rather than needing a restore step. On success the
/// fields are left as-is too: `on_created` drives `ListView` to unmount
/// this whole component immediately (closing the form and navigating
/// away), so there is no one left to observe a reset — only the failure
/// path needs to leave the control usable again.
#[component]
fn CreateSessionForm(mut submitting: Signal<bool>, on_created: EventHandler<Session>) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut cwd = use_signal(String::new);
    let mut invocation = use_signal(String::new);
    let mut title = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        form {
            class: "create-session-form",
            onsubmit: move |evt| {
                evt.prevent_default();
                // Double-submission guard: covers concurrent clicks on
                // THIS mounted form (a double-click, or a stray repeat
                // event) — the control is inert for the whole round trip,
                // not just until the first click handler returns. It does
                // NOT cover a retry after an ambiguous transport failure
                // (request sent, response lost) reaching the supervisor a
                // second time; closing that gap needs server-side
                // deduplication, which PLAN.md's M3 entry ("server-enforced
                // create idempotency") schedules as durability-milestone
                // work, not something a client-side flag can provide.
                if submitting() {
                    return;
                }
                let base = base.clone();
                let cwd_value = cwd();
                let invocation_value = invocation();
                let title_value = title();
                submitting.set(true);
                error.set(None);
                spawn(async move {
                    match create_session(&base, &cwd_value, &invocation_value, &title_value).await
                    {
                        Ok(session) => on_created.call(session),
                        Err(e) => {
                            error.set(Some(e));
                            submitting.set(false);
                        }
                    }
                });
            },
            // Working directory and agent command are literal text that
            // gets EXECUTED, never prose — OS-level text mangling has no
            // way to tell the difference and "corrects" them anyway
            // (observed directly: WKWebView's autocorrect silently
            // substituting "claude" with "Claude" in place, with no
            // visible suggestion popup to catch and reject). A
            // capitalized command or a suggestion-popup keystroke
            // swallowed mid-path corrupts what actually runs. Title IS
            // ordinary prose, but the same opt-out applies to it too, for
            // a narrower reason: whatever the user types is what should
            // come back out verbatim (SPEC.md's "auto-generated when
            // omitted" is the only substitution this field ever gets, and
            // it happens server-side, deliberately, not as a silent
            // client-side "helpful" rewrite) — so every input here opts
            // out of every form of text mangling a browser might apply on
            // its own, for whichever of these two reasons applies to it.
            label {
                "working directory"
                input {
                    r#type: "text",
                    required: true,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{cwd}",
                    oninput: move |evt| cwd.set(evt.value()),
                }
            }
            label {
                "agent command"
                input {
                    r#type: "text",
                    required: true,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{invocation}",
                    oninput: move |evt| invocation.set(evt.value()),
                }
            }
            label {
                "title (optional)"
                input {
                    r#type: "text",
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{title}",
                    oninput: move |evt| title.set(evt.value()),
                }
            }
            button {
                r#type: "submit",
                class: "btn create-session-submit",
                disabled: submitting(),
                "create"
            }
            if let Some(err) = error.read().clone() {
                div { class: "create-session-error", "{err}" }
            }
        }
    }
}

/// One row: a plain `<div>` wrapper around three real `<button>`s (open,
/// stop, delete) rather than a `div` with `role`/`tabindex`/a hand-rolled
/// `onkeydown` — every action gets Enter- and Space-activation, focus
/// styling, and screen-reader semantics for free from being a real
/// button, and none of it needs reimplementing here (a hand-rolled div
/// also had a latent bug: Space on a focused element scrolls the page
/// unless the handler prevents default, which native button activation
/// never triggers in the first place).
///
/// The wrapper is a `div`, not a `button`, because M1's whole-row button
/// cannot host the stop/delete actions PLAN_M2.md step 8 adds: HTML
/// forbids interactive content nested inside a `<button>`, so a `<button
/// class="session-row">` containing further `<button>`s would be invalid
/// markup with undefined browser behavior. Splitting the open action into
/// its own sibling button (`.session-row-open`) keeps every action a real,
/// individually focusable button while satisfying that constraint — tab
/// order simply walks open → stop → delete across the row in the NORMAL
/// (not-confirming) state; see "Inline delete confirmation" below for how
/// that changes while a delete prompt is open.
///
/// `data-session-id` stays on the outer wrapper for Playwright to key off
/// of. No `stop_propagation` on the stop/delete buttons: the wrapper
/// `<div>` has no click handler of its own for a click to bubble into
/// (the open action lives on its own sibling button), so there is nothing
/// here for propagation to trigger by accident.
///
/// `error` is this row's own entry from `ListView`'s per-session error map
/// (`None` when its last action succeeded or none has run yet); `busy` is
/// whether a stop/delete for THIS session is currently in flight, which
/// disables both action buttons — both are `ListView`'s per-session state
/// (see its own docs for why a single shared error/in-flight slot would be
/// wrong), just narrowed to this one row by the caller before it gets here.
///
/// `nav_disabled`, unlike `error`/`busy`, is NOT this row's own state — it
/// is `ListView`'s single global "something is in flight somewhere" flag
/// (see `nav_locked` in `ListView`), applied identically to every row's
/// open button. Opening ANY row unmounts `ListView` (navigation replaces
/// it with `SessionView`), which would silently cancel whatever this row
/// OR any other row's in-flight create/stop/delete was doing — the whole
/// point of disabling it is to keep that unmount from happening at all
/// while anything still needs `ListView` to stay alive to finish.
///
/// ## Inline delete confirmation
///
/// `confirming` (also `ListView`'s per-session state, same discipline as
/// `error`/`busy`) swaps the stop/delete pair out for a prompt plus
/// **confirm delete**/**cancel** in place — there used to be a
/// `window.confirm()` call here instead; wry ships no native JS dialogs on
/// macOS's WKWebView (observed directly running the desktop build), which
/// made delete-on-a-live-session silently do nothing on that target. An
/// in-page prompt has no such platform dependency, so it replaces the
/// eval-based one everywhere, not just on desktop. While it is open, tab
/// order instead walks open (disabled) → confirm delete → cancel; initial
/// FOCUS lands directly on cancel regardless of tab order (see below).
///
/// The prompt itself is TWO separate elements, not one combined sentence:
/// `.confirm-consequence` (from `confirm_consequence`, fixed wording with
/// no title in it at all) and `.confirm-title` (the title alone, quoted).
/// Both are ordinary Dioxus text interpolation, never `document::eval` —
/// a title containing quotes or JS-source-looking text (plausible, since a
/// supervisor over `--ssh` may be a different, possibly untrusted host)
/// renders as inert DOM text, never something parsed as script, which is
/// what makes the injection concern the old eval path needed
/// `serde_json`-encoding to guard against moot on this path. The SPLIT
/// exists for a different reason: a legal title can be tens of KB with no
/// whitespace at all, and app.css lets `.confirm-title` (only) shrink and
/// ellipsize under space pressure while `.confirm-consequence` never
/// shrinks — so the safety-critical "will be killed" half can never be
/// the one that gets clipped, which a single combined, single-ellipsized
/// string could not promise once the title ran long enough. Rendering the
/// consequence element FIRST is what makes "read the risk before the
/// title" the actual reading order, not just an incidental visual
/// side effect that a later DOM-order change could quietly undo.
///
/// Focus-on-open uses the plain HTML `autofocus` attribute on the cancel
/// button (below), not Dioxus's `onmounted`/`set_focus` API: `set_focus`
/// returns a `Result` future that can fail (`MountedError`, e.g. on a
/// renderer that does not support it), and since focus-on-cancel is a
/// safety default — landing keyboard focus on the SAFE action before a
/// stray Enter/Space can reach anything — silently discarding that
/// `Result` would let the safety behavior vanish with nothing to show for
/// it. `autofocus` cannot fail the same way: it is applied by the browser
/// itself at parse/insert time as a plain attribute, with no fallible
/// async call in the UI's own control to get wrong or ignore. It reliably
/// fires exactly once per entry into `confirming` for the same reason
/// `onmounted` would have: the button is only ever created fresh inside
/// the `if confirming` branch below.
#[component]
fn SessionRow(
    session: Session,
    error: Option<String>,
    busy: bool,
    confirming: bool,
    nav_disabled: bool,
    on_open: EventHandler<Session>,
    on_stop: EventHandler<String>,
    on_delete: EventHandler<DeleteTarget>,
    on_confirm_delete: EventHandler<String>,
    on_cancel_delete: EventHandler<String>,
) -> Element {
    let (badge_class, badge_text) = status_badge(&session.status);
    let open_session = session.clone();
    let stop_id = session.id.clone();
    let delete_target = DeleteTarget {
        id: session.id.clone(),
        status: session.status.clone(),
    };
    let confirm_id = session.id.clone();
    let cancel_id = session.id.clone();

    rsx! {
        div {
            class: "session-row",
            "data-session-id": "{session.id}",
            // Two stacked rows, not one: the buttons need a plain flex
            // ROW (see `.session-row-main` in app.css), but a per-session
            // error line needs its own full-width row underneath rather
            // than squeezing in as a fourth flex item next to the
            // buttons — hence the extra wrapper rather than putting
            // everything directly under `.session-row`.
            div { class: "session-row-main",
                button {
                    r#type: "button",
                    class: "session-row-open",
                    // Disabled by EITHER lock: the global nav lock (any
                    // in-flight op anywhere), or this row's own
                    // confirmation being open — the simplest way to
                    // satisfy "cancel is the only way back to normal"
                    // (see the component doc above) is to make the open
                    // button inert for the whole time the prompt is
                    // showing, rather than giving it a second, competing
                    // meaning as an implicit cancel.
                    disabled: nav_disabled || confirming,
                    onclick: move |_| on_open.call(open_session.clone()),
                    span { class: "session-title", "{session.title}" }
                    span { class: "session-cwd", "{session.cwd}" }
                    span { class: "session-invocation", "{session.invocation}" }
                    span { class: "status-badge {badge_class}", "{badge_text}" }
                }
                if confirming {
                    // Called inline, not hoisted into a `let` above this
                    // `if`: this is the ONLY place either half of the
                    // prompt is ever shown, so computing them
                    // unconditionally on every render regardless of
                    // `confirming` would be wasted work on the common
                    // (not-confirming) case, and — since `confirm_consequence`
                    // is documented as never being CALLED outside this
                    // state (see its own doc) — computing it only here is
                    // what actually keeps that contract true rather than
                    // just asserted.
                    //
                    // Two elements, consequence first: see the component
                    // doc above for why an untruncatable consequence and
                    // a separately truncatable title, in THIS order, is
                    // what keeps a long title from ever clipping the
                    // safety-critical half.
                    span {
                        class: "confirm-consequence",
                        "{confirm_consequence(&session.status)}"
                    }
                    span { class: "confirm-title", "\"{session.title}\"" }
                    button {
                        r#type: "button",
                        class: "btn confirm-delete",
                        onclick: move |_| on_confirm_delete.call(confirm_id.clone()),
                        "confirm delete"
                    }
                    button {
                        r#type: "button",
                        class: "btn confirm-cancel",
                        // Safe default: land keyboard focus on cancel, not
                        // confirm, the instant this prompt appears — a
                        // stray Enter/Space right after the row's delete
                        // click (residual focus, a fast typist) then backs
                        // OUT of the destructive action instead of into
                        // it. Declarative `autofocus`, not `onmounted` +
                        // `set_focus`: see the component doc above for why
                        // the fallible, discardable-`Result` async API was
                        // rejected in favor of a plain HTML attribute that
                        // cannot silently fail to apply.
                        autofocus: true,
                        onclick: move |_| on_cancel_delete.call(cancel_id.clone()),
                        "cancel"
                    }
                } else {
                    button {
                        r#type: "button",
                        class: "btn session-row-stop",
                        disabled: busy,
                        onclick: move |_| on_stop.call(stop_id.clone()),
                        "stop"
                    }
                    button {
                        r#type: "button",
                        class: "btn session-row-delete",
                        disabled: busy,
                        onclick: move |_| on_delete.call(delete_target.clone()),
                        "delete"
                    }
                }
            }
            if let Some(err) = &error {
                div { class: "action-error", "{err}" }
            }
        }
    }
}

/// Map a status to its badge's CSS modifier class and display text.
/// Kept as one function so the four cases (`Exited` has two — a known
/// code vs. an unrepresentable one) stay next to each other instead of
/// drifting apart across separate match arms in the render tree.
fn status_badge(status: &SessionStatus) -> (&'static str, String) {
    match status {
        SessionStatus::Alive => ("alive", "alive".to_string()),
        SessionStatus::Exited {
            exit_code: Some(code),
        } => ("exited", format!("exited (code {code})")),
        SessionStatus::Exited { exit_code: None } => ("exited", "exited".to_string()),
        SessionStatus::Unknown => ("unknown", "unknown".to_string()),
    }
}

/// The safety-critical half of the inline delete-confirmation prompt:
/// what deleting THIS session will actually do, worded so the risk reads
/// on its own without depending on the title. Rendered into its own
/// untruncatable DOM element (`SessionRow`'s `.confirm-consequence`, never
/// ellipsized) AHEAD of the title, which gets a separate, deliberately
/// truncatable element instead — a legal title can be tens of KB with no
/// whitespace at all, and the earlier single-string design (title
/// embedded mid-sentence, the whole thing ellipsized as one span) would
/// clip whichever half landed at the tail once a title ran long enough,
/// which for that wording was always this one: the actual consequence, is
/// still running and will be killed. Splitting the two apart, consequence
/// first, is what makes that unclippable regardless of title length.
///
/// Only ever CALLED for `Alive`/`Unknown` while genuinely confirming — see
/// its own call site in `SessionRow`, gated on `confirming` — but is
/// written total over `SessionStatus` rather than partial, because
/// `confirming` is `ListView`'s own state, decoupled from any single
/// render: a session that was `Alive` when the user opened this prompt
/// can flip to `Exited` under it (stopped from another client, say)
/// before either button is clicked, and this function re-runs on every
/// render off whatever status the row's LATEST prop carries. The
/// `Exited` arm is that residual case's fallback, not a wording SPEC.md's
/// confirm-contract actually specifies.
fn confirm_consequence(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Alive => "still running — deleting kills the agent:",
        SessionStatus::Unknown => {
            "status unknown — the agent may still be running and will be killed:"
        }
        SessionStatus::Exited { .. } => "delete anyway:",
    }
}

/// One session, terminal filling the window, with a back control above
/// it. The terminal div is handed to the JS island on mount; Dioxus
/// never touches its children again — that boundary is the whole
/// design.
///
/// ## Mount/unmount lifecycle (PLAN_M2.md step 7)
///
/// M1 never unmounted this component (it was the only view), so
/// `terminal.js` only ever needed a mount-time guard against a
/// re-render calling `mount()` twice. Now that `App` can navigate away
/// and back, this component must clean up on drop — otherwise that
/// guard (terminal.js's `active`, now the ONLY mount guard — see its
/// own docs) would permanently wedge shut, and reopening ANY session
/// after the first would silently no-op `mount()` instead of attaching.
/// `use_drop` fires `farhelmTerm.unmount()` for exactly that reason: it
/// is the regression this lifecycle work exists to prevent, not a
/// hypothetical.
///
/// ## The mount-generation token
///
/// `window.__farhelmMountGeneration` guards only the OUTER wait here —
/// for `window.farhelmTerm` to exist at all, since terminal.js's
/// `document::Script` is injected asynchronously and this component can
/// render before it has executed (a real, if rare, race). Bumping the
/// counter on every mount attempt AND on drop means backing out before
/// that wait resolves reliably cancels it. Once `window.farhelmTerm`
/// exists, `mountWhenReady`'s OWN wait for xterm's globals is a separate
/// concern guarded entirely inside terminal.js (its `pending`
/// replacement-and-clear scheme — see that function's docs); this token
/// does not reach that far.
#[component]
fn SessionView(session: Session, on_back: EventHandler<()>) -> Element {
    let base = use_context::<ApiBase>().0;
    let session_id = session.id.clone();

    use_effect(move || {
        // Values are JSON-encoded, never interpolated raw: the session
        // id comes from a supervisor, which with --ssh is a different
        // machine. A hostile or compromised host returning an id
        // containing a quote would otherwise get arbitrary JavaScript
        // running on the helm's origin — turning a remote-host
        // compromise into control of the local helm API.
        let path = serde_json::to_string(&format!("/api/sessions/{session_id}/term"))
            .expect("string is serializable");
        let base_js = serde_json::to_string(&base).expect("string is serializable");
        let js = format!(
            r#"(function() {{
                var gen = (window.__farhelmMountGeneration || 0) + 1;
                window.__farhelmMountGeneration = gen;
                (function waitForIsland() {{
                    if (window.__farhelmMountGeneration !== gen) return;
                    if (window.farhelmTerm) {{
                        farhelmTerm.mountWhenReady('terminal', {path}, {base_js});
                    }} else {{
                        setTimeout(waitForIsland, 50);
                    }}
                }})();
            }})();"#
        );
        document::eval(&js);
    });

    use_drop(|| {
        // Bump the generation FIRST so an in-flight outer wait (see the
        // module docs above) aborts before `unmount()` below can race
        // it; `unmount()` itself is what cancels `mountWhenReady`'s own
        // wait, once the outer one has already handed off to it.
        // Fire-and-forget: this runs on the way out (navigating back to
        // the list, or the whole app tearing down), so there is no
        // reactive state left to update with a result, and the JS side
        // is already written to be a no-op if nothing is mounted.
        document::eval(
            "window.__farhelmMountGeneration = (window.__farhelmMountGeneration || 0) + 1; \
             if (window.farhelmTerm) { farhelmTerm.unmount(); }",
        );
    });

    rsx! {
        div { class: "layout",
            header { class: "titlebar",
                button {
                    class: "btn back-button",
                    onclick: move |_| on_back.call(()),
                    "← back",
                }
                span { class: "title", "{session.title}" }
                span { class: "meta", "{session.cwd} — {session.invocation}" }
            }
            div { id: "term-banner", class: "banner" }
            div { id: "terminal", class: "terminal" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str, title: &str) -> Session {
        Session {
            id: id.into(),
            title: title.into(),
            cwd: format!("/{id}"),
            invocation: format!("agent-{id}"),
            status: SessionStatus::Unknown,
        }
    }

    /// The list's display order must be stable even though the
    /// supervisor's HashMap-backed wire order is not (M1 regression
    /// this helper originally existed to prevent for `first()`; it
    /// carries the same requirement for the whole list now). Reversing
    /// the response must not change the rendered order.
    #[test]
    fn session_order_is_stable_across_wire_orders() {
        let a = session("a", "A");
        let b = session("b", "B");

        let forward = sort_sessions(vec![a.clone(), b.clone()]);
        let reverse = sort_sessions(vec![b, a]);
        assert_eq!(
            forward.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!(
            reverse.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    /// Pins BOTH the badge's display text and its CSS modifier class per
    /// status — not just the text — since a class regression (e.g. an
    /// `Exited` row silently keeping the `alive` class) would only
    /// otherwise surface as a wrong-COLORED row in the browser, which no
    /// text-only assertion here would ever catch.
    #[test]
    fn status_badge_matches_text_and_class_for_each_status() {
        assert_eq!(
            status_badge(&SessionStatus::Alive),
            ("alive", "alive".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: Some(7) }),
            ("exited", "exited (code 7)".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: None }),
            ("exited", "exited".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Unknown),
            ("unknown", "unknown".to_string())
        );
    }

    /// Pins the exact two confirm-prompt wordings SPEC.md's no-guessing
    /// rule requires to stay distinct: `Alive` must claim the agent IS
    /// running, while `Unknown` must only ever admit uncertainty — a
    /// regression that quietly reused one string for both (or rounded
    /// `Unknown` up to `Alive`'s wording) is exactly what this guards
    /// against. Scoped to `confirm_consequence`'s own string-building
    /// alone — it says nothing about how `SessionRow` later renders the
    /// result, nor about the SEPARATE title element sitting next to it
    /// (both exercised by the Playwright suite instead, not by anything
    /// callable from this unit test).
    #[test]
    fn confirm_consequence_wording_differs_between_alive_and_unknown() {
        assert_eq!(
            confirm_consequence(&SessionStatus::Alive),
            "still running — deleting kills the agent:"
        );
        assert_eq!(
            confirm_consequence(&SessionStatus::Unknown),
            "status unknown — the agent may still be running and will be killed:"
        );
    }

    /// An old-shaped `Session` JSON (no `status` field at all — exactly
    /// what a pre-M2 peer would send) must decode as `Unknown`, mirroring
    /// farhelm-proto's own decode-tolerance contract for
    /// `SessionInfo::status`. A silent default of, say, `Alive` would be
    /// a fabricated liveness claim.
    #[test]
    fn session_without_status_field_decodes_as_unknown() {
        let json = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
        });
        let decoded: Session = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.status, SessionStatus::Unknown);
    }

    /// The other half of `SessionListing`'s missing-field tolerance
    /// (mirrors `session_without_status_field_decodes_as_unknown` above,
    /// and farhelm-proto's own
    /// `old_shape_session_list_json_decodes_with_defaulted_new_fields`):
    /// a reply with no `total`/`truncated` keys at all must decode with
    /// both defaulted to their empty-safe values (0 and `false`) rather
    /// than failing.
    ///
    /// NOT a claim that this covers talking to an actual pre-M2 helm:
    /// the real M1-to-M2 change to `GET /api/sessions` replaced a bare
    /// JSON array with this object entirely (farhelm-helm's own docs
    /// call it "a breaking shape change from M1's bare array"), which
    /// `SessionListing` cannot decode either way — there is no array
    /// fallback here. What this pins is forward tolerance WITHIN the
    /// object shape: a later field added to `SessionListing` must still
    /// let a build one step behind decode today's object, the same
    /// tolerance farhelm-proto's own wire types carry.
    #[test]
    fn session_listing_without_total_or_truncated_defaults_both() {
        let json = serde_json::json!({ "sessions": [] });
        let decoded: SessionListing = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.total, 0);
        assert!(!decoded.truncated);
    }
}
