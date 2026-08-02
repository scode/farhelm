//! The Farhelm UI: one Dioxus component tree, two targets.
//!
//! The same components render as the web app (wasm32, real DOM, served
//! by the helm at loopback) and the desktop app (wry webview). The
//! terminals themselves are xterm.js islands (assets/terminal.js) whose
//! byte paths bypass Dioxus entirely — Dioxus owns the chrome around a
//! terminal, never its content (SPEC_impl.md, "Terminal widget"). Since
//! M4's terminal tabs a session view holds SEVERAL of those islands at
//! once, all attached concurrently; the boundary is unchanged, only its
//! multiplicity (see [`SessionView`]).
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
//! tabs (or something else) actually demands one. M4's tabs came and went
//! without demanding one: a tab selection is view-local state, not a
//! location, and nothing links to a specific tab.

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
    /// The host rebooted while this session was still live — launching,
    /// running, or with a stop in flight — so its terminal is gone and
    /// nothing can ever be asked about the agent again (PLAN_M3.md item
    /// 2). Deliberately its own state rather than folded into `Exited`:
    /// the user is being told the system LOST TRACK, not that their agent
    /// finished — the two call for different actions (restart-with-resume
    /// vs. nothing).
    Interrupted,
    /// The agent could not be started at all — the launch shim's
    /// exec-failure sentinel (PLAN_M3.md item 3), read by the supervisor
    /// and surfaced here with `detail` carrying its own recorded report
    /// (errno, argv0, or which pre-exec step failed) verbatim. Distinct
    /// from `Exited`: the agent never ran, so there is nothing to say it
    /// "finished" — a failed exec and a command that ran and died look
    /// identical to tmux, and only the supervisor's own sentinel read
    /// tells them apart (see farhelm-proto's `SessionStatus::Error`).
    Error { detail: String },
}

/// Mirror of the helm's restart-offer JSON (farhelm-proto `RestartOffer`):
/// what restarting THIS session would do to its conversation, as the
/// supervisor currently understands it (PLAN_M3.md items 7-9).
///
/// The UI never derives this — it cannot see a session's integration
/// snapshot or its captured conversation identity — so the only honest
/// thing it can do with a reply that carries no `restart_offer` at all is
/// take the same safe default the wire type takes: `FreshOnly`. Defaulting
/// toward "captured" would let the UI offer a resume the supervisor would
/// then refuse.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RestartOffer {
    /// Nothing captured and no configured fallback: restart can only
    /// launch a fresh agent.
    #[default]
    FreshOnly,
    /// This session's own conversation was captured; restart resumes
    /// exactly it.
    Resume,
    /// No captured identity, but the session carries an explicit
    /// placeholder-free resume command that restart runs verbatim. Kept
    /// distinct from `FreshOnly` because the user configured it — SPEC.md
    /// requires it be labeled honestly rather than as a plain fresh launch.
    FallbackTemplate,
}

/// The wire spelling of the restart mode this offer authorizes, and the
/// ONLY mode the supervisor will accept for it.
///
/// The pairing is exact in both directions, which is why this is a function
/// of the offer rather than a user choice: SPEC.md has no fresh-restart
/// variant in v1 ("for a clean conversation, create a new session in the
/// same directory"), so a session that CAN resume has no legal "restart
/// fresh instead" — and a session that cannot has nothing to resume. The
/// supervisor rejects any other pairing with a conflict naming the current
/// offer, which is exactly the staleness case this UI handles by
/// refreshing (see `SessionView`).
fn restart_mode_for(offer: RestartOffer) -> &'static str {
    match offer {
        RestartOffer::FreshOnly => "fresh",
        RestartOffer::Resume => "resume",
        RestartOffer::FallbackTemplate => "fallback_template",
    }
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
    /// SPEC.md's qualifier on an ended session — "stopped by user" is the
    /// only one that exists (PLAN_M3.md item 4). Rendered as part of the
    /// status badge rather than as its own control, because SPEC.md is
    /// explicit that "stopped" is NOT a distinct status: it is how an
    /// exited session says who ended it. No `#[serde(default)]` is needed
    /// or present, unlike the fields above: serde already decodes a
    /// missing key on an `Option` as `None`, so the same old-peer
    /// tolerance holds without the attribute.
    pub annotation: Option<String>,
    /// What restarting this session would do to its conversation — the
    /// supervisor recomputes it on every reply, so a session whose identity
    /// was captured a moment ago starts offering a resume without anything
    /// here having to ask. `#[serde(default)]` for the same old-peer
    /// tolerance as `status`, defaulting to the safe `FreshOnly`.
    #[serde(default)]
    pub restart_offer: RestartOffer,
    /// The session's terminal tabs, in the supervisor's creation order
    /// (PLAN_M4.md item 6). This is the ONE authoritative statement of
    /// which tabs exist and in what order — a tab-open reply deliberately
    /// says nothing about ordering (farhelm-proto's `TabOpened`), so the
    /// positional labels the strip renders are derived from this list, not
    /// from the order this client happened to open things in.
    ///
    /// Carried on BOTH routes, and both matter to the session view: the
    /// listing is where its FIRST tab snapshot comes from (the `Session`
    /// the list hands `SessionView` when a row is opened is already
    /// populated, so a session with tabs renders its strip on the first
    /// frame rather than after a round trip), and the detail poll is what
    /// keeps it current afterwards.
    ///
    /// `#[serde(default)]` for the same old-peer tolerance as `status` —
    /// and, unlike `status`, the default is also the everyday case: a
    /// session with no tabs.
    #[serde(default)]
    pub tabs: Vec<Tab>,
}

/// Mirror of the helm's tab JSON (farhelm-proto `TabInfo`): an opaque,
/// supervisor-minted id and nothing else.
///
/// Deliberately as minimal as the wire type. SPEC.md gives tabs no names
/// and close is their only operation, so an id is the whole identity —
/// labels are positional and computed at render time (see `tab_label`).
/// The id is echoed back verbatim on the terminal WebSocket's `?tab=` and
/// on the close request; this UI never parses it.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Tab {
    pub id: String,
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

// ---------------------------------------------------------------------
// URL building
//
// Every identifier this UI puts in a URL came from a supervisor, which
// under `--ssh` is a DIFFERENT and possibly untrusted machine. Neither
// helper below trusts the shape of what it is given; both exist so that a
// hostile or merely buggy id cannot change which resource a request names.
// ---------------------------------------------------------------------

/// Percent-encode one URL query value (RFC 3986's unreserved set passes
/// through; everything else becomes `%XX`).
///
/// An unescaped `&` or `#` in a tab id or a lease would silently truncate
/// or re-split the terminal WebSocket's query and attach the WRONG
/// terminal, which is precisely the failure `TerminalSelector::Tab`'s docs
/// call worse than an outright error.
///
/// Encodes per BYTE, so non-ASCII is UTF-8 percent-encoded correctly
/// rather than mangled. Hand-rolled rather than pulling in a crate for two
/// tiny functions.
fn encode_query_value(value: &str) -> String {
    encode_bytes(
        value,
        |byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~'),
    )
}

/// Percent-encode one value that will occupy exactly ONE path segment.
///
/// Strictly narrower than `encode_query_value`, and the two exclusions are
/// the whole point:
///
/// - `/` would end the segment, so a tab id of `../../victim` would turn
///   `DELETE /api/sessions/S/tabs/<id>` into `DELETE /api/sessions/victim`
///   — a supervisor-supplied string choosing which SESSION this UI
///   deletes. Not theoretical: URL parsers resolve dot segments before the
///   request is ever sent, so the traversal happens client-side, below
///   anything the helm could refuse.
/// - `.` passes through the query-value set, but a segment that is exactly
///   `.` or `..` is *itself* a dot segment and gets resolved away even
///   with no slash in sight — `.../tabs/..` normalizes to the session
///   route. Encoding `.` everywhere in a segment is the cheap way to make
///   that impossible without special-casing two literals, and costs
///   nothing for the ids this actually carries (UUIDs contain none).
///
/// Not a claim that the resulting id EXISTS — an id that survives escaping
/// and names nothing is an ordinary 404, which is the honest outcome.
fn encode_path_segment(value: &str) -> String {
    encode_bytes(
        value,
        |byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'~'),
    )
}

