//! The helm's HTTP contract, as this UI speaks it: one `async fn` per
//! endpoint — the session routes
//! (fetch/create/stop/restart/rename/delete/tab-open/tab-close), the host
//! registry's (list/probe/provision/update/retarget/remove/adopt/retry) since
//! PLAN_M7.md item 7 extended the M6 host surface, and the helm-wide profile catalog's
//! (list/create/update/delete) since PLAN_M6_75.md item 5 did the same —
//! each exposing every failure — transport, status, or body-read — as a
//! single displayable `String`. Internally, the shared send funnel keeps a
//! typed distinction just long enough to separate the authentication
//! middleware's global state transition from an operation's own failure, and
//! its callers explicitly flatten both variants at the public boundary. That
//! flattening is deliberate, not laziness:
//! every caller (`list::ListView`, `list::CreateSessionForm`,
//! `hosts::HostsPanel`, `session_view::SessionView`) renders the message
//! directly to the user per SPEC.md's "concrete, actionable errors", so
//! there is no second consumer that would ever want a structured error to
//! match on.
//!
//! `SessionListBody`/`SessionListing`, `SessionFilter`, `POLL_INTERVAL_MS`,
//! and `restart_mode_for` live here too, even though none of them performs
//! I/O directly: the first pair is this module's own decoded response shape
//! and the listing it assembles from it, `SessionFilter` is the query surface's
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
//! One endpoint deliberately breaks the one-async-fn-per-endpoint,
//! error-as-displayable-string mold: the shared preference
//! (`fetch_preferences`, `store_preference`, and the serialized write queue
//! behind it). Its writes are fire-and-forget — SPEC.md's Errors and
//! diagnostics makes preference persistence the one best-effort exception,
//! so failures are logged here and never surfaced — and its reader is
//! `lib.rs`'s `PreferencesGate` rather than a view component.
//!
//! Every protected request below is issued through this module's own [`send`],
//! which attaches the device secret and reads the helm's build stamp off the
//! reply (`skew`). The sole exception is [`exchange_token`]: it is the public
//! bootstrap request that obtains that credential, so it cannot traverse an
//! authenticated funnel. It performs the same build-stamp classification
//! explicitly.
//!
//! The cross-module entry points are `pub(crate)`: they exist to be
//! called from the view components in `list`, `session_view`, and
//! `tabs`, never from outside this crate — `main.rs` only ever reaches
//! `App`/`ApiBase` (see `lib.rs`). Internal helpers (`client`, `send`,
//! `encode_bytes`, `install_field`, `eval_minted_id`, and the two
//! failure-text builders) stay private to this module.

use crate::skew;
use crate::{Host, HostId, Profile, RestartOffer, Session, Tab};
use serde::{Deserialize, Serialize};

/// Mirror of the helm's whole `GET /api/sessions` reply (farhelm-helm's
/// `SessionListBody`): `{"sessions": [...], "total": N, "matching": N,
/// "truncated": bool}` — one object, never a page. There is no cursor to
/// follow and no page size to ask for, by contract (SPEC.md's Session list
/// section); a helm that cannot fit the view under its cap says so with
/// `truncated`, and that flag is the only continuation story there is.
///
/// `total`/`truncated` keep `#[serde(default)]` for the same old-peer
/// tolerance as `Session::status`.
///
/// Private, unlike [`SessionListing`]: the request's own facts (what was
/// filtered, what it may be read as evidence about) are folded in by
/// [`fetch_sessions`], and no caller wants the bare wire object.
#[derive(Deserialize)]
struct SessionListBody {
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
    /// the client may honestly do with that depends on the REQUEST — see
    /// [`matching_count`].
    matching: Option<u64>,
    #[serde(default)]
    truncated: bool,
}

/// The session list's query surface: SPEC.md's dimensions, as the values a
/// user typed or chose.
///
/// Filtering is a QUERY, not a render pass. Every field here becomes a
/// parameter on `GET /api/sessions` and the helm answers with the matching
/// rows plus their count — which is the only arrangement that stays honest
/// under the helm's cap. A client filtering a list the cap had cut would
/// hide matches beyond the cut while reporting a count that included them,
/// and the count is what the banner says out loud.
///
/// Strings rather than `Option<String>` because a text field's empty value
/// IS its absent value, and the helm agrees — an exactly-empty parameter is
/// treated as absent there, which is what makes clearing a search box widen
/// the list instead of erroring.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SessionFilter {
    /// Whether the list includes archived sessions. False is the ordinary
    /// view and therefore still an active server-side predicate.
    pub(crate) include_archived: bool,
    /// A registered host's id, from `GET /api/hosts`.
    pub(crate) host: Option<HostId>,
    /// The exact session id whose direct children should be listed.
    pub(crate) parent: String,
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
    /// Whether a reply to this request MAY leave some of the fleet out — the
    /// EVIDENCE predicate.
    ///
    /// A statement about the request's SHAPE, not a detection of anything
    /// that actually went missing: it answers yes for any request the helm
    /// is allowed to answer with less than the whole fleet, whether or not
    /// this particular reply did. That is the only useful reading, because
    /// the reconciliations it gates — retiring an optimistic rename, closing
    /// a rename editor, dropping a delete confirmation — act on sessions
    /// that are NOT in the reply, and nothing in a reply can say why
    /// something is missing from it.
    ///
    /// The DEFAULT view answers yes, and must: it hides archived sessions,
    /// so an archived session missing from the rows has not left the fleet.
    ///
    /// Deliberately NOT the banner's question. See
    /// [`Self::narrows_beyond_archive`] for why the two diverge, and note
    /// which way each errs — this one is the conservative half, so when in
    /// doubt a caller wants this one.
    pub(crate) fn omits_fleet_members(&self) -> bool {
        self != &SessionFilter {
            include_archived: true,
            ..SessionFilter::default()
        }
    }

    /// Whether the USER narrowed this listing — the BANNER predicate.
    ///
    /// True for a filter the user applied (host, parent, directory, profile,
    /// status, title) and false for the archive switch in either position.
    /// That is what makes the ordinary list say "12 sessions" rather than "12
    /// matching of 12 sessions": with nothing typed there is no filter to
    /// report, and the helm now counts the same view the rows come from
    /// (`SessionListBody::total` there, [`SessionListing::total`] here), so
    /// the two numbers no longer need a sentence explaining why they differ.
    ///
    /// Turning the archive switch ON is not a narrowing either — it WIDENS
    /// the view, and the total widens with it — so it too keeps the
    /// unfiltered wording.
    ///
    /// This is the weaker of the pair by construction: every filter it
    /// reports is also one [`Self::omits_fleet_members`] reports, and the
    /// default view is the gap between them. Using this one to decide what a
    /// reply is evidence about would read an archived session's absence as a
    /// departure.
    pub(crate) fn narrows_beyond_archive(&self) -> bool {
        self != &SessionFilter {
            include_archived: self.include_archived,
            ..SessionFilter::default()
        }
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
        if self.include_archived {
            parts.push("include_archived=true".to_string());
        }
        if let Some(host) = self.host {
            parts.push(format!("host={host}"));
        }
        for (name, value) in [
            ("parent", &self.parent),
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

/// The order a listing is asked for — SPEC.md's three, in the helm's own
/// `?sort=` vocabulary.
///
/// A value of its own rather than a field on [`SessionFilter`], and the
/// split is the helm's (`farhelm_helm::store::ListSort` makes the same
/// statement from its side): a filter decides WHICH sessions a listing
/// holds, an order decides in what sequence they arrive. Folded together,
/// re-sorting would look like re-filtering — the count banner would announce
/// a filter nobody applied, and the evidence predicates
/// ([`SessionFilter::omits_fleet_members`]) would answer for a dimension
/// that cannot change what a reply covers.
///
/// The default here is deliberately NOT the wire's. A request naming no
/// order gets `created`, which is what every client written before there was
/// a choice keeps getting; this UI names one on EVERY read, and the one it
/// names before the user has expressed a preference is [`ListSort::Activity`]
/// — a list someone opens wants the sessions they were last working in at
/// the top. Nothing here reads the helm's default, so the two differ without
/// either being wrong.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ListSort {
    /// Effective activity descending: the session last observed producing
    /// output first, with one that has produced none sorting by its creation
    /// time rather than piling up at the epoch (the helm's
    /// `SessionInfo::effective_activity`).
    #[default]
    Activity,
    /// Creation time descending — the order this list had before there was a
    /// choice, and still the one the helm serves a request that names none.
    Created,
    /// Title ascending, case-insensitively, under the helm's own collation.
    Title,
}

impl ListSort {
    /// The word this order travels as, and the exact inverse of
    /// [`Self::from_key`].
    ///
    /// The helm's spelling rather than a translation of it: an unrecognized
    /// `sort` is a 400 there, so a private vocabulary on this side would
    /// produce a listing that FAILS rather than one that sorts differently.
    pub(crate) fn key(self) -> &'static str {
        match self {
            ListSort::Activity => "activity",
            ListSort::Created => "created",
            ListSort::Title => "title",
        }
    }

    /// One of the three words, or `None` for anything else.
    ///
    /// `None` is what the helm-stored preference decodes to when the row was
    /// written by a build with a different sort vocabulary — the row outlives
    /// the build that validated it. The PERSISTED-PREFERENCE decoder
    /// (`list::view::decoded_sort`) answers that with the default rather than
    /// with an error: a stored preference is a convenience, and refusing to
    /// draw a list over one would be a page that will not load because of a
    /// word the user cannot see and did not knowingly write.
    ///
    /// Defaulting is that decoder's rule and not a property of this
    /// function, which is why it returns an `Option` at all. The select
    /// handler is the caller that makes the opposite choice: every option in
    /// the control is one this build wrote, so an unrecognized value there is
    /// a word nobody offered, and it is ignored rather than defaulted —
    /// silently re-sorting the list would be a worse answer than doing
    /// nothing.
    pub(crate) fn from_key(text: &str) -> Option<Self> {
        match text {
            "activity" => Some(ListSort::Activity),
            "created" => Some(ListSort::Created),
            "title" => Some(ListSort::Title),
            _ => None,
        }
    }
}

