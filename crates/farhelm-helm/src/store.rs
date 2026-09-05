//! SQLite persistence for the helm: `helm.db` (PLAN_M6.md item 3), the
//! helm's first durable state. Deliberately mirrors
//! `farhelm-supervisor/src/store.rs`'s established shape — `PRAGMA
//! user_version` for schema versioning, a `spawn_blocking`-wrapped
//! `Mutex<Connection>`, doc-comment discipline on every non-obvious
//! decision — because the two databases share the same operational
//! constraints (a single writer process, synchronous rusqlite calls that
//! must not stall an async worker) and there is no reason for a reader who
//! already knows one to have to re-learn the other's idioms.
//!
//! This module is STORAGE ONLY: schema, types, and CRUD. It holds no
//! connections, makes no routing decisions, and knows nothing about which
//! hosts are currently reachable — [`crate::manager`] owns all of that, and
//! [`crate::aggregate`] is what joins the two into the served list. The one
//! other boundary is browser authentication: this module persists the
//! recoverable bootstrap token and hashed device credentials, while
//! [`crate::auth`] owns their protocol and comparison. SPEC_impl.md's "Helm
//! internals" section carries the settled data model this schema implements.
//!
//! ## Divergences from the supervisor store, and why
//!
//! - **No `may_migrate` gate.** `SessionStore::open` takes one because a
//!   supervisor restart can briefly overlap its predecessor holding the
//!   SAME state directory (a handoff), and upgrading a database the
//!   incumbent still owns would brick it the moment it next opens its own,
//!   now-unreadable schema. SPEC.md's helm has no analogous overlap:
//!   "exactly one helm runs at a time" is the whole model, so there is
//!   never a second, older process this database's owner must protect from
//!   its own upgrade. [`HelmStore::open`] therefore always migrates.
//! - **Concurrent-open safety despite the single-helm rule.** The token-control
//!   ownership lock prevents two serving helms or an offline rotation from
//!   claiming the state directory together, but this storage type remains
//!   independently openable by tests and maintenance callers.
//!   [`ensure_local_row`]'s conditional `ON CONFLICT` and
//!   [`HelmStore::add_ssh_host`]'s own conditional insert hold regardless: a
//!   second connection racing the first over the same file converges on one
//!   winner instead of corrupting the database. Cheap SQLite-level
//!   insurance is the honest response to a rule nothing actually enforces —
//!   the alternative is trusting every future caller of this module to
//!   never get the one-helm assumption wrong.
//! - **`PRAGMA foreign_keys = ON`.** The supervisor's schema has no foreign
//!   keys at all. This one does: `session_cache` references `hosts` with
//!   `ON DELETE CASCADE`, which is how removing a host purges its cache in
//!   one statement (SPEC.md's disposal rule) instead of a second,
//!   separately-fallible delete this module would otherwise have to keep in
//!   sync by hand. SQLite enforces foreign keys only when a connection asks
//!   for it, so the pragma is set once at open — unlike `user_version`, it
//!   is not durable in the database file itself.
//! - **Conditional writes instead of caught constraint violations**, for
//!   both the reserved local row and a duplicate ssh destination.
//!   [`ensure_local_row`] and [`HelmStore::add_ssh_host`] share the exact
//!   same `INSERT ... ON CONFLICT (...) WHERE ... DO NOTHING` shape, each
//!   targeting its own partial unique index below;
//!   [`HelmStore::update_ssh_destination`] reaches the same outcome with
//!   `UPDATE OR IGNORE` instead, since it is rewriting one already-existing
//!   row rather than inserting a new one. The UNIQUE indexes below still
//!   exist and still bind: the conditional query is what turns the common
//!   single-writer case into a clean, typed [`HostStoreError`] instead of a
//!   raw SQLite constraint message, while the index remains the actual
//!   invariant enforcement any writer — including a future one this module
//!   does not anticipate — cannot bypass.
//!
//! ## Identity claims are a SCHEMA invariant, not a call-order convention
//!
//! At most one row may hold a given `host_identity` (the
//! `hosts_identity_claim` partial unique index, schema version 2). That is
//! deliberately enforced in SQL rather than by the connection manager
//! checking for a twin before it records, because the check-then-record
//! shape has a TOCTOU that costs the user the whole host: two actors
//! reaching the same freshly-installed supervisor both see no twin, both
//! record, both connect — and at the next helm start each sees the other as
//! its twin and BOTH freeze as duplicates, so a live host appears ZERO
//! times. [`HelmStore::record_first_contact`] and
//! [`HelmStore::adopt_identity`] resolve the claim inside the same
//! transaction as the write, so whichever caller loses gets a typed answer
//! naming the winner instead of a durable collision.
//!
use crate::aggregate::host_display_name;
use anyhow::Context;
use farhelm_proto::{SessionInfo, SessionStatus};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use subtle::ConstantTimeEq;

/// Maximum number of authenticated browser profiles retained by one helm.
///
/// A device exchange is user-mediated and infrequent, so retaining the 64
/// newest credentials leaves ample room for ordinary use while preventing a
/// leaked bootstrap token from growing helm.db without bound.
pub(crate) const MAX_DEVICE_SESSIONS: usize = 64;

/// How long a query waits on `SQLITE_BUSY` before giving up.
///
/// Mirrors `farhelm-supervisor/src/store.rs`'s own constant, but the
/// justification is narrower here: SPEC.md's single-helm invariant means
/// this module never sees the handoff-restart overlap that motivates the
/// supervisor's identical value. The overlap this module DOES see is
/// entirely a test artifact — the concurrent-double-open tests below open
/// two independent `Connection`s against one file on purpose, to pin the
/// local row's uniqueness under a genuine second writer — and a bare
/// `SQLITE_BUSY` there would turn a timing-sensitive test flaky instead of
/// deterministic. Kept at the same five seconds as the supervisor's for no
/// reason other than one fewer arbitrary constant in the codebase.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The schema's current shape. See [`apply_schema`] for the version
/// history and the ladder future migrations extend.
const SCHEMA_VERSION: i64 = 18;

/// The helm-owned profile catalog uses the same durable row shape as the
/// former supervisor catalog so profiles remain portable across this move.
const PROFILES_SCHEMA: &str = "CREATE TABLE profiles (
                 id              TEXT PRIMARY KEY,
                 name            TEXT NOT NULL,
                 invocation      TEXT NOT NULL,
                 agent_kind      TEXT NOT NULL,
                 resume_template TEXT
             ) STRICT;";

/// The per-session "last seen" stamp (schema version 17): the activity
/// stamp that was current the last time some client had this session open,
/// compared against `SessionInfo::effective_activity` to decide whether a
/// session has unseen output (SPEC.md, Status; `SessionRow::seen_activity_at`
/// is what a listing reply carries this through as).
///
/// A separate table rather than a `session_cache` column, unlike most of
/// this file's per-session state: `session_cache` rows are replaced
/// WHOLESALE by `replace_host_sessions` on every host refresh under the
/// changed-only rule, and a column there would need that write to carry
/// forward a field the SUPERVISOR never reported — putting a viewer fact
/// inside the host-payload mirror and complicating the one write the cache
/// has. Keyed by session id alone, with no foreign key to `hosts` and no
/// `ON DELETE CASCADE`: session ids are supervisor-minted UUIDs the cache
/// already treats as globally unique (`session_cache_one_owner`), and a
/// bare id key is what lets this row survive a retarget or an adoption —
/// both of which are supposed to keep the session, unlike a host removal.
///
/// `sessions::delete_session` removes this row explicitly when THIS helm
/// deletes a session. A session deleted through another helm, or dropped
/// from a cache because its host was removed here, can leave a row behind:
/// that garbage is bounded by the number of sessions that ever existed, at
/// a few dozen bytes each, and is not worth a reaper yet — a later
/// migration could sweep rows whose id no `session_cache` row names.
const SESSION_SEEN_SCHEMA: &str = "CREATE TABLE session_seen (
                 session_id       TEXT NOT NULL PRIMARY KEY,
                 seen_activity_at INTEGER NOT NULL
             ) STRICT;";

/// Seed rows are inserted once with the schema migration, not repaired at
/// startup, so deleting or editing a starter remains durable.
const STARTER_PROFILES: &str = "INSERT INTO profiles \
                 (id, name, invocation, agent_kind, resume_template) VALUES \
                 ('starter-claude', 'claude', 'claude', 'claude', NULL), \
                 ('starter-claude-yolo', 'claude-yolo', \
                  'claude --dangerously-skip-permissions', 'claude', \
                  '[\"claude\",\"--dangerously-skip-permissions\",\"--resume\",\"{conversation}\"]'), \
                 ('starter-codex', 'codex', 'codex', 'codex', NULL), \
                 ('starter-codex-yolo', 'codex-yolo', 'codex --yolo', 'codex', \
                  '[\"codex\",\"--yolo\",\"resume\",\"{conversation}\"]');";

/// Surrogate primary key of a `hosts` row.
///
/// `AUTOINCREMENT` on the underlying column (see [`apply_schema`]) is what
/// makes this type meaningful as a long-lived identity rather than a mere
/// row position: without it, SQLite may recycle a plain `INTEGER PRIMARY
/// KEY` after a delete, and a recycled id handed to whatever keys off it
/// later (a connection actor, a REST reference a browser tab is still
/// holding) would silently start describing a DIFFERENT host. Removing and
/// re-registering the same destination is exactly PLAN_M6.md's supported
/// path (SPEC.md's remove-merely-forgets contract), so this collision is
/// not a hypothetical the schema can afford to leave open.
pub type HostId = i64;

/// A `hosts` row's kind: the reserved local row, or a registered ssh
/// destination. See the module docs and [`apply_schema`] for the shape
/// this constrains in SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    /// The one reserved row for the machine running the helm itself
    /// (SPEC.md: always present, never registered, never removable).
    /// [`HelmStore::open`] mints it if absent; no other writer may create
    /// one. Its DESTINATION and its existence are not user management
    /// surface: [`HelmStore::update_ssh_destination`] and
    /// [`HelmStore::remove_ssh_host`] refuse a [`HostKind::Local`] row
    /// outright. Everything else treats it like any other host —
    /// [`HelmStore::update_alias`] accepts it exactly as it accepts an ssh
    /// row (`plans/host-aliases.md`'s decisions section: "the local host
    /// can be aliased too"), and identity ([`HelmStore::record_first_contact`],
    /// [`HelmStore::adopt_identity`]) and cache
    /// ([`HelmStore::replace_host_sessions`]) operations serve it on the
    /// same terms as any other host: the local host learns its identity and
    /// caches its sessions exactly like a registered one (PLAN_M6.md item
    /// 3).
    Local,
    /// A registered ssh destination, addable, editable, and removable
    /// through the ordinary host-management API.
    Ssh,
}

impl HostKind {
    /// The exact inverse of the on-disk spelling `'local'`/`'ssh'`
    /// literals write directly (see [`ensure_local_row`] and
    /// [`HelmStore::add_ssh_host`]'s own SQL). Refuses rather than guesses
    /// on anything outside this build's vocabulary — the schema's `CHECK`
    /// constraint binds every writer THIS build controls, but the database
    /// file is still a trust boundary like any other input (a hand edit, a
    /// downgrade), so a corrupt value is reported, not silently coerced.
    fn from_column(text: &str) -> anyhow::Result<HostKind> {
        match text {
            "local" => Ok(HostKind::Local),
            "ssh" => Ok(HostKind::Ssh),
            other => anyhow::bail!("hosts row has unrecognized kind {other:?}"),
        }
    }
}

/// One `hosts` row, as read back by [`HelmStore::list_hosts`].
///
/// `destination`/`remote_farhelm`/`remote_state_dir` are `None` for the
/// local row by construction (the schema's `CHECK` constraint refuses any
/// other shape) and, for an ssh row, `destination` is always `Some` while
/// the other two are whatever was supplied at [`HelmStore::add_ssh_host`]
/// time — both legitimately absent, since a remote install may run its
/// default `farhelm` binary out of its default state directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRow {
    pub id: HostId,
    pub kind: HostKind,
    pub destination: Option<String>,
    /// An optional user-facing label. When present it replaces the derived
    /// destination or local label everywhere except the details view.
    pub alias: Option<String>,
    pub remote_farhelm: Option<String>,
    pub remote_state_dir: Option<String>,
    /// The identity this host's supervisor reported at last contact —
    /// `None` until [`HelmStore::record_first_contact`] has ever succeeded
    /// for this row, which for the local row too is possible right after
    /// `open` (see that method's docs on why minting is deliberately not
    /// this module's job).
    pub host_identity: Option<String>,
    /// Whether this host's cached session list was cut at the wire's cap
    /// when it was last written — the persisted half of SPEC.md's "could
    /// not read to the end" notice, read by the merged list for every host
    /// that serves from its cache, connected or not. Set by
    /// [`HelmStore::replace_host_sessions`] and nothing else; a failed
    /// refresh leaves it as it was, exactly as it leaves the rows.
    pub cache_truncated: bool,
}

/// One `hosts` row's columns, read positionally by [`HelmStore::list_hosts`]
/// before the fallible [`HostKind::from_column`] decode — mirrors the
/// supervisor store's `SessionColumns` two-stage split (a rusqlite row
/// mapper's error type cannot carry an `anyhow::Error`, so the semantic
/// decode has to happen a step later). A named alias rather than the bare
/// tuple purely to keep clippy's `type_complexity` lint quiet; it carries no
/// meaning beyond "positionally matches the `SELECT` in `list_hosts`".
type RawHostRow = (
    HostId,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    bool,
);

/// One decodable row of the session cache as [`HelmStore::cached_rows`]
/// reads it: which host filed it, the archive flag it was filed under, and
/// its payload.
///
/// Only rows that DECODE come back as one of these. A cache row is
/// last-known display data, so a payload that no longer decodes (or that
/// names a different session than the row is filed under) is dropped from
/// the read and logged, rather than failing the read — one bad blob must
/// not take a whole host's stale list down — and rather than being carried
/// as an unshowable placeholder: the merged list's counts describe rows a
/// client can see, and a row nobody can render is corruption for the log,
/// not an entry for the count (SPEC.md's Session list section).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedRow {
    pub host: HostId,
    pub archived: bool,
    pub info: SessionInfo,
}

/// One consistent read of the cache for a set of hosts: the rows, and
/// which of those hosts' caches were cut at the wire's cap — taken under
/// one lock hold so the flag always describes exactly these rows. See
/// [`HelmStore::cached_slice`] for why the pairing is load-bearing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CachedSlice {
    pub rows: Vec<CachedRow>,
    /// The subset of the requested hosts whose `cache_truncated` flag is
    /// set. A `Vec` rather than a set: it is a handful of ids at most.
    pub truncated_hosts: Vec<HostId>,
}

/// What one [`HelmStore::replace_host_sessions`] call did.
///
/// Two facts a wholesale replacement can only report from inside its own
/// transaction, and both are consumed by the connection actor: the ids it
/// had to drop because another host already claims them, and whether the
/// commit left this host's slice of the cache DIFFERENT than it found it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CacheReplacement {
    /// Session ids another host's cache already claims, so this refresh's
    /// rows for them were skipped (first claim holds). Empty for
    /// essentially every refresh that ever runs.
    pub contested: Vec<String>,
    /// Whether any stored row actually differs from what was there before
    /// — the invalidation feed's changed-only rule (PLAN_M6_75.md item 5).
    /// See [`HelmStore::replace_host_sessions`] for why the answer belongs
    /// to the write rather than to a comparison made afterwards.
    pub changed: bool,
    /// Whether convergence changed the profile a fresh create dialog sees.
    /// Provenance-only advances stay internal and do not wake clients.
    pub default_changed: bool,
}

/// Remembered-default columns needed to compare one source observation:
/// `(profile_id, source_host_id, source_creation_seq, source_created_at, source_session_id)`.
///
/// The alias keeps the SQL projection's positional contract visible without
/// making each query repeat an opaque tuple type.
type RememberedProfileRow = (
    String,
    Option<HostId>,
    Option<i64>,
    Option<i64>,
    Option<String>,
);

/// The five SQLite columns needed to reconstruct and revalidate one profile.
type ProfileColumns = (String, String, String, String, Option<String>);

/// What a helm-owned profile insertion did: either it stored a newly minted
/// profile or the bounded catalog refused the insertion without a write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileCreation {
    Created(farhelm_proto::Profile),
    CatalogFull,
}

/// Encode the shared agent-kind vocabulary used by the strict SQLite table.
fn agent_kind_column(kind: farhelm_proto::AgentKind) -> &'static str {
    match kind {
        farhelm_proto::AgentKind::Claude => "claude",
        farhelm_proto::AgentKind::Codex => "codex",
        farhelm_proto::AgentKind::Generic => "generic",
    }
}

/// Decode and reject unknown agent-kind values instead of silently changing
/// which integration a stored profile selects.
fn agent_kind_from_column(text: &str) -> anyhow::Result<farhelm_proto::AgentKind> {
    match text {
        "claude" => Ok(farhelm_proto::AgentKind::Claude),
        "codex" => Ok(farhelm_proto::AgentKind::Codex),
        "generic" => Ok(farhelm_proto::AgentKind::Generic),
        other => anyhow::bail!("row has unrecognized agent kind {other:?}"),
    }
}

/// Serialize the optional argv template in the same representation as the
/// supervisor catalog.
fn resume_template_column(template: Option<&[String]>) -> Option<String> {
    template.map(|value| serde_json::to_string(value).expect("strings serialize"))
}

/// Decode a stored optional argv template before shared validation runs.
fn resume_template_from_column(text: Option<String>) -> anyhow::Result<Option<Vec<String>>> {
    text.map(|value| serde_json::from_str(&value).context("decoding a stored resume template"))
        .transpose()
}

/// Read the profile columns in the fixed order used by catalog queries.
fn read_profile_columns(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProfileColumns> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    ))
}

/// Reconstruct a profile and revalidate it so hand-edited or old rows cannot
/// bypass the catalog's current field contract.
fn decode_profile_row(columns: ProfileColumns) -> anyhow::Result<farhelm_proto::Profile> {
    let (id, name, invocation, kind, template) = columns;
    let agent_kind = agent_kind_from_column(&kind).with_context(|| format!("profile {id}"))?;
    let resume_template =
        resume_template_from_column(template).with_context(|| format!("profile {id}"))?;
    farhelm_proto::validate_profile_fields(
        &name,
        &invocation,
        agent_kind,
        resume_template.as_deref(),
    )
    .map_err(|message| anyhow::anyhow!("profile {id}: {message}"))?;
    Ok(farhelm_proto::Profile {
        id,
        name,
        invocation,
        agent_kind,
        resume_template,
    })
}

/// One observation's ordering fields, borrowed while a store transaction compares them.
///
/// A sequence is meaningful only within `host`; `created_at` and `session_id`
/// remain the fleet-wide fallback when either source lacks that shared domain.
struct ProfileSource<'a> {
    host: Option<HostId>,
    sequence: Option<u64>,
    created_at: i64,
    session_id: &'a str,
}

/// Compare provenance without treating independent supervisor sequences as global time.
///
/// A missing sequence marks an older peer. In that mixed-version case the
/// established timestamp/id rule remains the only ordering both sides can
/// understand, so rollout does not make an old observation permanently
/// incomparable with a new one.
///
/// `false` means the candidate did not advance the stored source. That covers
/// both equality and rejection as older; callers must not interpret it as
/// proof that the two provenance records identify the same observation.
fn source_is_newer(candidate: ProfileSource<'_>, stored: ProfileSource<'_>) -> bool {
    match (
        candidate.host,
        candidate.sequence,
        stored.host,
        stored.sequence,
    ) {
        (Some(candidate_host), Some(candidate), Some(stored_host), Some(stored))
            if candidate_host == stored_host =>
        {
            candidate > stored
        }
        _ => {
            candidate.created_at > stored.created_at
                || (candidate.created_at == stored.created_at
                    && candidate.session_id < stored.session_id)
        }
    }
}

/// The predicates a merged-view read is narrowed by — SPEC.md's filtering
/// and search dimensions, including spawned-session parentage, as one value.
///
/// ## Why it lives in the STORE
///
/// The persisted majority of the merged list is paged out of this module,
/// while an identity-less host's sessions are merged in from memory by
/// [`crate::aggregate`] — two sources, one answer. A filter defined in
/// either one alone would have to be re-implemented in the other, and the
/// failure mode of a disagreement is silent: a session that matches on one
/// side and not on the other is simply missing from the page, with the
/// count still claiming it. So the predicate is defined once, here, and
/// BOTH sources call [`Self::matches`].
///
/// ## Match semantics, and why each is what it is
///
/// SPEC.md calls the feature "filtering and search" without saying which
/// dimensions belong to which half. The split below follows the
/// shape of the data rather than the wording:
///
/// - **archive is a default-off inclusion switch.** Withholding archived
///   rows is the ordinary view; enabling the switch removes that predicate
///   rather than selecting archived rows alone. It is also the one dimension
///   the served `total` follows (`crate::aggregate`): the switch picks
///   which view is being counted, while every other dimension narrows a view
///   whose size the count goes on reporting.
/// - **host, parent, status, profile — EXACT.** Each is an identifier or a
///   value chosen from a finite set
///   the client already has in hand (the hosts list, the status vocabulary,
///   the helm's profile catalog), so a substring match would only ever
///   create surprises: `error` matching nothing else today but matching a
///   future `error_recovered`, or a profile named `claude` also selecting
///   `claude-review`.
/// - **directory, title — case-insensitive SUBSTRING.** These are free text
///   the user types into a search box, and neither has a canonical prefix a
///   user reliably remembers: a session in `/home/me/src/farhelm` is found
///   by typing `farhelm`, and one titled "Refactor the drain" by typing
///   `drain`. Case folding is Rust's `to_lowercase` (Unicode-aware, not
///   ASCII-only) on both needle and haystack.
/// - **profile matches the SNAPSHOT, by id OR by name.** A session carries
///   the profile id and the name AS SNAPSHOTTED at creation
///   (`SourceProfile`), and nothing rewrites those when the profile is
///   edited or deleted. Matching the id is what makes a picker's selection
///   exact and rename-proof; matching the snapshotted name is what keeps a
///   DELETED profile's sessions filterable at all, since after the delete
///   the name is the only handle anyone still has. Both are accepted in one
///   parameter because a client has one search box and the two never
///   collide in practice (an id is supervisor-minted opaque text).
///
/// A session with no `source_profile` — a raw-invocation create — never
/// matches a profile filter. That is the honest reading of "sessions from
/// profile X" and not merely a convenience: such a session was never shaped
/// by any profile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionFilter {
    /// Whether archived rows participate in this view. False is the public
    /// default: archive removes a session from ordinary browsing without
    /// removing it from the fleet or its durable history.
    include_archived: bool,
    host: Option<HostId>,
    parent: Option<String>,
    directory: Option<Folded>,
    title: Option<Folded>,
    /// The raw needle AND its folded form: the id half of the match is a
    /// byte comparison against opaque text, the name half is case-folded,
    /// and precomputing the fold here keeps the per-row cost to a
    /// comparison rather than an allocation.
    profile: Option<Folded>,
    /// The `state` tag of [`SessionStatus`], as [`status_key`] spells it.
    /// `&'static str` rather than an enum of this module's own: the
    /// vocabulary is the protocol's, and a second copy of it here would be
    /// a second thing to keep in step with the wire.
    status: Option<&'static str>,
}

/// A search needle kept beside its case-folded form.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Folded {
    raw: String,
    folded: String,
}

impl Folded {
    fn new(raw: &str) -> Folded {
        Folded {
            raw: raw.to_string(),
            folded: raw.to_lowercase(),
        }
    }

    /// Whether `haystack` contains this needle, ignoring case.
    fn contained_in(&self, haystack: &str) -> bool {
        haystack.to_lowercase().contains(&self.folded)
    }
}

/// The `state` tag one [`SessionStatus`] serializes under — the word a
/// client filters by, and the same word the wire carries.
///
/// Written as a match rather than derived from serde so that adding a
/// status variant fails to compile here, which is exactly the reminder a
/// new status needs: a filter vocabulary that silently omitted a status
/// would make those sessions unfindable with no error anywhere.
pub fn status_key(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Unknown => "unknown",
        SessionStatus::Running => "running",
        SessionStatus::Waiting => "waiting",
        SessionStatus::Idle => "idle",
        SessionStatus::Exited { .. } => "exited",
        SessionStatus::Error { .. } => "error",
        SessionStatus::Interrupted => "interrupted",
    }
}

/// The status vocabulary a `?status=` parameter may name, or `None` for a
/// word this build does not know.
///
/// `unknown` is accepted even though it must never RENDER (see
/// `SessionStatus::Unknown`'s own docs): the value exists in the cache
/// while a freshly created session waits for its first classification, and
/// a filter that could not name it would leave those rows unreachable
/// rather than merely unbadged. Refusing an unrecognized word — rather
/// than matching nothing — is what turns a typo into a 400 the user can
/// read instead of an empty list they will believe.
pub fn parse_status_key(text: &str) -> Option<&'static str> {
    [
        "unknown",
        "running",
        "waiting",
        "idle",
        "exited",
        "error",
        "interrupted",
    ]
    .into_iter()
    .find(|known| *known == text)
}

/// Which order a merged listing is served in — the `?sort=` vocabulary, and
/// the single place the three orders are defined.
///
/// ## Every order ends in the same total order
///
/// Each variant names its own leading component and then falls back to
/// `created_at` DESCENDING, session id ascending, host id ascending. The
/// tail is what makes every order total, so two reads of an unchanged fleet
/// come back in the same sequence and the listing cap always cuts the same
/// rows. The leading component is allowed to tie; the order as a whole
/// never is. The comparison itself lives in `crate::aggregate::sort_rows`:
/// the list is sorted in memory, per request, and nothing in this store
/// orders by anything but the host slice a stale read asks for.
///
/// ## Not part of [`SessionFilter`], deliberately
///
/// A filter decides WHICH rows a view holds and a sort decides in what order
/// it hands them over; folding the second into the first would make a
/// re-sorted view look like a differently-filtered one, and neither count
/// may move with the sort.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ListSort {
    /// `created_at` descending — the order every listing had before there
    /// was a choice, and still the one a caller that names none gets. Kept
    /// as the default so every client and test written against the original
    /// list keeps its exact behavior.
    #[default]
    Created,
    /// Effective activity descending, then the creation-order tail.
    ///
    /// "Effective" is `SessionInfo::effective_activity`: a session whose
    /// sender predates `last_activity_at` (or that has produced no observed
    /// output) sorts by its creation time rather than piling up at the
    /// epoch.
    Activity,
    /// Lowercased title ascending, then the creation-order tail. The
    /// collation is Rust's `str::to_lowercase` compared as code points —
    /// case-insensitive across Unicode, otherwise ordinal, and deliberately
    /// neither locale-aware nor SQLite's ASCII-only `NOCASE`.
    Title,
}

/// The `?sort=` vocabulary, or `None` for a word this build does not know.
///
/// Refusing an unrecognized word rather than falling back to the default is
/// the same judgement [`parse_status_key`] makes: a typo answered with a
/// silently different order is a list the user reads as authoritative and
/// cannot tell is wrong.
pub fn parse_sort_key(text: &str) -> Option<ListSort> {
    match text {
        "created" => Some(ListSort::Created),
        "activity" => Some(ListSort::Activity),
        "title" => Some(ListSort::Title),
        _ => None,
    }
}

/// The client preference this helm remembers for every client at once
/// (SPEC.md, Session list): the chosen list order, session the user last
/// selected, and compact-row choice. One row, one shape — the stored row and
/// the `GET` reply.
///
/// Every field is `Option` because "never set" is a real state: the
/// client's default applies. `list_sort` is the bare `?sort=` word
/// ([`parse_sort_key`]), stored as the word rather than the enum so an
/// unset value has an honest representation; `last_selected` is a bare
/// session id, valid for this helm's fleet only, which is why the helm's
/// own database and not client storage is where it belongs. A `PUT` sends a
/// [`PreferencePatch`], not this type: a patch has to tell "leave alone"
/// from "clear", and a plain `Option` cannot.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_selected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact: Option<bool>,
}

/// A sparse change to [`Preferences`]: each field is absent (leave it as
/// it is), `null` (clear it back to unset), or a value (replace it).
///
/// Three states, so `Option<Option<T>>`: the outer level is presence in
/// the JSON, the inner is the value. Serde's default `Option` handling
/// folds `null` and absence together, so a custom deserializer
/// ([`double_option`]) keeps them apart — an absent key deserializes to
/// `None` through `default`, and a present key (null or not) goes through
/// the function and lands as `Some`. The distinction is what makes the
/// row shareable: a client sends only the field the user changed and
/// never carries its own stale copy of the other, yet a harness or a
/// deselect can still put a field back to "nothing remembered".
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferencePatch {
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub list_sort: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_selected: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option_bool",
        skip_serializing_if = "Option::is_none"
    )]
    pub compact: Option<Option<bool>>,
}

/// Deserialize a PRESENT field of [`PreferencePatch`] — serde only calls
/// this when the key exists, so wrapping the inner `Option` in `Some` is
/// exactly what distinguishes `"x": null` from no `"x"` at all.
fn double_option<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <Option<String> as serde::Deserialize>::deserialize(deserializer).map(Some)
}

/// Deserialize the boolean counterpart of [`double_option`].
///
/// Keeping this typed makes malformed compact patches a request error rather
/// than silently translating a misspelled string into an unrelated layout.
fn double_option_bool<'de, D>(deserializer: D) -> Result<Option<Option<bool>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <Option<bool> as serde::Deserialize>::deserialize(deserializer).map(Some)
}

impl SessionFilter {
    /// Admit archived rows as well as active ones.
    ///
    /// This is an inclusion switch, not a value to compare against: callers
    /// cannot ask for archived-only rows, and turning it on removes the
    /// implicit default predicate rather than adding a new one.
    pub fn include_archived(mut self, include: bool) -> SessionFilter {
        self.include_archived = include;
        self
    }

    /// Whether this view admits archived rows.
    ///
    /// Exposed for the DENOMINATOR rather than for the predicate: unlike
    /// every other dimension, this one changes which view `total` is a count
    /// of, so the merge (`crate::aggregate`) applies it as a scope before it
    /// counts. [`Self::matches`] applies it independently — the predicate is
    /// the contract and the scope is what makes the count right. The host
    /// dimension deliberately has no such scope: it is a filter like any
    /// other, and narrowing the read to it would make `total` follow it.
    pub fn includes_archived(&self) -> bool {
        self.include_archived
    }

    /// Narrow to one host.
    pub fn host(mut self, host: HostId) -> SessionFilter {
        self.host = Some(host);
        self
    }

    /// Narrow to direct children of one session id.
    pub fn parent(mut self, parent: &str) -> SessionFilter {
        self.parent = Some(parent.to_string());
        self
    }

    /// Narrow to sessions whose working directory contains `needle`.
    pub fn directory(mut self, needle: &str) -> SessionFilter {
        self.directory = Some(Folded::new(needle));
        self
    }

    /// Narrow to sessions whose title contains `needle`.
    pub fn title(mut self, needle: &str) -> SessionFilter {
        self.title = Some(Folded::new(needle));
        self
    }

    /// Narrow to sessions created from the profile named by `value` —
    /// either its id or its snapshotted name (see this type's docs).
    pub fn profile(mut self, value: &str) -> SessionFilter {
        self.profile = Some(Folded::new(value));
        self
    }

    /// Narrow to one status, by the tag [`status_key`] spells.
    pub fn status(mut self, status: &'static str) -> SessionFilter {
        self.status = Some(status);
        self
    }

    /// The one host this filter admits, when it names exactly one.
    ///
    /// Read by the merge to SCOPE the truncation notice, not the rows: a
    /// request for one host's sessions cannot be missing rows another
    /// host's cap cut, so only the named host's flag may raise "could not
    /// read to the end" there (`crate::aggregate`). The rows themselves are
    /// still read fleet-wide — `total` is the whole view's size whatever
    /// the filter says — and [`Self::matches`] still checks the host per
    /// row.
    pub fn host_scope(&self) -> Option<HostId> {
        self.host
    }

    /// Whether this filter narrows nothing — which is what decides whether
    /// a listing reply carries a `matching` count at all
    /// (`crate::aggregate::SessionListBody::matching`).
    pub fn is_empty(&self) -> bool {
        *self == SessionFilter::default().include_archived(true)
    }

    /// Whether one session, on `host`, satisfies every dimension set.
    ///
    /// AND across dimensions, deliberately: each parameter narrows, and a
    /// client that wants a union asks twice. Nothing here is fallible —
    /// an unmatched dimension is simply false — because a filter is a
    /// question about a row, and there is no row this can fail to answer
    /// for.
    pub fn matches(&self, host: HostId, info: &SessionInfo) -> bool {
        if info.archived && !self.include_archived {
            return false;
        }
        if let Some(wanted) = self.host
            && wanted != host
        {
            return false;
        }
        if let Some(status) = self.status
            && status != status_key(&info.status)
        {
            return false;
        }
        if let Some(parent) = &self.parent
            && info.parent.as_deref() != Some(parent.as_str())
        {
            return false;
        }
        if let Some(directory) = &self.directory
            && !directory.contained_in(&info.cwd)
        {
            return false;
        }
        if let Some(title) = &self.title
            && !title.contained_in(&info.title)
        {
            return false;
        }
        if let Some(profile) = &self.profile {
            let Some(source) = &info.source_profile else {
                return false;
            };
            if source.id != profile.raw && source.name.to_lowercase() != profile.folded {
                return false;
            }
        }
        true
    }
}

impl ListSort {}

/// One host to guarantee registered — the `--ensure-hosts` entry as
/// [`HelmStore::ensure_ssh_hosts`] consumes it.
///
/// A store-level type rather than the file's own deserialization target, so
/// the batch write's contract does not depend on JSON5 or on where the
/// entries came from: the same call would serve a future provisioning path
/// that produced entries some other way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnsureHost {
    pub destination: String,
    pub remote_farhelm: Option<String>,
    pub remote_state_dir: Option<String>,
}

/// A `HelmStore` operation refused for a reason a caller may need to act
/// on, distinctly from a bare I/O or SQL failure — the same reasoning as
/// `SupervisorError` on the client side (`client.rs`'s own docs): carried
/// through `anyhow` as a typed value (`anyhow::Error::new(..)`, downcast
/// with `.downcast_ref::<HostStoreError>()`) rather than a formatted
/// string, so a REST handler can recover which case happened without
/// parsing `Display`'s prose.
#[derive(Debug, thiserror::Error)]
pub enum HostStoreError {
    /// An `add_ssh_host` or `update_ssh_destination` call named a
    /// destination another ssh row already holds. Carries the rejected
    /// destination so the caller's error message can name it without a
    /// second lookup.
    #[error("a host is already registered at destination {0:?}")]
    DuplicateDestination(String),
    /// A destination that cannot be handed to `ssh` as a destination at
    /// all: empty, or shaped like an option (a leading `-`).
    ///
    /// The option-shaped case is a security refusal wearing a usability
    /// coat. `crate::ssh::ssh_base_args` already terminates ssh's option parsing
    /// before the destination, which is what actually stops
    /// `-oProxyCommand=...` from executing; refusing it HERE means a user
    /// who pastes such a string gets told what is wrong with it at the
    /// moment they register it, instead of a host that permanently fails
    /// to connect with ssh's own usage message. Defense in depth in the
    /// literal sense: neither layer is load-bearing alone.
    #[error("{0:?} is not a usable ssh destination")]
    InvalidDestination(String),
    /// An alias failed a LOCAL, syntax-only rule — a control character, or
    /// the 64-character cap — checked before any comparison against other
    /// hosts runs. A collision with another host's current display name is
    /// a separate case ([`HostStoreError::AliasTaken`]); this variant never
    /// carries one. The display text is safe to show directly to the
    /// caller.
    #[error("{0}")]
    InvalidAlias(String),
    /// A write would make two rows share one display name: a proposed alias
    /// or destination matches another row's current alias, or (alias-clear
    /// only) a row's restored derived name matches another row's current
    /// alias. Returned by [`HelmStore::add_ssh_host`],
    /// [`HelmStore::register_probed_ssh_host`], [`HelmStore::ensure_ssh_hosts`],
    /// [`HelmStore::update_ssh_destination`], and [`HelmStore::update_alias`]
    /// alike, which is why the message names no field — the caller did not
    /// necessarily submit an alias at all. The payload is the other row's
    /// current display name, not an internal id.
    #[error("a host is already displayed as {0:?}")]
    AliasTaken(String),
    /// A call named a [`HostId`] no row currently holds — including one
    /// that once existed and was removed.
    #[error("host {0} does not exist")]
    HostNotFound(HostId),
    /// A call tried to retarget or remove the reserved local row through
    /// the ssh-host management API. The local row's DESTINATION and its
    /// existence are not user management surface (PLAN_M6.md item 4): it is
    /// synthesized once by [`HelmStore::open`], never removable, and
    /// [`HelmStore::update_ssh_destination`]/[`HelmStore::remove_ssh_host`]
    /// refuse it outright. That is narrower than "untouchable": its alias
    /// IS editable ([`HelmStore::update_alias`] accepts it like any other
    /// host), and so are identity ([`HelmStore::record_first_contact`],
    /// [`HelmStore::adopt_identity`]) and cache
    /// ([`HelmStore::replace_host_sessions`]) operations, which the local
    /// row needs on the same terms as any other host (it "learns its
    /// identity the same way" — PLAN_M6.md item 3).
    #[error("the local host's destination cannot be changed and it cannot be removed")]
    LocalHostImmutable,
    /// The stored identity no longer matches what a caller assumed it still
    /// was. Two distinct callers hit this: [`HelmStore::adopt_identity`]'s
    /// compare-and-swap, when `expected_old` no longer names the currently
    /// stored value (someone else adopted, or first-contacted, first); and
    /// [`HelmStore::replace_host_sessions`]'s identity-bound write, when the
    /// identity a session-list refresh was produced under has since been
    /// superseded — the case this exists to catch is a refresh that was
    /// in flight when a user adopted a new identity, landing late enough to
    /// otherwise repopulate the very cache the adoption just purged.
    /// `actual` is `None` when nothing has ever been recorded for this
    /// host (a caller with a non-empty `expected_old`/identity can only see
    /// this if a mismatch existed already, since first contact is the only
    /// way OUT of `None` and it never lands here).
    #[error("host {host} identity is {actual:?}, not the expected {expected:?}")]
    IdentityMismatch {
        host: HostId,
        expected: String,
        actual: Option<String>,
    },
    /// [`HelmStore::adopt_identity`] would have claimed an identity another
    /// row already holds — the `hosts_identity_claim` index's refusal,
    /// resolved inside the adoption's own transaction so nothing was
    /// written.
    ///
    /// Reachable in practice, not merely in theory: a rival entry can
    /// first-contact the very identity a user is being asked to adopt in
    /// the window between the mismatch being displayed and the adopt
    /// arriving. The caller's correct response is to re-render the host,
    /// which now shows the duplicate-resolution surface instead of the
    /// adopt-or-fix one.
    #[error("host {owner} already holds identity {identity:?}; host {host} cannot adopt it")]
    IdentityClaimed {
        host: HostId,
        identity: String,
        owner: HostId,
    },
    /// The row no longer carries the connection-defining configuration the
    /// caller's attempt was made under (see [`DialedAs`]) — the user edited
    /// the destination while a handshake or an adoption decision was in
    /// flight, so the answer in hand describes a host this row no longer
    /// points at. Nothing was written.
    #[error("host {host} has been reconfigured since this attempt was made")]
    StaleAttempt { host: HostId },
    /// Two hosts' caches both claim one session id, so there is no honest
    /// answer to "which host owns this session".
    ///
    /// The `session_cache_one_owner` index makes this UNCONSTRUCTIBLE for
    /// any writer this build controls, and a lookup still checks for it,
    /// because the database file is a trust boundary like any other input —
    /// a downgrade or a hand edit can present rows no current writer could
    /// have made. Failing closed is the only safe answer: routing an
    /// operation to one of two candidate hosts would mean a stop or a
    /// delete aimed at one machine landing on another. Names both hosts so
    /// the user can remove whichever entry does not belong.
    #[error(
        "session {session} is cached under both host {first} and host {second}; refusing to \
         guess which one owns it"
    )]
    SessionOwnerAmbiguous {
        session: String,
        first: HostId,
        second: HostId,
    },
}

/// Canonicalize one optional host alias before it reaches durable storage.
///
/// Whitespace-only input clears the alias; every other accepted value is
/// trimmed so display and agent resolution never disagree over invisible
/// surrounding characters. Control characters are refused because an alias
/// reaches line-oriented agent CLI output.
fn validate_alias(alias: Option<&str>) -> Result<Option<String>, HostStoreError> {
    let Some(alias) = alias else {
        return Ok(None);
    };
    let alias = alias.trim();
    if alias.is_empty() {
        return Ok(None);
    }
    if alias.chars().any(char::is_control) {
        return Err(HostStoreError::InvalidAlias(
            "host alias must not contain control characters".to_string(),
        ));
    }
    if alias.chars().count() > 64 {
        return Err(HostStoreError::InvalidAlias(
            "host alias must be at most 64 characters".to_string(),
        ));
    }
    Ok(Some(alias.to_string()))
}

/// Whether `candidate` matches another row's current ALIAS — the narrow
/// half of the display-name uniqueness rule, shared by every write that
/// touches a DESTINATION rather than an alias: registering
/// ([`HelmStore::add_ssh_host`], [`HelmStore::register_probed_ssh_host`],
/// [`HelmStore::ensure_ssh_hosts`]), retargeting
/// ([`HelmStore::update_ssh_destination`]), and restoring the derived name
/// by clearing an alias ([`HelmStore::update_alias`]). Destination-versus-
/// destination collisions are the `hosts_ssh_destination` partial unique
/// index's job; this only needs to catch the cross-kind case an alias
/// introduces, which is why it reads the `alias` column alone rather than
/// every row's full derived display name (contrast `update_alias`'s SET
/// path, which does need the wide comparison — see that function's own
/// doc).
///
/// `exclude` is the row being written, when there is one to re-affirm its
/// own value against (registration has none: the row does not exist yet).
/// Every caller runs this inside the SAME transaction as the write it
/// gates, which is what makes the check and the write atomic against a
/// concurrent writer.
fn alias_collision(
    tx: &rusqlite::Transaction<'_>,
    exclude: Option<HostId>,
    candidate: &str,
) -> anyhow::Result<Option<String>> {
    let mut other = tx
        .prepare("SELECT alias FROM hosts WHERE (?1 IS NULL OR id != ?1) AND alias IS NOT NULL")
        .context("reading other hosts' aliases before a display-name-affecting write")?;
    let collision = other
        .query_map(rusqlite::params![exclude], |row| row.get::<_, String>(0))
        .context("querying other hosts' aliases")?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .find(|alias| alias == candidate);
    Ok(collision)
}

/// The connection-defining fields an attempt was dialed under, carried
/// back into the identity write it produced.
///
/// Identity writes are keyed by [`HostId`], and a HostId alone is not
/// enough to make the write correct: a hello that crossed the wire while
/// the user was editing the row's destination describes the OLD endpoint,
/// and committing its identity under the NEW configuration would durably
/// attribute one machine's identity to another. The connection manager
/// tears down in-flight attempts when a row is reconfigured, which narrows
/// that window; passing the dialed configuration back into the transaction
/// is what CLOSES it, since the check and the write then happen together.
///
/// All three fields, not just `destination`: `remote_farhelm` and
/// `remote_state_dir` select WHICH supervisor answers on that destination,
/// so editing either can change the identity a dial reaches just as surely
/// as retargeting the host. `None` throughout is the reserved local row's
/// shape and matches only itself.
///
/// Deliberately a value snapshot rather than a row revision counter: the
/// registry has no revision column, and adding one would mean every future
/// writer of any of these three columns had to remember to bump it — a
/// convention with the same failure mode this type exists to remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialedAs {
    pub destination: Option<String>,
    pub remote_farhelm: Option<String>,
    pub remote_state_dir: Option<String>,
}

impl DialedAs {
    /// The configuration `row` describes right now — what a caller records
    /// just before it dials, and hands back with whatever the dial learned.
    pub fn of(row: &HostRow) -> DialedAs {
        DialedAs {
            destination: row.destination.clone(),
            remote_farhelm: row.remote_farhelm.clone(),
            remote_state_dir: row.remote_state_dir.clone(),
        }
    }
}

/// Whether `destination` is something `ssh` can be handed as a
/// destination — see [`HostStoreError::InvalidDestination`] for why the
/// leading-`-` half of this exists.
///
/// Visible to the crate, not just to this module, because
/// [`crate::ensure`] validates an entire `--ensure-hosts` file BEFORE
/// writing any of it: the alternative is discovering the bad entry halfway
/// through, having already registered the good ones. The write paths below
/// still apply this themselves — this is a pre-check for a caller that
/// wants all-or-nothing, never a substitute for the store's own refusal.
///
/// The NUL clause is not a stylistic third case. A destination becomes ssh
/// ARGV, and argv is NUL-terminated by the kernel: an embedded NUL either
/// fails the spawn with an opaque error or — worse, depending on how the
/// value travels — silently truncates the destination to everything before
/// it, so `good.example\0evil.example` registers as one thing and dials
/// another. Refusing it at the registry boundary means the string that is
/// stored is the string that will be dialed.
pub(crate) fn destination_is_usable(destination: &str) -> bool {
    !destination.is_empty() && !destination.starts_with('-') && !destination.contains('\0')
}

/// The non-error result of [`HelmStore::record_first_contact`] — a
/// mismatched identity is an expected outcome the connection manager must
/// act on (SPEC.md: ask whether to adopt or fix the destination), not a
/// storage failure, so it is a value here rather than routed through
/// [`HostStoreError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirstContactOutcome {
    /// The identity is now on record: either this really was the first
    /// contact (the stored value was `NULL`), or `identity` matched what
    /// was already stored (an idempotent repeat hello). Both collapse to
    /// the same variant because a caller only cares whether the store now
    /// agrees with what was just reported, not which of the two cases
    /// applied.
    Recorded,
    /// A DIFFERENT identity was already on record; nothing was written.
    /// SPEC.md: this is exactly the "wiped and reinstalled host, a recycled
    /// address" case that must surface to the user as an adopt-or-fix
    /// choice rather than merge silently — [`HelmStore::adopt_identity`] is
    /// the explicit acknowledgment that performs the merge the user chose.
    Mismatch { recorded: String, reported: String },
    /// ANOTHER row already claims `identity`; nothing was written.
    ///
    /// This is the store-level answer to "two registry entries reach one
    /// host" — the duplicate the connection manager surfaces — and it is
    /// produced by the same transaction that would have written, so two
    /// actors racing first contact on one fresh identity cannot both win.
    /// `owner` is the row that holds the claim, which is exactly what the
    /// duplicate state must name.
    Collision { owner: HostId },
    /// The row's connection-defining configuration changed since this
    /// attempt was dialed (see [`DialedAs`]); nothing was written.
    ///
    /// Carries what the row says NOW, purely so the caller's log line can
    /// show what it was outrun by. A caller treats this as a failed
    /// attempt: the identity in hand belongs to the endpoint the row USED
    /// to name, and the next attempt will dial the new one.
    StaleAttempt { current: DialedAs },
}

/// The helm's registry-and-cache database.
///
/// Wraps the connection exactly as `farhelm-supervisor::store::SessionStore`
/// does — `Arc<Mutex<Connection>>` (a std mutex, never held across an
/// await; every hold is confined to one synchronous `spawn_blocking`
/// closure) — for the identical reason: rusqlite is synchronous and a
/// commit's fsync can block for real disk-flush time, so running it inline
/// on an async worker would stall that worker's entire share of the
/// runtime for the duration of one write.
#[derive(Clone, Debug)]
pub struct HelmStore {
    conn: Arc<Mutex<Connection>>,
}

/// Bring the database up to [`SCHEMA_VERSION`], creating it from scratch
/// (`user_version` 0) or migrating it forward one step at a time — the same
/// ladder shape as the supervisor store's `apply_schema`, kept identical on
/// purpose so a future migration here reads exactly like one there. Every
/// step (including the fresh-create path) runs in ONE `BEGIN IMMEDIATE`
/// transaction together with the `user_version` read that decides what to
/// do and the bump that claims the result: taking SQLite's write lock
/// BEFORE the version read, rather than after, is what lets a database
/// creation race between two `open` callers serialize on that lock instead
/// of needing every DDL statement to tolerate a stale read (see the
/// `version == 0` branch below and `concurrent_double_open_...` in the
/// tests). An interrupted upgrade also leaves the database at the version
/// it actually has, never one whose tables are half-built, for the same
/// single-transaction reason.
///
/// A fresh database is created directly in its FINAL shape rather than
/// built up by replaying history — the migrations below exist to preserve
/// data, and a new file has none — with
/// `a_migrated_database_matches_a_freshly_created_one` pinning that the two
/// paths agree, exactly as the supervisor store's own
/// `migrated_and_fresh_schemas_agree` does.
///
/// Version history:
/// - 1: PLAN_M6.md item 3 — `hosts` (the registry: the reserved local row
///   plus registered ssh destinations) and `session_cache` (the last-known
///   session list, replaced wholesale per host).
/// - 2: the `hosts_identity_claim` partial unique index, making
///   at-most-one-row-per-identity a schema invariant instead of something
///   the connection manager checked for before writing (see the module
///   docs' own section on why that check-then-write shape loses hosts). The
///   migration also has DATA to resolve, since a version-1 database predates
///   the constraint and may already hold rows sharing an identity.
/// - 3: the `session_cache_one_owner` unique index (at most one HOST may
///   cache a given session id) and a `session_cache_order` widened to carry
///   `host_id`. Both serve PLAN_M6.md item 5's merged list: the first makes
///   cross-host session-id collision — a hostile or merely buggy supervisor
///   reporting another host's session id — unconstructible rather than
///   something every lookup has to defend against separately, and the
///   second is the index the merged page query actually walks, so that
///   ordering by the full `(created_at DESC, session_id ASC, host_id ASC)`
///   key needs no sort step. Like version 2, this arrives with data that
///   predates it and resolves it by the same rule: the lowest host id keeps
///   the claim.
/// - 4: PLAN_M6_75.md item 3's live-status split, as it lands in this
///   table's PAYLOAD. `SessionStatus::Alive` was replaced rather than
///   deprecated, so a v9-era cache row's `{"state":"alive"}` no longer
///   decodes, and the read path's skip-and-log posture would silently drop
///   exactly the rows the stale list exists to serve. The migration rewrites
///   the stored spelling to `running`; it has no DDL at all, which is what
///   makes it the first entry here that is purely about DATA FORMAT rather
///   than about constraints. The `session_cache` DDL's own comment
///   anticipated this case — it is the worked example of the rule it states.
/// - 5: PLAN_M6_75.md item 5's `remembered_profiles` — the last-used
///   profile per host, which SPEC_impl.md's helm-internals section assigns
///   to helm.db rather than to the wire. This historical table remains on
///   the ladder only so old databases can reach the current schema.
/// - 6: `remembered_profiles.host_identity`, which bound that default to
///   the install it was recorded against and was revalidated on every read.
///   Removed again at version 12.
/// - 7: PLAN_M7.md item 3 — web-token authentication and device sessions.
/// - 8: PLAN_M7.md item 4 — provenance for the remembered profile default,
///   so a completed drain can advance it without letting an older snapshot
///   overwrite a newer successful create.
/// - 9: a supervisor-local creation sequence for provenance ordering. Older
///   cached supervisors omit it, so the timestamp/id ordering remains the
///   compatibility fallback until a sequenced observation arrives.
/// - 10: `session_cache.archived`, so the default view can skip archived
///   rows before decoding them (see [`HelmStore::cached_rows`]). Like version 4 this arrives with DATA that
///   predates it — the flag has always lived inside `info_json` — so the
///   migration backfills the column by decoding each existing row once.
/// - 11: `session_cache.activity_at` and `session_cache.title_sort`, plus a
///   per-host index for each, so `GET /api/sessions?sort=` can serve the
///   activity and title orders ([`ListSort`]) as index range scans. Same
///   shape as version 10 and the same reason — an order must not mean
///   decoding every payload — and the same DATA problem: both values have
///   only ever existed inside `info_json`, so the migration backfills them
///   from the rows that predate the columns. No GLOBAL index accompanies
///   either, unlike the pre-existing creation order: the per-host page query
///   of the day already read every order through a `UNION ALL` merge, so a
///   global index would never have been read by anything.
/// - 12: `remembered_profiles.host_identity` goes. The remembered default is
///   a bare profile id per registry row (SPEC.md, Sessions / Creation): not
///   bound to the install behind the row, not revalidated on read, and no
///   longer deleted by a retarget or an adoption. The rows are carried
///   forward — a same-id profile on a successor install being preselected is
///   exactly the accepted outcome — and the table is rebuilt rather than
///   `DROP COLUMN`ed so its stored DDL matches the fresh-create branch.
/// - 13: the session list is served WHOLE (SPEC.md's Session list section:
///   no pagination, no cursors, no per-order server-side indexing at any
///   layer), so every ordering index and both derived ordering columns go:
///   `session_cache_order`, `session_cache_by_host_order`,
///   `session_cache_by_host_activity_order`,
///   `session_cache_by_host_title_order`, `activity_at` and `title_sort`.
///   The merge reads a host's whole slice and sorts it in Rust, so a column
///   whose only reader was an `ORDER BY` has nothing left to serve. No DATA
///   moves: `created_at` and `archived` stay, and every row's payload is
///   untouched. Version 11's step is kept on the ladder but no longer
///   backfills what it adds, since this step drops it again in the same
///   transaction. The same step adds `hosts.cache_truncated`: whether a
///   host's cached list was cut at the wire's cap, kept with the cache so
///   the "could not read to the end" notice outlives the connection.
/// - 14: the `preferences` singleton — the list order and last-selected
///   session the helm remembers for every client at once (SPEC.md, Session
///   list). Nothing migrates INTO it: the per-client copies it replaces
///   lived in browser storage and the desktop state file, which the helm
///   never saw, so every upgraded helm starts from defaults.
/// - 15: the helm-owned `profiles` catalog and one remembered-default row.
///   The old per-host rows are dropped without migration so the catalog starts
///   with the same four seeded profiles and an empty default.
/// - 16: the nullable `hosts.alias` display label. See [`validate_alias`]
///   and `update_alias` for why uniqueness is checked against every other
///   host's current DISPLAY name (derived or aliased) rather than only
///   other aliases.
/// - 17: `session_seen` — the per-session "last seen" activity stamp behind
///   the idle/unseen-idle dot split and the mark read/mark unread toggle
///   (SPEC.md, Status). A new table, not a `session_cache` column; see the
///   table's own comment for why. Nothing migrates INTO it: no prior schema
///   recorded whether a session had ever been looked at, so every upgraded
///   helm starts every session unseen — the same "no client kept a copy of
///   this" starting point version 14's `preferences` table began from.
fn apply_schema(conn: &mut Connection) -> anyhow::Result<()> {
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .context("beginning schema transaction")?;
    let mut version: i64 = tx
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .context("reading schema version")?;
    if version == 0 {
        // Plain `CREATE TABLE`/`CREATE INDEX` — no `IF NOT EXISTS` — is
        // safe here specifically BECAUSE `BEGIN IMMEDIATE` above already
        // took SQLite's write lock before this connection's `user_version`
        // read ran. A second `open` racing this one blocks acquiring ITS
        // OWN `BEGIN IMMEDIATE` until this transaction commits or rolls
        // back, so the loser's version read — which only happens after it
        // finally gets the lock — sees the version this branch just
        // committed and takes the `version == SCHEMA_VERSION` no-op path
        // below instead of ever reaching this branch a second time (pinned
        // by `concurrent_double_open_...` in the tests). That determinism
        // is also why these statements are no longer tolerant of
        // already-existing objects: a version-0 database that already
        // contains an incompatibly-shaped table under one of these names
        // (impossible via two racing `open`s, but reachable from a
        // hand-edited or half-migrated file) must fail this whole
        // transaction loudly, rather than `IF NOT EXISTS` silently
        // accepting the foreign table and stamping `user_version = 1` as
        // though this schema had actually been applied to it — see
        // `open_fails_atomically_on_an_incompatible_preexisting_table`
        // below.
        //
        // The `PRAGMA user_version` this batch ends on MUST equal
        // `SCHEMA_VERSION` exactly — it is a bare SQL literal because a
        // string interpolated into this `execute_batch` cannot reference a
        // Rust `const`, so nothing in the compiler catches the two drifting
        // apart the way `version = SCHEMA_VERSION` just below (which only
        // updates this function's in-memory belief) would suggest. A
        // mismatch is SILENT here: this branch still reports success and the
        // fresh database still has every column the current shape needs
        // (this table literally includes `alias` below). It only surfaces on
        // a LATER open, which reads the wrong stamp back, believes a
        // migration is still owed, and reruns that migration's `ALTER TABLE
        // ADD COLUMN` against a column this branch already created — failing
        // loudly with "duplicate column name" on what looks like an
        // unrelated reopen. This exact bug shipped once (a fresh database
        // stamped 15 despite already carrying schema 16's `hosts.alias`
        // column) and is why this comment exists.
        tx.execute_batch(&format!(
            "CREATE TABLE hosts (
                 id               INTEGER PRIMARY KEY AUTOINCREMENT,
                 kind             TEXT NOT NULL CHECK (kind IN ('local', 'ssh')),
                 destination      TEXT,
                 remote_farhelm   TEXT,
                 remote_state_dir TEXT,
                 host_identity    TEXT,
                 -- Whether the cached session list below was cut at the
                 -- wire's cap when it was last written (schema version 13).
                 -- A property of the CACHE, not of the connection: a cut
                 -- list served stale after the host went down, or after a
                 -- helm restart, must still carry the could-not-read-to-the-end
                 -- notice (SPEC.md), and a flag held only in the actor's
                 -- memory would forget it in both cases. Written in the
                 -- same transaction as the rows it describes
                 -- (HelmStore::replace_host_sessions).
                 cache_truncated  INTEGER NOT NULL DEFAULT 0,
                 -- A user-chosen display name; NULL keeps the derived name
                 -- (schema version 16). LAST in this list, matching where
                 -- `ALTER TABLE hosts ADD COLUMN alias` lands it on a
                 -- database migrating up to this version — SQLite always
                 -- appends an added column, and a fresh-create branch
                 -- putting it anywhere else would leave a migrated database
                 -- with a different `sqlite_master` SQL text than a fresh
                 -- one despite being schema-equivalent
                 -- (`a_migrated_database_matches_a_freshly_created_one`
                 -- compares that text byte for byte).
                 alias            TEXT,
                 CHECK (
                     (kind = 'local' AND destination IS NULL AND remote_farhelm IS NULL
                          AND remote_state_dir IS NULL)
                     OR (kind = 'ssh' AND destination IS NOT NULL)
                 )
             ) STRICT;
             -- At most one local row: a partial unique index over a
             -- constant per-row value ('local') collapses to \"unique among
             -- rows where kind = 'local'\", i.e. at most one such row can
             -- ever exist. Enforced here rather than trusted to the API
             -- alone, per the module docs' \"SQL where cheap\" rule.
             CREATE UNIQUE INDEX hosts_one_local_row
                 ON hosts (kind) WHERE kind = 'local';
             -- Destination uniqueness applies to ssh rows only — the local
             -- row's destination is always NULL, and NULL is never equal to
             -- NULL under SQL uniqueness anyway, but the partial WHERE
             -- makes the intent explicit rather than relying on that
             -- incidental behavior.
             CREATE UNIQUE INDEX hosts_ssh_destination
                 ON hosts (destination) WHERE kind = 'ssh';
             -- At most one row may CLAIM a given host identity (schema
             -- version 2). Partial so the many rows that have not yet
             -- learned an identity (host_identity IS NULL) do not collide
             -- with each other -- SQL uniqueness already treats NULLs as
             -- distinct, but the partial WHERE states the intent instead
             -- of relying on that. This index is what makes the
             -- check-for-a-twin-then-record race unconstructible; see the
             -- module docs and HelmStore::record_first_contact.
             CREATE UNIQUE INDEX hosts_identity_claim
                 ON hosts (host_identity) WHERE host_identity IS NOT NULL;
             -- The durable, cross-restart cache of each host's last-known
             -- session list (the \"stale list\" a down host is served from,
             -- SPEC.md). Its rows persist across helm binary upgrades
             -- WITHOUT going through the hello version gate that guards
             -- live host communication (farhelm-proto's own wire
             -- version): an old row a new binary reads back must still
             -- decode. A `SessionInfo` change that is not
             -- backward-compatible with whatever rows already exist here
             -- needs its own helm schema migration (bump SCHEMA_VERSION
             -- and rewrite or drop the old rows); a purely additive
             -- `SessionInfo` field must instead stay serde-tolerant (e.g.
             -- `#[serde(default)]`) so an old row still decodes under the
             -- new shape. HelmStore::cached_sessions/cached_rows's
             -- skip-and-log read posture is the last line of defense for
             -- whatever this contract does not catch ahead of time — see
             -- those methods' own docs.
             CREATE TABLE session_cache (
                 host_id    INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
                 session_id TEXT NOT NULL,
                 -- Extracted from the stored SessionInfo JSON at write time
                 -- (HelmStore::replace_host_sessions). Since schema version
                 -- 12 nothing orders by this column -- the merge sorts in
                 -- memory -- but it stays as the identity cross-check a read
                 -- applies to a decoded payload (HelmStore::cached_rows) and
                 -- as the half of a row the changed-only rule compares
                 -- beside the payload.
                 created_at INTEGER NOT NULL,
                 -- The durable, potentially STALE serialized SessionInfo
                 -- itself — see this table's own comment above for the
                 -- cross-upgrade format contract this column is bound by.
                 info_json  TEXT NOT NULL,
                 -- Whether the payload said this session was archived,
                 -- extracted at write time exactly as created_at is and for
                 -- the same reason (schema version 10): the default view
                 -- EXCLUDES archived rows, and the column lets a read leave
                 -- an archived row undecoded rather than decoding every
                 -- payload to find out which view it belongs to.
                 --
                 -- Written from the payload, never re-derived. A row whose
                 -- info_json has since gone undecodable is dropped from every
                 -- read and every count (see HelmStore::cached_rows), so the
                 -- column's value for such a row decides nothing any more;
                 -- version 10's backfill files a payload it cannot parse as
                 -- active for want of anything better to read.
                 archived   INTEGER NOT NULL DEFAULT 0,
                 PRIMARY KEY (host_id, session_id)
             ) STRICT;
             -- At most one HOST may cache a given session id (schema
             -- version 3). Session ids are supervisor-minted UUIDs, so two
             -- hosts naming the same one is either a bug or a hostile
             -- supervisor claiming a session it does not own -- and the
             -- consequence is not cosmetic: owner lookup would resolve one
             -- host while the merged list showed another's row, so a stop
             -- aimed at one machine could land on a different one. Enforced
             -- in SQL rather than checked before writing, for the same
             -- reason hosts_identity_claim is: a check-then-write shape has
             -- a window, and this one is a routing decision.
             -- HelmStore::replace_host_sessions's conditional insert turns
             -- the refusal into a skipped row plus a warning (first claim
             -- holds) rather than a failed refresh.
             --
             -- The ONLY index on this table besides its primary key, by
             -- contract (schema version 13): the session list is served
             -- whole and sorted in memory, so an ordering index would have
             -- no reader, and SPEC.md's Session list section says none is
             -- wanted at any layer.
             CREATE UNIQUE INDEX session_cache_one_owner
                 ON session_cache (session_id);
             -- The helm-wide last profile a session was created from.
             -- (schema version 5 originally stored this per host.) The
             -- profile id is deliberately not a foreign key: a deleted
             -- profile remains visible as a dangling default so clients can
             -- ask instead of silently selecting another profile.
             -- The source_* columns are
             -- provenance for ORDERING only -- which observation of a
             -- profile-backed create is newer -- so a delayed drain cannot
             -- roll the default backward past a newer create.
             {PROFILES_SCHEMA}
             {STARTER_PROFILES}
             CREATE TABLE remembered_profile (
                 singleton     INTEGER PRIMARY KEY CHECK (singleton = 1),
                 profile_id    TEXT NOT NULL,
                 source_host_id INTEGER,
                 source_created_at INTEGER,
                 source_session_id TEXT,
                 source_creation_seq INTEGER,
                 CHECK ((source_created_at IS NULL) = (source_session_id IS NULL))
             ) STRICT;
             -- One recoverable web token. The fixed primary key makes the
             -- single-row rule a schema invariant rather than a convention
             -- shared by whichever commands happen to write it.
             CREATE TABLE web_token (
                 singleton  INTEGER PRIMARY KEY CHECK (singleton = 1),
                 token      TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             ) STRICT;
             -- Device credentials are never recoverable. Only SHA-256 output
             -- reaches this table, and the length check makes that storage
             -- contract visible to any future writer that bypasses this API.
             CREATE TABLE device_sessions (
                 cookie_hash BLOB PRIMARY KEY CHECK (length(cookie_hash) = 32),
                 created_at  INTEGER NOT NULL
             ) STRICT;
             -- The ONE client preference row (SPEC.md, Session list): the
             -- chosen list order, last user-selected session, and compact rows, shared
             -- by every client of this helm. Singleton for the same reason
             -- web_token is: no client keeps its own copy, so there is
             -- exactly one answer to remember. Preference columns are nullable — an
             -- unset preference is a real state (the default) and the row
             -- may hold one without the other.
             CREATE TABLE preferences (
                 singleton     INTEGER PRIMARY KEY CHECK (singleton = 1),
                 list_sort     TEXT,
                 last_selected TEXT,
                 compact       INTEGER CHECK (compact IN (0, 1))
             ) STRICT;
             {SESSION_SEEN_SCHEMA}
             -- Must equal SCHEMA_VERSION exactly — see the Rust comment
             -- above this whole `execute_batch` call for what goes wrong
             -- when the two drift.
             PRAGMA user_version = 18;",
        ))
        .context("creating schema")?;
        version = SCHEMA_VERSION;
    }
    if version == 1 {
        // The constraint arrives with data that predates it, so the
        // resolution rule has to be stated rather than left to whichever
        // row SQLite happens to reject: the LOWEST HostId keeps its claim
        // (ids are assigned in registration order and never recycled, so
        // this is "the entry that learned it first"), and every later row
        // sharing that identity has its claim erased.
        //
        // Erasing a claim is not the same as forgetting the host: those
        // rows re-learn their identity at the next contact and, finding it
        // taken, freeze as duplicates properly — visible, resolvable, and
        // exactly what they should have been all along. Their cache rows
        // go with the claim for the same reason
        // `HelmStore::adopt_identity` purges: a cache is only meaningful
        // under the identity it was fetched for, and this row no longer
        // claims that identity. Order matters — the cache delete reads the
        // claims the UPDATE is about to erase.
        tx.execute_batch(
            "DELETE FROM session_cache WHERE host_id IN (
                 SELECT h.id FROM hosts h
                 WHERE h.host_identity IS NOT NULL
                   AND h.id > (SELECT MIN(o.id) FROM hosts o
                               WHERE o.host_identity = h.host_identity)
             );
             UPDATE hosts SET host_identity = NULL
             WHERE host_identity IS NOT NULL
               AND id > (SELECT MIN(o.id) FROM hosts o
                         WHERE o.host_identity = hosts.host_identity);
             CREATE UNIQUE INDEX hosts_identity_claim
                 ON hosts (host_identity) WHERE host_identity IS NOT NULL;
             PRAGMA user_version = 2;",
        )
        .context("migrating helm.db to schema version 2")?;
        version = 2;
    }
    if version == 2 {
        // Same shape as the version-2 migration above, and the same
        // resolution rule for the data that predates the constraint: where
        // two hosts cached one session id, the LOWEST host id keeps the
        // claim and the later rows go. "Later" is the honest reading of the
        // ambiguity — ids are assigned in registration order and never
        // recycled — and dropping a duplicate row costs nothing real, since
        // the owning host re-drains its whole list on its next refresh.
        //
        // The order-index rebuild is separate from the dedupe only because
        // the unique index cannot be created until the duplicates are gone.
        tx.execute_batch(
            "DELETE FROM session_cache
             WHERE host_id > (SELECT MIN(o.host_id) FROM session_cache o
                              WHERE o.session_id = session_cache.session_id);
             CREATE UNIQUE INDEX session_cache_one_owner
                 ON session_cache (session_id);
             DROP INDEX session_cache_order;
             CREATE INDEX session_cache_order
                 ON session_cache (created_at DESC, session_id ASC, host_id ASC);
             PRAGMA user_version = 3;",
        )
        .context("migrating helm.db to schema version 3")?;
        version = 3;
    }
    if version == 3 {
        // farhelm-proto's `PROTOCOL_VERSION` 10 REPLACED `SessionStatus::
        // Alive` with `Running`/`Waiting`/`Idle` (PLAN_M6_75.md item 3), and
        // a replaced tagged variant does not decode — so every cache row
        // this helm wrote before the upgrade carries a `status` its new
        // binary rejects outright.
        //
        // This is exactly the case the `session_cache` DDL above warns
        // about, and the consequence is the one that makes it worth a
        // migration rather than a shrug: the read path
        // (`cached_sessions`/`cached_rows`) SKIPS an undecodable row and
        // logs it, so without this the sessions of every DOWN host would
        // quietly VANISH from the list on the first start after an upgrade
        // — the stale-list promise SPEC.md makes for an unreachable host,
        // silently broken, with nothing on screen to say why.
        //
        // `running` is the target for the same reason the supervisor
        // classifies every live pane that way at this step: it is the
        // closest reading of what `alive` meant. A cached status is
        // last-known-and-possibly-stale by construction, and the owning
        // host's next successful drain replaces it wholesale.
        //
        // ## Why a substring rewrite is safe here
        //
        // `info_json` is `serde_json`'s compact encoding of `SessionInfo`,
        // so the status field is the literal byte sequence below — no
        // spaces, no reordering (serde emits struct fields in declaration
        // order). The same bytes CANNOT occur inside any user-controlled
        // string field (a title, a cwd, an invocation): serde escapes the
        // quotes inside a string as `\"`, so a session titled
        // `"status":{"state":"alive"}` is stored with backslashes and does
        // not match. Anchoring on `"status":` rather than on the bare state
        // object narrows it further, to the one field that is a
        // `SessionStatus` at all.
        //
        // STORAGE ONLY. The WIRE must keep rejecting `alive` — that
        // rejection is what makes a version skew visible at the handshake
        // (see `PROTOCOL_VERSION`'s own docs, and the proto tests that pin
        // the refusal). This rewrites rows already at rest in this helm's
        // own database, which no handshake guards and no peer ever sees.
        tx.execute_batch(
            "UPDATE session_cache
             SET info_json = replace(
                 info_json,
                 '\"status\":{\"state\":\"alive\"}',
                 '\"status\":{\"state\":\"running\"}'
             )
             WHERE info_json LIKE '%\"status\":{\"state\":\"alive\"}%';
             PRAGMA user_version = 4;",
        )
        .context("migrating helm.db to schema version 4")?;
        version = 4;
    }
    if version == 4 {
        // Pure DDL with no data to resolve: nothing before this version
        // remembered a default, so the table starts empty and every host
        // simply has no remembered profile until the first profile-backed
        // create on it. See the fresh-create branch above for the shape and
        // for why the profile id references nothing.
        tx.execute_batch(
            "CREATE TABLE remembered_profiles (
                 host_id    INTEGER PRIMARY KEY
                            REFERENCES hosts (id) ON DELETE CASCADE,
                 profile_id TEXT NOT NULL
             ) STRICT;
             PRAGMA user_version = 5;",
        )
        .context("migrating helm.db to schema version 5")?;
        version = 5;
    }
    if version == 5 {
        // Version 6 made the remembered default identity-bound at rest, and
        // this rung originally DROPPED the version-5 rows: a bare
        // (host_id, profile_id) had nothing to validate against the new
        // column. Version 12 removed the binding again, so the FINAL
        // contract is exactly the bare shape version 5 shipped — and the
        // rows are now carried forward with a NULL identity instead of
        // being forgotten across an upgrade that lands where they started.
        // Reshaping a historical rung is safe here because nothing between
        // this step and version 12 READS the table (later rungs only
        // reshape it): the NULL that version-6-era code would have
        // mistrusted never meets that code on the upgrade path.
        //
        // Rebuilt rather than `ALTER TABLE ... ADD COLUMN`, so the stored
        // DDL is byte-identical to what the fresh-create branch wrote in
        // that era (pinned by `a_migrated_database_matches_a_freshly_created_one`).
        tx.execute_batch(
            "ALTER TABLE remembered_profiles RENAME TO remembered_profiles_v5;
             CREATE TABLE remembered_profiles (
                 host_id       INTEGER PRIMARY KEY
                               REFERENCES hosts (id) ON DELETE CASCADE,
                 profile_id    TEXT NOT NULL,
                 host_identity TEXT
             ) STRICT;
             INSERT INTO remembered_profiles (host_id, profile_id)
             SELECT host_id, profile_id FROM remembered_profiles_v5;
             DROP TABLE remembered_profiles_v5;
             PRAGMA user_version = 6;",
        )
        .context("migrating helm.db to schema version 6")?;
        version = 6;
    }
    if version == 6 {
        tx.execute_batch(
            "CREATE TABLE web_token (
                 singleton  INTEGER PRIMARY KEY CHECK (singleton = 1),
                 token      TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             ) STRICT;
             CREATE TABLE device_sessions (
                 cookie_hash BLOB PRIMARY KEY CHECK (length(cookie_hash) = 32),
                 created_at  INTEGER NOT NULL
             ) STRICT;
             PRAGMA user_version = 7;",
        )
        .context("migrating helm.db to schema version 7")?;
        version = 7;
    }
    if version == 7 {
        // Rebuild rather than ALTER so an upgraded database and a fresh one
        // carry the same schema text. Existing preferences survive, but
        // their provenance starts unknown; the first successful drain can
        // then establish a trustworthy ordering point.
        tx.execute_batch(
            "ALTER TABLE remembered_profiles RENAME TO remembered_profiles_v7;
             CREATE TABLE remembered_profiles (
                 host_id       INTEGER PRIMARY KEY
                               REFERENCES hosts (id) ON DELETE CASCADE,
                 profile_id    TEXT NOT NULL,
                 host_identity TEXT,
                 source_created_at INTEGER,
                 source_session_id TEXT,
                 CHECK ((source_created_at IS NULL) = (source_session_id IS NULL))
             ) STRICT;
             INSERT INTO remembered_profiles (host_id, profile_id, host_identity)
             SELECT host_id, profile_id, host_identity FROM remembered_profiles_v7;
             DROP TABLE remembered_profiles_v7;
             PRAGMA user_version = 8;",
        )
        .context("migrating helm.db to schema version 8")?;
        version = 8;
    }
    if version == 8 {
        tx.execute_batch(
            "ALTER TABLE remembered_profiles ADD COLUMN source_creation_seq INTEGER;
             PRAGMA user_version = 9;",
        )
        .context("migrating helm.db to schema version 9")?;
        version = 9;
    }
    if version == 9 {
        // The archive flag has always been INSIDE `info_json`; version 10
        // lifts a copy of it out beside `created_at` so the default view's
        // own count is an indexed-scope `COUNT(*)` rather than a decode of
        // the whole fleet. Every row already here predates the column, so
        // the backfill is the migration: read the flag out of each payload
        // ONCE, here, or the first read after the upgrade would report every
        // archived session as part of the default view.
        //
        // `ALTER TABLE ... ADD COLUMN` rather than the rebuild versions 6
        // and 8 used, and that is safe for the schema-agreement invariant
        // (`a_migrated_database_matches_a_freshly_created_one`) precisely
        // because SQLite splices the new definition in after the LAST COLUMN
        // rather than at the end of the statement — which lands it exactly
        // where the fresh-create branch above writes it, before the table
        // constraint. A rebuild would also have meant copying every cached
        // row for a column the next statement fills in anyway.
        //
        // A payload SQLite cannot parse as JSON, or that carries no
        // `archived` member at all, keeps the column's `0` default: the row
        // stays CLASSIFIED as active in the database, recoverable by any
        // future repair that can read it. Serving is a different story —
        // today's reads (HelmStore::cached_rows) drop a row whose payload
        // does not decode from both the rows and the counts, so this
        // classification decides nothing on screen until the payload is
        // readable again.
        tx.execute_batch(
            "ALTER TABLE session_cache ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;",
        )
        .context("adding the session_cache archive column")?;
        // ONE statement over the whole table, deliberately, and the reason is
        // memory rather than elegance: the shape this replaced read every
        // cached row into Rust to decode it, which made the peak cost of an
        // upgrade proportional to the fleet's cache. SQLite streams this and
        // holds a row at a time.
        //
        // It asks the payload TEXT rather than `SessionInfo`, which is a
        // deliberate difference from every other reader here. `json_extract`
        // wants only the one member, so a payload written by a NEWER farhelm
        // — carrying fields this build's struct would reject — is still
        // classified correctly, where a `serde_json` decode would have
        // silently filed it as active. That direction matters because the
        // cache is explicitly a cross-version format (see `session_cache`'s
        // own comment). JSON1 is compiled into the bundled SQLite and has
        // been built in since 3.38, so the functions are always present.
        //
        // `json_valid` guards the extract rather than relying on it to
        // return NULL: `json_extract` raises on malformed JSON, and a
        // poisoned row must not abort an upgrade.
        tx.execute_batch(
            "UPDATE session_cache SET archived = 1 \
             WHERE json_valid(info_json) AND json_extract(info_json, '$.archived') = 1;",
        )
        .context("backfilling the session_cache archive flags")?;
        tx.execute_batch("PRAGMA user_version = 10;")
            .context("migrating helm.db to schema version 10")?;
        version = 10;
    }
    if version == 10 {
        // Version 11 added two denormalized ordering columns so the paged
        // list of the day could serve its activity and title orders as
        // index range scans. Kept as a rung so the ladder still climbs from
        // a version-10 file; see version 13 below for why the rung is now
        // hollow.
        //
        // `ADD COLUMN` for version 10's reason too (SQLite splices each new
        // definition in after the last COLUMN, which is where the
        // fresh-create branch writes them, keeping
        // `a_migrated_database_matches_a_freshly_created_one` honest), and
        // the defaults are what make `NOT NULL` legal on an existing table.
        tx.execute_batch(
            "ALTER TABLE session_cache ADD COLUMN activity_at INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE session_cache ADD COLUMN title_sort TEXT NOT NULL DEFAULT '';",
        )
        .context("adding the session_cache ordering columns")?;
        // Version 11 backfilled both columns here (a two-statement activity
        // backfill and a batched, Rust-folded title backfill) and then built
        // the two per-host ordering indexes. None of that survives: version
        // 13, applied in this same transaction, drops both columns and every
        // ordering index, so filling them would be work whose only result
        // is thrown away a statement later. The columns are still added so
        // the ladder's every rung leaves the schema a version-11 binary
        // wrote, and the test that compares a migrated database against a
        // fresh one keeps meaning what it says.
        tx.execute_batch("PRAGMA user_version = 11;")
            .context("migrating helm.db to schema version 11")?;
        version = 11;
    }
    if version == 11 {
        // A rebuild rather than `ALTER TABLE ... DROP COLUMN`, so the stored
        // DDL is byte-identical to the fresh-create branch's (pinned by
        // `a_migrated_database_matches_a_freshly_created_one`). Every row
        // survives with its provenance: the identity column was the only
        // thing that could make a stored default unreadable, and without it
        // each row is simply the last profile used on that registry row.
        tx.execute_batch(
            "ALTER TABLE remembered_profiles RENAME TO remembered_profiles_v11;
             CREATE TABLE remembered_profiles (
                 host_id       INTEGER PRIMARY KEY
                               REFERENCES hosts (id) ON DELETE CASCADE,
                 profile_id    TEXT NOT NULL,
                 source_created_at INTEGER,
                 source_session_id TEXT,
                 source_creation_seq INTEGER,
                 CHECK ((source_created_at IS NULL) = (source_session_id IS NULL))
             ) STRICT;
             INSERT INTO remembered_profiles (
                 host_id, profile_id, source_created_at, source_session_id, source_creation_seq
             )
             SELECT host_id, profile_id, source_created_at, source_session_id, source_creation_seq
             FROM remembered_profiles_v11;
             DROP TABLE remembered_profiles_v11;
             PRAGMA user_version = 12;",
        )
        .context("migrating helm.db to schema version 12")?;
        version = 12;
    }
    if version == 12 {
        // The session list is served WHOLE (SPEC.md's Session list section):
        // no layer paginates it, so no order is ever an index range scan and
        // the two derived ordering columns have no reader left. Indexes go
        // first because SQLite refuses to drop a column an index names.
        // `session_cache_by_host_order` served the per-host stale read too,
        // and that read now sorts the handful of rows it decodes in Rust
        // (HelmStore::cached_sessions) — the primary key already makes the
        // host's slice a range.
        //
        // The one thing version 13 ADDS is `hosts.cache_truncated`: the
        // per-host record of whether the cached list was cut at the wire's
        // cap, which has to live beside the rows it describes so the notice
        // survives the host going down and the helm restarting (see the
        // column's comment in the fresh-create branch). It starts false for
        // every migrated host and is set by the next refresh.
        //
        // Nothing here touches a payload or a row's `created_at`/`archived`,
        // so a version-13 binary rolling back to 12 loses nothing but the
        // columns, and a version-12 binary would refuse the file at its
        // version gate rather than misread it.
        tx.execute_batch(
            "DROP INDEX IF EXISTS session_cache_order;
             DROP INDEX IF EXISTS session_cache_by_host_order;
             DROP INDEX IF EXISTS session_cache_by_host_activity_order;
             DROP INDEX IF EXISTS session_cache_by_host_title_order;
             ALTER TABLE session_cache DROP COLUMN activity_at;
             ALTER TABLE session_cache DROP COLUMN title_sort;
             ALTER TABLE hosts ADD COLUMN cache_truncated INTEGER NOT NULL DEFAULT 0;
             PRAGMA user_version = 13;",
        )
        .context("migrating helm.db to schema version 13")?;
        version = 13;
    }
    if version == 13 {
        // Version 14 moves the remembered list order and last-selected
        // session from each client's own storage (browser localStorage,
        // the desktop app's state file) into the helm, as one row every
        // client reads and writes. Nothing is migrated INTO it: the old
        // per-client copies were never visible to the helm, and starting
        // from defaults is exactly what SPEC.md's best-effort clause
        // allows a helm that lost the preference.
        tx.execute_batch(
            "CREATE TABLE preferences (
                 singleton     INTEGER PRIMARY KEY CHECK (singleton = 1),
                 list_sort     TEXT,
                 last_selected TEXT
             ) STRICT;
             PRAGMA user_version = 14;",
        )
        .context("migrating helm.db to schema version 14")?;
        version = 14;
    }
    if version == 14 {
        // Profile definitions now belong to this helm, while the old
        // remembered values were host-scoped and therefore have no valid
        // migration target. Dropping them is intentional: the new singleton
        // starts empty, and the catalog is seeded exactly once here.
        tx.execute_batch(&format!(
            "DROP TABLE remembered_profiles;
             {PROFILES_SCHEMA}
             {STARTER_PROFILES}
             CREATE TABLE remembered_profile (
                 singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                 profile_id TEXT NOT NULL,
                 source_host_id INTEGER,
                 source_created_at INTEGER,
                 source_session_id TEXT,
                 source_creation_seq INTEGER,
                 CHECK ((source_created_at IS NULL) = (source_session_id IS NULL))
             ) STRICT;
             PRAGMA user_version = 15;"
        ))
        .context("migrating helm.db to schema version 15")?;
        version = 15;
    }
    if version == 15 {
        tx.execute_batch("ALTER TABLE hosts ADD COLUMN alias TEXT; PRAGMA user_version = 16;")
            .context("migrating helm.db to schema version 16")?;
        version = 16;
    }
    if version == 16 {
        // Pure DDL, nothing to backfill: no prior schema recorded whether a
        // session had ever been looked at, so every session in an upgraded
        // database starts unseen — the same starting point version 14's
        // `preferences` table began from, and for the same reason (the fact
        // never lived anywhere the helm could read it back from).
        tx.execute_batch(&format!("{SESSION_SEEN_SCHEMA} PRAGMA user_version = 17;"))
            .context("migrating helm.db to schema version 17")?;
        version = 17;
    }
    if version == 17 {
        // Compactness is a boolean presentation preference, but it belongs
        // in the same shared row as sort and selection so a browser and the
        // desktop client seed the same layout after their next read.
        tx.execute_batch(
            "ALTER TABLE preferences ADD COLUMN compact INTEGER CHECK (compact IN (0, 1)); \
             PRAGMA user_version = 18;",
        )
        .context("migrating helm.db to schema version 18")?;
        version = 18;
    }
    if version == SCHEMA_VERSION {
        // Nothing to change; commit the otherwise-empty transaction to
        // release the write lock cleanly rather than leaving it to an
        // implicit rollback on drop.
        tx.commit().context("committing schema no-op")?;
        return Ok(());
    }
    anyhow::bail!(
        "helm.db has schema version {version}, but this build only understands version \
         {SCHEMA_VERSION}; refusing to open it rather than risk misreading it"
    )
}

/// Mint the reserved local row if no row of [`HostKind::Local`] exists yet.
///
/// The conditional `ON CONFLICT ... DO NOTHING` — targeting the SAME
/// partial unique index [`apply_schema`] creates — is what makes this safe
/// under a genuine concurrent race between two independent `Connection`s
/// opening the same fresh file (the `concurrent_double_open_...` test
/// below): whichever racer's `INSERT` lands first satisfies the index, and
/// the loser's conflicts against it and is silently dropped rather than
/// erroring or creating a second row. Idempotent across any number of
/// calls, in this process or any other — exactly the invariant `open`'s own
/// docs promise: the local row always exists after `open` returns, with no
/// caller ever needing to check for it first.
fn ensure_local_row(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO hosts (kind) VALUES ('local') \
         ON CONFLICT (kind) WHERE kind = 'local' DO NOTHING",
        [],
    )
    .context("minting the reserved local host row")?;
    Ok(())
}

/// One row's recorded identity together with the connection-defining
/// configuration it currently carries, or `None` if no such row exists.
///
/// Shared by the two identity writers because both must answer the same
/// pair of questions — "is this row's identity what I think it is" and "is
/// this row still the host I dialed" — from ONE read inside their own
/// transaction. Two separate reads would be two separate answers.
fn read_identity_and_config(
    tx: &rusqlite::Transaction<'_>,
    host: HostId,
) -> anyhow::Result<Option<(Option<String>, DialedAs)>> {
    tx.query_row(
        "SELECT host_identity, destination, remote_farhelm, remote_state_dir \
         FROM hosts WHERE id = ?1",
        rusqlite::params![host],
        |r| {
            Ok((
                r.get(0)?,
                DialedAs {
                    destination: r.get(1)?,
                    remote_farhelm: r.get(2)?,
                    remote_state_dir: r.get(3)?,
                },
            ))
        },
    )
    .optional()
    .context("reading the host's identity and configuration")
}

/// The row OTHER than `host` that currently claims `identity`, if any.
///
/// The `hosts_identity_claim` index guarantees there is at most one, so
/// this is a lookup rather than a scan-and-choose. Called inside the
/// caller's own write transaction, never before it: the whole point is that
/// the answer cannot change between being read and being acted on.
fn claimant_of(
    tx: &rusqlite::Transaction<'_>,
    host: HostId,
    identity: &str,
) -> anyhow::Result<Option<HostId>> {
    tx.query_row(
        "SELECT id FROM hosts WHERE host_identity = ?1 AND id <> ?2",
        rusqlite::params![identity, host],
        |r| r.get(0),
    )
    .optional()
    .context("looking up the current claimant of a host identity")
}

impl HelmStore {
    /// Open (or create) `helm.db` at `path`, applying the schema and
    /// minting the reserved local row if either is missing.
    ///
    /// The always-present local-row invariant lives HERE rather than in
    /// every caller that might need it (PLAN_M6.md item 3, catching a
    /// review finding from the plan's own first draft: a row-less local
    /// host would have nowhere to cache its sessions, which is exactly what
    /// lets a stopped local supervisor's sessions serve stale like any
    /// other host's). A caller never checks whether the local row exists;
    /// it simply does, unconditionally, once this returns `Ok`.
    ///
    /// As with the supervisor store's `open`, confidentiality of what this
    /// stores — a cached `SessionInfo.invocation` may embed credentials
    /// passed on an agent's command line, exactly like the supervisor's own
    /// launch specs — rests on the state directory's `0700` mode, which
    /// every caller of this function is required to have already
    /// established (`farhelm_supervisor::ensure_private_dir`, already
    /// called for this same directory by `lib.rs` today). The
    /// `set_permissions` call below narrows the file's own mode as a repair
    /// for whatever the ambient umask left behind, not as the actual
    /// boundary — rusqlite creates the file before this function can touch
    /// it, so a permissive umask leaves a create-then-chmod window only the
    /// private directory itself closes.
    ///
    /// No `may_migrate` parameter, unlike the supervisor store's `open` —
    /// see the module docs' "Divergences" section for why the handoff
    /// scenario that parameter exists for cannot happen here.
    pub async fn open(path: &Path) -> anyhow::Result<HelmStore> {
        let path = path.to_path_buf();
        let conn = tokio::task::spawn_blocking(move || -> anyhow::Result<Connection> {
            // Explicit flags rather than `Connection::open`'s defaults —
            // see the supervisor store's identical `open` for why
            // `SQLITE_OPEN_URI` is deliberately left out (a state-dir path
            // is not attacker input, but URI reinterpretation is not a
            // feature this module wants regardless) and why
            // `SQLITE_OPEN_NO_MUTEX` is correct (this module already
            // serializes every access through its own `Mutex`).
            let mut conn = Connection::open_with_flags(
                &path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .with_context(|| format!("opening helm database {}", path.display()))?;
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                    .with_context(|| format!("restricting mode of {}", path.display()))?;
            }
            conn.busy_timeout(BUSY_TIMEOUT)
                .context("setting sqlite busy timeout")?;
            // Not durable in the database file — SQLite enforces foreign
            // keys only when a connection has asked for it, so every open
            // must set this pragma itself. See the module docs'
            // "Divergences" section for why this schema needs it at all
            // (the supervisor's does not) and `apply_schema`'s
            // `session_cache` DDL for the `ON DELETE CASCADE` it enables.
            conn.pragma_update(None, "foreign_keys", true)
                .context("enabling sqlite foreign key enforcement")?;
            apply_schema(&mut conn)?;
            ensure_local_row(&conn)?;
            Ok(conn)
        })
        .await
        .context("helm store open task panicked")??;
        Ok(HelmStore {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Return the recoverable web token, inserting `candidate` if this helm
    /// has never minted one before.
    ///
    /// The insert and read share one immediate transaction so concurrent
    /// `token show` callers converge on the token that actually committed.
    /// A caller must never assume its candidate won merely because the row
    /// Read the recoverable web token without creating one.
    ///
    /// Keeping this read separate lets callers avoid consuming randomness or
    /// consulting the clock on the overwhelmingly common existing-token path.
    pub async fn web_token(&self) -> anyhow::Result<Option<String>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
            conn.lock()
                .expect("helm db mutex poisoned")
                .query_row(
                    "SELECT token FROM web_token WHERE singleton = 1",
                    [],
                    |row| row.get(0),
                )
                .optional()
                .context("reading the web token")
        })
        .await
        .context("web token read task panicked")?
    }

    /// Commit `candidate` only if the singleton token was still absent.
    ///
    /// Two fresh processes may both decide they need to mint. The immediate
    /// transaction makes one candidate authoritative and returns that same
    /// committed value to both callers.
    pub async fn web_token_or_insert(
        &self,
        candidate: String,
        created_at: i64,
    ) -> anyhow::Result<String> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .context("beginning web-token transaction")?;
            tx.execute(
                "INSERT INTO web_token (singleton, token, created_at) VALUES (1, ?1, ?2) \
                 ON CONFLICT (singleton) DO NOTHING",
                rusqlite::params![candidate, created_at],
            )
            .context("minting the web token")?;
            let token = tx
                .query_row(
                    "SELECT token FROM web_token WHERE singleton = 1",
                    [],
                    |row| row.get(0),
                )
                .context("reading the web token")?;
            tx.commit().context("committing the web token")?;
            Ok(token)
        })
        .await
        .context("web token task panicked")?
    }

    /// Replace the web token and invalidate every device session atomically.
    ///
    /// Neither half may commit alone: a new token with old credentials still
    /// admitted would make rotation lie, while deleted credentials paired with
    /// the old token would log every device out without changing the secret
    /// the user asked to rotate.
    pub async fn rotate_web_token(
        &self,
        replacement: String,
        created_at: i64,
    ) -> anyhow::Result<String> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .context("beginning web-token rotation")?;
            tx.execute(
                "INSERT INTO web_token (singleton, token, created_at) VALUES (1, ?1, ?2) \
                 ON CONFLICT (singleton) DO UPDATE SET token = excluded.token, \
                     created_at = excluded.created_at",
                rusqlite::params![replacement, created_at],
            )
            .context("replacing the web token")?;
            tx.execute("DELETE FROM device_sessions", [])
                .context("deleting device sessions during token rotation")?;
            tx.commit().context("committing web-token rotation")?;
            Ok(replacement)
        })
        .await
        .context("web token rotation task panicked")?
    }

    /// Validate a bootstrap token and record its device credential atomically.
    ///
    /// The immediate transaction serializes this decision with rotation. An
    /// exchange using the old token therefore either commits before rotation
    /// and is deleted by it, or validates afterwards and is refused.
    pub async fn exchange_device_session(
        &self,
        supplied_token: String,
        device_hash: [u8; 32],
        created_at: i64,
    ) -> anyhow::Result<bool> {
        self.exchange_device_session_inner(supplied_token, device_hash, created_at, None)
            .await
    }

    /// Pause a test after validation while the immediate transaction remains
    /// open, exposing the exact exchange-versus-rotation serialization seam.
    #[cfg(test)]
    pub(crate) async fn exchange_device_session_with_pause(
        &self,
        supplied_token: String,
        device_hash: [u8; 32],
        created_at: i64,
        after_validation: Box<dyn FnOnce() + Send>,
    ) -> anyhow::Result<bool> {
        self.exchange_device_session_inner(
            supplied_token,
            device_hash,
            created_at,
            Some(after_validation),
        )
        .await
    }

    /// Shared transaction body with an optional test-only pause after
    /// validation. The pause runs while the immediate transaction is held so
    /// a race test can prove rotation waits at the real serialization point.
    async fn exchange_device_session_inner(
        &self,
        supplied_token: String,
        device_hash: [u8; 32],
        created_at: i64,
        after_validation: Option<Box<dyn FnOnce() + Send>>,
    ) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .context("beginning device exchange")?;
            let expected: Option<String> = tx
                .query_row(
                    "SELECT token FROM web_token WHERE singleton = 1",
                    [],
                    |row| row.get(0),
                )
                .optional()
                .context("reading the web token during device exchange")?;
            let accepted = expected.is_some_and(|expected| {
                Sha256::digest(expected.as_bytes())
                    .ct_eq(&Sha256::digest(supplied_token.as_bytes()))
                    .into()
            });
            if !accepted {
                return Ok(false);
            }
            if let Some(after_validation) = after_validation {
                after_validation();
            }
            tx.execute(
                "INSERT INTO device_sessions (cookie_hash, created_at) VALUES (?1, ?2) \
                     ON CONFLICT (cookie_hash) DO NOTHING",
                rusqlite::params![device_hash.as_slice(), created_at],
            )
            .context("recording a device session")?;
            tx.execute(
                "DELETE FROM device_sessions WHERE cookie_hash IN (\
                     SELECT cookie_hash FROM device_sessions \
                     ORDER BY created_at DESC, cookie_hash DESC LIMIT -1 OFFSET ?1\
                 )",
                [i64::try_from(MAX_DEVICE_SESSIONS).expect("device-session cap fits i64")],
            )
            .context("evicting old device sessions")?;
            tx.commit().context("committing device exchange")?;
            Ok(true)
        })
        .await
        .context("device exchange task panicked")?
    }

    /// Test whether an exact device digest is present through its primary-key
    /// index. The digest is public output of SHA-256, so SQLite's equality
    /// lookup reveals no useful secret-dependent prefix information.
    pub async fn has_device_session(&self, device_hash: [u8; 32]) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM device_sessions WHERE cookie_hash = ?1)",
                [device_hash.as_slice()],
                |row| row.get(0),
            )
            .context("looking up a device session")
        })
        .await
        .context("device-session read task panicked")?
    }

    /// Insert a device digest without token validation for storage fixtures.
    #[cfg(test)]
    pub(crate) async fn insert_device_session(
        &self,
        device_hash: [u8; 32],
        created_at: i64,
    ) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            conn.lock()
                .expect("helm db mutex poisoned")
                .execute(
                    "INSERT INTO device_sessions (cookie_hash, created_at) VALUES (?1, ?2)",
                    rusqlite::params![device_hash.as_slice(), created_at],
                )
                .context("inserting a fixture device session")?;
            Ok(())
        })
        .await
        .context("fixture device-session insert task panicked")?
    }

    /// Decode every digest for storage tests that assert exact rotation rows.
    #[cfg(test)]
    pub(crate) async fn device_session_hashes(&self) -> anyhow::Result<Vec<[u8; 32]>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<[u8; 32]>> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            let mut stmt = conn
                .prepare("SELECT cookie_hash FROM device_sessions ORDER BY cookie_hash")
                .context("preparing fixture device-session read")?;
            stmt.query_map([], |row| {
                let bytes: Vec<u8> = row.get(0)?;
                bytes.try_into().map_err(|bytes: Vec<u8>| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        format!("device-session digest is {} bytes", bytes.len()).into(),
                    )
                })
            })
            .context("reading fixture device sessions")?
            .collect::<Result<Vec<_>, _>>()
            .context("decoding fixture device-session digests")
        })
        .await
        .context("fixture device-session read task panicked")?
    }

    /// Count retained device credentials for rotation and bound tests.
    #[cfg(test)]
    pub(crate) async fn device_session_count(&self) -> anyhow::Result<usize> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<usize> {
            let count: i64 = conn
                .lock()
                .expect("helm db mutex poisoned")
                .query_row("SELECT COUNT(*) FROM device_sessions", [], |row| row.get(0))
                .context("counting device sessions")?;
            usize::try_from(count).context("device-session count does not fit usize")
        })
        .await
        .context("device-session count task panicked")?
    }

    /// Install a deterministic device-insert failure for authentication error
    /// surface tests.
    #[cfg(test)]
    pub(crate) async fn refuse_device_inserts_for_test(&self) {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            conn.lock()
                .expect("helm db mutex poisoned")
                .execute_batch(
                    "CREATE TRIGGER refuse_auth_insert BEFORE INSERT ON device_sessions \
                     BEGIN SELECT RAISE(ABORT, 'secret database detail'); END;",
                )
                .expect("install device-insert refusal");
        })
        .await
        .expect("device-insert refusal task panicked");
    }

    // ---- The shared client preference ---------------------------------

    /// The one preference row, or the all-unset default when no client has
    /// written it yet.
    ///
    /// "Unset" and "absent row" deliberately read the same: a client seeds
    /// its list order and auto-select from whatever comes back, and neither
    /// answer should make it behave differently from a fresh install.
    pub async fn preferences(&self) -> anyhow::Result<Preferences> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Preferences> {
            conn.lock()
                .expect("helm db mutex poisoned")
                .query_row(
                    "SELECT list_sort, last_selected, compact FROM preferences WHERE singleton = 1",
                    [],
                    |row| {
                        Ok(Preferences {
                            list_sort: row.get(0)?,
                            last_selected: row.get(1)?,
                            compact: row.get(2)?,
                        })
                    },
                )
                .optional()
                .map(Option::unwrap_or_default)
                .context("reading the client preference")
        })
        .await
        .context("preference read task panicked")?
    }

    /// Merge `patch` into the preference row: an absent field is left as it
    /// was, a `Some(None)` clears it, a `Some(Some(value))` replaces it.
    ///
    /// A MERGE rather than a whole-row replace, and the reason is the
    /// preference being shared: two clients change different fields at
    /// nearly the same time (one re-sorts, the other clicks a row), and a
    /// whole-row write from either would reinstall its own stale copy of the
    /// field the other just wrote. Sparse patches make the last writer win
    /// per FIELD, which is the only order anyone can observe anyway. The
    /// clear exists for a deselect and for test harnesses that need a "nothing
    /// remembered" precondition; the UI's ordinary writes never send one.
    ///
    /// Validation (that `list_sort` is a word this helm serves) belongs to
    /// the handler, not here: the store records what it is told, exactly as
    /// it does for every other table.
    pub async fn update_preferences(&self, patch: PreferencePatch) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            // Each field travels as (present, value): `CASE WHEN present`
            // is what lets a NULL value mean "clear" rather than "keep",
            // which a COALESCE could not express.
            let sort_present = patch.list_sort.is_some();
            let selected_present = patch.last_selected.is_some();
            let compact_present = patch.compact.is_some();
            let sort = patch.list_sort.flatten();
            let selected = patch.last_selected.flatten();
            let compact = patch.compact.flatten();
            conn.lock()
                .expect("helm db mutex poisoned")
                .execute(
                    "INSERT INTO preferences (singleton, list_sort, last_selected, compact) \
                     VALUES (1, ?1, ?2, ?3) \
                     ON CONFLICT (singleton) DO UPDATE SET \
                         list_sort = CASE WHEN ?4 THEN excluded.list_sort ELSE list_sort END, \
                         last_selected = CASE WHEN ?5 THEN excluded.last_selected \
                                              ELSE last_selected END, \
                         compact = CASE WHEN ?6 THEN excluded.compact ELSE compact END",
                    rusqlite::params![
                        sort,
                        selected,
                        compact,
                        sort_present,
                        selected_present,
                        compact_present
                    ],
                )
                .context("writing the client preference")?;
            Ok(())
        })
        .await
        .context("preference write task panicked")?
    }

    // ---- Per-session "seen" state ---------------------------------------

    /// The recorded `seen_activity_at` stamp for every id in `ids` that has
    /// one. An id with no row — never seen, or unknown to this table
    /// entirely — is simply absent from the map rather than present with a
    /// placeholder, the same "absence carries meaning" shape
    /// `SessionRow::seen_activity_at` serializes onto the wire.
    ///
    /// Bounded by the caller's own id list (`WHERE session_id IN (...)`,
    /// same shape as [`Self::cached_slice`]'s host scoping) rather than
    /// reading the whole table: the one caller today,
    /// `aggregate::session_list_staged`, already knows every id the reply
    /// will carry before it needs this map, so there is no reason to pull
    /// rows for sessions outside the current listing.
    pub async fn seen_activity(
        &self,
        ids: &[String],
    ) -> anyhow::Result<std::collections::HashMap<String, i64>> {
        if ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let conn = Arc::clone(&self.conn);
        let ids = ids.to_vec();
        tokio::task::spawn_blocking(
            move || -> anyhow::Result<std::collections::HashMap<String, i64>> {
                let conn = conn.lock().expect("helm db mutex poisoned");
                let placeholders = (1..=ids.len())
                    .map(|index| format!("?{index}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut stmt = conn
                    .prepare(&format!(
                        "SELECT session_id, seen_activity_at FROM session_seen \
                         WHERE session_id IN ({placeholders})"
                    ))
                    .context("preparing the seen-activity query")?;
                let rows: Vec<(String, i64)> = stmt
                    .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
                        Ok((r.get(0)?, r.get(1)?))
                    })
                    .context("querying seen-activity rows")?
                    .collect::<Result<_, _>>()
                    .context("reading seen-activity rows")?;
                Ok(rows.into_iter().collect())
            },
        )
        .await
        .context("seen-activity read task panicked")?
    }

    /// Upsert `session_id`'s seen stamp to `activity_at` (a manual "mark
    /// read", or the automatic mark the session view issues on open and on
    /// every activity advance), returning whether the stored value actually
    /// changed.
    ///
    /// The `WHERE` clause on the `DO UPDATE` is what makes the return value
    /// meaningful: SQLite only counts a row as touched by
    /// `Connection::execute` when the update clause's `WHERE` matched, so
    /// re-marking the SAME stamp that is already stored reports zero rows
    /// changed rather than one. That is exactly the signal the
    /// `PUT /api/sessions/{id}/seen` handler needs to honor SPEC_impl.md's
    /// "bump the fleet-events revision on a change, not on a no-op" rule —
    /// without it, reopening an already-seen session with no new activity
    /// behind it, or a client retrying a PUT whose response it missed,
    /// would re-bump the revision for nothing, waking every other
    /// connected client to redraw a dot that never moved.
    pub async fn mark_seen(&self, session_id: &str, activity_at: i64) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        let session_id = session_id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let changed = conn
                .lock()
                .expect("helm db mutex poisoned")
                .execute(
                    "INSERT INTO session_seen (session_id, seen_activity_at) VALUES (?1, ?2) \
                     ON CONFLICT (session_id) DO UPDATE SET seen_activity_at = excluded.seen_activity_at \
                     WHERE session_seen.seen_activity_at != excluded.seen_activity_at",
                    rusqlite::params![session_id, activity_at],
                )
                .context("writing the seen stamp")?;
            Ok(changed > 0)
        })
        .await
        .context("seen-stamp write task panicked")?
    }

    /// Delete `session_id`'s seen stamp — a manual "mark unread", which
    /// [`SessionInfo::effective_activity`] then reads as unseen because any
    /// recorded activity is newer than no stamp at all. Returns whether a
    /// row actually existed to delete, the clear-direction twin of
    /// [`Self::mark_seen`]'s change flag and for the same reason: clearing
    /// an already-absent stamp is a no-op the caller must not bump the
    /// fleet-events revision over.
    ///
    /// Also the primitive `sessions::delete_session` calls, unconditionally
    /// and discarding the flag, to drop this table's row when the session
    /// itself is deleted — see SPEC_impl.md's `session_seen` paragraph for
    /// why a deleted session's row does not simply cascade away on its own.
    pub async fn clear_seen(&self, session_id: &str) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        let session_id = session_id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let changed = conn
                .lock()
                .expect("helm db mutex poisoned")
                .execute(
                    "DELETE FROM session_seen WHERE session_id = ?1",
                    rusqlite::params![session_id],
                )
                .context("clearing the seen stamp")?;
            Ok(changed > 0)
        })
        .await
        .context("seen-stamp clear task panicked")?
    }

    /// Test-only failure-injection seam: drop `session_seen` out from under
    /// this store's own connection, so [`Self::clear_seen`] fails for a
    /// REAL reason (the table is gone) rather than a mock returning a
    /// canned error.
    ///
    /// `sessions_tests.rs`'s `delete_session_succeeds_even_when_clearing_
    /// the_seen_row_fails` is the one caller: it needs `sessions::
    /// delete_session`'s best-effort `clear_seen` call to genuinely fail
    /// without touching the database file directly, which `HelmStore`
    /// deliberately gives no caller outside this module a path to (unlike
    /// `store.rs`'s own migration-downgrade fixtures, which reopen the file
    /// by its OS path because they run IN this module and are testing the
    /// schema ladder itself, not a caller's error handling).
    #[cfg(test)]
    pub(crate) async fn break_session_seen_table_for_test(&self) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            conn.lock()
                .expect("helm db mutex poisoned")
                .execute_batch("DROP TABLE session_seen;")
                .context("dropping session_seen for a failure-injection test")
        })
        .await
        .context("session_seen drop task panicked")?
    }

    /// Every registered host, local row included, ordered by [`HostId`] —
    /// which, thanks to `AUTOINCREMENT`, is also insertion order: the local
    /// row first (minted at the first-ever `open`), then ssh rows in the
    /// order they were added.
    ///
    /// A row whose `kind` fails [`HostKind::from_column`] FAILS this whole
    /// call rather than being skipped — the opposite posture from
    /// [`Self::cached_sessions`]/[`Self::cached_rows`]'s
    /// skip-and-log reads (see those methods' own docs). `hosts` is the
    /// registry: it is authority for which hosts exist at all, not
    /// last-known display data a caller can afford to see a little short,
    /// so a corrupt row here must surface as an error a caller can act on
    /// instead of silently vanishing from the list.
    pub async fn list_hosts(&self) -> anyhow::Result<Vec<HostRow>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<HostRow>> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            let mut stmt = conn
                .prepare(
                    "SELECT id, kind, destination, alias, remote_farhelm, remote_state_dir, \
                     host_identity, cache_truncated FROM hosts ORDER BY id ASC",
                )
                .context("preparing host list query")?;
            let raw: Vec<RawHostRow> = stmt
                .query_map([], |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                    ))
                })
                .context("querying hosts")?
                .collect::<Result<_, _>>()
                .context("reading host rows")?;
            raw.into_iter()
                .map(
                    |(
                        id,
                        kind,
                        destination,
                        alias,
                        remote_farhelm,
                        remote_state_dir,
                        host_identity,
                        cache_truncated,
                    )| {
                        Ok(HostRow {
                            id,
                            kind: HostKind::from_column(&kind)?,
                            destination,
                            alias,
                            remote_farhelm,
                            remote_state_dir,
                            host_identity,
                            cache_truncated,
                        })
                    },
                )
                .collect()
        })
        .await
        .context("list hosts task panicked")?
    }

    /// Register a new ssh host, returning its assigned [`HostId`].
    ///
    /// `remote_farhelm`/`remote_state_dir` are the argv fields M1's
    /// `--remote-farhelm`/`--remote-state-dir` used to carry (PLAN_M6.md
    /// item 3); `None` means "use the remote's own default" at connection
    /// time, not "unset for now" — there is no later default to fall back
    /// on once a value IS set, so a caller wanting the default forever
    /// simply never supplies one.
    ///
    /// A duplicate `destination` is refused as [`HostStoreError::DuplicateDestination`]
    /// with NOTHING written — see the module docs' "Divergences" section for
    /// why this is a conditional `ON CONFLICT ... DO NOTHING` (targeting the
    /// `hosts_ssh_destination` partial unique index [`apply_schema`]
    /// creates — the exact same shape [`ensure_local_row`] uses against
    /// `hosts_one_local_row`) rather than a caught constraint violation.
    /// Creating a second local row is not a case this function can even
    /// express: it always inserts `kind = 'ssh'`, so the reserved row's
    /// uniqueness needs no enforcement on this path at all.
    ///
    /// A destination that is not usable AS a destination — empty, or
    /// option-shaped — is refused as
    /// [`HostStoreError::InvalidDestination`] before anything is written;
    /// see that variant's docs for why the registry, and not only the ssh
    /// argv builder, takes a position on this.
    ///
    /// A destination matching another host's current ALIAS is refused as
    /// [`HostStoreError::AliasTaken`], inside the same transaction as the
    /// insert (`alias_collision`'s own doc explains why only aliases, not
    /// every derived name, are compared here). Without this, a fresh
    /// registration could land a row whose derived name collides with an
    /// existing alias — nothing in the unique index would catch it, since
    /// the index only knows about destinations — and `resolve_host` would
    /// then correctly refuse the ambiguous name, silently breaking the
    /// alias as an agent target.
    pub async fn add_ssh_host(
        &self,
        destination: &str,
        remote_farhelm: Option<&str>,
        remote_state_dir: Option<&str>,
    ) -> anyhow::Result<HostId> {
        let conn = Arc::clone(&self.conn);
        let destination = destination.to_string();
        if !destination_is_usable(&destination) {
            return Err(anyhow::Error::new(HostStoreError::InvalidDestination(
                destination,
            )));
        }
        let remote_farhelm = remote_farhelm.map(str::to_string);
        let remote_state_dir = remote_state_dir.map(str::to_string);
        tokio::task::spawn_blocking(move || -> anyhow::Result<HostId> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning add ssh host transaction")?;
            if let Some(name) = alias_collision(&tx, None, &destination)? {
                return Err(anyhow::Error::new(HostStoreError::AliasTaken(name)));
            }
            let inserted = tx
                .execute(
                    // 'ssh' hardcoded directly, exactly as ensure_local_row
                    // hardcodes 'local' — this insert can only ever create
                    // an ssh row, so there is no second kind for a decode
                    // step to choose between. See HostKind::from_column's
                    // own docs for the read-side half of this vocabulary.
                    "INSERT INTO hosts (kind, destination, remote_farhelm, remote_state_dir) \
                     VALUES ('ssh', ?1, ?2, ?3) \
                     ON CONFLICT (destination) WHERE kind = 'ssh' DO NOTHING",
                    rusqlite::params![destination, remote_farhelm, remote_state_dir],
                )
                .context("inserting ssh host")?;
            if inserted == 0 {
                return Err(anyhow::Error::new(HostStoreError::DuplicateDestination(
                    destination,
                )));
            }
            let host = tx.last_insert_rowid();
            tx.commit().context("committing new ssh host")?;
            Ok(host)
        })
        .await
        .context("add ssh host task panicked")?
    }

    /// Register a supervisor discovered through one resolved SSH dial.
    ///
    /// The connection-defining paths and reported identity land in the same
    /// transaction as the row itself. An existing destination is converged
    /// to those paths, but a different stored identity is never overwritten:
    /// discovery must not turn a retarget race into silent adoption. The
    /// boolean reports whether this transaction inserted the row, so a
    /// caller whose live-registry reconciliation fails can roll back only
    /// the row it owns rather than deleting a concurrent registration.
    ///
    /// A genuinely NEW destination matching another host's current ALIAS is
    /// refused as [`HostStoreError::AliasTaken`] before the insert — see
    /// [`HelmStore::add_ssh_host`]'s doc for why this check exists at all.
    /// The CONVERGE branch (an already-registered destination) never runs
    /// it: that branch never writes `destination`, so it cannot introduce a
    /// new collision.
    pub async fn register_probed_ssh_host(
        &self,
        destination: &str,
        remote_farhelm: Option<&str>,
        remote_state_dir: Option<&str>,
        host_identity: Option<&str>,
    ) -> anyhow::Result<(HostId, bool)> {
        if !destination_is_usable(destination) {
            return Err(anyhow::Error::new(HostStoreError::InvalidDestination(
                destination.to_string(),
            )));
        }
        let conn = Arc::clone(&self.conn);
        let destination = destination.to_string();
        let remote_farhelm = remote_farhelm.map(str::to_string);
        let remote_state_dir = remote_state_dir.map(str::to_string);
        let host_identity = host_identity.map(str::to_string);
        tokio::task::spawn_blocking(move || -> anyhow::Result<(HostId, bool)> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .context("beginning discovered-host registration")?;
            let existing: Option<(HostId, Option<String>)> = tx
                .query_row(
                    "SELECT id, host_identity FROM hosts \
                     WHERE kind = 'ssh' AND destination = ?1",
                    rusqlite::params![destination],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .context("looking up a discovered ssh destination")?;

            let (host, inserted) = if let Some((host, recorded)) = existing {
                if let (Some(recorded), Some(reported)) = (&recorded, &host_identity)
                    && recorded != reported
                {
                    return Err(anyhow::Error::new(HostStoreError::IdentityMismatch {
                        host,
                        expected: recorded.clone(),
                        actual: Some(reported.clone()),
                    }));
                }
                if recorded.is_none()
                    && let Some(identity) = &host_identity
                    && let Some(owner) = claimant_of(&tx, host, identity)?
                {
                    return Err(anyhow::Error::new(HostStoreError::IdentityClaimed {
                        host,
                        identity: identity.clone(),
                        owner,
                    }));
                }
                tx.execute(
                    "UPDATE hosts SET remote_farhelm = ?2, remote_state_dir = ?3, \
                     host_identity = COALESCE(host_identity, ?4) WHERE id = ?1",
                    rusqlite::params![host, remote_farhelm, remote_state_dir, host_identity],
                )
                .context("converging the discovered ssh host")?;
                (host, false)
            } else {
                if let Some(identity) = &host_identity {
                    let owner: Option<HostId> = tx
                        .query_row(
                            "SELECT id FROM hosts WHERE host_identity = ?1",
                            rusqlite::params![identity],
                            |row| row.get(0),
                        )
                        .optional()
                        .context("checking the discovered identity claim")?;
                    if let Some(owner) = owner {
                        anyhow::bail!(
                            "host {owner} already holds discovered identity {identity:?}"
                        );
                    }
                }
                if let Some(name) = alias_collision(&tx, None, &destination)? {
                    return Err(anyhow::Error::new(HostStoreError::AliasTaken(name)));
                }
                tx.execute(
                    "INSERT INTO hosts (kind, destination, remote_farhelm, remote_state_dir, \
                     host_identity) VALUES ('ssh', ?1, ?2, ?3, ?4)",
                    rusqlite::params![destination, remote_farhelm, remote_state_dir, host_identity],
                )
                .context("inserting the discovered ssh host")?;
                (tx.last_insert_rowid(), true)
            };
            tx.commit()
                .context("committing discovered-host registration")?;
            Ok((host, inserted))
        })
        .await
        .context("register discovered ssh host task panicked")?
    }

    /// Register every destination in `entries` that is not registered
    /// already, in ONE transaction — the `--ensure-hosts` floor
    /// (PLAN_M6.md item 5), applied atomically.
    ///
    /// Atomic because a half-applied guarantee is worse than a refused one:
    /// a helm that came up with three of five guaranteed hosts looks
    /// healthy, and the two that are missing look like hosts the user forgot
    /// to add. Doing this as a loop of [`Self::add_ssh_host`] calls could
    /// not offer that — each call is its own transaction, so a failure
    /// halfway leaves the earlier ones committed. This is the same reason
    /// the ingestion path validates every entry up front, and the two
    /// together are what make the contract true rather than merely likely.
    ///
    /// ADDITIVE, never corrective: a destination already in the registry is
    /// left exactly as it is, including its `remote_farhelm`,
    /// `remote_state_dir`, and its learned identity. helm.db is the durable
    /// authority and `/api/hosts` is how it is edited; a startup file that
    /// overwrote user edits on every boot would make the two fight, and the
    /// interactive one would lose. Returns the ids actually created, in
    /// input order, so a caller can log what it added rather than what it
    /// asked for.
    ///
    /// Refuses an unusable destination ([`destination_is_usable`]) and a
    /// destination repeated within `entries` — two entries for one host
    /// would disagree about its remote binary and state directory, and
    /// silently letting the first win would make the file's meaning depend
    /// on line order.
    ///
    /// An entry that is ACTUALLY NEW (not already registered) is refused as
    /// [`HostStoreError::AliasTaken`], aborting the WHOLE batch, if its
    /// destination matches another host's current alias — see
    /// [`HelmStore::add_ssh_host`]'s doc for why. Checked only for entries
    /// this call would actually insert: an entry that is already registered
    /// changes nothing (ADDITIVE, above), so a pre-existing collision that
    /// predates this call and that this call is not creating must not fail
    /// an otherwise ordinary startup.
    pub async fn ensure_ssh_hosts(&self, entries: Vec<EnsureHost>) -> anyhow::Result<Vec<HostId>> {
        for (index, entry) in entries.iter().enumerate() {
            anyhow::ensure!(
                destination_is_usable(&entry.destination),
                "entry {index} names {:?}, which is not a usable ssh destination (it must be \
                 non-empty, must not start with '-', and must contain no NUL byte)",
                entry.destination
            );
            anyhow::ensure!(
                !entries[..index]
                    .iter()
                    .any(|earlier| earlier.destination == entry.destination),
                "entry {index} repeats destination {:?}; a destination may appear once, since \
                 two entries for one host would disagree about its remote farhelm and state \
                 directory",
                entry.destination
            );
        }
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<HostId>> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning the ensure-hosts transaction")?;
            let mut added = Vec::new();
            for entry in &entries {
                // Newness is checked FIRST, and separately from the
                // conditional insert below, specifically so the alias
                // check that follows applies only to entries this call
                // would actually create — see this method's own doc for
                // why an already-registered entry must not be blamed for a
                // collision it did not introduce.
                let already_registered = tx
                    .query_row(
                        "SELECT 1 FROM hosts WHERE kind = 'ssh' AND destination = ?1",
                        rusqlite::params![entry.destination],
                        |_| Ok(()),
                    )
                    .optional()
                    .with_context(|| {
                        format!(
                            "checking whether {:?} is already registered",
                            entry.destination
                        )
                    })?
                    .is_some();
                if !already_registered
                    && let Some(name) = alias_collision(&tx, None, &entry.destination)?
                {
                    return Err(anyhow::Error::new(HostStoreError::AliasTaken(name)));
                }
                // The same conditional insert `add_ssh_host` uses, for the
                // same reason: "already registered" is the ordinary,
                // expected outcome here, not an error to catch.
                let inserted = tx
                    .execute(
                        "INSERT INTO hosts (kind, destination, remote_farhelm, remote_state_dir) \
                         VALUES ('ssh', ?1, ?2, ?3) \
                         ON CONFLICT (destination) WHERE kind = 'ssh' DO NOTHING",
                        rusqlite::params![
                            entry.destination,
                            entry.remote_farhelm,
                            entry.remote_state_dir
                        ],
                    )
                    .with_context(|| {
                        format!("registering guaranteed host {:?}", entry.destination)
                    })?;
                if inserted > 0 {
                    added.push(tx.last_insert_rowid());
                }
            }
            tx.commit().context("committing the ensure-hosts batch")?;
            Ok(added)
        })
        .await
        .context("ensure ssh hosts task panicked")?
    }

    /// Change an ssh host's destination in place, refusing the local row
    /// and a destination collision with a DIFFERENT host.
    ///
    /// The kind/existence lookup and the write run in ONE transaction, which
    /// is what lets `changed == 0` from the `UPDATE OR IGNORE` below mean
    /// exactly one thing: a destination collision. Read in isolation the
    /// `UPDATE` alone could not tell "the row vanished", "it turned out to
    /// be the local row", and "the unique index rejected it" apart — all
    /// three leave `changed == 0` — but the preliminary read already ruled
    /// the first two out, and nothing else can invalidate that finding
    /// before the `UPDATE` runs because the transaction holds the write lock
    /// for the whole span. `OR IGNORE` (rather than a `NOT EXISTS` guard, as
    /// this used before) needs no self-exclusion clause either: it updates
    /// THIS row's own `destination` column directly, so re-affirming a
    /// host's own current destination can never collide with itself the way
    /// a `WHERE ... destination = ?2` sub-select would have to guard
    /// against.
    ///
    /// Shares [`Self::add_ssh_host`]'s
    /// [`HostStoreError::InvalidDestination`] refusal: an edit is exactly
    /// as capable of introducing an unusable destination as an add, and a
    /// registry that validated only one of the two paths would be
    /// validating nothing.
    ///
    /// ## What a retarget leaves alone
    ///
    /// Everything else the helm knows about the row. The learned identity and
    /// the session cache are facts about an install, and the next handshake
    /// against the new endpoint decides what happens to them (a mismatch
    /// freezes, and adoption is what purges). The helm-wide remembered
    /// profile survives too. It is a singleton, not state owned by this
    /// registry row, so changing one host's destination has no profile
    /// preference to migrate or clear.
    pub async fn update_ssh_destination(
        &self,
        host: HostId,
        destination: &str,
    ) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        let destination = destination.to_string();
        if !destination_is_usable(&destination) {
            return Err(anyhow::Error::new(HostStoreError::InvalidDestination(
                destination,
            )));
        }
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning update destination transaction")?;
            let current: Option<String> = tx
                .query_row(
                    "SELECT kind FROM hosts WHERE id = ?1",
                    rusqlite::params![host],
                    |r| r.get(0),
                )
                .optional()
                .context("looking up host before updating its destination")?;
            let current = current
                .map(|kind| HostKind::from_column(&kind))
                .transpose()?;
            match current {
                None => Err(anyhow::Error::new(HostStoreError::HostNotFound(host))),
                Some(HostKind::Local) => {
                    Err(anyhow::Error::new(HostStoreError::LocalHostImmutable))
                }
                Some(HostKind::Ssh) => {
                    // Only ALIASES are compared here — colliding with
                    // another row's plain, unaliased destination is not
                    // this check's job. That case is caught below by
                    // `UPDATE OR IGNORE` against the `hosts_ssh_destination`
                    // partial unique index and reported as
                    // `DuplicateDestination`, the more specific refusal for
                    // two ssh hosts fighting over the same literal
                    // destination text. Widening this scan to every row's
                    // DERIVED display name (as an earlier version of this
                    // check did, by running it through
                    // `host_display_name`) would report every ordinary
                    // destination collision as an alias conflict even where
                    // no alias is involved on either side, and would shadow
                    // `DuplicateDestination` entirely since this check runs
                    // first.
                    if let Some(name) = alias_collision(&tx, Some(host), &destination)? {
                        return Err(anyhow::Error::new(HostStoreError::AliasTaken(name)));
                    }
                    let changed = tx
                        .execute(
                            "UPDATE OR IGNORE hosts SET destination = ?2 WHERE id = ?1",
                            rusqlite::params![host, destination],
                        )
                        .context("updating ssh destination")?;
                    if changed == 0 {
                        return Err(anyhow::Error::new(HostStoreError::DuplicateDestination(
                            destination,
                        )));
                    }
                    tx.commit().context("committing destination update")?;
                    Ok(())
                }
            }
        })
        .await
        .context("update ssh destination task panicked")?
    }

    /// Replace one host's optional alias and report whether its canonical
    /// stored value changed. The uniqueness scan shares the write transaction
    /// so an alias can never race another host into the display-name space.
    ///
    /// SETTING an alias is checked against every OTHER host's full current
    /// display name (alias or derived) — the wide check, since a stored
    /// alias is arbitrary text with no dedicated uniqueness index. CLEARING
    /// one is checked the other way: this row's RESTORED derived name
    /// (kind and destination, ignoring the alias about to be dropped)
    /// against only other hosts' current aliases — the narrow check
    /// `alias_collision` shares with registration and retargeting — because
    /// a restored derived name can only collide with something that was
    /// never a plain destination collision in the first place. Skipping
    /// this on clear would let a host silently reclaim its raw destination
    /// as a display name even while another host is already showing under
    /// an alias identical to it.
    pub async fn update_alias(&self, host: HostId, alias: Option<&str>) -> anyhow::Result<bool> {
        let alias = validate_alias(alias).map_err(anyhow::Error::new)?;
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning update alias transaction")?;
            let current: Option<(String, Option<String>, Option<String>)> = tx
                .query_row(
                    "SELECT kind, destination, alias FROM hosts WHERE id = ?1",
                    rusqlite::params![host],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .context("looking up host before updating its alias")?;
            let Some((kind, destination, current_alias)) = current else {
                return Err(anyhow::Error::new(HostStoreError::HostNotFound(host)));
            };
            let kind = HostKind::from_column(&kind)?;
            if current_alias == alias {
                tx.commit().context("committing unchanged alias")?;
                return Ok(false);
            }
            match alias.as_deref() {
                Some(candidate) => {
                    let mut other = tx
                        .prepare("SELECT kind, destination, alias FROM hosts WHERE id != ?1")
                        .context("reading display names before updating an alias")?;
                    let rows = other
                        .query_map(rusqlite::params![host], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, Option<String>>(1)?,
                                row.get::<_, Option<String>>(2)?,
                            ))
                        })
                        .context("querying other host display names")?
                        .collect::<Result<Vec<_>, _>>()?;
                    drop(other);
                    // A row whose `kind` this build cannot decode is a
                    // reason to REFUSE the write, not to silently drop that
                    // row from the comparison: `list_hosts` fails the whole
                    // registry read on the identical corruption, and an
                    // alias committed against a registry this function
                    // could not fully interpret is exactly the kind of
                    // state the later manager sync would then fail to
                    // reconcile against.
                    let names = rows
                        .into_iter()
                        .map(|(kind, destination, alias)| {
                            HostKind::from_column(&kind).map(|kind| {
                                host_display_name(kind, destination.as_deref(), alias.as_deref())
                            })
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    if let Some(name) = names.into_iter().find(|name| name == candidate) {
                        return Err(anyhow::Error::new(HostStoreError::AliasTaken(name)));
                    }
                }
                None => {
                    // Clearing restores this row's DERIVED name — compute
                    // it exactly as `host_display_name` would once the
                    // alias is gone, and run it through the same narrow
                    // check `alias_collision` gives registration and
                    // retargeting.
                    let restored = host_display_name(kind, destination.as_deref(), None);
                    if let Some(name) = alias_collision(&tx, Some(host), &restored)? {
                        return Err(anyhow::Error::new(HostStoreError::AliasTaken(name)));
                    }
                }
            }
            tx.execute(
                "UPDATE hosts SET alias = ?2 WHERE id = ?1",
                rusqlite::params![host, alias],
            )
            .context("updating host alias")?;
            tx.commit().context("committing alias update")?;
            Ok(true)
        })
        .await
        .context("update alias task panicked")?
    }

    /// Forget a registered ssh host — SPEC.md's remove-merely-forgets
    /// contract: the row and, via `ON DELETE CASCADE`, its `session_cache`
    /// entries are gone, but nothing about the actual remote install is
    /// touched, and re-adding the same destination later starts a fresh
    /// [`HostId`] with an empty cache (see [`HostId`]'s own docs on why ids
    /// are never recycled).
    ///
    /// Refuses [`HostStoreError::LocalHostImmutable`] for the reserved
    /// row — the local host is not user management surface at all
    /// (PLAN_M6.md item 4) — and [`HostStoreError::HostNotFound`] for an
    /// id nothing currently holds.
    pub async fn remove_ssh_host(&self, host: HostId) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            let kind: Option<String> = conn
                .query_row(
                    "SELECT kind FROM hosts WHERE id = ?1",
                    rusqlite::params![host],
                    |r| r.get(0),
                )
                .optional()
                .context("looking up host before removing it")?;
            let kind = kind.map(|k| HostKind::from_column(&k)).transpose()?;
            match kind {
                None => Err(anyhow::Error::new(HostStoreError::HostNotFound(host))),
                Some(HostKind::Local) => {
                    Err(anyhow::Error::new(HostStoreError::LocalHostImmutable))
                }
                Some(HostKind::Ssh) => {
                    conn.execute("DELETE FROM hosts WHERE id = ?1", rusqlite::params![host])
                        .context("removing ssh host")?;
                    Ok(())
                }
            }
        })
        .await
        .context("remove ssh host task panicked")?
    }

    /// Record `identity` as learned at this host's first-ever successful
    /// hello — writes ONLY when the stored `host_identity` is still `NULL`.
    /// Split out from what a DIFFERENT-identity hello means
    /// ([`Self::adopt_identity`]) so silent identity merging is
    /// structurally impossible at this layer: this method can never
    /// overwrite an existing, different identity, no matter what a caller
    /// passes — there is no code path here that does so, rather than a
    /// runtime check a future edit could weaken.
    ///
    /// Five outcomes, none of them ambiguous:
    /// - stored is `NULL` and no other row claims `identity` → written,
    ///   [`FirstContactOutcome::Recorded`].
    /// - stored equals `identity` → nothing to write, still
    ///   [`FirstContactOutcome::Recorded`] (an idempotent repeat hello).
    /// - stored is a DIFFERENT identity → nothing written,
    ///   [`FirstContactOutcome::Mismatch`] carrying both values so the
    ///   caller can surface SPEC.md's adopt-or-fix-destination choice.
    /// - ANOTHER row already claims `identity` → nothing written,
    ///   [`FirstContactOutcome::Collision`] naming that row.
    /// - the row was reconfigured since `dialed` was captured → nothing
    ///   written, [`FirstContactOutcome::StaleAttempt`].
    ///
    /// **Everything happens in ONE `BEGIN IMMEDIATE` transaction**, which
    /// is what makes the collision answer trustworthy rather than advisory.
    /// Resolving the claim in a separate call first — the shape this
    /// replaced — leaves a window in which two actors both see no claimant
    /// and both write; the `hosts_identity_claim` index would then reject
    /// one of them with a raw constraint error at best, and at worst (two
    /// processes, two connections) leave the losing entry believing it had
    /// recorded. Holding the write lock from the first read means the
    /// claim this call reports is the claim it would have written against.
    ///
    /// `dialed` is the row's connection-defining configuration as the
    /// caller saw it before dialing (see [`DialedAs`]): the identity in an
    /// attempt's hand belongs to whatever endpoint it actually reached, so
    /// committing it under a row whose destination has since been edited
    /// would attribute one machine's identity to another.
    ///
    /// [`HostStoreError::HostNotFound`] for an id nothing currently holds.
    pub async fn record_first_contact(
        &self,
        host: HostId,
        dialed: &DialedAs,
        identity: &str,
    ) -> anyhow::Result<FirstContactOutcome> {
        let conn = Arc::clone(&self.conn);
        let identity = identity.to_string();
        let dialed = dialed.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<FirstContactOutcome> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .context("beginning first-contact transaction")?;
            let Some((current, configured)) = read_identity_and_config(&tx, host)? else {
                return Err(anyhow::Error::new(HostStoreError::HostNotFound(host)));
            };
            let outcome = if configured != dialed {
                FirstContactOutcome::StaleAttempt {
                    current: configured,
                }
            } else {
                match current {
                    Some(recorded) if recorded == identity => FirstContactOutcome::Recorded,
                    Some(recorded) => FirstContactOutcome::Mismatch {
                        recorded,
                        reported: identity,
                    },
                    None => match claimant_of(&tx, host, &identity)? {
                        Some(owner) => FirstContactOutcome::Collision { owner },
                        None => {
                            tx.execute(
                                "UPDATE hosts SET host_identity = ?2 WHERE id = ?1",
                                rusqlite::params![host, identity],
                            )
                            .context("recording first-contact identity")?;
                            FirstContactOutcome::Recorded
                        }
                    },
                }
            };
            // Committed on every path, including the ones that wrote
            // nothing: an explicit commit releases the write lock this
            // transaction took at BEGIN IMMEDIATE, rather than leaving it
            // to a rollback on drop.
            tx.commit().context("committing first contact")?;
            Ok(outcome)
        })
        .await
        .context("record first contact task panicked")?
    }

    /// Compare-and-swap a host's identity: succeeds ONLY when the currently
    /// stored value equals `expected_old`, replacing it with `new` and
    /// erasing what the OLD install left behind — that host's
    /// `session_cache` rows — in the SAME transaction. PLAN_M6.md item 4's
    /// user-initiated adoption of an
    /// identity-mismatched host (SPEC.md: the helm never silently merges;
    /// this is the explicit acknowledgment that performs the merge the user
    /// chose after seeing [`FirstContactOutcome::Mismatch`]).
    ///
    /// The purge is IN the transaction, never a follow-up call: a different
    /// identity at a known destination means the install that produced the
    /// OLD identity is gone, so its cached sessions describe agents that no
    /// longer exist behind this [`HostId`] — carrying them forward under the
    /// NEW identity would misattribute a dead install's history to a live
    /// one. A separate follow-up call could leave the two writes torn by a
    /// crash or a concurrent reader between them; one transaction cannot.
    ///
    /// The remembered default profile is NOT purged. Adoption replaces one
    /// host's installation identity, while the preference is a helm-wide
    /// singleton with no ownership relationship to that host or its cache.
    ///
    /// A STALE `expected_old` (the stored value has already moved on — a
    /// second adoption, or a first contact that landed first) is refused as
    /// [`HostStoreError::IdentityMismatch`] with NOTHING changed, same as
    /// [`HostStoreError::HostNotFound`] for an id nothing currently holds.
    ///
    /// Adoption inherits first contact's two atomic guards, in the same
    /// transaction and for the same reasons:
    /// - a rival row that claimed `new` between the mismatch being shown
    ///   and this call arriving is refused as
    ///   [`HostStoreError::IdentityClaimed`] — the user is then looking at
    ///   a duplicate to resolve, not an adoption to confirm, and the
    ///   compare-and-swap alone would not have noticed;
    /// - a row reconfigured since `dialed` was captured is refused as
    ///   [`HostStoreError::StaleAttempt`], because the identity being
    ///   adopted was reported by an endpoint this row no longer names.
    pub async fn adopt_identity(
        &self,
        host: HostId,
        dialed: &DialedAs,
        expected_old: &str,
        new: &str,
    ) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        let expected_old = expected_old.to_string();
        let new = new.to_string();
        let dialed = dialed.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .context("beginning identity adoption transaction")?;
            let Some((current, configured)) = read_identity_and_config(&tx, host)? else {
                return Err(anyhow::Error::new(HostStoreError::HostNotFound(host)));
            };
            if configured != dialed {
                return Err(anyhow::Error::new(HostStoreError::StaleAttempt { host }));
            }
            if current.as_deref() != Some(expected_old.as_str()) {
                return Err(anyhow::Error::new(HostStoreError::IdentityMismatch {
                    host,
                    expected: expected_old,
                    actual: current,
                }));
            }
            if let Some(owner) = claimant_of(&tx, host, &new)? {
                return Err(anyhow::Error::new(HostStoreError::IdentityClaimed {
                    host,
                    identity: new,
                    owner,
                }));
            }
            // `cache_truncated` is reset with the cache it describes: the
            // flag was the PREDECESSOR install's word about the rows being
            // purged below, and an empty successor cache marked incomplete
            // would show the notice indefinitely if the first refresh under
            // the new identity failed.
            tx.execute(
                "UPDATE hosts SET host_identity = ?2, cache_truncated = 0 WHERE id = ?1",
                rusqlite::params![host, new],
            )
            .context("adopting new host identity")?;
            tx.execute(
                "DELETE FROM session_cache WHERE host_id = ?1",
                rusqlite::params![host],
            )
            .context("purging the superseded identity's cached sessions")?;
            tx.commit().context("committing identity adoption")?;
            Ok(())
        })
        .await
        .context("adopt identity task panicked")?
    }

    /// Replace one host's ENTIRE cached session list, atomically — the
    /// wholesale-replacement contract PLAN_M6.md item 3 settles: every
    /// successful list refresh from a live host calls this with the
    /// refresh's full result, never a partial update, so a session absent
    /// from `entries` is understood to no longer exist on that host and is
    /// dropped from the cache along with it (a session appearing in a later
    /// refresh under the same id simply overwrites its older cached self,
    /// via the delete-then-insert below rather than any per-row update
    /// logic).
    ///
    /// Delete-then-insert in ONE transaction, never a partial mix: a caller
    /// observing a failure here must be able to assume the OLD cache is
    /// still intact rather than half-cleared, because the alternative — a
    /// host's cache silently emptied by a refresh that itself failed
    /// partway — would make a transient serialization error look
    /// indistinguishable from "this host truly has zero sessions now."
    /// Pinned by `replace_host_sessions_rolls_back_a_mid_batch_failure`
    /// below, which forces the failure with a duplicate id WITHIN one
    /// `entries` batch — a real primary-key collision the schema already
    /// rejects, not an injected fault — and asserts the pre-existing rows
    /// survive untouched. (The plan's alternative of a poisoned row that
    /// fails `serde_json` serialization was not used: `SessionInfo`
    /// serializes infallibly for any value this module can construct, so
    /// there is no such row to plant honestly; a genuine schema-level
    /// constraint violation is the cheaper, and more realistic, seam.)
    ///
    /// `created_at` and `archived` are extracted from each `entry` into
    /// columns of their own beside the payload: `created_at` is the identity
    /// cross-check every read applies to a decoded row, and `archived` lets
    /// the default view skip a row before decoding it. Nothing orders by
    /// either — the merge sorts in memory.
    ///
    /// `truncated` is written to the host's own row in the same transaction
    /// (`hosts.cache_truncated`): whether this list was cut at the wire's
    /// cap is a fact about the list, and it has to be kept with the list so
    /// a cut cache served stale still says so (see [`HostRow::cache_truncated`]).
    ///
    /// Two checks run inside the SAME transaction as the delete-then-insert,
    /// against exactly the state a live refresh's caller cannot otherwise
    /// observe atomically:
    /// - **Host existence.** A refresh naming an id that has since been
    ///   removed is refused as [`HostStoreError::HostNotFound`] rather than
    ///   quietly deleting zero rows and inserting zero more — an EMPTY
    ///   `entries` batch against a gone host used to succeed silently
    ///   (nothing to delete, nothing to insert, no error), which reads
    ///   identically to "this host truly has zero sessions" from the
    ///   caller's side even though the host does not exist at all.
    /// - **Identity binding.** `identity` must equal what is CURRENTLY
    ///   stored for this host. A session list is only ever meaningful for
    ///   the identity it was fetched from; without this check a refresh
    ///   already in flight when a user adopts a new identity
    ///   ([`Self::adopt_identity`]) could land AFTER the adoption's purge
    ///   and repopulate the cache with the dead install's sessions under
    ///   the new identity's row — exactly the silent-merge outcome
    ///   adoption exists to prevent, just arriving by a side door. A stale
    ///   `identity` is refused as [`HostStoreError::IdentityMismatch`] with
    ///   the existing cache left untouched.
    ///
    /// ## Reports whether it CHANGED anything
    ///
    /// [`CacheReplacement::changed`] is the invalidation feed's changed-only
    /// rule at its source (PLAN_M6_75.md item 5), and it is answered here
    /// rather than by a caller comparing lists afterwards because only this
    /// transaction can see both sides atomically. A refresh runs every few
    /// seconds per host and, in a settled fleet, writes back exactly what
    /// was already there; a feed that treated every committed replacement as
    /// a change would wake every open client on that timer and be strictly
    /// worse than the polling it replaces. The comparison is over this
    /// host's stored payloads before and after, so a row that was DROPPED as
    /// contested is correctly not a change (it was not written either time).
    pub async fn replace_host_sessions(
        &self,
        host: HostId,
        identity: &str,
        entries: Vec<SessionInfo>,
        truncated: bool,
    ) -> anyhow::Result<CacheReplacement> {
        let conn = Arc::clone(&self.conn);
        let identity = identity.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<CacheReplacement> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning cache replace transaction")?;
            let current: Option<Option<String>> = tx
                .query_row(
                    "SELECT host_identity FROM hosts WHERE id = ?1",
                    rusqlite::params![host],
                    |r| r.get(0),
                )
                .optional()
                .context("reading current host identity")?;
            let Some(current) = current else {
                return Err(anyhow::Error::new(HostStoreError::HostNotFound(host)));
            };
            if current.as_deref() != Some(identity.as_str()) {
                return Err(anyhow::Error::new(HostStoreError::IdentityMismatch {
                    host,
                    expected: identity,
                    actual: current,
                }));
            }
            // Read BEFORE the delete, inside the same transaction as the
            // rewrite: this is the "what did we have" half of the
            // changed-only answer, and any read outside the transaction
            // could be describing a different moment than the write.
            //
            // BOTH stored halves, not just the payload: `created_at` is a
            // column of its own (the identity cross-check every read
            // applies), so a row whose payload is unchanged while its
            // timestamp is repaired IS a change — and reporting otherwise
            // would starve the feed of exactly the re-read a client needs.
            // The `archived` column needs no further comparison: it is
            // extracted from the payload on the way in, so it cannot move
            // without `info_json` moving with it. A session that produced
            // output, or was renamed, therefore already flips `changed` —
            // which is what makes the activity and title orders live
            // surfaces rather than ones that settle at the next refresh.
            //
            // ONE map, consumed as the rewrite goes: entries are removed as
            // they are matched, so this holds the host's cache once rather
            // than twice at the peak. What remains at the end is what
            // DISAPPEARED, which no per-row comparison of the new list could
            // notice on its own. The transient cost is one host's slice at
            // the listing cap (`farhelm_proto::LIST_SESSIONS_CAP` rows),
            // which is the same data the caller already holds in `entries`.
            let mut previous: std::collections::HashMap<String, (i64, String)> = {
                let mut stmt = tx
                    .prepare(
                        "SELECT session_id, created_at, info_json FROM session_cache \
                         WHERE host_id = ?1",
                    )
                    .context("preparing the pre-replace cache read")?;
                let rows = stmt
                    .query_map(rusqlite::params![host], |r| {
                        Ok((r.get(0)?, (r.get(1)?, r.get(2)?)))
                    })
                    .context("reading the cache this refresh is replacing")?;
                rows.collect::<Result<_, _>>()
                    .context("collecting the cache this refresh is replacing")?
            };
            tx.execute(
                "DELETE FROM session_cache WHERE host_id = ?1",
                rusqlite::params![host],
            )
            .context("clearing the stale cache")?;
            let mut contested: Vec<String> = Vec::new();
            let mut changed = false;
            let mut newest_profile_source: Option<(Option<u64>, i64, String, String)> = None;
            let mut present_session_ids = std::collections::HashSet::new();
            for entry in &entries {
                let json = serde_json::to_string(entry).context("serializing cached session")?;
                let inserted = tx
                    .execute(
                        // Conditional on the one-owner index rather than
                        // fallible against it: a supervisor reporting a
                        // session id another host already caches must cost
                        // that ONE row, not the whole refresh — the rest of
                        // this host's list is perfectly good data, and
                        // failing the refresh would also mean keeping the
                        // previous cache forever for as long as the
                        // collision persisted. First claim holds; the
                        // skipped row is announced below rather than
                        // swallowed.
                        "INSERT INTO session_cache \
                             (host_id, session_id, created_at, info_json, archived) \
                         VALUES (?1, ?2, ?3, ?4, ?5) \
                         ON CONFLICT (session_id) DO NOTHING",
                        rusqlite::params![host, entry.id, entry.created_at, json, entry.archived],
                    )
                    .context("inserting cached session")?;
                if inserted == 0 {
                    let owner: Option<HostId> = tx
                        .query_row(
                            "SELECT host_id FROM session_cache WHERE session_id = ?1",
                            rusqlite::params![entry.id],
                            |r| r.get(0),
                        )
                        .optional()
                        .context("reading the host that already claims this session id")?;
                    // Two very different situations reach one conflict, and
                    // they get opposite treatment. This host's own rows were
                    // deleted at the top of this transaction, so a conflict
                    // against THIS host can only mean the peer listed one
                    // session id twice in a single reply — a malformed list,
                    // which fails the whole refresh and keeps the previous
                    // cache, exactly as this method has always treated a
                    // list it could not store whole. A conflict against
                    // ANOTHER host is the cross-host collision: one row is
                    // dropped, the first claim holds, and the rest of this
                    // host's perfectly good list is still cached.
                    if owner == Some(host) {
                        anyhow::bail!(
                            "the host listed session id {:?} more than once in a single reply; \
                             refusing to cache a list that contradicts itself",
                            entry.id
                        );
                    }
                    // Reported rather than logged here. One line per
                    // colliding row per refresh tick is a log a hostile (or
                    // merely confused) host writes for you; the caller
                    // coalesces these into one bounded summary per refresh,
                    // and holds the set as the host's live contested state.
                    contested.push(entry.id.clone());
                    continue;
                }
                present_session_ids.insert(entry.id.clone());
                if let Some(source) = &entry.source_profile {
                    let candidate = (
                        entry.creation_seq,
                        entry.created_at,
                        entry.id.clone(),
                        source.id.clone(),
                    );
                    let replaces = newest_profile_source.as_ref().is_none_or(
                        |(creation_seq, created_at, session_id, _)| {
                            source_is_newer(
                                ProfileSource { host: Some(host), sequence: candidate.0, created_at: candidate.1, session_id: &candidate.2 },
                                ProfileSource { host: Some(host), sequence: *creation_seq, created_at: *created_at, session_id },
                            )
                        },
                    );
                    if replaces {
                        newest_profile_source = Some(candidate);
                    }
                }
                // A row that was already stored EXACTLY as it is being
                // written back is not a change; anything else — a new id, a
                // repaired timestamp, a different payload — is.
                match previous.remove(&entry.id) {
                    Some((created_at, stored))
                        if created_at == entry.created_at && stored == json => {}
                    _ => changed = true,
                }
            }
            // Whatever is left in `previous` was cached a moment ago and is
            // not any more: a session that ended, was deleted, or moved
            // hosts. Folded in before the commit so the generation below and
            // the reported answer are the same judgement.
            let changed = changed || !previous.is_empty();
            // The cap flag is part of what this list IS, so a flip in it is
            // a change clients must re-read for: the notice it drives is on
            // screen. Compared against the stored value rather than written
            // blindly, for the same changed-only reason the rows are.
            let truncated_changed = tx
                .execute(
                    "UPDATE hosts SET cache_truncated = ?2                      WHERE id = ?1 AND cache_truncated != ?2",
                    rusqlite::params![host, truncated],
                )
                .context("recording whether the cached list was cut")?
                != 0;
            let changed = changed || truncated_changed;
            // A drain is the authoritative observation that a profile was
            // actually used. Carry its session ordering key beside the
            // preference so a delayed, older drain cannot roll the default
            // backward after a newer create or refresh has already landed.
            let mut default_changed = false;
            let remembered: Option<RememberedProfileRow> = tx
                .query_row(
                    "SELECT profile_id, source_host_id, source_creation_seq, source_created_at, \
                            source_session_id \
                     FROM remembered_profile WHERE singleton = 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()
                .context("reading remembered profile provenance")?;
            // Absence is only evidence in an UNTRUNCATED reply: a capped
            // list omits every session past the cut, so a remembered source
            // missing from it may simply be old rather than gone. Under
            // truncation the absence is unknown — the stored provenance is
            // neither cleared nor treated as vacated, and a replacement
            // must prove itself newer through the ordinary provenance
            // comparison below.
            let source_disappeared = !truncated
                && remembered
                    .as_ref()
                    .filter(|(_, source_host, _, _, _)| *source_host == Some(host))
                    .and_then(|(_, _, _, _, session_id)| session_id.as_deref())
                    .is_some_and(|session_id| !present_session_ids.contains(session_id));
            // A remembered source that is no longer among this host's
            // sessions is the retarget/adopt/reinstall shape (or a plain
            // deletion of the establishing session). Under the bare-id
            // contract the PREFERENCE survives it — deleting the row here
            // would quietly rebuild the install eviction schema v12 removed
            // — but the provenance does not: it was minted in an ordering
            // domain this drain can no longer see, and keeping it would let
            // a predecessor's high sequence numbers veto the successor's
            // genuinely newer creates. So the id is kept and the source_*
            // columns are cleared, which drops the row into the same
            // "opaque until a direct create re-establishes provenance"
            // state a v7-era migrated preference starts in.
            if source_disappeared {
                tx.execute(
                    "UPDATE remembered_profile SET source_host_id = NULL, source_creation_seq = NULL, \
                     source_created_at = NULL, source_session_id = NULL WHERE singleton = 1",
                    [],
                )
                .context("orphaning a remembered profile whose source disappeared")?;
            }
            if let Some((creation_seq, created_at, session_id, profile_id)) = newest_profile_source
            {
                // Only a demonstrably NEWER source advances the default,
                // judged against the provenance as it stood BEFORE any
                // orphaning above — a disappeared source is no longer a
                // license to promote whatever survived (the old rule), it
                // only stops mattering as a comparison point once cleared.
                // A survivor that fails the comparison leaves the bare id
                // in place.
                let advances = match &remembered {
                    None => true,
                    Some((_, stored_host, stored_seq, Some(stored_at), Some(stored_id))) => source_is_newer(
                        ProfileSource { host: Some(host), sequence: creation_seq, created_at, session_id: &session_id },
                        ProfileSource { host: *stored_host, sequence: stored_seq.and_then(|seq| u64::try_from(seq).ok()), created_at: *stored_at, session_id: stored_id },
                    ),
                    // A v7 -> v8 migrated preference has no source at
                    // all. The first post-upgrade drain cannot prove its
                    // newest SURVIVING session is newer than the session
                    // the user actually chose before upgrading: that
                    // source may already have been deleted. Keep the
                    // opaque preference until a direct create records a
                    // real source, after which ordinary drain ordering
                    // applies again.
                    Some((_, _, None, None, None)) => false,
                    Some(_) => true,
                };
                if advances {
                    default_changed = remembered
                        .as_ref()
                        .is_none_or(|(stored_profile, _, _, _, _)| stored_profile != &profile_id);
                    tx.execute(
                        "INSERT INTO remembered_profile (\
                             singleton, profile_id, source_host_id, source_creation_seq, \
                             source_created_at, source_session_id\
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                         ON CONFLICT (singleton) DO UPDATE SET \
                             profile_id = excluded.profile_id, \
                             source_host_id = excluded.source_host_id, \
                             source_creation_seq = excluded.source_creation_seq, \
                             source_created_at = excluded.source_created_at, \
                             source_session_id = excluded.source_session_id",
                        rusqlite::params![
                            1,
                            profile_id,
                            host,
                            creation_seq
                                .map(i64::try_from)
                                .transpose()
                                .context("creation sequence exceeds SQLite's integer range")?,
                            created_at,
                            session_id
                        ],
                    )
                    .context("advancing the remembered profile from the completed drain")?;
                }
            }
            tx.commit().context("committing cache replace")?;
            // SORTED, so the set is compared by CONTENT rather than by the
            // order a peer happened to list its sessions in. The published
            // set is compared against the previous one to decide whether to
            // invalidate every client (`crate::manager::FleetEvents`), and a
            // peer that merely permuted its reply would otherwise look like a
            // routing change and wake the fleet on every refresh tick.
            contested.sort_unstable();
            Ok(CacheReplacement {
                changed,
                contested,
                default_changed,
            })
        })
        .await
        .context("replace host sessions task panicked")?
    }

    /// Add ONE session to a host's cache slice, leaving the rest of it
    /// alone — the seed a fresh create needs (PLAN_M6.md item 5).
    ///
    /// Exists because [`Self::replace_host_sessions`] is the wrong shape for
    /// this and cannot be bent into it: it is a wholesale replacement, so
    /// using it to add a row would mean reading the host's whole cache,
    /// appending, and writing it back — a read-modify-write racing the
    /// actor's own refresh, which is the one writer this table was designed
    /// to have. This inserts exactly the row that is new.
    ///
    /// Carries the same identity binding as the wholesale write, for the
    /// same reason: a create whose reply landed after a user adopted a new
    /// identity describes the dead install, and seeding it under the new
    /// identity's row would repopulate by a side door the very cache the
    /// adoption purged. A stale `identity` is [`HostStoreError::IdentityMismatch`]
    /// with nothing written.
    ///
    /// Idempotent for the SAME host (the row is replaced), and refused for a
    /// DIFFERENT one: a session id another host already claims is
    /// [`HostStoreError::SessionOwnerAmbiguous`], because a create cannot be
    /// the thing that resolves a collision — see that variant's docs.
    ///
    /// The id is bounded like every other peer-supplied one
    /// (`crate::manager::MAX_SESSION_ID_BYTES`): a create's reply is a peer
    /// ingress point exactly as a drain's rows are, and an id no later
    /// request could carry in its frame head must not enter the cache
    /// through either.
    ///
    /// The cap holds here by EVICTION: a seed that would leave the slice
    /// past `farhelm_proto::LIST_SESSIONS_CAP` rows drops the oldest OTHER
    /// row and sets the host's `cache_truncated` flag, because the new
    /// session must be routable (the create already succeeded) while the
    /// cache stays bounded — see the eviction comment in the body.
    ///
    /// Returns whether the stored row actually CHANGED — the same
    /// changed-only rule [`Self::replace_host_sessions`] answers, applied to
    /// one row. A retried create that re-records a byte-identical session is
    /// a successful write that invalidates nothing. An eviction counts as
    /// changed: a row disappeared from what clients can read.
    pub async fn remember_session(
        &self,
        host: HostId,
        identity: &str,
        entry: &SessionInfo,
    ) -> anyhow::Result<bool> {
        anyhow::ensure!(
            entry.id.len() <= crate::manager::MAX_SESSION_ID_BYTES,
            "session id of {} bytes exceeds the {} this helm can build resumable cursors over",
            entry.id.len(),
            crate::manager::MAX_SESSION_ID_BYTES
        );
        let conn = Arc::clone(&self.conn);
        let identity = identity.to_string();
        let entry = entry.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning cache seed transaction")?;
            let current: Option<Option<String>> = tx
                .query_row(
                    "SELECT host_identity FROM hosts WHERE id = ?1",
                    rusqlite::params![host],
                    |r| r.get(0),
                )
                .optional()
                .context("reading current host identity")?;
            let Some(current) = current else {
                return Err(anyhow::Error::new(HostStoreError::HostNotFound(host)));
            };
            if current.as_deref() != Some(identity.as_str()) {
                return Err(anyhow::Error::new(HostStoreError::IdentityMismatch {
                    host,
                    expected: identity,
                    actual: current,
                }));
            }
            // `created_at` comes back too, for the changed-only comparison
            // at the bottom: it is a column of its own and part of the
            // ordering key, so a row whose payload matches while its
            // timestamp does not is a change like any other.
            let claimed: Option<(HostId, i64, String)> = tx
                .query_row(
                    "SELECT host_id, created_at, info_json FROM session_cache \
                     WHERE session_id = ?1",
                    rusqlite::params![entry.id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()
                .context("reading the current owner of this session id")?;
            if let Some((owner, _, _)) = &claimed
                && *owner != host
            {
                return Err(anyhow::Error::new(HostStoreError::SessionOwnerAmbiguous {
                    session: entry.id.clone(),
                    first: *owner,
                    second: host,
                }));
            }
            // Read and merged INSIDE the transaction that overwrites it, so
            // no refresh can land between "what did we know" and "what do
            // we now record". A mutation's reply is authoritative about
            // everything except liveness and the activity stamp — see
            // `crate::manager::merge_cached_session` for why `Unknown`
            // must not erase a definite status, why the previous value is
            // kept even at the cost of being briefly stale, and why a
            // reply may only push `last_activity_at` forward. The same
            // helper serves the in-memory cache, so the two shapes cannot
            // drift apart.
            let mut entry = entry;
            if let Some((_, _, previous)) = &claimed
                && let Ok(previous) = serde_json::from_str::<SessionInfo>(previous)
            {
                crate::manager::merge_cached_session(&previous, &mut entry);
            }
            let entry = &entry;
            let json = serde_json::to_string(entry).context("serializing cached session")?;
            tx.execute(
                "INSERT INTO session_cache \
                     (host_id, session_id, created_at, info_json, archived) \
                 VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT (session_id) DO UPDATE SET \
                     created_at = excluded.created_at, info_json = excluded.info_json, \
                     archived = excluded.archived",
                rusqlite::params![host, entry.id, entry.created_at, json, entry.archived],
            )
            .context("seeding a cached session")?;
            // The cap holds for the seed path too — but by EVICTION, never
            // refusal: the create already succeeded on the supervisor, and
            // a cache without its row would leave a session the caller was
            // just told exists unroutable until the next refresh. The
            // oldest OTHER row goes (largest under the creation order's
            // sort: smallest created_at, tie-broken by the later id), and
            // the host's flag records that the cache no longer holds
            // everything known — which is exactly what the notice means.
            let over_cap = {
                let count: i64 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM session_cache WHERE host_id = ?1",
                        rusqlite::params![host],
                        |r| r.get(0),
                    )
                    .context("counting the seeded cache slice")?;
                count as usize > farhelm_proto::LIST_SESSIONS_CAP
            };
            if over_cap {
                tx.execute(
                    "DELETE FROM session_cache WHERE host_id = ?1 AND session_id = (                         SELECT session_id FROM session_cache                          WHERE host_id = ?1 AND session_id != ?2                          ORDER BY created_at ASC, session_id DESC LIMIT 1)",
                    rusqlite::params![host, entry.id],
                )
                .context("evicting the oldest cached session past the cap")?;
                tx.execute(
                    "UPDATE hosts SET cache_truncated = 1 WHERE id = ?1",
                    rusqlite::params![host],
                )
                .context("recording the seed eviction as a cut")?;
            }
            tx.commit().context("committing cache seed")?;
            // Compared against BOTH stored halves this host already held,
            // AFTER the status merge above: the merge is what makes a
            // restart's `Unknown`-carrying reply a no-op for an unchanged
            // session, and comparing before it would report a change the row
            // does not actually show. The timestamp is in the comparison for
            // the same reason it is in the wholesale write's — it is the
            // ordering column, not a copy of something in the payload.
            let changed = over_cap
                || match &claimed {
                    Some((_, created_at, stored)) => {
                        *created_at != entry.created_at || stored.as_str() != json.as_str()
                    }
                None => true,
            };
            Ok(changed)
        })
        .await
        .context("remember session task panicked")?
    }

    /// Drop ONE session from a host's cache slice — the delete's counterpart
    /// to [`Self::remember_session`].
    ///
    /// A delete's reply carries no `SessionInfo`, but it carries the fact
    /// that the session is gone, and the cache is what the merged list is
    /// served from: leaving the row behind means the list keeps showing a
    /// session that no longer exists until the owning host's next refresh.
    /// That is not merely untidy — a client that deletes and immediately
    /// re-creates sees BOTH, which is indistinguishable from a duplicate.
    ///
    /// Identity-bound like every other write here, for the same reason: a
    /// delete whose reply landed after an adoption must not reach into the
    /// new install's cache. Removing a row that is not there is success —
    /// the caller asked for it to be gone, and it is.
    ///
    /// Returns whether a row was actually removed, which is what the
    /// invalidation feed's changed-only rule needs: "it was already gone" is
    /// a success that changes nothing observable.
    pub async fn forget_session(
        &self,
        host: HostId,
        identity: &str,
        session_id: &str,
    ) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        let identity = identity.to_string();
        let session_id = session_id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning cache forget transaction")?;
            let current: Option<Option<String>> = tx
                .query_row(
                    "SELECT host_identity FROM hosts WHERE id = ?1",
                    rusqlite::params![host],
                    |r| r.get(0),
                )
                .optional()
                .context("reading current host identity")?;
            let Some(current) = current else {
                return Err(anyhow::Error::new(HostStoreError::HostNotFound(host)));
            };
            if current.as_deref() != Some(identity.as_str()) {
                return Err(anyhow::Error::new(HostStoreError::IdentityMismatch {
                    host,
                    expected: identity,
                    actual: current,
                }));
            }
            let removed = tx
                .execute(
                    "DELETE FROM session_cache WHERE host_id = ?1 AND session_id = ?2",
                    rusqlite::params![host, session_id],
                )
                .context("forgetting a cached session")?;
            tx.commit().context("committing cache forget")?;
            // Removing a row that was not there is still success (see
            // above), but it is not a CHANGE: nothing any client could read
            // says anything different than it did before.
            Ok(removed > 0)
        })
        .await
        .context("forget session task panicked")?
    }

    /// One host's cached sessions, in creation order (`created_at`
    /// descending, `id` ascending) — a deterministic order for THIS
    /// helper's callers, not the order any client sees: the REST list is
    /// served through [`Self::cached_rows`] and sorted per request into
    /// whichever of the three orders was asked for, stale hosts included.
    ///
    /// Sorted in Rust after decoding, by the `created_at` COLUMN rather
    /// than the payload's copy — the column is the identity the row was
    /// filed under. (Unlike [`Self::cached_rows`] this read does not
    /// cross-check the decoded payload against the column; it feeds
    /// diagnostics and tests, not the served list.) There is no index
    /// to order by (schema version 13 dropped every ordering index, per
    /// SPEC.md's Session list section), and none is needed — a host's
    /// slice is at most `farhelm_proto::LIST_SESSIONS_CAP` rows or a
    /// seed's worth more (see [`Self::remember_session`]).
    ///
    /// SKIP-AND-LOG, not fail-the-whole-read: a row whose `info_json` no
    /// longer decodes (see `session_cache`'s schema comment in
    /// [`apply_schema`] for how this can legitimately happen across a helm
    /// upgrade) is dropped from the returned list and reported via
    /// `tracing::warn!` naming the host and session id, rather than
    /// turning the whole call into an error. These rows are last-known
    /// DISPLAY data for the stale list a caller falls back to when a host
    /// is unreachable, not an authority anything else depends on being
    /// complete — one poisoned blob taking down the entire stale view for
    /// a host would be a worse outcome than quietly omitting the one entry
    /// that cannot be shown. Contrast [`Self::list_hosts`], which fails
    /// loudly on a corrupt registry row instead: see that method's own
    /// docs for why the two reads deliberately disagree.
    pub async fn cached_sessions(&self, host: HostId) -> anyhow::Result<Vec<SessionInfo>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<SessionInfo>> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            let mut stmt = conn
                .prepare(
                    "SELECT session_id, created_at, info_json FROM session_cache \
                     WHERE host_id = ?1",
                )
                .context("preparing cached session query")?;
            let mut rows: Vec<(String, i64, String)> = stmt
                .query_map(rusqlite::params![host], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .context("querying cached sessions")?
                .collect::<Result<_, _>>()
                .context("reading cached session rows")?;
            rows.sort_by(|a, b| {
                (std::cmp::Reverse(a.1), a.0.as_str()).cmp(&(std::cmp::Reverse(b.1), b.0.as_str()))
            });
            Ok(rows
                .into_iter()
                .filter_map(|(session_id, _, json)| match serde_json::from_str(&json) {
                    Ok(info) => Some(info),
                    Err(error) => {
                        // See this method's own docs for why a decode
                        // failure here is skipped-and-logged rather than
                        // propagated: unlike `list_hosts`, this cache is
                        // not authority for anything.
                        tracing::warn!(
                            host,
                            session_id = session_id.as_str(),
                            error = %error,
                            "skipping a cached session whose info_json no longer decodes"
                        );
                        None
                    }
                })
                .collect())
        })
        .await
        .context("cached sessions task panicked")?
    }

    /// Every cached row of the given hosts, decoded where possible — the
    /// persisted half of the merged session list (`crate::aggregate`).
    ///
    /// One read, one lock hold, every row in scope: the list is served whole
    /// (SPEC.md's Session list section), so this decodes each payload in
    /// scope on every request and hands the merge a plain `Vec` to filter,
    /// count and sort in memory. The work is bounded PER HOST — up to
    /// `farhelm_proto::LIST_SESSIONS_CAP` rows each — so the decode cost
    /// grows with the number of cache-serving hosts, and only the merged
    /// OUTPUT is cut back to one cap. At the scale SPEC.md fixes (a few
    /// hosts) that is still a few hundred small JSON blobs, and it is the
    /// wanted shape — the paged, indexed read this replaced existed to
    /// avoid exactly this decode, for fleets the product is not built for.
    ///
    /// A row is left OUT of the result in two cases, each a `warn!` naming
    /// the host and session id: its payload no longer decodes, or it
    /// decodes to a session whose `id` or `created_at` disagrees with the
    /// columns it is filed under. The second is poison too — the columns
    /// are what the one-owner index and every lookup here are built on, so
    /// showing such a row would list it under one identity and route it
    /// under another. Neither row reaches the served list or its counts
    /// (see [`CachedRow`]); the warning, repeated on every read for as long
    /// as the row exists, is what makes the corruption visible.
    ///
    /// `hosts` is the SCOPE: a host that serves from memory (no identity to
    /// bind a cache write to) is left out by the caller, from the same
    /// snapshot its in-memory rows are merged from, so one host can never
    /// contribute twice. The `IN`-list is bound by position; SQLite has no
    /// array parameter.
    pub async fn cached_rows(&self, hosts: &[HostId]) -> anyhow::Result<Vec<CachedRow>> {
        Ok(self.cached_slice(hosts).await?.rows)
    }

    /// [`Self::cached_rows`] plus, from the SAME lock hold, which of the
    /// scoped hosts' caches were cut at the wire's cap
    /// (`hosts.cache_truncated`).
    ///
    /// One method rather than two calls because the pairing is a
    /// correctness requirement, not convenience: a refresh replaces a
    /// host's rows and its flag in one transaction, so a reader that took
    /// the flag in one call and the rows in another could pair a newly
    /// capped cache's rows with the pre-cap "complete" flag — and a reply
    /// built from that pair presents a cut list as whole, exactly what
    /// SPEC.md's Session list section forbids. Both queries run under one
    /// hold of the store's connection mutex, which no writer can
    /// interleave.
    pub async fn cached_slice(&self, hosts: &[HostId]) -> anyhow::Result<CachedSlice> {
        if hosts.is_empty() {
            return Ok(CachedSlice::default());
        }
        let conn = Arc::clone(&self.conn);
        let hosts = hosts.to_vec();
        tokio::task::spawn_blocking(move || -> anyhow::Result<CachedSlice> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            let placeholders = (1..=hosts.len())
                .map(|index| format!("?{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT host_id, session_id, created_at, archived, info_json \
                     FROM session_cache WHERE host_id IN ({placeholders})"
                ))
                .context("preparing the cached rows query")?;
            let rows: Vec<(HostId, String, i64, bool, String)> = stmt
                .query_map(rusqlite::params_from_iter(hosts.iter()), |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                })
                .context("querying cached rows")?
                .collect::<Result<_, _>>()
                .context("reading cached rows")?;
            // Same lock hold as the rows, per this method's contract.
            let mut flags = conn
                .prepare(&format!(
                    "SELECT id FROM hosts WHERE cache_truncated AND id IN ({placeholders})"
                ))
                .context("preparing the cache flag query")?;
            let truncated_hosts: Vec<HostId> = flags
                .query_map(rusqlite::params_from_iter(hosts.iter()), |r| r.get(0))
                .context("querying cache flags")?
                .collect::<Result<_, _>>()
                .context("reading cache flags")?;
            let rows = rows
                .into_iter()
                .filter_map(|(host, session_id, created_at, archived, json)| {
                    let info = match serde_json::from_str::<SessionInfo>(&json) {
                        Ok(info) if info.id == session_id && info.created_at == created_at => info,
                        Ok(info) => {
                            tracing::warn!(
                                host,
                                session_id = session_id.as_str(),
                                payload_id = info.id.as_str(),
                                payload_created_at = info.created_at,
                                "skipping a cached session whose payload names a different \
                                 session than the row it is filed under"
                            );
                            return None;
                        }
                        Err(error) => {
                            tracing::warn!(
                                host,
                                session_id = session_id.as_str(),
                                error = %error,
                                "skipping a cached session whose info_json no longer decodes"
                            );
                            return None;
                        }
                    };
                    Some(CachedRow {
                        host,
                        archived,
                        info,
                    })
                })
                .collect();
            Ok(CachedSlice {
                rows,
                truncated_hosts,
            })
        })
        .await
        .context("cached slice task panicked")?
    }

    /// One host's cached entry for `session_id`, if it has one and it still
    /// decodes.
    ///
    /// The stale-detail read: what `GET /api/sessions/{id}` serves behind
    /// SPEC.md's host-unreachable notice. Scoped to a host rather than
    /// searching, because the caller has already resolved the owner
    /// ([`Self::host_of_session`]) and asking again by session id alone
    /// would be a second, independently-fallible answer to a question
    /// already settled.
    pub async fn cached_session(
        &self,
        host: HostId,
        session_id: &str,
    ) -> anyhow::Result<Option<SessionInfo>> {
        let conn = Arc::clone(&self.conn);
        let session_id = session_id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<SessionInfo>> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            let json: Option<String> = conn
                .query_row(
                    "SELECT info_json FROM session_cache WHERE host_id = ?1 AND session_id = ?2",
                    rusqlite::params![host, session_id],
                    |r| r.get(0),
                )
                .optional()
                .context("reading one cached session")?;
            Ok(json.and_then(|json| match serde_json::from_str::<SessionInfo>(&json) {
                // The payload must AGREE with the id it was filed under.
                // Serving a row whose own id says something else would show
                // the caller one session's metadata under another's name —
                // the same column/payload contradiction the page scan
                // treats as poison, and the same answer: nothing is shown,
                // and it is said out loud.
                Ok(info) if info.id != session_id => {
                    tracing::warn!(
                        host,
                        session_id = session_id.as_str(),
                        payload_id = info.id.as_str(),
                        "a cached session's payload names a different session; it cannot be shown"
                    );
                    None
                }
                Ok(info) => Some(info),
                Err(error) => {
                    tracing::warn!(
                        host,
                        session_id = session_id.as_str(),
                        error = %error,
                        "a cached session's info_json no longer decodes; it cannot be shown"
                    );
                    None
                }
            }))
        })
        .await
        .context("cached session task panicked")?
    }

    /// Which host holds `session_id` in its cache — the owner lookup every
    /// session operation routes through (PLAN_M6.md item 5).
    ///
    /// Deliberately does NOT read `info_json`. Routing is a question about
    /// where to ask, not about what the session is, so a row whose payload
    /// no longer decodes still routes correctly and a live session is never
    /// made unreachable by a corrupt copy of its own metadata.
    ///
    /// `None` means the cache does not know this session. That is a 404 at
    /// the REST edge for a session nobody has, and NOT the end of the
    /// lookup for one that exists: an identity-less connected host caches
    /// nothing at all, and its sessions are resolved from the manager's
    /// in-memory list instead (see `crate::route_session`).
    ///
    /// FAILS CLOSED on two owners, with [`HostStoreError::SessionOwnerAmbiguous`].
    /// The `session_cache_one_owner` index makes that unconstructible for
    /// this build's writers, so the `LIMIT 2` here is defense against a
    /// database this build did not write — and the alternative, picking the
    /// lower host id, would mean silently routing a stop at whichever of two
    /// machines sorted first.
    pub async fn host_of_session(&self, session_id: &str) -> anyhow::Result<Option<HostId>> {
        let conn = Arc::clone(&self.conn);
        let session_id = session_id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<HostId>> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            let mut stmt = conn
                .prepare(
                    "SELECT host_id FROM session_cache WHERE session_id = ?1 \
                     ORDER BY host_id ASC LIMIT 2",
                )
                .context("preparing the session owner query")?;
            let owners: Vec<HostId> = stmt
                .query_map(rusqlite::params![session_id], |r| r.get(0))
                .context("looking up a cached session's host")?
                .collect::<Result<_, _>>()
                .context("reading session owner rows")?;
            match owners.as_slice() {
                [] => Ok(None),
                [only] => Ok(Some(*only)),
                [first, second, ..] => {
                    Err(anyhow::Error::new(HostStoreError::SessionOwnerAmbiguous {
                        session: session_id,
                        first: *first,
                        second: *second,
                    }))
                }
            }
        })
        .await
        .context("session owner lookup task panicked")?
    }

    /// Return the helm-owned catalog in stable id order.
    ///
    /// Listing does not repair or reseed rows: user edits and deletions are
    /// durable choices, while malformed persisted data is reported by the
    /// decoder instead of silently normalized into a different profile.
    pub async fn profiles(&self) -> anyhow::Result<Vec<farhelm_proto::Profile>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            let mut statement = conn
                .prepare("SELECT id, name, invocation, agent_kind, resume_template FROM profiles ORDER BY id")
                .context("preparing profile list query")?;
            statement
                .query_map([], read_profile_columns)
                .context("querying profiles")?
                .map(|row| {
                    let columns = row.context("reading profile row")?;
                    decode_profile_row(columns)
                })
                .collect()
        })
        .await
        .context("profile list task panicked")?
    }

    /// Read one helm-owned profile, returning `None` for an unknown id so
    /// update and delete routes can distinguish absence from storage failure.
    pub async fn profile(&self, id: &str) -> anyhow::Result<Option<farhelm_proto::Profile>> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            let row = conn
                .query_row(
                    "SELECT id, name, invocation, agent_kind, resume_template FROM profiles WHERE id = ?1",
                    rusqlite::params![id],
                    read_profile_columns,
                )
                .optional()
                .context("reading one profile")?;
            row.map(decode_profile_row).transpose()
        })
        .await
        .context("profile read task panicked")?
    }

    /// Insert a validated profile while enforcing the catalog bound in the
    /// same transaction as the insert, avoiding a check-then-insert race.
    pub async fn create_profile(
        &self,
        name: String,
        invocation: String,
        agent_kind: farhelm_proto::AgentKind,
        resume_template: Option<Vec<String>>,
    ) -> anyhow::Result<ProfileCreation> {
        farhelm_proto::validate_profile_fields(
            &name,
            &invocation,
            agent_kind,
            resume_template.as_deref(),
        )
        .map_err(|message| anyhow::anyhow!("refusing to store this profile: {message}"))?;
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn.transaction().context("beginning profile create transaction")?;
            let count: i64 = tx
                .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
                .context("counting profiles")?;
            if count as usize >= farhelm_proto::MAX_PROFILES {
                return Ok(ProfileCreation::CatalogFull);
            }
            let profile = farhelm_proto::Profile {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                invocation,
                agent_kind,
                resume_template,
            };
            tx.execute(
                "INSERT INTO profiles (id, name, invocation, agent_kind, resume_template) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    profile.id,
                    profile.name,
                    profile.invocation,
                    agent_kind_column(profile.agent_kind),
                    resume_template_column(profile.resume_template.as_deref()),
                ],
            )
            .context("inserting profile row")?;
            tx.commit().context("committing profile create")?;
            Ok(ProfileCreation::Created(profile))
        })
        .await
        .context("profile create task panicked")?
    }

    /// Replace a complete profile definition, preserving its immutable id so
    /// existing session snapshots continue to refer to the same definition.
    pub async fn update_profile(
        &self,
        profile: farhelm_proto::Profile,
    ) -> anyhow::Result<Option<farhelm_proto::Profile>> {
        farhelm_proto::validate_profile_fields(
            &profile.name,
            &profile.invocation,
            profile.agent_kind,
            profile.resume_template.as_deref(),
        )
        .map_err(|message| anyhow::anyhow!("refusing to store this profile: {message}"))?;
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            let changed = conn.execute(
                "UPDATE profiles SET name = ?2, invocation = ?3, agent_kind = ?4, resume_template = ?5 WHERE id = ?1",
                rusqlite::params![
                    profile.id,
                    profile.name,
                    profile.invocation,
                    agent_kind_column(profile.agent_kind),
                    resume_template_column(profile.resume_template.as_deref()),
                ],
            ).context("updating profile row")?;
            Ok((changed > 0).then_some(profile))
        })
        .await
        .context("profile update task panicked")?
    }

    /// Plant a profile row that the normal catalog decoder must reject.
    ///
    /// Mutation-order tests need a catalog read to fail without damaging the
    /// session cache or connection registry they use for routing. Production
    /// writers validate every field, so this test-only seam bypasses them in
    /// the narrowest possible way and leaves all ordinary store behavior real.
    #[cfg(test)]
    pub(crate) async fn plant_invalid_profile_for_test(&self) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            conn.execute(
                "INSERT INTO profiles (id, name, invocation, agent_kind, resume_template) \
                 VALUES ('invalid-test-profile', 'broken', 'agent', 'unknown', NULL)",
                [],
            )
            .context("planting invalid profile row")?;
            Ok(())
        })
        .await
        .context("invalid profile fixture task panicked")?
    }

    /// Delete one profile and report whether its id existed; the raw
    /// remembered default is intentionally left untouched when it dangles.
    pub async fn delete_profile(&self, id: &str) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            Ok(conn.execute("DELETE FROM profiles WHERE id = ?1", rusqlite::params![id])? > 0)
        })
        .await
        .context("profile delete task panicked")?
    }

    /// Read the raw remembered id, including an id whose profile was deleted.
    pub async fn remembered_profile(&self) -> anyhow::Result<Option<String>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            conn.query_row(
                "SELECT profile_id FROM remembered_profile WHERE singleton = 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .context("reading the remembered default profile")
        })
        .await
        .context("remembered profile task panicked")?
    }

    /// Remember `profile_id` without a session provenance marker.
    ///
    /// Kept for administrative and test callers that do not have the
    /// creating session in hand. Production create handling uses
    /// [`Self::remember_profile_default_from_session`] so later drains can
    /// prove which observation is newer.
    pub async fn remember_profile_default(&self, profile_id: &str) -> anyhow::Result<bool> {
        self.remember_profile_default_with_source(profile_id, None, None, None, None)
            .await
    }

    /// Remember a successful profile-backed create whose host owns its sequence.
    ///
    /// `source_host` makes a supervisor-local sequence comparable only with
    /// another observation from that same supervisor. Cross-host observations
    /// instead use the timestamp/id fallback, which is the only ordering key
    /// those independent catalogs share.
    pub async fn remember_profile_default_from_host_session(
        &self,
        profile_id: &str,
        source_host: HostId,
        source_creation_seq: Option<u64>,
        source_created_at: i64,
        source_session_id: &str,
    ) -> anyhow::Result<bool> {
        self.remember_profile_default_with_source(
            profile_id,
            Some(source_host),
            source_creation_seq,
            Some(source_created_at),
            Some(source_session_id),
        )
        .await
    }

    /// Remember a successful profile-backed create without a known host.
    ///
    /// This remains for administrative and test callers. Production session
    /// creation uses [`Self::remember_profile_default_from_host_session`] so
    /// the stored sequence keeps its ordering domain. Without that domain,
    /// comparisons deliberately ignore both local sequences and use the
    /// timestamp/session-id fallback.
    pub async fn remember_profile_default_from_session(
        &self,
        profile_id: &str,
        source_creation_seq: Option<u64>,
        source_created_at: i64,
        source_session_id: &str,
    ) -> anyhow::Result<bool> {
        self.remember_profile_default_with_source(
            profile_id,
            None,
            source_creation_seq,
            Some(source_created_at),
            Some(source_session_id),
        )
        .await
    }

    /// Write `profile_id` as the helm-wide last-used profile, replacing
    /// whatever was there.
    ///
    /// Written both by a successful profile-backed create and by a completed
    /// session drain. Both observations mean a session was actually created
    /// from the profile; merely opening a picker does not. Their supervisor
    /// creation sequence decides chronology only when both observations came
    /// from the same host. Otherwise `(created_at, session id)` is the one
    /// fleet-wide ordering key available.
    /// Returns whether the visible profile id changed, so the invalidation
    /// feed does not wake every client each time a user creates from the same
    /// profile twice in a row. `false` does not necessarily mean the candidate
    /// matched the stored provenance: it also means an out-of-order candidate
    /// was rejected as older. Callers must not treat it as proof that this
    /// observation became the remembered source.
    ///
    /// The value is intentionally not tied to a host or installation. A
    /// create whose reply lands after a host retarget records the id all the
    /// same; the client can still replace the suggestion before creating.
    async fn remember_profile_default_with_source(
        &self,
        profile_id: &str,
        source_host: Option<HostId>,
        source_creation_seq: Option<u64>,
        source_created_at: Option<i64>,
        source_session_id: Option<&str>,
    ) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        let profile_id = profile_id.to_string();
        let source_session_id = source_session_id.map(str::to_string);
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let stored_source_creation_seq = source_creation_seq
                .map(i64::try_from)
                .transpose()
                .context("creation sequence exceeds SQLite's integer range")?;
            let mut conn = conn.lock().expect("helm db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning remembered-default transaction")?;
            // Read the singleton once inside the write transaction. The
            // provenance comparison and replacement must judge the same
            // prior row; a separate read would make an out-of-order drain
            // race with another writer.
            let known: Option<RememberedProfileRow> = tx
                .query_row(
                    "SELECT profile_id, source_host_id, source_creation_seq, source_created_at, source_session_id \
                     FROM remembered_profile WHERE singleton = 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()
                .context("checking the remembered default row")?;
            let (previous_profile, previous_host, previous_creation_seq, previous_created_at, previous_session_id) =
                known.unwrap_or_default();
            if let (Some(candidate_at), Some(candidate_id), Some(stored_at), Some(stored_id)) = (
                source_created_at,
                source_session_id.as_deref(),
                previous_created_at,
                previous_session_id.as_deref(),
            ) && !source_is_newer(
                ProfileSource { host: source_host, sequence: source_creation_seq, created_at: candidate_at, session_id: candidate_id },
                ProfileSource { host: previous_host, sequence: previous_creation_seq.and_then(|seq| u64::try_from(seq).ok()), created_at: stored_at, session_id: stored_id },
            ) {
                tx.commit()
                    .context("committing an unchanged remembered default")?;
                return Ok(false);
            }
            let changed = previous_profile != profile_id;
            tx.execute(
                "INSERT INTO remembered_profile (\
                     singleton, profile_id, source_host_id, source_creation_seq, \
                     source_created_at, source_session_id\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT (singleton) DO UPDATE SET profile_id = excluded.profile_id, \
                     source_host_id = excluded.source_host_id, \
                     source_creation_seq = excluded.source_creation_seq, \
                     source_created_at = excluded.source_created_at, \
                     source_session_id = excluded.source_session_id",
                rusqlite::params![
                    1,
                    profile_id,
                    source_host,
                    stored_source_creation_seq,
                    source_created_at,
                    source_session_id
                ],
            )
            .context("remembering the default profile")?;
            tx.commit().context("committing the remembered default")?;
            Ok(changed)
        })
        .await
        .context("remember profile default task panicked")?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Open a fresh store in a throwaway directory. The `TempDir` guard
    /// must outlive the store in every caller — dropping it deletes the
    /// backing file out from under an open `Connection`.
    async fn fresh_store() -> (tempfile::TempDir, HelmStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");
        let store = HelmStore::open(&db_path).await.expect("open");
        (dir, store)
    }

    /// A minimal, valid [`SessionInfo`] for a given id/creation time —
    /// mirrors `client.rs`'s own test helper of the same shape, since both
    /// modules need "a session that round-trips" and neither needs the
    /// other's full field coverage.
    fn session(id: &str, created_at: i64) -> SessionInfo {
        SessionInfo {
            parent: None,
            archived: false,
            id: id.to_string(),
            title: id.to_string(),
            created_at,
            last_activity_at: created_at,
            creation_seq: None,
            cwd: format!("/{id}"),
            invocation: "agent".to_string(),
            status: farhelm_proto::SessionStatus::Running,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
        }
    }

    /// A cached session whose immutable source snapshot names `profile`.
    fn profiled_session(id: &str, created_at: i64, profile: &str) -> SessionInfo {
        SessionInfo {
            source_profile: Some(farhelm_proto::SourceProfile {
                id: profile.to_string(),
                name: format!("Profile {profile}"),
                existence: farhelm_proto::ProfileExistence::Present,
            }),
            ..session(id, created_at)
        }
    }

    /// A profile-backed fixture with a supervisor-assigned creation sequence.
    fn sequenced_profiled_session(
        id: &str,
        created_at: i64,
        creation_seq: u64,
        profile: &str,
    ) -> SessionInfo {
        SessionInfo {
            creation_seq: Some(creation_seq),
            ..profiled_session(id, created_at, profile)
        }
    }

    /// Register an ssh host and immediately first-contact it with
    /// `identity`, returning the assigned id — the setup practically every
    /// cache test below needs now that [`HelmStore::replace_host_sessions`]
    /// is identity-bound (A2): a cache write requires a currently-matching
    /// identity to already be on record, which in the real system is
    /// exactly the order a connection manager's hello-then-list-refresh
    /// sequencing produces.
    async fn host_with_identity(store: &HelmStore, destination: &str, identity: &str) -> HostId {
        let host = store.add_ssh_host(destination, None, None).await.unwrap();
        let outcome = store
            .record_first_contact(host, &dialed_as(store, host).await, identity)
            .await
            .unwrap();
        assert_eq!(
            outcome,
            FirstContactOutcome::Recorded,
            "helper assumes a freshly added host with no prior identity"
        );
        host
    }

    /// The configuration a host currently carries — what a real caller
    /// captures before it dials and hands back to the identity writers
    /// (see [`DialedAs`]). Every test that is not ABOUT staleness reads it
    /// fresh, so it always matches and the guard stays out of the way.
    async fn dialed_as(store: &HelmStore, host: HostId) -> DialedAs {
        store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .find(|row| row.id == host)
            .map(DialedAs::of)
            .expect("host row")
    }

    /// One host's claim as the registry actually holds it — the
    /// independent check that a refused write really did write nothing,
    /// rather than merely returning an error on its way out.
    async fn recorded_identity(store: &HelmStore, host: HostId) -> Option<String> {
        store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .expect("host row")
            .host_identity
    }

    /// A host's cached session ids, in wire order — a terser way to assert
    /// "exactly these sessions, in this order" than mapping
    /// `cached_sessions`'s output by hand at every call site below.
    async fn cached_ids(store: &HelmStore, host: HostId) -> Vec<String> {
        store
            .cached_sessions(host)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect()
    }

    // ---- Tracing capture, for the skip-and-log tests -------------------
    //
    // `cached_sessions`/`cached_rows`'s skip-and-log posture (item
    // 2's split with `list_hosts`'s fail-loudly registry reads) logs a
    // dropped row via `tracing::warn!` rather than merely omitting it from
    // the returned `Vec` — and a silently-dropped row is observationally
    // IDENTICAL to a warned one from the `Vec` alone, so a test asserting
    // only "the poisoned row is missing" could not tell a future
    // regression that deleted the `tracing::warn!` call from one that
    // never had it. Observing the log line is what closes that gap.
    //
    // The capture itself lives in `crate::test_capture` rather than here:
    // it must be installed process-globally to reach the `spawn_blocking`
    // thread these warnings fire on, a process global can only be claimed
    // once, and this crate's whole test suite shares one binary — so the
    // buffer is everyone's, and each test below filters it down to its own
    // unique session id.

    /// Every "skipped a cached session" warning captured so far.
    fn skip_warnings() -> Vec<crate::test_capture::CapturedEvent> {
        // The shared PREFIX of every skip warning this module writes, rather
        // than one reader's exact sentence: the per-host read explains the
        // decode failure inline, while the page scan carries the reason in a
        // field (its predicate is shared with the matching count, which
        // reports nothing). What both must always do — and what these tests
        // are actually about — is say that a row was skipped, and name the
        // host and session it belonged to.
        crate::test_capture::matching(&crate::test_capture::install(), "skipping a cached session")
    }

    /// Whether a host's cached list was cut at the cap is kept WITH the
    /// cache: written by the same replacement that writes the rows, read
    /// back from the registry row, counted as a change when it flips, and
    /// still there after the store is closed and reopened.
    ///
    /// The reopen leg is the one that matters: SPEC.md forbids presenting a
    /// cut list as the whole one, and a helm restart serves every host's
    /// stale cache before any host has been re-drained. A flag that lived
    /// only in an actor's memory would be false for exactly that window.
    #[tokio::test]
    async fn a_cut_lists_flag_is_kept_with_the_cache_and_survives_a_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");
        let host = {
            let store = HelmStore::open(&db_path).await.expect("open");
            let host = host_with_identity(&store, "capped@host", "capped-identity").await;
            let first = store
                .replace_host_sessions(host, "capped-identity", vec![session("s-1", 100)], true)
                .await
                .expect("first drain");
            assert!(first.changed);
            let row = |rows: Vec<HostRow>| rows.into_iter().find(|r| r.id == host).unwrap();
            assert!(
                row(store.list_hosts().await.unwrap()).cache_truncated,
                "the flag is read back from the host's own registry row"
            );

            let same = store
                .replace_host_sessions(host, "capped-identity", vec![session("s-1", 100)], true)
                .await
                .expect("same drain again");
            assert!(
                !same.changed,
                "the same rows under the same flag are not a change the feed should announce"
            );
            let cleared = store
                .replace_host_sessions(host, "capped-identity", vec![session("s-1", 100)], false)
                .await
                .expect("uncut drain");
            assert!(
                cleared.changed,
                "the flag flipping IS a change: the notice it drives is on screen"
            );
            assert!(!row(store.list_hosts().await.unwrap()).cache_truncated);
            store
                .replace_host_sessions(host, "capped-identity", vec![session("s-1", 100)], true)
                .await
                .expect("cut again");
            host
        };

        let reopened = HelmStore::open(&db_path).await.expect("reopen");
        let row = reopened
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.id == host)
            .unwrap();
        assert!(
            row.cache_truncated,
            "a helm restarting over this database must still know the cached list was cut"
        );
    }

    /// A database migrated from before the cap flag existed reports every
    /// host's cache as NOT cut, and keeps saying so across a reopen.
    ///
    /// The migration adds `hosts.cache_truncated` with a false default; a
    /// defect that initialized existing hosts to true would pass the
    /// schema-equivalence test while showing every upgraded, currently
    /// unreachable host a "could not read to the end" notice no refresh
    /// could clear. Read back through `HostRow`, which is the read the
    /// merge actually uses.
    #[tokio::test]
    async fn migration_initializes_the_cap_flag_to_false_for_existing_hosts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");
        {
            let conn = plant_v1_database(&db_path);
            conn.execute_batch(
                "INSERT INTO hosts (kind, destination) VALUES ('ssh', 'user@planted');
                 INSERT INTO session_cache (host_id, session_id, created_at, info_json)
                 VALUES (1, 'planted-1', 100, '{}');",
            )
            .expect("plant a host with a cached row");
        }
        let flag = |rows: Vec<HostRow>| {
            rows.into_iter()
                .find(|r| r.destination.as_deref() == Some("user@planted"))
                .expect("planted row")
                .cache_truncated
        };
        let store = HelmStore::open(&db_path).await.expect("migrate and open");
        assert!(
            !flag(store.list_hosts().await.expect("list")),
            "an upgraded host starts with no cut on record"
        );
        drop(store);
        let reopened = HelmStore::open(&db_path).await.expect("reopen");
        assert!(!flag(reopened.list_hosts().await.expect("list")));
    }

    /// Identity adoption resets the cap flag along with the cache it purges.
    ///
    /// The flag is the predecessor install's word about rows the adoption
    /// deletes; carried over, an empty successor cache would show "could
    /// not read to the end" until a refresh succeeded — indefinitely, for a
    /// host whose first post-adoption refresh fails.
    #[tokio::test]
    async fn adoption_clears_the_cap_flag_with_the_cache() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "adopt@host", "old-identity").await;
        store
            .replace_host_sessions(host, "old-identity", vec![session("old-1", 100)], true)
            .await
            .expect("capped drain under the old identity");
        let row = |rows: Vec<HostRow>| rows.into_iter().find(|r| r.id == host).unwrap();
        assert!(row(store.list_hosts().await.unwrap()).cache_truncated);

        store
            .adopt_identity(
                host,
                &dialed_as(&store, host).await,
                "old-identity",
                "new-identity",
            )
            .await
            .expect("adopt");
        let after = row(store.list_hosts().await.unwrap());
        assert!(
            !after.cache_truncated,
            "the successor starts with no cut on record; the flag described purged rows"
        );
        assert!(
            store.cached_rows(&[host]).await.unwrap().is_empty(),
            "and the cache it described is gone"
        );
    }

    /// The 501st seed evicts the OLDEST cached row rather than growing the
    /// slice or refusing the new one, and records the cut on the host.
    ///
    /// The new row must land — the create already succeeded on the
    /// supervisor, and a cache without it leaves a session the caller was
    /// just told exists unroutable — so the cap holds by eviction, and the
    /// flag is what tells every client the cache no longer carries
    /// everything known.
    #[tokio::test]
    async fn a_seed_past_the_cap_evicts_the_oldest_row_and_records_the_cut() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "full@host", "full-identity").await;
        let full: Vec<SessionInfo> = (0..farhelm_proto::LIST_SESSIONS_CAP)
            .map(|n| session(&format!("s-{n:04}"), 1_000 + n as i64))
            .collect();
        store
            .replace_host_sessions(host, "full-identity", full, false)
            .await
            .expect("fill the cache to the cap");

        let changed = store
            .remember_session(host, "full-identity", &session("fresh", 10_000))
            .await
            .expect("seed past the cap");
        assert!(changed, "an eviction is a change clients must re-read for");
        let rows = store.cached_rows(&[host]).await.expect("cached rows");
        assert_eq!(rows.len(), farhelm_proto::LIST_SESSIONS_CAP);
        let ids: Vec<&str> = rows.iter().map(|row| row.info.id.as_str()).collect();
        assert!(ids.contains(&"fresh"), "the new session must be routable");
        assert!(
            !ids.contains(&"s-0000"),
            "the oldest row is the one evicted"
        );
        let slice = store.cached_slice(&[host]).await.expect("slice");
        assert_eq!(
            slice.truncated_hosts,
            vec![host],
            "the eviction is recorded as a cut on the host"
        );
    }

    // ---- The shared client preference ---------------------------------

    /// The preference row is one shared answer with three-way per-field
    /// merge semantics: a fresh store reads as all-unset, an ABSENT field in
    /// a patch leaves the stored value alone, a `null` clears it, a value
    /// replaces it, and a patch naming nothing changes nothing.
    ///
    /// Absent-versus-null is the property worth pinning. Two clients share
    /// this row, and the failure a whole-row write would produce — a
    /// re-sort in one window silently un-selecting what the other window
    /// just clicked — is invisible until someone relaunches and lands on
    /// the wrong session; the clear, conversely, must really clear, or a
    /// harness asking for "nothing remembered" inherits the last test's row.
    #[tokio::test]
    async fn the_preference_row_merges_per_field_and_null_clears() {
        let (_dir, store) = fresh_store().await;
        assert_eq!(
            store.preferences().await.unwrap(),
            Preferences::default(),
            "a fresh store has nothing remembered"
        );

        // The JSON shapes a client actually sends, decoded through serde so
        // the absent/null distinction is exercised where it is made.
        let patch = |json: &str| serde_json::from_str::<PreferencePatch>(json).unwrap();
        store
            .update_preferences(patch(r#"{"list_sort":"title"}"#))
            .await
            .unwrap();
        store
            .update_preferences(patch(r#"{"last_selected":"session-1"}"#))
            .await
            .unwrap();
        store
            .update_preferences(patch(r#"{"compact":true}"#))
            .await
            .unwrap();
        assert_eq!(
            store.preferences().await.unwrap(),
            Preferences {
                list_sort: Some("title".to_string()),
                last_selected: Some("session-1".to_string()),
                compact: Some(true),
            },
            "a selection write must not discard the sort written before it"
        );

        store
            .update_preferences(patch(r#"{"list_sort":"created"}"#))
            .await
            .unwrap();
        store.update_preferences(patch("{}")).await.unwrap();
        assert_eq!(
            store.preferences().await.unwrap(),
            Preferences {
                list_sort: Some("created".to_string()),
                last_selected: Some("session-1".to_string()),
                compact: Some(true),
            },
            "a later sort replaces the earlier one, and an empty patch is a no-op"
        );

        store
            .update_preferences(patch(r#"{"last_selected":null}"#))
            .await
            .unwrap();
        assert_eq!(
            store.preferences().await.unwrap(),
            Preferences {
                list_sort: Some("created".to_string()),
                last_selected: None,
                compact: Some(true),
            },
            "an explicit null clears exactly the field it names"
        );

        store
            .update_preferences(patch(r#"{"compact":false}"#))
            .await
            .unwrap();
        assert_eq!(
            store.preferences().await.unwrap().compact,
            Some(false),
            "false is an explicit stored choice rather than the absent default"
        );
        store
            .update_preferences(patch(r#"{"compact":null}"#))
            .await
            .unwrap();
        assert_eq!(
            store.preferences().await.unwrap().compact,
            None,
            "null restores the absent compact choice without touching other fields"
        );
    }

    // ---- Per-session "seen" state ---------------------------------------

    /// The whole read/write/clear cycle behind the idle-dot's blue/grey
    /// split (SPEC.md, Status): marking sets the stamp,
    /// re-marking a DIFFERENT stamp replaces it, and clearing removes it
    /// entirely rather than leaving a tombstone a later read might trip
    /// over. No `session_cache` row is planted for any of the ids used here
    /// — deliberately, since `session_seen` carries no foreign key to that
    /// table (see the table's own doc comment) and this test would
    /// otherwise imply a dependency that does not exist.
    #[tokio::test]
    async fn mark_seen_and_clear_seen_round_trip() {
        let (_dir, store) = fresh_store().await;

        assert!(
            store
                .seen_activity(&["s-1".to_string()])
                .await
                .unwrap()
                .is_empty(),
            "an id nothing has marked has no row at all"
        );

        store.mark_seen("s-1", 1_700_000_000).await.unwrap();
        let map = store.seen_activity(&["s-1".to_string()]).await.unwrap();
        assert_eq!(map.get("s-1"), Some(&1_700_000_000));

        // Re-marking with a NEWER stamp replaces the row rather than
        // refusing a second write — the store records whatever the caller
        // last observed, exactly as `update_preferences` does for its own
        // fields.
        store.mark_seen("s-1", 1_700_000_100).await.unwrap();
        let map = store.seen_activity(&["s-1".to_string()]).await.unwrap();
        assert_eq!(map.get("s-1"), Some(&1_700_000_100));

        store.clear_seen("s-1").await.unwrap();
        assert!(
            store
                .seen_activity(&["s-1".to_string()])
                .await
                .unwrap()
                .is_empty(),
            "a cleared stamp must not merely reset to some sentinel value — the row is gone"
        );
    }

    /// [`HelmStore::seen_activity`]'s listing join must answer for exactly
    /// the ids that have a row, silently omitting the rest, over a MIX of
    /// marked and unmarked ids in one call — the shape
    /// `aggregate::session_list_staged` actually uses it in, joining a
    /// whole page of rows at once rather than one id at a time.
    #[tokio::test]
    async fn seen_activity_joins_a_mixed_id_list() {
        let (_dir, store) = fresh_store().await;
        store.mark_seen("has-a-row", 42).await.unwrap();

        let ids = vec![
            "has-a-row".to_string(),
            "never-marked".to_string(),
            "also-never-marked".to_string(),
        ];
        let map = store.seen_activity(&ids).await.unwrap();
        assert_eq!(map.len(), 1, "only the marked id produces an entry");
        assert_eq!(map.get("has-a-row"), Some(&42));
        assert_eq!(map.get("never-marked"), None);
    }

    /// The store's own half of "bump the fleet-events revision on a
    /// change, not on a no-op" (SPEC_impl.md): the
    /// `PUT /api/sessions/{id}/seen` handler decides whether to bump off
    /// this return value, so it has to be right in both directions and at
    /// the boundary between them.
    #[tokio::test]
    async fn mark_seen_and_clear_seen_report_whether_anything_changed() {
        let (_dir, store) = fresh_store().await;

        assert!(
            store.mark_seen("s-1", 100).await.unwrap(),
            "the first mark for an id always changes the stored state"
        );
        assert!(
            !store.mark_seen("s-1", 100).await.unwrap(),
            "re-marking the SAME stamp must report no change"
        );
        assert!(
            store.mark_seen("s-1", 200).await.unwrap(),
            "a genuinely different stamp must report a change"
        );

        assert!(
            store.clear_seen("s-1").await.unwrap(),
            "clearing a row that exists must report a change"
        );
        assert!(
            !store.clear_seen("s-1").await.unwrap(),
            "clearing an id with no row must report no change"
        );
        assert!(
            !store.clear_seen("never-marked-at-all").await.unwrap(),
            "clearing an id this table has never heard of is a no-op, not an error"
        );
    }

    // ---- Schema and the version mechanism ----------------------------

    /// A fresh database must come up on the current schema with the reserved
    /// local row already present — the two invariants every other test in
    /// this module assumes without re-checking.
    #[tokio::test]
    async fn fresh_open_creates_the_current_schema_with_the_local_row_present() {
        let (_dir, store) = fresh_store().await;
        let conn = Arc::clone(&store.conn);
        let version: i64 = tokio::task::spawn_blocking(move || {
            conn.lock()
                .unwrap()
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap()
        })
        .await
        .unwrap();
        assert_eq!(version, SCHEMA_VERSION);

        let hosts = store.list_hosts().await.expect("list hosts");
        assert_eq!(hosts.len(), 1, "exactly one row after a fresh open");
        assert_eq!(hosts[0].kind, HostKind::Local);
        assert_eq!(hosts[0].destination, None);
    }

    /// Reopening an already-current database must be a no-op on the schema
    /// and must not mint a second local row — the sequential counterpart to
    /// the genuine-race test below.
    #[tokio::test]
    async fn reopen_is_stable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");
        {
            let store = HelmStore::open(&db_path).await.expect("first open");
            assert_eq!(store.list_hosts().await.unwrap().len(), 1);
        }
        let reopened = HelmStore::open(&db_path).await.expect("reopen");
        let hosts = reopened.list_hosts().await.expect("list hosts");
        assert_eq!(hosts.len(), 1, "reopening must not mint a second local row");
        assert_eq!(hosts[0].kind, HostKind::Local);
    }

    /// The migration-fixture scaffold `apply_schema`'s own docs promise: the
    /// REFUSAL half of the version mechanism — a database claiming a
    /// version this build does not understand — is exercised exactly like
    /// the supervisor store's `open_refuses_an_unrecognized_schema_version`,
    /// by planting a raw `user_version` directly with rusqlite rather than
    /// through `HelmStore::open`. The day a version 2 migration lands, its
    /// test plants the preceding fixture the same way this plants a too-new
    /// one.
    ///
    /// `SCHEMA_VERSION + 1` is the one value planted, not also some
    /// arbitrarily-larger one (e.g. 99): `apply_schema`'s refusal branch is
    /// a single `anyhow::bail!` with no further version-dependent logic
    /// behind it, so every version above `SCHEMA_VERSION` takes the exact
    /// same path — a second value would pin the identical assertion twice,
    /// not a distinct behavior.
    #[tokio::test]
    async fn open_refuses_an_unrecognized_schema_version() {
        let version = SCHEMA_VERSION + 1;
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");
        {
            let conn = Connection::open(&db_path).expect("create raw db");
            conn.pragma_update(None, "user_version", version).unwrap();
        }
        let err = HelmStore::open(&db_path)
            .await
            .expect_err("unrecognized schema version must be refused");
        assert!(
            format!("{err:#}").contains(&version.to_string()),
            "error must name the unrecognized version: {err:#}"
        );
    }

    /// A version-1 database, planted verbatim as that version shipped it:
    /// the schema BEFORE `hosts_identity_claim` existed, which is the only
    /// state from which the version-2 migration's duplicate resolution has
    /// anything to do. Written out in full rather than derived from
    /// [`apply_schema`] on purpose — a fixture that tracked the current
    /// code would stop being a fixture, and would quietly start testing the
    /// migration against a database no user ever had.
    fn plant_v1_database(path: &Path) -> Connection {
        let conn = Connection::open(path).expect("create raw db");
        conn.execute_batch(
            "CREATE TABLE hosts (
                 id               INTEGER PRIMARY KEY AUTOINCREMENT,
                 kind             TEXT NOT NULL CHECK (kind IN ('local', 'ssh')),
                 destination      TEXT,
                 remote_farhelm   TEXT,
                 remote_state_dir TEXT,
                 host_identity    TEXT,
                 CHECK (
                     (kind = 'local' AND destination IS NULL AND remote_farhelm IS NULL
                          AND remote_state_dir IS NULL)
                     OR (kind = 'ssh' AND destination IS NOT NULL)
                 )
             ) STRICT;
             CREATE UNIQUE INDEX hosts_one_local_row
                 ON hosts (kind) WHERE kind = 'local';
             CREATE UNIQUE INDEX hosts_ssh_destination
                 ON hosts (destination) WHERE kind = 'ssh';
             CREATE TABLE session_cache (
                 host_id    INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
                 session_id TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 info_json  TEXT NOT NULL,
                 PRIMARY KEY (host_id, session_id)
             ) STRICT;
             CREATE INDEX session_cache_order
                 ON session_cache (created_at DESC, session_id ASC);
             CREATE INDEX session_cache_by_host_order
                 ON session_cache (host_id, created_at DESC, session_id ASC);
             PRAGMA user_version = 1;",
        )
        .expect("plant the version-1 schema");
        conn
    }

    /// Every object the schema defines, as SQLite records it, normalized
    /// down to the part that is actually the schema — the comparison
    /// `a_migrated_database_matches_a_freshly_created_one` makes.
    ///
    /// SQLite stores each object's `CREATE` statement VERBATIM, comments
    /// and indentation included, and this module's DDL is heavily
    /// commented. A raw comparison would therefore fail on prose alone
    /// while the two databases were structurally identical — and, worse,
    /// could be "fixed" by pasting the comments into the migration, which
    /// would make the test pass without checking anything. Stripping `--`
    /// comments and collapsing whitespace compares the schema instead.
    /// `ALTER TABLE ADD COLUMN` also relocates whitespace before commas and
    /// closing parentheses; those differences do not change a constraint.
    /// This normalization is scoped to the DDL here: its string literals
    /// contain neither `--` nor the punctuation/space pairs removed below.
    fn schema_objects(conn: &Connection) -> Vec<(String, String)> {
        fn normalize(sql: &str) -> String {
            sql.lines()
                .map(|line| line.split("--").next().unwrap_or_default())
                .flat_map(str::split_whitespace)
                .collect::<Vec<_>>()
                .join(" ")
                .replace(" ,", ",")
                .replace(" )", ")")
        }

        let mut stmt = conn
            .prepare(
                "SELECT name, COALESCE(sql, '') FROM sqlite_master \
                 WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .expect("prepare schema query");
        stmt.query_map([], |r| {
            let name: String = r.get(0)?;
            let sql: String = r.get(1)?;
            Ok((name, normalize(&sql)))
        })
        .expect("query schema")
        .collect::<Result<_, _>>()
        .expect("read schema")
    }

    /// The migration's DATA half: a version-1 database may already hold two
    /// rows claiming ONE identity, because nothing stopped it. Version 2's
    /// index cannot simply be created over that, and which row keeps the
    /// claim must be a decision this code makes rather than one SQLite
    /// makes for it — the lowest [`HostId`] (the entry that learned it
    /// first) keeps it, later rows are demoted to unclaimed and lose the
    /// cache that was only ever meaningful under that identity.
    ///
    /// The stakes are the whole reason the constraint exists: left as-is,
    /// each of those rows sees the OTHER as its twin at the next helm start
    /// and both freeze as duplicates, so a live host appears ZERO times.
    /// The last assertion here is that the resolved database can no longer
    /// be talked into that shape — the demoted row's next contact is
    /// refused as a collision naming the survivor, which is a visible,
    /// resolvable duplicate rather than a mutual standoff.
    #[tokio::test]
    async fn migrating_from_v1_resolves_duplicated_identity_claims() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");
        {
            let conn = plant_v1_database(&db_path);
            conn.execute_batch(
                "INSERT INTO hosts (kind) VALUES ('local');
                 INSERT INTO hosts (kind, destination, host_identity)
                     VALUES ('ssh', 'first@host', 'shared-identity');
                 INSERT INTO hosts (kind, destination, host_identity)
                     VALUES ('ssh', 'second@host', 'shared-identity');
                 INSERT INTO hosts (kind, destination, host_identity)
                     VALUES ('ssh', 'unrelated@host', 'its-own-identity');",
            )
            .expect("plant two rows sharing one identity");
            // One session id PER HOST, deliberately distinct: this test is
            // about duplicated IDENTITY claims, and giving three hosts one
            // session id would additionally trip the version-3 migration's
            // own one-owner dedupe, which would then be doing the deleting
            // and the assertions below would be pinning the wrong rule.
            let identified: Vec<HostId> = {
                let mut stmt = conn
                    .prepare("SELECT id FROM hosts WHERE host_identity IS NOT NULL ORDER BY id")
                    .unwrap();
                stmt.query_map([], |r| r.get(0))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap()
            };
            for host in identified {
                let cached =
                    serde_json::to_string(&session(&format!("ghost-{host}"), 100)).unwrap();
                conn.execute(
                    "INSERT INTO session_cache (host_id, session_id, created_at, info_json)
                     VALUES (?1, ?2, 100, ?3)",
                    rusqlite::params![host, format!("ghost-{host}"), cached],
                )
                .expect("give every identified row a cache");
            }
        }

        let store = HelmStore::open(&db_path).await.expect("migrate and open");
        let hosts = store.list_hosts().await.expect("list hosts");
        let by_destination = |destination: &str| -> HostRow {
            hosts
                .iter()
                .find(|h| h.destination.as_deref() == Some(destination))
                .expect("planted row")
                .clone()
        };
        let first = by_destination("first@host");
        let second = by_destination("second@host");
        let unrelated = by_destination("unrelated@host");
        assert_eq!(
            first.host_identity.as_deref(),
            Some("shared-identity"),
            "the lowest id keeps the claim"
        );
        assert_eq!(
            second.host_identity, None,
            "every later claimant is demoted to unclaimed"
        );
        assert_eq!(
            unrelated.host_identity.as_deref(),
            Some("its-own-identity"),
            "a row whose identity was never duplicated must be left entirely alone"
        );
        assert_eq!(
            store.cached_sessions(first.id).await.unwrap().len(),
            1,
            "the surviving claimant keeps the cache its identity still vouches for"
        );
        assert_eq!(
            store.cached_sessions(second.id).await.unwrap(),
            Vec::new(),
            "a demoted row's cache goes with its claim — it was only meaningful under it"
        );
        assert_eq!(
            store.cached_sessions(unrelated.id).await.unwrap().len(),
            1,
            "an untouched row's cache must survive the migration"
        );

        // The both-frozen, zero-owner outcome is now unconstructible: the
        // demoted row cannot re-take the identity behind the survivor's
        // back, no matter how the two actors interleave at the next start.
        let outcome = store
            .record_first_contact(second.id, &DialedAs::of(&second), "shared-identity")
            .await
            .expect("a collision is a value, not a storage failure");
        assert_eq!(
            outcome,
            FirstContactOutcome::Collision { owner: first.id },
            "the demoted row's next contact must be a duplicate naming the survivor"
        );
    }

    /// The version-4 migration, which is the whole reason a helm upgraded
    /// across `PROTOCOL_VERSION` 10 does not silently lose the stale lists
    /// it exists to serve.
    ///
    /// The failure without it is quiet in the worst way: a cache row
    /// carrying the pre-v10 `{"state":"alive"}` no longer decodes, and the
    /// read path is deliberately forgiving of an undecodable row (it skips
    /// and logs, so one poisoned blob cannot take a whole list down) — so
    /// the sessions of every host that is currently DOWN would simply stop
    /// appearing, with the UI showing an empty list rather than any sign
    /// that something went wrong. Exactly the SPEC.md promise (an
    /// unreachable host's sessions stay listed) broken by an upgrade.
    ///
    /// Driven through a genuinely planted OLD database rather than by
    /// hand-writing a row into a current one: the fixture is what a real
    /// user's file looks like, and the whole ladder runs over it. All THREE
    /// cache readers are exercised, because they decode independently and a
    /// migration that fixed one would leave the others just as broken —
    /// `cached_sessions` (the per-host stale list), `cached_rows` (the
    /// merged-list backing read), and `cached_session` (the stale detail view
    /// behind an unreachable-host notice).
    #[tokio::test]
    async fn migrating_from_v3_rewrites_pre_split_cached_statuses() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");
        // The exact bytes a v9-era helm wrote: `alive`, and a `SessionInfo`
        // with none of the fields version 10 added. Hand-written rather
        // than serialized from any current type — a fixture built from
        // today's structs would stop being a fixture the moment they
        // changed, which is the same argument `plant_v1_database` makes for
        // the schema itself.
        let legacy_row = r#"{"id":"old-1","title":"before the upgrade","created_at":100,
             "cwd":"/work","invocation":"claude","status":{"state":"alive"},
             "annotation":null,"restart_offer":"fresh_only","tabs":[]}"#
            .replace('\n', "")
            .replace("             ", "");
        {
            let conn = plant_v1_database(&db_path);
            conn.execute_batch("INSERT INTO hosts (kind) VALUES ('local');")
                .expect("plant the local row");
            conn.execute(
                "INSERT INTO session_cache (host_id, session_id, created_at, info_json)
                 VALUES (1, 'old-1', 100, ?1)",
                rusqlite::params![legacy_row],
            )
            .expect("plant a pre-split cache row");
        }

        let store = HelmStore::open(&db_path).await.expect("migrate and open");
        // The reserved local row planted above; ids start at 1.
        let host: HostId = 1;

        let listed = store.cached_sessions(host).await.expect("stale list");
        assert_eq!(
            listed.len(),
            1,
            "an upgraded helm must still serve the session it cached before the upgrade"
        );
        assert_eq!(listed[0].id, "old-1");
        assert_eq!(
            listed[0].status,
            farhelm_proto::SessionStatus::Running,
            "the pre-split spelling maps to the status closest to what it meant"
        );
        // The rest of the record survives the rewrite untouched — a
        // migration that mangled the payload while fixing the status would
        // satisfy the assertion above and still be wrong.
        assert_eq!(listed[0].title, "before the upgrade");
        assert_eq!(listed[0].cwd, "/work");
        assert_eq!(listed[0].created_at, 100);
        assert_eq!(
            listed[0].source_profile, None,
            "a row written before the field existed reads as raw-created"
        );

        let rows = store.cached_rows(&[host]).await.expect("cached rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].info.id, "old-1",
            "the merged-list read must carry the row as DATA"
        );

        let detail = store
            .cached_session(host, "old-1")
            .await
            .expect("stale detail");
        assert_eq!(
            detail.map(|info| info.status),
            Some(farhelm_proto::SessionStatus::Running),
            "the stale detail view behind an unreachable-host notice must decode too"
        );
    }

    /// The version-10 migration, which is what makes an UPGRADED helm's
    /// default view count itself correctly on its very first read.
    ///
    /// Spec: every row that predates the `archived` column is backfilled
    /// from the flag inside its own payload, the version stamp lands at 10,
    /// and both counts survive a reopen — so the default view's `total`
    /// excludes archived sessions from the first read onward and the
    /// inclusion switch brings them back.
    ///
    /// Without the backfill the column would be 0 everywhere and every
    /// archived session would silently rejoin the denominator on upgrade —
    /// the exact incoherence this schema version exists to remove, restored
    /// by the migration meant to fix it.
    ///
    /// Three payload shapes, because the backfill reads JSON TEXT rather
    /// than `SessionInfo` and each shape pins a different consequence of
    /// that:
    ///
    /// - A well-formed payload is the ordinary case, in both flag positions.
    /// - A payload that is not JSON at all is the one case the backfill
    ///   cannot answer, and the direction it fails in is a decision rather
    ///   than an accident: it stays ACTIVE, matching [`HelmStore::cached_rows`]'s
    ///   standing rule that a row's presence in the fleet is never quietly
    ///   hidden just because nothing can read it.
    /// - A payload that is valid JSON this build's struct would reject —
    ///   what a NEWER farhelm's cache looks like to an older one — is still
    ///   classified from its `archived` member. That is the whole reason the
    ///   statement asks SQLite's JSON functions instead of serde, and a
    ///   regression to a struct decode would file such a row as active and
    ///   put a session the user archived back into the ordinary list.
    ///
    /// Reopened at the end because the point of a COLUMN is durability: a
    /// second `open` must find version 10, skip the ladder entirely, and
    /// still report the same two numbers. That is what distinguishes a
    /// backfilled column from a value some read path recomputes.
    ///
    /// Driven through a genuinely planted OLD database, like the version-4
    /// test above, so the whole ladder runs over the fixture a real user's
    /// file would have been.
    #[tokio::test]
    async fn migrating_to_v10_backfills_the_archive_flag_from_each_payload() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");
        {
            let conn = plant_v1_database(&db_path);
            conn.execute_batch("INSERT INTO hosts (kind) VALUES ('local');")
                .expect("plant the local row");
            let archived = serde_json::to_string(&SessionInfo {
                archived: true,
                ..session("archived-1", 300)
            })
            .unwrap();
            let active = serde_json::to_string(&session("active-1", 200)).unwrap();
            for (id, created_at, json) in [
                ("archived-1", 300, archived.as_str()),
                ("active-1", 200, active.as_str()),
                // Not JSON at all: the payload the backfill cannot read.
                ("undecodable", 100, "not valid json"),
                // Valid JSON, and NOT a `SessionInfo` this build could
                // decode — the stand-in for a cache written by a newer
                // farhelm. Its archive flag is still right there to read.
                (
                    "from-the-future",
                    50,
                    r#"{"archived":true,"shape":"unknown"}"#,
                ),
            ] {
                conn.execute(
                    "INSERT INTO session_cache (host_id, session_id, created_at, info_json)
                     VALUES (1, ?1, ?2, ?3)",
                    rusqlite::params![id, created_at, json],
                )
                .expect("plant a pre-column cache row");
            }
        }

        let store = HelmStore::open(&db_path).await.expect("migrate and open");
        // The reserved local row planted above; ids start at 1.
        let host: HostId = 1;

        let (flags, user_version): (Vec<(String, i64)>, i64) = {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().unwrap();
                let flags = {
                    let mut stmt = conn
                        .prepare(
                            "SELECT session_id, archived FROM session_cache ORDER BY session_id",
                        )
                        .unwrap();
                    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                        .unwrap()
                        .collect::<Result<_, _>>()
                        .unwrap()
                };
                let version: i64 = conn
                    .query_row("PRAGMA user_version", [], |r| r.get(0))
                    .unwrap();
                (flags, version)
            })
            .await
            .unwrap()
        };
        assert_eq!(
            flags,
            vec![
                ("active-1".to_string(), 0),
                ("archived-1".to_string(), 1),
                ("from-the-future".to_string(), 1),
                ("undecodable".to_string(), 0),
            ],
            "each row's flag must come from the JSON in its own payload, and a payload that is \
             not JSON at all stays active"
        );
        assert_eq!(
            user_version, SCHEMA_VERSION,
            "an unstamped migration replays the ADD COLUMN on the next open and fails there"
        );

        // What each view SERVES, as a pair, taken twice: once against the
        // store that just migrated, and once against a fresh open of the
        // same file. `cached_rows` is the merged-list backing read now — it
        // returns every row that decodes, regardless of view, and the
        // `archived` column is what a caller filters the two views by. The
        // two rows that do not decode are dropped from the read (and so
        // from both totals), which is what keeps the served counts about
        // rows a client can see; the column assertions above are what pin
        // that the backfill still classified them.
        async fn both_totals(store: &HelmStore, host: HostId) -> (u64, u64) {
            let rows = store.cached_rows(&[host]).await.expect("cached rows");
            let default_view = rows.iter().filter(|row| !row.archived).count() as u64;
            let widened_view = rows.len() as u64;
            (default_view, widened_view)
        }
        assert_eq!(
            both_totals(&store, host).await,
            (1, 2),
            "the default view serves the one active row that decodes; the inclusion switch \
             brings the decodable archived row back into the denominator, and neither view \
             counts the two rows nothing can show"
        );

        drop(store);
        let reopened = HelmStore::open(&db_path)
            .await
            .expect("reopen at the current version");
        assert_eq!(
            both_totals(&reopened, host).await,
            (1, 2),
            "the flag is stored, not recomputed: a reopen that skips the ladder must count the \
             same two views"
        );
    }

    /// A migrated database and a freshly created one must end up with
    /// identical schemas after SQL formatting normalization — the invariant
    /// that lets [`apply_schema`]'s version-0 branch create the final shape directly instead of
    /// replaying every historical step. Without this, a migration that
    /// forgot an index would leave upgraded installs subtly different from
    /// new ones, and every later test would pass on whichever of the two
    /// the CI machine happened to create.
    #[tokio::test]
    async fn a_migrated_database_matches_a_freshly_created_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        let migrated_path = dir.path().join("migrated.db");
        drop(plant_v1_database(&migrated_path));
        let migrated = HelmStore::open(&migrated_path).await.expect("migrate");
        let fresh = HelmStore::open(&dir.path().join("fresh.db"))
            .await
            .expect("create");

        let read = |store: &HelmStore| {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || schema_objects(&conn.lock().unwrap()))
        };
        let (migrated, fresh) = tokio::join!(read(&migrated), read(&fresh));
        assert_eq!(
            migrated.unwrap(),
            fresh.unwrap(),
            "the migration ladder and the fresh-create path must agree on the final schema"
        );
    }

    /// Schema 18 adds presentation state to an existing singleton without
    /// changing either preference older clients already wrote.
    ///
    /// This starts from the exact schema-17 difference rather than a fresh
    /// database: the compact column is absent, sort and selection are both
    /// populated, and opening through [`HelmStore`] must preserve them while
    /// exposing compact as the old-client default.
    #[tokio::test]
    async fn schema_18_preserves_the_version_17_preference_row() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("helm.db");
        {
            let store = HelmStore::open(&path).await.expect("create current schema");
            drop(store);
            let conn = Connection::open(&path).expect("reopen raw");
            conn.execute_batch(
                "ALTER TABLE preferences DROP COLUMN compact;
                 INSERT INTO preferences (singleton, list_sort, last_selected)
                 VALUES (1, 'title', 'session-before-compact');
                 PRAGMA user_version = 17;",
            )
            .expect("plant schema-17 preferences");
        }

        let migrated = HelmStore::open(&path).await.expect("migrate schema 17");
        assert_eq!(
            migrated.preferences().await.unwrap(),
            Preferences {
                list_sort: Some("title".to_string()),
                last_selected: Some("session-before-compact".to_string()),
                compact: None,
            },
            "the new field defaults absent while both existing choices survive"
        );
    }

    /// A version-5 database (a bare per-host default and nothing else) reaches the
    /// current schema with NO remembered default and its host registry intact.
    ///
    /// Schema 15 changed what the default IS — one helm-wide id instead of one per
    /// host — and the settled call was to drop the per-host rows rather than pick a
    /// winner among them. This pins that a real v5 file (built by downgrading a
    /// current one, so the row sits in the schema the old release actually shipped)
    /// migrates cleanly to the empty singleton, and that forgetting the preference
    /// never costs the registry.
    #[tokio::test]
    async fn a_version_5_bare_default_is_dropped_by_schema_15() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("helm.db");
        let host = {
            let store = HelmStore::open(&path).await.expect("create");
            let host = store.add_ssh_host("user@host", None, None).await.unwrap();
            store
                .remember_profile_default("starter-claude")
                .await
                .unwrap();
            host
        };

        // Back to the shape version 5 shipped: no identity column, and a row
        // recorded under it. The session cache is downgraded too — a fixture
        // stamped `user_version = 5` while still carrying a later version's
        // `archived` or ordering columns would make the ladder replay the ADD
        // COLUMN over a column that is already there, which is the migration
        // failing loudly at a state no real database can be in. (Version 12
        // already dropped the version-11 ordering columns, so only the
        // version-10 flag is left to remove.) `session_seen` (version 17)
        // goes the same way as every other post-version-5 table: it must
        // not exist yet, or the version-17 migration's plain `CREATE TABLE`
        // fails against one that is already there.
        {
            let conn = Connection::open(&path).expect("reopen raw");
            conn.execute_batch(
                "DROP TABLE device_sessions;
                 DROP TABLE web_token;
                 DROP TABLE profiles;
                 DROP TABLE remembered_profile;
                 DROP TABLE preferences;
                 DROP TABLE session_seen;
                 ALTER TABLE hosts DROP COLUMN cache_truncated;
                 ALTER TABLE hosts DROP COLUMN alias;
                 ALTER TABLE session_cache DROP COLUMN archived;
                 CREATE TABLE remembered_profiles (
                     host_id    INTEGER PRIMARY KEY
                                REFERENCES hosts (id) ON DELETE CASCADE,
                     profile_id TEXT NOT NULL
                 ) STRICT;
                 PRAGMA user_version = 5;",
            )
            .expect("downgrade the table");
            conn.execute(
                "INSERT INTO remembered_profiles (host_id, profile_id) VALUES (?1, 'starter-claude')",
                rusqlite::params![host],
            )
            .expect("plant a version-5 remembered default");
        }

        let migrated = HelmStore::open(&path).await.expect("migrate");
        assert_eq!(
            migrated.remembered_profile().await.unwrap().as_deref(),
            None,
            "schema 15 starts the replacement singleton empty"
        );
        // The host itself survives — this is a forgotten preference, not a
        // lost registry.
        assert!(
            migrated
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == host)
        );
    }

    /// A legacy remembered row is discarded when schema 15 replaces the
    /// per-host table with the empty helm-wide singleton.
    ///
    /// This fixture plants a fully populated legacy row and verifies that the
    /// v14 -> v15 step drops it while retaining the host registry.
    #[tokio::test]
    async fn a_version_11_remembered_rows_are_dropped_by_schema_15() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let host = {
            let store = HelmStore::open(&path).await.unwrap();
            store.add_ssh_host("v11@host", None, None).await.unwrap()
        };
        {
            // Down to the version-11 shape everywhere a later rung looks:
            // the identity column version 12 removes, the two ordering
            // columns version 13 drops (they must EXIST for the drop to
            // succeed), and the absence of what versions 13, 14, 16, and 17
            // add (`hosts.cache_truncated`, the `preferences` table,
            // `hosts.alias`, and `session_seen`).
            let conn = Connection::open(&path).expect("reopen raw");
            conn.execute_batch(
                "DROP TABLE profiles;
                 DROP TABLE remembered_profile;
                 CREATE TABLE remembered_profiles (
                     host_id       INTEGER PRIMARY KEY
                                   REFERENCES hosts (id) ON DELETE CASCADE,
                     profile_id    TEXT NOT NULL,
                     host_identity TEXT,
                     source_created_at INTEGER,
                     source_session_id TEXT,
                     source_creation_seq INTEGER,
                     CHECK ((source_created_at IS NULL) = (source_session_id IS NULL))
                 ) STRICT;
                 ALTER TABLE session_cache ADD COLUMN activity_at INTEGER;
                 ALTER TABLE session_cache ADD COLUMN title_sort TEXT;
                 ALTER TABLE hosts DROP COLUMN cache_truncated;
                 DROP TABLE session_seen;
                 ALTER TABLE hosts DROP COLUMN alias;
                 -- IF EXISTS: the preferences table only exists one stack
                 -- level up; this fixture runs at both.
                 DROP TABLE IF EXISTS preferences;
                 PRAGMA user_version = 11;",
            )
            .expect("downgrade the table");
            conn.execute(
                "INSERT INTO remembered_profiles (host_id, profile_id, host_identity, \
                 source_created_at, source_session_id, source_creation_seq) \
                 VALUES (?1, 'p-keep', 'install-x', 700, 'sess-700', 7)",
                rusqlite::params![host],
            )
            .expect("plant a version-11 row with full provenance");
        }

        let migrated = HelmStore::open(&path).await.expect("migrate");
        assert_eq!(migrated.remembered_profile().await.unwrap(), None);
        assert!(
            migrated
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == host),
            "schema 15 drops only the legacy preference, not its host registry row"
        );
        drop(migrated);
        let conn = Connection::open(&path).expect("reopen raw");
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM remembered_profile", [], |row| row
                .get::<_, i64>(0),)
                .unwrap(),
            0,
            "schema 15 intentionally starts the singleton without a legacy row"
        );
    }

    /// A legacy version-7 preference is discarded when schema 15 replaces
    /// the per-host table with the empty helm-wide singleton.
    ///
    /// The legacy host-scoped value has no comparable meaning in the new
    /// singleton, so migration discards it. The first completed
    /// profile-backed drain may establish that empty preference; a later
    /// direct create then advances it through ordinary provenance ordering.
    #[tokio::test]
    async fn version_7_remembered_default_is_dropped_by_schema_15() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let host = {
            let store = HelmStore::open(&path).await.unwrap();
            host_with_identity(&store, "v7@host", "v7-identity").await
        };
        {
            let conn = Connection::open(&path).unwrap();
            // The session cache goes back with it: a fixture stamped at
            // version 7 that still carried a later version's `archived` or
            // ordering columns would replay the ADD COLUMN over an existing
            // one and fail the open, on a state no real database reaches.
            // (Version 12 already dropped the version-11 ordering columns.)
            conn.execute_batch(
                "DROP TABLE profiles;
                 DROP TABLE remembered_profile;
                 DROP TABLE preferences;
                 DROP TABLE session_seen;
                 ALTER TABLE hosts DROP COLUMN cache_truncated;
                 ALTER TABLE hosts DROP COLUMN alias;
                 ALTER TABLE session_cache DROP COLUMN archived;
                 CREATE TABLE remembered_profiles (
                     host_id INTEGER PRIMARY KEY REFERENCES hosts(id) ON DELETE CASCADE,
                     profile_id TEXT NOT NULL,
                     host_identity TEXT
                 ) STRICT;
                 PRAGMA user_version = 7;",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO remembered_profiles (host_id, profile_id, host_identity)
                 VALUES (?1, 'legacy-profile', 'v7-identity')",
                rusqlite::params![host],
            )
            .unwrap();
        }

        let store = HelmStore::open(&path).await.unwrap();
        assert_eq!(store.remembered_profile().await.unwrap(), None);
        let replacement = store
            .replace_host_sessions(
                host,
                "v7-identity",
                vec![sequenced_profiled_session(
                    "older-surviving-source",
                    100,
                    1,
                    "older-surviving-profile",
                )],
                false,
            )
            .await
            .unwrap();
        assert!(replacement.default_changed);
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("older-surviving-profile")
        );

        assert!(
            store
                .remember_profile_default_from_host_session(
                    "established-profile",
                    host,
                    Some(2),
                    200,
                    "new-create",
                )
                .await
                .unwrap(),
            "a create observed after migration is demonstrably new"
        );
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("established-profile")
        );
    }

    /// The version-17 migration itself (SPEC.md, Status): opening a
    /// version-16 database creates `session_seen` empty and
    /// leaves everything else — the host registry here — untouched, rather
    /// than needing any data carried forward (nothing pre-16 ever recorded
    /// whether a session had been looked at, so there is nothing TO carry).
    ///
    /// Planted by removing the later additions from a current database:
    /// version 17 added the seen table, and version 18 added compactness.
    /// Both must be absent before assigning version 16, or the migration
    /// would run against a shape no released version could have created.
    #[tokio::test]
    async fn a_version_16_database_gains_the_session_seen_table() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("helm.db");
        let host = {
            let store = HelmStore::open(&path).await.expect("create");
            store.add_ssh_host("v15@host", None, None).await.unwrap()
        };
        {
            let conn = Connection::open(&path).expect("reopen raw");
            conn.execute_batch("DROP TABLE session_seen; ALTER TABLE preferences DROP COLUMN compact; PRAGMA user_version = 16;")
                .expect("downgrade to version 16");
        }

        let migrated = HelmStore::open(&path).await.expect("migrate");
        assert!(
            migrated
                .seen_activity(&["anything".to_string()])
                .await
                .unwrap()
                .is_empty(),
            "the fresh table starts with nothing marked"
        );
        assert!(
            migrated.mark_seen("s-1", 1_700_000_000).await.unwrap(),
            "the migrated table must accept writes exactly like a fresh one"
        );
        assert!(
            migrated
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == host),
            "the migration must not disturb the host registry it has nothing to do with"
        );
    }

    /// First need mints one recoverable token, and later callers read that
    /// committed value rather than replacing it with their own candidate.
    #[tokio::test]
    async fn web_token_is_recoverable_and_stable_after_first_need() {
        let (_dir, store) = fresh_store().await;
        assert_eq!(
            store
                .web_token_or_insert("first-token".to_string(), 100)
                .await
                .unwrap(),
            "first-token"
        );
        assert_eq!(
            store
                .web_token_or_insert("losing-candidate".to_string(), 200)
                .await
                .unwrap(),
            "first-token"
        );

        let conn = Arc::clone(&store.conn);
        let created_at: i64 = tokio::task::spawn_blocking(move || {
            conn.lock()
                .unwrap()
                .query_row(
                    "SELECT created_at FROM web_token WHERE singleton = 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        })
        .await
        .unwrap();
        assert_eq!(created_at, 100, "a losing mint must not rewrite history");
    }

    /// Rotation's user-visible meaning is one database commit: the new token
    /// and the absence of every old device row become visible together.
    #[tokio::test]
    async fn rotation_replaces_the_token_and_deletes_every_device_session() {
        let (_dir, store) = fresh_store().await;
        store
            .web_token_or_insert("old-token".to_string(), 100)
            .await
            .unwrap();
        store.insert_device_session([1; 32], 101).await.unwrap();
        store.insert_device_session([2; 32], 102).await.unwrap();

        assert_eq!(
            store
                .rotate_web_token("new-token".to_string(), 200)
                .await
                .unwrap(),
            "new-token"
        );
        assert!(store.device_session_hashes().await.unwrap().is_empty());
        assert_eq!(
            store
                .web_token_or_insert("unused".to_string(), 300)
                .await
                .unwrap(),
            "new-token"
        );
    }

    /// A failure deleting device rows must roll the token replacement back
    /// too; this is the transaction boundary rotation promises.
    #[tokio::test]
    async fn rotation_rolls_back_both_halves_when_device_deletion_fails() {
        let (_dir, store) = fresh_store().await;
        store
            .web_token_or_insert("old-token".to_string(), 100)
            .await
            .unwrap();
        store.insert_device_session([7; 32], 101).await.unwrap();
        let conn = Arc::clone(&store.conn);
        tokio::task::spawn_blocking(move || {
            conn.lock()
                .unwrap()
                .execute_batch(
                    "CREATE TRIGGER refuse_device_delete BEFORE DELETE ON device_sessions \
                     BEGIN SELECT RAISE(ABORT, 'scripted delete refusal'); END;",
                )
                .unwrap();
        })
        .await
        .unwrap();

        let error = store
            .rotate_web_token("must-not-commit".to_string(), 200)
            .await
            .expect_err("the trigger refuses deletion");
        assert!(format!("{error:#}").contains("scripted delete refusal"));
        assert_eq!(
            store
                .web_token_or_insert("unused".to_string(), 300)
                .await
                .unwrap(),
            "old-token",
            "the token update must roll back with the failed delete"
        );
        assert_eq!(store.device_session_hashes().await.unwrap(), vec![[7; 32]]);
    }

    /// The confidentiality repair `open` performs on top of the state
    /// directory's own 0700 boundary (see `open`'s own docs) — mirrors
    /// `farhelm-supervisor::store`'s identical
    /// `open_restricts_the_database_files_mode`. Planting the file at
    /// 0o666 BEFORE calling `open` (rather than mutating the process
    /// umask, which this project's tests must not do) is what forces the
    /// repair path to actually run: a runner whose ambient umask already
    /// narrows new files to 0600 would pass this vacuously otherwise,
    /// without ever exercising `set_permissions`.
    #[tokio::test]
    async fn open_restricts_a_pre_existing_database_files_mode() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");
        std::fs::write(&db_path, b"").expect("plant a fresh file");
        std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o666))
            .expect("widen the planted file's mode");

        let _store = HelmStore::open(&db_path).await.expect("open");
        let mode = std::fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "database file must be repaired to owner-only, got {mode:o}"
        );
    }

    /// The failure mode dropping `IF NOT EXISTS` exists to catch (see
    /// `apply_schema`'s `version == 0` branch): a version-0 database that
    /// ALREADY contains a table under one of the schema's own names —
    /// planted here directly with a raw `Connection`, standing in for a
    /// hand-edited or half-migrated file `open`'s own two racing callers
    /// could never produce — must fail `open` loudly rather than being
    /// silently accepted and stamped `user_version = 1` as though this
    /// schema had genuinely been applied to it. `BEGIN IMMEDIATE` wrapping
    /// the whole creation in one transaction is what makes the failure
    /// ATOMIC: the `hosts` table (created earlier in the same statement
    /// batch) must not survive either, so both the version and the
    /// planted table's exclusive claim on the name are checked after the
    /// failed open.
    #[tokio::test]
    async fn open_fails_atomically_on_an_incompatible_preexisting_table() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");
        {
            let conn = Connection::open(&db_path).expect("create raw db");
            conn.execute_batch("CREATE TABLE session_cache (not_the_real_shape TEXT);")
                .expect("plant an incompatible session_cache table ahead of any real open");
        }

        let err = HelmStore::open(&db_path)
            .await
            .expect_err("a pre-existing, incompatibly-shaped table must fail open");
        assert!(
            format!("{err:#}").contains("session_cache"),
            "error must name the table that could not be created: {err:#}"
        );

        let conn = Connection::open(&db_path).expect("reopen raw db to inspect the aftermath");
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            version, 0,
            "a failed schema creation must not stamp user_version"
        );
        let hosts_exists: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'hosts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            hosts_exists, 0,
            "the rolled-back transaction must not leave the hosts table \
             (created earlier in the same batch) behind either — no hosts \
             row can exist without it"
        );
    }

    // ---- The reserved local row ---------------------------------------

    /// The race `reopen_is_stable` cannot reach: two independent
    /// `HelmStore`s (genuinely separate `rusqlite::Connection`s) opened
    /// against the SAME fresh path — mirroring the supervisor store's
    /// `concurrent_first_mint_converges_on_one_identity`.
    ///
    /// `apply_schema`'s `BEGIN IMMEDIATE` transaction is what actually
    /// carries this race's correctness now: it takes SQLite's write lock
    /// before either racer ever reads `user_version`, so whichever `open`
    /// call loses genuinely BLOCKS on that lock rather than racing its own
    /// stale pre-lock read against the winner's DDL — by the time the
    /// loser's transaction proceeds, it reads back the version the winner
    /// just committed and takes the no-op path instead of attempting a
    /// second `CREATE TABLE`. `ensure_local_row`'s conditional `ON
    /// CONFLICT` (targeting the SAME partial unique index the schema
    /// transaction just created) still separately converges the local-row
    /// mint that runs AFTER `apply_schema` returns, once per `open` call
    /// rather than only on creation — see that function's own docs for why
    /// it needs its own conditional shape regardless. What this test
    /// verifies, on every run regardless of how the two opens happened to
    /// interleave: two `HelmStore`s legitimately opened against the same
    /// file both see exactly one local row.
    #[tokio::test]
    async fn concurrent_double_open_mints_exactly_one_local_row() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");

        let (store_a, store_b) = tokio::join!(HelmStore::open(&db_path), HelmStore::open(&db_path));
        let store_a = store_a.expect("open a");
        let store_b = store_b.expect("open b");

        let hosts_a = store_a.list_hosts().await.expect("list from a");
        let hosts_b = store_b.list_hosts().await.expect("list from b");
        for hosts in [&hosts_a, &hosts_b] {
            let local_rows = hosts.iter().filter(|h| h.kind == HostKind::Local).count();
            assert_eq!(
                local_rows, 1,
                "two racing opens must converge on exactly one local row"
            );
        }
    }

    /// SPEC.md: the local host is always present, never removable. Refusing
    /// its removal is what keeps that true, not merely usually true — its
    /// alias is editable (a separate, narrower exception; see
    /// `update_alias_accepts_the_local_row`), but its existence is not up
    /// for removal the way an ssh row's is.
    #[tokio::test]
    async fn remove_ssh_host_refuses_the_local_row() {
        let (_dir, store) = fresh_store().await;
        let local_id = store.list_hosts().await.unwrap()[0].id;
        let err = store
            .remove_ssh_host(local_id)
            .await
            .expect_err("removing the local row must be refused");
        assert!(
            err.downcast_ref::<HostStoreError>()
                .is_some_and(|e| matches!(e, HostStoreError::LocalHostImmutable)),
            "must refuse with LocalHostImmutable, got: {err:#}"
        );
    }

    /// The same refusal on the update path — the local host's destination
    /// is not editable, and that must hold for every ssh-host MANAGEMENT
    /// mutation, not just removal. (`add_ssh_host` cannot even
    /// express `kind = 'local'`, so it needs no equivalent test — the
    /// invariant is structural there, not merely untested. The partial
    /// unique index and this crate's lifecycle tests already own that
    /// story; a dedicated "adding ssh hosts never touches the local row
    /// count" test would only restate it.)
    #[tokio::test]
    async fn update_ssh_destination_refuses_the_local_row() {
        let (_dir, store) = fresh_store().await;
        let local_id = store.list_hosts().await.unwrap()[0].id;
        let err = store
            .update_ssh_destination(local_id, "not@allowed")
            .await
            .expect_err("updating the local row's destination must be refused");
        assert!(
            err.downcast_ref::<HostStoreError>()
                .is_some_and(|e| matches!(e, HostStoreError::LocalHostImmutable)),
            "must refuse with LocalHostImmutable, got: {err:#}"
        );
        let hosts = store.list_hosts().await.unwrap();
        assert_eq!(
            hosts.iter().find(|h| h.id == local_id).unwrap().destination,
            None,
            "the refused update must not have touched the local row"
        );
    }

    // ---- Schema invariants, pinned independent of the API ---------------
    //
    // The tests below go around HelmStore entirely, writing raw SQL
    // through the same underlying `Connection` (exactly like
    // `read_order_follows_the_extracted_columns_not_the_json` does further
    // down) specifically to prove the CHECK constraint and the partial
    // unique index bind on their own — the module docs' claim that "the
    // index remains the actual invariant enforcement any writer... cannot
    // bypass" is only true if it holds for a writer that never goes
    // through `add_ssh_host`'s conditional insert at all.

    /// The `hosts` table's row-shape `CHECK` (a local row with a
    /// destination, or an ssh row without one) must reject both malformed
    /// shapes at the SQL level, not merely go unexercised by the API's own
    /// well-formed inserts.
    #[tokio::test]
    async fn schema_check_rejects_a_malformed_row_shape() {
        let (_dir, store) = fresh_store().await;
        let conn = Arc::clone(&store.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO hosts (kind, destination) VALUES ('local', 'not-allowed')",
                [],
            )
            .expect_err("a local row with a destination must violate the CHECK constraint");
            conn.execute("INSERT INTO hosts (kind) VALUES ('ssh')", [])
                .expect_err("an ssh row without a destination must violate the CHECK constraint");
        })
        .await
        .unwrap();
    }

    /// `hosts_ssh_destination`'s uniqueness must hold even for an insert
    /// that never goes through `add_ssh_host`'s conditional
    /// `ON CONFLICT ... DO NOTHING` — the index, not the API's conditional
    /// query, is the actual invariant enforcement per the module docs.
    #[tokio::test]
    async fn schema_index_rejects_a_duplicate_destination_bypassing_the_api() {
        let (_dir, store) = fresh_store().await;
        let conn = Arc::clone(&store.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO hosts (kind, destination) VALUES ('ssh', 'raw@host')",
                [],
            )
            .expect("first raw insert");
            conn.execute(
                "INSERT INTO hosts (kind, destination) VALUES ('ssh', 'raw@host')",
                [],
            )
            .expect_err(
                "the partial unique index must reject a duplicate destination \
                 even for a raw insert the API's conditional query never sees",
            );
        })
        .await
        .unwrap();
    }

    /// `hosts_identity_claim`'s uniqueness must bind at the SQL level, not
    /// merely inside `record_first_contact`'s transaction — the module
    /// docs' claim is that at-most-one-row-per-identity is a schema
    /// invariant, which is only true if a writer that never goes through
    /// this module's API is refused too.
    #[tokio::test]
    async fn schema_index_rejects_a_duplicate_identity_bypassing_the_api() {
        let (_dir, store) = fresh_store().await;
        let conn = Arc::clone(&store.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO hosts (kind, destination, host_identity) \
                 VALUES ('ssh', 'claim-a@host', 'one-identity')",
                [],
            )
            .expect("first raw claim");
            conn.execute(
                "INSERT INTO hosts (kind, destination, host_identity) \
                 VALUES ('ssh', 'claim-b@host', 'one-identity')",
                [],
            )
            .expect_err(
                "the partial unique index must refuse a second row claiming one identity, \
                 even for a raw insert no API call ever sees",
            );
            // The partial WHERE is what keeps unclaimed rows out of the
            // index's way: any number of them may coexist.
            for destination in ["null-a@host", "null-b@host"] {
                conn.execute(
                    "INSERT INTO hosts (kind, destination) VALUES ('ssh', ?1)",
                    rusqlite::params![destination],
                )
                .expect("rows with no identity yet must not collide with each other");
            }
        })
        .await
        .unwrap();
    }

    /// [`HelmStore::list_hosts`]'s fail-loudly posture on a corrupt `kind`
    /// (see that method's own docs, and contrast
    /// `cached_sessions_skips_a_poisoned_blob_and_serves_the_rest` below,
    /// where the SAME kind of corruption in a DIFFERENT table is skipped
    /// instead). The schema's own `CHECK (kind IN ('local', 'ssh'))` means
    /// an ordinary insert can never produce this row in the first place —
    /// `PRAGMA ignore_check_constraints` disables that enforcement for
    /// this one connection so the test can plant the row `HostKind::
    /// from_column` must still refuse, standing in for a hand-edited or
    /// downgraded database file the CHECK never got a chance to guard.
    #[tokio::test]
    async fn list_hosts_fails_loudly_on_a_corrupt_kind_bypassing_the_check() {
        let (_dir, store) = fresh_store().await;
        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().unwrap();
                conn.pragma_update(None, "ignore_check_constraints", true)
                    .expect("disable CHECK enforcement for this connection");
                conn.execute(
                    "INSERT INTO hosts (kind, destination) VALUES ('bogus', NULL)",
                    [],
                )
                .expect("plant a corrupt kind bypassing the CHECK constraint");
            })
            .await
            .unwrap();
        }

        let err = store
            .list_hosts()
            .await
            .expect_err("a corrupt kind must fail list_hosts loudly, not be silently skipped");
        assert!(
            format!("{err:#}").contains("bogus"),
            "error must name the corrupt kind: {err:#}"
        );
    }

    // ---- ssh hosts ------------------------------------------------------

    /// The ordinary lifecycle: add, see it listed, update its destination,
    /// remove it, see it gone — the round trip PLAN_M6.md item 3's API
    /// surface exists to serve.
    #[tokio::test]
    async fn ssh_host_add_list_update_remove_round_trips() {
        let (_dir, store) = fresh_store().await;
        let id = store
            .add_ssh_host("user@host", Some("farhelm-remote"), Some("/remote/state"))
            .await
            .expect("add");

        let hosts = store.list_hosts().await.unwrap();
        let added = hosts
            .iter()
            .find(|h| h.id == id)
            .expect("added host listed");
        assert_eq!(added.kind, HostKind::Ssh);
        assert_eq!(added.destination.as_deref(), Some("user@host"));
        assert_eq!(added.remote_farhelm.as_deref(), Some("farhelm-remote"));
        assert_eq!(added.remote_state_dir.as_deref(), Some("/remote/state"));
        assert_eq!(added.host_identity, None);

        store
            .update_ssh_destination(id, "user@otherhost")
            .await
            .expect("update");
        let hosts = store.list_hosts().await.unwrap();
        let updated = hosts.iter().find(|h| h.id == id).expect("still listed");
        assert_eq!(updated.destination.as_deref(), Some("user@otherhost"));

        store.remove_ssh_host(id).await.expect("remove");
        let hosts = store.list_hosts().await.unwrap();
        assert!(
            hosts.iter().all(|h| h.id != id),
            "removed host must be gone"
        );
    }

    /// A duplicate destination at `add_ssh_host` time must be refused
    /// cleanly, with nothing written — the caller's retry (a different
    /// destination, or editing the existing entry) has a real error to act
    /// on instead of a generic constraint failure.
    #[tokio::test]
    async fn add_ssh_host_rejects_a_duplicate_destination() {
        let (_dir, store) = fresh_store().await;
        store
            .add_ssh_host("dup@host", None, None)
            .await
            .expect("first add");
        let err = store
            .add_ssh_host("dup@host", None, None)
            .await
            .expect_err("duplicate destination must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::DuplicateDestination(d)) if d == "dup@host"
            ),
            "must name the duplicate destination: {err:#}"
        );
        let hosts = store.list_hosts().await.unwrap();
        assert_eq!(
            hosts
                .iter()
                .filter(|h| h.destination.as_deref() == Some("dup@host"))
                .count(),
            1,
            "the rejected add must not have written a second row"
        );
    }

    /// The registry half of the ssh argv-injection fix: a destination that
    /// OpenSSH would read as an option (`-oProxyCommand=...` executes a
    /// local command) never becomes a registry row in the first place, and
    /// neither does an empty one. `crate::ssh::ssh_base_args`' terminator placement
    /// is the real guard — it protects callers that never touch this
    /// module — so what this pins is the OTHER half: the user finds out at
    /// registration time, with a typed error naming the value, instead of
    /// owning a host that fails to connect for reasons ssh explains in its
    /// own usage message.
    ///
    /// Both entry points, because an edit introduces a destination exactly
    /// as an add does.
    #[tokio::test]
    async fn option_shaped_destinations_are_refused_at_both_entry_points() {
        let (_dir, store) = fresh_store().await;
        let existing = store.add_ssh_host("real@host", None, None).await.unwrap();

        for rejected in ["-oProxyCommand=touch /tmp/pwned", "-4", ""] {
            let err = store
                .add_ssh_host(rejected, None, None)
                .await
                .expect_err("an unusable destination must be refused at add");
            assert!(
                matches!(
                    err.downcast_ref::<HostStoreError>(),
                    Some(HostStoreError::InvalidDestination(d)) if d == rejected
                ),
                "must name the rejected destination: {err:#}"
            );

            let err = store
                .update_ssh_destination(existing, rejected)
                .await
                .expect_err("an unusable destination must be refused at update");
            assert!(
                matches!(
                    err.downcast_ref::<HostStoreError>(),
                    Some(HostStoreError::InvalidDestination(d)) if d == rejected
                ),
                "must name the rejected destination: {err:#}"
            );
        }

        let hosts = store.list_hosts().await.unwrap();
        assert_eq!(
            hosts.len(),
            2,
            "no refused destination may have created a row: {hosts:?}"
        );
        assert_eq!(
            hosts
                .iter()
                .find(|h| h.id == existing)
                .unwrap()
                .destination
                .as_deref(),
            Some("real@host"),
            "no refused edit may have rewritten the existing row"
        );
    }

    /// The same duplicate check on the update path — `UPDATE OR IGNORE`
    /// against the same partial unique index `add_ssh_host` targets, rather
    /// than a second hand-written conditional query. A collision must
    /// leave the row it tried to rewrite EXACTLY as it was beforehand
    /// (re-read after the error, not merely inferred from the error type)
    /// — `UPDATE OR IGNORE` rolling back its own partial effects is the
    /// property this pins, distinct from the error being returned at all.
    #[tokio::test]
    async fn update_ssh_destination_rejects_a_collision_with_another_host() {
        let (_dir, store) = fresh_store().await;
        let a = store.add_ssh_host("a@host", None, None).await.unwrap();
        store.add_ssh_host("b@host", None, None).await.unwrap();

        let err = store
            .update_ssh_destination(a, "b@host")
            .await
            .expect_err("colliding with another host's destination must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::DuplicateDestination(d)) if d == "b@host"
            ),
            "must name the colliding destination: {err:#}"
        );
        let hosts = store.list_hosts().await.unwrap();
        assert_eq!(
            hosts
                .iter()
                .find(|h| h.id == a)
                .unwrap()
                .destination
                .as_deref(),
            Some("a@host"),
            "a rejected collision must leave the row's destination exactly as it was"
        );

        // A host keeping ITS OWN current destination must not trip the
        // unique index against itself — `UPDATE OR IGNORE` rewrites this
        // row's own column directly, so there is no self-row to collide
        // with the way a `WHERE destination = ?2 AND id != ?1` sub-select
        // would have needed to guard against.
        store
            .update_ssh_destination(a, "a@host")
            .await
            .expect("a host may re-affirm its own destination");
    }

    /// `update_ssh_destination` must preserve every OTHER column on the
    /// row it touches — `remote_farhelm`/`remote_state_dir`, the learned
    /// identity, and the cached sessions keyed off this host's stable id —
    /// pinned with all three seeded so an implementation that rewrote the
    /// whole row (rather than just the `destination` column) would be
    /// caught.
    #[tokio::test]
    async fn update_ssh_destination_preserves_everything_else() {
        let (_dir, store) = fresh_store().await;
        let host = store
            .add_ssh_host("full@host", Some("farhelm-remote"), Some("/remote/state"))
            .await
            .unwrap();
        store
            .record_first_contact(host, &dialed_as(&store, host).await, "identity-full")
            .await
            .unwrap();
        store
            .replace_host_sessions(host, "identity-full", vec![session("s1", 100)], false)
            .await
            .unwrap();

        store
            .update_ssh_destination(host, "full@otherhost")
            .await
            .expect("update");

        let hosts = store.list_hosts().await.unwrap();
        let row = hosts.iter().find(|h| h.id == host).unwrap();
        assert_eq!(row.destination.as_deref(), Some("full@otherhost"));
        assert_eq!(row.remote_farhelm.as_deref(), Some("farhelm-remote"));
        assert_eq!(row.remote_state_dir.as_deref(), Some("/remote/state"));
        assert_eq!(row.host_identity.as_deref(), Some("identity-full"));
        assert_eq!(
            store.cached_sessions(host).await.unwrap().len(),
            1,
            "a destination update must not touch the cache"
        );
    }

    /// `update_ssh_destination` against an id nothing holds must refuse
    /// with the typed [`HostStoreError::HostNotFound`], naming the exact
    /// id, rather than a bare SQL-shaped failure — the connection manager
    /// downcasts this to decide whether to retry or surface a "host
    /// vanished" error to the user. A live, unrelated host is seeded
    /// alongside the ghost id specifically so "no mutation" has something
    /// concrete to check: its row, re-read after the error, must be
    /// byte-for-byte what it was before the failed call touched anything.
    #[tokio::test]
    async fn update_ssh_destination_reports_typed_host_not_found() {
        let (_dir, store) = fresh_store().await;
        let live = store
            .add_ssh_host("live@host", Some("farhelm"), Some("/state"))
            .await
            .unwrap();
        let ghost = store.add_ssh_host("ghost@host", None, None).await.unwrap();
        store.remove_ssh_host(ghost).await.unwrap();
        let before = store.list_hosts().await.unwrap();

        let err = store
            .update_ssh_destination(ghost, "new@host")
            .await
            .expect_err("a removed host's id must not be updatable");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::HostNotFound(id)) if *id == ghost
            ),
            "must name the missing id: {err:#}"
        );

        let after = store.list_hosts().await.unwrap();
        assert_eq!(after, before, "a refused update must mutate nothing at all");
        assert!(
            after.iter().any(|h| h.id == live),
            "the live host must still be there"
        );
    }

    /// The same typed-error coverage on the remove path, with the same
    /// live-host control for "no mutation".
    #[tokio::test]
    async fn remove_ssh_host_reports_typed_host_not_found() {
        let (_dir, store) = fresh_store().await;
        let live = store.add_ssh_host("live2@host", None, None).await.unwrap();
        let ghost = store.add_ssh_host("ghost2@host", None, None).await.unwrap();
        store.remove_ssh_host(ghost).await.unwrap();
        let before = store.list_hosts().await.unwrap();

        let err = store
            .remove_ssh_host(ghost)
            .await
            .expect_err("removing an already-removed host must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::HostNotFound(id)) if *id == ghost
            ),
            "must name the missing id: {err:#}"
        );

        let after = store.list_hosts().await.unwrap();
        assert_eq!(after, before, "a refused remove must mutate nothing at all");
        assert!(
            after.iter().any(|h| h.id == live),
            "the live host must still be there"
        );
    }

    /// `HostId` is `AUTOINCREMENT` specifically so a removed id is never
    /// handed to a different, later host (see [`HostId`]'s own docs) —
    /// pinned here by removing the currently-HIGHEST id and confirming the
    /// next insert still lands strictly above it rather than recycling the
    /// gap a plain `INTEGER PRIMARY KEY` would happily reuse.
    #[tokio::test]
    async fn removed_ids_are_never_recycled() {
        let (_dir, store) = fresh_store().await;
        let first = store
            .add_ssh_host("recycle-a@host", None, None)
            .await
            .unwrap();
        let highest = store
            .add_ssh_host("recycle-b@host", None, None)
            .await
            .unwrap();
        assert!(highest > first);

        store.remove_ssh_host(highest).await.unwrap();
        let reused = store
            .add_ssh_host("recycle-b@host", None, None)
            .await
            .expect("re-adding the same destination after removal");
        assert!(
            reused > highest,
            "a fresh id must never reuse a removed one, got {reused} after removing {highest}"
        );
    }

    /// Removing a host must purge exactly its own cached sessions via the
    /// schema's `ON DELETE CASCADE` — SPEC.md's disposal rule, pinned at
    /// the store level ahead of any REST surface that will trigger it.
    #[tokio::test]
    async fn remove_ssh_host_cascades_its_session_cache() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "cascade@host", "cascade-identity").await;
        store
            .replace_host_sessions(host, "cascade-identity", vec![session("s1", 100)], false)
            .await
            .expect("seed cache");
        assert_eq!(store.cached_sessions(host).await.unwrap().len(), 1);

        store.remove_ssh_host(host).await.expect("remove");

        // Re-querying by the now-dead id must read back nothing rather
        // than erroring: if the CASCADE had NOT fired, this exact query
        // would still find the orphaned session_cache row (host_id alone
        // is enough to read it back — nothing about this read touches the
        // now-deleted hosts row), so an empty result here is what actually
        // proves the CASCADE ran, not just that removal itself succeeded.
        assert_eq!(
            store.cached_sessions(host).await.unwrap(),
            Vec::new(),
            "cache must be empty for a host id that no longer exists"
        );
    }

    // ---- Host aliases -----------------------------------------------------

    /// [`validate_alias`]'s trim-and-clear rule: surrounding whitespace never
    /// reaches storage, and an input that trims to nothing — empty or
    /// whitespace-only — is a CLEAR, the same as sending no alias at all.
    /// Pinned directly against the pure function rather than through a
    /// store round trip, the same way `the_session_filter_matches_by_the_documented_rules`
    /// pins `SessionFilter::matches` — no database needed to prove a string
    /// transform.
    #[test]
    fn validate_alias_trims_and_empty_clears() {
        assert_eq!(
            validate_alias(Some("  My Box  ")).unwrap(),
            Some("My Box".to_string())
        );
        assert_eq!(
            validate_alias(Some("")).unwrap(),
            None,
            "empty clears the alias"
        );
        assert_eq!(
            validate_alias(Some("   ")).unwrap(),
            None,
            "whitespace-only input clears too, since it trims to empty"
        );
        assert_eq!(
            validate_alias(None).unwrap(),
            None,
            "no alias in the write at all is itself a clear"
        );
    }

    /// A control character anywhere in the trimmed alias is refused outright
    /// — never silently stripped — because an alias reaches `farhelm agent`'s
    /// line-oriented stdout (known-hosts listings, refusal messages), and a
    /// smuggled newline there forges a second line the same way an
    /// unsanitized session id would (SPEC_impl.md makes the identical
    /// argument for session ids).
    #[test]
    fn validate_alias_refuses_control_characters() {
        let err = validate_alias(Some("bad\u{0007}name")).unwrap_err();
        assert!(
            matches!(&err, HostStoreError::InvalidAlias(msg) if msg == "host alias must not contain control characters"),
            "got: {err:?}"
        );
        let err = validate_alias(Some("bad\nname")).unwrap_err();
        assert!(
            matches!(&err, HostStoreError::InvalidAlias(msg) if msg == "host alias must not contain control characters"),
            "a newline is the exact byte this check exists to catch: {err:?}"
        );
    }

    /// The 64-character cap counts CHARACTERS, not bytes — pinned with a
    /// multi-byte string whose byte length exceeds 64 while its char count
    /// is exactly the limit, which only passes if the check reads
    /// `.chars().count()` rather than `.len()`.
    #[test]
    fn validate_alias_caps_at_64_characters() {
        let sixty_five = "a".repeat(65);
        let err = validate_alias(Some(&sixty_five)).unwrap_err();
        assert!(
            matches!(&err, HostStoreError::InvalidAlias(msg) if msg == "host alias must be at most 64 characters"),
            "got: {err:?}"
        );

        let sixty_four = "a".repeat(64);
        assert_eq!(
            validate_alias(Some(&sixty_four)).unwrap(),
            Some(sixty_four),
            "the cap is inclusive: exactly 64 is accepted"
        );

        let sixty_four_multibyte = "é".repeat(64);
        assert!(
            sixty_four_multibyte.len() > 64,
            "sanity: 64 two-byte characters is more than 64 BYTES"
        );
        assert_eq!(
            validate_alias(Some(&sixty_four_multibyte)).unwrap(),
            Some(sixty_four_multibyte),
            "64 CHARACTERS must be accepted regardless of byte length"
        );
    }

    /// `update_alias` reports whether the CANONICAL stored value actually
    /// changed, not whether a write happened to run — a repeat of the same
    /// trimmed value, or clearing an already-clear alias, must report
    /// `false` so callers (the REST route) know not to bump the fleet
    /// revision for a no-op. The trimmed value round-trips through
    /// `list_hosts`, closing the loop `validate_alias_trims_and_empty_clears`
    /// only proves in isolation.
    #[tokio::test]
    async fn update_alias_reports_changed_and_stores_the_trimmed_value() {
        let (_dir, store) = fresh_store().await;
        let host = store
            .add_ssh_host("alias-round-trip@host", None, None)
            .await
            .unwrap();
        let row = |rows: Vec<HostRow>| rows.into_iter().find(|h| h.id == host).unwrap();

        let changed = store
            .update_alias(host, Some("  My Box  "))
            .await
            .expect("set");
        assert!(changed, "a real change must report true");
        assert_eq!(
            row(store.list_hosts().await.unwrap()).alias.as_deref(),
            Some("My Box"),
            "the TRIMMED value is what lands in the row"
        );

        let repeat = store
            .update_alias(host, Some("  My Box  "))
            .await
            .expect("repeat");
        assert!(
            !repeat,
            "writing the same value again, even re-trimmed, must report unchanged"
        );

        let cleared = store.update_alias(host, Some("")).await.expect("clear");
        assert!(cleared, "clearing a set alias is a real change");
        assert_eq!(row(store.list_hosts().await.unwrap()).alias, None);

        let repeat_clear = store.update_alias(host, None).await.expect("repeat clear");
        assert!(
            !repeat_clear,
            "clearing an already-clear alias must report unchanged, whether the caller sends \
             an empty string or no alias at all"
        );
    }

    /// Aliasing a host onto a string another host's ALIAS already holds is
    /// refused, naming the taken host's current display name.
    #[tokio::test]
    async fn update_alias_rejects_a_collision_with_another_hosts_alias() {
        let (_dir, store) = fresh_store().await;
        let taken = store.add_ssh_host("taken@host", None, None).await.unwrap();
        store
            .update_alias(taken, Some("Shared Name"))
            .await
            .expect("seed the taken alias");
        let contender = store
            .add_ssh_host("contender@host", None, None)
            .await
            .unwrap();

        let err = store
            .update_alias(contender, Some("Shared Name"))
            .await
            .expect_err("aliasing onto another host's current alias must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::AliasTaken(name)) if name == "Shared Name"
            ),
            "must name the colliding host's display name: {err:#}"
        );
        let contender_row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|h| h.id == contender)
            .unwrap();
        assert_eq!(
            contender_row.alias, None,
            "a refused alias write must not touch the row"
        );
    }

    /// Aliasing onto another host's DERIVED name — an unaliased ssh host's
    /// raw destination, or the local row's default "this machine" — is
    /// refused the same way as colliding with an explicit alias.
    ///
    /// This is the whole reason `update_alias` computes every OTHER row's
    /// [`crate::aggregate::host_display_name`] rather than comparing the
    /// raw `alias` column: checking aliases alone would let an ssh host
    /// alias itself to another host's plain destination, or to "this
    /// machine", and either would make `farhelm agent`'s name resolution
    /// ambiguous the instant a real host happened to derive to that same
    /// string.
    #[tokio::test]
    async fn update_alias_rejects_a_collision_with_another_hosts_derived_name() {
        let (_dir, store) = fresh_store().await;
        store.add_ssh_host("plain@host", None, None).await.unwrap();
        let contender = store
            .add_ssh_host("contender2@host", None, None)
            .await
            .unwrap();

        let err = store
            .update_alias(contender, Some("plain@host"))
            .await
            .expect_err("aliasing onto another host's raw destination must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::AliasTaken(name)) if name == "plain@host"
            ),
            "got: {err:#}"
        );

        let err = store
            .update_alias(contender, Some("this machine"))
            .await
            .expect_err("aliasing onto the local row's derived name must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::AliasTaken(name)) if name == "this machine"
            ),
            "got: {err:#}"
        );
    }

    /// A host may alias itself to its own current derived name —
    /// `plans/host-aliases.md`'s decisions section settles this explicitly
    /// (the product-facing `SPEC.md` update is a later PR's work), and it
    /// falls out of the
    /// implementation for free (the collision scan is `WHERE id != ?1`, so
    /// the row being written is never compared against itself), but the
    /// behavior is worth pinning directly: renaming a host to exactly what
    /// it already displays as must not be mistaken for a collision.
    #[tokio::test]
    async fn update_alias_allows_a_host_to_alias_itself_to_its_own_derived_name() {
        let (_dir, store) = fresh_store().await;
        let host = store
            .add_ssh_host("self-alias@host", None, None)
            .await
            .unwrap();
        let changed = store
            .update_alias(host, Some("self-alias@host"))
            .await
            .expect("a host may alias itself to its own derived name");
        assert!(
            changed,
            "the alias column goes from NULL to a value, which IS a change, \
             even though the DISPLAYED name does not move"
        );
        let row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|h| h.id == host)
            .unwrap();
        assert_eq!(row.alias.as_deref(), Some("self-alias@host"));
    }

    /// The local row accepts an alias exactly like an ssh row — unlike
    /// `update_ssh_destination`, which refuses the local row outright
    /// (`update_ssh_destination_refuses_the_local_row`), `update_alias` does
    /// not special-case `HostKind::Local` at all, per
    /// `plans/host-aliases.md`'s decisions section ("the local host can be
    /// aliased too").
    #[tokio::test]
    async fn update_alias_accepts_the_local_row() {
        let (_dir, store) = fresh_store().await;
        let local_id = store.list_hosts().await.unwrap()[0].id;
        let changed = store
            .update_alias(local_id, Some("My Laptop"))
            .await
            .expect("alias the local row");
        assert!(changed);
        let row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|h| h.id == local_id)
            .unwrap();
        assert_eq!(row.alias.as_deref(), Some("My Laptop"));
    }

    /// `update_ssh_destination`'s mirror of `update_alias`'s collision
    /// check: retargeting a destination onto another host's current ALIAS
    /// must be refused the same way a destination-vs-destination collision
    /// is (`update_ssh_destination_rejects_a_collision_with_another_host`),
    /// or the display-name uniqueness invariant would only hold in one
    /// direction — a destination edit could silently steal an alias out
    /// from under the host wearing it.
    #[tokio::test]
    async fn update_ssh_destination_rejects_a_collision_with_another_hosts_alias() {
        let (_dir, store) = fresh_store().await;
        let aliased = store
            .add_ssh_host("aliased@host", None, None)
            .await
            .unwrap();
        store
            .update_alias(aliased, Some("Shared Name"))
            .await
            .expect("seed the alias");
        let mover = store.add_ssh_host("mover@host", None, None).await.unwrap();

        let err = store
            .update_ssh_destination(mover, "Shared Name")
            .await
            .expect_err("retargeting onto another host's alias must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::AliasTaken(name)) if name == "Shared Name"
            ),
            "must name the colliding alias: {err:#}"
        );
        let mover_row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|h| h.id == mover)
            .unwrap();
        assert_eq!(
            mover_row.destination.as_deref(),
            Some("mover@host"),
            "a refused retarget must not touch the row"
        );
    }

    /// A version-15 database — the schema immediately before `hosts.alias` —
    /// migrates existing hosts to a `NULL` alias, and the column is usable
    /// immediately afterward, not merely present.
    ///
    /// Downgraded from a freshly created (current-schema) store rather than
    /// replayed from `plant_v1_database`, matching this file's other
    /// "a version-N database" fixtures
    /// (`a_version_5_bare_default_is_dropped_by_schema_15` and its
    /// siblings) that anchor at the schema immediately preceding the
    /// migration under test. Removing the later seen table and compact
    /// preference restores that historical shape before reversing schema
    /// 16's alias addition.
    #[tokio::test]
    async fn a_version_15_database_migrates_hosts_to_a_null_alias() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("helm.db");
        let host = {
            let store = HelmStore::open(&path).await.expect("create");
            store.add_ssh_host("v15@host", None, None).await.unwrap()
        };
        {
            let conn = Connection::open(&path).expect("reopen raw");
            conn.execute_batch(
                "DROP TABLE session_seen;
                 ALTER TABLE preferences DROP COLUMN compact;
                 ALTER TABLE hosts DROP COLUMN alias;
                 PRAGMA user_version = 15;",
            )
            .expect("downgrade to the version-15 shape");
        }

        let migrated = HelmStore::open(&path).await.expect("migrate");
        let row = migrated
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|h| h.id == host)
            .unwrap();
        assert_eq!(
            row.alias, None,
            "an upgraded host starts with no alias on record"
        );

        let changed = migrated
            .update_alias(host, Some("Post-Migration Name"))
            .await
            .expect("the migrated column must accept a write immediately");
        assert!(changed);

        drop(migrated);
        let reopened = HelmStore::open(&path)
            .await
            .expect("reopen at the current version");
        let row = reopened
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|h| h.id == host)
            .unwrap();
        assert_eq!(
            row.alias.as_deref(),
            Some("Post-Migration Name"),
            "the alias set right after migrating must survive a reopen"
        );
    }

    /// Registering a NEW destination that matches another host's current
    /// alias is refused, naming the colliding alias, exactly like an
    /// `update_ssh_destination` retarget onto the same value would be.
    /// Without this, `POST /api/hosts` could land a row whose derived name
    /// collides with an existing alias — nothing in the unique index would
    /// catch it, since the index knows only about destinations — and
    /// `resolve_host` would then correctly refuse the now-ambiguous name,
    /// silently breaking the alias as an agent target.
    #[tokio::test]
    async fn add_ssh_host_rejects_a_destination_matching_another_hosts_alias() {
        let (_dir, store) = fresh_store().await;
        let aliased = store
            .add_ssh_host("aliased-owner@host", None, None)
            .await
            .unwrap();
        store
            .update_alias(aliased, Some("Shared Name"))
            .await
            .expect("seed the alias");

        let err = store
            .add_ssh_host("Shared Name", None, None)
            .await
            .expect_err(
                "registering a destination that collides with another host's alias must be refused",
            );
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::AliasTaken(name)) if name == "Shared Name"
            ),
            "got: {err:#}"
        );
        assert_eq!(
            store.list_hosts().await.unwrap().len(),
            2,
            "a refused registration must not have created a row"
        );
    }

    /// The same registration-time collision, through the discovery path
    /// `provisioning/service.rs` calls after a successful probe —
    /// independently, because `register_probed_ssh_host` shares no code
    /// with `add_ssh_host`; fixing one would not fix the other.
    #[tokio::test]
    async fn register_probed_ssh_host_rejects_a_destination_matching_another_hosts_alias() {
        let (_dir, store) = fresh_store().await;
        let aliased = store
            .add_ssh_host("probed-owner@host", None, None)
            .await
            .unwrap();
        store
            .update_alias(aliased, Some("Probed Name"))
            .await
            .expect("seed the alias");

        let err = store
            .register_probed_ssh_host("Probed Name", None, None, Some("identity-probed"))
            .await
            .expect_err("a freshly discovered destination colliding with another host's alias must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::AliasTaken(name)) if name == "Probed Name"
            ),
            "got: {err:#}"
        );
        assert_eq!(
            store.list_hosts().await.unwrap().len(),
            2,
            "a refused discovery-registration must not have created a row"
        );
    }

    /// `register_probed_ssh_host`'s OTHER branch — converging fields onto an
    /// ALREADY-registered destination — never runs the alias check at all,
    /// because it never writes `destination` and so cannot introduce a new
    /// collision. Pinned so a future edit that moved the check earlier
    /// (before the existing-row branch) does not start refusing ordinary
    /// reconnects to a host that happens to already collide with something
    /// (a pre-existing state this call did not create and is not
    /// responsible for policing).
    #[tokio::test]
    async fn register_probed_ssh_host_converging_an_existing_row_skips_the_alias_check() {
        let (_dir, store) = fresh_store().await;
        let existing = store
            .add_ssh_host("converge-owner@host", None, None)
            .await
            .unwrap();
        let local_id = store.list_hosts().await.unwrap()[0].id;
        // A pre-existing collision this call did not create — planted
        // directly through the raw connection, bypassing `update_alias`
        // (which would itself now refuse to create it — that is the whole
        // point of this fixup round), to stand in for a state from before
        // this write path was hardened, e.g. a hand-edited database.
        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                conn.lock().unwrap().execute(
                    "UPDATE hosts SET alias = 'converge-owner@host' WHERE id = ?1",
                    rusqlite::params![local_id],
                )
            })
            .await
            .unwrap()
            .expect("plant a pre-existing collision on the local row");
        }

        let (host, inserted) = store
            .register_probed_ssh_host("converge-owner@host", Some("farhelm-remote"), None, None)
            .await
            .expect(
                "converging fields on an already-registered destination must not be refused, \
                 even with a pre-existing collision elsewhere in the registry",
            );
        assert_eq!(
            host, existing,
            "must resolve to the SAME row, not insert a new one"
        );
        assert!(!inserted, "the row already existed");
    }

    /// `ensure_ssh_hosts` refuses a batch where one genuinely NEW entry
    /// collides with an existing alias, and the refusal rolls back the
    /// WHOLE batch — the entries before the colliding one must not have
    /// committed either, preserving the method's documented all-or-nothing
    /// guarantee.
    #[tokio::test]
    async fn ensure_ssh_hosts_rejects_a_batch_entry_matching_another_hosts_alias_atomically() {
        let (_dir, store) = fresh_store().await;
        let aliased = store
            .add_ssh_host("ensure-owner@host", None, None)
            .await
            .unwrap();
        store
            .update_alias(aliased, Some("Ensure Name"))
            .await
            .expect("seed the alias");

        let err = store
            .ensure_ssh_hosts(vec![
                EnsureHost {
                    destination: "ensure-clean@host".to_string(),
                    remote_farhelm: None,
                    remote_state_dir: None,
                },
                EnsureHost {
                    destination: "Ensure Name".to_string(),
                    remote_farhelm: None,
                    remote_state_dir: None,
                },
            ])
            .await
            .expect_err("a batch containing a colliding entry must be refused as a whole");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::AliasTaken(name)) if name == "Ensure Name"
            ),
            "got: {err:#}"
        );
        assert!(
            store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .all(|h| h.destination.as_deref() != Some("ensure-clean@host")),
            "the EARLIER entry in the same batch must not have committed either — the whole \
             transaction rolls back on the later entry's refusal"
        );
    }

    /// An entry `ensure_ssh_hosts` finds ALREADY registered is left alone
    /// even if its destination happens to equal another host's alias — a
    /// pre-existing state the additive, startup-time call did not create
    /// and must not fail an otherwise ordinary boot over. Only entries this
    /// call would actually INSERT are checked.
    #[tokio::test]
    async fn ensure_ssh_hosts_does_not_refuse_an_already_registered_entry_over_a_pre_existing_collision()
     {
        let (_dir, store) = fresh_store().await;
        // Both destinations registered while nothing is aliased, so
        // neither registration collides with anything yet.
        let owner = store
            .add_ssh_host("pre-existing-owner@host", None, None)
            .await
            .unwrap();
        store
            .add_ssh_host("Pre Existing Name", None, None)
            .await
            .expect("the plain destination index does not itself forbid a second host");
        // The collision is planted directly through the raw connection,
        // bypassing `update_alias` (which would itself now refuse to
        // create it — that is the whole point of this fixup round), to
        // stand in for a state that predates this write path being
        // hardened: a hand-edited database, or one aliased before this
        // check existed.
        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                conn.lock().unwrap().execute(
                    "UPDATE hosts SET alias = 'Pre Existing Name' WHERE id = ?1",
                    rusqlite::params![owner],
                )
            })
            .await
            .unwrap()
            .expect("plant the pre-existing collision");
        }

        let added = store
            .ensure_ssh_hosts(vec![EnsureHost {
                destination: "Pre Existing Name".to_string(),
                remote_farhelm: None,
                remote_state_dir: None,
            }])
            .await
            .expect("an already-registered entry must not be refused over a collision it did not create");
        assert!(added.is_empty(), "nothing new was inserted: {added:?}");
    }

    /// Clearing an alias is checked too: the row's RESTORED derived name
    /// (its own destination, once the alias is gone) must not collide with
    /// another host's current alias. Reachable sequence this closes: host A
    /// has destination `builder` and alias `workstation`; host B takes
    /// alias `builder` while A displays as `workstation`; clearing A's
    /// alias would otherwise silently restore the ambiguous `builder` name
    /// on two hosts at once.
    #[tokio::test]
    async fn update_alias_clearing_rejects_a_collision_with_another_hosts_alias() {
        let (_dir, store) = fresh_store().await;
        let a = store.add_ssh_host("builder", None, None).await.unwrap();
        store
            .update_alias(a, Some("workstation"))
            .await
            .expect("alias A so its raw destination is free to be taken");
        let b = store.add_ssh_host("b-host", None, None).await.unwrap();
        store
            .update_alias(b, Some("builder"))
            .await
            .expect("B takes A's now-unused derived name as its own alias");

        let err = store
            .update_alias(a, None)
            .await
            .expect_err("clearing A's alias would restore a name B is currently displaying as");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::AliasTaken(name)) if name == "builder"
            ),
            "got: {err:#}"
        );
        let row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|h| h.id == a)
            .unwrap();
        assert_eq!(
            row.alias.as_deref(),
            Some("workstation"),
            "a refused clear must leave the alias exactly as it was"
        );
    }

    /// The same clear-time collision for the LOCAL row: its derived name is
    /// always "this machine" rather than a destination, but the check is
    /// identical — restoring it must not collide with another host's
    /// current alias either.
    #[tokio::test]
    async fn update_alias_clearing_the_local_row_rejects_a_collision_with_another_hosts_alias() {
        let (_dir, store) = fresh_store().await;
        let local_id = store.list_hosts().await.unwrap()[0].id;
        store
            .update_alias(local_id, Some("workstation"))
            .await
            .expect("alias the local row so its derived name is free to be taken");
        let other = store.add_ssh_host("other@host", None, None).await.unwrap();
        store
            .update_alias(other, Some("this machine"))
            .await
            .expect("another host takes the local row's now-unused derived name");

        let err = store.update_alias(local_id, None).await.expect_err(
            "clearing the local row's alias would restore a name another host displays as",
        );
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::AliasTaken(name)) if name == "this machine"
            ),
            "got: {err:#}"
        );
    }

    /// A registry row this build cannot decode (an unrecognized `kind`,
    /// reachable only by bypassing the API — see the "Schema invariants"
    /// tests above) is a reason to REFUSE an alias write, not to silently
    /// drop that row from the collision scan. `list_hosts` already fails
    /// the whole registry read on the identical corruption; an alias
    /// committed against a registry this function could not fully
    /// interpret would leave the later manager sync to fail on a durable
    /// change nothing could roll back.
    #[tokio::test]
    async fn update_alias_refuses_when_a_comparison_row_has_a_corrupt_kind() {
        let (_dir, store) = fresh_store().await;
        let host = store
            .add_ssh_host("corrupt-scan@host", None, None)
            .await
            .unwrap();
        {
            // The same bypass `list_hosts_fails_loudly_on_a_corrupt_kind_bypassing_the_check`
            // uses: the schema's column-level `CHECK (kind IN ('local',
            // 'ssh'))` refuses this through any real writer (including a
            // raw `INSERT`), so reaching the row this fixture needs means
            // disabling CHECK enforcement on this one connection first —
            // standing in for a hand-edited or pre-CHECK database file.
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().unwrap();
                conn.pragma_update(None, "ignore_check_constraints", true)
                    .expect("disable CHECK enforcement for this connection");
                conn.execute(
                    "INSERT INTO hosts (kind, destination) VALUES ('bogus', NULL)",
                    [],
                )
                .expect("plant a corrupt kind bypassing the CHECK constraint")
            })
            .await
            .unwrap();
        }

        let err = store
            .update_alias(host, Some("New Name"))
            .await
            .expect_err("an alias write must refuse rather than silently skip an undecodable row");
        assert!(
            format!("{err:#}").contains("bogus"),
            "error must name the corrupt kind, matching list_hosts's own error: {err:#}"
        );
        // `list_hosts` fails the SAME way on the SAME corruption
        // (`list_hosts_fails_loudly_on_a_corrupt_kind_bypassing_the_check`)
        // — checked here too, so a future edit could not "fix" this
        // function by reading through a helper that quietly tolerates what
        // the rest of the module refuses to.
        assert!(
            store.list_hosts().await.is_err(),
            "list_hosts must fail on this fixture as well, not just update_alias"
        );
        // Read the alias column directly, bypassing the broken decode
        // path entirely, since `list_hosts` cannot serve this host's row
        // in isolation once ANY row in the registry is corrupt.
        let conn = Arc::clone(&store.conn);
        let alias: Option<String> = tokio::task::spawn_blocking(move || {
            conn.lock().unwrap().query_row(
                "SELECT alias FROM hosts WHERE id = ?1",
                rusqlite::params![host],
                |row| row.get(0),
            )
        })
        .await
        .unwrap()
        .expect("read back the alias column directly");
        assert_eq!(
            alias, None,
            "the refused write must not have changed this host's alias"
        );
    }

    // ---- Identity -----------------------------------------------------

    /// First contact on a host whose `host_identity` is still `NULL` must
    /// record it and preserve even a PRE-IDENTITY cache — planted here via
    /// raw SQL (the identity-bound `replace_host_sessions` cannot itself
    /// populate a cache ahead of any recorded identity, so a raw insert is
    /// the only honest way to construct this precondition; the same
    /// bypass technique `schema_index_rejects_a_duplicate_destination_...`
    /// above and `read_order_follows_the_extracted_columns_not_the_json`
    /// below both use). `record_first_contact` must never purge — only
    /// [`HelmStore::adopt_identity`] does that, and only for a genuine
    /// identity CHANGE.
    #[tokio::test]
    async fn record_first_contact_preserves_a_pre_identity_cache() {
        let (_dir, store) = fresh_store().await;
        let host = store.add_ssh_host("id@host", None, None).await.unwrap();
        {
            let conn = Arc::clone(&store.conn);
            let json = serde_json::to_string(&session("s1", 100)).unwrap();
            tokio::task::spawn_blocking(move || {
                conn.lock()
                    .unwrap()
                    .execute(
                        "INSERT INTO session_cache (host_id, session_id, created_at, info_json) \
                         VALUES (?1, 's1', 100, ?2)",
                        rusqlite::params![host, json],
                    )
                    .unwrap();
            })
            .await
            .unwrap();
        }

        let outcome = store
            .record_first_contact(host, &dialed_as(&store, host).await, "identity-a")
            .await
            .expect("record");
        assert_eq!(outcome, FirstContactOutcome::Recorded);

        let hosts = store.list_hosts().await.unwrap();
        let row = hosts.iter().find(|h| h.id == host).unwrap();
        assert_eq!(row.host_identity.as_deref(), Some("identity-a"));
        assert_eq!(
            store.cached_sessions(host).await.unwrap().len(),
            1,
            "first contact must preserve even a cache that predates any recorded identity"
        );
    }

    /// A repeat hello reporting the SAME identity is idempotent: still
    /// `Recorded`, and — since nothing actually changed — the cache is
    /// left untouched. Distinct from the previous test: this one exercises
    /// the branch where `host_identity` is already `Some` and matches,
    /// not `NULL`.
    #[tokio::test]
    async fn record_first_contact_is_idempotent_for_a_matching_identity() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "stable@host", "identity-x").await;
        store
            .replace_host_sessions(host, "identity-x", vec![session("s1", 100)], false)
            .await
            .unwrap();

        let outcome = store
            .record_first_contact(host, &dialed_as(&store, host).await, "identity-x")
            .await
            .expect("re-recording the same identity is not an error");
        assert_eq!(outcome, FirstContactOutcome::Recorded);

        assert_eq!(
            store.cached_sessions(host).await.unwrap().len(),
            1,
            "an unchanged identity must not purge the cache"
        );
    }

    /// The out-of-order case A1 exists to make structurally impossible: a
    /// hello reporting a DIFFERENT identity than what is already on record
    /// must change NOTHING — no write to `host_identity`, no cache purge —
    /// and instead hand back [`FirstContactOutcome::Mismatch`] so the
    /// caller can surface SPEC.md's adopt-or-fix-destination choice. Only
    /// [`HelmStore::adopt_identity`], called with the user's explicit
    /// go-ahead, may ever perform that write.
    #[tokio::test]
    async fn record_first_contact_leaves_a_conflicting_identity_unchanged() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "mismatch@host", "identity-old").await;
        store
            .replace_host_sessions(host, "identity-old", vec![session("s1", 100)], false)
            .await
            .unwrap();

        let outcome = store
            .record_first_contact(host, &dialed_as(&store, host).await, "identity-new")
            .await
            .expect("a conflicting identity is a value, not an error");
        assert_eq!(
            outcome,
            FirstContactOutcome::Mismatch {
                recorded: "identity-old".to_string(),
                reported: "identity-new".to_string(),
            }
        );

        let hosts = store.list_hosts().await.unwrap();
        assert_eq!(
            hosts
                .iter()
                .find(|h| h.id == host)
                .unwrap()
                .host_identity
                .as_deref(),
            Some("identity-old"),
            "a mismatched first contact must not overwrite the recorded identity"
        );
        assert_eq!(
            store.cached_sessions(host).await.unwrap().len(),
            1,
            "a mismatched first contact must not purge the cache either — only an \
             explicit adopt_identity may do that"
        );
    }

    /// The race the `hosts_identity_claim` index exists for, run with two
    /// genuinely independent `Connection`s (as two helm processes, or two
    /// actors on two blocking-pool threads, would be): two registry entries
    /// reach the SAME fresh identity at the same moment.
    ///
    /// The outcome must be decided rather than interleaving-dependent —
    /// exactly one `Recorded` and one `Collision` naming the winner — which
    /// is what the check-then-write shape could not promise. With that
    /// shape both callers could see no claimant and both write, and the
    /// database would then hold the very state that makes BOTH entries
    /// freeze as duplicates at the next start, hiding a live host
    /// completely.
    #[tokio::test]
    async fn concurrent_first_contact_on_one_identity_has_exactly_one_winner() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");
        let store_a = HelmStore::open(&db_path).await.expect("open a");
        let store_b = HelmStore::open(&db_path).await.expect("open b");
        let host_a = store_a
            .add_ssh_host("race-a@host", None, None)
            .await
            .unwrap();
        let host_b = store_a
            .add_ssh_host("race-b@host", None, None)
            .await
            .unwrap();
        let config_a = dialed_as(&store_a, host_a).await;
        let config_b = dialed_as(&store_a, host_b).await;

        let (a, b) = tokio::join!(
            store_a.record_first_contact(host_a, &config_a, "contested"),
            store_b.record_first_contact(host_b, &config_b, "contested"),
        );
        let outcomes = [
            a.expect("no storage failure"),
            b.expect("no storage failure"),
        ];

        let recorded = outcomes
            .iter()
            .filter(|o| **o == FirstContactOutcome::Recorded)
            .count();
        assert_eq!(recorded, 1, "exactly one racer may record: {outcomes:?}");
        let winner = if outcomes[0] == FirstContactOutcome::Recorded {
            host_a
        } else {
            host_b
        };
        assert!(
            outcomes.contains(&FirstContactOutcome::Collision { owner: winner }),
            "the loser must be told who won, not merely refused: {outcomes:?}"
        );
        let claimants = store_a
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .filter(|row| row.host_identity.as_deref() == Some("contested"))
            .count();
        assert_eq!(claimants, 1, "the database must hold exactly one claim");
    }

    /// The adopt half of the same race: a rival first-contacts the identity
    /// the user is being asked to adopt, in the window between the mismatch
    /// being displayed and the adopt arriving. The compare-and-swap alone
    /// would happily proceed — `expected_old` is still accurate — so the
    /// claim check inside the same transaction is what refuses it, leaving
    /// the user with a duplicate to resolve instead of two rows claiming
    /// one host.
    #[tokio::test]
    async fn adopt_identity_refuses_an_identity_a_rival_already_claimed() {
        let (_dir, store) = fresh_store().await;
        let mismatched = host_with_identity(&store, "mismatched@host", "identity-old").await;
        store
            .replace_host_sessions(
                mismatched,
                "identity-old",
                vec![session("kept", 100)],
                false,
            )
            .await
            .unwrap();
        let rival = host_with_identity(&store, "rival@host", "identity-reinstalled").await;

        let err = store
            .adopt_identity(
                mismatched,
                &dialed_as(&store, mismatched).await,
                "identity-old",
                "identity-reinstalled",
            )
            .await
            .expect_err("adopting a claimed identity must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::IdentityClaimed { host, identity, owner })
                    if *host == mismatched && identity == "identity-reinstalled" && *owner == rival
            ),
            "must name the rival that holds the claim: {err:#}"
        );
        assert_eq!(
            recorded_identity(&store, mismatched).await.as_deref(),
            Some("identity-old"),
            "a refused adoption must leave the identity exactly as it was"
        );
        assert_eq!(
            store.cached_sessions(mismatched).await.unwrap().len(),
            1,
            "a refused adoption must not purge the cache either"
        );
    }

    /// The window cluster 6 closes: a hello that was in flight while the
    /// user retargeted the row would otherwise commit the OLD endpoint's
    /// identity under the NEW destination — a durable lie about which
    /// machine this entry is. The write is refused instead, and the outcome
    /// says what the row says now so the log can show what outran it.
    ///
    /// Both identity writers are pinned, because they carry the same risk
    /// from opposite directions: first contact learns an identity, adoption
    /// accepts one, and neither is meaningful once the row points somewhere
    /// else.
    #[tokio::test]
    async fn identity_writes_are_refused_after_the_row_is_retargeted() {
        let (_dir, store) = fresh_store().await;
        let host = store.add_ssh_host("dialed@host", None, None).await.unwrap();
        let dialed = dialed_as(&store, host).await;
        store
            .update_ssh_destination(host, "retargeted@host")
            .await
            .expect("the user edits the destination mid-handshake");

        let outcome = store
            .record_first_contact(host, &dialed, "identity-from-the-old-endpoint")
            .await
            .expect("staleness is a value, not a storage failure");
        assert!(
            matches!(
                &outcome,
                FirstContactOutcome::StaleAttempt { current }
                    if current.destination.as_deref() == Some("retargeted@host")
            ),
            "must report what the row says now: {outcome:?}"
        );
        assert_eq!(
            recorded_identity(&store, host).await,
            None,
            "the stale attempt must not have claimed anything"
        );

        // The same row, now genuinely dialed as it stands, records fine —
        // the guard is about staleness, not about refusing this row.
        let fresh = dialed_as(&store, host).await;
        assert_eq!(
            store
                .record_first_contact(host, &fresh, "identity-from-the-new-endpoint")
                .await
                .unwrap(),
            FirstContactOutcome::Recorded
        );

        // And adoption inherits the guard, against a configuration change
        // that does not touch `destination` at all.
        let stale_for_adopt = dialed_as(&store, host).await;
        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                conn.lock()
                    .unwrap()
                    .execute(
                        "UPDATE hosts SET remote_state_dir = '/elsewhere' WHERE id = ?1",
                        rusqlite::params![host],
                    )
                    .unwrap();
            })
            .await
            .unwrap();
        }
        let err = store
            .adopt_identity(
                host,
                &stale_for_adopt,
                "identity-from-the-new-endpoint",
                "identity-from-somewhere-else",
            )
            .await
            .expect_err("adoption under a superseded configuration must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::StaleAttempt { host: h }) if *h == host
            ),
            "must be the typed staleness refusal: {err:#}"
        );
    }

    /// The ADOPT path: a successful compare-and-swap purges exactly that
    /// host's cache, leaving every other host's cache untouched — pinned
    /// with a second host present specifically so a buggy unconditional
    /// `DELETE FROM session_cache` (no `WHERE host_id`) would be caught
    /// rather than accidentally passing.
    #[tokio::test]
    async fn adopt_identity_purges_only_that_hosts_cache() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "adopt@host", "identity-old").await;
        let other = host_with_identity(&store, "other@host", "other-identity").await;
        store
            .replace_host_sessions(host, "identity-old", vec![session("s1", 100)], false)
            .await
            .unwrap();
        store
            .replace_host_sessions(other, "other-identity", vec![session("s2", 200)], false)
            .await
            .unwrap();

        store
            .adopt_identity(
                host,
                &dialed_as(&store, host).await,
                "identity-old",
                "identity-new",
            )
            .await
            .expect("adopt a new identity");

        assert_eq!(
            store.cached_sessions(host).await.unwrap(),
            Vec::new(),
            "the adopted host's cache must be purged"
        );
        assert_eq!(
            store.cached_sessions(other).await.unwrap().len(),
            1,
            "an unrelated host's cache must survive another host's adoption"
        );
        let hosts = store.list_hosts().await.unwrap();
        assert_eq!(
            hosts
                .iter()
                .find(|h| h.id == host)
                .unwrap()
                .host_identity
                .as_deref(),
            Some("identity-new")
        );
    }

    /// The compare-and-swap half of A1: `adopt_identity` with a STALE
    /// `expected_old` (one that no longer names the currently stored
    /// value) must change nothing at all — same discipline as
    /// `record_first_contact`'s mismatch case, but as a typed error here
    /// rather than a value, since a stale CAS from the connection manager's
    /// point of view genuinely IS a failure to act on, not an expected
    /// steady state.
    #[tokio::test]
    async fn adopt_identity_rejects_a_stale_expected_old() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "stale-adopt@host", "identity-x").await;
        store
            .replace_host_sessions(host, "identity-x", vec![session("s1", 100)], false)
            .await
            .unwrap();

        let err = store
            .adopt_identity(
                host,
                &dialed_as(&store, host).await,
                "wrong-expected-old",
                "identity-y",
            )
            .await
            .expect_err("a stale expected_old must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::IdentityMismatch { host: h, expected, actual })
                    if *h == host && expected == "wrong-expected-old"
                        && actual.as_deref() == Some("identity-x")
            ),
            "must name both the wrong expectation and the actual value: {err:#}"
        );

        let hosts = store.list_hosts().await.unwrap();
        assert_eq!(
            hosts
                .iter()
                .find(|h| h.id == host)
                .unwrap()
                .host_identity
                .as_deref(),
            Some("identity-x"),
            "a rejected CAS must not overwrite the recorded identity"
        );
        assert_eq!(
            store.cached_sessions(host).await.unwrap().len(),
            1,
            "a rejected CAS must not purge the cache either"
        );
    }

    /// Typed [`HostStoreError::HostNotFound`] coverage for both identity
    /// operations against an id nothing currently holds — the same
    /// discipline the ssh-host management methods pin above, extended to
    /// the identity surface A1 introduces. A live host with its own
    /// established identity and cache is seeded alongside the ghost id so
    /// "no mutation" means something: both failed calls target the ghost
    /// only, and the live host's identity/cache must come through
    /// untouched.
    #[tokio::test]
    async fn identity_operations_report_typed_host_not_found() {
        let (_dir, store) = fresh_store().await;
        let live = host_with_identity(&store, "live3@host", "live-identity").await;
        store
            .replace_host_sessions(live, "live-identity", vec![session("s1", 100)], false)
            .await
            .unwrap();
        let ghost = store.add_ssh_host("ghost3@host", None, None).await.unwrap();
        // Captured while the row still exists, exactly as a real caller
        // would have captured it before dialing — so the refusal below is
        // "this host is gone", never "this attempt is stale".
        let ghost_config = dialed_as(&store, ghost).await;
        store.remove_ssh_host(ghost).await.unwrap();

        let err = store
            .record_first_contact(ghost, &ghost_config, "identity-z")
            .await
            .expect_err("first contact against a removed host must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::HostNotFound(id)) if *id == ghost
            ),
            "must name the missing id: {err:#}"
        );

        let err = store
            .adopt_identity(ghost, &ghost_config, "anything", "identity-z")
            .await
            .expect_err("adoption against a removed host must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::HostNotFound(id)) if *id == ghost
            ),
            "must name the missing id: {err:#}"
        );

        let hosts = store.list_hosts().await.unwrap();
        assert_eq!(
            hosts
                .iter()
                .find(|h| h.id == live)
                .unwrap()
                .host_identity
                .as_deref(),
            Some("live-identity"),
            "an unrelated host's identity must survive both refused calls"
        );
        assert_eq!(
            store.cached_sessions(live).await.unwrap().len(),
            1,
            "an unrelated host's cache must survive both refused calls"
        );
    }

    /// Adoption's atomicity is not merely asserted by its docs — this
    /// forces the cache-purge half of the transaction to fail via a
    /// TEMPORARY trigger that `RAISE(ABORT)`s on `DELETE FROM
    /// session_cache`, then confirms the WHOLE transaction rolled back:
    /// both the identity write AND the (attempted, aborted) purge must be
    /// undone together, leaving the OLD identity and the OLD cache intact.
    /// A `TEMP TRIGGER` lives on the connection, not the database file, so
    /// it only ever affects this one test's `HelmStore` — no seam had to
    /// be added to `adopt_identity` itself to make the failure
    /// injectable.
    #[tokio::test]
    async fn adopt_identity_rolls_back_wholly_if_the_purge_fails() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "atomic@host", "identity-before").await;
        store
            .replace_host_sessions(host, "identity-before", vec![session("s1", 100)], false)
            .await
            .unwrap();

        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                conn.lock()
                    .unwrap()
                    .execute_batch(
                        "CREATE TEMP TRIGGER abort_cache_purge
                             BEFORE DELETE ON session_cache
                         BEGIN
                             SELECT RAISE(ABORT, 'forced failure for adopt_identity_rolls_back_wholly_if_the_purge_fails');
                         END;",
                    )
                    .unwrap();
            })
            .await
            .unwrap();
        }

        store
            .adopt_identity(
                host,
                &dialed_as(&store, host).await,
                "identity-before",
                "identity-after",
            )
            .await
            .expect_err("the forced trigger failure must fail the whole adoption");

        let hosts = store.list_hosts().await.unwrap();
        assert_eq!(
            hosts
                .iter()
                .find(|h| h.id == host)
                .unwrap()
                .host_identity
                .as_deref(),
            Some("identity-before"),
            "a rolled-back adoption must leave the OLD identity in place, not the new one"
        );
        assert_eq!(
            store.cached_sessions(host).await.unwrap().len(),
            1,
            "a rolled-back adoption must leave the OLD cache intact, not partially purged"
        );
    }

    // ---- Cache ----------------------------------------------------------

    /// The transactional-replace contract: a batch that fails PARTWAY
    /// THROUGH must leave the pre-existing cache exactly as it was, not
    /// half-cleared. The failure is a genuine primary-key collision (two
    /// entries sharing a `session_id` within one `entries` batch) rather
    /// than an injected fault — see `replace_host_sessions`'s own docs for
    /// why that is the more honest seam available here.
    #[tokio::test]
    async fn replace_host_sessions_rolls_back_a_mid_batch_failure() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "rollback@host", "rollback-identity").await;
        store
            .replace_host_sessions(
                host,
                "rollback-identity",
                vec![session("old-1", 100), session("old-2", 200)],
                false,
            )
            .await
            .expect("seed the cache");

        let poisoned = vec![
            session("new-1", 300),
            session("dup", 400),
            session("dup", 500), // collides with the row just inserted above
        ];
        store
            .replace_host_sessions(host, "rollback-identity", poisoned, false)
            .await
            .expect_err("a duplicate id within one batch must fail the whole replace");

        let mut ids: Vec<String> = store
            .cached_sessions(host)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        ids.sort();
        assert_eq!(
            ids,
            vec!["old-1".to_string(), "old-2".to_string()],
            "a failed replace must leave the PRE-EXISTING cache untouched, not partially cleared"
        );
    }

    /// `replace_host_sessions` against an id nothing holds must refuse with
    /// [`HostStoreError::HostNotFound`] for BOTH an empty and a non-empty
    /// `entries` batch — before A2 an EMPTY batch against a gone host
    /// succeeded silently (delete zero rows, insert zero rows, no error),
    /// indistinguishable from "this host truly has zero sessions." Both
    /// shapes are pinned here so neither regresses independently.
    #[tokio::test]
    async fn replace_host_sessions_reports_typed_host_not_found() {
        let (_dir, store) = fresh_store().await;
        let ghost = store.add_ssh_host("ghost4@host", None, None).await.unwrap();
        store.remove_ssh_host(ghost).await.unwrap();

        for (label, entries) in [("empty", vec![]), ("non-empty", vec![session("s1", 100)])] {
            let err = store
                .replace_host_sessions(ghost, "any-identity", entries, false)
                .await
                .unwrap_err();
            assert!(
                matches!(
                    err.downcast_ref::<HostStoreError>(),
                    Some(HostStoreError::HostNotFound(id)) if *id == ghost
                ),
                "{label} replace against a missing host must name it: {err:#}"
            );
        }
    }

    /// The identity-binding half of A2: a refresh carrying a STALE identity
    /// — one that no longer matches what is currently stored, because an
    /// adoption landed first — must be refused with the cache left exactly
    /// as the adoption left it. This is the scenario A2 exists to close: a
    /// session-list fetch that was in flight under the OLD identity landing
    /// AFTER a user adopts a new one must not repopulate the cache the
    /// adoption just purged with sessions belonging to the dead install.
    #[tokio::test]
    async fn replace_host_sessions_rejects_a_stale_identity() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "stale-refresh@host", "identity-before").await;
        store
            .replace_host_sessions(
                host,
                "identity-before",
                vec![session("pre-adopt", 100)],
                false,
            )
            .await
            .unwrap();
        store
            .adopt_identity(
                host,
                &dialed_as(&store, host).await,
                "identity-before",
                "identity-after",
            )
            .await
            .expect("adopt");
        store
            .replace_host_sessions(
                host,
                "identity-after",
                vec![session("post-adopt", 200)],
                false,
            )
            .await
            .expect("a fresh refresh under the NEW identity must succeed");

        // The delayed refresh: still carrying the identity the connection
        // observed BEFORE the adoption, arriving after the fact.
        let err = store
            .replace_host_sessions(
                host,
                "identity-before",
                vec![session("delayed", 300)],
                false,
            )
            .await
            .expect_err("a refresh under a superseded identity must be refused");
        assert!(
            matches!(
                err.downcast_ref::<HostStoreError>(),
                Some(HostStoreError::IdentityMismatch { host: h, expected, actual })
                    if *h == host && expected == "identity-before"
                        && actual.as_deref() == Some("identity-after")
            ),
            "must name both the stale identity and the current one: {err:#}"
        );

        let cached: Vec<String> = store
            .cached_sessions(host)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(
            cached,
            vec!["post-adopt".to_string()],
            "the rejected stale refresh must not have touched the cache the \
             post-adoption refresh legitimately populated"
        );
    }

    /// Ordering must come from the stored COLUMNS, never from parsing
    /// `info_json` — pinned by tampering the `created_at` COLUMN directly
    /// (through a raw connection, bypassing `replace_host_sessions`)
    /// without touching the JSON blob at all, then asserting the read
    /// order follows the tampered column. A reader that re-derived order
    /// from the JSON would report the ORIGINAL order and fail this
    /// assertion.
    #[tokio::test]
    async fn read_order_follows_the_extracted_columns_not_the_json() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "order@host", "order-identity").await;
        store
            .replace_host_sessions(
                host,
                "order-identity",
                vec![session("first", 100), session("second", 200)],
                false,
            )
            .await
            .unwrap();
        // "first" naturally sorts after "second" in wire order (created_at
        // descending) at this point. Flip it by rewriting ONLY the
        // created_at COLUMN for "first" to something newer, leaving its
        // info_json (whose embedded created_at still says 100) untouched.
        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                conn.lock()
                    .unwrap()
                    .execute(
                        "UPDATE session_cache SET created_at = 999 \
                         WHERE session_id = 'first'",
                        [],
                    )
                    .unwrap();
            })
            .await
            .unwrap();
        }

        let ordered = store.cached_sessions(host).await.unwrap();
        let ids: Vec<&str> = ordered.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["first", "second"],
            "read order must follow the tampered COLUMN, proving it is never re-derived \
             from the (untouched) JSON blob"
        );
    }

    /// A session reappearing in a later refresh under the SAME id replaces
    /// its older cached self entirely — the wholesale-replacement contract
    /// applied to one overlapping row rather than the whole set.
    #[tokio::test]
    async fn a_later_refresh_replaces_an_overlapping_sessions_older_self() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "overlap@host", "overlap-identity").await;
        store
            .replace_host_sessions(host, "overlap-identity", vec![session("s1", 100)], false)
            .await
            .unwrap();

        let mut newer = session("s1", 100);
        newer.title = "renamed".to_string();
        store
            .replace_host_sessions(host, "overlap-identity", vec![newer], false)
            .await
            .unwrap();

        let cached = store.cached_sessions(host).await.unwrap();
        assert_eq!(cached.len(), 1, "the overlapping id must not duplicate");
        assert_eq!(cached[0].title, "renamed", "the newer copy must win");
    }

    /// The wholesale-replacement contract stress-tested across a SHRINKING
    /// sequence — several rows, then a smaller non-empty set, then an
    /// EMPTY set — asserting exact contents after each step. An
    /// upsert-only implementation (one that inserts/updates named entries
    /// but never deletes rows absent from `entries`) would pass a naive
    /// "does the new content show up" check while silently leaving stale
    /// rows behind; only the empty-set step at the end actually catches
    /// that, since a shrink-to-empty has no new rows to point an upsert-only
    /// diff at all.
    #[tokio::test]
    async fn wholesale_replacement_handles_shrinking_to_empty() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "shrink@host", "shrink-identity").await;

        store
            .replace_host_sessions(
                host,
                "shrink-identity",
                vec![session("a", 100), session("b", 200), session("c", 300)],
                false,
            )
            .await
            .unwrap();
        let mut ids = cached_ids(&store, host).await;
        ids.sort();
        assert_eq!(
            ids,
            vec!["a", "b", "c"],
            "all three seeded rows must be present"
        );

        store
            .replace_host_sessions(host, "shrink-identity", vec![session("b", 200)], false)
            .await
            .unwrap();
        assert_eq!(
            cached_ids(&store, host).await,
            vec!["b"],
            "shrinking to one entry must drop the other two, not just add/update \"b\""
        );

        store
            .replace_host_sessions(host, "shrink-identity", vec![], false)
            .await
            .unwrap();
        assert_eq!(
            cached_ids(&store, host).await,
            Vec::<String>::new(),
            "an empty replacement must leave the cache genuinely empty, not \
             still holding \"b\" from before"
        );
    }

    /// Two sessions sharing the same `created_at` (a real possibility —
    /// nothing prevents two agents starting in the same wall-clock second)
    /// must tie-break on `session_id` ASCENDING — pinned by inserting them
    /// in REVERSE id order so an implementation that accidentally preserved
    /// insertion order, instead of sorting on the id column, would be
    /// caught.
    ///
    /// Scoped to the per-host read: cross-host merge order is
    /// `crate::aggregate`'s concern now (the list is sorted in memory from
    /// whatever [`HelmStore::cached_rows`] hands back), so it has its own
    /// coverage there rather than pinned a second time against this store.
    #[tokio::test]
    async fn equal_created_at_ties_break_ascending_by_session_id() {
        let (_dir, store) = fresh_store().await;
        let a = host_with_identity(&store, "tie-a@host", "tie-a-identity").await;

        // Reverse-id insertion order: if the read path fell back to
        // insertion/rowid order instead of sorting by session_id, this
        // would surface as "c", "b", "a" instead of the expected ascending
        // order.
        store
            .replace_host_sessions(
                a,
                "tie-a-identity",
                vec![session("c", 100), session("b", 100), session("a", 100)],
                false,
            )
            .await
            .unwrap();

        assert_eq!(
            cached_ids(&store, a).await,
            vec!["a", "b", "c"],
            "per-host read must tie-break equal created_at ascending by session_id"
        );
    }

    /// The per-host half of item 2's skip-and-log posture: a row whose
    /// `info_json` has been corrupted directly (bypassing
    /// `replace_host_sessions`, which can never write anything but valid
    /// JSON) must be dropped from the returned list AND logged via
    /// `tracing::warn!` naming its host and session id — not silently
    /// omitted, and not allowed to fail the whole read the way a corrupt
    /// `hosts.kind` fails `list_hosts`
    /// (`list_hosts_fails_loudly_on_a_corrupt_kind_bypassing_the_check`
    /// above). The session id embeds this test's own name so a stray
    /// warning from another test running concurrently in the same process
    /// (this module's `tracing` capture is necessarily process-global —
    /// see its own header comment) can never be mistaken for this one's.
    #[tokio::test]
    async fn cached_sessions_skips_a_poisoned_blob_and_serves_the_rest() {
        // Installed before anything is logged; read back at the end.
        let _capture = crate::test_capture::install();
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "poison@host", "poison-identity").await;
        store
            .replace_host_sessions(
                host,
                "poison-identity",
                vec![
                    session("good-1", 100),
                    session("cached-sessions-poisoned", 200),
                ],
                false,
            )
            .await
            .unwrap();
        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                conn.lock()
                    .unwrap()
                    .execute(
                        "UPDATE session_cache SET info_json = 'not valid json' \
                         WHERE session_id = 'cached-sessions-poisoned'",
                        [],
                    )
                    .unwrap();
            })
            .await
            .unwrap();
        }

        let sessions = store
            .cached_sessions(host)
            .await
            .expect("a poisoned row must not fail the whole read");
        let ids: Vec<String> = sessions.into_iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            vec!["good-1".to_string()],
            "the poisoned row must be skipped and every other row still served"
        );

        let events = skip_warnings();
        let hit = events
            .iter()
            .find(|e| e.field("session_id") == Some("cached-sessions-poisoned"));
        let hit = hit.expect(
            "the skipped row must be logged via tracing::warn! naming its session id, \
             not just silently dropped from the returned Vec",
        );
        assert_eq!(
            hit.field("host"),
            Some(host.to_string().as_str()),
            "the warning must name the host the poisoned row belongs to"
        );
    }

    /// The cross-host half of item 2's skip-and-log posture: the same
    /// poisoned-blob treatment as
    /// `cached_sessions_skips_a_poisoned_blob_and_serves_the_rest`, but
    /// through [`HelmStore::cached_rows`]'s multi-host read, with an entirely
    /// healthy second host present to prove one host's corruption cannot
    /// take down another host's rows in the shared merged-list read either.
    #[tokio::test]
    async fn cached_rows_skips_a_poisoned_blob_and_serves_the_rest() {
        // Installed before anything is logged; read back at the end.
        let _capture = crate::test_capture::install();
        let (_dir, store) = fresh_store().await;
        let poisoned_host =
            host_with_identity(&store, "poison-all@host", "poison-all-identity").await;
        let healthy_host =
            host_with_identity(&store, "healthy-all@host", "healthy-all-identity").await;
        store
            .replace_host_sessions(
                poisoned_host,
                "poison-all-identity",
                vec![session("cached-rows-poisoned", 100)],
                false,
            )
            .await
            .unwrap();
        store
            .replace_host_sessions(
                healthy_host,
                "healthy-all-identity",
                vec![session("healthy-1", 200)],
                false,
            )
            .await
            .unwrap();
        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                conn.lock()
                    .unwrap()
                    .execute(
                        "UPDATE session_cache SET info_json = 'not valid json' \
                         WHERE session_id = 'cached-rows-poisoned'",
                        [],
                    )
                    .unwrap();
            })
            .await
            .unwrap();
        }

        let rows = store
            .cached_rows(&[poisoned_host, healthy_host])
            .await
            .expect("cached rows");
        let decoded: Vec<&str> = rows.iter().map(|row| row.info.id.as_str()).collect();
        assert_eq!(
            decoded,
            vec!["healthy-1"],
            "the poisoned row must be dropped while the other host's row is still served"
        );

        let events = skip_warnings();
        let hit = events
            .iter()
            .find(|e| e.field("session_id") == Some("cached-rows-poisoned"));
        let hit = hit.expect(
            "the skipped row must be logged via tracing::warn! naming its session id, \
             not just silently dropped from `info`",
        );
        assert_eq!(
            hit.field("host"),
            Some(poisoned_host.to_string().as_str()),
            "the warning must name the host the poisoned row belongs to"
        );
    }

    // ---- Persistence ------------------------------------------------

    /// Everything above must survive a full close-and-reopen of the
    /// database file — helm.db's entire reason to exist (SPEC.md: the
    /// stale list survives a helm restart).
    #[tokio::test]
    async fn everything_survives_a_store_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("helm.db");

        let (local_id, ssh_id) = {
            let store = HelmStore::open(&db_path).await.expect("open");
            let local_id = store.list_hosts().await.unwrap()[0].id;
            let ssh_id = store
                .add_ssh_host("persist@host", Some("farhelm"), Some("/state"))
                .await
                .unwrap();
            store
                .record_first_contact(
                    local_id,
                    &dialed_as(&store, local_id).await,
                    "local-identity",
                )
                .await
                .unwrap();
            store
                .record_first_contact(ssh_id, &dialed_as(&store, ssh_id).await, "remote-identity")
                .await
                .unwrap();
            store
                .replace_host_sessions(
                    local_id,
                    "local-identity",
                    vec![session("local-1", 100)],
                    false,
                )
                .await
                .unwrap();
            store
                .replace_host_sessions(
                    ssh_id,
                    "remote-identity",
                    vec![session("remote-1", 200)],
                    false,
                )
                .await
                .unwrap();
            (local_id, ssh_id)
        };

        let reopened = HelmStore::open(&db_path).await.expect("reopen");
        let hosts = reopened.list_hosts().await.unwrap();
        assert_eq!(hosts.len(), 2, "both hosts must survive");
        let local = hosts.iter().find(|h| h.id == local_id).unwrap();
        assert_eq!(local.kind, HostKind::Local);
        assert_eq!(local.host_identity.as_deref(), Some("local-identity"));
        let ssh = hosts.iter().find(|h| h.id == ssh_id).unwrap();
        assert_eq!(ssh.destination.as_deref(), Some("persist@host"));
        assert_eq!(ssh.remote_farhelm.as_deref(), Some("farhelm"));
        assert_eq!(ssh.remote_state_dir.as_deref(), Some("/state"));
        assert_eq!(ssh.host_identity.as_deref(), Some("remote-identity"));

        assert_eq!(
            reopened
                .cached_sessions(local_id)
                .await
                .unwrap()
                .into_iter()
                .map(|s| s.id)
                .collect::<Vec<_>>(),
            vec!["local-1".to_string()]
        );
        assert_eq!(
            reopened
                .cached_sessions(ssh_id)
                .await
                .unwrap()
                .into_iter()
                .map(|s| s.id)
                .collect::<Vec<_>>(),
            vec!["remote-1".to_string()]
        );
    }

    /// The changed-only rule at its source: a wholesale replacement that
    /// writes the row set already stored reports no change.
    ///
    /// This is what stops the invalidation feed from being a per-host,
    /// per-refresh-interval wakeup for every open client (PLAN_M6_75.md item
    /// 5) — in a settled fleet nearly every drain writes back exactly what
    /// was there. Both directions are asserted from ONE store, because
    /// "reports no change" is only meaningful beside a comparable write that
    /// does.
    #[tokio::test]
    async fn a_replacement_reports_a_change_only_when_a_row_actually_differs() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;

        let first = store
            .replace_host_sessions(host, "identity-1", vec![session("s-1", 100)], false)
            .await
            .unwrap();
        assert!(
            first.changed,
            "filling an empty cache is a change by any reading"
        );

        let repeat = store
            .replace_host_sessions(host, "identity-1", vec![session("s-1", 100)], false)
            .await
            .unwrap();
        assert!(
            !repeat.changed,
            "a drain that writes back an identical row set must wake nobody"
        );

        // A payload difference that leaves the ordering columns alone is
        // still a change: the comparison is over what is STORED, not over
        // the key it is filed under, and a status flip is exactly the
        // milestone's motivating case.
        let flipped = SessionInfo {
            status: farhelm_proto::SessionStatus::Waiting,
            ..session("s-1", 100)
        };
        let changed = store
            .replace_host_sessions(host, "identity-1", vec![flipped], false)
            .await
            .unwrap();
        assert!(changed.changed, "a status flip is a change");

        // And so is losing a session, which no per-row comparison of the
        // NEW list against itself would ever notice.
        let emptied = store
            .replace_host_sessions(host, "identity-1", Vec::new(), false)
            .await
            .unwrap();
        assert!(emptied.changed, "a session disappearing is a change");
    }

    /// A rewrite that repairs a drifted `created_at` COLUMN is a change, even
    /// though the payload beside it is untouched.
    ///
    /// `created_at` is a column of its own, extracted from the payload at
    /// write time, and it can therefore disagree with the payload's own copy
    /// in a database this build did not write (a hand edit, a downgrade, an
    /// older writer). [`HelmStore::cached_rows`]'s identity cross-check
    /// treats that disagreement as poison — the drifted row is dropped from
    /// every read until something rewrites the column back into agreement —
    /// so comparing payloads only would report the repair as "nothing
    /// changed" and the feed would starve: the row would stay missing with
    /// no client ever told to look again.
    #[tokio::test]
    async fn a_repaired_ordering_column_counts_as_a_change() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;
        store
            .replace_host_sessions(host, "identity-1", vec![session("s-1", 100)], false)
            .await
            .unwrap();

        // The column drifts away from the payload, which is only reachable
        // by writing behind this module's back — exactly the provenance the
        // schema's own comments say the read path must tolerate.
        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                conn.lock()
                    .unwrap()
                    .execute(
                        "UPDATE session_cache SET created_at = 999 WHERE session_id = 's-1'",
                        [],
                    )
                    .unwrap();
            })
            .await
            .unwrap();
        }

        let repaired = store
            .replace_host_sessions(host, "identity-1", vec![session("s-1", 100)], false)
            .await
            .unwrap();
        assert!(
            repaired.changed,
            "the row went from dropped to served, so clients must be told to look again"
        );
        // And the write that follows it, with nothing left to repair, is a
        // no-op again — so this is a comparison rather than a permanent
        // "changed" latch.
        let settled = store
            .replace_host_sessions(host, "identity-1", vec![session("s-1", 100)], false)
            .await
            .unwrap();
        assert!(!settled.changed);
    }

    /// A mutation's reply may push a session's `last_activity_at` forward
    /// and never back — including when the reply carries the old wire
    /// value of 0.
    ///
    /// The durable half of `crate::manager::merge_cached_session`'s
    /// contract, pinned where it is actually reachable. The race is
    /// routine rather than exotic: a refresh drain and a create, rename,
    /// restart, or archive reply both write this cache, nothing orders
    /// them, and a reply merely echoes whatever the supervisor's session
    /// entry held when it was built. So a reply that overwrote the field
    /// wholesale would regularly move a session BACKWARDS in a
    /// most-recently-active list at the moment the user acted on it, and
    /// leave it there until the owning host's next refresh.
    ///
    /// The `0` case is the one with no natural correction at all: it means
    /// "this sender predates the field" (`SessionInfo::last_activity_at`),
    /// and letting it win would replace a real observation with the epoch
    /// — a value a reader cannot tell from a genuine one, since the
    /// `created_at` fallback is applied at read time and never written
    /// down.
    #[tokio::test]
    async fn a_mutation_reply_never_walks_the_activity_stamp_backwards() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;

        let stamped = |at: i64| SessionInfo {
            last_activity_at: at,
            ..session("s-1", 100)
        };
        let cached_stamp = async |store: &HelmStore| {
            let rows = store
                .cached_rows(&[host])
                .await
                .expect("read the cache back");
            rows[0].info.last_activity_at
        };

        // A drain commits a fresh observation.
        store
            .replace_host_sessions(host, "identity-1", vec![stamped(900)], false)
            .await
            .expect("drain");
        assert_eq!(cached_stamp(&store).await, 900);

        // A mutation reply built before that drain landed.
        store
            .remember_session(host, "identity-1", &stamped(500))
            .await
            .expect("stale reply");
        assert_eq!(
            cached_stamp(&store).await,
            900,
            "a reply carrying an older observation must not undo a newer one"
        );

        // And one from a sender that does not know the field at all.
        store
            .remember_session(host, "identity-1", &stamped(0))
            .await
            .expect("reply from an old sender");
        assert_eq!(
            cached_stamp(&store).await,
            900,
            "0 means unknown, not 1970; it may never replace a real observation"
        );

        // Forward is still allowed — the merge is monotonic, not frozen.
        store
            .remember_session(host, "identity-1", &stamped(1_500))
            .await
            .expect("fresh reply");
        assert_eq!(cached_stamp(&store).await, 1_500);
    }

    /// The single-row writes answer the same question, so a mutation that
    /// re-records what the cache already says invalidates nothing.
    ///
    /// The case this exists for is a RETRIED create under an idempotency
    /// key: the supervisor replays the same session, the helm re-records it,
    /// and nothing about the fleet is different than it was.
    #[tokio::test]
    async fn single_row_writes_report_change_the_same_way() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;

        assert!(
            store
                .remember_session(host, "identity-1", &session("s-1", 100))
                .await
                .unwrap(),
            "seeding a session nobody had is a change"
        );
        assert!(
            !store
                .remember_session(host, "identity-1", &session("s-1", 100))
                .await
                .unwrap(),
            "re-recording an identical session must wake nobody"
        );
        assert!(
            store
                .forget_session(host, "identity-1", "s-1")
                .await
                .unwrap(),
            "removing a row that was there is a change"
        );
        assert!(
            !store
                .forget_session(host, "identity-1", "s-1")
                .await
                .unwrap(),
            "forgetting a session that was already gone succeeds, but changes nothing"
        );

        // The single-row write compares the ordering column too, for the
        // wholesale write's reason: a seed that repairs a drifted
        // `created_at` moves the row in the merged order while leaving its
        // payload untouched.
        store
            .remember_session(host, "identity-1", &session("s-2", 100))
            .await
            .unwrap();
        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                conn.lock()
                    .unwrap()
                    .execute(
                        "UPDATE session_cache SET created_at = 999 WHERE session_id = 's-2'",
                        [],
                    )
                    .unwrap();
            })
            .await
            .unwrap();
        }
        assert!(
            store
                .remember_session(host, "identity-1", &session("s-2", 100))
                .await
                .unwrap(),
            "a seed that moves the row in the merged order is a change"
        );
    }

    /// The remembered default is one value for the whole helm: replaceable,
    /// durable across a reopen, and not tied to any host.
    ///
    /// Durability is the point of storing it in helm.db at all rather than in memory:
    /// SPEC.md's create dialog defaults to the last-used profile, and a default that
    /// evaporated on every helm restart would send the user back to picking one by
    /// hand exactly when they had just established a habit. Host removal is
    /// included to prove registry lifecycle cannot erase unrelated singleton
    /// state; it does not claim profile ids are portable across host catalogs.
    #[tokio::test]
    async fn a_remembered_default_is_helm_wide_replaceable_and_durable() {
        let (dir, store) = fresh_store().await;
        let ssh = store.add_ssh_host("user@host", None, None).await.unwrap();

        assert_eq!(store.remembered_profile().await.unwrap(), None);
        // `None` throughout: neither of these hosts has ever reported an
        // identity, which is itself the value the write must match.
        assert!(
            store.remember_profile_default("p-1").await.unwrap(),
            "the first remembered default is a change"
        );
        assert!(
            !store.remember_profile_default("p-1").await.unwrap(),
            "creating from the same profile twice changes nothing observable"
        );
        assert!(store.remember_profile_default("p-2").await.unwrap());
        assert_eq!(
            store.remembered_profile().await.unwrap(),
            Some("p-2".to_string()),
            "the latest choice replaces the previous one rather than accumulating"
        );
        // Durable across a genuine reopen of the same file.
        drop(store);
        let reopened = HelmStore::open(&dir.path().join("helm.db"))
            .await
            .expect("reopen");
        assert_eq!(
            reopened.remembered_profile().await.unwrap(),
            Some("p-2".to_string())
        );

        // Removing a host does not affect the helm-wide preference.
        reopened.remember_profile_default("p-3").await.unwrap();
        reopened.remove_ssh_host(ssh).await.unwrap();
        assert_eq!(
            reopened.remembered_profile().await.unwrap(),
            Some("p-3".to_string())
        );
    }

    /// Host lifecycle changes leave the helm-wide remembered default alone.
    ///
    /// Learning an identity, adopting a successor installation, and
    /// retargeting one registry row can change that host and its cache, but
    /// none owns the singleton. This pins non-interference without claiming
    /// the remembered id already names the same definition on every host.
    #[tokio::test]
    async fn host_lifecycle_changes_do_not_touch_the_helm_wide_default() {
        let (_dir, store) = fresh_store().await;
        let host = store
            .add_ssh_host("user@learner", None, None)
            .await
            .unwrap();
        assert!(
            store
                .remember_profile_default("starter-claude")
                .await
                .unwrap()
        );

        // The host's first successful hello teaches the registry an identity.
        store
            .record_first_contact(host, &dialed_as(&store, host).await, "identity-1")
            .await
            .unwrap();
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("starter-claude"),
            "learning an identity is not a reason to forget a preference"
        );

        // The install behind the row is replaced and the user adopts it.
        store
            .adopt_identity(
                host,
                &dialed_as(&store, host).await,
                "identity-1",
                "identity-2",
            )
            .await
            .unwrap();
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("starter-claude"),
            "an adoption purges the session cache, not the remembered default"
        );

        // And the row is pointed somewhere else entirely.
        store
            .update_ssh_destination(host, "user@elsewhere")
            .await
            .unwrap();
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("starter-claude"),
            "a retarget leaves the remembered default alone"
        );

        // Writing the same id again is still no change, so the invalidation
        // feed is not woken for a user creating from one profile repeatedly.
        assert!(
            !store
                .remember_profile_default("starter-claude")
                .await
                .unwrap()
        );
    }

    /// The helm-wide remembered default accepts a raw id even without a
    /// matching host or catalog row.
    #[tokio::test]
    async fn remembering_a_default_is_independent_of_host_registry_rows() {
        let (_dir, store) = fresh_store().await;
        assert!(store.remember_profile_default("p-1").await.unwrap());
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("p-1")
        );
    }

    /// A cached row that cannot be shown is DROPPED from the read — not
    /// carried as a placeholder, not counted — and the drop is logged.
    ///
    /// The two ways a row fails to decode are both staged, because they
    /// took different code paths before this method unified them: an
    /// undecodable payload, and one that decodes but names a different
    /// session than the row it is filed under (poison wearing valid JSON).
    /// Dropping rather than counting is the contract the merged list's
    /// counts rest on: `total` and `matching` describe rows a client can
    /// see, and a corrupt row is a warning in the log, not an entry in the
    /// denominator (SPEC.md's Session list section).
    #[tokio::test]
    async fn a_row_that_cannot_be_shown_is_dropped_from_the_read_and_logged() {
        let _capture = crate::test_capture::install();
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "poison@host", "poison-identity").await;
        store
            .replace_host_sessions(
                host,
                "poison-identity",
                vec![
                    SessionInfo {
                        title: "keeper".to_string(),
                        ..session("good-1", 500)
                    },
                    SessionInfo {
                        title: "keeper".to_string(),
                        ..session("undecodable", 400)
                    },
                    SessionInfo {
                        title: "keeper".to_string(),
                        ..session("mislabelled", 300)
                    },
                ],
                false,
            )
            .await
            .expect("seed the cache");
        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().unwrap();
                conn.execute(
                    "UPDATE session_cache SET info_json = 'not valid json' \
                     WHERE session_id = 'undecodable'",
                    [],
                )
                .unwrap();
                // Decodes perfectly, and names a DIFFERENT session than the
                // row it is filed as — the poison a naive decode misses.
                let impostor = serde_json::to_string(&SessionInfo {
                    title: "keeper".to_string(),
                    ..SessionInfo {
                        id: "somebody-else".to_string(),
                        ..session("mislabelled", 300)
                    }
                })
                .unwrap();
                conn.execute(
                    "UPDATE session_cache SET info_json = ?1 WHERE session_id = 'mislabelled'",
                    rusqlite::params![impostor],
                )
                .unwrap();
            })
            .await
            .unwrap();
        }

        let rows = store.cached_rows(&[host]).await.expect("cached rows");
        let ids: Vec<&str> = rows.iter().map(|row| row.info.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["good-1"],
            "only the row that actually decodes to itself is served; the other two are gone \
             from the read, not present as holes"
        );

        let events = skip_warnings();
        for dropped in ["undecodable", "mislabelled"] {
            let hit = events
                .iter()
                .find(|e| e.field("session_id") == Some(dropped))
                .unwrap_or_else(|| panic!("a warning must name the dropped row {dropped}"));
            assert_eq!(
                hit.field("host"),
                Some(host.to_string().as_str()),
                "and the host it belongs to"
            );
        }
    }

    /// A session that comes back UNARCHIVED rejoins the default view's
    /// column, and therefore whatever count a caller derives from it.
    ///
    /// The single-row write's `ON CONFLICT` clause has to carry `archived`
    /// in both directions, and only one of them is exercised by ordinary
    /// use. Drop it from the update and an archived row's flag becomes
    /// permanent in the cache: the session would be invisible in the
    /// ordinary list no matter what the supervisor went on to say about it,
    /// with no way back short of deleting the cache — a wrong answer that
    /// survives every refresh is the worst kind for a denormalized column to
    /// give.
    #[tokio::test]
    async fn a_session_that_comes_back_unarchived_rejoins_the_default_view() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "flip@host", "flip-identity").await;
        let default_view = async |store: &HelmStore| {
            store
                .cached_rows(&[host])
                .await
                .expect("cached rows")
                .into_iter()
                .filter(|row| !row.archived)
                .count()
        };

        store
            .remember_session(
                host,
                "flip-identity",
                &SessionInfo {
                    archived: true,
                    ..session("s-1", 100)
                },
            )
            .await
            .expect("remember an archived session");
        assert_eq!(
            default_view(&store).await,
            0,
            "an archived session is outside the default view"
        );

        assert!(
            store
                .remember_session(host, "flip-identity", &session("s-1", 100))
                .await
                .expect("remember it as active"),
            "the payload changed, so the write is a change clients must be told about"
        );
        assert_eq!(
            default_view(&store).await,
            1,
            "and the row rejoins the default view the moment its column flips back"
        );
    }

    /// `hosts` is [`HelmStore::cached_rows`]'s SCOPE, not a predicate over a
    /// wider read: a host left out of the slice contributes nothing, and a
    /// host that never registered any sessions contributes nothing either.
    ///
    /// Spec: scoping to one host returns exactly that host's rows, in a
    /// fleet where a second host has rows too; scoping to a host with no
    /// cached sessions returns an empty `Vec` rather than every host's rows
    /// (what an unguarded empty `IN`-list would quietly mean).
    #[tokio::test]
    async fn cached_rows_scopes_to_exactly_the_requested_hosts() {
        let (_dir, store) = fresh_store().await;
        let alpha = host_with_identity(&store, "user@alpha", "identity-alpha").await;
        let beta = host_with_identity(&store, "user@beta", "identity-beta").await;
        store
            .replace_host_sessions(alpha, "identity-alpha", vec![session("a-1", 300)], false)
            .await
            .unwrap();
        store
            .replace_host_sessions(
                beta,
                "identity-beta",
                vec![session("b-1", 200), session("b-2", 100)],
                false,
            )
            .await
            .unwrap();

        let both: Vec<String> = store
            .cached_rows(&[alpha, beta])
            .await
            .expect("both hosts")
            .into_iter()
            .map(|row| row.info.id)
            .collect();
        assert_eq!(
            both.len(),
            3,
            "scoping to both hosts returns every row either of them cached"
        );

        let mut beta_only: Vec<String> = store
            .cached_rows(&[beta])
            .await
            .expect("beta only")
            .into_iter()
            .map(|row| row.info.id)
            .collect();
        beta_only.sort();
        assert_eq!(
            beta_only,
            vec!["b-1".to_string(), "b-2".to_string()],
            "scoping to beta returns only beta's rows, not alpha's"
        );

        // A host with cached sessions but left OUT of the slice — not a host
        // that never registered any — is the sharper version of the same
        // check: alpha genuinely has a row, and it must not leak in.
        let alpha_only: Vec<String> = store
            .cached_rows(&[alpha])
            .await
            .expect("alpha only")
            .into_iter()
            .map(|row| row.info.id)
            .collect();
        assert_eq!(alpha_only, vec!["a-1".to_string()]);
    }

    /// The filter's match semantics, pinned where they are defined.
    ///
    /// The REST tests cover the query string and the two totals; this covers
    /// the predicate itself, including the three rules a reader is most
    /// likely to get wrong when touching it: substring versus exact per
    /// dimension, the profile filter's id-OR-snapshotted-name reading, and
    /// the fact that dimensions AND together.
    #[test]
    fn the_session_filter_matches_by_the_documented_rules() {
        use farhelm_proto::{ProfileExistence, SessionStatus, SourceProfile};

        let info = SessionInfo {
            parent: Some("parent-7".to_string()),
            cwd: "/home/Me/src/Farhelm".to_string(),
            title: "Refactor the Drain".to_string(),
            status: SessionStatus::Waiting,
            source_profile: Some(SourceProfile {
                id: "p-7".to_string(),
                name: "Claude Code".to_string(),
                // Deliberately deleted: existence is DERIVED at reply time
                // and says nothing about whether a filter matches, because
                // the snapshot is what the session actually carries.
                existence: ProfileExistence::Deleted,
            }),
            ..session("s-1", 100)
        };

        // Free text: substring, case-insensitive, on both dimensions.
        assert!(
            SessionFilter::default()
                .directory("src/farhelm")
                .matches(1, &info)
        );
        assert!(SessionFilter::default().title("DRAIN").matches(1, &info));
        assert!(
            !SessionFilter::default()
                .title("drainpipe")
                .matches(1, &info)
        );

        // Enumerable: exact.
        assert!(SessionFilter::default().status("waiting").matches(1, &info));
        assert!(!SessionFilter::default().status("running").matches(1, &info));
        assert!(SessionFilter::default().host(1).matches(1, &info));
        assert!(!SessionFilter::default().host(2).matches(1, &info));
        assert!(
            SessionFilter::default()
                .parent("parent-7")
                .matches(1, &info)
        );
        assert!(!SessionFilter::default().parent("parent").matches(1, &info));

        // Profile: by id (exact, opaque) or by snapshotted name
        // (case-insensitive), and never by prefix.
        assert!(SessionFilter::default().profile("p-7").matches(1, &info));
        assert!(
            SessionFilter::default()
                .profile("claude code")
                .matches(1, &info)
        );
        assert!(!SessionFilter::default().profile("claude").matches(1, &info));
        assert!(!SessionFilter::default().profile("p-").matches(1, &info));

        // A raw-created session matches no profile filter at all.
        let raw = SessionInfo {
            source_profile: None,
            ..info.clone()
        };
        assert!(!SessionFilter::default().profile("p-7").matches(1, &raw));

        // Dimensions AND: adding one can only ever narrow.
        assert!(
            !SessionFilter::default()
                .title("drain")
                .status("running")
                .matches(1, &info)
        );
        assert!(
            !SessionFilter::default().is_empty(),
            "the default archive exclusion requires the predicate scan"
        );
        assert!(SessionFilter::default().include_archived(true).is_empty());
        assert!(!SessionFilter::default().title("x").is_empty());
        assert!(!SessionFilter::default().parent("parent-7").is_empty());
    }

    /// Completed drains converge the remembered default to their newest
    /// profile-backed source and never let an older snapshot roll it back.
    #[tokio::test]
    async fn drain_convergence_advances_only_to_newer_profile_provenance() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "profiles@host", "profile-identity").await;
        store
            .remember_profile_default_from_session("profile-old", Some(2), 200, "old-source")
            .await
            .unwrap();

        let advanced = store
            .replace_host_sessions(
                host,
                "profile-identity",
                vec![
                    session("raw-newer", 400),
                    sequenced_profiled_session("new-source", 300, 3, "profile-new"),
                ],
                false,
            )
            .await
            .unwrap();
        assert!(advanced.default_changed);
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("profile-new")
        );

        let delayed = store
            .replace_host_sessions(
                host,
                "profile-identity",
                vec![
                    sequenced_profiled_session("new-source", 300, 3, "profile-new"),
                    sequenced_profiled_session("older-source", 300, 1, "profile-older"),
                ],
                false,
            )
            .await
            .unwrap();
        assert!(!delayed.default_changed);
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("profile-new"),
            "an older completed drain cannot overwrite newer provenance"
        );

        let provenance_only = store
            .replace_host_sessions(
                host,
                "profile-identity",
                vec![sequenced_profiled_session(
                    "same-profile-new-source",
                    300,
                    4,
                    "profile-new",
                )],
                false,
            )
            .await
            .unwrap();
        assert!(
            !provenance_only.default_changed,
            "advancing provenance for the same default must not wake clients"
        );
        let no_rollback = store
            .replace_host_sessions(
                host,
                "profile-identity",
                vec![
                    sequenced_profiled_session("same-profile-new-source", 300, 4, "profile-new"),
                    sequenced_profiled_session("late-old", 300, 2, "profile-old"),
                ],
                false,
            )
            .await
            .unwrap();
        assert!(!no_rollback.default_changed);
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("profile-new")
        );

        let retreated = store
            .replace_host_sessions(
                host,
                "profile-identity",
                vec![sequenced_profiled_session(
                    "older-source",
                    300,
                    1,
                    "profile-older",
                )],
                false,
            )
            .await
            .unwrap();
        assert!(
            !retreated.default_changed,
            "the visible default did not move, so nobody is woken"
        );
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("profile-new"),
            "a drain that orphans the provenance keeps the bare id rather than guessing from \
             survivors"
        );

        let cleared = store
            .replace_host_sessions(host, "profile-identity", Vec::new(), false)
            .await
            .unwrap();
        assert!(!cleared.default_changed);
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("profile-new"),
            "an empty drain does not forget the helm-wide preference either"
        );
    }

    /// A retarget-shaped drain — the remembered source is gone and nothing
    /// provably newer replaces it — keeps the bare default and re-opens it
    /// to the next direct create.
    ///
    /// This is the install transition the bare-id contract exists for.
    /// Deleting the row here would rebuild the install-bound eviction schema
    /// v12 removed; replacing it from a surviving session would guess; and
    /// keeping the predecessor's provenance would let its high sequence
    /// numbers refuse the successor's own first create (a fresh supervisor
    /// restarts sequences low). So: the id survives, the provenance is
    /// cleared, survivors do not advance it, and a direct create with a
    /// RESET sequence does.
    #[tokio::test]
    async fn a_retarget_shaped_drain_keeps_the_bare_default() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "moving@host", "moving-identity").await;
        store
            .remember_profile_default_from_host_session(
                "profile-kept",
                host,
                Some(9),
                900,
                "gone-source",
            )
            .await
            .unwrap();

        // The successor install knows nothing of the establishing session.
        let survived = store
            .replace_host_sessions(host, "moving-identity", Vec::new(), false)
            .await
            .unwrap();
        assert!(
            !survived.default_changed,
            "the visible default did not move, so nobody is woken"
        );
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("profile-kept")
        );

        // A surviving OLDER session is not evidence of a newer choice.
        let not_replaced = store
            .replace_host_sessions(
                host,
                "moving-identity",
                vec![sequenced_profiled_session("older", 300, 1, "profile-other")],
                false,
            )
            .await
            .unwrap();
        assert!(!not_replaced.default_changed);
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("profile-kept"),
            "an orphaned preference stays opaque rather than being replaced from survivors"
        );

        // A fresh supervisor's first create restarts sequence numbers low.
        // With the predecessor's provenance cleared it must win — refused,
        // it would leave the default permanently stuck on the old id.
        assert!(
            store
                .remember_profile_default_from_session("profile-new", Some(1), 100, "fresh")
                .await
                .unwrap()
        );
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("profile-new")
        );
    }

    /// Old supervisors omit creation sequences, so equal-second drains keep
    /// the pre-upgrade ascending-id tiebreak.
    #[tokio::test]
    async fn drain_provenance_falls_back_to_timestamp_and_id_when_sequence_is_absent() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "fallback@host", "fallback-identity").await;
        store
            .replace_host_sessions(
                host,
                "fallback-identity",
                vec![
                    profiled_session("z-source", 100, "profile-z"),
                    profiled_session("a-source", 100, "profile-a"),
                ],
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("profile-a")
        );
    }

    /// A caller without the source host cannot assign a domain to supervisor
    /// sequences. The shared timestamp/id fallback must therefore decide
    /// between unattributed observations instead of comparing local counters.
    #[tokio::test]
    async fn unattributed_remembered_sources_use_the_fleet_wide_fallback() {
        let (_dir, store) = fresh_store().await;
        assert!(
            store
                .remember_profile_default_from_session("new", Some(10), 100, "new-source",)
                .await
                .unwrap()
        );
        assert!(
            store
                .remember_profile_default_from_session("old", Some(9), 200, "old-source",)
                .await
                .unwrap()
        );
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("old")
        );
    }

    /// Independent supervisors restart their creation sequences, so a later
    /// drain from host B must not lose merely because host A reached a larger
    /// local number. This drives the production refresh path rather than the
    /// singleton writer directly, pinning both persisted domain provenance
    /// and the cross-host timestamp fallback.
    #[tokio::test]
    async fn cross_host_drains_do_not_compare_local_sequences() {
        let (_dir, store) = fresh_store().await;
        let host_a = host_with_identity(&store, "a@host", "a-identity").await;
        let host_b = host_with_identity(&store, "b@host", "b-identity").await;

        store
            .replace_host_sessions(
                host_a,
                "a-identity",
                vec![sequenced_profiled_session(
                    "a-source",
                    100,
                    100,
                    "profile-a",
                )],
                false,
            )
            .await
            .unwrap();
        let newer = store
            .replace_host_sessions(
                host_b,
                "b-identity",
                vec![sequenced_profiled_session("b-source", 200, 1, "profile-b")],
                false,
            )
            .await
            .unwrap();
        assert!(newer.default_changed);
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("profile-b")
        );
    }

    /// Refreshing host B cannot establish that host A's remembered source
    /// disappeared. Keeping the provenance prevents an unrelated refresh
    /// from reopening ordering to a delayed, older observation.
    #[tokio::test]
    async fn an_unrelated_host_refresh_keeps_remembered_provenance() {
        let (_dir, store) = fresh_store().await;
        let host_a = host_with_identity(&store, "a@host", "a-identity").await;
        let host_b = host_with_identity(&store, "b@host", "b-identity").await;

        store
            .replace_host_sessions(
                host_a,
                "a-identity",
                vec![sequenced_profiled_session("a-source", 200, 9, "profile-a")],
                false,
            )
            .await
            .unwrap();
        store
            .replace_host_sessions(host_b, "b-identity", Vec::new(), false)
            .await
            .unwrap();
        let delayed = store
            .replace_host_sessions(
                host_a,
                "a-identity",
                vec![sequenced_profiled_session("a-old", 100, 8, "profile-old")],
                false,
            )
            .await
            .unwrap();
        assert!(!delayed.default_changed);
        assert_eq!(
            store.remembered_profile().await.unwrap().as_deref(),
            Some("profile-a")
        );
    }

    /// Every status the wire can carry has a filter word, and nothing else
    /// is accepted.
    ///
    /// The round trip is what matters: a status whose key could not be
    /// parsed back would make those sessions unfilterable, and a word
    /// accepted that no status produces would silently match nothing.
    #[test]
    fn every_status_key_round_trips_and_unknown_words_are_refused() {
        use farhelm_proto::SessionStatus;

        for status in [
            SessionStatus::Unknown,
            SessionStatus::Running,
            SessionStatus::Waiting,
            SessionStatus::Idle,
            SessionStatus::Exited { exit_code: Some(3) },
            SessionStatus::Error {
                detail: "no such file".to_string(),
            },
            SessionStatus::Interrupted,
        ] {
            let key = status_key(&status);
            assert_eq!(
                parse_status_key(key),
                Some(key),
                "{key} must parse back to itself"
            );
        }
        for unknown in ["alive", "", "Running", "waiting "] {
            assert_eq!(
                parse_status_key(unknown),
                None,
                "{unknown:?} is not a status"
            );
        }
    }

    /// Every order this helm serves has a `?sort=` word, and nothing else is
    /// accepted.
    ///
    /// [`parse_status_key`]'s reasoning one dimension over: an order whose
    /// word did not parse back would be unreachable from a query string, and
    /// a word accepted that names no order would serve a list in a sequence
    /// the caller did not ask for — which, unlike an empty result, looks
    /// entirely plausible.
    #[test]
    fn every_sort_key_round_trips_and_unknown_words_are_refused() {
        for (word, sort) in [
            ("created", ListSort::Created),
            ("activity", ListSort::Activity),
            ("title", ListSort::Title),
        ] {
            assert_eq!(
                parse_sort_key(word),
                Some(sort),
                "{word:?} must parse to {sort:?}"
            );
        }
        assert_eq!(
            ListSort::default(),
            ListSort::Created,
            "the absent-parameter order is the one every client had before there was a choice"
        );
        for unknown in ["", "cwd", "Created", "activity ", "-created"] {
            assert_eq!(parse_sort_key(unknown), None, "{unknown:?} is not an order");
        }
    }

    /// [`HelmStore::cached_rows`] reflects every write path that touches the
    /// cache — [`HelmStore::replace_host_sessions`],
    /// [`HelmStore::remember_session`], and [`HelmStore::forget_session`] —
    /// the moment each commits, with no separate read-repair step.
    ///
    /// This is the load-bearing property behind the single-row and
    /// wholesale-replacement tests elsewhere in this module, which mostly
    /// read back through the narrower `cached_sessions`: since
    /// `crate::aggregate` now serves the whole merged list from
    /// `cached_rows` alone, a regression that updated `session_cache` in a
    /// way `cached_sessions` still saw correctly but `cached_rows` did not
    /// (a stale join, a forgotten column in its `SELECT`) would otherwise
    /// have no test that could see it.
    #[tokio::test]
    async fn cached_rows_reflects_every_cache_write_path() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "reflect@host", "reflect-identity").await;

        let ids = |rows: &[CachedRow]| -> Vec<String> {
            let mut ids: Vec<String> = rows.iter().map(|row| row.info.id.clone()).collect();
            ids.sort();
            ids
        };

        store
            .replace_host_sessions(host, "reflect-identity", vec![session("a", 100)], false)
            .await
            .unwrap();
        assert_eq!(
            ids(&store.cached_rows(&[host]).await.unwrap()),
            vec!["a".to_string()]
        );

        store
            .remember_session(host, "reflect-identity", &session("b", 200))
            .await
            .unwrap();
        assert_eq!(
            ids(&store.cached_rows(&[host]).await.unwrap()),
            vec!["a".to_string(), "b".to_string()]
        );

        store
            .forget_session(host, "reflect-identity", "a")
            .await
            .unwrap();
        assert_eq!(
            ids(&store.cached_rows(&[host]).await.unwrap()),
            vec!["b".to_string()]
        );
    }

    /// A fresh catalog contains every starter definition exactly once, and
    /// reopening it preserves user edits and deletions.
    ///
    /// Count-only coverage would allow a typo in an invocation, integration,
    /// or resume template to ship. Reopening after mutations also pins that
    /// schema setup is initialization, not a startup repair that resurrects
    /// or overwrites a starter the user changed.
    #[tokio::test]
    async fn starter_profiles_are_complete_and_seeded_only_once() {
        let (dir, store) = fresh_store().await;
        let starters = vec![
            farhelm_proto::Profile {
                id: "starter-claude".to_string(),
                name: "claude".to_string(),
                invocation: "claude".to_string(),
                agent_kind: farhelm_proto::AgentKind::Claude,
                resume_template: None,
            },
            farhelm_proto::Profile {
                id: "starter-claude-yolo".to_string(),
                name: "claude-yolo".to_string(),
                invocation: "claude --dangerously-skip-permissions".to_string(),
                agent_kind: farhelm_proto::AgentKind::Claude,
                resume_template: Some(vec![
                    "claude".to_string(),
                    "--dangerously-skip-permissions".to_string(),
                    "--resume".to_string(),
                    "{conversation}".to_string(),
                ]),
            },
            farhelm_proto::Profile {
                id: "starter-codex".to_string(),
                name: "codex".to_string(),
                invocation: "codex".to_string(),
                agent_kind: farhelm_proto::AgentKind::Codex,
                resume_template: None,
            },
            farhelm_proto::Profile {
                id: "starter-codex-yolo".to_string(),
                name: "codex-yolo".to_string(),
                invocation: "codex --yolo".to_string(),
                agent_kind: farhelm_proto::AgentKind::Codex,
                resume_template: Some(vec![
                    "codex".to_string(),
                    "--yolo".to_string(),
                    "resume".to_string(),
                    "{conversation}".to_string(),
                ]),
            },
        ];
        assert_eq!(store.profiles().await.unwrap(), starters);

        let edited = farhelm_proto::Profile {
            id: "starter-claude".to_string(),
            name: "claude-local".to_string(),
            invocation: "claude --model local".to_string(),
            agent_kind: farhelm_proto::AgentKind::Claude,
            resume_template: None,
        };
        assert_eq!(
            store.update_profile(edited.clone()).await.unwrap(),
            Some(edited.clone())
        );
        assert!(store.delete_profile("starter-codex").await.unwrap());

        drop(store);
        let reopened = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let mut expected = starters;
        expected[0] = edited;
        expected.retain(|profile| profile.id != "starter-codex");
        assert_eq!(reopened.profiles().await.unwrap(), expected);
    }

    /// Both profile readers fail loudly on every malformed persisted shape
    /// and identify the row that blocked the read.
    ///
    /// Store methods validate ordinary writes, so these fixtures bypass that
    /// boundary as a damaged or hand-edited database would. Silently skipping
    /// or normalizing one would turn corruption into a plausible catalog with
    /// a different meaning.
    #[tokio::test]
    async fn malformed_profile_rows_fail_single_and_catalog_reads() {
        let cases = [
            (
                "corrupt-kind",
                "profile",
                "agent",
                "unknown",
                None,
                "unrecognized agent kind",
            ),
            (
                "corrupt-template",
                "profile",
                "agent",
                "generic",
                Some("not json"),
                "decoding a stored resume template",
            ),
            (
                "corrupt-invocation",
                "profile",
                "",
                "generic",
                None,
                "profile invocation is empty",
            ),
        ];

        for (id, name, invocation, kind, template, reason) in cases {
            let (_dir, store) = fresh_store().await;
            {
                let conn = store.conn.lock().unwrap();
                conn.execute(
                    "INSERT INTO profiles (id, name, invocation, agent_kind, resume_template) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![id, name, invocation, kind, template],
                )
                .unwrap();
            }

            for error in [
                store.profile(id).await.expect_err("single read must fail"),
                store.profiles().await.expect_err("catalog read must fail"),
            ] {
                let rendered = format!("{error:#}");
                assert!(rendered.contains(id), "error must name {id}: {rendered}");
                assert!(
                    rendered.contains(reason),
                    "error for {id} must explain {reason:?}: {rendered}"
                );
            }
        }
    }

    /// The helm catalog owns durable CRUD, rejects invalid replacements
    /// without a write, and enforces the exact shared catalog bound.
    #[tokio::test]
    async fn helm_profile_catalog_crud_is_bounded_and_validated() {
        let (_dir, store) = fresh_store().await;
        assert_eq!(store.profiles().await.unwrap().len(), 4);

        let created = match store
            .create_profile(
                "wrapper".to_string(),
                "wrapper --agent".to_string(),
                farhelm_proto::AgentKind::Generic,
                None,
            )
            .await
            .unwrap()
        {
            ProfileCreation::Created(profile) => profile,
            ProfileCreation::CatalogFull => panic!("the starter catalog has room"),
        };
        assert_eq!(
            store.profile(&created.id).await.unwrap(),
            Some(created.clone())
        );

        let updated = farhelm_proto::Profile {
            id: created.id.clone(),
            name: "renamed".to_string(),
            invocation: "wrapper --renamed".to_string(),
            agent_kind: farhelm_proto::AgentKind::Codex,
            resume_template: Some(vec!["wrapper".to_string(), "{conversation}".to_string()]),
        };
        assert_eq!(
            store.update_profile(updated.clone()).await.unwrap(),
            Some(updated.clone())
        );
        assert_eq!(
            store.profile(&updated.id).await.unwrap(),
            Some(updated.clone())
        );
        assert_eq!(
            store
                .profiles()
                .await
                .unwrap()
                .iter()
                .find(|profile| profile.id == updated.id),
            Some(&updated)
        );
        let invalid = farhelm_proto::Profile {
            name: " ".to_string(),
            ..updated.clone()
        };
        assert!(store.update_profile(invalid).await.is_err());
        assert_eq!(
            store.profile(&updated.id).await.unwrap(),
            Some(updated.clone())
        );
        assert_eq!(
            store
                .profiles()
                .await
                .unwrap()
                .iter()
                .find(|profile| profile.id == updated.id),
            Some(&updated)
        );
        assert!(store.delete_profile(&updated.id).await.unwrap());
        assert_eq!(store.profile(&updated.id).await.unwrap(), None);
        assert!(!store.delete_profile(&updated.id).await.unwrap());
        assert!(
            store
                .create_profile(
                    " ".to_string(),
                    "wrapper".to_string(),
                    farhelm_proto::AgentKind::Generic,
                    None,
                )
                .await
                .is_err()
        );

        let starting_len = store.profiles().await.unwrap().len();
        for index in starting_len..farhelm_proto::MAX_PROFILES {
            assert!(matches!(
                store
                    .create_profile(
                        format!("profile-{index}"),
                        "agent".to_string(),
                        farhelm_proto::AgentKind::Generic,
                        None,
                    )
                    .await
                    .unwrap(),
                ProfileCreation::Created(_)
            ));
        }
        assert_eq!(
            store.profiles().await.unwrap().len(),
            farhelm_proto::MAX_PROFILES
        );
        assert_eq!(
            store
                .create_profile(
                    "past-bound".to_string(),
                    "agent".to_string(),
                    farhelm_proto::AgentKind::Generic,
                    None,
                )
                .await
                .unwrap(),
            ProfileCreation::CatalogFull
        );
        let after_refusal = store.profiles().await.unwrap();
        assert_eq!(after_refusal.len(), farhelm_proto::MAX_PROFILES);
        assert!(
            after_refusal
                .iter()
                .all(|profile| profile.name != "past-bound")
        );
    }
}
