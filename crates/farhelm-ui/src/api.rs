//! The helm's HTTP contract, as this UI speaks it: one `async fn` per
//! endpoint (fetch/create/stop/restart/delete/tab-open/tab-close), each
//! flattening every failure — transport, status, or body-read — into a
//! single displayable `String` rather than a typed error. That flattening
//! is deliberate, not laziness: every caller (`list::ListView`,
//! `list::CreateSessionForm`, `session_view::SessionView`) renders the
//! message directly to the user per SPEC.md's "concrete, actionable
//! errors", so there is no second consumer that would ever want a
//! structured error to match on.
//!
//! `SessionListing`, `POLL_INTERVAL_MS`, and `restart_mode_for` live here
//! too, even though none of them performs I/O directly: the first is this
//! module's own decoded response shape, the second is the cadence both
//! pollers (`ListView`'s listing poll and `SessionView`'s detail poll)
//! share, and the third documents the wire-level pairing `restart_session`
//! enforces from the caller's side — all three are part of the HTTP
//! contract this module owns, not the view code that consumes it.
//!
//! The URL-building helpers (`encode_query_value`, `encode_path_segment`)
//! are `pub(crate)` rather than private: every endpoint below that embeds
//! an opaque id runs it through `encode_path_segment`, and
//! `tabs::terminal_ws_path` needs both encoders for the terminal
//! WebSocket's path and query. `encode_bytes` is their shared,
//! module-private implementation.
//!
//! The cross-module entry points are `pub(crate)`: they exist to be
//! called from the view components in `list`, `session_view`, and
//! `tabs`, never from outside this crate — `main.rs` only ever reaches
//! `App`/`ApiBase` (see `lib.rs`). Internal helpers (`encode_bytes`,
//! `sort_sessions`, `eval_minted_id`) stay private to this module.

use crate::{RestartOffer, Session, Tab};
use serde::Deserialize;

