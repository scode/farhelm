//! The helm's HTTP contract, as this UI speaks it: one `async fn` per
//! endpoint — the session routes
//! (fetch/create/stop/restart/rename/delete/tab-open/tab-close) and, since
//! PLAN_M6.md item 5 froze them, the host registry's
//! (list/add/retarget/remove/adopt/retry) — each flattening every failure
//! — transport, status, or body-read — into a single displayable `String`
//! rather than a typed error. That flattening is deliberate, not laziness:
//! every caller (`list::ListView`, `list::CreateSessionForm`,
//! `hosts::HostsPanel`, `session_view::SessionView`) renders the message
//! directly to the user per SPEC.md's "concrete, actionable errors", so
//! there is no second consumer that would ever want a structured error to
//! match on.
//!
//! `SessionPage`/`SessionListing`, `SessionFilter`, `POLL_INTERVAL_MS`, and
//! `restart_mode_for` live here too, even though none of them performs I/O
//! directly: the first pair is this module's own decoded response shape and
//! the walked result it assembles, `SessionFilter` is the query surface's
//! half of the helm's server-side filtering (PLAN_M6_75.md item 7),
//! `POLL_INTERVAL_MS` is the cadence the FALLBACK polls run at, and the last
//! documents the wire-level pairing `restart_session` enforces from the
//! caller's side — all of them are part of the HTTP contract this module
//! owns, not the view code that consumes it.
//!
//! The URL-building helpers (`encode_query_value`, `encode_path_segment`)
//! are `pub(crate)` rather than private: every endpoint below that embeds
//! an opaque id runs it through `encode_path_segment`, and
//! `tabs::terminal_ws_path` needs both encoders for the terminal
//! WebSocket's path and query. `encode_bytes` is their shared,
//! module-private implementation.
//!
//! Every request below is issued through this module's own [`send`], which
//! is also where the helm's build stamp is read off the reply (`skew`).
//! That funnel is a contract rather than a convenience: the skew check only
//! means anything if there is no second path to the helm that skips it.
//!
//! The cross-module entry points are `pub(crate)`: they exist to be
//! called from the view components in `list`, `session_view`, and
//! `tabs`, never from outside this crate — `main.rs` only ever reaches
//! `App`/`ApiBase` (see `lib.rs`). Internal helpers (`client`, `send`,
//! `encode_bytes`, `install_field`, `eval_minted_id`, and the two
//! failure-text builders) stay private to this module.

use crate::skew;
use crate::{Host, HostId, RestartOffer, Session, Tab};
use serde::Deserialize;

/// Mirror of one PAGE of the helm's `GET /api/sessions` response
/// (farhelm-helm's `SessionPageBody`): `{"sessions": [...], "total": N,
/// "matching": N, "truncated": bool, "next_cursor": "…"}`.
///
/// `total`/`truncated` keep `#[serde(default)]` for the same old-peer
/// tolerance as `Session::status`; `next_cursor` needs none, since serde
/// already decodes a missing `Option` key as `None` — and absent is exactly
/// what "this was the last page" means on the wire.
///
/// Private, unlike the walked result: no caller wants one page. See
/// [`SessionListing`], which is what a walk produces.
#[derive(Deserialize)]
struct SessionPage {
    sessions: Vec<Session>,
    #[serde(default)]
    total: u64,
    /// How many sessions match the request's filter across the whole merged
    /// view — the other half of "N matching of M" (PLAN_M6_75.md item 5).
    ///
    /// An `Option` rather than a `#[serde(default)]` u64, and the difference
    /// is the difference between a tolerated old reply and a fabricated
    /// count. Defaulting to 0 would make a helm that predates the field say
    /// "0 matching of 700 sessions" over a list full of rows — a claim
    /// nothing supports, on the one line whose whole job is to be believed.
    /// Absent means "this helm does not report a matching count", and what
    /// the walk may honestly do with that depends on the REQUEST — see
    /// [`matching_count`].
    matching: Option<u64>,
    #[serde(default)]
    truncated: bool,
    /// Opaque resume key for the next page — replayed verbatim, never
    /// constructed or interpreted here.
    next_cursor: Option<String>,
}

/// The session list's query surface (PLAN_M6_75.md item 7): SPEC.md's five
/// dimensions, as the values a user typed or chose.
///
/// Filtering is a QUERY, not a render pass. Every field here becomes a
/// parameter on `GET /api/sessions` and the helm answers with the matching
/// rows plus their count — which is the only arrangement that can be
/// coherent with pagination at all. A client filtering the page it was
/// handed would hide matches beyond the page cut while reporting a count
/// that included them, and the count is what the banner says out loud.
///
/// The parent-reference dimension SPEC.md ties to spawned sessions is
/// deliberately absent: it ships in M7 beside the feature that mints parent
/// references, not here where no session has one (PLAN_M6_75.md's Out).
///
/// Strings rather than `Option<String>` because a text field's empty value
/// IS its absent value, and the helm agrees — an exactly-empty parameter is
/// treated as absent there, which is what makes clearing a search box widen
/// the list instead of erroring.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SessionFilter {
    /// A registered host's id, from `GET /api/hosts`.
    pub(crate) host: Option<HostId>,
    /// Substring of the working directory, case-insensitively.
    pub(crate) directory: String,
    /// A profile, named by its id or by the name a session snapshotted at
    /// creation — the latter is what keeps a DELETED profile's sessions
    /// findable, and is why this is free text rather than a picker over the
    /// catalog as it stands today.
    pub(crate) profile: String,
    /// A status, spelled exactly as the wire spells it. The helm refuses a
    /// word it does not know with a 400 rather than answering "no sessions",
    /// so this is offered as a choice rather than typed.
    pub(crate) status: String,
    /// Substring of the title, case-insensitively — SPEC.md's "search".
    pub(crate) title: String,
}

impl SessionFilter {
    /// Whether anything is being filtered on.
    ///
    /// Drives the count banner's wording (`rows::count_banner`) and, more
    /// quietly, what a reply is EVIDENCE about: a filtered listing says
    /// nothing about the sessions it excluded, so the reconciliations that
    /// treat absence as departure are held back for one (see
    /// `list::ListView`'s commit path).
    pub(crate) fn is_active(&self) -> bool {
        self.host.is_some()
            || !self.directory.is_empty()
            || !self.profile.is_empty()
            || !self.status.is_empty()
            || !self.title.is_empty()
    }

    /// This filter as the query string's parameters, percent-encoded and
    /// joined — empty when nothing is set.
    ///
    /// Values travel BYTE FOR BYTE apart from the encoding: no trimming, on
    /// purpose and in step with the helm, which drops only the exactly-empty
    /// value. A directory may legitimately contain surrounding whitespace
    /// and a title may be `fix  the  spacing`, so trimming would make text
    /// that is actually there unfindable — and would collapse `" "` and `""`
    /// into one request, which the user can see as a space silently clearing
    /// their filter.
    fn query(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(host) = self.host {
            parts.push(format!("host={host}"));
        }
        for (name, value) in [
            ("directory", &self.directory),
            ("profile", &self.profile),
            ("status", &self.status),
            ("title", &self.title),
        ] {
            if !value.is_empty() {
                parts.push(format!("{name}={}", encode_query_value(value)));
            }
        }
        parts.join("&")
    }
}