/// The one client preference the helm remembers for every client at once
/// (SPEC.md, Session list): the chosen list order and the last
/// user-selected session. The wire shape of `GET`/`PUT /api/preferences`.
///
/// Both fields are `Option` because one type is the whole reply AND the
/// sparse patch a write sends: on the way in, `None` is "never chosen, use
/// the default"; on the way out, `None` is "leave that field alone", which
/// is what lets two clients sharing the row each write only the field the
/// user changed (see [`store_preference`]). `list_sort` stays the bare wire
/// word rather than a decoded [`ListSort`] so an unrecognized value can be
/// carried through to `list::view::decoded_sort`'s fallback instead of
/// failing the decode of the whole reply.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, Deserialize)]
pub(crate) struct Preferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) list_sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_selected: Option<String>,
}

/// How long the seed read of the shared preference may take before the
/// gate gives up and mounts with defaults.
///
/// Deliberately much shorter than [`REQUEST_TIMEOUT`]: `PreferencesGate`
/// holds the ENTIRE authenticated tree behind this one read, and the
/// preference is best-effort startup convenience — a stalled
/// `/api/preferences` must cost a few seconds and the remembered values,
/// never a minute of blank page in front of a helm whose session list is
/// perfectly able to answer.
const PREFERENCE_SEED_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Read the shared preference once, right after authentication.
///
/// The caller (`PreferencesGate`) holds the whole authenticated tree off
/// the screen until this answers, which is what makes the first render
/// seeded rather than corrected: the sort control and the auto-select
/// effect see the remembered values on their first run, never their
/// fallbacks. A failure — including [`PREFERENCE_SEED_TIMEOUT`] running
/// out — is the caller's to treat as "nothing remembered": SPEC.md makes
/// this preference best-effort, and a helm that cannot answer it can still
/// list sessions.
pub(crate) async fn fetch_preferences(base: &str) -> Result<Preferences, String> {
    let url = format!("{base}/api/preferences");
    let resp = send_within(client().get(&url), PREFERENCE_SEED_TIMEOUT).await?;
    if !resp.status().is_success() {
        return Err(read_failure("GET", &url, resp).await);
    }
    resp.json::<Preferences>().await.map_err(|e| e.to_string())
}

/// Which half of the shared preference a write names.
///
/// The write queue below serializes writes PER FIELD: the two fields are
/// independent last-writer-wins values (the helm merges sparse patches per
/// field), so a slow selection write must not delay a sort write, while two
/// writes to the SAME field must reach the helm in the order the user made
/// them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreferenceField {
    Sort,
    Selected,
}

impl PreferenceField {
    /// The field's wire key in a `PUT /api/preferences` patch.
    fn key(self) -> &'static str {
        match self {
            PreferenceField::Sort => "list_sort",
            PreferenceField::Selected => "last_selected",
        }
    }
}

/// One field's slice of the write queue.
///
/// `latest` is the newest locally chosen value this session, `acked`
/// whether a PUT carrying exactly that value has succeeded, `in_flight`
/// whether a writer task currently owns the field. "Dirty" — chosen but
/// not acked — is the state [`PreferenceWrites::dirty`] reports and the
/// reauthentication replay exists for.
#[derive(Default)]
struct FieldWrite {
    latest: Option<String>,
    acked: bool,
    in_flight: bool,
}

/// The write queue's whole state: a latest-wins slot per field.
///
/// This is a pure state machine, deliberately separate from the async
/// writer that acts on it, so the ordering contract — the reason it exists
/// — is unit-testable without a runtime. The contract: at most one PUT per
/// field is ever in flight, and when a newer value is recorded mid-flight
/// the writer sends it AFTER the current request settles. Independent
/// spawned requests could finish in either order, and the stale value
/// finishing last would silently win the row (the page would show the new
/// choice while every other client — and the next launch — read the old
/// one).
///
/// It also carries this client's unpersisted choices across an
/// authentication remount: `PreferencesGate` re-reads the helm's row after
/// a credential exchange, and without [`Self::dirty`] overlaying the
/// still-unacked local values, that re-read would roll the current client
/// back to whatever the helm last stored — a silent persistence failure
/// becoming a visible reversal, which SPEC.md's best-effort clause does
/// not license. The state is a process-wide static precisely so it lives
/// OUTSIDE every remounted subtree.
#[derive(Default)]
struct PreferenceWrites {
    sort: FieldWrite,
    selected: FieldWrite,
}

impl PreferenceWrites {
    fn field(&mut self, field: PreferenceField) -> &mut FieldWrite {
        match field {
            PreferenceField::Sort => &mut self.sort,
            PreferenceField::Selected => &mut self.selected,
        }
    }

    /// Record a new local choice. Returns whether the caller must start a
    /// writer (none is running for this field).
    fn record(&mut self, field: PreferenceField, value: String) -> bool {
        let slot = self.field(field);
        slot.latest = Some(value);
        slot.acked = false;
        if slot.in_flight {
            return false;
        }
        slot.in_flight = true;
        true
    }

    /// The value the writer should send next — always the newest recorded.
    fn next_to_send(&mut self, field: PreferenceField) -> Option<String> {
        self.field(field).latest.clone()
    }

    /// A PUT for `sent` settled. Returns whether the writer must go again
    /// because a newer value arrived while it was out; otherwise the field
    /// is released (and marked acked on success).
    ///
    /// A FAILED write is not retried on its own: the value stays dirty for
    /// the next reauthentication replay or the next user change, per the
    /// fire-and-forget policy — persistence failures cost the next launch,
    /// never a retry loop against a helm that is refusing.
    fn finished(&mut self, field: PreferenceField, sent: &str, success: bool) -> bool {
        let slot = self.field(field);
        if slot.latest.as_deref() != Some(sent) {
            return true;
        }
        slot.acked = success;
        slot.in_flight = false;
        false
    }

    /// The writer was cancelled mid-request (its task dropped): release the
    /// field so a later write can start a replacement, keeping the value
    /// dirty so nothing pretends it was persisted.
    fn writer_lost(&mut self, field: PreferenceField) {
        self.field(field).in_flight = false;
    }

    /// This session's newest choice for `field` when no PUT has confirmed
    /// it — what a post-reauthentication seed must overlay and replay.
    /// `None` both when nothing was chosen and when the choice is safely on
    /// the helm (where the fetched row is the better answer: another client
    /// may have written since).
    fn dirty(&self, field: PreferenceField) -> Option<String> {
        let slot = match field {
            PreferenceField::Sort => &self.sort,
            PreferenceField::Selected => &self.selected,
        };
        if slot.acked {
            return None;
        }
        slot.latest.clone()
    }

    /// Claim a dirty field for a replay writer. Returns the value to send,
    /// or `None` when there is nothing dirty or a writer already owns it.
    fn begin_replay(&mut self, field: PreferenceField) -> Option<String> {
        let value = self.dirty(field)?;
        let slot = self.field(field);
        if slot.in_flight {
            return None;
        }
        slot.in_flight = true;
        Some(value)
    }
}

/// The process-wide write queue. A static, not component state, on
/// purpose: it must survive `PreferencesGate` (and on desktop the whole
/// `AppBody`) being unmounted and remounted by credential recovery — see
/// [`PreferenceWrites`].
static PREFERENCE_WRITES: std::sync::LazyLock<std::sync::Mutex<PreferenceWrites>> =
    std::sync::LazyLock::new(Default::default);

fn preference_writes() -> std::sync::MutexGuard<'static, PreferenceWrites> {
    PREFERENCE_WRITES
        .lock()
        .expect("preference write queue lock poisoned")
}

/// Clear `writer_lost` on drop unless the writer finished cleanly.
///
/// `spawn_forever` outlives any one component, but the app can still tear
/// the task down mid-await; without this, a cancelled writer would strand
/// `in_flight = true` and every later write to that field would enqueue
/// forever behind a writer that no longer exists.
struct WriterGuard {
    field: PreferenceField,
    armed: bool,
}

impl Drop for WriterGuard {
    fn drop(&mut self) {
        if self.armed {
            preference_writes().writer_lost(self.field);
        }
    }
}

/// Record one changed field of the shared preference and see that it
/// reaches the helm, in order.
///
/// Fire-and-forget by contract (SPEC.md, Errors and diagnostics: the
/// preference is the one best-effort exception): the choice has already
/// taken effect in this client before any request leaves, and a failure
/// costs the NEXT launch its convenience, never this one its choice —
/// failures are logged here, not surfaced. Always a one-field patch,
/// because the row is shared and a whole-row write would carry this
/// client's stale copy of the other field over whatever another client
/// wrote since. Same-field writes are serialized latest-wins through
/// [`PreferenceWrites`] so a burst of changes cannot land on the helm in
/// reverse order.
pub(crate) fn store_preference(base: &str, field: PreferenceField, value: String) {
    if preference_writes().record(field, value) {
        spawn_preference_writer(base.to_string(), field);
    }
}

/// Overlay this session's unpersisted choices onto a freshly fetched row,
/// restarting their writes.
///
/// Called by `PreferencesGate` every time it (re)seeds — most importantly
/// after credential recovery, where the fetched row predates a choice
/// whose PUT died with the old credential. Fields whose last write was
/// acknowledged are NOT overlaid: the helm's answer is newer authority for
/// those (another client may have written since this one did).
pub(crate) fn seed_with_local_changes(base: &str, mut seed: Preferences) -> Preferences {
    for field in [PreferenceField::Sort, PreferenceField::Selected] {
        let claimed = {
            let mut queue = preference_writes();
            match queue.dirty(field) {
                Some(value) => {
                    let claimed = queue.begin_replay(field).is_some();
                    match field {
                        PreferenceField::Sort => seed.list_sort = Some(value),
                        PreferenceField::Selected => seed.last_selected = Some(value),
                    }
                    claimed
                }
                None => false,
            }
        };
        if claimed {
            spawn_preference_writer(base.to_string(), field);
        }
    }
    seed
}

