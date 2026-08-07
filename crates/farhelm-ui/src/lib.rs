//! The Farhelm UI: one Dioxus component tree, two targets.
//!
//! The same components render as the web app (wasm32, real DOM, served
//! by the helm at loopback) and the desktop app (wry webview). The
//! terminals themselves are xterm.js islands (assets/terminal.js) whose
//! byte paths bypass Dioxus entirely — Dioxus owns the chrome around a
//! terminal, never its content (SPEC_impl.md, "Terminal widget"). A
//! session view holds SEVERAL of those islands at once, all attached
//! concurrently (terminal tabs, PLAN_M4.md); tabs multiply the
//! boundary's instances, never the boundary itself (see [`SessionView`]).
//!
//! Data fetching uses reqwest, which works on both native (desktop) and
//! wasm (browser fetch) — one code path, no per-target HTTP client.
//!
//! ## Navigation (M2, PLAN_M2.md step 7)
//!
//! `App` holds `Signal<Option<Session>>` rather than pulling in a router
//! crate: `None` renders [`ListView`], `Some(session)` renders
//! [`SessionView`] plus a back control that clears the signal.
//! PLAN_M2.md named a premature router as a risk to avoid — two states
//! and one signal cover everything needed so far, and a router can still
//! be introduced when something actually demands one. Terminal tabs did
//! not: a tab selection is view-local state, not a location, and nothing
//! links to a specific tab.
//!
//! ## Module layout
//!
//! This file keeps only what every module needs: the wire-mirror types
//! (`Session`, `SessionStatus`, `RestartOffer`, `Tab`), the `ApiBase`
//! context type, and `App` itself. Everything else is split by concern so
//! each piece can be read (and tested) without the others' unrelated
//! details in view:
//!
//! - `api`: every `async fn` that calls the helm's HTTP API, the
//!   response-shape types those calls decode, the URL-building helpers,
//!   the session-list filter both the query surface and the readers share,
//!   and the cadence the fallback polls run at.
//! - `feed`: the invalidation feed (PLAN_M6_75.md item 6) — the App-level
//!   subscription that replaced all four periodic loops, the revision
//!   counter the mounted page re-reads on, and the rule deciding when the
//!   documented poll fallback runs instead. Mounted here, beside the skew
//!   notice, because the channel has to survive navigation between the two
//!   mutually exclusive pages below.
//! - `list`: [`ListView`], `SessionRow`, and `CreateSessionForm` — the
//!   session list and its lifecycle actions.
//! - `hosts`: the hosts panel (PLAN_M6.md item 6) — the per-host state
//!   chips SPEC.md requires to be always visible, the add/edit/remove/
//!   adopt/retry management surface, and the renderer-free wording helpers
//!   that turn one `HostPhase` into the sentences a user acts on (shared
//!   with `session_view`, which puts the same wording behind a stale
//!   session's notice).
//! - `profiles`: agent profiles as this UI handles them (PLAN_M6_75.md item
//!   8) — the per-host catalog read every profile surface goes through, the
//!   hosts panel's profiles section (create/edit/delete), and the
//!   renderer-free rules the create dialog's picker needs: which profile a
//!   fresh dialog preselects, when it must ask instead of guessing, and how
//!   a session's snapshotted profile reads once the catalog has moved on.
//! - `peer`: how text this UI did not write is SHOWN — the escape rule for
//!   invisible and directional characters, and the run split that keeps each
//!   peer value in its own direction-isolated element. Used by every surface
//!   that mixes our words with a host's, a supervisor's, or the helm's, which
//!   is why it is not part of `hosts`.
//! - `rows`: what the session list SHOWS, derived from what the helm sent —
//!   the optimistic-rename overlay and the pruning rule that retires it, plus
//!   the count banner's wording. Pure functions of a listing reply, so `list`
//!   is left holding only the component, the state, and the handlers.
//! - `status`: what a session's status SAYS — the badge both the list and
//!   the session view render it as, and the consequence sentence a delete
//!   confirmation opens with. Wording only; what a status MEANS is
//!   `SessionStatus`'s own business, right here.
//! - `tabs`: the tab-domain half of terminal tabs (PLAN_M4.md item 6) —
//!   the renderer-free derivations (which tabs to show, their labels and
//!   DOM ids, the WebSocket path) plus the strip's one presentational
//!   piece, `TabStripItem`, and the close-confirmation wording.
//! - `reconnect`: the terminal auto-reconnect domain (PLAN_M6.md item 7) —
//!   the backoff ladder, the boundary between active retries and background
//!   probing, the heartbeat's timings, and the wording a recovering
//!   terminal shows, serialized into the page for terminal.js to apply. The
//!   same split as `attachments`, for the same reason: the decisions are
//!   Rust's and unit-testable, the socket handling is not.
//! - `skew`: the client↔helm build-stamp check (PLAN_M6.md item 6) — the
//!   comparison itself plus the one signal `App` renders its reload prompt
//!   from, so a tab left open across a helm upgrade says so instead of
//!   failing in ways nothing explains.
//! - `auth`: the full-page bootstrap-token exchange (PLAN_M7.md item 3),
//!   raised by `api`'s typed 401 funnel and unmounted after exchange so every
//!   authenticated reader starts again from a clean state.
//! - `rename`: `RenameForm`, the one control both rename surfaces share
//!   (PLAN_M5.md item 6) — a single-line field that sends what the user
//!   typed verbatim, with the request, the optimistic paint, and the
//!   refusal text left to whichever surface mounted it.
//! - `attachments`: the attachment domain of paste/drop interception
//!   (PLAN_M4.md item 7) — the classification rule, the naming rule, the
//!   upload endpoint, and the wording of every message a transfer can put
//!   on screen, serialized into the page for terminal.js to apply (see
//!   that module's header for why the runtime path cannot live in Rust).
//! - `session_view`: [`SessionView`] itself, the stateful component that
//!   owns one session's terminals, restart affordance, and tab lifecycle,
//!   calling into `api` for I/O and `tabs`/`attachments` for the pure
//!   derivations.
//!
//! All of them are private modules with `pub(crate)` entry points: nothing
//! outside this crate has a legitimate reason to reach into any of them,
//! so `main.rs` only ever sees `App`/`ApiBase`, both defined and exported
//! here.