/// Shared body of the two encoders above: keep the bytes `keep` accepts,
/// percent-encode every other byte. Split out so the two differ only in
/// their allowed set, which is the only thing that should ever distinguish
/// them.
fn encode_bytes(value: &str, keep: impl Fn(u8) -> bool) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if keep(byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
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

/// How often the views refetch (PLAN_M2.md: "Polling for list freshness"
/// is the M2 mechanism; live push is out of scope until M5).
///
/// Shared by both polls, deliberately one constant rather than two:
/// `ListView` polls the listing and `SessionView` polls its own session's
/// detail for tab-list changes (PLAN_M4.md item 6 asks for "the same
/// polling M2 settled for the session list"), and M5 replaces both with
/// live push together, so a divergence here would be a difference no one
/// chose and no one would maintain.
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
///
/// `intent_key` is the create's idempotency key (PLAN_M3.md item 6): the
/// server treats two requests carrying the same key and the same fields as
/// ONE intended create, so a retry after an ambiguous failure returns the
/// original session instead of launching a second agent. It is a required
/// argument, not an option — a create from this UI always carries one (see
/// `CreateSessionForm`).
async fn create_session(
    base: &str,
    cwd: &str,
    invocation: &str,
    title: &str,
    intent_key: &str,
) -> Result<Session, String> {
    let url = format!("{base}/api/sessions");
    // `title` is the API's `Option<String>`, not a bare string: an empty
    // field means "auto-generate", per SPEC.md's "Title: optional;
    // auto-generated when omitted" — sending `Some("")` would instead ask
    // the supervisor to name the session the empty string.
    let title = (!title.trim().is_empty()).then_some(title);
    let body = serde_json::json!({
        "cwd": cwd,
        "invocation": invocation,
        "title": title,
        "intent_key": intent_key,
    });
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

/// The JavaScript the WEB (wasm) build's random client-side identifiers
/// come from — see `mint_lease` for why the desktop build does not use
/// this at all.
///
/// `crypto.randomUUID()` is the real generator, and the ONLY one this
/// constant offers: it is defined in a SECURE context, which the web build
/// always has (the helm serves it over loopback). wasm has no other way to
/// reach an OS random-number generator without extra plumbing (a JS
/// import, a getrandom shim), and `crypto.randomUUID()` is already right
/// there and Playwright-tested — standing up that plumbing for one UUID
/// was not worth it. Returning `null` rather than substituting something
/// weaker is what lets the caller decide: see `mint_lease` (which must
/// refuse) and `mint_intent_key` (which may degrade).
///
/// wasm-only (the desktop branches never reference it): gated so the
/// desktop build does not carry a dead string constant for a code path it
/// cannot take.
#[cfg(target_arch = "wasm32")]
const CSPRNG_ID_JS: &str =
    "return (globalThis.crypto && crypto.randomUUID) ? crypto.randomUUID() : null;";

/// The intent key's LAST RESORT, used only when `CSPRNG_ID_JS` came back
/// empty-handed — a browser with no `crypto.randomUUID` at all.
///
/// Not cryptographically random and does not need to be for that one
/// caller: an intent key only has to be unique among the creates ONE user
/// is making, so a millisecond timestamp plus two `Math.random()` draws is
/// far past enough, and the alternative (refusing every create on such a
/// browser) is strictly worse for a value that authorizes nothing.
///
/// Deliberately NOT reachable from `mint_lease`. A lease is specified as
/// high-entropy (`ControlMsg::Attach::lease` in farhelm-proto: "version-6
/// clients must mint it high-entropy and non-empty"), and the reason is
/// concrete rather than ceremonial — leases are grouped by BARE EQUALITY,
/// so two clients that collide fuse into one lease and silently bypass the
/// visible takeover SPEC.md's one-attached-client rule is built on.
/// `Math.random()` is not seeded per client and has no such guarantee; two
/// views opened in the same millisecond on the same engine are exactly the
/// case it cannot promise to separate. So the lease fails closed instead
/// (see `SessionView`, which already has that path for the eval channel
/// dying).
#[cfg(target_arch = "wasm32")]
const WEAK_ID_JS: &str = "return Date.now().toString(36) + '-' \
     + Math.random().toString(36).slice(2) \
     + Math.random().toString(36).slice(2);";

/// Run one id-minting snippet through the document eval channel, flattening
/// "the channel failed" and "the snippet declined" into the same `None`.
///
/// The two callers differ only in what they do with that `None`, which is
/// why the distinction is not preserved here: neither can act on WHY it has
/// no id, only on whether it has one.
#[cfg(target_arch = "wasm32")]
async fn eval_minted_id(js: &str) -> Option<String> {
    match dioxus::document::eval(js).await {
        Ok(serde_json::Value::String(id)) if !id.is_empty() => Some(id),
        _ => None,
    }
}

/// Mints this view's attachment lease (PLAN_M4.md item 3) — high-entropy
/// or nothing.
///
/// The two renderers deliberately do NOT share a code path. wasm has no
/// direct line to an OS RNG, so it runs `CSPRNG_ID_JS` through the document
/// eval channel — see that const's docs for why that is an acceptable,
/// tested tradeoff there. The desktop renderer must NOT do the same: manual
/// macOS testing found that wry's eval channel on WKWebView resolves every
/// call to `Err(Finished)` — the channel is dead on arrival, not merely
/// slow — which made `CreateSessionForm`'s fail-closed guard refuse EVERY
/// create (MT-5). Minting the UUID in Rust removes the dependency on that
/// channel entirely rather than working around one platform's flaky eval;
/// desktop already links a real RNG (`uuid`'s `v4` feature, `getrandom`
/// underneath), so there was never a reason to route this through the
/// webview to begin with. That precedent is why the lease is minted here at
/// all rather than in terminal.js, where the desktop build would have hit
/// exactly the same dead channel.
///
/// `Err` carries a message suitable for direct display. Both of its causes
/// — a dead eval channel and a browser without `crypto.randomUUID` — are
/// reported the same way, because the caller's response is the same either
/// way: attach nothing, and say so.
async fn mint_lease() -> Result<String, String> {
    #[cfg(target_arch = "wasm32")]
    {
        eval_minted_id(CSPRNG_ID_JS).await.ok_or_else(|| {
            "this browser did not provide crypto.randomUUID (or the eval channel failed), and a \
             session lease must be high-entropy — a guessable or colliding one would silently \
             merge two clients into one attachment"
                .to_string()
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(uuid::Uuid::new_v4().to_string())
    }
}

/// Mints one intended create's idempotency key (PLAN_M3.md item 6, MT-5).
///
/// Same two-renderer split as `mint_lease` (see its docs for the wry
/// eval-channel history), with one deliberate difference: this one accepts
/// the `WEAK_ID_JS` fallback when no CSPRNG is available, because an intent
/// key only has to be unique among one user's own creates. See that
/// constant's docs for why the lease may not make the same trade.
///
/// Returns `Err` only when BOTH snippets fail — in practice, a dead eval
/// channel — with a message suitable for direct display.
async fn mint_intent_key() -> Result<String, String> {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(id) = eval_minted_id(CSPRNG_ID_JS).await {
            return Ok(id);
        }
        eval_minted_id(WEAK_ID_JS)
            .await
            .ok_or_else(|| "the browser eval channel produced no value".to_string())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(uuid::Uuid::new_v4().to_string())
    }
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
    let url = format!("{base}/api/sessions/{}/stop", encode_path_segment(id));
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

/// Fetch ONE session's current state (`GET /api/sessions/{id}`).
///
/// Used by the session view rather than the full listing, because a
/// listing REPLY is a whole page of sessions this view has no use for.
///
/// ## What `Ok(None)` does and does not mean
///
/// It means the helm answered 404 for this id. It does NOT mean the
/// session is gone, and callers must not treat it that way — an earlier
/// version of this doc claimed exactly that, and it was wrong about the
/// server it describes. The helm's detail route (`get_session` in
/// farhelm-helm) is not a per-session query at all: it fetches the
/// LISTING and searches it, so it inherits the supervisor's listing cap.
/// On a host with more sessions than that cap, a perfectly healthy session
/// beyond it answers 404 here every time, and a session near the boundary
/// can move in and out of the reply as other sessions come and go.
///
/// So a 404 is genuinely ambiguous between "deleted" and "not in this
/// page", and the honest client behavior is to keep the last known state
/// rather than either fabricate a deletion or silently pretend the refresh
/// worked — see `SessionView`, which keeps what it has and says the
/// refresh is not landing.
///
/// A transport failure stays an `Err`, because "the helm did not answer"
/// and "the helm answered 404" must not be confused either.
async fn fetch_session(base: &str, id: &str) -> Result<Option<Session>, String> {
    let url = format!("{base}/api/sessions/{}", encode_path_segment(id));
    let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
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
    resp.json::<Session>()
        .await
        .map(Some)
        .map_err(|e| e.to_string())
}

/// POST the restart endpoint for one session, returning the session's
/// freshly recomputed state (SPEC.md's restart; PLAN_M3.md item 9).
///
/// `mode` is not this caller's choice to make freely — it is whatever the
/// session's CURRENT offer authorizes (`restart_mode_for`). The supervisor
/// re-derives that offer at handling time and refuses a mismatch with a
/// 409, which is the staleness case the caller handles by refreshing the
/// session rather than retrying (see `SessionView`).
///
/// `stop_if_running` carries the user's explicit consent to stop a live
/// agent first; the caller only sets it after the inline confirmation, and
/// the supervisor rechecks real liveness before honoring it. Same
/// error-surfacing shape as `stop_session` above, including the
/// body-read-failure context.
async fn restart_session(
    base: &str,
    id: &str,
    mode: &str,
    stop_if_running: bool,
) -> Result<Session, String> {
    let url = format!("{base}/api/sessions/{}/restart", encode_path_segment(id));
    let body = serde_json::json!({ "mode": mode, "stop_if_running": stop_if_running });
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

/// DELETE a session. See `stop_session`'s docs — same error-surfacing
/// shape (including the body-read-failure context), different verb and
/// endpoint.
async fn delete_session(base: &str, id: &str) -> Result<(), String> {
    let url = format!("{base}/api/sessions/{}", encode_path_segment(id));
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

/// The success body of `POST /api/sessions/{id}/tabs`: the newly opened
/// tab, wrapped (farhelm-helm's `open_tab` returns `{"tab": TabInfo}`
/// rather than a bare object, because a client needs the minted id before
/// it can attach). Private and `Deserialize`-only — nothing keeps one of
/// these around past unwrapping it.
#[derive(Deserialize)]
struct TabOpened {
    tab: Tab,
}

/// POST the tab-open endpoint for one session (PLAN_M4.md item 6), giving
/// back the tab the supervisor minted.
///
/// No request body: unlike a create, a tab has nothing for a caller to
/// specify — it is always a plain shell in the session's own working
/// directory. Failures reach the user as the supervisor's own words, the
/// same as every other endpoint here: SPEC.md wants the vanished-working-
/// directory and restart-first refusals to say exactly what is wrong, and
/// they only do that if this returns the body verbatim rather than a
/// generic "could not open a tab".
///
/// Deliberately NOT idempotency-keyed, unlike `create_session`. The key
/// there exists because a lost reply could otherwise cost the user a
/// second AGENT — a real process doing real work under a duplicate
/// session, invisible until it collides with the first.
///
/// A duplicated tab is a smaller problem, though not a free one: it is a
/// real login shell, and a login shell runs the user's rc files, which can
/// start anything. What makes it acceptable is that it is VISIBLE and
/// individually reversible — the next detail poll lists both tabs, and
/// closing one is a click that reaps that shell's whole process tree. The
/// duplicate-agent case has neither property, which is the actual
/// distinction. This UI also cannot lose a reply and retry silently: the
/// add control is single-shot per click (`opening_tab`), so a duplicate
/// takes a second deliberate press.
async fn open_tab(base: &str, session_id: &str) -> Result<Tab, String> {
    let url = format!(
        "{base}/api/sessions/{}/tabs",
        encode_path_segment(session_id)
    );
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
    resp.json::<TabOpened>()
        .await
        .map(|opened| opened.tab)
        .map_err(|e| e.to_string())
}

/// DELETE one terminal tab: kill its shell and everything it left behind
/// (SPEC.md's per-tab close, the whole per-tab operation set in v1). Same
/// error-surfacing shape as `delete_session`.
///
/// A 404 is NOT special-cased into success here, even though "the tab is
/// already gone" is the outcome the caller wanted: the caller acts on the
/// answer by tearing down that tab's island, and a 404 is at least as
/// likely to mean the id was wrong (a bug) as to mean another client got
/// there first. Reporting it lets the user see something disagreed, while
/// the poll that follows reconciles the list either way.
async fn close_tab(base: &str, session_id: &str, tab_id: &str) -> Result<(), String> {
    let url = format!(
        "{base}/api/sessions/{}/tabs/{}",
        encode_path_segment(session_id),
        encode_path_segment(tab_id)
    );
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
            // `Interrupted` joins `Exited` here for a stronger version of
            // the same argument: a host reboot is what produced this
            // status, and a reboot leaves no descendants at all — there
            // is not even the stray-MCP-server residual to accept. The
            // session's agent is definitively not running, so confirming
            // would be asking about a danger that cannot exist.
            // `Error` joins them for the strongest version yet: the login
            // shell and the launch shim DID run briefly (the shim is what
            // WRITES this very sentinel, from inside a real process), but
            // the AGENT'S OWN exec is what failed (PLAN_M3.md item 3) —
            // before it, before anything the agent itself might have
            // spawned. There is no lingering process tree to worry about,
            // not because nothing ever ran, but because the one thing
            // that could have left descendants never got the chance to.
            SessionStatus::Exited { .. }
            | SessionStatus::Interrupted
            | SessionStatus::Error { .. } => do_delete_on_confirm(target.id),
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
///
/// ## The intent key (PLAN_M3.md item 6)
///
/// One key per INTENDED create, reused across every retry of it. The
/// lifecycle is deliberately tied to the form's values rather than to its
/// mount: minted at first submit, kept across a failed submit (the retry
/// case the key exists for), and dropped the moment any field changes
/// (which makes the next submit a different intent). Both edges matter —
/// keeping it across an edit would send a request the server refuses as a
/// key reuse once the first attempt has a durable outcome, and dropping it
/// on failure would make a retry able to create a second session for the
/// same intent, which is the exact gap this closes.
///
/// The inputs are DISABLED while a create is in flight, which is what makes
/// that lifecycle a rule rather than a race: key generation is itself
/// asynchronous (`mint_intent_key` is an `await` on both renderers, even
/// though only the wasm build's half of it actually yields), so without it
/// a keystroke could land between minting a key and sending it, publishing
/// a key that belongs to values the user has already changed. Disabling was
/// chosen over reconciling generations afterwards because the form is inert
/// for that window anyway — the submit button and both navigation controls
/// are already disabled by the same flag.
///
/// A create from this form ALWAYS carries a key. If the key cannot be
/// generated the create is refused locally, with the failure shown like any
/// other: falling back to an unkeyed create would silently drop the one
/// protection this whole feature exists to provide, at exactly the moment
/// something is already wrong with the environment, and a user who retries
/// after a dropped reply would get a duplicate agent with nothing to
/// indicate why.
///
/// The server's key-reuse and already-deleted refusals need no handling of
/// their own here: they arrive as ordinary create failures (a 409 with the
/// supervisor's own message) and render in the same `.create-session-error`
/// line as every other one, which is what SPEC.md's "concrete, actionable
/// errors" asks for — the message names the key and what happened to it.
#[component]
fn CreateSessionForm(mut submitting: Signal<bool>, on_created: EventHandler<Session>) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut cwd = use_signal(String::new);
    let mut invocation = use_signal(String::new);
    let mut title = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    // This form's current intended create, if one has been submitted yet
    // (PLAN_M3.md item 6). Minted at FIRST SUBMIT and reused by every later
    // submit of the same values; cleared inline by every field's `oninput`,
    // because an edit makes the next submit a different intent. See this
    // component's own docs for both edges of that rule.
    let mut intent_key = use_signal(|| None::<String>);

    rsx! {
        form {
            class: "create-session-form",
            onsubmit: move |evt| {
                evt.prevent_default();
                // Double-submission guard: covers concurrent clicks on
                // THIS mounted form (a double-click, or a stray repeat
                // event) — the control is inert for the whole round trip,
                // not just until the first click handler returns.
                //
                // The OTHER half — a retry after an ambiguous transport
                // failure (request sent, response lost) reaching the
                // supervisor a second time — is what `intent_key` closes,
                // and it cannot be closed here: only the server knows
                // whether the lost reply belonged to a session that
                // actually exists. This handler's job is merely to send
                // the SAME key for every retry of one intent.
                if submitting() {
                    return;
                }
                let base = base.clone();
                let cwd_value = cwd();
                let invocation_value = invocation();
                let title_value = title();
                // Snapshotted before disabling the form, not re-read
                // inside the task: either way is race-free (no edit can
                // land once `submitting` is true, below), but reading it
                // here keeps the "already have one" retry path free of an
                // await instead of calling `mint_intent_key` unconditionally.
                let needs_key = intent_key().is_none();
                submitting.set(true);
                error.set(None);
                spawn(async move {
                    if needs_key {
                        match mint_intent_key().await {
                            Ok(key) => intent_key.set(Some(key)),
                            Err(reason) => {
                                // No key, no create: see this component's
                                // docs on why an unkeyed create is not an
                                // acceptable degradation. The message says
                                // what failed rather than blaming the
                                // request, since nothing the user typed
                                // caused it.
                                error.set(Some(format!(
                                    "could not generate an idempotency key for this create, so \
                                     it was not sent (a retry could otherwise create a second \
                                     session): {reason}"
                                )));
                                submitting.set(false);
                                return;
                            }
                        }
                    }
                    let key = intent_key().expect("a key was just generated or already held");
                    match create_session(
                            &base,
                            &cwd_value,
                            &invocation_value,
                            &title_value,
                            &key,
                        )
                        .await
                    {
                        Ok(session) => on_created.call(session),
                        Err(e) => {
                            // The key deliberately SURVIVES a failure:
                            // this is exactly the case it exists for. A
                            // failure whose cause was an ambiguous
                            // transport error may have created a session
                            // the user cannot see, and resubmitting
                            // unchanged must reach that same session
                            // rather than launch a second agent. A user
                            // who instead fixes the form gets a new key
                            // from the fields' own `oninput`.
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
                    disabled: submitting(),
                    oninput: move |evt| {
                        cwd.set(evt.value());
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
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
                    disabled: submitting(),
                    oninput: move |evt| {
                        invocation.set(evt.value());
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
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
                    disabled: submitting(),
                    oninput: move |evt| {
                        title.set(evt.value());
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
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
/// order walks confirm delete → cancel; initial FOCUS lands directly on
/// cancel regardless of tab order (see below).
///
/// The `.session-row-open` button (title/cwd/invocation/badge) is given
/// the extra `confirming` class and hidden outright (`display: none` in
/// app.css) rather than merely staying `disabled`, which is what it did
/// before this fix (MT-8): `.session-row-main` lays out its children in
/// one non-wrapping flex row, and that button's own children each carry a
/// `min-width` floor (see `.session-title`/`.session-cwd`/
/// `.session-invocation`) that does not shrink to nothing just because
/// the OUTER flex algorithm hands the button a narrower slot to make room
/// for the confirm prompt's own elements. Past that floor the button's
/// content overflows its shrunk box — CSS flexbox does not clip
/// overflow by default — and renders on top of the confirm prompt sitting
/// immediately after it in the row, rather than being replaced by it.
/// Removing the button from layout entirely while confirming is open
/// sidesteps that interaction completely instead of trying to out-shrink
/// it: the confirm prompt already repeats the title (`.confirm-title`
/// below), so nothing the hidden button showed is lost information while
/// it is gone.
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
    let (badge_class, badge_text) = status_badge(&session.status, session.annotation.as_deref());
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
                    // The `confirming` modifier is what app.css's
                    // `.session-row-open.confirming` hides (MT-8, see the
                    // "Inline delete confirmation" section of this
                    // component's doc above) — without it, this button's
                    // own title/cwd/invocation content overflows its
                    // flex-shrunk box and paints over the confirm prompt
                    // rendered right after it.
                    class: if confirming { "session-row-open confirming" } else { "session-row-open" },
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

/// Map a status — and, for an ended session, its annotation — to the
/// badge's CSS modifier class and display text. Kept as one function so
/// every case stays next to its siblings instead of drifting apart across
/// separate match arms in the render tree.
///
/// The annotation is a QUALIFIER on the exited status, never a
/// replacement for it: SPEC.md is explicit that "stopped" is not a
/// distinct status, so a user-stopped session reads "exited — stopped by
/// user (code 0)". An earlier version rendered the annotation alone, which
/// read as a fourth status word and quietly dropped the one fact every
/// row's badge is supposed to state. The annotation is ignored for every
/// other status — it describes how a run ENDED, and a live session has
/// not.
fn status_badge(status: &SessionStatus, annotation: Option<&str>) -> (&'static str, String) {
    match status {
        SessionStatus::Alive => ("alive", "alive".to_string()),
        SessionStatus::Exited { exit_code } => {
            let mut text = "exited".to_string();
            if let Some(annotation) = annotation {
                text.push_str(" — ");
                text.push_str(annotation);
            }
            if let Some(code) = exit_code {
                text.push_str(&format!(" (code {code})"));
            }
            ("exited", text)
        }
        SessionStatus::Interrupted => ("interrupted", "interrupted".to_string()),
        // The shim's exec-failure sentinel (PLAN_M3.md item 3): the agent
        // never ran at all, which is a different claim from `Exited`'s
        // "it ran and finished" — so it gets its own word and its own
        // red-family color (`app.css`'s `.status-badge.error`), the one
        // case in this match that IS reporting a failure. `detail` (the
        // shim's own errno/argv0 report) rides straight into the badge
        // text rather than being tucked behind a tooltip or a separate
        // element: it is usually short, and it is the one piece of
        // information that actually explains why the row needs attention.
        SessionStatus::Error { detail } => ("error", format!("error — {detail}")),
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
/// Only ever OPENED from `Alive`/`Unknown` (see `on_delete`'s own match) —
/// but is written total over `SessionStatus` rather than partial, because
/// `confirming` is `ListView`'s own state, decoupled from any single
/// render: a session that was `Alive` when the user opened this prompt
/// can flip to `Exited` under it (stopped from another client, say)
/// before either button is clicked, and this function re-runs on every
/// render off whatever status the row's LATEST prop carries. The
/// `Exited`, `Interrupted`, AND `Error` arms are all that residual case's
/// fallback, not wordings SPEC.md's confirm-contract actually specifies —
/// and `Error` is not merely a defensive completeness case: a session
/// that was genuinely `Alive` when this prompt opened, whose agent then
/// turns out never to have execed at all (the launch shim's sentinel is
/// read only once the pane goes dead-or-absent — `service.rs`'s
/// dead-or-absent gate), can flip straight from `Alive` to `Error` under
/// an already-open prompt exactly like the `Exited` case above, just with
/// a narrower window.
///
/// `Interrupted`'s wording is deliberately NOT a killing warning
/// (PLAN_M3.md item 2): the status exists only because the HOST rebooted,
/// which took the agent and every descendant of it with it, so there is
/// nothing left for a delete to kill and claiming otherwise would be the
/// mirror image of the fabricated-liveness mistake `Unknown`'s wording
/// exists to avoid. What deleting actually costs is the session itself —
/// worth saying, because an interrupted session is the one case where the
/// record outlives everything it described and is all that is left to
/// lose (and, since restart landed, the only route back into that
/// conversation).
fn confirm_consequence(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Alive => "still running — deleting kills the agent:",
        SessionStatus::Unknown => {
            "status unknown — the agent may still be running and will be killed:"
        }
        SessionStatus::Exited { .. } => "delete anyway:",
        SessionStatus::Interrupted => {
            "interrupted by a host reboot — nothing left to kill; deleting discards the session:"
        }
        // `Error` never OPENS this prompt (see `on_delete`'s own match),
        // but a prompt already open for an `Alive` session CAN land here —
        // see this function's own docs — so this arm is reachable, not
        // merely a defensive completeness case.
        SessionStatus::Error { .. } => {
            "the agent never started — nothing to kill; deleting discards the session:"
        }
    }
}

// ---------------------------------------------------------------------
// Terminal tabs (PLAN_M4.md item 6)
//
// The session view holds one xterm.js island per terminal — the agent's
// plus one per open tab — all attached CONCURRENTLY. Everything below is
// the pure part of that: which tabs to show, what to call them, which DOM
// nodes they own, and how their WebSocket URLs are built. The stateful
// half lives in `SessionView`, and the mount/unmount half lives in
// terminal.js; keeping the derivations here as free functions is what
// makes them testable without a renderer.
// ---------------------------------------------------------------------

/// The DOM id of the agent terminal's mount point, and of its banner.
///
/// Unchanged from before tabs existed, deliberately: "the terminal" of a
/// session still unambiguously means its agent terminal, the browser suite
/// keys off these two ids throughout, and a session with no tabs must
/// render exactly the DOM it always did.
const AGENT_TERMINAL_ELEMENT_ID: &str = "terminal";
const AGENT_BANNER_ELEMENT_ID: &str = "term-banner";

/// How many TAB terminals this view will ever mount at once. The agent
/// terminal is always mounted and is not counted against it.
///
/// PLAN_M4.md item 6 deliberately adds no artificial cap on TABS, and this
/// is not one: it is a bound on what this client will do with a LIST it
/// did not author. The plan's reasoning — "tab count is bounded by the
/// user opening them by hand" — is a statement about a healthy supervisor,
/// and the tab list arrives over a connection that under `--ssh` reaches a
/// different machine. A supervisor that is compromised, or merely wrong
/// (a rediscovery bug adopting every window on a shared tmux server),
/// could list thousands, and this view would answer by constructing
/// thousands of xterm.js instances and opening thousands of WebSockets —
/// wedging the browser, and with it the user's only way to see what
/// happened. A trust boundary needs a bound even where the cooperative
/// case does not.
///
/// 32 is chosen to be far past hand-driven use (SPEC.md's tabs are for
/// "poking at the workspace next to the agent") while still being a
/// resource count a browser will not notice. Tabs beyond it are NOT hidden
/// — the strip lists every one the server reports, and their panes say
/// they are not attached — because silently dropping tabs would be its own
/// lie about what the session holds.
const MAX_MOUNTED_TAB_ISLANDS: usize = 32;

/// The two properties the number above has to keep, checked at COMPILE
/// time rather than by a test: both are statements about a constant, so a
/// violation should never get as far as being runnable. Below the floor it
/// stops being a trust boundary and becomes a feature limit a real user
/// could hit by hand; above the ceiling it stops bounding what a hostile
/// or buggy tab list can make this browser construct.
const _: () = assert!(MAX_MOUNTED_TAB_ISLANDS >= 16);
const _: () = assert!(MAX_MOUNTED_TAB_ISLANDS <= 256);

/// `tab_errors`' key for the add-tab control, which — unlike a close — has
/// no tab id of its own to be keyed by yet. A fixed non-id string rather
/// than an `Option` key so the map stays one flat, renderable structure;
/// it cannot collide with a real tab id, which is a supervisor-minted
/// opaque token and never this word.
const TAB_OPEN_ERROR_KEY: &str = "open";

/// The DOM id of a tab's mount point, and of its banner. Derived from the
/// tab id rather than from its position so that a tab's island keeps its
/// identity when a SIBLING is closed — a position-derived id would make
/// every tab after the closed one look like a different island to
/// terminal.js's reconciliation and force a pointless remount (and, with
/// it, a full replay) of terminals nothing happened to.
///
/// The tab id is supervisor-minted and opaque; it reaches the DOM only
/// through `getElementById`, which imposes no syntax on an id, so nothing
/// here has to assume it is a UUID even though today it always is.
fn tab_terminal_element_id(tab_id: &str) -> String {
    format!("terminal-{tab_id}")
}

/// The banner half of the pair above; see `tab_terminal_element_id` for
/// why both are derived from the id rather than the position.
fn tab_banner_element_id(tab_id: &str) -> String {
    format!("term-banner-{tab_id}")
}

/// A tab's display label: purely positional, one-based (PLAN_M4.md item
/// 6). SPEC.md gives tabs no names and close is their only operation, so
/// v1 invents no naming surface — and because the position comes from
/// `SessionInfo::tabs`' creation order, two clients looking at the same
/// session agree on which tab is "Terminal 2".
fn tab_label(index: usize) -> String {
    format!("Terminal {}", index + 1)
}

/// The tabs to render, in order: the server's authoritative list, then any
/// this view opened that the server has not listed back yet, minus any it
/// has closed that the server still lists.
///
/// Both corrections are the same optimistic-rendering bargain `ListView`
/// makes for a deleted row, for the same reason — a tab list refreshed by
/// a 3-second poll would otherwise take up to a full interval to show the
/// user the result of their own click — but they point in opposite
/// directions and both edges matter:
///
/// - `opened` appends. A tab-open reply carries the new tab's id and
///   deliberately says NOTHING about ordering (farhelm-proto's
///   `TabOpened`), so an optimistic tab goes at the END and gets its real
///   position from the next refresh. That is honest for the common case
///   (the newest tab is last in creation order) and self-correcting when
///   another client's concurrent open makes it wrong.
/// - `closed` suppresses. A DELETE that has already succeeded must not
///   leave a tab on screen — clicking it would attach a terminal the
///   supervisor has destroyed — until a poll that was already in flight
///   before the close finally returns.
///
/// Neither set is trusted to be accurate forever: `SessionView`'s poll
/// prunes an `opened` id once the server lists it and a `closed` id once
/// the server stops, so a tab another client re-opens (impossible today —
/// ids are never reused — or a supervisor that answered oddly) cannot be
/// permanently hidden by a stale entry.
/// `opened` carries `(id, observed_from)` pairs, but only the ids matter
/// here — the sequence numbers are the POLL's business (see `SessionView`),
/// deciding when an entry may be retired, never whether it is shown.
fn visible_tabs(server: &[Tab], opened: &[(String, u64)], closed: &HashSet<String>) -> Vec<String> {
    let mut ids: Vec<String> = server.iter().map(|tab| tab.id.clone()).collect();
    for (id, _) in opened {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    ids.retain(|id| !closed.contains(id));
    ids
}

/// The helm WebSocket path one terminal of a session attaches on.
///
/// `lease` rides on EVERY terminal of the view, the agent's included
/// (PLAN_M4.md item 3): the supervisor groups a session's channels by it
/// to enforce SPEC.md's one-attached-client rule across all of them, so a
/// view that leased only its tabs would have its own agent terminal
/// take over — and be taken over by — the tabs sitting beside it.
///
/// `tab` is absent for the agent terminal, which is what the pre-M4
/// (agent-only) reading of this endpoint already means, so the agent's URL
/// differs from an old build's only by the lease.
fn terminal_ws_path(session_id: &str, tab_id: Option<&str>, lease: &str) -> String {
    let session = encode_path_segment(session_id);
    let lease = encode_query_value(lease);
    match tab_id {
        None => format!("/api/sessions/{session}/term?lease={lease}"),
        Some(tab) => {
            let tab = encode_query_value(tab);
            format!("/api/sessions/{session}/term?tab={tab}&lease={lease}")
        }
    }
}

/// The safety-critical half of the inline close-tab confirmation, the
/// counterpart to `confirm_consequence` for sessions and worded from the
/// same rule: say what the click actually destroys before naming the
/// thing.
///
/// Unconditional, with no status to branch on — a tab has none. That is
/// not an omission: SPEC.md makes close "kills that shell and its
/// processes" whatever the shell is currently doing, and a tab whose shell
/// already exited still closes (the supervisor's own idempotency), so
/// there is no state in which a softer wording would be more honest. The
/// daemonized-child clause is in it because the supervisor's tab-scoped
/// reap really does go after them, which is exactly the consequence a user
/// cannot see coming from the word "close".
const CLOSE_TAB_CONSEQUENCE: &str =
    "closing kills this terminal's shell and every process it started:";

/// `tab_errors`' entries in a stable display order, newest-agnostic and
/// purely lexical by key.
///
/// A `HashMap` iterates in no defined order, and the error lines sit in a
/// list the user reads: without this, every unrelated re-render could
/// reshuffle them, which reads as messages appearing and disappearing
/// rather than accumulating. The ORDER itself carries no meaning and is not
/// claimed to — only its stability does.
fn sorted_tab_errors(errors: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = errors
        .iter()
        .map(|(key, message)| (key.clone(), message.clone()))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

/// One session: a tab strip over one terminal per open terminal, with a
/// back control above them. Each terminal div is handed to the JS island
/// on mount; Dioxus never touches its children again — that boundary is
/// the whole design.
///
/// ## Tabs (PLAN_M4.md item 6)
///
/// The agent terminal comes first and cannot be closed; every open tab
/// follows it, labeled positionally. Every one of them is ATTACHED at the
/// same time, and selecting a tab is a CSS visibility change, never an
/// attach cutover — attach-on-select was considered and rejected, since
/// each switch would pay a full replay and exercise the takeover machinery
/// for no gain. What that costs is one xterm instance, one WebSocket, and
/// one supervisor-side control client per tab; what it buys is that a
/// background tab keeps consuming its own stream, which per-terminal flow
/// control (PLAN_M4.md item 3) makes safe.
///
/// Hidden panes are hidden with `visibility`, not `display: none`, and
/// that is load-bearing rather than stylistic: a `display: none` element
/// has no layout box, so `FitAddon.fit()` would size every unselected
/// terminal to zero columns at mount time — and every tab in a session
/// with more than one mounts while hidden. See app.css's `.terminal-pane`.
///
/// A tab whose WINDOW is gone (closed from another client, erased by a
/// reboot) needs no special case here: its attach fails and the helm
/// relays the reason as an ordinary detach notice, which is the same
/// no-terminal explanation the session view has always rendered. A tab
/// whose SHELL merely exited is a live, viewable dead pane, exactly as the
/// agent terminal is.
///
/// ## The attachment lease (PLAN_M4.md item 3)
///
/// One high-entropy lease per VIEW INSTANCE, minted here (see
/// `mint_lease`) and carried on every terminal WebSocket this view
/// opens, agent included. The supervisor groups a session's channels by it
/// to enforce SPEC.md's one-attached-client rule across all of a session's
/// terminals at once.
///
/// This deliberately sharpens what a takeover means: a second view of the
/// same session now detaches ALL of the first view's terminals, not just
/// the one they have in common. That is the rule working as specified —
/// the attached client owns the session's terminals — and each losing
/// terminal still banners its own detach.
///
/// No terminal mounts until the lease exists, and a lease that cannot be
/// minted is fatal to this view's terminals rather than something to
/// degrade around. That is not caution copied from `CreateSessionForm`: an
/// un-leased attach is read as its own one-terminal client, so N un-leased
/// attachments from ONE view would take each other over in turn and the
/// session would end up with exactly one live terminal and no explanation.
/// Refusing loudly beats shipping that.
///
/// ## Mount/unmount lifecycle (PLAN_M2.md step 7)
///
/// M1 never unmounted this component (it was the only view), so
/// `terminal.js` only ever needed a mount-time guard against a
/// re-render calling `mount()` twice. Now that `App` can navigate away
/// and back, this component must clean up on drop — otherwise that
/// guard (terminal.js's `islands` map, now the ONLY mount guard — see its
/// own docs) would permanently wedge shut, and reopening ANY session
/// after the first would silently no-op `mount()` instead of attaching.
/// `use_drop` fires `farhelmTerm.unmountAll()` for exactly that reason: it
/// is the regression this lifecycle work exists to prevent, not a
/// hypothetical.
///
/// Individual mounts are NOT driven from here. This component computes the
/// full set of terminals it wants and hands it to `farhelmTerm.sync()`,
/// which reconciles (see terminal.js's header for why the diff belongs on
/// that side). The effect below is therefore idempotent by construction,
/// which is what makes it safe for a 3-second poll to re-run it forever.
///
/// ## The sync-generation token
///
/// `window.__farhelmSyncGeneration` guards only the OUTER wait here —
/// for `window.farhelmTerm` to exist at all, since terminal.js's
/// `document::Script` is injected asynchronously and this component can
/// render before it has executed (a real, if rare, race). Bumping the
/// counter on every sync attempt AND on drop means backing out before
/// that wait resolves reliably cancels it. Once `window.farhelmTerm`
/// exists, `mountWhenReady`'s OWN per-island wait for xterm's globals is a
/// separate concern guarded entirely inside terminal.js (its `pendings`
/// replacement-and-clear scheme — see that function's docs); this token
/// does not reach that far.
#[component]
fn SessionView(session: Session, on_back: EventHandler<()>) -> Element {
    let base = use_context::<ApiBase>().0;
    // The session as this view currently understands it. Seeded from the
    // prop and then owned here, because a restart changes it: the reply
    // carries the session's recomputed status and offer, and a REFUSED
    // restart is exactly the case where re-reading the server's current
    // answer matters (a stale offer is what caused the refusal).
    let mut current = use_signal(|| session.clone());
    // Bumped on every successful restart to force the AGENT terminal's
    // island to remount — and only that one, since restart touches the
    // agent terminal alone (the supervisor's `detach_for_restart`) and a
    // tab's attachment survives it untouched. Both reuse cases need it: a
    // reused pane's attachment was deliberately torn down by the restart,
    // and a fresh terminal is a different pane entirely. Remounting also
    // replays the pane's scrollback, which for a reused terminal is what
    // puts the PRIOR run's output back on screen above the new one.
    let mut mount_generation = use_signal(|| 0_u32);
    // Whether a restart is in flight (disables the control and guards
    // re-entry, mirroring `ListView`'s `pending`), and whether the inline
    // confirm prompt is open for a still-running agent (the same in-page
    // pattern delete uses — never a browser dialog, which wry's macOS
    // webview does not have at all).
    let mut restarting = use_signal(|| false);
    let mut confirming = use_signal(|| false);
    let mut restart_error = use_signal(|| None::<String>);
    // Counts restart ATTEMPTS, and versions every read of this session
    // against them. The detail poll below is a round trip that can outlive
    // a restart the user starts while one of its fetches is in flight;
    // without this, that stale answer would land on top of the restart's
    // own fresher one and the view would go back to describing the run
    // that no longer exists.
    let mut restart_epoch = use_signal(|| 0_u32);

    // This view's attachment lease, `None` until minting finishes (it is
    // an `await` on both renderers) and `lease_error` instead if it never
    // does. See this component's own docs for why nothing attaches without
    // it rather than falling back to an un-leased attach.
    let mut lease = use_signal(|| None::<String>);
    let mut lease_error = use_signal(|| None::<String>);
    // Which terminal is on screen: `None` is the agent terminal, `Some(id)`
    // a tab. Only the SELECTION changes here — every terminal stays
    // attached regardless of what this holds.
    let mut selected = use_signal(|| None::<String>);
    // The optimistic corrections `visible_tabs` applies over the server's
    // list, so the user sees their own open/close before the next poll
    // confirms it. Pruned by that poll (below) once the server agrees.
    //
    // `opened_tabs` carries a SEQUENCE NUMBER per entry, not just an id,
    // and that is what keeps a phantom tab from living forever. A reply
    // that omits an optimistic tab is only evidence of absence if the
    // request behind it was sent AFTER the open completed; a poll already
    // in flight when the tab was created legitimately predates it. Without
    // the number, "absent" could never be distinguished from "too early",
    // so an entry the server would never list — one another client closed
    // before this view ever saw it listed — had no way to be retired, and
    // the strip would show a tab that attaches to nothing until the view
    // was closed. `poll_sequence` below is the counter both sides read.
    let mut opened_tabs = use_signal(Vec::<(String, u64)>::new);
    let mut closed_tabs = use_signal(HashSet::<String>::new);
    // How many detail polls have been STARTED by this view. Incremented at
    // each fetch's launch, so a poll's own index is the value it read
    // before incrementing, and an optimistic open recording the current
    // value names the first poll that could possibly know about it.
    let mut poll_sequence = use_signal(|| 0_u64);
    // In-flight guards, mirroring `ListView`'s `pending`: one flag for the
    // single add control, a set for closes (which are per tab and can
    // legitimately overlap).
    let mut opening_tab = use_signal(|| false);
    let mut closing_tabs = use_signal(HashSet::<String>::new);
    // Which tab, if any, is showing the inline close confirmation.
    let mut confirming_close = use_signal(|| None::<String>);
    // Set when the detail poll gets a 404 for this session: what is on
    // screen is the last state that DID arrive, and this says so. See
    // `fetch_session` for why a 404 cannot be read as "deleted" — the
    // helm's detail route is listing-backed and inherits its cap — and
    // therefore why this is a staleness notice rather than an obituary.
    let mut refresh_stale = use_signal(|| false);
    // Tab-operation failures, keyed by OPERATION rather than pooled into
    // one slot — the same discipline `ListView` keeps for its per-session
    // errors, and for the same reason: with a single slot, closing tab B
    // successfully would wipe the still-unread failure from closing tab A,
    // and two concurrent failures would clobber each other so the user
    // only ever learned about whichever lost the race. The key is
    // `TAB_OPEN_ERROR_KEY` for the add control and the tab's own id for a
    // close, so an operation only ever clears its own message.
    let mut tab_errors = use_signal(HashMap::<String, String>::new);

    // Minted once, on mount, and never re-minted: the lease identifies
    // THIS view instance for as long as it lives, so a remount of one
    // terminal (a restart) must keep it — reusing the lease is exactly
    // what makes that remount an ordinary reconnect of one channel rather
    // than a takeover of the view's own sibling terminals.
    use_future(move || async move {
        match mint_lease().await {
            Ok(minted) => lease.set(Some(minted)),
            Err(reason) => lease_error.set(Some(format!(
                "could not generate this view's terminal lease, so no terminal was attached \
                 (without one, this session's terminals would take each other over): {reason}"
            ))),
        }
    });

    // Poll the session DETAIL for as long as this view is open, at the
    // same cadence `ListView` polls the listing (PLAN_M4.md item 6).
    //
    // Through M3 this was ONE refresh on open, and its doc said polling
    // was deliberately not wanted: the `Session` this view is handed can
    // be a create-time placeholder (`SessionCreated` reports `Unknown`
    // deliberately — see the supervisor's create docs) and the restart
    // affordance reads `status`, but that status is only ever a UI HINT —
    // the supervisor rechecks real liveness and refuses a restart that
    // would kill an agent without consent — so one fetch was enough. Tabs
    // change that, and only that: the tab list has no server-side recheck
    // standing behind it, and SPEC.md's changes-appear-automatically rule
    // means a tab opened or closed from another client has to show up
    // without a reload. Polling the detail is the interim mechanism for
    // exactly that reason M2 chose it for the list; M5's live push
    // replaces both together. The status refresh comes along for free and
    // is strictly better than the single shot it replaces.
    //
    // Scoped to a `use_future` owned by this component, so it stops for
    // free when the view unmounts — the same lifecycle `ListView`'s poll
    // relies on.
    let refresh_base = base.clone();
    let refresh_id = session.id.clone();
    use_future(move || {
        let base = refresh_base.clone();
        let id = refresh_id.clone();
        async move {
            loop {
                // Both counters are read per iteration, not once outside
                // the loop. `started_at` versions THIS fetch against
                // restarts, so a restart landing mid-flight invalidates
                // only the request that was in the air when it happened;
                // `index` is this poll's position in the view's own poll
                // order, which is what tells an optimistic tab whether
                // this reply is late enough to be evidence about it.
                let started_at = restart_epoch.peek().to_owned();
                let index = poll_sequence.peek().to_owned();
                poll_sequence += 1;
                let fetched = fetch_session(&base, &id).await;
                // A 404 is surfaced rather than absorbed, and deliberately
                // NOT acted on: the view keeps everything it has —
                // metadata, tabs, live terminals — and merely stops
                // claiming to be current. `fetch_session`'s docs carry the
                // reason this is not "the session was deleted"; a
                // transport error is left silent because a poll that
                // failed to reach the helm says nothing about the session
                // at all, and one dropped request every few seconds is not
                // worth a banner.
                if matches!(fetched, Ok(None)) {
                    refresh_stale.set(true);
                }
                if let Ok(Some(fresh)) = fetched
                    && *restart_epoch.peek() == started_at
                {
                    refresh_stale.set(false);
                    // Prune the optimistic corrections this reply settles.
                    // Deliberately NOT done on a failed or 404 fetch — an
                    // error carries no tab list at all, and a transient
                    // failure is not evidence about what exists
                    // (`ListView`'s poll makes the same call for the same
                    // reason).
                    let live: HashSet<&str> =
                        fresh.tabs.iter().map(|tab| tab.id.as_str()).collect();
                    // An optimistic open retires two ways: the server now
                    // lists it (it graduated to the real list), or this
                    // poll STARTED after the open and still does not
                    // mention it (it is genuinely gone — closed from
                    // another client between creation and this view's
                    // first sight of it). A poll that predates the open
                    // says nothing either way and leaves it alone.
                    opened_tabs.write().retain(|(id, observed_from)| {
                        !live.contains(id.as_str()) && index < *observed_from
                    });
                    closed_tabs.write().retain(|id| live.contains(id.as_str()));
                    current.set(fresh);
                }
                // Same per-target sleep split as `ListView`'s poll — see
                // its own comment for why each target gets its own idiom.
                #[cfg(target_arch = "wasm32")]
                gloo_timers::future::TimeoutFuture::new(POLL_INTERVAL_MS as u32).await;
                #[cfg(not(target_arch = "wasm32"))]
                tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
            }
        }
    });

    let restart_base = base.clone();
    let restart = move |stop_if_running: bool| {
        if restarting() {
            return;
        }
        restarting.set(true);
        restart_error.set(None);
        restart_epoch += 1;
        let base = restart_base.clone();
        let id = current.read().id.clone();
        let mode = restart_mode_for(current.read().restart_offer);
        spawn(async move {
            let outcome = restart_session(&base, &id, mode, stop_if_running).await;
            if let Err(e) = &outcome {
                restart_error.set(Some(e.clone()));
            }
            // Refreshed on BOTH paths, and from the server rather than
            // from the reply, for two different reasons that land on the
            // same call:
            //
            // - After a success, the reply's `status` is a deliberate
            //   `Unknown` (the supervisor cannot claim the agent execed
            //   yet), and a view that kept it would think the session is
            //   not running — so the NEXT restart click would skip the
            //   confirmation that exists to stop it killing a live agent.
            // - After a failure, the refusal is most often a STALE OFFER,
            //   whose prescribed handling is to re-present the offer the
            //   session has NOW rather than retry. And a failure is not
            //   even proof the restart did not happen: the reply can be
            //   lost after the relaunch succeeded, which only the server's
            //   own answer can settle.
            //
            // A refresh that fails leaves the reply's own view in place
            // (or, having none, the last known one): an unreachable helm
            // is not evidence about the session.
            match fetch_session(&base, &id).await {
                Ok(Some(fresh)) => current.set(fresh),
                Ok(None) => {
                    // The session is genuinely gone (deleted from
                    // elsewhere, or a restart that raced a delete). Say so
                    // rather than leaving a view describing something that
                    // no longer exists.
                    restart_error.set(Some(format!("session {id} no longer exists on the server")));
                }
                Err(_) => {
                    if let Ok(session) = outcome.as_ref() {
                        current.set(session.clone());
                    }
                }
            }
            // Remounted on both paths too. A success obviously needs it
            // (new pane, or a respawned one, and the server tore the old
            // attachment down). A FAILURE needs it just as much: the
            // server detaches before anything can fail, so a view that
            // only remounted on success would leave the user staring at a
            // permanently detached terminal for a session that is running
            // perfectly well. Remounting when nothing changed is merely a
            // reattach — the same thing a reload does.
            mount_generation += 1;
            // A SECOND bump, closing the other half of the staleness this
            // counter exists for. The first bump (before the request)
            // invalidates polls that were already in flight; without this
            // one, a poll that STARTED during the restart shares the new
            // epoch, passes its own guard, and can land its
            // mid-restart answer on top of the authoritative refresh above
            // — putting the view back to describing the run that just
            // ended. Bumping again means only polls launched after the
            // restart finished are allowed to commit, which is exactly the
            // set that can have seen the result.
            restart_epoch += 1;
            restarting.set(false);
        });
    };
    // One closure, two call sites (the confirm button and the direct
    // restart), cloned rather than duplicated so both send exactly the
    // same request shape and share the same in-flight guard.
    let mut confirm_restart = restart.clone();
    let mut fresh_restart = restart;

    // The add-tab control. Unlike `ListView`'s create, navigating away
    // while this is in flight is deliberately NOT locked out: a stranded
    // create can cost the user a duplicate AGENT they never see, whereas a
    // stranded open just means a tab that exists and is listed the next
    // time this session is opened — nothing is lost and nothing is
    // duplicated. The in-flight flag here only guards re-entry and
    // disables the button.
    let add_base = base.clone();
    let add_session_id = session.id.clone();
    let on_add_tab = move |_| {
        // Signal-level re-entry check, not just the `disabled` attribute:
        // the attribute's DOM update from a rerender is not synchronous
        // with a click event, so a second click landing in that gap would
        // still reach this handler.
        if opening_tab() {
            return;
        }
        opening_tab.set(true);
        // Cleared at the START of this operation and keyed to it alone, so
        // a retry drops its own stale message without touching a close
        // failure the user has not read yet.
        tab_errors.write().remove(TAB_OPEN_ERROR_KEY);
        let base = add_base.clone();
        let session_id = add_session_id.clone();
        spawn(async move {
            match open_tab(&base, &session_id).await {
                Ok(tab) => {
                    // Rendered immediately at the END of the strip and
                    // selected, so the user lands in the terminal they
                    // just asked for; the next poll gives it its real
                    // position (see `visible_tabs`).
                    //
                    // The sequence number is read AFTER the reply, not
                    // before the request: it names the first poll that
                    // could possibly have seen this tab, and a poll
                    // launched while the POST was still in flight could
                    // not have.
                    let observed_from = poll_sequence.peek().to_owned();
                    opened_tabs.write().push((tab.id.clone(), observed_from));
                    selected.set(Some(tab.id));
                }
                // The supervisor's own words, verbatim: a vanished working
                // directory and a session needing a restart first are both
                // refusals SPEC.md requires to name what is wrong.
                Err(e) => {
                    tab_errors
                        .write()
                        .insert(TAB_OPEN_ERROR_KEY.to_string(), format!("open tab: {e}"));
                }
            }
            opening_tab.set(false);
        });
    };

    // The DELETE itself, reached only through the confirmation below.
    let close_base = base.clone();
    let close_session_id = session.id.clone();
    let mut do_close_tab = move |tab_id: String| {
        // Re-entry guard on the per-tab in-flight set, mirroring
        // `ListView`'s `pending`: `insert` returning `false` means a close
        // for this tab is already running.
        if !closing_tabs.write().insert(tab_id.clone()) {
            return;
        }
        // This tab's own previous failure, cleared at the start of the
        // retry that supersedes it — never anyone else's (see
        // `tab_errors`).
        tab_errors.write().remove(&tab_id);
        let base = close_base.clone();
        let session_id = close_session_id.clone();
        spawn(async move {
            match close_tab(&base, &session_id, &tab_id).await {
                Ok(()) => {
                    // Acting on a response already in hand, not guessing:
                    // the same deliberate optimism `ListView` applies to a
                    // deleted row, and for the same reason — leaving the
                    // tab on screen until the next poll would leave a
                    // clickable control that attaches a terminal the
                    // supervisor has already destroyed.
                    closed_tabs.write().insert(tab_id.clone());
                    // Retired from the OPTIMISTIC list too, not just
                    // suppressed by `closed_tabs`. A tab opened and closed
                    // before any poll observed it would otherwise sit in
                    // `opened_tabs` forever — `closed_tabs` is itself
                    // pruned once the server stops listing the id, and for
                    // a tab the server never listed that is immediately,
                    // at which point the stale optimistic entry would
                    // resurrect a tab that does not exist.
                    opened_tabs.write().retain(|(id, _)| id != &tab_id);
                    if selected.read().as_deref() == Some(tab_id.as_str()) {
                        selected.set(None);
                    }
                }
                Err(e) => {
                    tab_errors
                        .write()
                        .insert(tab_id.clone(), format!("close tab: {e}"));
                }
            }
            closing_tabs.write().remove(&tab_id);
        });
    };

    // The close affordance's first click: opens the in-page confirmation
    // and never calls the API. In-page for the reason every confirmation
    // in this UI is (wry ships no native JS dialogs on macOS's WKWebView,
    // where a `window.confirm()` silently does nothing), and confirmed
    // unconditionally rather than only for a busy shell: the UI cannot see
    // what a tab's shell is running, and close kills it either way.
    //
    // Refuses a tab whose close is already in flight, the cross-guard
    // mirroring `ListView`'s: the button is only disabled once a rerender
    // following the `closing_tabs` insert lands, so a click queued just
    // ahead of that could otherwise open a prompt for an operation already
    // under way.
    let on_close_tab = move |tab_id: String| {
        if closing_tabs.read().contains(&tab_id) {
            return;
        }
        confirming_close.set(Some(tab_id));
    };

    // The confirm button. Proceeds ONLY when this tab is still the one
    // being confirmed — `Some(id)` matching — which is what stops a
    // confirm click queued behind a cancel (both fired in the same burst)
    // from closing a tab the user just told the UI to leave alone.
    let mut confirm_close_tab = move |tab_id: String| {
        if confirming_close.read().as_deref() != Some(tab_id.as_str()) {
            return;
        }
        confirming_close.set(None);
        do_close_tab(tab_id);
    };

    // What this view currently wants mounted, as the JSON array
    // `farhelmTerm.sync()` takes — `None` until the lease exists, which is
    // what keeps any terminal from attaching un-leased.
    //
    // A memo rather than computing this inside the effect: the poll above
    // writes `current` on every tick whether or not the session changed,
    // and each write marks its subscribers dirty. A memo re-COMPUTES just
    // as often, but only notifies when the resulting value differs, so the
    // effect — and with it an eval round trip into the page — runs on real
    // changes only. Correctness does not depend on that (`sync()` is
    // idempotent), but a `document::eval` every 3 seconds forever would be
    // pure waste.
    let spec_session_id = session.id.clone();
    let terminal_specs = use_memo(move || {
        let lease = lease.read().clone()?;
        let session = current.read();
        let tabs = visible_tabs(&session.tabs, &opened_tabs.read(), &closed_tabs.read());
        let active = selected.read().clone().filter(|id| tabs.contains(id));
        // Only the tabs within the island cap are ever handed to
        // terminal.js — see `MAX_MOUNTED_TAB_ISLANDS`. The strip still
        // lists the rest; their panes explain themselves instead.
        let mountable = tabs.iter().take(MAX_MOUNTED_TAB_ISLANDS);
        // Every value here goes through serde_json rather than string
        // interpolation: the session and tab ids come from a supervisor,
        // which with --ssh is a different machine. A hostile or compromised
        // host returning an id containing a quote would otherwise get
        // arbitrary JavaScript running on the helm's origin — turning a
        // remote-host compromise into control of the local helm API.
        let mut specs = vec![serde_json::json!({
            "el": AGENT_TERMINAL_ELEMENT_ID,
            "banner": AGENT_BANNER_ELEMENT_ID,
            "path": terminal_ws_path(&spec_session_id, None, &lease),
            // Only the agent terminal carries the restart generation: a
            // restart detaches the agent's attachment alone (the
            // supervisor's `detach_for_restart`), so bumping a tab's would
            // tear down and replay a terminal the restart never touched.
            "gen": mount_generation(),
            // The agent terminal owns terminal.js's legacy singleton test
            // globals — see that file's `mount()`.
            "primary": true,
            "focus": active.is_none(),
        })];
        for id in mountable {
            specs.push(serde_json::json!({
                "el": tab_terminal_element_id(id),
                "banner": tab_banner_element_id(id),
                "path": terminal_ws_path(&spec_session_id, Some(id), &lease),
                "gen": 0,
                "primary": false,
                "focus": active.as_deref() == Some(id.as_str()),
            }));
        }
        Some(serde_json::Value::Array(specs).to_string())
    });

    use_effect(move || {
        // Nothing attaches until the lease is minted; `lease_error` is
        // what the user sees if it never is.
        let Some(specs) = terminal_specs() else {
            return;
        };
        let base_js = serde_json::to_string(&base).expect("string is serializable");
        let js = format!(
            r#"(function() {{
                var gen = (window.__farhelmSyncGeneration || 0) + 1;
                window.__farhelmSyncGeneration = gen;
                (function waitForIsland() {{
                    if (window.__farhelmSyncGeneration !== gen) return;
                    if (window.farhelmTerm) {{
                        farhelmTerm.sync({base_js}, {specs});
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
        // component docs above) aborts before `unmountAll()` below can
        // race it; `unmountAll()` itself is what cancels each island's own
        // pending `mountWhenReady` wait, once the outer one has already
        // handed off to it. Fire-and-forget: this runs on the way out
        // (navigating back to the list, or the whole app tearing down), so
        // there is no reactive state left to update with a result, and the
        // JS side is already written to be a no-op if nothing is mounted.
        document::eval(
            "window.__farhelmSyncGeneration = (window.__farhelmSyncGeneration || 0) + 1; \
             if (window.farhelmTerm) { farhelmTerm.unmountAll(); }",
        );
    });

    let shown = current.read().clone();
    let alive = shown.status == SessionStatus::Alive;
    let tabs = visible_tabs(&shown.tabs, &opened_tabs.read(), &closed_tabs.read());
    // Both of these are DERIVED rather than written back to their signals
    // when they go stale, and that is safe precisely because tab ids are
    // never reused (farhelm-proto's `TabInfo::id`): an id that has left the
    // list can never come back, so a signal still holding it can only ever
    // resolve to "no such tab" and will be overwritten by the user's next
    // click. Writing during render would be the alternative, and Dioxus
    // makes that a re-render loop rather than a fix.
    let active_tab = selected.read().clone().filter(|id| tabs.contains(id));
    let confirming_tab = confirming_close
        .read()
        .clone()
        .filter(|id| tabs.contains(id));
    // The prompt names the tab by the SAME positional label the strip
    // shows, recomputed from the list as it is right now — so a tab that
    // shifts position under an open prompt (a lower-numbered sibling
    // closed from another client) is still identified correctly.
    let confirming_label = confirming_tab
        .as_ref()
        .and_then(|id| tabs.iter().position(|candidate| candidate == id))
        .map(tab_label);
    rsx! {
        div { class: "layout",
            header { class: "titlebar",
                button {
                    class: "btn back-button",
                    onclick: move |_| on_back.call(()),
                    "← back",
                }
                span { class: "title", "{shown.title}" }
                span { class: "meta", "{shown.cwd} — {shown.invocation}" }
            }
            // SPEC.md: "Opening an interrupted session offers
            // restart-with-resume" — which is why this leads the view
            // rather than hiding behind a menu, and why its wording states
            // what restarting would do to the CONVERSATION rather than
            // just naming the action. Declining is simply not clicking it:
            // there is no dismiss, and nothing about the session changes.
            // The same affordance serves every other state too, since
            // restart is the one relaunch mechanism there is.
            div { class: "restart-offer",
                span { class: "restart-offer-text",
                    "{restart_offer_text(&shown.status, shown.restart_offer)}"
                }
                if confirming() {
                    // The inline confirm delete already uses, for the same
                    // reason (SPEC.md: restart on a running agent confirms,
                    // stops, then relaunches) and with the same safety
                    // defaults — consequence text first, focus on cancel.
                    span { class: "confirm-consequence",
                        "still running — restarting stops the agent and its whole process tree first:"
                    }
                    button {
                        r#type: "button",
                        class: "btn restart-confirm",
                        disabled: restarting(),
                        onclick: move |_| {
                            if !confirming() {
                                return;
                            }
                            confirming.set(false);
                            // The only place `stop_if_running` is ever
                            // true: it carries THIS click's consent onto
                            // the wire, which the supervisor then checks
                            // against liveness it rechecks itself.
                            confirm_restart(true);
                        },
                        "confirm restart"
                    }
                    button {
                        r#type: "button",
                        class: "btn restart-cancel",
                        autofocus: true,
                        onclick: move |_| confirming.set(false),
                        "cancel"
                    }
                } else {
                    button {
                        r#type: "button",
                        class: "btn restart-primary",
                        // Whether this click opens the confirmation rather
                        // than restarting outright — the status-derived
                        // decision, exposed so it is inspectable (the
                        // browser suite waits on it) instead of only
                        // observable after the fact by clicking.
                        "data-confirms": "{alive}",
                        disabled: restarting(),
                        onclick: move |_| {
                            if restarting() {
                                return;
                            }
                            if alive {
                                // Never a direct request for a live agent:
                                // restarting one kills it, so the click
                                // only opens the confirmation.
                                confirming.set(true);
                            } else {
                                fresh_restart(false);
                            }
                        },
                        "{restart_button_label(shown.restart_offer)}"
                    }
                }
                if let Some(err) = restart_error.read().clone() {
                    div { class: "restart-error", "{err}" }
                }
            }
            // The tab strip: the agent terminal first and unclosable
            // (SPEC.md gives a session one agent terminal, and closing it
            // is not one of the operations that exist), then every open
            // tab in the server's creation order, then the add control.
            div { class: "tab-strip",
                button {
                    r#type: "button",
                    class: if active_tab.is_none() { "btn tab tab-agent selected" } else { "btn tab tab-agent" },
                    "data-terminal": "agent",
                    onclick: move |_| selected.set(None),
                    "agent"
                }
                // `key` is the tab id, not the loop index, and that is
                // load-bearing rather than a lint: closing a tab shifts
                // every later sibling's position, and an index key would
                // make Dioxus reuse each item's DOM for a DIFFERENT tab —
                // carrying the wrong id into the close handler of a
                // still-open prompt. The id keys the PANES below for the
                // same reason, with more at stake: reused pane DOM would
                // hand one tab's mounted island to another's element.
                for (index , tab_id) in tabs.iter().enumerate() {
                    TabStripItem {
                        key: "{tab_id}",
                        tab_id: tab_id.clone(),
                        label: tab_label(index),
                        selected: active_tab.as_deref() == Some(tab_id.as_str()),
                        busy: closing_tabs.read().contains(tab_id),
                        on_select: move |id| selected.set(Some(id)),
                        on_close: on_close_tab,
                    }
                }
                button {
                    r#type: "button",
                    class: "btn tab-add",
                    disabled: opening_tab(),
                    onclick: on_add_tab,
                    "+ terminal"
                }
            }
            // The close confirmation, on its own row under the strip
            // rather than in place of the tab's own controls: the strip is
            // a single-line flex row that a prompt would push tabs out of,
            // and unlike a session row there is no per-row action area for
            // it to take over. Consequence first, then the label, then the
            // buttons — the same reading order (and the same untruncatable
            // consequence / truncatable name split) the delete prompt uses;
            // see `confirm_consequence`'s docs for why that order is the
            // safety-critical part.
            if let Some(tab_id) = confirming_tab {
                div { class: "tab-confirm",
                    span { class: "confirm-consequence", "{CLOSE_TAB_CONSEQUENCE}" }
                    span { class: "confirm-title",
                        "{confirming_label.clone().unwrap_or_default()}"
                    }
                    button {
                        r#type: "button",
                        class: "btn confirm-delete confirm-close-tab",
                        onclick: move |_| confirm_close_tab(tab_id.clone()),
                        "confirm close"
                    }
                    button {
                        r#type: "button",
                        class: "btn confirm-cancel",
                        // Safe default, exactly as the delete prompt does
                        // it: keyboard focus lands on the way OUT of the
                        // destructive action, via the plain HTML attribute
                        // rather than a fallible `set_focus` whose
                        // discarded `Result` could silently drop the
                        // safety behavior.
                        autofocus: true,
                        onclick: move |_| confirming_close.set(None),
                        "cancel"
                    }
                }
            }
            // One line per failed operation, each keyed to the operation
            // that produced it (see `tab_errors`), so an unrelated success
            // never clears a message the user has not read. Sorted so the
            // lines do not reshuffle on every render — a `HashMap` has no
            // order of its own, and rows jumping around under the strip
            // would make a second failure look like a replaced one.
            for (key , err) in sorted_tab_errors(&tab_errors.read()) {
                div { key: "{key}", class: "tab-error", "data-tab-error": "{key}", "{err}" }
            }
            // Fatal to the terminals, not to the view: the metadata, the
            // restart affordance, and the tab strip all still work, and
            // saying so beats rendering empty panes with no explanation.
            if let Some(err) = lease_error.read().clone() {
                div { class: "lease-error", "{err}" }
            }
            // Worded to state the FACT (the helm stopped listing this
            // session) and the CONSEQUENCE (what is shown may be stale),
            // then BOTH readings — never a pick between them, because this
            // view genuinely cannot tell them apart (see `fetch_session`).
            //
            // An earlier wording ended "its terminals are unaffected",
            // which is a claim this line has no standing to make: under
            // the cap reading it is true, but a session deleted from
            // another client has had its terminals killed, and they will
            // say so in their own banners a moment later. Naming the two
            // possibilities and stopping is the most this can honestly do.
            if refresh_stale() {
                div { class: "refresh-stale",
                    "the helm stopped listing this session, so what is shown here may be out of \
                     date — it was deleted from another client, or there are more sessions than \
                     the helm lists at once"
                }
            }
            // Every terminal is mounted at once and stacked; only the
            // selected one is visible. The agent's pane is a fixed node
            // rather than part of the loop below so that Dioxus never
            // recreates its DOM (and with it `#term-banner`, which
            // long-lived observers in the browser suite hold a reference
            // to) just because the tab list changed around it.
            div { class: "terminal-panes",
                div {
                    class: if active_tab.is_none() { "terminal-pane selected" } else { "terminal-pane" },
                    "data-terminal": "agent",
                    div { id: "{AGENT_BANNER_ELEMENT_ID}", class: "banner" }
                    div { id: "{AGENT_TERMINAL_ELEMENT_ID}", class: "terminal" }
                }
                for (index , tab_id) in tabs.iter().enumerate() {
                    div {
                        key: "{tab_id}",
                        class: if active_tab.as_deref() == Some(tab_id.as_str()) { "terminal-pane selected" } else { "terminal-pane" },
                        "data-terminal": "{tab_id}",
                        // Past the island cap this view renders an
                        // explanation instead of a mount point, and
                        // deliberately renders no terminal DIV at all: an
                        // element with the island's id but no island
                        // behind it is exactly what a later `sync()` would
                        // mount into.
                        if index < MAX_MOUNTED_TAB_ISLANDS {
                            div { id: "{tab_banner_element_id(tab_id)}", class: "banner" }
                            div { id: "{tab_terminal_element_id(tab_id)}", class: "terminal" }
                        } else {
                            div { class: "terminal-not-mounted",
                                "this session reports more than {MAX_MOUNTED_TAB_ISLANDS} terminal tabs; \
                                 this one is listed but not attached (close some to attach it)"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One tab in the strip: its selection button plus its own close
/// affordance, as a child component for the same reason `SessionRow` is
/// one — an rsx `for` body cannot bind locals, and two event handlers in
/// one iteration each need their own owned copy of the tab id. Props give
/// both of them one without the loop having to clone by hand.
///
/// The close control is a SIBLING button, not nested inside the selection
/// button: HTML forbids interactive content inside a `<button>`, so the
/// obvious "× inside the tab" markup would be invalid with undefined
/// browser behavior — the same constraint that split `SessionRow`'s open
/// action out of its row wrapper. Being a real button also means Enter and
/// Space activation, focus styling, and screen-reader semantics come for
/// free.
///
/// There is deliberately no such item for the agent terminal: it is
/// rendered directly by `SessionView` precisely because it must NOT carry
/// a close affordance.
#[component]
fn TabStripItem(
    tab_id: String,
    label: String,
    selected: bool,
    busy: bool,
    on_select: EventHandler<String>,
    on_close: EventHandler<String>,
) -> Element {
    let select_id = tab_id.clone();
    let close_id = tab_id.clone();
    rsx! {
        div { class: "tab-slot", "data-tab-id": "{tab_id}",
            button {
                r#type: "button",
                class: if selected { "btn tab selected" } else { "btn tab" },
                "data-terminal": "{tab_id}",
                onclick: move |_| on_select.call(select_id.clone()),
                "{label}"
            }
            button {
                r#type: "button",
                class: "tab-close",
                // The visible glyph is a multiplication sign, which reads
                // as nothing useful to a screen reader — the accessible
                // name has to carry the tab's identity instead, or every
                // tab's close button would announce identically.
                "aria-label": "close {label}",
                disabled: busy,
                onclick: move |_| on_close.call(close_id.clone()),
                "×"
            }
        }
    }
}

/// What restarting this session would do to its conversation, in the
/// user's own terms — SPEC.md's "restart says so and offers the fallback
/// or a fresh launch — it must never silently resume the wrong
/// conversation", which is a promise about what the user is TOLD, not only
/// about what runs.
///
/// The status leads for `Interrupted` because that is the one state where
/// the user needs to know why their terminal is gone before they are asked
/// to act (SPEC.md: opening an interrupted session offers
/// restart-with-resume). Everything else states the offer alone.
fn restart_offer_text(status: &SessionStatus, offer: RestartOffer) -> String {
    let offered = match offer {
        RestartOffer::Resume => "restarting resumes this session's own conversation",
        RestartOffer::FallbackTemplate => {
            "no conversation was captured, so restarting runs this session's configured resume \
             command"
        }
        RestartOffer::FreshOnly => {
            "no conversation was captured for this session, so restarting launches a fresh agent \
             in the same directory"
        }
    };
    match status {
        SessionStatus::Interrupted => {
            format!("interrupted by a host reboot — {offered}.")
        }
        SessionStatus::Error { .. } => format!("the agent never started — {offered}."),
        _ => format!("{offered}."),
    }
}

/// The restart control's label, which names the OFFER rather than the
/// action: "restart" alone would leave a user guessing whether their
/// conversation survives, which is the exact question SPEC.md requires an
/// honest answer to before they click.
fn restart_button_label(offer: RestartOffer) -> &'static str {
    match offer {
        RestartOffer::Resume => "resume conversation",
        RestartOffer::FallbackTemplate => "restart with the configured resume command",
        RestartOffer::FreshOnly => "restart (fresh launch)",
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
            annotation: None,
            restart_offer: RestartOffer::FreshOnly,
            tabs: Vec::new(),
        }
    }

    /// A tab with the given id — the whole of `Tab`, which is why this is
    /// a one-liner rather than a builder.
    fn tab(id: &str) -> Tab {
        Tab { id: id.into() }
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
            status_badge(&SessionStatus::Alive, None),
            ("alive", "alive".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: Some(7) }, None),
            ("exited", "exited (code 7)".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: None }, None),
            ("exited", "exited".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Unknown, None),
            ("unknown", "unknown".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Interrupted, None),
            ("interrupted", "interrupted".to_string())
        );
        assert_eq!(
            status_badge(
                &SessionStatus::Error {
                    detail: "exec_failed argv0=/nope errno=2".to_string()
                },
                None
            ),
            (
                "error",
                "error — exec_failed argv0=/nope errno=2".to_string()
            ),
            "the shim's own recorded detail must reach the badge text, not just its class"
        );
    }

    /// SPEC.md: "'stopped' is not a distinct status" — a user-stopped
    /// session is an EXITED session carrying a qualifier, so the badge
    /// must still SAY exited and add the supervisor's own wording after
    /// it, with the exit code still visible when there is one. Rendering
    /// the annotation alone (an earlier shape of this) reads as a fourth
    /// status word and drops the one fact the badge exists to state. The
    /// `exited` CSS class is asserted alongside the text for the same
    /// reason: a stopped session must still LOOK like an ended one.
    ///
    /// The live-session case is the one a naive implementation gets
    /// wrong: an annotation describes how a run ENDED, so it must never
    /// leak onto a session that is running — which is exactly what a
    /// stopped-then-restarted session is.
    #[test]
    fn stop_annotation_qualifies_the_exited_badge_without_replacing_it() {
        assert_eq!(
            status_badge(
                &SessionStatus::Exited { exit_code: None },
                Some("stopped by user")
            ),
            ("exited", "exited — stopped by user".to_string())
        );
        assert_eq!(
            status_badge(
                &SessionStatus::Exited { exit_code: Some(0) },
                Some("stopped by user")
            ),
            ("exited", "exited — stopped by user (code 0)".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Alive, Some("stopped by user")),
            ("alive", "alive".to_string()),
            "an annotation must never describe a session that is running"
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

    /// An interrupted session is NOT alive (a host reboot is what made it
    /// interrupted), so its consequence line must not claim anything will
    /// be killed — the same no-fabrication rule that keeps `Unknown` from
    /// borrowing `Alive`'s wording, applied in the opposite direction.
    /// Asserted as properties rather than as one exact string so the
    /// wording can be improved without the test having to be rewritten
    /// each time; what must not change is that it stops promising a kill
    /// and starts naming what deleting actually costs.
    #[test]
    fn interrupted_consequence_promises_no_kill() {
        let wording = confirm_consequence(&SessionStatus::Interrupted);
        assert!(
            !wording.contains("kills") && !wording.contains("will be killed"),
            "nothing survives a reboot for a delete to kill: {wording}"
        );
        assert!(
            wording.contains("reboot") && wording.contains("discard"),
            "the honest consequence is losing the session record itself: {wording}"
        );
    }

    /// Review-swarm fix batch item 22: `confirm_consequence`'s `Error` arm
    /// is reachable — not a defensive completeness case — via the exact
    /// same residual race `Interrupted`'s own test above exercises in
    /// prose: a confirm prompt opened while a session was `Alive` stays
    /// open under a LATER render whose status has since moved on, and
    /// `Error` is one of the statuses it can have moved to. The wording
    /// must match `Error`'s actual meaning (never started, not merely
    /// "finished"), not borrow `Interrupted`'s reboot-specific phrasing.
    #[test]
    fn error_consequence_promises_no_kill_and_names_no_reboot() {
        let wording = confirm_consequence(&SessionStatus::Error {
            detail: "exec_failed argv0=/nope errno=2".to_string(),
        });
        assert!(
            !wording.contains("kills") && !wording.contains("will be killed"),
            "an agent that never started leaves nothing for a delete to kill: {wording}"
        );
        assert!(
            !wording.contains("reboot"),
            "an exec failure is not a reboot; the wording must not borrow that framing: {wording}"
        );
        assert!(
            wording.contains("discard"),
            "the honest consequence is losing the session record itself: {wording}"
        );
    }

    /// A `Session` JSON with no `annotation` key (every session that was
    /// never stopped, and every reply from a helm predating PLAN_M3.md
    /// item 4) must decode as `None` rather than failing the whole
    /// listing — the same decode tolerance `status` carries.
    #[test]
    fn session_without_annotation_field_decodes_as_none() {
        let json = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
            "status": { "state": "interrupted" },
        });
        let decoded: Session = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.status, SessionStatus::Interrupted);
        assert_eq!(decoded.annotation, None);
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

    /// A `Session` JSON with no `restart_offer` (a helm predating
    /// PLAN_M3.md item 9) must decode as `FreshOnly`, never as something
    /// that would make this UI offer a resume the supervisor would then
    /// refuse. The same no-fabrication direction `status`'s own default
    /// takes.
    #[test]
    fn session_without_restart_offer_decodes_as_fresh_only() {
        let json = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
            "status": { "state": "interrupted" },
        });
        let decoded: Session = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.restart_offer, RestartOffer::FreshOnly);

        let resumable = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
            "status": { "state": "interrupted" },
            "restart_offer": "resume",
        });
        let decoded: Session = serde_json::from_value(resumable).unwrap();
        assert_eq!(decoded.restart_offer, RestartOffer::Resume);
    }

    /// Each offer authorizes exactly one mode, and the wire spellings must
    /// be the ones farhelm-proto's `RestartMode` decodes — a typo here
    /// would turn every restart into a 400 nothing in this crate could
    /// explain.
    #[test]
    fn each_offer_maps_to_its_one_legal_mode() {
        assert_eq!(restart_mode_for(RestartOffer::FreshOnly), "fresh");
        assert_eq!(restart_mode_for(RestartOffer::Resume), "resume");
        assert_eq!(
            restart_mode_for(RestartOffer::FallbackTemplate),
            "fallback_template"
        );
    }

    /// SPEC.md requires restart to SAY what it would do to the
    /// conversation — "it must never silently resume the wrong
    /// conversation" is a promise about what the user is told before they
    /// click, not only about what runs. So the three offers must read
    /// differently, and a fresh launch must say so rather than borrowing
    /// resume's wording.
    ///
    /// The interrupted case leads with WHY the terminal is gone, since
    /// that is the state where the user is being asked to act on something
    /// they did not do (SPEC.md: opening an interrupted session offers
    /// restart-with-resume).
    #[test]
    fn the_offer_text_states_what_would_happen_to_the_conversation() {
        let resumable = restart_offer_text(&SessionStatus::Interrupted, RestartOffer::Resume);
        assert!(
            resumable.contains("reboot") && resumable.contains("resumes"),
            "an interrupted, resumable session must say both: {resumable}"
        );

        let fresh = restart_offer_text(&SessionStatus::Interrupted, RestartOffer::FreshOnly);
        assert!(
            fresh.contains("no conversation was captured") && fresh.contains("fresh agent"),
            "a fresh-only restart must say plainly that nothing is resumed: {fresh}"
        );

        let fallback =
            restart_offer_text(&SessionStatus::Interrupted, RestartOffer::FallbackTemplate);
        assert!(
            fallback.contains("configured resume command"),
            "a configured fallback is labeled honestly, not as a plain fresh launch: {fallback}"
        );

        let error = restart_offer_text(
            &SessionStatus::Error {
                detail: "exec_failed".to_string(),
            },
            RestartOffer::FreshOnly,
        );
        assert!(
            error.contains("never started"),
            "an errored session's own reason leads instead: {error}"
        );
    }

    /// The button names the OFFER, not the action: "restart" alone leaves
    /// the user guessing whether their conversation survives, which is the
    /// exact question SPEC.md requires answered before they click.
    #[test]
    fn the_restart_button_label_names_the_offer() {
        assert_eq!(
            restart_button_label(RestartOffer::Resume),
            "resume conversation"
        );
        assert!(
            restart_button_label(RestartOffer::FreshOnly).contains("fresh"),
            "a fresh launch must not be labeled as a resume"
        );
        assert!(
            restart_button_label(RestartOffer::FallbackTemplate).contains("resume command"),
            "a configured fallback is its own thing, distinct from both"
        );
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

    // -----------------------------------------------------------------
    // Terminal tabs (PLAN_M4.md item 6)
    // -----------------------------------------------------------------

    /// A `Session` JSON with no `tabs` key — every reply from a helm that
    /// predates PLAN_M4.md item 5 — must decode as "no tabs known" rather
    /// than failing the whole view, the same old-peer tolerance `status`
    /// and `restart_offer` carry. Fabricating tabs in either direction is
    /// impossible here (there is only one empty value), so the risk this
    /// pins is purely the decode ERROR that a missing field would
    /// otherwise be.
    #[test]
    fn session_without_tabs_field_decodes_as_no_tabs() {
        let json = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
        });
        let decoded: Session = serde_json::from_value(json).unwrap();
        assert!(decoded.tabs.is_empty());

        let with_tabs = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
            "tabs": [{ "id": "tab-1" }, { "id": "tab-2" }],
        });
        let decoded: Session = serde_json::from_value(with_tabs).unwrap();
        assert_eq!(
            decoded.tabs,
            vec![tab("tab-1"), tab("tab-2")],
            "the server's order is the one the strip's positional labels are derived from, so \
             decoding must preserve it"
        );
    }

    /// The server's list is the spine of the rendered order, and the two
    /// optimistic corrections ride on top of it: a just-opened tab the
    /// server has not listed yet appears at the END (a tab-open reply
    /// carries no ordering — see `visible_tabs`), and a just-closed tab
    /// disappears immediately even while a poll in flight still lists it.
    /// Getting either edge wrong is directly visible to the user as a
    /// click that does nothing or a tab that vanishes and comes back.
    #[test]
    fn visible_tabs_applies_the_optimistic_corrections_over_the_server_list() {
        let server = vec![tab("a"), tab("b")];

        assert_eq!(
            visible_tabs(&server, &[], &HashSet::new()),
            vec!["a".to_string(), "b".to_string()],
            "with nothing pending, the server's list is the whole answer, in its order"
        );

        assert_eq!(
            visible_tabs(&server, &[("c".to_string(), 0)], &HashSet::new()),
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            "an optimistic open lands at the end until a refresh places it"
        );

        let closed: HashSet<String> = ["a".to_string()].into_iter().collect();
        assert_eq!(
            visible_tabs(&server, &[], &closed),
            vec!["b".to_string()],
            "a closed tab is gone at once, even while the server still lists it"
        );
    }

    /// The one case both corrections meet: a poll that has caught up
    /// already lists the optimistically-added tab, and it must appear
    /// ONCE, in the server's position — not twice, and not pinned to the
    /// end. `SessionView`'s poll prunes the optimistic entry, but this
    /// deduplication is what keeps the render correct in the window before
    /// that prune runs.
    #[test]
    fn visible_tabs_never_duplicates_an_optimistic_tab_the_server_now_lists() {
        let server = vec![tab("a"), tab("b"), tab("c")];
        assert_eq!(
            visible_tabs(&server, &[("b".to_string(), 0)], &HashSet::new()),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    /// Labels are positional and one-based, derived from list position
    /// rather than from anything the tab itself carries — SPEC.md gives
    /// tabs no names, so this IS the naming rule, not a placeholder for
    /// one.
    #[test]
    fn tab_labels_are_one_based_positions() {
        assert_eq!(tab_label(0), "Terminal 1");
        assert_eq!(tab_label(1), "Terminal 2");
    }

    /// Each terminal gets its own mount point and its own banner, and the
    /// agent's two ids are exactly the ones the pre-tabs UI used — a
    /// session with no tabs must render the DOM it always did, and the
    /// browser suite keys off both names throughout. The per-tab ids must
    /// be distinct from the agent's and from each other, since terminal.js
    /// uses them as the identity of an island.
    #[test]
    fn terminal_element_ids_are_distinct_per_terminal() {
        assert_eq!(AGENT_TERMINAL_ELEMENT_ID, "terminal");
        assert_eq!(AGENT_BANNER_ELEMENT_ID, "term-banner");
        assert_eq!(tab_terminal_element_id("t1"), "terminal-t1");
        assert_eq!(tab_banner_element_id("t1"), "term-banner-t1");
        assert_ne!(tab_terminal_element_id("t1"), tab_terminal_element_id("t2"));
        assert_ne!(tab_terminal_element_id("t1"), tab_banner_element_id("t1"));
    }

    /// The lease rides on EVERY terminal, the agent's included: a view
    /// that leased only its tabs would have its own terminals take each
    /// other over (PLAN_M4.md item 3). The agent's URL carries no `?tab=`
    /// at all, which is the pre-M4 reading the helm still honors.
    #[test]
    fn every_terminal_path_carries_the_lease_and_only_tabs_carry_a_selector() {
        assert_eq!(
            terminal_ws_path("s1", None, "lease-1"),
            "/api/sessions/s1/term?lease=lease-1"
        );
        assert_eq!(
            terminal_ws_path("s1", Some("t1"), "lease-1"),
            "/api/sessions/s1/term?tab=t1&lease=lease-1"
        );
    }

    /// A tab id or lease containing query syntax must not be able to
    /// re-split the URL and change which terminal gets attached — the tab
    /// id in particular comes from a supervisor that, under `--ssh`, is a
    /// different machine. Percent-encoding is what makes that structural
    /// rather than a matter of trusting the id's shape.
    #[test]
    fn query_values_are_percent_encoded() {
        assert_eq!(encode_query_value("plain-id_1.0~"), "plain-id_1.0~");
        assert_eq!(encode_query_value("a&b=c"), "a%26b%3Dc");
        assert_eq!(encode_query_value("a b"), "a%20b");
        // Per byte, so multi-byte UTF-8 encodes to several escapes rather
        // than being mangled into one.
        assert_eq!(encode_query_value("é"), "%C3%A9");
        assert_eq!(
            terminal_ws_path("s1", Some("t&lease=stolen"), "mine"),
            "/api/sessions/s1/term?tab=t%26lease%3Dstolen&lease=mine",
            "an injected parameter must stay part of the tab VALUE, never become a second key"
        );
    }

    /// Close is destructive in a way the word "close" understates: the
    /// supervisor's tab-scoped reap goes after the shell's daemonized
    /// descendants too (SPEC.md: close "kills that shell and its
    /// processes"). The consequence line is what tells the user that
    /// before they confirm, so it must promise the kill and mention what
    /// else goes with it.
    #[test]
    fn close_tab_consequence_states_the_kill_and_its_reach() {
        assert_eq!(
            CLOSE_TAB_CONSEQUENCE,
            "closing kills this terminal's shell and every process it started:",
            "the exact sentence is the contract — it is the last thing a user reads before \
             destroying a shell and everything under it, so a reworded or softened version is a \
             change to review, not an implementation detail to drift"
        );
    }

    /// A tab id is supervisor-supplied and lands in a URL PATH, so an id
    /// carrying path syntax must not be able to choose which resource the
    /// request names. `../../victim` is the concrete attack: unescaped, a
    /// URL parser resolves the dot segments before the request is ever
    /// sent, turning a tab close into `DELETE /api/sessions/victim` — a
    /// remote supervisor deleting a local session. Both offenders are
    /// pinned, because either alone is enough: `/` ends the segment, and a
    /// segment that is exactly `..` is resolved away even with no slash in
    /// it at all.
    #[test]
    fn path_segments_cannot_escape_their_segment() {
        assert_eq!(
            encode_path_segment("../../victim"),
            "%2E%2E%2F%2E%2E%2Fvictim",
            "a traversal must survive as literal text inside one segment"
        );
        assert_eq!(
            encode_path_segment(".."),
            "%2E%2E",
            "a bare dot segment resolves away without any slash, so the dots themselves have to go"
        );
        assert_eq!(
            encode_path_segment("9c3d5a71-0000-4000-8000-0000000000ff"),
            "9c3d5a71-0000-4000-8000-0000000000ff",
            "the ids this actually carries must pass through untouched"
        );
        // The narrower set is the whole difference between the two
        // encoders, so it is asserted as a difference rather than twice
        // over: `.` is legal in a query value and not in a segment.
        assert_eq!(encode_query_value("a.b"), "a.b");
        assert_eq!(encode_path_segment("a.b"), "a%2Eb");
    }

    /// Error lines are rendered in a list the user reads, and a `HashMap`
    /// has no iteration order — without a sort, an unrelated re-render
    /// could reshuffle them, which reads as messages being replaced rather
    /// than accumulating. What is pinned is STABILITY (the same set always
    /// renders the same way), not the particular order.
    #[test]
    fn tab_errors_render_in_a_stable_order() {
        let errors: HashMap<String, String> = [
            ("tab-b".to_string(), "close tab: boom".to_string()),
            (TAB_OPEN_ERROR_KEY.to_string(), "open tab: nope".to_string()),
            ("tab-a".to_string(), "close tab: bang".to_string()),
        ]
        .into_iter()
        .collect();
        let keys: Vec<String> = sorted_tab_errors(&errors)
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert_eq!(keys, vec!["open", "tab-a", "tab-b"]);
    }
}