/// Start the one writer task a claimed field gets.
///
/// `spawn_forever` rather than `spawn`: a scope-owned task would be
/// cancelled with whatever component happened to call `store_preference`,
/// and a selection made on the way out of a view still deserves its write.
fn spawn_preference_writer(base: String, field: PreferenceField) {
    dioxus::core::spawn_forever(async move {
        let mut guard = WriterGuard { field, armed: true };
        loop {
            let Some(value) = preference_writes().next_to_send(field) else {
                break;
            };
            let success = send_preference_patch(&base, field, &value).await;
            if !preference_writes().finished(field, &value, success) {
                break;
            }
        }
        guard.armed = false;
    });
}

/// One `PUT /api/preferences` carrying a single-field patch. Failures are
/// logged and reported to the queue, never surfaced.
async fn send_preference_patch(base: &str, field: PreferenceField, value: &str) -> bool {
    let url = format!("{base}/api/preferences");
    let patch = serde_json::json!({ field.key(): value });
    let outcome = match send(client().put(&url).json(&patch)).await {
        Ok(resp) if resp.status().is_success() => return true,
        Ok(resp) => read_failure("PUT", &url, resp).await,
        Err(detail) => detail,
    };
    dioxus::logger::tracing::warn!(
        target: "preferences",
        "could not store the list preference on the helm: {outcome}"
    );
    false
}

/// One listing request's whole query string: the order first, then the
/// filter's own parameters.
///
/// The order is present on every request this UI makes, including the
/// unfiltered ones — see [`ListSort`] for why the UI never leans on the
/// helm's own default. The consequence worth naming here is that the string
/// is never empty, which is what lets [`fetch_sessions`] build its URL
/// without a has-any-parameters case.
fn list_query(filter: &SessionFilter, sort: ListSort) -> String {
    let ordering = format!("sort={}", sort.key());
    let filtered = filter.query();
    if filtered.is_empty() {
        ordering
    } else {
        format!("{ordering}&{filtered}")
    }
}