use dioxus::prelude::*;
use serde::Deserialize;

mod api;
mod archive;
mod attachments;
mod auth;
mod feed;
mod hosts;
mod list;
mod ops;
mod peer;
mod profiles;
mod reader;
mod reconnect;
mod rename;
mod rows;
mod session_view;
mod skew;
mod status;
mod tabs;

use list::ListView;
use session_view::SessionView;

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
    ///
    /// **Never rendered as a badge** (PLAN_M6_75.md item 3): `status`'s
    /// wording module returns no badge at all for this variant, so a row
    /// whose status nothing has classified yet simply shows none rather
    /// than a word that reads like a verdict. This is the UI half of a
    /// two-part rule — the helm's never-overwrite-definite merge covers the
    /// restart case, where a prior classification still exists to keep on
    /// screen; this covers the create case, where none does.
    #[default]
    Unknown,
    /// The agent is alive and appears to be working — and the status a
    /// live session carries whenever nothing more specific has been
    /// established.
    ///
    /// Two of those cases are worth knowing when reading a screen: a
    /// session the supervisor has not yet sampled twice (it has no
    /// comparison to draw a conclusion from, and a just-launched agent is
    /// working), and one whose screen changed at its last sample. See
    /// farhelm-proto's `SessionStatus::Running` for the full contract; the
    /// rule that matters here is that all three live statuses are
    /// heuristic and cosmetic, so nothing in this UI may gate on which one
    /// it is.
    Running,
    /// The agent is alive and appears to be blocked on the user — a
    /// detected question or approval prompt with no answer yet. The status
    /// SPEC.md's fleet list exists to surface.
    Waiting,
    /// The agent is alive and at rest. Distinct from `Exited`: the session
    /// is still there and still takes input.
    Idle,
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

impl SessionStatus {
    /// Whether the agent behind this session is still running.
    ///
    /// A predicate rather than an equality against one live variant at each
    /// site, and that is about the SHAPE of this enum, not about brevity.
    /// M6.75's status work replaced the single live status with a
    /// running/waiting/idle discrimination (PLAN_M6_75.md item 3) — three
    /// statuses that are ALL live, differing only in what the agent is
    /// doing. That day has arrived, and this predicate is why it cost one
    /// edit here instead of a hunt for every `== Alive` in the tree, each of
    /// which would have quietly started answering `false` for a session that
    /// is very much alive, with nothing failing to compile to say so. The
    /// same argument holds unchanged for the next status ever added.
    ///
    /// Written as an exhaustive `match` rather than a `matches!`, and that is
    /// the half that actually holds the line: `matches!` would send every
    /// future variant to `false` by default, which is the exact silent
    /// mis-answer this predicate exists to prevent — it would move the trap
    /// rather than remove it. Spelling out every arm makes a new status a
    /// compile error here, and here is where the decision belongs.
    ///
    /// The motivating site is `session_view`'s restart gate, which decides
    /// whether a restart click opens a confirmation or restarts outright.
    /// A stale `false` there would restart a live agent WITHOUT asking —
    /// killing it — which is precisely what the confirmation exists to
    /// prevent.
    ///
    /// `Unknown` is deliberately NOT live. SPEC.md's no-guessing rule says
    /// an unresolved status is presented as uncertain, and rounding it up
    /// to a liveness claim is precisely the guess that rule forbids.
    pub(crate) fn is_live(&self) -> bool {
        match self {
            SessionStatus::Running | SessionStatus::Waiting | SessionStatus::Idle => true,
            SessionStatus::Unknown
            | SessionStatus::Exited { .. }
            | SessionStatus::Interrupted
            | SessionStatus::Error { .. } => false,
        }
    }