/// The whole session list, as this UI holds it: every page of the helm's
/// cursor walk concatenated, in the helm's own order.
///
/// `pub(crate)` on the type and every field: `list::ListView` holds one of
/// these in its own signal and reads all three directly, which a private
/// struct cannot allow across the module boundary this split introduced.
/// Nothing outside the crate has any business seeing it.
pub(crate) struct SessionListing {
    pub(crate) sessions: Vec<Session>,
    /// Every session in the merged view across every host, as the helm
    /// counts them — which is what SPEC.md's "showing N of M" is about, and
    /// is NOT the same as `sessions.len()` whenever a walk stopped early.
    ///
    /// Counted BEFORE any filter, deliberately: it is the fleet's size, so
    /// "N matching of M sessions" has an M that does not move when the user
    /// types.
    pub(crate) total: u64,
    /// How many sessions matched the filter, fleet-wide — or `None` when
    /// this helm did not say and no honest number can be substituted.
    ///
    /// Fleet-wide rather than page-wide is the whole point: a walk that
    /// stopped at a ceiling holds fewer rows than matched, and the banner's
    /// job is to say so.
    ///
    /// `None` is only ever produced for a FILTERED walk against a helm that
    /// predates the count (see [`matching_count`]), and it travels this far
    /// rather than being resolved in the walk because it is a fact about the
    /// helm the banner has to render — `rows::count_banner` says the filter
    /// went unanswered instead of printing a number nobody counted.
    pub(crate) matching: Option<u64>,
    /// Whether this walk carried a filter at all.
    ///
    /// From the REQUEST, never derived by comparing `matching` against
    /// `total`: a filter that happens to match everything is still a filter,
    /// and the banner should say "5 matching of 5 sessions" rather than
    /// silently reverting to the unfiltered wording and leaving the user
    /// wondering whether their filter took.
    pub(crate) filtered: bool,
    /// Whether entries remain beyond what `sessions` carries: the walk hit
    /// one of its own ceilings, the helm's last page still reported more, or
    /// the walk turned out `incoherent`.
    pub(crate) truncated: bool,
    /// Whether the walk collected more rows than the helm says exist.
    ///
    /// Reported separately from `truncated` because the two mean different
    /// things to a reader: truncated is "there is more", incoherent is "this
    /// changed underneath the walk, so the count and the rows disagree". The
    /// UI says so rather than presenting either number as authoritative.
    pub(crate) incoherent: bool,
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

// ---------------------------------------------------------------------
// Every request, one door
// ---------------------------------------------------------------------

/// The HTTP client every request below is built from.
///
/// A SEAM, not an optimization. It exists so that the next thing which has to
/// be true of every outbound request — M7's token auth — is one edit here
/// rather than fifteen edits spread across this module, with the fifteenth
/// one forgotten and silently unauthenticated. That is the same argument
/// [`send`] makes on the reply side, and the two together are what make "every
/// request goes through one door" a property rather than a habit.
///
/// Deliberately constructs a FRESH client per call, which is what the fifteen
/// call sites did before this function existed. A shared client is the
/// obvious-looking improvement and is a real behavior change, not a
/// refactor: `reqwest::Client` owns the connection pool, and it also captures
/// proxy settings, TLS configuration and the DNS resolver at construction, so
/// hoisting one into a `static` freezes all of that at whatever the process
/// looked like on the first request and keeps connections alive across the
/// whole run.
///
/// M7 is where it deliberately becomes shared, because that is when sharing
/// starts carrying meaning the UI needs rather than just saving handshakes: a
/// cookie jar and a device session have to persist ACROSS requests to work at
/// all. Making that switch there gets it reviewed as the behavior change it
/// is, alongside the auth it serves.
fn client() -> reqwest::Client {
    reqwest::Client::new()
}

/// How long any one request may take before it is abandoned.
///
/// It exists because of `reader`, not because of the network. A surface now
/// runs ONE read at a time: a request that never completes and never fails
/// would hold that reader forever, and the surface would sit stale with no
/// retry ever scheduled — the previous arrangement, which spawned a read per
/// notification, at least kept trying (while accumulating tasks against a
/// helm that had stopped answering, which is the problem the reader solves).
/// Bounding the request is what makes the single-flight reader safe: a hung
/// connection becomes an ordinary failed read, and the retry ladder takes
/// over.
///
/// Sixty seconds is deliberately generous rather than tuned. A read is
/// expected to take milliseconds, but this door is shared with the host
/// mutations, and those do real work on another machine — an add or an adopt
/// opens an SSH connection and inspects an install. The number matches the
/// helm's own stall bounds (`uploads.rs`'s sixty-second deadlines) so the
/// two sides give up on roughly the same scale. Nothing here streams a large
/// body: uploads never pass through this module (terminal.js owns them, see
/// `attachments`), so a total-request deadline cannot cut a transfer short.
///
/// Applied in [`send`] rather than on the client, so it holds for every
/// request by construction — the same argument the funnel itself makes.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Send one request and read the helm's build stamp off its reply
/// (PLAN_M6.md item 6's client↔helm skew edge).
///
/// EVERY request this module makes goes through here, and that is the point
/// rather than tidiness: the skew check's whole premise is that a tab left
/// open across a helm upgrade notices on whatever it does next, which only
/// holds if there is no second path to the helm that skips the observation.
/// A call site that reached for `reqwest` directly would be a hole in that,
/// invisible until someone upgraded a helm under a real tab.
///
/// The reply is handed back untouched — the observation only reads a
/// header — so this is a one-line substitution at each call site and every
/// status/decode decision below is unchanged.
///
/// The [`REQUEST_TIMEOUT`] is applied here for the same funnel reason: a
/// per-call-site deadline is a deadline someone eventually forgets, and the
/// one request left unbounded is the one that wedges a surface.
///
/// One caller needs a SHORTER deadline than the default and reaches
/// [`send_within`] for it — the listing walk, whose pages share one budget
/// (see `walk_step`). Nothing may reach for a longer one, which is why this
/// is the door everything else uses.
async fn send(request: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
    send_within(request, REQUEST_TIMEOUT).await
}

/// [`send`] with an explicit deadline, for a request that is one step of a
/// larger bounded operation.
///
/// Separate from `send` rather than a parameter on it, because the choice is
/// not one every call site should be invited to make: fifteen endpoints want
/// the standard deadline and exactly one — a page of the listing walk — has
/// a budget of its own to divide up. The build-stamp observation is the same
/// either way, which is the property that must not vary.
async fn send_within(
    request: reqwest::RequestBuilder,
    timeout: std::time::Duration,
) -> Result<reqwest::Response, String> {
    let resp = request
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    // Called for its effect, in a statement of its own. Folding it into a
    // `.map()` reads as a transformation of the response and is not one —
    // the value is unchanged and the point is entirely the side effect.
    skew::note_build(&resp);
    Ok(resp)
}

// ---------------------------------------------------------------------
// Failure text
//
// Two shapes, and the difference between them is deliberate rather than
// incidental. A READ that fails is reported with the request that failed
// (a user seeing "failed to load sessions" needs to know what was asked of
// whom), while a MUTATION that is refused surfaces the helm's own words and
// nothing else — SPEC.md wants the supervisor's "working directory does not
// exist" or the host's "is unreachable-reprobing, so this operation is
// refused" to reach the user unwrapped, not buried behind a URL.
//
// Both live here rather than being spelled out at each call site because
// every endpoint below needs one of the two, and the one thing that must
// never drift is which endpoints hand back the helm's own words unwrapped.
// What those words look like ON SCREEN is a rendering question, answered
// separately and identically for every peer string (`peer::PeerLine`):
// preserved raw here, escaped and direction-isolated where displayed.
// ---------------------------------------------------------------------

/// The message a NON-SUCCESS status produces on a read: method, URL, status,
/// and whatever the body said.
///
/// Scoped to that one case, which is worth stating because the module's
/// other two failure modes deliberately do NOT go through here and read
/// differently as a result: a transport failure never got a status to
/// report (`reqwest`'s own message stands alone), and a decode failure got a
/// 2xx whose body this build could not read (serde's message stands alone).
/// Only a status the server chose has a method/URL/status triple worth
/// printing.
///
/// A body-read failure INSIDE this path is reported WITH the
/// method/URL/status that were already known rather than swallowed: turning
/// "we do not know why this failed" into "it failed for no stated reason" is
/// strictly worse for a line the user is meant to act on.
async fn read_failure(method: &str, url: &str, resp: reqwest::Response) -> String {
    let status = resp.status();
    let detail = match resp.text().await {
        Ok(detail) => detail,
        Err(error) => return format!("{method} {url}: {status}: reading error response: {error}"),
    };
    let detail = detail.trim();
    if detail.is_empty() {
        format!("{method} {url}: {status}")
    } else {
        format!("{method} {url}: {status}: {detail}")
    }
}

/// The message a refused MUTATION produces: the helm's own body, preserved
/// as the raw value apart from surrounding whitespace.
///
/// RAW rather than "verbatim as the user sees it", and the distinction is
/// worth keeping straight now that a refusal can quote peer-supplied text
/// (an adoption superseded by a re-probe embeds the identity a host is
/// reporting): what this returns is unaltered, and the surfaces that DISPLAY
/// it escape directional and invisible characters and isolate the run
/// (`peer::PeerLine`). Nothing rewrites the message; the rendering makes it
/// unable to rearrange the sentence around it.
///
/// The trim is the one liberty taken here, and it is also what decides the
/// fallback: a refusal that carried no text at all would otherwise render as
/// an empty line, so method/URL/status stand in.
async fn refusal_text(method: &str, url: &str, resp: reqwest::Response) -> String {
    let status = resp.status();
    let detail = match resp.text().await {
        Ok(detail) => detail,
        Err(error) => return format!("{method} {url}: {status}: reading error response: {error}"),
    };
    let detail = detail.trim();
    if detail.is_empty() {
        format!("{method} {url}: {status}")
    } else {
        detail.to_string()
    }
}

/// How many pages one listing walk will follow before it stops.
///
/// A termination guarantee, not a tuning knob. The loop's continuation
/// condition is a value the SERVER supplies (`next_cursor`), so a helm that
/// is buggy, or merely racing its own session churn in a way nobody
/// anticipated, could hand back a cursor forever and park this poll in an
/// unbounded loop — every three seconds, in a browser tab. Two hundred pages
/// of the helm's 500-row default is a hundred thousand sessions, far past
/// any fleet a person manages, and stopping short is visible rather than
/// silent: the count line says how many of the total are shown.
///
/// Deliberately mirrors the helm's own `REFRESH_PAGE_LIMIT`, which bounds
/// its walk of a supervisor for exactly the same reason one layer down.
const MAX_LIST_PAGES: usize = 200;

/// How many rows one walk will accumulate before it stops.
///
/// The page ceiling alone does not bound the WORK: a helm serving a caller's
/// `?limit=` (up to its own 5,000) makes 200 pages a million rows, all of
/// them decoded, cloned into an optimistic-rename pass, and rendered as DOM.
/// This is the bound that matches what the browser actually has to survive.
///
/// Twenty thousand is far past any fleet a person manages by hand and still
/// an order of magnitude under what a tab renders comfortably — and stopping
/// here is visible rather than silent, because the count line then reads
/// "showing N of M".
const MAX_LIST_ROWS: usize = 20_000;

/// How many bytes of listing body one walk will accumulate before it stops.
///
/// The third bound, and the one the other two cannot stand in for: a session
/// carries user-supplied text with no cap this side enforces (a title may be
/// tens of KB), so a few hundred fat rows can be larger than a hundred
/// thousand lean ones. Sixteen mebibytes is generous for any honest list and
/// bounded enough that a hostile or broken helm cannot make a browser tab
/// grow without limit three seconds at a time.
///
/// Counted on the RAW body rather than on the decoded rows, because that is
/// what was actually transferred and held.
const MAX_LIST_BYTES: usize = 16 * 1024 * 1024;

/// What this reply may honestly claim matched, given what it reported and
/// what was asked for.
///
/// The substitution — an absent count answered with the fleet total — is
/// correct for an UNFILTERED request and only for one: with no filter, every
/// session matches, so `total` is not a stand-in but the same number by
/// another name. That is what makes a helm one version behind produce the
/// banner it always did.
///
/// Under an active filter the same substitution is a fabrication, and a
/// specific one: a helm that predates the matching count also predates
/// server-side filtering (both landed together — PLAN_M6_75.md item 5), so
/// it answered the filter by IGNORING it. Substituting `total` would then
/// print "700 matching of 700 sessions" over 700 unfiltered rows — the UI
/// vouching for a filter that never ran. `None` carries that ignorance to
/// the banner instead, which says so in words (`rows::count_banner`).
///
/// Absent-with-a-filter is therefore a fact about the HELM rather than about
/// the fleet, which is why the decision reads the request and not the reply
/// alone.
fn matching_count(filter_active: bool, reported: Option<u64>, total: u64) -> Option<u64> {
    reported.or_else(|| (!filter_active).then_some(total))
}

/// How long one logical WALK may spend before it stops and reports what it
/// has (`truncated`), joining [`MAX_LIST_PAGES`], [`MAX_LIST_ROWS`] and
/// [`MAX_LIST_BYTES`] as a fourth ceiling of exactly the same kind.
///
/// The three existing bounds are about SIZE, and none of them bounds time: a
/// helm answering each page just under [`REQUEST_TIMEOUT`] can hold a walk
/// for two hundred pages — hours — while every trigger behind it queues.
/// That was tolerable when a read was one request among many; it is not now
/// that a surface has ONE reader (`reader`), because the walk IS the
/// surface's only way to hear anything.
///
/// Ninety seconds is chosen against the page bounds rather than plucked: a
/// fleet large enough to walk forty pages at 500 rows each is already past
/// what this UI renders comfortably, and forty pages of a healthy helm take
/// well under a second. What the number really buys is that the worst case
/// is a minute and a half plus one request timeout — bounded, and shorter
/// than any user's patience for a page that has stopped moving — rather than
/// unbounded.
///
/// Stopping is not an error, exactly as it is not for the other three: the
/// rows collected are real and in order, so they are returned with
/// `truncated` set and the count line says "showing N of M". A read that
/// FAILED would be the wrong answer here — it would blank a list the client
/// successfully collected most of.
const MAX_LIST_MILLIS: u64 = 90_000;

/// What the walk may do next, given how long it has already spent.
///
/// A value rather than an `if` inside the loop, because the boundary is
/// exactly where this gets subtle: `>` instead of `>=` buys one more page
/// past the ceiling, and a page issued with the FULL request timeout can run
/// the walk to half again its budget. Both are invisible in review and both
/// are pinned by [`walk_step`]'s tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WalkStep {
    /// Fetch another page, with THIS timeout — the smaller of the ordinary
    /// per-request deadline and whatever is left of the walk's budget.
    Fetch { timeout_ms: u64 },
    /// The budget is spent: stop and report what has been collected.
    Stop,
}