/// The whole session list, as this UI holds it: the helm's one reply, in
/// the helm's own order, plus the facts about the REQUEST the banner and
/// the absence rules need.
///
/// `pub(crate)` on the type and every field: `list::ListView` holds one of
/// these in its own signal and reads all three directly, which a private
/// struct cannot allow across the module boundary this split introduced.
/// Nothing outside the crate has any business seeing it.
pub(crate) struct SessionListing {
    pub(crate) sessions: Vec<Session>,
    /// Every session in the merged view across every host, as the helm
    /// counts them — which is what SPEC.md's "showing N of M" is about, and
    /// is NOT the same as `sessions.len()` whenever the helm's cap cut the
    /// reply.
    ///
    /// Counted before any of the user's search dimensions, deliberately: an
    /// M that moved when the user typed would make "N matching of M" compare
    /// a number against itself.
    ///
    /// It DOES follow the archive switch, because that switch says which
    /// list this is rather than narrowing one: the default view's rows and
    /// its M are both about the non-archived fleet, and turning the switch on
    /// widens both. The helm computes it that way
    /// (`aggregate::SessionListBody::total`), and nothing here adjusts the
    /// number it was given.
    pub(crate) total: u64,
    /// How many sessions matched the filter, fleet-wide — or `None` when
    /// this helm did not say and no honest number can be substituted.
    ///
    /// Fleet-wide rather than reply-wide is the whole point: a reply the
    /// cap cut holds fewer rows than matched, and the banner's job is to
    /// say so.
    ///
    /// `None` is only ever produced for a FILTERED request against a helm
    /// that predates the count (see [`matching_count`]), and it travels this
    /// far rather than being resolved in the fetch because it is a fact
    /// about the helm the banner has to render — `rows::count_banner` says
    /// the filter went unanswered instead of printing a number nobody
    /// counted.
    pub(crate) matching: Option<u64>,
    /// Whether the USER narrowed this request — what the banner's wording
    /// follows (`SessionFilter::narrows_beyond_archive`).
    ///
    /// From the REQUEST, never derived by comparing `matching` against
    /// `total`: a filter that happens to match everything is still a filter,
    /// and the banner should say "5 matching of 5 sessions" rather than
    /// silently reverting to the unfiltered wording and leaving the user
    /// wondering whether their filter took.
    ///
    /// The archive switch is not one of those filters in either position —
    /// see the predicate's own docs, and [`Self::omits_fleet_members`] for
    /// the field that DOES count it.
    pub(crate) filtered: bool,
    /// Whether the request behind this listing PERMITTED the helm to leave some
    /// of the fleet out (`SessionFilter::omits_fleet_members`) — the flag
    /// that decides what an absence here may be read as.
    ///
    /// True does not mean anything was actually withheld; it means nothing
    /// missing from `sessions` can be assumed gone. That is the only
    /// question a reader can answer from a reply, since a session that is
    /// not here left no trace saying why.
    ///
    /// A second flag rather than a second reading of `filtered`, because the
    /// two questions have different answers for exactly one listing: the
    /// DEFAULT view, which is unfiltered to a reader (`filtered` is false, so
    /// the banner says "12 sessions") while still hiding every archived
    /// session (so an absent row is not a departure). Collapsing them would
    /// make a poll retire an optimistic rename, close an editor, or drop a
    /// confirmation the moment a session was archived somewhere else.
    ///
    /// From the REQUEST as well, for the same reason: what a reply covers is
    /// a property of what was asked, not of what came back.
    pub(crate) omits_fleet_members: bool,
    /// Whether entries remain beyond what `sessions` carries — the helm's
    /// own word, passed through: some host's reply hit the wire's cap, or
    /// the merged, filtered, sorted view did. Nothing on this side adds to
    /// it; there is no walk with ceilings of its own any more.
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

// ---------------------------------------------------------------------
// Protected requests, one door
// ---------------------------------------------------------------------

/// The HTTP client every request below is built from.
///
/// A seam, not an optimization. Every protected REST request is built here,
/// then [`send`] attaches the current origin-scoped device secret. The
/// single construction and classification path makes explicit authentication
/// and build-stamp handling properties of the client rather than call-site
/// habits that a new endpoint can forget.
///
/// Deliberately constructs a FRESH client per call, which is what the fifteen
/// call sites did before this function existed. A shared client is the
/// obvious-looking improvement and is a real behavior change, not a
/// refactor: `reqwest::Client` owns the connection pool, and it also captures
/// TLS configuration and the DNS resolver at construction, so hoisting one
/// into a `static` freezes all of that at whatever the process looked like on
/// the first request and keeps connections alive across the whole run. Native
/// desktop clients deliberately disable proxies at construction because this
/// funnel attaches a loopback bearer credential; browser builds retain their
/// platform transport behavior.
///
/// The device session does not live in this object. Browser localStorage owns
/// it under the helm's complete origin, and [`send`] reads it for each
/// request before adding the explicit Authorization header. A shared client
/// would therefore add connection pooling, not authentication semantics.
fn client() -> reqwest::Client {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("proxy-free desktop HTTP client construction is infallible")
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
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

/// Send one protected request and read the helm's build stamp off its reply
/// (PLAN_M6.md item 6's client↔helm skew edge).
///
/// Every protected request uses this function, and that is the point rather
/// than tidiness: the skew and authentication checks only mean anything if
/// no protected path skips them. The sole bypass is [`exchange_token`],
/// which cannot carry the credential it exists to mint and performs its own
/// build-stamp observation.
///
/// Successful and non-401 refusal replies are handed back for their endpoint
/// to decode. Every 401 is consumed here: the authentication middleware's
/// marker becomes a page-wide transition, while any other 401 remains the
/// originating operation's ordinary failure.
///
/// The [`REQUEST_TIMEOUT`] is applied here for the same funnel reason: a
/// per-call-site deadline is a deadline someone eventually forgets, and the
/// one request left unbounded is the one that wedges a surface. It is the
/// only deadline there is — the paged listing once divided a budget of its
/// own across pages, and with it went the last caller that wanted anything
/// but the default. In the desktop build, a recognized 401 refreshes the
/// native credential, remounts the independently authenticated webview
/// gate, and retries once, all inside one absolute deadline — recovery
/// cannot turn one request's remaining budget into two fresh ones; browser
/// builds retain the ordinary full-page token prompt.
async fn send(request: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
    send_inner(request, REQUEST_TIMEOUT)
        .await
        .map_err(send_error_text)
}

/// [`send`] with a caller-chosen deadline instead of [`REQUEST_TIMEOUT`].
///
/// One caller earns this: the preference seed behind `PreferencesGate`,
/// which holds the whole authenticated tree and must give up in seconds
/// rather than let a stalled best-effort read cost a minute of blank page
/// (see [`PREFERENCE_SEED_TIMEOUT`]). Everything else goes through
/// [`send`]; a deadline chosen per call site is a deadline someone
/// eventually forgets.
async fn send_within(
    request: reqwest::RequestBuilder,
    timeout: std::time::Duration,
) -> Result<reqwest::Response, String> {
    send_inner(request, timeout).await.map_err(send_error_text)
}

/// Failure at the one response-classification point every Rust-side API
/// request traverses.
///
/// `Unauthenticated` is deliberately typed rather than formatted into an
/// ordinary status string: the full-page token surface is a global state
/// transition, not one call site's inline error. [`send`] matches these
/// variants when it flattens public failures.
#[derive(Debug)]
enum SendError {
    Unauthenticated,
    Request(String),
}

/// Deliberately match the typed funnel result at the point endpoint contracts
/// flatten to display text.
fn send_error_text(error: SendError) -> String {
    match error {
        SendError::Unauthenticated => "authentication is required".to_string(),
        SendError::Request(detail) => detail,
    }
}

/// [`send`]'s typed body — split only so `send` can flatten the typed
/// error at one seam; every caller but one goes through `send` and its
/// [`REQUEST_TIMEOUT`]. (A deadline parameter left when the paged listing
/// went and returned with the preference seed: [`send_within`] is the one
/// caller-chosen deadline, and `PreferencesGate`'s docs say why it earns
/// the exception the paged listing lost.)
async fn send_inner(
    mut request: reqwest::RequestBuilder,
    timeout: std::time::Duration,
) -> Result<reqwest::Response, SendError> {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    let deadline = tokio::time::Instant::now() + timeout;
    if let Some(secret) = crate::auth::device_secret() {
        request = request.bearer_auth(secret);
    }
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    let retry = request.try_clone();
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    let request_timeout = remaining(deadline)?;
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    let request_timeout = timeout;
    let resp = request
        .timeout(request_timeout)
        .send()
        .await
        .map_err(|error| SendError::Request(error.to_string()))?;
    // Called for its effect, in a statement of its own. Folding it into a
    // `.map()` reads as a transformation of the response and is not one —
    // the value is unchanged and the point is entirely the side effect.
    skew::note_build(&resp);
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
        let body = tokio::time::timeout_at(deadline, resp.text())
            .await
            .map_err(|_| SendError::Request("request deadline elapsed".to_string()))?
            .map_err(|error| SendError::Request(error.to_string()))?;
        #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
        let body = resp
            .text()
            .await
            .map_err(|error| SendError::Request(error.to_string()))?;
        if device_auth_required(&body) {
            #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
            if !skew::build_skew_detected_now()
                && let Some(retry) = retry
            {
                return retry_desktop_request(retry, deadline, || async {
                    crate::desktop::refresh_native_device()
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            }
            // Stamp classification happens first. A bundle that disagrees
            // with the helm cannot safely interpret even this marker, so the
            // skew prompt wins and this one stays dormant. Only the browser
            // owns the token prompt; desktop recovery is native and the
            // webview gate cannot repair the native request from that form.
            #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
            if !skew::build_skew_detected_now() {
                crate::auth::require_token();
            }
            return Err(SendError::Unauthenticated);
        }
        let detail = body.trim();
        return Err(SendError::Request(if detail.is_empty() {
            "the helm refused this request as unauthorized".to_string()
        } else {
            detail.to_string()
        }));
    }
    Ok(resp)
}

/// Refresh and retry inside the original request's absolute deadline.
///
/// The injected refresh future is a narrow test seam for the deadline span;
/// production still has exactly one caller and one native refresh operation.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
async fn retry_desktop_request<Refresh, Refreshed>(
    retry: reqwest::RequestBuilder,
    deadline: tokio::time::Instant,
    refresh: Refresh,
) -> Result<reqwest::Response, SendError>
where
    Refresh: FnOnce() -> Refreshed,
    Refreshed: std::future::Future<Output = Result<(String, bool), String>>,
{
    let (secret, replaced) = tokio::time::timeout_at(deadline, refresh())
        .await
        .map_err(|_| SendError::Request("request deadline elapsed".to_string()))?
        .map_err(SendError::Request)?;
    if replaced {
        crate::auth::require_desktop_webview_reauth();
    }
    let retried = retry
        .bearer_auth(secret)
        .timeout(remaining(deadline)?)
        .send()
        .await
        .map_err(|error| SendError::Request(error.to_string()))?;
    skew::note_build(&retried);
    if retried.status() != reqwest::StatusCode::UNAUTHORIZED {
        return Ok(retried);
    }
    let retry_body = tokio::time::timeout_at(deadline, retried.text())
        .await
        .map_err(|_| SendError::Request("request deadline elapsed".to_string()))?
        .map_err(|error| SendError::Request(error.to_string()))?;
    if device_auth_required(&retry_body) {
        // Desktop recovery already remounted the webview's independent gate.
        // The browser token prompt cannot persist a native credential.
        return Err(SendError::Unauthenticated);
    }
    let detail = retry_body.trim();
    Err(SendError::Request(if detail.is_empty() {
        "the helm refused this request as unauthorized".to_string()
    } else {
        detail.to_string()
    }))
}

/// Remaining time in one request's absolute budget, including desktop
/// credential recovery and its single retry.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
fn remaining(deadline: tokio::time::Instant) -> Result<std::time::Duration, SendError> {
    deadline
        .checked_duration_since(tokio::time::Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| SendError::Request("request deadline elapsed".to_string()))
}

/// Only the authentication middleware emits this structured error code;
/// supervisor authorization refusals may share status 401 but not meaning.
pub(crate) fn device_auth_required(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .is_some_and(|value| {
            value.get("code").and_then(|code| code.as_str()) == Some("device_auth_required")
        })
}

/// Exchange a pasted bootstrap token for an origin-scoped device secret.
///
/// This request deliberately does not feed its own 401 back into the global
/// 401 funnel: the token form is already mounted, and a rejected token belongs
/// as an error on that form rather than as another request to show it.
pub(crate) async fn exchange_token(base: &str, token: &str) -> Result<String, String> {
    let url = format!("{base}/api/auth/token");
    let resp = client()
        .post(&url)
        .json(&serde_json::json!({ "token": token }))
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    skew::note_build(&resp);
    if skew::build_skew_detected_now() {
        return Err("the helm build changed; reload before authenticating".to_string());
    }
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("that token was not accepted".to_string());
    }
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    #[derive(serde::Deserialize)]
    struct DeviceExchange {
        device_secret: String,
    }
    resp.json::<DeviceExchange>()
        .await
        .map(|exchange| exchange.device_secret)
        .map_err(|error| format!("the helm returned an unreadable device session: {error}"))
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

/// What this reply may honestly claim matched, given what it reported and
/// what was asked for.
///
/// The substitution — an absent count answered with the view's total — is
/// correct for an UNFILTERED request and only for one: with no filter, every
/// session in the view matches, so `total` is not a stand-in but the same
/// number by another name. That is what makes a helm one version behind
/// produce the banner it always did.
///
/// "Unfiltered" here is the BANNER's reading
/// (`SessionFilter::narrows_beyond_archive`), which is what keeps the
/// ordinary view and the archive switch out of the ignored-filter clause
/// below. Both are honest under it: the modern helm answers the default view
/// with a real matching count, and an older one that never filtered still
/// served the whole fleet, which is what `total` then describes.
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
fn matching_count(narrowed: bool, reported: Option<u64>, total: u64) -> Option<u64> {
    reported.or_else(|| (!narrowed).then_some(total))
}

/// Fetch the whole session listing in one request, flattening every failure
/// into a displayable string.
///
/// One `GET`, one object: the helm serves the entire view — merged across
/// hosts, filtered, sorted — up to its cap, and says with `truncated` when
/// the cap cut it (SPEC.md's Session list section). This UI follows no
/// cursor, keeps no ceilings of its own, and never asks a second time to
/// reconcile what it got: the rows and both counts come from one snapshot
/// on the helm's side, so there is nothing to reconcile.
///
/// ## The order is the helm's
///
/// No client-side sort. The helm serves the order the request asked for —
/// `sort` names it, each order with the same stable tiebreaks — so the way
/// to change the order is to ASK for a different one, which is exactly what
/// the sidebar's sort control does.
///
/// The message on failure is a `String`, not `reqwest::Error`, because it is
/// rendered to the user directly (SPEC.md wants concrete errors) — the URL
/// and status are folded into the message here rather than logged and
/// dropped.
pub(crate) async fn fetch_sessions(
    base: &str,
    filter: &SessionFilter,
    sort: ListSort,
) -> Result<SessionListing, String> {
    let query = list_query(filter, sort);
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    crate::desktop::log_smoke_session_query(&query);
    // No empty-query case: `list_query` always carries the order, so every
    // request this UI makes has at least one parameter.
    let url = format!("{base}/api/sessions?{query}");
    let resp = send(client().get(&url)).await?;
    if !resp.status().is_success() {
        return Err(read_failure("GET", &url, resp).await);
    }
    let body = resp
        .json::<SessionListBody>()
        .await
        .map_err(|e| e.to_string())?;
    // BOTH predicates, because they answer different questions about the
    // same request and the listing carries both: what the banner says
    // happened, and what this reply is allowed to be evidence about. See
    // `SessionFilter::narrows_beyond_archive` for where they part.
    let filtered = filter.narrows_beyond_archive();
    Ok(SessionListing {
        sessions: body.sessions,
        total: body.total,
        matching: matching_count(filtered, body.matching, body.total),
        filtered,
        omits_fleet_members: filter.omits_fleet_members(),
        truncated: body.truncated,
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

/// What a create says to launch: a command line, or a profile from the helm
/// catalog (PLAN_M6_75.md item 3's two creation modes).
///
/// One argument rather than two optional ones, because the wire treats them
/// as a CHOICE: a body naming both is refused (a profile already states what
/// to run, so there is no honest merge) and a body naming neither is refused
/// too. Two `Option` parameters would let a caller express both illegal
/// shapes and find out over the network; this type lets it express neither.
///
/// Borrowed rather than owned: every caller already holds the string it is
/// about to send, and a create is one request rather than something stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreateAgent<'a> {
    /// A raw invocation, shell-split by the supervisor.
    Command(&'a str),
    /// A `Profile::id` from the helm catalog. The helm resolves it before
    /// forwarding the resulting invocation to the selected host.
    Profile(&'a str),
}

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
/// single-machine setup. A host that is not connected fails the create with
/// an ordinary conflict, and that refusal arrives here as the helm's own
/// words like any other.
///
/// `agent` selects between the two creation modes PLAN_M6_75.md item 3 made
/// mutually exclusive on the wire — see [`CreateAgent`] for why they arrive
/// here as one argument rather than as two optional ones.
pub(crate) async fn create_session(
    base: &str,
    cwd: &str,
    agent: CreateAgent<'_>,
    title: &str,
    intent_key: &str,
    host: Option<HostId>,
    expected_incarnation: Option<u64>,
) -> Result<Session, String> {
    let url = format!("{base}/api/sessions");
    let resp = send(client().post(&url).json(&create_body(
        cwd,
        agent,
        title,
        intent_key,
        host,
        expected_incarnation,
    )))
    .await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    resp.json::<Session>().await.map_err(|e| e.to_string())
}

/// One create's request body.
///
/// Split from the request purely so the two rules it has to keep can be
/// exercised without a helm: the title's absent-versus-empty distinction, and
/// the creation mode's exclusivity — a body carrying both `invocation` and
/// `profile_id`, or neither, is a 400 rather than a create, and the failure
/// would arrive at the moment a user pressed the button.
fn create_body(
    cwd: &str,
    agent: CreateAgent<'_>,
    title: &str,
    intent_key: &str,
    host: Option<HostId>,
    expected_incarnation: Option<u64>,
) -> serde_json::Value {
    // `title` is the API's `Option<String>`, not a bare string: an empty
    // field means "auto-generate", per SPEC.md's "Title: optional;
    // auto-generated when omitted" — sending `Some("")` would instead ask
    // the supervisor to name the session the empty string.
    let title = (!title.trim().is_empty()).then_some(title);
    let mut body = serde_json::json!({
        "cwd": cwd,
        "title": title,
        "intent_key": intent_key,
        "host": host,
    });
    // Exactly one of the two mode fields is ever written. Building it from
    // ONE value is what makes the exclusivity structural rather than a rule
    // each caller has to remember, and there is no honest merge to fall back
    // on: a profile already states what to run.
    match agent {
        CreateAgent::Command(invocation) => body["invocation"] = serde_json::json!(invocation),
        CreateAgent::Profile(profile_id) => body["profile_id"] = serde_json::json!(profile_id),
    }
    // The connection this create was prepared against. It matters most in
    // PROFILE mode, where the id would otherwise resolve on whatever install
    // now answers for that host — every fresh supervisor seeds the same
    // starters, so the wrong install is a successful launch of the wrong
    // thing rather than a refusal. Sent in raw mode too: the directory and
    // the command were chosen for a machine, and the same substitution puts
    // them on another one. This is the only request in the API that still
    // carries such a claim; profile edits and catalog reads carry none.
    if let Some(incarnation) = expected_incarnation {
        body["expected_incarnation"] = serde_json::json!(incarnation);
    }
    body
}

/// The marker the helm appends to a create refused because the host is no
/// longer on the connection the create named (`precondition.rs` in the helm).
const INCARNATION_MARKER: &str = "[farhelm:precondition/incarnation]";

/// Classify a create refusal: `(stale, prose)`, where `stale` says the helm
/// refused because the world moved under the request — re-read and re-seed,
/// rather than show a permanent error — and `prose` is the sentence with the
/// machine marker stripped for display.
///
/// Every OTHER 409 (a host that is not connected, a supervisor's own refusal)
/// carries no marker and must not be answered by re-reading, which is why
/// this is a marker check and not a status check.
pub(crate) fn precondition_of(refusal: &str) -> (bool, String) {
    let trimmed = refusal.trim_end();
    match trimmed.strip_suffix(INCARNATION_MARKER) {
        Some(prose) => (true, prose.trim_end().to_string()),
        None => (false, refusal.to_string()),
    }
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

/// Archive a session and return its retained, terminal-less state.
///
/// The response is authoritative even for a retry: archive is idempotent,
/// so an ambiguous first request can be repeated without turning recovery
/// into an error. The caller uses the returned `archived` flag rather than
/// guessing from an emptied tab list or an exited status.
pub(crate) async fn archive_session(base: &str, id: &str) -> Result<Session, String> {
    let url = format!("{base}/api/sessions/{}/archive", encode_path_segment(id));
    let resp = send(client().post(&url)).await?;
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

/// A discovery result from `POST /api/hosts/probe`.
///
/// The helm owns the distinction between positive absence and a transport
/// failure. A caller may offer setup only for [`Self::Provisionable`]; a
/// failed request never reaches this type, and [`Self::Manual`] is a
/// supported host whose setup still has to use the documented manual path.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "result", rename_all = "kebab-case")]
pub(crate) enum ProbeResponse {
    /// A supervisor answered and the helm registered it as-is.
    Discovered,
    /// No supervisor answered, and the retained plan may be consumed once.
    Provisionable {
        /// Opaque, one-use confirmation id. Never parsed or reconstructed.
        probe_id: String,
        /// The plan rendered by the helm from the executor's own actions.
        confirmation: String,
    },
    /// The transport worked, but automatic provisioning does not cover the
    /// target.
    Manual { reason: String },
    /// The successful response may already have registered an answering
    /// supervisor, but this build could not decode which outcome occurred.
    /// Callers must refresh the registry and must not invent a setup offer.
    #[serde(skip)]
    Unvalidated(String),
}

/// The one-use plan returned by the first explicit UPDATE request.
///
/// UPDATE uses the same inspect-then-confirm discipline as ADD. Posting
/// `probe_id` back to the host route consumes exactly the plan whose
/// `confirmation` the user saw.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct UpdatePlan {
    /// Opaque, one-use confirmation id bound to this host and plan.
    pub(crate) probe_id: String,
    /// The concrete converge plan the second request authorizes.
    pub(crate) confirmation: String,
}

/// Identity returned once the helm has accepted a long provisioning run.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct ProvisioningAccepted {
    /// The row whose host-scoped progress route now owns the run.
    pub(crate) host_id: HostId,
    /// Opaque diagnostic identity; displayed only if the host id is wrong.
    pub(crate) run_id: String,
}

/// What an accepted provisioning POST told this build.
///
/// A 202 commits the run before its body is decoded. Treating a malformed
/// body as a refusal would invite the user to consume the one-use plan again
/// even though the helm already did; callers instead refresh the registry
/// and host-scoped run, which are authoritative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProvisioningSubmission {
    /// The run identity decoded and may be checked against the calling row.
    Accepted(ProvisioningAccepted),
    /// The 202 committed, but this build could not decode its identity.
    Unvalidated(String),
}

/// Whether a retained run was an ADD convergence or an explicit UPDATE.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProvisioningOperation {
    Add,
    Update,
}

/// A provisioning run's retained aggregate state.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProvisioningStatus {
    Running,
    Completed,
    Failed,
}

/// One executor action as `GET /api/hosts/{id}/provisioning` reports it.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct ProvisioningStep {
    /// Stable executor label, shown as supplied.
    pub(crate) step: String,
    /// Kept as a string so a newer step outcome costs only its styling, not
    /// the entire progress panel.
    pub(crate) status: String,
    /// The executor's bounded explanation, including a failing command's
    /// own message. Rendered through the peer-text boundary.
    pub(crate) message: Option<String>,
}

/// The host-scoped progress snapshot retained in this helm process.
///
/// Another run replaces it, and restarting the helm clears it. The durable
/// host registration survives either event; this view is only live progress
/// and the most recent result for the current process lifetime.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct ProvisioningView {
    /// Absent only for the explicit idle view.
    pub(crate) run_id: Option<String>,
    /// Absent with `run_id`; otherwise which idempotent operation may rerun.
    pub(crate) operation: Option<ProvisioningOperation>,
    pub(crate) status: ProvisioningStatus,
    /// Executor order, retained across completion until another run starts.
    pub(crate) steps: Vec<ProvisioningStep>,
    /// Aggregate explanation. Step messages remain separately available.
    pub(crate) message: Option<String>,
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
    decode_hosts(resp).await
}

/// Decode the frozen host-list envelope for the desktop bootstrap and the
/// ordinary UI client alike. Bootstrap cannot call [`fetch_hosts`] before a
/// Dioxus runtime exists because the shared send funnel updates UI signals;
/// sharing this typed boundary still prevents it from inventing a second
/// `serde_json::Value` interpretation of the same wire contract.
pub(crate) async fn decode_hosts(resp: reqwest::Response) -> Result<Vec<Host>, String> {
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

/// The sentence a committed-but-unreadable reply produces.
///
/// `authority` names the surface that WILL say what happened — the read that
/// follows this mutation — and it is a parameter rather than a constant
/// because the answer differs per resource: a host verb is settled by the
/// hosts list, a profile verb by the helm catalog. A message naming the
/// wrong one is worse than a vague one, since it sends the user to a surface
/// that has nothing to do with what they just did.
fn unvalidated_note(error: impl std::fmt::Display, authority: &str) -> String {
    format!(
        "the helm accepted this change, but its reply could not be read by this build ({error}); \
         {authority} is the authoritative view of what happened"
    )
}

/// Classify a successful response by whether its body decoded.
///
/// Shared by the two mutations that answer with a host row so both make the
/// same call about the same situation — the alternative being one of them
/// quietly reverting to "a bad body is a refusal" the next time it is
/// touched.
async fn commit_of<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
    authority: &str,
) -> Commit {
    match resp.json::<T>().await {
        Ok(_) => Commit::Confirmed,
        Err(error) => Commit::Unvalidated(unvalidated_note(error, authority)),
    }
}

/// Discover an ssh destination before either registering or offering setup.
///
/// The optional install coordinates follow the old add form's byte-for-byte
/// rule: blank means absent, while every non-blank path is sent exactly as
/// typed. Discovery itself decides whether the supervisor is registered
/// as-is, a concrete plan is retained for confirmation, or the target stays
/// manual-only.
pub(crate) async fn probe_ssh_host(
    base: &str,
    ssh: &str,
    remote_farhelm: &str,
    remote_state_dir: &str,
) -> Result<ProbeResponse, String> {
    let url = format!("{base}/api/hosts/probe");
    let body = serde_json::json!({
        "target": { "kind": "ssh", "destination": ssh },
        "remote_farhelm": install_field(remote_farhelm),
        "remote_state_dir": install_field(remote_state_dir),
    });
    let resp = send(client().post(&url).json(&body)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    Ok(match resp.json::<ProbeResponse>().await {
        Ok(probed) => probed,
        Err(error) => ProbeResponse::Unvalidated(format!(
            "the helm accepted the host probe, but its reply could not be read: {error}"
        )),
    })
}

/// Probe the reserved local host without SSH-to-self.
pub(crate) async fn probe_local_host(base: &str) -> Result<ProbeResponse, String> {
    let url = format!("{base}/api/hosts/probe");
    let resp = send(
        client()
            .post(&url)
            .json(&serde_json::json!({ "target": { "kind": "local" } })),
    )
    .await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    Ok(match resp.json::<ProbeResponse>().await {
        Ok(probed) => probed,
        Err(error) => ProbeResponse::Unvalidated(format!(
            "the helm accepted the local probe, but its reply could not be read: {error}"
        )),
    })
}

/// Consume one confirmed ADD plan.
///
/// The helm registers the destination before it claims the id-scoped run.
/// The response therefore already names the durable row whose progress URL
/// the caller must read.
pub(crate) async fn provision_host(
    base: &str,
    probe_id: &str,
) -> Result<ProvisioningSubmission, String> {
    let url = format!("{base}/api/hosts/provision");
    let resp = send(
        client()
            .post(&url)
            .json(&serde_json::json!({ "probe_id": probe_id })),
    )
    .await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    Ok(match resp.json::<ProvisioningAccepted>().await {
        Ok(accepted) => ProvisioningSubmission::Accepted(accepted),
        Err(error) => ProvisioningSubmission::Unvalidated(format!(
            "the helm accepted provisioning, but its run identity could not be read ({error}); \
             the hosts list and provisioning panels are the authoritative view of what happened"
        )),
    })
}

/// Freeze an explicit UPDATE plan without changing the host.
pub(crate) async fn plan_host_update(base: &str, host: HostId) -> Result<UpdatePlan, String> {
    let url = format!("{base}/api/hosts/{host}/update");
    let resp = send(client().post(&url)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    resp.json::<UpdatePlan>().await.map_err(|error| {
        format!("the helm planned the update, but its reply could not be read: {error}")
    })
}

/// Consume the one-use plan returned by [`plan_host_update`].
pub(crate) async fn update_host(
    base: &str,
    host: HostId,
    probe_id: &str,
) -> Result<ProvisioningSubmission, String> {
    let url = format!("{base}/api/hosts/{host}/update");
    let resp = send(
        client()
            .post(&url)
            .json(&serde_json::json!({ "probe_id": probe_id })),
    )
    .await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    Ok(match resp.json::<ProvisioningAccepted>().await {
        Ok(accepted) => ProvisioningSubmission::Accepted(accepted),
        Err(error) => ProvisioningSubmission::Unvalidated(format!(
            "the helm accepted the update, but its run identity could not be read ({error}); \
             this host's provisioning panel is the authoritative view of what happened"
        )),
    })
}

/// Read the latest retained provisioning run for one registered host.
pub(crate) async fn fetch_provisioning(
    base: &str,
    host: HostId,
) -> Result<ProvisioningView, String> {
    let url = format!("{base}/api/hosts/{host}/provisioning");
    let resp = send(client().get(&url)).await?;
    if !resp.status().is_success() {
        return Err(read_failure("GET", &url, resp).await);
    }
    resp.json::<ProvisioningView>()
        .await
        .map_err(|error| format!("GET {url} returned an unreadable provisioning state: {error}"))
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
    Ok(commit_of::<Host>(resp, "the hosts list below").await)
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

// ---------------------------------------------------------------------
// Agent profiles (PLAN_M6_75.md items 5 and 8)
//
// Profiles belong to the helm and every host consumes this one catalog. The
// UI therefore reads and writes only `/api/profiles`; selected-host state is
// not part of any profile request.
// ---------------------------------------------------------------------

/// What `GET /api/profiles` answers with: the helm catalog and its remembered
/// default together (farhelm-helm's `ProfilesView`).
///
/// The pairing is the point rather than a convenience. SPEC.md's creation
/// rule — default to the last-used profile, ASK when it is gone — is a
/// question about two facts at once, and the moment that matters is exactly
/// the one where a profile has just been deleted. Two separate reads would
/// have to be reconciled by every client, at whichever moments they happened
/// to land.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub(crate) struct ProfileCatalog {
    /// The catalog in the helm's own order (by id, stable across
    /// renames). Not re-sorted here: the helm does not sort it either, and a
    /// picker that reordered itself on every rename would move options out
    /// from under a user mid-choice.
    pub(crate) profiles: Vec<Profile>,
    /// The id of the profile a session was last created from in this helm, or
    /// `None` if none ever was.
    ///
    /// May name a profile ABSENT from `profiles`, and that combination is
    /// meaningful rather than a bug: it is a deleted default, which is
    /// precisely what SPEC.md's ask-don't-guess fallback keys off. The helm
    /// deliberately does not filter it out, and neither does this.
    #[serde(default)]
    pub(crate) default_profile: Option<String>,
}

/// A profile's whole definition, as a create or an edit sends it.
///
/// There is no partial-update shape and there deliberately is not one: an
/// edit REPLACES the definition, because per-field optionality would make
/// "clear the resume template" and "leave it alone" the same request. Every
/// caller therefore has to have read what it is replacing, which is what the
/// editor does.
///
/// Owned rather than borrowed, unlike [`CreateAgent`]: the editing surface
/// assembles one of these from drafts it owns and hands it over.
///
/// Serialized DIRECTLY as the request body of both mutations — the type is
/// the wire shape. That is a guard, not a convenience: the far side replaces
/// the whole definition, so a hand-written body that fell out of sync with
/// the struct would silently clear whichever field it stopped sending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProfileSpec {
    pub(crate) name: String,
    pub(crate) invocation: String,
    /// The wire spelling of the kind (`claude`, `codex`, `generic`), echoed
    /// verbatim — including one this build does not recognize, which is why
    /// it is a string here as well as on [`Profile`].
    pub(crate) agent_kind: String,
    /// The resume argv, or absent — and absent is a real value rather than a
    /// missing field, with an outcome that depends on the kind: an integrated
    /// kind (`claude`, `codex`) has the supervisor DERIVE a template from the
    /// invocation, while a generic one derives none at all and can therefore
    /// only be restarted fresh. Never a synonym for the empty vector.
    pub(crate) resume_template: Option<Vec<String>>,
}

/// Read the helm-wide profile catalog and its remembered default together.
///
/// The pair is one snapshot because a dangling remembered id is meaningful:
/// it tells the picker to ask instead of silently substituting another
/// profile. Every host uses this same catalog.
pub(crate) async fn fetch_profiles(base: &str) -> Result<ProfileCatalog, String> {
    let url = format!("{base}/api/profiles");
    let resp = send(client().get(&url)).await?;
    if !resp.status().is_success() {
        return Err(read_failure("GET", &url, resp).await);
    }
    resp.json::<ProfileCatalog>()
        .await
        .map_err(|e| e.to_string())
}

/// How a profile mutation the helm ACCEPTED came back.
///
/// [`Commit`]'s shape plus the one thing the host verbs have no use for: the
/// profile as the helm now holds it. The caller needs it, and needs it
/// synchronously — the authoritative catalog re-read is a round trip away,
/// and until it lands this client would otherwise still be handing out the
/// definition it just replaced (see `profiles::CatalogRead::absorb`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProfileCommit {
    /// Accepted, and the reply described the result: the profile as the
    /// helm now holds it.
    Confirmed(Profile),
    /// Accepted, but the reply could not be read by this build. Carries the
    /// decode failure, for a warning line — and, deliberately, nothing to
    /// reconcile from: an unread reply is exactly the case where only the
    /// catalog read that follows can say what happened.
    Unvalidated(String),
}

/// Classify a profile mutation's successful response.
///
/// The catalog — not the hosts list — is what settles a profile change, and
/// this is where that is said (see [`unvalidated_note`]).
async fn profile_commit(resp: reqwest::Response) -> ProfileCommit {
    match resp.json::<Profile>().await {
        Ok(profile) => ProfileCommit::Confirmed(profile),
        Err(error) => ProfileCommit::Unvalidated(unvalidated_note(error, "the profile list below")),
    }
}

/// Define a new profile in the helm catalog (`POST /api/profiles`).
///
/// Nothing is validated here. The name's control-character rule, the
/// per-field size cap, the catalog bound and the `{conversation}` placeholder
/// rule for an integrated kind's resume template are all the helm's,
/// and its refusal is what the user acts on — a second copy of those rules in
/// the client would be the one that drifted, and it could not check the
/// catalog bound at all.
///
/// A 2xx whose body will not decode is [`ProfileCommit::Unvalidated`] rather
/// than an error, on the same reasoning as the host mutations: the profile
/// exists, and telling the user their change was rejected when it
/// demonstrably happened is the worse failure. The catalog re-read that
/// follows is the authoritative account either way.
pub(crate) async fn create_profile(
    base: &str,
    spec: &ProfileSpec,
) -> Result<ProfileCommit, String> {
    let url = format!("{base}/api/profiles");
    let resp = send(client().post(&url).json(&spec)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    Ok(profile_commit(resp).await)
}

/// Replace a helm profile's whole definition (`POST /api/profiles/{profile_id}`).
///
/// A POST to the resource rather than a PUT or PATCH, matching this API's own
/// verb vocabulary (`/stop`, `/rename`, `/destination`) — and emphatically
/// not a partial update: see [`ProfileSpec`].
///
/// Nothing this does touches the sessions already created from the profile.
/// Their launch and resume snapshots are their own (SPEC.md's snapshot rule),
/// and a rename simply starts showing up as `Renamed` on their
/// `SourceProfile` — which is what the session list renders rather than
/// silently adopting the new name.
pub(crate) async fn update_profile(
    base: &str,
    profile_id: &str,
    spec: &ProfileSpec,
) -> Result<ProfileCommit, String> {
    let url = format!("{base}/api/profiles/{}", encode_path_segment(profile_id));
    let resp = send(client().post(&url).json(&spec)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("POST", &url, resp).await);
    }
    Ok(profile_commit(resp).await)
}

/// Remove a profile from the helm catalog (`DELETE /api/profiles/{profile_id}`).
///
/// The reply is an empty object, exactly like `stop` and session `delete`, so
/// a 200 IS the whole answer and there is nothing for a decode to fail on.
///
/// Existing sessions are untouched, including the one thing that looks like
/// an exception: the helm deliberately does NOT clear a remembered default
/// that named this profile, because a default outliving its profile is what
/// lets the next create dialog say "the one you last used is gone, pick
/// another" instead of quietly offering nothing.
pub(crate) async fn delete_profile(base: &str, profile_id: &str) -> Result<(), String> {
    let url = format!("{base}/api/profiles/{}", encode_path_segment(profile_id));
    let resp = send(client().delete(&url)).await?;
    if !resp.status().is_success() {
        return Err(refusal_text("DELETE", &url, resp).await);
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

    /// Only the middleware's structured marker means device authentication;
    /// an unrelated 401 must stay with the operation that received it.
    #[test]
    fn device_authentication_requires_its_specific_marker() {
        assert!(device_auth_required(
            r#"{"error":"unauthenticated","code":"device_auth_required"}"#
        ));
        assert!(!device_auth_required(
            r#"{"error":"spawn identity is unauthorized"}"#
        ));
        assert!(!device_auth_required("spawn identity is unauthorized"));
    }

    /// Time spent on the first response and serialized refresh must reduce
    /// the retry's allowance; otherwise one page can consume two advertised
    /// request budgets while claiming to remain bounded by one.
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    #[tokio::test]
    async fn desktop_refresh_and_retry_share_the_original_deadline() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        });
        let started = tokio::time::Instant::now();
        let deadline = started + std::time::Duration::from_millis(150);
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;

        let result = retry_desktop_request(
            client().get(format!("http://{addr}/stalled-retry")),
            deadline,
            || async {
                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                Ok(("replacement".to_string(), false))
            },
        )
        .await;

        assert!(matches!(result, Err(SendError::Request(_))));
        assert!(started.elapsed() < std::time::Duration::from_millis(250));
        server.abort();
    }

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