    /// Whether the agent behind this session is definitively finished —
    /// exited, lost to a reboot, or never started at all.
    ///
    /// The complement of [`SessionStatus::is_live`] over the KNOWN states,
    /// not its logical negation: `Unknown` is neither, for the same
    /// no-guessing reason. Callers that must do something for every status
    /// therefore still need a third branch, which is the point — a status
    /// nobody has resolved yet is a real case, and a two-way split would
    /// quietly file it under one of the answers.
    ///
    /// Exhaustive for the same reason `is_live` is, with one extra edge: a
    /// default-`false` here reads as "not finished", which for a delete
    /// confirmation means a new status would start prompting rather than
    /// silently skipping the prompt. That is the safe direction, which is
    /// exactly why it would go unnoticed — so this side gets the compile
    /// error too, not just the side whose failure is loud.
    ///
    /// Existed alongside `is_live` before the M6.75 status split precisely
    /// so that split would be a single edit for BOTH questions rather than
    /// one that fixed the live-side call sites and left every ended-side
    /// match quietly stale. It was, and the pair stays for the same reason.
    pub(crate) fn has_ended(&self) -> bool {
        match self {
            SessionStatus::Exited { .. }
            | SessionStatus::Interrupted
            | SessionStatus::Error { .. } => true,
            SessionStatus::Running
            | SessionStatus::Waiting
            | SessionStatus::Idle
            | SessionStatus::Unknown => false,
        }
    }
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
    /// Whether archive has deliberately removed this session's processes
    /// and terminal while retaining the conversation metadata and committed
    /// attachments.
    /// Missing on older helm replies, where no session could be archived.
    #[serde(default)]
    pub archived: bool,
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
    /// frame rather than after a round trip), and the feed-driven detail
    /// read is what keeps it current afterwards.
    ///
    /// `#[serde(default)]` for the same old-peer tolerance as `status` —
    /// and, unlike `status`, the default is also the everyday case: a
    /// session with no tabs.
    #[serde(default)]
    pub tabs: Vec<Tab>,
    /// Which registered host this session lives on — the join key into the
    /// hosts panel, and what a create alongside it names (PLAN_M6.md item
    /// 5).
    ///
    /// `Option` because only the LIST routes carry it: `POST /api/sessions`
    /// answers with the bare `SessionInfo` the supervisor produced, on the
    /// helm's own reasoning that the caller already knows which host it
    /// asked for. The create path therefore fills this in from what it
    /// selected (see `list::CreateSessionForm`) rather than reading it back.
    pub host: Option<HostId>,
    /// The host's display name as the helm renders it, denormalized onto
    /// the row so a list can name every session's host without a second
    /// request. `None` for the same reason `host` is.
    pub host_name: Option<String>,
    /// Whether this row is the helm's LAST-KNOWN knowledge rather than a
    /// live report — true for every session of a host in any non-connected
    /// state.
    ///
    /// SPEC.md requires such sessions to stay listed and be clearly marked,
    /// which is what the row's stale badge and the session view's notice are
    /// built on. `#[serde(default)]` to `false` is the safe direction for
    /// the same reason `status` defaults to `Unknown`: a reply that says
    /// nothing must not have staleness invented for it, and a live session
    /// wrongly marked stale would hide its terminal.
    #[serde(default)]
    pub stale: bool,
    /// The profile this session was CREATED from, as the session itself
    /// remembers it (PLAN_M6_75.md item 3), or `None` for a raw-invocation
    /// create.
    ///
    /// Absent is the ordinary case rather than a gap — every session made by
    /// typing a command has no profile — and serde decodes a missing key on
    /// an `Option` as `None`, which is also what a helm predating the field
    /// sends. Both readings agree: no profile is known for this session.
    pub source_profile: Option<SourceProfile>,
}

/// Mirror of the helm's `source_profile` object (farhelm-proto's
/// `SourceProfile`): which profile a session was created from, as the session
/// snapshotted it, plus what a catalog lookup on that id found when the reply
/// was built.
///
/// The split is the whole contract and this UI must not blur it. `id` and
/// `name` are IMMUTABLE — the choice as it was made — which is what keeps a
/// list stable and filterable no matter what later happens to the catalog,
/// while `existence` is recomputed for every reply and is therefore the only
/// part that can change under a session nobody touched. Rendering the name is
/// how SPEC.md's snapshot rule ("editing or deleting a profile affects future
/// sessions only") becomes visible; rendering the existence beside it is what
/// stops that name from reading as a claim about the catalog as it stands
/// now.
///
/// Note what is deliberately NOT here: the profile's CURRENT name. A renamed
/// profile's new name is knowable server-side and is withheld on purpose, so
/// that there is exactly one copy of existence truth — a surface that needs
/// today's name (the profiles section) reads the catalog, where it is
/// authoritative.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SourceProfile {
    /// The profile's immutable identity as snapshotted at creation: opaque,
    /// never parsed, and never sent anywhere by this client.
    ///
    /// It is what the HELM derived `existence` from when it built this reply,
    /// and what its filter matches server-side — the list's own profile filter
    /// is free text the user types (a deleted profile's snapshotted NAME has
    /// to remain searchable), so nothing here ever echoes this value back.
    pub id: String,
    /// The profile's name AS SNAPSHOTTED — not its current name.
    pub name: String,
    /// What the catalog held for `id` when this reply was built.
    pub existence: ProfileExistence,
}

/// Mirror of the helm's `existence` vocabulary (farhelm-proto's
/// `ProfileExistence`).
///
/// Tolerant of a word this build has never heard of, on the same terms as
/// [`HostPhase::Unrecognized`] and for a sharper reason: this value rides on
/// every session row, so a strict decode would turn one unknown existence
/// state into an empty session LIST rather than into one row that says less
/// than it might have.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileExistence {
    /// The catalog still holds this id under the snapshotted name.
    Present,
    /// The catalog still holds this id, under a DIFFERENT name.
    Renamed,
    /// No profile with this id is in the catalog any more.
    Deleted,
    /// A state this build does not know. Deliberately not a `Default`:
    /// nothing here may reach for an existence it was not given.
    #[serde(other)]
    Unrecognized,
}

/// Mirror of one entry in a host's profile catalog (farhelm-proto's
/// `Profile`): a named, editable definition of how to launch an agent, and
/// how to resume one.
///
/// Per-supervisor by construction — an id minted on one host means nothing on
/// another (SPEC.md leaves profile syncing to post-v1) — which is why every
/// surface here holds a catalog together with the host it came from, and why
/// the create dialog drops a profile choice when its target host changes.
///
/// `agent_kind` stays the wire STRING rather than becoming an enum, unlike
/// [`HostKind`], and the difference is what the value is FOR: a kind decides
/// nothing in this UI (the supervisor selects its own heuristics from it) and
/// is only displayed and echoed back on an edit. An enum with an
/// `Unrecognized` catch-all would lose the actual word, so editing a profile
/// whose kind a newer helm introduced would silently rewrite it to something
/// else — the one outcome an editor must never produce.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Profile {
    /// Supervisor-minted and stable across every rename: what a create names
    /// and what a session's [`SourceProfile`] snapshots.
    pub id: String,
    /// The user's label, as the CATALOG holds it today: what the profiles
    /// section lists and what the create dialog's picker offers.
    ///
    /// Deliberately not what a session row shows. A row renders the name its
    /// session SNAPSHOTTED ([`SourceProfile::name`]), which this one can have
    /// diverged from at any time — that divergence is SPEC.md's snapshot rule
    /// working, and rendering today's label on a historical row would be the
    /// rewrite the rule forbids.
    pub name: String,
    /// The launch command line, shell-split by the supervisor exactly as a
    /// raw create's invocation is.
    pub invocation: String,
    /// Which integrated agent this profile IS, in the wire's own spelling
    /// (`claude`, `codex`, `generic`) — see the type docs for why it stays a
    /// string.
    pub agent_kind: String,
    /// The resume invocation as an argv vector, or absent. What absence means
    /// is per kind (an integrated kind derives one, a generic one gets none),
    /// which is the supervisor's rule and not this UI's.
    pub resume_template: Option<Vec<String>>,
}