/// Whether a walk that has spent `elapsed_ms` may fetch another page, and
/// how long that page gets.
///
/// The second half is the one a between-pages check alone gets wrong. A page
/// STARTED just under the ceiling would otherwise carry the full
/// [`REQUEST_TIMEOUT`], so a walk bounded at ninety seconds could run to a
/// hundred and fifty — the ceiling would bound when the last page is asked
/// for rather than when the walk ends, which is not what a budget is.
/// Shrinking each page's own deadline to the remaining budget makes the two
/// meanings the same.
///
/// The floor of one millisecond is deliberate: a zero timeout is not a
/// bounded request on every client, and this function's contract is "a
/// deadline", not "an immediate failure". The remaining-zero case is
/// [`WalkStep::Stop`] anyway.
fn walk_step(elapsed_ms: u64) -> WalkStep {
    let Some(remaining) = MAX_LIST_MILLIS
        .checked_sub(elapsed_ms)
        .filter(|left| *left > 0)
    else {
        return WalkStep::Stop;
    };
    WalkStep::Fetch {
        timeout_ms: remaining.min(REQUEST_TIMEOUT.as_millis() as u64).max(1),
    }
}

/// What a page request's FAILURE means, given the clock and what has already
/// been collected.
///
/// A request cut short by the walk's own budget is not an error — it is the
/// time ceiling doing its job, and the rows already collected are real. The
/// distinction matters because the two outcomes are opposites on screen: a
/// truncated listing shows the rows with "showing N of M", while an error
/// replaces a mostly-complete list with a failure line and sends the surface
/// reader off to retry a walk that will hit the same ceiling.
///
/// The exception is a walk that has collected NOTHING. There are no rows to
/// present and no counts to present them against — the first page never
/// landed — so the honest answer is the failure, which the reader then
/// retries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageFailure {
    /// Report the rows collected so far, marked truncated.
    Truncate,
    /// Surface the error.
    Fail,
}