    /// Provisioning decoders keep the opaque plan out of the UI while
    /// preserving the server-rendered confirmation and progress vocabulary.
    ///
    /// The plan is intentionally not mirrored: execution and confirmation
    /// both belong to the helm, and teaching this client every action field
    /// would create a second renderer that can drift. The opaque one-use id
    /// and confirmation text are the complete client contract before POST.
    #[test]
    fn provisioning_replies_decode_without_reimplementing_the_plan() {
        let offered: ProbeResponse = serde_json::from_value(serde_json::json!({
            "result": "provisionable",
            "probe_id": "add-plan",
            "plan": {
                "operation": "add",
                "actions": [{ "step": "a-future-action", "future_field": true }],
            },
            "confirmation": "line one\nline two\n",
        }))
        .expect("the helm's concrete plan may grow without teaching the UI its fields");
        assert_eq!(
            offered,
            ProbeResponse::Provisionable {
                probe_id: "add-plan".to_string(),
                confirmation: "line one\nline two\n".to_string(),
            }
        );

        let update: UpdatePlan = serde_json::from_value(serde_json::json!({
            "probe_id": "update-plan",
            "plan": { "operation": "update", "actions": [] },
            "confirmation": "replace then restart\n",
        }))
        .unwrap();
        assert_eq!(update.probe_id, "update-plan");
        assert_eq!(update.confirmation, "replace then restart\n");

        let progress: ProvisioningView = serde_json::from_value(serde_json::json!({
            "host_id": 7,
            "run_id": "run-7",
            "operation": "update",
            "status": "failed",
            "steps": [{
                "step": "restart-supervisor",
                "status": "failed",
                "message": "systemctl reported a concrete failure",
            }],
            "message": "rerun provisioning to continue",
        }))
        .unwrap();
        assert_eq!(progress.status, ProvisioningStatus::Failed);
        assert_eq!(progress.steps[0].status, "failed");
        assert_eq!(
            progress.steps[0].message.as_deref(),
            Some("systemctl reported a concrete failure")
        );
    }