/// The helm's registry id for one host (farhelm-helm's `store::HostId`).
///
/// A plain surrogate integer on the wire, opaque to this UI: it is echoed
/// back on the host verbs and on a create, never parsed or ordered.
pub type HostId = i64;

/// Mirror of the helm's `GET /api/hosts` row (farhelm-helm's `HostView`,
/// PLAN_M6.md item 5): the registry's own facts plus the live connection
/// state that makes them actionable.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Host {
    pub id: HostId,
    pub kind: HostKind,
    /// `None` for the local row, always present for an ssh row.
    pub destination: Option<String>,
    /// The helm's own rendering of the host's name — the same string
    /// session rows carry in `host_name`, so a row and its chip never
    /// disagree about what a host is called.
    pub name: String,
    /// The identity the REGISTRY holds, not what the host is reporting. The
    /// two differ exactly while an identity decision is pending, and the
    /// reported one rides `state` — see [`HostPhase::IdentityMismatch`],
    /// whose `reported` is what an adopt must name.
    pub identity: Option<String>,
    pub remote_farhelm: Option<String>,
    pub remote_state_dir: Option<String>,
    pub state: HostPhase,
    /// Which CONNECTION this host is on — the helm's own opaque, monotonic
    /// token, which changes whenever the host's client does (a retarget, an
    /// adoption, a reconnection, or the connection going away).
    ///
    /// Compared and echoed, never interpreted: it is handed back as
    /// `expected_incarnation` on every profile mutation and profile-backed
    /// create this UI sends, so the helm can refuse a request prepared against
    /// an install that has since been replaced (farhelm-helm's `precondition`
    /// module). `0` means never connected, which is nothing to assert — and
    /// `#[serde(default)]` decodes a helm that predates the field to exactly
    /// that, so an older helm simply gets requests carrying no expectation,
    /// which is the behavior it already has.
    ///
    /// Never persisted anywhere: the number is a counter over one helm
    /// process's connections and means nothing across a restart.
    #[serde(default)]
    pub incarnation: u64,
}

/// Which kind of registry row a host is (farhelm-helm's `HostKind`, as the
/// `kind` string on the wire).
///
/// An enum rather than the raw string, because the distinction is not
/// cosmetic: it is what decides whether a row offers edit and remove at all.
/// A magic-string comparison spread across two modules is one typo away from
/// offering the reserved local row a remove button the helm then refuses.
///
/// `Unrecognized` follows [`HostPhase`]'s forward-compatibility pattern for
/// the same reason: a kind this build has never heard of must cost that ONE
/// row its management controls, not cost the panel every host's connection
/// state. It is deliberately treated as unmanageable — offering verbs for a
/// row whose nature is unknown is exactly the guess this UI does not make.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    /// The helm's own machine: the one reserved row, always present, never
    /// registered, and never editable or removable.
    Local,
    /// A registered ssh destination.
    Ssh,
    #[serde(other)]
    Unrecognized,
}