fn page_failure(pages: usize, elapsed_ms: u64) -> PageFailure {
    if pages > 0 && elapsed_ms >= MAX_LIST_MILLIS {
        PageFailure::Truncate
    } else {
        PageFailure::Fail
    }
}

/// A monotonic-enough stopwatch for [`MAX_LIST_MILLIS`], on both renderers.
///
/// A `cfg` pair rather than a crate: `std::time::Instant::now()` panics on
/// wasm32-unknown-unknown (the browser target has no std clock), and the
/// browser's own answer — `performance.now()` — is monotonic and needs
/// nothing beyond a web-sys feature this crate already carries for `Window`.
///
/// A browser without `performance` (there is none in practice, but the API
/// is fallible) yields a clock that reports no elapsed time at all, which
/// disables the ceiling rather than failing the walk. That is the same
/// direction every other tolerance in this module leans: a missing
/// capability costs a safeguard, never a feature.
struct WalkClock {
    #[cfg(not(target_arch = "wasm32"))]
    started: std::time::Instant,
    #[cfg(target_arch = "wasm32")]
    started: Option<f64>,
}

impl WalkClock {
    fn start() -> Self {
        WalkClock {
            #[cfg(not(target_arch = "wasm32"))]
            started: std::time::Instant::now(),
            #[cfg(target_arch = "wasm32")]
            started: web_sys::window()
                .and_then(|window| window.performance())
                .map(|performance| performance.now()),
        }
    }

    fn elapsed_ms(&self) -> u64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.started.elapsed().as_millis() as u64
        }
        #[cfg(target_arch = "wasm32")]
        {
            let Some(started) = self.started else {
                return 0;
            };
            web_sys::window()
                .and_then(|window| window.performance())
                .map(|performance| (performance.now() - started).max(0.0) as u64)
                .unwrap_or(0)
        }
    }
}