    /// A create names exactly ONE of the two modes, never both and never
    /// neither.
    ///
    /// The helm refuses both illegal shapes with a 400, so getting this wrong
    /// fails at the moment a user presses create — and the "both" shape is
    /// the one a client reaches by accident, by keeping a stale invocation
    /// beside a freshly chosen profile. Asserting the ABSENCE of the other
    /// key is therefore the load-bearing half of each case.
    #[test]
    fn a_create_body_carries_one_creation_mode_and_not_the_other() {
        let raw = create_body(
            "/tmp",
            CreateAgent::Command("claude"),
            "",
            "key-1",
            Some(7),
            Some(11),
        );
        assert_eq!(raw["invocation"], serde_json::json!("claude"));
        assert!(
            raw.get("profile_id").is_none(),
            "a raw create must not also name a profile"
        );
        assert_eq!(
            raw["title"],
            serde_json::Value::Null,
            "an empty title asks the supervisor to generate one; an empty STRING would name the \
             session that"
        );
        assert_eq!(raw["host"], serde_json::json!(7));

        let by_profile = create_body(
            "/tmp",
            CreateAgent::Profile("p-1"),
            "named",
            "key-2",
            Some(7),
            Some(11),
        );
        assert_eq!(by_profile["profile_id"], serde_json::json!("p-1"));
        assert!(
            by_profile.get("invocation").is_none(),
            "a profile already says what to run, and a body naming both is refused outright"
        );
        assert_eq!(by_profile["title"], serde_json::json!("named"));
    }