/// Mirror of one host's connection state (farhelm-helm's `HostStateView`).
///
/// The `phase` tag's values are the helm's own stable vocabulary — the same
/// strings its log lines and its refusal sentences use — so the chips this
/// renders read identically to an error a user compares them against. That
/// is why the chip text IS the phase string rather than a prettier synonym
/// invented here.
///
/// ## Decode posture: strict fields, tolerant vocabulary
///
/// The two halves pull in opposite directions and both are deliberate.
///
/// The VOCABULARY is tolerant: an unrecognized phase decodes as
/// [`HostPhase::Unrecognized`] (serde's `other`, legal on a unit variant of
/// an internally tagged enum) rather than failing. SPEC.md promises that
/// per-host connection state is always visible, and a serde error anywhere
/// in the list takes the WHOLE panel down — so a host in a state this build
/// has never heard of costs exactly one chip.
///
/// The FIELDS are strict: no `#[serde(default)]` on a payload the helm
/// always sends. Defaulting them looked like more of the same tolerance and
/// is the opposite: a missing `peer_protocol` would render as "protocol 0",
/// a missing `twin` as "host 0", and a missing `reported` as an adopt button
/// approving the empty identity — fabricated facts, presented with the same
/// confidence as real ones, on the surface whose entire job is to be
/// believable. A truncated or reshaped reply is a FAILED READ (see
/// `hosts::HostsRead`), which the panel shows as such while keeping the last
/// snapshot it trusts.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "phase")]
pub enum HostPhase {
    /// Inside the active-retry window: attempts are happening right now, so
    /// no user action is called for.
    #[serde(rename = "connecting")]
    Connecting {
        attempt: u32,
        /// Absent before the first attempt has failed — genuinely optional
        /// on the wire, unlike the fields around it.
        last_error: Option<String>,
    },
    /// The active window is spent and background re-probing continues
    /// forever, so this host comes back by itself once it is back.
    #[serde(rename = "unreachable-reprobing")]
    Unreachable {
        /// `"local-supervisor-not-running"` is the one cause with a remedy
        /// on the machine the user is already sitting at, and the panel's
        /// manual-start hint keys off exactly this string.
        cause: String,
        last_error: String,
    },
    #[serde(rename = "connected")]
    Connected {
        /// Genuinely optional: a supervisor may report no identity at all.
        identity: Option<String>,
        build_version: String,
        refresh: RefreshHealth,
    },
    /// Refused at the hello. Both versions are named so the user can see
    /// WHICH side is behind, and `remediation` is the helm's own sentence to
    /// act on — rendered verbatim rather than paraphrased here.
    #[serde(rename = "version-skew")]
    VersionSkew {
        peer_protocol: u32,
        peer_build: String,
        our_protocol: u32,
        our_build: String,
        remediation: String,
    },
    /// Frozen awaiting a user decision: adopt `reported`, or fix the
    /// destination. `reported` is the value an adopt request must carry —
    /// the helm refuses an adopt that names anything else, so that a
    /// re-probe landing between the display and the click cannot silently
    /// adopt a third install.
    #[serde(rename = "identity-mismatch")]
    IdentityMismatch { recorded: String, reported: String },
    /// Frozen because the destination answered with NO identity while this
    /// entry has one on record — so there is nothing to compare and,
    /// deliberately, nothing to adopt.
    ///
    /// A renderer must not offer the adopt verb here: the helm would refuse
    /// it, and the offer itself would misdescribe what is on the table. The
    /// remedies are fixing the host so it identifies itself, retargeting the
    /// entry, or removing it — and the state re-probes itself, so a host
    /// that starts identifying again recovers unaided.
    #[serde(rename = "identity-unverified")]
    IdentityUnverified { recorded: String },
    /// This ENTRY reaches a host another entry already owns. The host itself
    /// is listed exactly once, under `twin`; this row exists so the
    /// duplicate entry can be edited or removed.
    #[serde(rename = "duplicate")]
    Duplicate { twin: HostId, identity: String },
    /// No connection actor is running for this row. Reported rather than
    /// hidden, because an operation refused against it has to have something
    /// honest to name — and because retry is what brings it back.
    #[serde(rename = "retired")]
    Retired { reason: String },
    /// A phase this build does not know. Not a state the helm can be in —
    /// it is what a UI one version behind sees, and it exists so that host
    /// costs the rest of the panel nothing (see the type's own docs).
    ///
    /// Deliberately NOT a `Default`: nothing in this UI may reach for a
    /// connection state it was not given, and a defaultable one invites
    /// exactly that.
    #[serde(other)]
    Unrecognized,
}

/// Mirror of a connected host's last cache refresh (farhelm-helm's
/// `RefreshView`).
///
/// Beside the connection rather than inside it, exactly as the helm models
/// it: a failed refresh does not disconnect a host, and collapsing the two
/// would make a host that is answering perfectly well read as unreachable.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum RefreshHealth {
    /// Connected; the first refresh has not landed yet.
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "ok")]
    Ok { sessions: u64 },
    /// The last refresh failed while the PREVIOUS cache stayed in place, so
    /// this host's sessions are still listed.
    #[serde(rename = "failed")]
    Failed { error: String },
    /// Same tolerance posture as [`HostPhase::Unrecognized`], one level
    /// down: an unknown refresh status must not cost the panel the host's
    /// connection state, which is the part SPEC.md requires to be visible.
    /// Not a `Default`, for the reason recorded there.
    #[serde(other)]
    Unrecognized,
}

/// Mirror of the helm's tab JSON (farhelm-proto `TabInfo`): an opaque,
/// supervisor-minted id and nothing else.
///
/// Deliberately as minimal as the wire type. SPEC.md gives tabs no names
/// and close is their only operation, so an id is the whole identity —
/// labels are positional and computed at render time (see
/// `tabs::tab_label`). The id is echoed back verbatim on the terminal
/// WebSocket's `?tab=` and on the close request; this UI never parses it.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Tab {
    pub id: String,
}

const VENDOR_XTERM_CSS: Asset = asset!("/assets/vendor/xterm.css");
const VENDOR_XTERM_JS: Asset = asset!("/assets/vendor/xterm.js");
const VENDOR_FIT_JS: Asset = asset!("/assets/vendor/addon-fit.js");
// The `onBinary` byte-conversion helper terminal.js calls into (PLAN_M6_5.md
// item 1) — a separate asset, registered ahead of terminal.js, purely so
// `node --test` can load this exact file rather than a copy of its logic.
// Registration order is not execution order (script injection is async);
// terminal.js's mount readiness gate waits for the helper's global.
const TERM_BYTES_JS: Asset = asset!("/assets/term-bytes.js");
const TERMINAL_JS: Asset = asset!("/assets/terminal.js");
// The invalidation feed's socket (PLAN_M6_75.md item 6) — its own asset
// rather than a corner of terminal.js, because it has nothing to do with a
// terminal: it outlives every island, carries no bytes, and is subscribed
// once for the whole page. Registration order is not execution order here
// either; `feed::FleetFeed`'s snippet waits for the global this file
// assigns.
const EVENTS_JS: Asset = asset!("/assets/events.js");
const APP_CSS: Asset = asset!("/assets/app.css");