/// Mirror of the helm's `GET /api/sessions` response body (farhelm-helm's
/// `SessionListing`, PLAN_M2.md step 6): `{"sessions": [...], "total": N,
/// "truncated": bool}`. `total`/`truncated` are `#[serde(default)]` for
/// the same old-peer tolerance as `Session::status` above; a decoder that
/// only ever talks to this build's own helm will never hit that path, but
/// nothing here should assume that. `Deserialize` is the only derive this
/// needs: it is read once by reference out of the signal that holds it
/// (see `fetch_sessions`'s docs), never cloned or compared.
///
/// `pub(crate)` on the type and every field: `list::ListView` holds one of
/// these in its own signal and reads `sessions`/`total`/`truncated`
/// directly, which a private struct (this module's default) cannot allow
/// across the module boundary this split introduced. Nothing outside the
/// crate has any business seeing it.
#[derive(Deserialize)]
pub(crate) struct SessionListing {
    pub(crate) sessions: Vec<Session>,
    #[serde(default)]
    pub(crate) total: u64,
    #[serde(default)]
    pub(crate) truncated: bool,
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
/// terminal, which is precisely the failure `tabs::terminal_ws_path`'s docs
/// call worse than an outright error.
///
/// Encodes per BYTE, so non-ASCII is UTF-8 percent-encoded correctly
/// rather than mangled. Hand-rolled rather than pulling in a crate for two
/// tiny functions.
///
/// `pub(crate)`: `tabs::terminal_ws_path` is this function's only caller
/// outside this module, but it needs it for both a tab id and a lease.
pub(crate) fn encode_query_value(value: &str) -> String {
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
///
/// `pub(crate)`: every endpoint in this module uses it on its own ids, and
/// `tabs::terminal_ws_path` uses it on the session id shared by every
/// terminal's WebSocket path.
pub(crate) fn encode_path_segment(value: &str) -> String {
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
pub(crate) async fn fetch_sessions(base: &str) -> Result<SessionListing, String> {
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
/// `list::ListView` polls the listing and `session_view::SessionView`
/// polls its own session's detail for tab-list changes (PLAN_M4.md item 6
/// asks for "the same polling M2 settled for the session list"), and M5
/// replaces both with live push together, so a divergence here would be a
/// difference no one chose and no one would maintain.
pub(crate) const POLL_INTERVAL_MS: u64 = 3_000;

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
/// `list::CreateSessionForm`).
pub(crate) async fn create_session(
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
/// (see `session_view::SessionView`, which already has that path for the
/// eval channel dying).
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
/// slow — which made `list::CreateSessionForm`'s fail-closed guard refuse
/// EVERY create (MT-5). Minting the UUID in Rust removes the dependency on
/// that channel entirely rather than working around one platform's flaky
/// eval; desktop already links a real RNG (`uuid`'s `v4` feature,
/// `getrandom` underneath), so there was never a reason to route this
/// through the webview to begin with. That precedent is why the lease is
/// minted here at all rather than in terminal.js, where the desktop build
/// would have hit exactly the same dead channel.
///
/// `Err` carries a message suitable for direct display. Both of its causes
/// — a dead eval channel and a browser without `crypto.randomUUID` — are
/// reported the same way, because the caller's response is the same either
/// way: attach nothing, and say so.
pub(crate) async fn mint_lease() -> Result<String, String> {
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
pub(crate) async fn mint_intent_key() -> Result<String, String> {
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
pub(crate) async fn stop_session(base: &str, id: &str) -> Result<(), String> {
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
/// worked — see `session_view::SessionView`, which keeps what it has and
/// says the refresh is not landing.
///
/// A transport failure stays an `Err`, because "the helm did not answer"
/// and "the helm answered 404" must not be confused either.
pub(crate) async fn fetch_session(base: &str, id: &str) -> Result<Option<Session>, String> {
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
/// session rather than retrying (see `session_view::SessionView`).
///
/// `stop_if_running` carries the user's explicit consent to stop a live
/// agent first; the caller only sets it after the inline confirmation, and
/// the supervisor rechecks real liveness before honoring it. Same
/// error-surfacing shape as `stop_session` above, including the
/// body-read-failure context.
pub(crate) async fn restart_session(
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
pub(crate) async fn delete_session(base: &str, id: &str) -> Result<(), String> {
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
pub(crate) async fn open_tab(base: &str, session_id: &str) -> Result<Tab, String> {
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
pub(crate) async fn close_tab(base: &str, session_id: &str, tab_id: &str) -> Result<(), String> {
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

/// The wire spelling of the restart mode this offer authorizes, and the
/// ONLY mode the supervisor will accept for it.
///
/// The pairing is exact in both directions, which is why this is a function
/// of the offer rather than a user choice: SPEC.md has no fresh-restart
/// variant in v1 ("for a clean conversation, create a new session in the
/// same directory"), so a session that CAN resume has no legal "restart
/// fresh instead" — and a session that cannot has nothing to resume. The
/// supervisor rejects any other pairing with a conflict naming the current
/// offer, which is exactly the staleness case `restart_session`'s caller
/// handles by refreshing (see `session_view::SessionView`).
///
/// `pub(crate)`, not private: this module owns the wire contract, but the
/// only call site is `SessionView`'s restart closure in `session_view`, on
/// the other side of the module split.
pub(crate) fn restart_mode_for(offer: RestartOffer) -> &'static str {
    match offer {
        RestartOffer::FreshOnly => "fresh",
        RestartOffer::Resume => "resume",
        RestartOffer::FallbackTemplate => "fallback_template",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionStatus;

    /// Shared test fixture: a minimally distinct `Session` keyed by `id`.
    /// Lives in this module (rather than a crate-wide test-support module)
    /// because it is only `sort_sessions`'s own ordering test that needs
    /// more than one distinguishable session at a time.
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

    /// The other half of `SessionListing`'s missing-field tolerance
    /// (mirrors `crate::tests::session_without_status_field_decodes_as_unknown`,
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
}