    /// A create carries the connection it was prepared against, in both
    /// modes, and omits the field when it has nothing to assert.
    ///
    /// This is the one precondition the API still has, and the guard it
    /// feeds refuses a create that would otherwise LAUNCH the wrong profile
    /// on a retargeted host; the absent form is what keeps a caller with no
    /// connection to name (a script, an older client) able to create at all.
    #[test]
    fn a_create_names_the_connection_it_was_prepared_against() {
        let body = create_body(
            "/tmp",
            CreateAgent::Profile("starter-claude"),
            "",
            "key",
            Some(3),
            Some(12),
        );
        assert_eq!(body["expected_incarnation"], serde_json::json!(12));
        let unguarded = create_body(
            "/tmp",
            CreateAgent::Command("claude"),
            "",
            "key",
            Some(3),
            None,
        );
        assert!(
            unguarded.get("expected_incarnation").is_none(),
            "a caller with nothing to assert must still be able to create"
        );
    }

    /// A create refusal is recognized as stale by its MARKER, and the marker
    /// is stripped before the sentence is shown.
    ///
    /// The marker is what tells a client "the world moved, re-read" apart
    /// from every other 409, which must NOT be answered by re-reading; and it
    /// is a machine token a user cannot act on, so it never reaches the form.
    #[test]
    fn a_stale_create_refusal_is_classified_by_marker_and_shown_without_it() {
        let (stale, prose) = precondition_of(
            "host 1 is not the connection this request was prepared against \
             [farhelm:precondition/incarnation]",
        );
        assert!(stale);
        assert_eq!(
            prose,
            "host 1 is not the connection this request was prepared against"
        );
        let (stale, prose) = precondition_of("host 1 is not connected");
        assert!(!stale);
        assert_eq!(prose, "host 1 is not connected");
    }

    /// An edit sends the profile's WHOLE definition, every field present.
    ///
    /// The far side replaces rather than merges, so a field this body omitted
    /// would be cleared on every save — which for `resume_template` means an
    /// editor that never showed the field would quietly strip a starter
    /// profile's resume command the first time anyone renamed it. The
    /// explicit `null` is how "no template" is stated rather than implied.
    #[test]
    fn a_profile_spec_sends_its_whole_definition_including_an_absent_template() {
        let spec = ProfileSpec {
            name: "Claude Code".to_string(),
            invocation: "claude".to_string(),
            agent_kind: "claude".to_string(),
            resume_template: Some(vec![
                "claude".into(),
                "--resume".into(),
                "{conversation}".into(),
            ]),
        };
        let body = serde_json::to_value(&spec).expect("a spec always serializes");
        assert_eq!(body["name"], serde_json::json!("Claude Code"));
        assert_eq!(body["invocation"], serde_json::json!("claude"));
        assert_eq!(body["agent_kind"], serde_json::json!("claude"));
        assert_eq!(
            body["resume_template"],
            serde_json::json!(["claude", "--resume", "{conversation}"])
        );

        let generic = ProfileSpec {
            resume_template: None,
            ..spec
        };
        assert_eq!(
            serde_json::to_value(&generic).expect("a spec always serializes")["resume_template"],
            serde_json::Value::Null,
            "absence is a value here, and it has to be SENT to replace a template that was there"
        );
    }