/// Root component: switches between the session list and one open
/// terminal. No router crate (see the module docs) — just a signal.
///
/// ## No create default travels through here
///
/// SPEC.md's creation default is "the host of the currently open session,
/// else the helm's own host", and for a while this component derived the
/// first clause and passed it to `ListView`. It could never fire: the list
/// and a session are mutually exclusive views here, so whenever the create
/// dialog exists, nothing is open. The plumbing was unreachable code with a
/// live-looking test around it, which is worse than an absent feature.
///
/// `list::default_create_host` now answers with the local row and records
/// what to restore — and, importantly, what NOT to: a remembered
/// last-viewed host is a different rule wearing this one's clothes, since a
/// session the user backed out of is not open.
#[component]
pub fn App() -> Element {
    let mut current = use_signal(|| None::<Session>);
    let build_skew = skew::build_skew_detected();
    let token_required = *auth::TOKEN_REQUIRED.read();

    rsx! {
        document::Link { rel: "stylesheet", href: VENDOR_XTERM_CSS }
        document::Link { rel: "stylesheet", href: APP_CSS }
        document::Script { src: VENDOR_XTERM_JS }
        document::Script { src: VENDOR_FIT_JS }
        document::Script { src: TERM_BYTES_JS }
        document::Script { src: TERMINAL_JS }
        document::Script { src: EVENTS_JS }
        // Above both views and outside the match, deliberately: a build
        // mismatch is a fact about this whole PAGE rather than about
        // whatever it happens to be showing, and it must not disappear
        // because the user navigated into a session while reading it.
        skew::BuildSkewNotice {}
        if token_required && !build_skew {
            auth::TokenPrompt {}
        } else {
            // Beside it, and outside the match for a related reason: the
            // invalidation feed is the whole page's channel, and a subscription
            // owned by either view below would be torn down and re-handshaked
            // every time the user opened or closed a session — turning
            // navigation into a window with no live updates and a fallback poll
            // spinning back up to cover it. It renders nothing (PLAN_M6_75.md
            // item 6); what it produces is the revision counter each page
            // re-reads on.
            feed::FleetFeed {}
            match &*current.read() {
                None => rsx! {
                    ListView { on_open: move |session: Session| current.set(Some(session)) }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tab with the given id — the whole of `Tab`, which is why this is
    /// a one-liner rather than a builder.
    fn tab(id: &str) -> Tab {
        Tab { id: id.into() }
    }

    /// Every `SessionStatus` variant, once, with the answer both predicates
    /// owe it.
    ///
    /// A table rather than scattered asserts so that adding a status forces
    /// a row here — the compiler already forces the two `match`es to grow,
    /// and this is what forces someone to say out loud what the new status
    /// MEANS rather than picking whichever arm compiles.
    fn status_truth_table() -> Vec<(SessionStatus, bool, bool)> {
        vec![
            // All three live statuses answer alike, which is the property
            // that matters: they differ in what the agent is DOING, never
            // in whether it is there, and a predicate that told them apart
            // would leak a cosmetic distinction into a destructive gate.
            (SessionStatus::Running, true, false),
            (SessionStatus::Waiting, true, false),
            (SessionStatus::Idle, true, false),
            (SessionStatus::Unknown, false, false),
            (SessionStatus::Exited { exit_code: Some(0) }, false, true),
            (SessionStatus::Exited { exit_code: None }, false, true),
            (SessionStatus::Interrupted, false, true),
            (
                SessionStatus::Error {
                    detail: "exec_failed".to_string(),
                },
                false,
                true,
            ),
        ]
    }

    /// Pins both predicates against every variant, because the whole reason
    /// they exist is that a WRONG answer here is silent everywhere else.
    ///
    /// `is_live` gates `session_view`'s restart confirmation: a false
    /// negative restarts a running agent without asking, killing it.
    /// `has_ended` gates `list`'s delete confirmation: a false positive
    /// deletes a live session with no prompt at all. Neither failure shows
    /// up as a crash or a failed request — the UI just quietly does the
    /// destructive thing — so the table is the only place either is caught.
    #[test]
    fn each_status_answers_both_liveness_predicates() {
        for (status, live, ended) in status_truth_table() {
            assert_eq!(
                status.is_live(),
                live,
                "{status:?} must{} be live",
                if live { "" } else { " not" }
            );
            assert_eq!(
                status.has_ended(),
                ended,
                "{status:?} must{} have ended",
                if ended { "" } else { " not" }
            );
        }
    }

    /// The two predicates are not each other's negation, and that gap is
    /// deliberate: `Unknown` answers `false` to BOTH.
    ///
    /// SPEC.md's no-guessing rule is what puts it there — an unresolved
    /// status is presented as uncertain rather than rounded toward either
    /// answer. A future refactor that "simplifies" one predicate into
    /// `!other()` would erase exactly that, and would do it silently, since
    /// every other variant agrees. Asserting the gap explicitly is what
    /// makes such a change fail here instead of in a confirmation prompt.
    #[test]
    fn an_unresolved_status_is_neither_live_nor_ended() {
        assert!(!SessionStatus::Unknown.is_live());
        assert!(!SessionStatus::Unknown.has_ended());
        // And no OTHER variant shares that gap: every resolved status
        // answers exactly one of the two.
        for (status, live, ended) in status_truth_table() {
            if status == SessionStatus::Unknown {
                continue;
            }
            assert!(
                live != ended,
                "{status:?} is resolved, so exactly one predicate must claim it"
            );
        }
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

    /// Every live status the helm can send, decoded through the REAL
    /// `Session` — the UI's half of PLAN_M6_75.md item 3's live split.
    ///
    /// This crate carries its OWN mirror of `SessionStatus` (see that
    /// type's docs for why), which means farhelm-proto's golden tests
    /// cannot see a drift here: rename a variant in this file, or drop the
    /// `rename_all`, and the proto suite stays green while every live
    /// session in the browser fails to decode and the whole listing
    /// disappears. Pinning the wire SPELLINGS against this decoder is the
    /// only place that failure is catchable.
    ///
    /// Decoded from a whole `Session` object rather than from the status
    /// alone, because the nesting is part of what a drift would break —
    /// the status arrives as a field of a listing row, never on its own.
    #[test]
    fn every_live_status_spelling_decodes_through_the_ui_mirror() {
        for (state, expected) in [
            ("running", SessionStatus::Running),
            ("waiting", SessionStatus::Waiting),
            ("idle", SessionStatus::Idle),
        ] {
            let json = serde_json::json!({
                "id": "s1",
                "title": "demo",
                "cwd": "/tmp",
                "invocation": "agent",
                "status": { "state": state },
            });
            let decoded: Session = serde_json::from_value(json)
                .unwrap_or_else(|e| panic!("the helm's `{state}` must decode here: {e}"));
            assert_eq!(decoded.status, expected);
            assert!(
                decoded.status.is_live(),
                "`{state}` is a live status; a mirror that decoded it as anything else would \
                 make the delete and restart gates lie"
            );
        }

        // And the status this UI no longer understands: `alive` was
        // REPLACED, so a helm still sending it is a version skew the build
        // stamp is supposed to have caught. Failing the decode is what
        // keeps that from being mistaken for a session with no status.
        let stale = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
            "status": { "state": "alive" },
        });
        serde_json::from_value::<Session>(stale)
            .expect_err("`alive` was replaced at PROTOCOL_VERSION 10, not kept as an alias");
    }

    /// An old-shaped `Session` JSON (no `status` field at all — exactly
    /// what a pre-M2 peer would send) must decode as `Unknown`, mirroring
    /// farhelm-proto's own decode-tolerance contract for
    /// `SessionInfo::status`. A silent default of, say, `Running` would be
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

    /// A session's source profile must decode with its snapshot INTACT and
    /// its existence separate, and a session without one must stay ordinary.
    ///
    /// Both halves are load-bearing. Absence is the common case (every
    /// raw-created session, and every session at all from a helm predating
    /// the field), so a required field here would turn the whole listing into
    /// a decode failure. And the snapshot/existence split is what the row
    /// renders: a name that came out of the reply's `existence` instead of
    /// its `name` would silently start showing the profile's CURRENT label,
    /// which is precisely the promise SPEC.md's snapshot rule makes about
    /// what an edit does not touch.
    #[test]
    fn a_sessions_source_profile_decodes_as_a_snapshot_plus_a_derived_existence() {
        let raw_created: Session = serde_json::from_value(serde_json::json!({
            "id": "s1", "title": "demo", "cwd": "/tmp", "invocation": "agent",
        }))
        .unwrap();
        assert_eq!(raw_created.source_profile, None);

        for (existence, expected) in [
            ("present", ProfileExistence::Present),
            ("renamed", ProfileExistence::Renamed),
            ("deleted", ProfileExistence::Deleted),
            // A later helm's word costs this row its existence detail and
            // nothing else — the listing still decodes.
            ("quarantined", ProfileExistence::Unrecognized),
        ] {
            let decoded: Session = serde_json::from_value(serde_json::json!({
                "id": "s1", "title": "demo", "cwd": "/tmp", "invocation": "agent",
                "source_profile": {
                    "id": "p-1", "name": "Claude Code", "existence": existence,
                },
            }))
            .unwrap_or_else(|e| panic!("`{existence}` must decode: {e}"));
            let source = decoded.source_profile.expect("the snapshot is present");
            assert_eq!(source.id, "p-1");
            assert_eq!(source.name, "Claude Code");
            assert_eq!(source.existence, expected);
        }
    }

    /// A catalog entry decodes with the kind as the WORD the helm sent, and
    /// with an absent resume template staying absent.
    ///
    /// The kind matters because an edit REPLACES a profile's whole
    /// definition: a decode that mapped an unknown kind onto some known one
    /// would make saving an untouched profile rewrite it to a different
    /// agent. Absence matters for the same reason from the other side — an
    /// absent template is a real state (the supervisor derives one per kind),
    /// not an empty list to be sent back as an explicit "no resume".
    #[test]
    fn a_profile_decodes_with_its_kind_verbatim_and_its_template_optional() {
        let profile: Profile = serde_json::from_value(serde_json::json!({
            "id": "p-1", "name": "Claude Code", "invocation": "claude",
            "agent_kind": "claude", "resume_template": ["claude", "--resume", "{conversation}"],
        }))
        .unwrap();
        assert_eq!(profile.agent_kind, "claude");
        assert_eq!(
            profile.resume_template,
            Some(vec![
                "claude".to_string(),
                "--resume".to_string(),
                "{conversation}".to_string()
            ])
        );

        let unknown_kind: Profile = serde_json::from_value(serde_json::json!({
            "id": "p-2", "name": "Something New", "invocation": "novel",
            "agent_kind": "novel-agent", "resume_template": null,
        }))
        .unwrap();
        assert_eq!(
            unknown_kind.agent_kind, "novel-agent",
            "a kind this build does not know must survive verbatim, or an edit would rewrite it"
        );
        assert_eq!(unknown_kind.resume_template, None);
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

    /// The two shapes a `Session` arrives in must BOTH decode, because they
    /// come from routes this UI calls minutes apart: a list row carries the
    /// host fields (PLAN_M6.md item 5), while a create reply deliberately
    /// does not — the helm's `create_session` answers with the bare
    /// `SessionInfo`, on the reasoning that the caller already knows which
    /// host it asked for. A required `host` would turn every successful
    /// create into a decode failure.
    #[test]
    fn a_session_decodes_with_or_without_the_multi_host_fields() {
        let create_reply = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
        });
        let decoded: Session = serde_json::from_value(create_reply).unwrap();
        assert_eq!(decoded.host, None);
        assert_eq!(decoded.host_name, None);
        assert!(
            !decoded.stale,
            "a reply that says nothing about staleness must not have it invented: a live session \
             marked stale would hide its terminal"
        );

        let list_row = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
            "host": 7,
            "host_name": "user@box",
            "stale": true,
        });
        let decoded: Session = serde_json::from_value(list_row).unwrap();
        assert_eq!(decoded.host, Some(7));
        assert_eq!(decoded.host_name.as_deref(), Some("user@box"));
        assert!(decoded.stale);
    }

    /// Every phase of the helm's connection-state vocabulary must decode
    /// with its evidence intact — the evidence IS the actionable half of
    /// each chip (both versions on skew, both identities on a mismatch, the
    /// twin on a duplicate), and a field lost in decoding would leave a
    /// user with a diagnosis and no remedy.
    #[test]
    fn every_host_phase_decodes_with_the_evidence_its_chip_renders() {
        let hosts: Vec<Host> = serde_json::from_value(serde_json::json!([
            {
                "id": 1, "kind": "local", "destination": null, "name": "this machine",
                "identity": "identity-local", "remote_farhelm": null, "remote_state_dir": null,
                "state": {
                    "phase": "connected", "identity": "identity-local",
                    "build_version": "0.1.0", "refresh": { "status": "ok", "sessions": 3 },
                },
            },
            {
                "id": 2, "kind": "ssh", "destination": "user@box", "name": "user@box",
                "identity": null, "remote_farhelm": null, "remote_state_dir": null,
                "state": {
                    "phase": "version-skew", "peer_protocol": 9, "peer_build": "0.2.0",
                    "our_protocol": 8, "our_build": "0.1.0",
                    "remediation": "update the host's farhelm binary",
                },
            },
            {
                "id": 3, "kind": "ssh", "destination": "user@twin", "name": "user@twin",
                "identity": null, "remote_farhelm": null, "remote_state_dir": null,
                "state": { "phase": "duplicate", "twin": 2, "identity": "identity-shared" },
            },
        ]))
        .unwrap();

        assert!(matches!(
            &hosts[0].state,
            HostPhase::Connected {
                refresh: RefreshHealth::Ok { sessions: 3 },
                ..
            }
        ));
        assert!(matches!(
            &hosts[1].state,
            HostPhase::VersionSkew { peer_protocol: 9, our_protocol: 8, remediation, .. }
                if remediation == "update the host's farhelm binary"
        ));
        assert!(matches!(
            &hosts[2].state,
            HostPhase::Duplicate { twin: 2, identity } if identity == "identity-shared"
        ));
    }

    /// A phase (or refresh status) this build has never heard of must cost
    /// the panel exactly one row's detail and nothing else.
    ///
    /// The failure this pins against is not hypothetical decoding pedantry:
    /// serde fails the whole `Vec<Host>` on one bad element, so without the
    /// `other` fallback a single host in a newer state would blank the ENTIRE
    /// hosts panel — the one surface SPEC.md requires to always show every
    /// host's connection state.
    #[test]
    fn an_unknown_phase_costs_one_row_rather_than_the_whole_panel() {
        let hosts: Vec<Host> = serde_json::from_value(serde_json::json!([
            {
                "id": 1, "kind": "ssh", "destination": "user@future", "name": "user@future",
                "identity": null, "remote_farhelm": null, "remote_state_dir": null,
                "state": { "phase": "quarantined", "reason": "invented by a later helm" },
            },
            {
                "id": 2, "kind": "local", "destination": null, "name": "this machine",
                "identity": null, "remote_farhelm": null, "remote_state_dir": null,
                "state": {
                    "phase": "connected", "identity": null, "build_version": "0.1.0",
                    "refresh": { "status": "reticulating" },
                },
            },
        ]))
        .unwrap();

        assert_eq!(
            hosts.len(),
            2,
            "the known host must survive the unknown one"
        );
        assert_eq!(hosts[0].state, HostPhase::Unrecognized);
        assert!(matches!(
            &hosts[1].state,
            HostPhase::Connected {
                refresh: RefreshHealth::Unrecognized,
                ..
            }
        ));
    }

    /// A kind this build does not know degrades that ONE row to
    /// unmanageable, on the same forward-compatibility terms as an unknown
    /// phase — and, critically, is not mistaken for `ssh`. Offering edit and
    /// remove for a row whose nature is unknown would be a guess about what
    /// the helm would accept.
    #[test]
    fn an_unknown_host_kind_decodes_as_unrecognized_rather_than_ssh() {
        let host: Host = serde_json::from_value(serde_json::json!({
            "id": 1, "kind": "quantum", "destination": null, "name": "odd",
            "identity": null, "remote_farhelm": null, "remote_state_dir": null,
            "state": { "phase": "connecting", "attempt": 1, "last_error": null },
        }))
        .unwrap();
        assert_eq!(host.kind, HostKind::Unrecognized);
    }

    /// A required payload field that is MISSING must fail the decode rather
    /// than default.
    ///
    /// This is the half that is easy to get backwards, because it looks like
    /// the opposite of the unknown-phase tolerance above and is in fact its
    /// complement. A defaulted `peer_protocol` renders as "protocol 0", a
    /// defaulted `twin` as "host 0", a defaulted `reported` as an adopt
    /// button approving the empty identity — fabricated facts on the one
    /// surface whose whole job is to be believable. Failing instead makes it
    /// a FAILED READ, which the panel reports as such while keeping the
    /// snapshot it still trusts (`hosts::HostsRead`).
    #[test]
    fn a_missing_required_field_fails_the_decode_rather_than_fabricating_a_value() {
        for incomplete in [
            serde_json::json!({ "phase": "version-skew", "peer_build": "b", "our_protocol": 8,
                                "our_build": "a", "remediation": "update" }),
            serde_json::json!({ "phase": "identity-mismatch", "recorded": "id-old" }),
            serde_json::json!({ "phase": "duplicate", "identity": "id" }),
            serde_json::json!({ "phase": "connected", "identity": null,
                                "build_version": "0.1.0" }),
            serde_json::json!({ "phase": "retired" }),
        ] {
            assert!(
                serde_json::from_value::<HostPhase>(incomplete.clone()).is_err(),
                "{incomplete} must not decode into fabricated values"
            );
        }
    }
}