/// Fetch the whole session listing, following the helm's cursor to
/// exhaustion, flattening every failure into a displayable string.
///
/// ## Why this walks rather than taking the first page
///
/// The helm's list is cursor-paginated (PLAN_M6.md item 5) and its default
/// page is 500 rows. A client that read only the first page would show a
/// FLEET's list truncated at an arbitrary boundary — and, worse, would show
/// it as though it were complete, because `total` is the merged count while
/// the rows are one page of it. Multi-host aggregation is what makes lists
/// long enough for that to happen, so the walk lands with the panel that
/// makes fleets manageable.
///
/// Each page is requested with the PREVIOUS page's `next_cursor` replayed
/// verbatim; the cursor is opaque and this UI never constructs or interprets
/// one. The helm's page-walk contract makes the resume point a KEY rather
/// than an offset, so a session created between two page fetches appears at
/// the front of a later refresh instead of tearing the walk, and one deleted
/// between them simply is not there.
///
/// An EMPTY page with a cursor is legitimate and must not stop the walk: the
/// helm skips rows whose cached metadata no longer decodes while still
/// advancing past them, so a page can carry nothing and still have more
/// behind it. That is why the loop is driven by the cursor alone.
///
/// ## The order is the helm's
///
/// No client-side sort. The helm serves creation-time-descending with
/// stable tiebreaks, and its cursor walks that exact order — so re-sorting
/// the rows here (this used to sort by session id) would scramble the pages
/// back together in an order no cursor agrees with, making the boundary
/// between page 1 and page 2 land in the middle of the list.
///
/// ## Three bounds, and what hitting one means
///
/// [`MAX_LIST_PAGES`], [`MAX_LIST_ROWS`], [`MAX_LIST_BYTES`] and
/// [`MAX_LIST_MILLIS`] each stop the walk independently, because each bounds
/// a different resource and none implies the others. Stopping is not an
/// error: the rows collected are real and in order, so they are returned
/// with `truncated` set, and the count line's "showing N of M" is the
/// continuation story a user reads. Failing the whole poll instead would
/// replace a partial list with no list at all, which is strictly worse for a
/// surface the feed refreshes on every change.
///
/// The time ceiling is the one that also reaches INSIDE a page: each request
/// is issued with the smaller of its own timeout and the walk's remaining
/// budget (`walk_step`), and a page cut short by that budget is truncation
/// rather than failure (`page_failure`). Checking only between pages would
/// bound when the last page is ASKED FOR rather than when the walk ends,
/// which is not a budget at all.
///
/// The message on failure is a `String`, not `reqwest::Error`, because it is
/// rendered to the user directly (SPEC.md wants concrete errors) — the URL
/// and status are folded into the message here rather than logged and
/// dropped.
///
/// ## The filter travels on every page
///
/// `filter` is appended to each request, cursor pages included, because a
/// cursor names a position in the helm's ORDER rather than in a result set:
/// a walk whose later pages dropped the filter would quietly start listing
/// rows its first page had excluded. The helm makes the same statement from
/// its side (`ListQuery::cursor`), and this is the client half of it.
pub(crate) async fn fetch_sessions(
    base: &str,
    filter: &SessionFilter,
) -> Result<SessionListing, String> {
    let mut sessions: Vec<Session> = Vec::new();
    let mut cursor: Option<String> = None;
    // The LAST page's counts win — they are the freshest read of numbers that
    // can legitimately change under a walk — and they live in an `Option` so
    // that a walk which stopped before any page landed cannot publish counts
    // no page ever reported. The ceiling breaks below cannot be taken that
    // early (`walk_step` never stops a walk that has spent no time, and
    // `page_failure` refuses to truncate an empty walk), and this is how that
    // reasoning is enforced rather than merely believed.
    let mut counts: Option<(u64, Option<u64>, bool)> = None;
    // Whether one of OUR ceilings ended the walk, as opposed to the helm
    // running out of pages.
    let mut hit_ceiling = false;
    let mut pages = 0;
    let mut bytes = 0_usize;
    // Started before the first request, so the ceiling covers the whole
    // logical read rather than each page's share of it (see
    // `MAX_LIST_MILLIS`).
    let clock = WalkClock::start();
    let query = filter.query();
    // Hoisted: the filter cannot change under a walk (it is a snapshot the
    // caller took before the first request), so asking it once per page was
    // asking the same question over and over.
    let filtered = filter.is_active();
    loop {
        let url = match (&cursor, query.is_empty()) {
            (None, true) => format!("{base}/api/sessions"),
            (None, false) => format!("{base}/api/sessions?{query}"),
            (Some(cursor), true) => {
                format!("{base}/api/sessions?cursor={}", encode_query_value(cursor))
            }
            (Some(cursor), false) => format!(
                "{base}/api/sessions?{query}&cursor={}",
                encode_query_value(cursor)
            ),
        };
        // Each page is asked for under the SMALLER of its own timeout and
        // what is left of the walk's budget, so the ceiling bounds when the
        // walk ENDS rather than when its last page is requested.
        let WalkStep::Fetch { timeout_ms } = walk_step(clock.elapsed_ms()) else {
            // Only reachable when the budget ran out inside the previous
            // page's own request; the post-page check below catches the
            // ordinary case first.
            hit_ceiling = true;
            break;
        };
        let fetched = send_within(
            client().get(&url),
            std::time::Duration::from_millis(timeout_ms),
        )
        .await;
        let resp = match fetched {
            Ok(resp) => resp,
            Err(reason) => match page_failure(pages, clock.elapsed_ms()) {
                // The budget expired mid-page. The rows already collected are
                // real and in order, so this is the time ceiling stopping the
                // walk rather than a failure to report — see `page_failure`.
                PageFailure::Truncate => {
                    hit_ceiling = true;
                    break;
                }
                PageFailure::Fail => return Err(reason),
            },
        };
        if !resp.status().is_success() {
            return Err(read_failure("GET", &url, resp).await);
        }
        // Read as text and decode from it, rather than `resp.json()`, so the
        // byte ceiling counts what was actually transferred. Decoding stays
        // strict either way.
        let body = resp.text().await.map_err(|e| e.to_string())?;
        bytes = bytes.saturating_add(body.len());
        let page = serde_json::from_str::<SessionPage>(&body).map_err(|e| e.to_string())?;
        sessions.extend(page.sessions);
        counts = Some((
            page.total,
            matching_count(filtered, page.matching, page.total),
            page.truncated,
        ));
        pages += 1;
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
        // Stopped short by one of OUR ceilings rather than by the helm. Each
        // is checked after taking the page, so a walk always makes progress
        // and never spins on a bound it has already exceeded — the time
        // ceiling included, which is why a slow helm always yields at least
        // one page rather than an empty truncated list.
        if pages >= MAX_LIST_PAGES
            || sessions.len() >= MAX_LIST_ROWS
            || bytes >= MAX_LIST_BYTES
            || clock.elapsed_ms() >= MAX_LIST_MILLIS
        {
            hit_ceiling = true;
            break;
        }
    }
    // Unreachable by construction (see `counts`), and stated as an error
    // rather than an `unwrap` because the honest answer for a walk with no
    // pages IS a failure: there are no rows to show and no counts to show
    // them against.
    let (total, matching, page_truncated) = counts.ok_or_else(|| {
        format!("the session list walk at {base} ended before its first page arrived")
    })?;
    let truncated = page_truncated || hit_ceiling;
    // A walk that collected MORE rows than the helm says exist is
    // incoherent: the list changed underneath it in a way that makes
    // "complete" unprovable — a session deleted from an earlier page shrinks
    // `total` while the rows already taken stay taken, and a cursor replayed
    // across such a change can re-serve a row. Presenting that as a finished
    // walk would claim a completeness nothing supports, so it is reported as
    // truncated and the next read settles it — which, since whatever changed
    // the list also bumped the helm's revision, is one the feed is already
    // on its way to asking for.
    // Checked against the FLEET total rather than against `matching`, and
    // that is the helm's own reading of the two numbers (PLAN_M6_75.md item
    // 5): a filtered page holding fewer rows than the fleet is not an
    // incoherent list, it is a working filter, so the check stays against
    // the count it has always described.
    let incoherent = sessions.len() as u64 > total;
    Ok(SessionListing {
        sessions,
        total,
        matching,
        filtered,
        truncated: truncated || incoherent,
        incoherent,
    })
}