    /// A catalog with no remembered default decodes as "none ever", and one
    /// whose default names a profile the catalog no longer holds decodes
    /// intact.
    ///
    /// The second half is the case the whole shape exists for: a deleted
    /// default is what SPEC.md's ask-don't-guess fallback keys off, so a
    /// decoder that dropped it — or that helpfully filtered it against the
    /// catalog — would turn "your last profile is gone, pick another" into a
    /// silent nothing.
    #[test]
    fn a_catalog_keeps_a_remembered_default_that_no_longer_resolves() {
        let fresh: ProfileCatalog = serde_json::from_value(serde_json::json!({
            "profiles": [],
        }))
        .expect("a helm with nothing remembered still answers");
        assert_eq!(fresh.default_profile, None);

        let stale: ProfileCatalog = serde_json::from_value(serde_json::json!({
            "profiles": [{
                "id": "p-1", "name": "Codex", "invocation": "codex",
                "agent_kind": "codex", "resume_template": null,
            }],
            "default_profile": "p-gone",
        }))
        .unwrap();
        assert_eq!(stale.default_profile.as_deref(), Some("p-gone"));
        assert_eq!(stale.profiles.len(), 1);
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

    /// The other half of `SessionListBody`'s missing-field tolerance
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
    /// object shape: a later field added to `SessionListBody` must still
    /// let a build one step behind decode today's object, the same
    /// tolerance farhelm-proto's own wire types carry.
    ///
    #[test]
    fn session_list_body_without_total_or_truncated_defaults_both() {
        let json = serde_json::json!({ "sessions": [] });
        let decoded: SessionListBody = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.total, 0);
        assert!(!decoded.truncated);
        assert_eq!(
            decoded.matching, None,
            "an absent matching count must stay absent rather than becoming a zero the banner \
             would then print over a list full of rows"
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
        let filtered: SessionListBody = serde_json::from_value(serde_json::json!({
            "sessions": [], "total": 700, "matching": 12, "truncated": false,
        }))
        .unwrap();
        assert_eq!(filtered.matching, Some(12));
        assert_eq!(
            matching_count(true, filtered.matching, filtered.total),
            Some(12),
            "a helm that answered the filter is believed"
        );

        let older: SessionListBody = serde_json::from_value(serde_json::json!({
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

    /// The archive switch parts the two predicates, and each answers the
    /// question it exists for.
    ///
    /// This is the whole point of there being two. The DEFAULT view sends an
    /// empty query string (omission is the wire spelling of
    /// `include_archived=false`) and reads as unfiltered to a person, so the
    /// banner says "12 sessions" — while the helm is still withholding every
    /// archived row, so the reply covers less than the fleet and nothing may
    /// read an absence in it as a departure. Collapse the two and one of
    /// those goes wrong: either the ordinary list announces a filter nobody
    /// applied, or a session archived from another client is mistaken for one
    /// that left.
    ///
    /// Turning the switch ON is the mirror case: it widens the view rather
    /// than narrowing it, so the banner stays unfiltered and the reply
    /// becomes fleet-wide.
    #[test]
    fn the_archive_switch_is_a_view_rather_than_a_filter() {
        let ordinary = SessionFilter::default();
        assert!(
            !ordinary.narrows_beyond_archive(),
            "nothing was typed, so the banner has no filter to report"
        );
        assert!(
            ordinary.omits_fleet_members(),
            "the ordinary view still hides archived rows, so its absences prove nothing"
        );
        assert_eq!(ordinary.query(), "");

        let widened = SessionFilter {
            include_archived: true,
            ..SessionFilter::default()
        };
        assert!(
            !widened.narrows_beyond_archive(),
            "the switch widens the view; it is not a filter in either position"
        );
        assert!(
            !widened.omits_fleet_members(),
            "and with it on the reply is the whole fleet, so absence IS evidence"
        );

        let searched = SessionFilter {
            title: "needle".to_string(),
            ..SessionFilter::default()
        };
        assert!(
            searched.narrows_beyond_archive() && searched.omits_fleet_members(),
            "a filter a person applied answers both questions the same way"
        );
    }

    /// The sidebar's two filter controls read the switch differently, on
    /// purpose, and this pins the pair rather than either half alone.
    ///
    /// `rows::count_banner` chooses its matching wording with
    /// [`SessionFilter::narrows_beyond_archive`], while `list::ListView`
    /// enables Clear with a full comparison against the default. The switch
    /// is the one setting where those disagree, and each direction is a
    /// separate way to get it wrong: call the widened view matching and the
    /// ordinary count lies; hide the switch from Clear and a user who turned
    /// it on has no control offering to put it back.
    ///
    /// Kept beside the predicate rather than in the view because that is
    /// where the decision is testable at all — the count wording is Dioxus
    /// markup a browser has to render, and the e2e archive spec pins the
    /// rendered half.
    #[test]
    fn the_archive_switch_is_clearable_without_being_announced() {
        let widened = SessionFilter {
            include_archived: true,
            ..SessionFilter::default()
        };
        assert!(
            !widened.narrows_beyond_archive(),
            "the badge must stay off: the switch chose a view, it did not narrow one"
        );
        assert_ne!(
            widened,
            SessionFilter::default(),
            "and Clear must stay live: the switch is still a setting to undo"
        );
        assert_eq!(
            SessionFilter {
                include_archived: false,
                host: None,
                parent: String::new(),
                directory: String::new(),
                profile: String::new(),
                status: String::new(),
                title: String::new(),
            },
            SessionFilter::default(),
            "while the archive-excluding view with nothing typed IS the default, so Clear has \
             nothing to offer there — the fact that makes the comparison above a real one"
        );
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
            include_archived: true,
            host: Some(7),
            parent: "session/root".to_string(),
            directory: "/srv/my project".to_string(),
            profile: "claude code".to_string(),
            status: "waiting".to_string(),
            title: "a&b".to_string(),
        };
        assert!(filter.narrows_beyond_archive());
        assert_eq!(
            filter.query(),
            "include_archived=true&host=7&parent=session%2Froot&directory=%2Fsrv%2Fmy%20project&\
             profile=claude%20code&status=waiting&title=a%26b"
        );
    }

    /// Every listing request names its order, unfiltered ones included, and
    /// names it in the helm's own vocabulary.
    ///
    /// The always half is what makes the sidebar's control mean anything: the
    /// helm answers a request that names no order with `created`, so a UI
    /// whose default was "send nothing" would show creation order while its
    /// control said "recently active". The vocabulary half is why the words
    /// are asserted literally rather than round-tripped through
    /// [`ListSort::from_key`] — a round trip agrees with itself no matter
    /// which words it invented, and an invented one is a 400 at the helm.
    #[test]
    fn every_listing_request_names_its_order() {
        assert_eq!(
            list_query(&SessionFilter::default(), ListSort::default()),
            "sort=activity",
            "the default view is still an explicit request for activity order"
        );
        assert_eq!(
            list_query(&SessionFilter::default(), ListSort::Created),
            "sort=created"
        );
        assert_eq!(
            list_query(&SessionFilter::default(), ListSort::Title),
            "sort=title"
        );

        // Alongside a filter, the order leads and the filter's own
        // parameters follow unchanged — the two are independent dimensions
        // of one request (see `ListSort`), not one merged query.
        let searched = SessionFilter {
            title: "needle".to_string(),
            ..SessionFilter::default()
        };
        assert_eq!(
            list_query(&searched, ListSort::Title),
            "sort=title&title=needle"
        );

        // And the words survive a round trip, so a value written to storage
        // under one build is read back as the same order by the next.
        for sort in [ListSort::Activity, ListSort::Created, ListSort::Title] {
            assert_eq!(ListSort::from_key(sort.key()), Some(sort));
        }
        assert_eq!(
            ListSort::from_key("recent"),
            None,
            "a word this build does not know must not resolve to some other order"
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
            include_archived: true,
            title: String::new(),
            ..SessionFilter::default()
        };
        assert!(
            !blank.narrows_beyond_archive(),
            "an empty box filters nothing"
        );

        let spaced = SessionFilter {
            include_archived: true,
            title: " ".to_string(),
            ..SessionFilter::default()
        };
        assert!(spaced.narrows_beyond_archive());
        assert_eq!(
            spaced.query(),
            "include_archived=true&title=%20",
            "a space is a search for a space, not a cleared filter"
        );

        // And the host dimension, whose emptiness is an absent id rather
        // than an empty string — including id 0, which is a value and not a
        // blank.
        assert!(
            SessionFilter {
                include_archived: true,
                host: Some(0),
                ..SessionFilter::default()
            }
            .narrows_beyond_archive()
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

/// The preference write queue's ordering and replay contract, pinned on the
/// pure state machine so no runtime or network is involved.
#[cfg(test)]
mod preference_write_tests {
    use super::{PreferenceField, PreferenceWrites};

    /// Same-field writes are serialized latest-wins: a value recorded while
    /// a PUT is out is sent AFTER that PUT settles, and the settled PUT's
    /// older value never releases the field as the final answer.
    ///
    /// This is the whole reason the queue exists (review F1): independent
    /// spawned requests can finish in reverse order, and the older value
    /// finishing last would win the helm's row while the page showed the
    /// newer one — invisible until a reload or another client read it back.
    #[test]
    fn a_newer_value_recorded_mid_flight_is_sent_after_and_wins() {
        let mut queue = PreferenceWrites::default();
        assert!(
            queue.record(PreferenceField::Sort, "created".to_string()),
            "the first write starts a writer"
        );
        assert_eq!(
            queue.next_to_send(PreferenceField::Sort).as_deref(),
            Some("created")
        );

        assert!(
            !queue.record(PreferenceField::Sort, "title".to_string()),
            "a write while one is in flight must NOT start a second writer"
        );
        assert!(
            queue.finished(PreferenceField::Sort, "created", true),
            "settling the older value must send the writer around again"
        );
        assert_eq!(
            queue.next_to_send(PreferenceField::Sort).as_deref(),
            Some("title"),
            "and what it sends next is the newest value"
        );
        assert!(!queue.finished(PreferenceField::Sort, "title", true));
        assert_eq!(
            queue.dirty(PreferenceField::Sort),
            None,
            "an acknowledged newest value is clean — the helm's row is authority again"
        );
    }

    /// The two fields are independent queues: a selection write neither
    /// waits behind nor is reordered against a sort write.
    #[test]
    fn the_two_fields_do_not_share_a_writer() {
        let mut queue = PreferenceWrites::default();
        assert!(queue.record(PreferenceField::Sort, "title".to_string()));
        assert!(
            queue.record(PreferenceField::Selected, "session-1".to_string()),
            "a selection write starts its own writer even with a sort write in flight"
        );
    }

    /// A failed write leaves the value DIRTY and the field free: no retry
    /// loop (fire-and-forget), but the choice is not forgotten — it is what
    /// the post-reauthentication seed overlays and replays (review F2).
    #[test]
    fn a_failed_write_stays_dirty_and_is_claimed_exactly_once_for_replay() {
        let mut queue = PreferenceWrites::default();
        assert!(queue.record(PreferenceField::Selected, "session-9".to_string()));
        assert!(!queue.finished(PreferenceField::Selected, "session-9", false));

        assert_eq!(
            queue.dirty(PreferenceField::Selected).as_deref(),
            Some("session-9"),
            "the unpersisted choice must survive for the replay"
        );
        assert_eq!(
            queue.begin_replay(PreferenceField::Selected).as_deref(),
            Some("session-9")
        );
        assert_eq!(
            queue.begin_replay(PreferenceField::Selected),
            None,
            "a second gate mount must not start a rival writer for the same field"
        );
        assert!(!queue.finished(PreferenceField::Selected, "session-9", true));
        assert_eq!(queue.dirty(PreferenceField::Selected), None);
    }

    /// A cancelled writer releases the field but keeps the value dirty, so
    /// a later write starts a fresh writer instead of queueing forever
    /// behind a task that no longer exists.
    #[test]
    fn a_lost_writer_releases_the_field_without_forgetting_the_value() {
        let mut queue = PreferenceWrites::default();
        assert!(queue.record(PreferenceField::Sort, "title".to_string()));
        queue.writer_lost(PreferenceField::Sort);
        assert_eq!(queue.dirty(PreferenceField::Sort).as_deref(), Some("title"));
        assert!(
            queue.record(PreferenceField::Sort, "created".to_string()),
            "the next write must be able to start a replacement writer"
        );
    }

    /// A clean field yields nothing to replay: after an acknowledged write,
    /// the fetched row (possibly another client's newer choice) wins.
    #[test]
    fn an_acknowledged_field_is_not_overlaid_onto_a_fetched_seed() {
        let mut queue = PreferenceWrites::default();
        assert!(queue.record(PreferenceField::Sort, "title".to_string()));
        assert!(!queue.finished(PreferenceField::Sort, "title", true));
        assert_eq!(queue.dirty(PreferenceField::Sort), None);
        assert_eq!(queue.begin_replay(PreferenceField::Sort), None);
    }
}