/// How often the FALLBACK polls refetch (PLAN_M6_75.md item 6).
///
/// This was the cadence of four periodic loops — the session list, the hosts
/// panel, a session's detail, and the session view's host-state read — and
/// M6.75 removed all four. What survives is a fallback and nothing else:
/// `feed::fallback_polls` runs a read at this cadence exactly while the
/// invalidation feed is unhealthy AND no build mismatch has been latched.
/// Under skew it does not run at all, which is the point of the withdrawal
/// rule rather than an omission.
///
/// Still one constant rather than one per surface, and still for the reason
/// it always was: every fallback reads the same helm through the same feed's
/// absence, so a divergence here would be a difference nobody chose. Three
/// seconds is unchanged deliberately — it is the interval this UI ran on for
/// four milestones, and a fallback is exactly the wrong place to introduce
/// an untested cadence.
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
///
/// `host` names which registered host to create on (PLAN_M6.md item 5),
/// carrying SPEC.md's default — the host of the currently open session, else
/// the helm's own — as a value the CLIENT decided, because only the client
/// knows what the user is looking at. `None` leaves the helm to fall back to
/// its local row, which is what every hand-written caller means on a
/// single-machine setup. A host that is not connected fails the create as a
/// precondition, and that refusal arrives here as the helm's own words like
/// any other.
pub(crate) async fn create_session(
    base: &str,
    cwd: &str,
    invocation: &str,
    title: &str,
    intent_key: &str,
    host: Option<HostId>,
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
        "host": host,
    });
    let resp = send(client().post(&url).json(&body)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
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
    let resp = send(client().post(&url)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
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
    let resp = send(client().get(&url)).await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(read_failure("GET", &url, resp).await);
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
    let resp = send(client().post(&url).json(&body)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    resp.json::<Session>().await.map_err(|e| e.to_string())
}

/// POST the rename endpoint for one session, returning the session as the
/// supervisor now describes it (SPEC.md's rename verb; PLAN_M5.md item 6).
///
/// `title` goes on the wire EXACTLY as the user typed it — no trimming, no
/// emptiness check, no control-character screening, deliberately unlike
/// `create_session` above (which trims only to decide between "auto-
/// generate" and an explicit title, a distinction rename does not have:
/// there is no "un-name this session", and an explicit empty title is a
/// legal rename). Three reasons, all pointing the same way: the
/// supervisor's refusal text is the contract a user acts on, a second copy
/// of its rules here would drift from it, and rewriting caller data before
/// sending it is the exact move the supervisor itself refuses to make.
///
/// The reply is the session's freshly recomputed state, not an ack — the
/// supervisor re-probes status and rediscovers tabs while building it — so
/// a caller can paint the new title from this answer instead of waiting for
/// its next read. Same error-surfacing shape as `restart_session`: the
/// supervisor's own words, which for the control-character refusal is the
/// whole point of the feature.
pub(crate) async fn rename_session(base: &str, id: &str, title: &str) -> Result<Session, String> {
    let url = format!("{base}/api/sessions/{}/rename", encode_path_segment(id));
    let body = serde_json::json!({ "title": title });
    let resp = send(client().post(&url).json(&body)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    resp.json::<Session>().await.map_err(|e| e.to_string())
}

/// DELETE a session. See `stop_session`'s docs — same error-surfacing
/// shape (including the body-read-failure context), different verb and
/// endpoint.
pub(crate) async fn delete_session(base: &str, id: &str) -> Result<(), String> {
    let url = format!("{base}/api/sessions/{}", encode_path_segment(id));
    let resp = send(client().delete(&url)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("DELETE", &url, resp).await);
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
/// individually reversible — the next detail read lists both tabs, and
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
    let resp = send(client().post(&url)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
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
/// the read that follows reconciles the list either way.
pub(crate) async fn close_tab(base: &str, session_id: &str, tab_id: &str) -> Result<(), String> {
    let url = format!(
        "{base}/api/sessions/{}/tabs/{}",
        encode_path_segment(session_id),
        encode_path_segment(tab_id)
    );
    let resp = send(client().delete(&url)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("DELETE", &url, resp).await);
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Host management (PLAN_M6.md item 5's REST surface, item 6's consumer)
//
// Five mutations and one read, mirroring the helm's `/api/hosts` routes
// one-for-one. Every mutation surfaces the helm's own words on refusal,
// exactly as the session verbs do — and here that matters more than
// anywhere else in this file, because each host refusal is a DIFFERENT
// remedy: an address already registered, a row that cannot be edited at
// all, an adoption superseded by a re-probe. A generic "could not add
// host" would leave the user with no way to tell them apart.
// ---------------------------------------------------------------------

/// Mirror of `GET /api/hosts`'s body: `{"hosts": [...]}`.
///
/// `hosts` is REQUIRED. Defaulting it — which this briefly did, reasoning
/// from the panel's never-blank rule — gets that rule exactly backwards: a
/// body with no `hosts` key is a malformed reply, and decoding it as an
/// empty fleet would render "no hosts at all" on a helm that always has at
/// least its own local row. That is a fabricated claim, and a far more
/// alarming one than the honest alternative. Failing here makes it a failed
/// READ instead, which `hosts::HostsRead` reports while keeping the last
/// snapshot it trusts.
#[derive(Deserialize)]
struct HostListing {
    hosts: Vec<Host>,
}

/// Fetch the whole host registry with each host's live connection state.
///
/// Never paginated, because the helm does not paginate it: SPEC.md's promise
/// is that per-host connection state is ALWAYS visible, and a fleet whose
/// host count needed paging is not one a person manages by hand.
pub(crate) async fn fetch_hosts(base: &str) -> Result<Vec<Host>, String> {
    let url = format!("{base}/api/hosts");
    let resp = send(client().get(&url)).await?;
    if !resp.status().is_success() {
        return Err(read_failure("GET", &url, resp).await);
    }
    resp.json::<HostListing>()
        .await
        .map(|listing| listing.hosts)
        .map_err(|e| e.to_string())
}

/// One optional install field as it goes on the wire: absent when the user
/// left it blank, and otherwise exactly what they typed.
///
/// The trim decides PRESENCE only. It is tempting to trim the value too —
/// a stray trailing space in a path field looks like a typo — but these are
/// paths on a machine this UI cannot see, a path may legally contain leading
/// or trailing spaces, and an entry silently dialing a path the user did not
/// type fails in a way nothing on screen explains.
fn install_field(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

/// How a mutation that the helm ACCEPTED came back.
///
/// The distinction exists because a 2xx whose body this build cannot decode
/// is not a refusal, and treating it as one — which the two host mutations
/// did — tells the user their change did not happen when it demonstrably
/// did. The registry row was written; only the description of it was
/// unreadable. Everything downstream of that difference is different: the
/// form closes, the authoritative hosts refresh fires, and the decode
/// problem is reported as a warning about this CLIENT rather than as the
/// helm rejecting something.
///
/// Decoding stays strict (see `HostPhase`'s own docs on why a fabricated
/// field is worse than a failed read). This changes what a failed decode
/// MEANS on a path that has already committed, not how hard it is to fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Commit {
    /// Accepted, and the reply described the result.
    Confirmed,
    /// Accepted, but the reply could not be read by this build. Carries the
    /// decode failure, for a warning line.
    Unvalidated(String),
}

/// Classify a successful response by whether its body decoded.
///
/// Shared by the two mutations that answer with a host row so both make the
/// same call about the same situation — the alternative being one of them
/// quietly reverting to "a bad body is a refusal" the next time it is
/// touched.
async fn commit_of<T: serde::de::DeserializeOwned>(resp: reqwest::Response) -> Commit {
    match resp.json::<T>().await {
        Ok(_) => Commit::Confirmed,
        Err(error) => Commit::Unvalidated(format!(
            "the helm accepted this change, but its reply could not be read by this build \
             ({error}); the hosts list below is the authoritative view of what happened"
        )),
    }
}

/// Register an ssh destination (`POST /api/hosts`).
///
/// Adding ALWAYS registers when the destination is usable at all: whether
/// anything answered is the new host's connection STATE, not this request's
/// status. So a caller must not read `Ok` as "the host is up" — the panel
/// that renders the returned row is where "there was nothing there" shows
/// up, as an ordinary connecting-then-unreachable chip.
///
/// The two optional fields describe the INSTALL rather than the address:
/// where the remote farhelm binary lives and which state directory it
/// serves. A blank field is sent as ABSENT rather than as an empty value —
/// the helm would take `Some("")` literally and dial a binary named nothing
/// — and a field with anything in it is sent BYTE FOR BYTE.
///
/// The distinction matters more than it looks: trimming decides only
/// whether the field is present, never what it contains. These are
/// filesystem paths on someone else's machine, and a path may legally begin
/// or end with a space — trimming one away produces an entry that dials a
/// path the user did not type and cannot see the difference in. The same
/// posture the create dialog takes with a working directory, for the same
/// reason.
///
/// A 2xx whose body will not decode is [`Commit::Unvalidated`], not an
/// error: the row was written (see [`Commit`]).
pub(crate) async fn add_host(
    base: &str,
    ssh: &str,
    remote_farhelm: &str,
    remote_state_dir: &str,
) -> Result<Commit, String> {
    let url = format!("{base}/api/hosts");
    let body = serde_json::json!({
        "ssh": ssh,
        "remote_farhelm": install_field(remote_farhelm),
        "remote_state_dir": install_field(remote_state_dir),
    });
    let resp = send(client().post(&url).json(&body)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    Ok(commit_of::<Host>(resp).await)
}

/// Retarget a registered host (`POST /api/hosts/{id}/destination`).
///
/// The body carries only `ssh`: the remote binary and state directory
/// describe the install, not the address it is reached at, so the helm
/// deliberately keeps them across a retarget. Refused for the reserved local
/// row, which has no destination to change.
///
/// A 2xx whose body will not decode is [`Commit::Unvalidated`], not an
/// error: the retarget was written and the actor re-dialled (see [`Commit`]).
pub(crate) async fn set_host_destination(
    base: &str,
    host: HostId,
    ssh: &str,
) -> Result<Commit, String> {
    let url = format!("{base}/api/hosts/{host}/destination");
    let resp = send(client().post(&url).json(&serde_json::json!({ "ssh": ssh }))).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    // Decoded and discarded: the reply's `Host` validates the contract shape
    // and nothing more, because the caller repaints from the
    // generation-disciplined hosts refresh rather than from this one-off
    // snapshot.
    Ok(commit_of::<Host>(resp).await)
}

/// Forget a registered host (`DELETE /api/hosts/{id}`).
///
/// SPEC.md's remove-merely-forgets contract: the registry row and the
/// helm's cached sessions for it go, while the host itself — its supervisor,
/// its running agents — is untouched, and re-adding the same destination
/// later rediscovers all of it. That is why the panel's confirmation says
/// "forget", not "delete".
pub(crate) async fn remove_host(base: &str, host: HostId) -> Result<(), String> {
    let url = format!("{base}/api/hosts/{host}");
    let resp = send(client().delete(&url)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("DELETE", &url, resp).await);
    }
    Ok(())
}

/// Accept the identity a mismatched host is reporting
/// (`POST /api/hosts/{id}/adopt`).
///
/// `reported` is not a formality and must be the value the user was SHOWN —
/// the `reported` field of the very `HostPhase::IdentityMismatch` the adopt
/// control was rendered from. The helm compares it under its own lock and
/// answers 409 if the host has since started reporting something else, which
/// is what turns "a re-probe landed between the prompt and the click" from a
/// silent adoption of a third install into a refusal the user answers by
/// looking again. Sending whatever is current at request time would defeat
/// the entire check, so this argument is never derived from a fresh read.
///
/// Deliberately has no counterpart for `identity-unverified`: there is
/// nothing to adopt there, and the helm refuses it (see that phase's docs).
pub(crate) async fn adopt_host(base: &str, host: HostId, reported: &str) -> Result<(), String> {
    let url = format!("{base}/api/hosts/{host}/adopt");
    let resp = send(
        client()
            .post(&url)
            .json(&serde_json::json!({ "reported": reported })),
    )
    .await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    Ok(())
}

/// Reconnect one host now (`POST /api/hosts/{id}/retry`).
///
/// One attempt, not a fresh retry ladder — a click is not evidence the host
/// is back, and the two-regime cadence exists to avoid hammering. TWO cases
/// are exceptions, and both are exceptions because there is no attempt in
/// progress for one more to be added to:
///
/// - A RETIRED host has no actor at all, so retry respawns it from the
///   current registry row and the fresh actor starts on the full ladder.
///   That is what makes this control load-bearing rather than a
///   convenience: nothing else ever restarts a dead actor.
/// - A FROZEN host — either identity state — is not attempting anything
///   either, so resolving the freeze earns the ladder from the actor rather
///   than spending a single attempt (farhelm-helm's `retry_now`:
///   "resolving a freeze still earns the ladder").
pub(crate) async fn retry_host(base: &str, host: HostId) -> Result<(), String> {
    let url = format!("{base}/api/hosts/{host}/retry");
    let resp = send(client().post(&url)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
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

    /// A blank install field must reach the wire as ABSENT, and a non-blank
    /// one byte for byte.
    ///
    /// Both halves are real failures. `Some("")` is taken literally by the
    /// helm — a host registered to dial a binary named nothing, which then
    /// never connects for a reason no chip can explain. And a trimmed value
    /// is a path the user did not type: paths may legally begin or end with
    /// a space, and the entry would dial one thing while the form shows
    /// another.
    #[test]
    fn blank_install_fields_are_absent_while_others_survive_verbatim() {
        assert_eq!(install_field(""), None);
        assert_eq!(install_field("   "), None);
        assert_eq!(install_field("\t\n"), None);
        assert_eq!(
            install_field(" /opt/with space/farhelm "),
            Some(" /opt/with space/farhelm ".to_string()),
            "trimming decides presence, never content"
        );
        assert_eq!(install_field("/srv/state"), Some("/srv/state".to_string()));
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

    /// The other half of `SessionPage`'s missing-field tolerance
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
    /// object shape: a later field added to `SessionPage` must still
    /// let a build one step behind decode today's object, the same
    /// tolerance farhelm-proto's own wire types carry.
    ///
    /// `next_cursor` is covered by the same assertion for a different
    /// reason: absent IS its meaningful value ("this was the last page"), so
    /// a body without it must decode as a terminated walk rather than fail.
    #[test]
    fn session_page_without_total_or_truncated_defaults_both() {
        let json = serde_json::json!({ "sessions": [] });
        let decoded: SessionPage = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.total, 0);
        assert!(!decoded.truncated);
        assert_eq!(
            decoded.next_cursor, None,
            "no cursor is how a walk learns it has reached the end"
        );
        assert_eq!(
            decoded.matching, None,
            "an absent matching count must stay absent rather than becoming a zero the banner \
             would then print over a list full of rows"
        );
    }

    /// The walk's time ceiling, at the boundary and inside a page.
    ///
    /// Both halves have a wrong version that looks right in review: `>`
    /// instead of `>=` buys one extra page every single walk, and issuing
    /// that page with the full request timeout lets a ninety-second budget
    /// run to a hundred and fifty seconds — with the surface's single reader
    /// blocked for all of it.
    #[test]
    fn the_walk_stops_at_its_time_ceiling_rather_than_one_page_past_it() {
        // The clock is a PARAMETER here, which is the only way this boundary
        // can be asserted at all: the alternative is a test that waits ninety
        // seconds to find out whether the comparison was `>` or `>=`.
        assert_eq!(
            walk_step(MAX_LIST_MILLIS - 1),
            WalkStep::Fetch { timeout_ms: 1 },
            "one millisecond under the ceiling still buys a page — with one millisecond to run it"
        );
        assert_eq!(
            walk_step(MAX_LIST_MILLIS),
            WalkStep::Stop,
            "and AT the ceiling the walk stops: `>` here would fetch one more page, every time"
        );
        assert_eq!(
            walk_step(MAX_LIST_MILLIS * 2),
            WalkStep::Stop,
            "well past it, too — the budget does not wrap"
        );

        // Early in a walk the page gets the ordinary request deadline; the
        // budget only starts biting once less of it remains than that.
        let full = REQUEST_TIMEOUT.as_millis() as u64;
        assert_eq!(walk_step(0), WalkStep::Fetch { timeout_ms: full });
        assert_eq!(
            walk_step(MAX_LIST_MILLIS - full),
            WalkStep::Fetch { timeout_ms: full },
            "exactly one request's worth left is still a full request"
        );
        assert_eq!(
            walk_step(MAX_LIST_MILLIS - full + 1),
            WalkStep::Fetch {
                timeout_ms: full - 1
            },
            "past that the page is cut to what is left, or the walk would overrun its budget by \
             nearly a full request timeout"
        );
    }

    /// A page request that fails after the budget is spent is TRUNCATION, not
    /// an error — unless nothing was collected at all.
    ///
    /// The distinction decides what the user sees: rows plus "showing N of
    /// M", or a failure line where a mostly-complete list should be, followed
    /// by a reader retrying a walk that will hit the same ceiling. The empty
    /// case is the exception because there is nothing to show and no counts
    /// to show it against — the first page never landed.
    #[test]
    fn a_page_cut_short_by_the_budget_truncates_only_once_something_was_collected() {
        assert_eq!(
            page_failure(3, MAX_LIST_MILLIS),
            PageFailure::Truncate,
            "three pages in hand and the budget spent: report them"
        );
        assert_eq!(
            page_failure(3, MAX_LIST_MILLIS - 1),
            PageFailure::Fail,
            "a failure with budget left is a real failure, not a ceiling"
        );
        assert_eq!(
            page_failure(0, MAX_LIST_MILLIS * 2),
            PageFailure::Fail,
            "an empty walk has nothing to truncate, so the reader is told the read failed"
        );
    }

    /// The two-count reply (PLAN_M6_75.md item 5), and what a reply carrying
    /// only one of them may be turned into.
    ///
    /// Routed through `matching_count` rather than through an `unwrap_or`
    /// written out in the assertion, which is the whole point: an assertion
    /// that repeats the conversion it is checking passes for every possible
    /// conversion, including the one this test exists to forbid. What must
    /// hold is that absence becomes a CLAIM only where the claim is true by
    /// construction — no filter, so everything matched — and stays an
    /// absence under a filter, where the substituted number would vouch for
    /// a filter that never ran.
    #[test]
    fn an_absent_matching_count_becomes_a_number_only_where_it_cannot_lie() {
        let filtered: SessionPage = serde_json::from_value(serde_json::json!({
            "sessions": [], "total": 700, "matching": 12, "truncated": false,
        }))
        .unwrap();
        assert_eq!(filtered.matching, Some(12));
        assert_eq!(
            matching_count(true, filtered.matching, filtered.total),
            Some(12),
            "a helm that answered the filter is believed"
        );

        let older: SessionPage = serde_json::from_value(serde_json::json!({
            "sessions": [], "total": 700, "truncated": false,
        }))
        .unwrap();
        assert_eq!(
            matching_count(false, older.matching, older.total),
            Some(700),
            "with no filter, the fleet total IS the matching count"
        );
        assert_eq!(
            matching_count(true, older.matching, older.total),
            None,
            "a helm that predates the count also predates filtering, so it matched nothing it \
             can be quoted on"
        );
    }

    /// An empty filter must produce an empty query string — a request byte
    /// for byte identical to the unfiltered one this UI has always sent.
    ///
    /// Not tidiness: the helm caps a FILTERED page lower than an unfiltered
    /// one and counts a request as filtered by what it carries, so a filter
    /// that sent `?title=` for a cleared search box would narrow nothing
    /// while paying the filtered path's ceiling — and would make the banner
    /// claim a filter is active when none is.
    #[test]
    fn an_empty_filter_asks_for_exactly_what_an_unfiltered_walk_asks_for() {
        let empty = SessionFilter::default();
        assert!(!empty.is_active());
        assert_eq!(empty.query(), "");
    }

    /// Each dimension reaches the wire under the helm's own parameter name,
    /// and every value is encoded rather than pasted.
    ///
    /// The encoding is the load-bearing half: an unescaped `&` in a title
    /// search would split into a second parameter, and a search for
    /// `a&status=exited` would silently become a status filter the user
    /// never asked for.
    #[test]
    fn every_filter_dimension_travels_under_its_own_encoded_parameter() {
        let filter = SessionFilter {
            host: Some(7),
            directory: "/srv/my project".to_string(),
            profile: "claude code".to_string(),
            status: "waiting".to_string(),
            title: "a&b".to_string(),
        };
        assert!(filter.is_active());
        assert_eq!(
            filter.query(),
            "host=7&directory=%2Fsrv%2Fmy%20project&profile=claude%20code&status=waiting&\
             title=a%26b"
        );
    }

    /// Only the EXACTLY-empty value clears a dimension; whitespace is a
    /// search like any other.
    ///
    /// The helm draws the line in the same place, and both sides have to for
    /// the same user-visible reason: a directory may legitimately contain
    /// surrounding spaces, so trimming would make what is actually there
    /// unfindable — and would turn typing a single space into a silent
    /// clear, which is the one version of this a user can watch happen.
    #[test]
    fn only_an_exactly_empty_value_clears_a_dimension() {
        let blank = SessionFilter {
            title: String::new(),
            ..SessionFilter::default()
        };
        assert!(!blank.is_active(), "an empty box filters nothing");

        let spaced = SessionFilter {
            title: " ".to_string(),
            ..SessionFilter::default()
        };
        assert!(spaced.is_active());
        assert_eq!(
            spaced.query(),
            "title=%20",
            "a space is a search for a space, not a cleared filter"
        );

        // And the host dimension, whose emptiness is an absent id rather
        // than an empty string — including id 0, which is a value and not a
        // blank.
        assert!(
            SessionFilter {
                host: Some(0),
                ..SessionFilter::default()
            }
            .is_active()
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
}
