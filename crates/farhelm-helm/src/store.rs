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
//! ## The generation counter: how a reader knows its data has not moved
//!
//! One piece of state here is not in the database at all. [`HelmStore`]
//! carries an in-memory counter advanced by every committed write that
//! CHANGED a cached row, and [`HelmStore::merged_page`] reads it inside the
//! same lock hold that produces a page — so a page, its totals and that
//! number are true of each other by construction.
//!
//! It exists because "how many sessions match this filter" is a full decode
//! of the scope, and a reader that had to repeat it per page would make a
//! walk quadratic in the fleet under the one mutex everything else needs.
//! `crate::aggregate` remembers the answer and names the generation it was
//! taken at; this side is the only place that can decide whether that is
//! still true, because it is the only place a write cannot slip past the
//! comparison. Nothing published AFTER a commit — the fleet's invalidation
//! revision, most temptingly — can serve that purpose.
//!
//! It is process-local and starts at zero on every open, which is safe
//! precisely because its only consumer is an in-memory cache that a restart
//! empties too.

use anyhow::Context;
use farhelm_proto::{SessionInfo, SessionStatus};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
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
const SCHEMA_VERSION: i64 = 9;

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
    /// one. SSH-host MANAGEMENT mutations
    /// ([`HelmStore::update_ssh_destination`], [`HelmStore::remove_ssh_host`])
    /// refuse a [`HostKind::Local`] row outright — it is not user management
    /// surface at all — but identity ([`HelmStore::record_first_contact`],
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
    pub remote_farhelm: Option<String>,
    pub remote_state_dir: Option<String>,
    /// The identity this host's supervisor reported at last contact —
    /// `None` until [`HelmStore::record_first_contact`] has ever succeeded
    /// for this row, which for the local row too is possible right after
    /// `open` (see that method's docs on why minting is deliberately not
    /// this module's job).
    pub host_identity: Option<String>,
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
);

/// One cached session's identity plus its payload, as returned by
/// [`HelmStore::cached_sessions_all`].
///
/// A dedicated wrapper rather than a bare `Vec<SessionInfo>` because a
/// merged, cross-host view is meaningless without knowing which host each
/// row belongs to — [`HelmStore::cached_sessions`], scoped to one host by
/// its caller, has no need for the same wrapper and returns the bare
/// `SessionInfo` list instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedSession {
    pub host: HostId,
    pub info: SessionInfo,
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

/// Remembered-default columns needed to compare one source observation.
///
/// The alias keeps the SQL projection's positional contract visible without
/// making each query repeat an opaque four-element tuple type.
type RememberedProfileRow = (
    String,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<String>,
);

/// A host identity followed by its optional remembered-default row.
///
/// The joined projection distinguishes an unknown host from a known host
/// that has not remembered a profile yet; flattening either case into one
/// `Option` would lose that distinction at the write boundary.
type HostRememberedProfileRow = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<String>,
);

/// Compare provenance from one supervisor, preferring its strict sequence.
///
/// A missing sequence marks an older peer. In that mixed-version case the
/// established timestamp/id rule remains the only ordering both sides can
/// understand, so rollout does not make an old observation permanently
/// incomparable with a new one.
///
/// `false` means the candidate did not advance the stored source. That covers
/// both equality and rejection as older; callers must not interpret it as
/// proof that the two provenance records identify the same observation.
fn source_is_newer(
    candidate_seq: Option<u64>,
    candidate_at: i64,
    candidate_id: &str,
    stored_seq: Option<u64>,
    stored_at: i64,
    stored_id: &str,
) -> bool {
    match (candidate_seq, stored_seq) {
        (Some(candidate), Some(stored)) => candidate > stored,
        _ => candidate_at > stored_at || (candidate_at == stored_at && candidate_id < stored_id),
    }
}

/// One row of a [`HelmStore::scan_page`] scan: its ordering key always,
/// its payload only when the stored JSON still decodes.
///
/// The split is the whole point. Display data can be skipped (a cache is
/// last-known display data, not authority — see
/// [`HelmStore::cached_sessions`]), but an ordering key cannot: a caller
/// paging this order has to be able to advance past a row it cannot show,
/// or that row becomes a permanent wall in front of everything after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedRow {
    pub key: CacheKey,
    /// `None` for a row whose `info_json` no longer decodes. Logged when it
    /// happens; not an error.
    pub info: Option<SessionInfo>,
}

/// What one [`HelmStore::scan_page`] scan found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CachePage {
    /// Every row scanned, in order — decoded or not (see [`ScannedRow`]).
    pub rows: Vec<ScannedRow>,
    /// Whether the scan stopped because a bound was reached rather than
    /// because the order ran out. Answered by the scan itself (it fetches
    /// one row past the limit) rather than by a second query that could
    /// disagree with it.
    pub more: bool,
    /// The key of the FIRST row this scan did not return, when `more` is
    /// true — the fence a merger must not advance past.
    ///
    /// Load-bearing whenever the scan stops for its BYTE bound, which it can
    /// do having returned fewer rows than asked for. The merge then still
    /// has capacity, so it goes on taking items from the in-memory sources —
    /// and every persisted row between here and wherever it stops is skipped
    /// by a cursor that never named it. `None` when the order simply ran
    /// out, which is the only case in which there is nothing beyond.
    pub frontier: Option<CacheKey>,
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
///   rather than selecting archived rows alone. The fleet total still counts
///   both, so the default view can say how much durable history it hides.
/// - **host, parent, status, profile — EXACT.** Each is an identifier or a
///   value chosen from a finite set
///   the client already has in hand (the hosts list, the status vocabulary,
///   the host's profile catalog), so a substring match would only ever
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

    /// Whether this filter narrows nothing, so a caller may take its
    /// unfiltered fast path (an indexed `COUNT(*)`, a `LIMIT`ed scan)
    /// instead of walking rows to find out.
    pub fn is_empty(&self) -> bool {
        *self == SessionFilter::default().include_archived(true)
    }

    /// The one host this filter admits, if it names one.
    ///
    /// Exposed so the read can narrow its SQL SCOPE rather than decoding
    /// every host's rows and rejecting them in Rust: the host dimension is
    /// the only one the schema can answer by itself (it is an indexed
    /// column, not a field inside the payload), and a fleet-wide scan to
    /// serve "show me this host" is work nothing needs.
    ///
    /// [`Self::matches`] still checks the host independently. The scope is
    /// an optimization; the predicate is the contract, and a caller that
    /// forgot the scope must still get the right answer.
    pub fn host_scope(&self) -> Option<HostId> {
        self.host
    }

    /// The canonical encoding of what this filter selects — the input every
    /// derived identity of a filter is built from.
    ///
    /// Length-prefixed per field rather than delimiter-joined, so no
    /// combination of user text can spell another filter's encoding: a title
    /// of `x|s=running` produces a different string than a title of `x`
    /// beside a status of `running`, which a naive join would not.
    ///
    /// Compared, never parsed. It does NOT travel anywhere — it is unbounded
    /// user text, and a cursor carrying it would grow with the search box
    /// (see [`Self::digest`]).
    pub fn fingerprint(&self) -> String {
        fn field(out: &mut String, tag: char, value: Option<&str>) {
            match value {
                None => out.push_str(&format!("{tag}-;")),
                Some(text) => out.push_str(&format!("{tag}{}:{text};", text.len())),
            }
        }
        let mut out = String::new();
        field(
            &mut out,
            'i',
            Some(if self.include_archived { "1" } else { "0" }),
        );
        field(
            &mut out,
            'h',
            self.host.map(|host| host.to_string()).as_deref(),
        );
        field(&mut out, 'a', self.parent.as_deref());
        field(
            &mut out,
            'd',
            self.directory.as_ref().map(|f| f.raw.as_str()),
        );
        field(&mut out, 't', self.title.as_ref().map(|f| f.raw.as_str()));
        field(&mut out, 'p', self.profile.as_ref().map(|f| f.raw.as_str()));
        field(&mut out, 's', self.status);
        out
    }

    /// A fixed-size, PROCESS-LOCAL name for this filter — what a cursor is
    /// BOUND to, and what a cached matching count is keyed by.
    ///
    /// ## Why a cursor carries this rather than the filter itself
    ///
    /// A resume point is only meaningful within the result set it was taken
    /// from. Replayed against a different filter it names a position in a
    /// sequence it never described, so the walk silently resumes mid-order
    /// and every earlier match is skipped — no error, no gap a client can
    /// see. Binding every cursor (an unfiltered one included) to its filter
    /// is what turns that into a refusal.
    ///
    /// Fixed-size because the alternative is a cursor that grows with the
    /// search box: a first page whose query string already sits near an HTTP
    /// head limit would mint a follow-up cursor nobody could replay.
    ///
    /// ## Keyed, and exactly how much that is worth
    ///
    /// The key is a process-random [`std::collections::hash_map::RandomState`],
    /// so the binding a cursor carries means something only within the process
    /// that minted it: a token cannot be composed off-line, and one process's
    /// cursors are not the next one's.
    ///
    /// It is NOT an authenticator, and calling it unforgeable — as an earlier
    /// version of this comment did — would misdescribe what stands behind it.
    /// `RandomState` is `SipHash-1-3` used as a hash-flooding defense, not as
    /// a MAC; the output is 64 bits, so collisions are reachable by search;
    /// and nothing here is constant-time. The threat this actually answers is
    /// ACCIDENTAL reuse — a stale cursor from a previous walk, a token pasted
    /// from another tab — plus casual tampering on an endpoint that listens on
    /// loopback and serves one user. It buys a probabilistic, process-local
    /// binding, which is proportionate to what a mismatch costs (a page walk
    /// that resumes in the wrong result set, on data the caller may already
    /// read in full). It deliberately remains outside the API's credential
    /// boundary: an authenticated caller can still reuse a cursor accidentally
    /// or edit it.
    ///
    /// The price is that cursors do not survive a helm restart: the new
    /// process has a new key, every old token fails to match, and the answer
    /// is a 400 that says to start a fresh walk. That is the right side of
    /// the trade — a restart drops every host connection and every client
    /// re-reads anyway — and it is the same reason the count cache this keys
    /// is in memory rather than at rest.
    pub fn digest(&self) -> String {
        use std::hash::{BuildHasher, Hasher};

        // ONE key for the process, minted on first use. A key per call would
        // make a digest unreproducible even for the same filter; a constant
        // key would make one process's tokens replayable in every other.
        static KEY: std::sync::OnceLock<std::collections::hash_map::RandomState> =
            std::sync::OnceLock::new();
        let mut hasher = KEY
            .get_or_init(std::collections::hash_map::RandomState::new)
            .build_hasher();
        // The canonical encoding is hashed as ONE byte string, so the
        // length-prefixing that makes it unambiguous carries straight through
        // to the digest.
        hasher.write(self.fingerprint().as_bytes());
        format!("{:016x}", hasher.finish())
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

/// The PERSISTED half of `GET /api/sessions`, coherent within itself: the
/// page and both of its counts, all produced by [`HelmStore::merged_page`]'s
/// single read.
///
/// Not the whole answer, and the distinction matters to anyone reading this
/// as a contract. `crate::aggregate::session_page` merges the in-memory rows
/// of every connected host that reports no identity — hosts this cache holds
/// nothing for — into the page, adds their share to both counts, and only
/// then has something to serve. What this struct guarantees is that helm.db's
/// contribution is one moment's worth: the rows, `total`, `matching` and
/// `generation` all come from one hold of one mutex.
///
/// Bundled rather than returned as a tuple of three separate calls because
/// the bundling IS the contract — see that method for what taking them
/// apart produces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergedRead {
    pub page: CachePage,
    /// Every cached row in the merged view, filter or no filter.
    pub total: u64,
    /// How many of them match the filter — present exactly when this read
    /// COUNTED (see [`MatchingCount`]).
    ///
    /// `None` is never "zero" and never "unknown": it means this read was
    /// told not to count, either because the caller wants no matching claim
    /// at all or because the count it already holds is still true. Which of
    /// the two it was is the caller's own question; [`MergedRead::generation`]
    /// is what answers it.
    pub matching: Option<u64>,
    /// The store's mutation generation, read INSIDE this read's lock hold.
    ///
    /// The tie between a count and the data it describes. Everything in this
    /// struct was produced under one hold of one mutex, so a count taken here
    /// and this number are true of each other by construction; a later read
    /// that finds the same generation is looking at the same rows. See
    /// [`HelmStore::generation`] for why nothing sampled OUTSIDE the lock can
    /// serve this purpose.
    pub generation: u64,
}

/// What [`HelmStore::merged_page`] should do about the matching count.
///
/// ## Why a page read decides this at all
///
/// Counting matches means decoding every row in the scope: there is no index
/// over a JSON payload, and "how many match" is exactly the question a
/// stopping-early scan cannot answer. Done per page, a `limit=1` walk of the
/// fleet is quadratic in it — under the one mutex every other request needs,
/// which is what turns a cost into a stall for everybody.
///
/// So a walk counts ONCE and the caller remembers the number
/// (`crate::aggregate`'s count cache). What it cannot do is decide on its own
/// whether that number is still true: a write can commit between the caller
/// sampling anything and this read starting. [`Self::ComputeUnless`] hands
/// the decision to the only place that can make it safely — inside the lock
/// hold that produces the page.
///
/// ## The count and the page are ONE pass
///
/// When this read does count, it counts in the same walk that collects the
/// page rather than in a scan of its own. The shape this replaced decoded the
/// whole scope twice for a zero-match request, both times under the mutex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchingCount {
    /// Count, unconditionally — a filtered read with nothing to reuse.
    Compute,
    /// Count only if the store has moved since this generation. The caller
    /// holds a count taken at it and that count still stands otherwise, in
    /// which case [`MergedRead::matching`] comes back `None`.
    ComputeUnless(u64),
    /// Do not count at all. An UNFILTERED listing makes no matching claim —
    /// see [`HelmStore::merged_page`] — so there is nothing to compute.
    Skip,
}

/// Decode one cached row's payload, or say why it cannot be trusted AS THAT
/// ROW.
///
/// The single validation the page and the matching count are both decided
/// by, and the sharing is load-bearing rather than tidy: they must agree
/// exactly about which rows are usable, or "N matching" would count rows the
/// page refuses to display and no page walk could ever reach N. Since
/// [`HelmStore::scan_page`] answers both in one walk, that agreement is now
/// structural rather than a discipline two functions have to keep.
///
/// Two ways a row fails, and the second is the subtle one. An undecodable
/// payload is obvious. A payload that DECODES but whose own `id` or
/// `created_at` disagrees with the columns it is filed under is poison too:
/// the columns are what every order, cursor, and lookup here is built on, so
/// showing such a row would list it under one identity and route it under
/// another.
///
/// The `Err` carries a reason string for the caller to log. It is logged only
/// where the PAGE would have shown the row: a counting walk covers the whole
/// scope, and one warning per unreadable row per keystroke in a search box is
/// a log the user writes by typing.
fn usable_cached_session(key: &CacheKey, json: &str) -> Result<SessionInfo, String> {
    let info: SessionInfo = serde_json::from_str(json)
        .map_err(|error| format!("its info_json no longer decodes: {error}"))?;
    if info.id != key.session_id || info.created_at != key.created_at {
        return Err(format!(
            "its payload names session {:?} at {}, but it is filed as {:?} at {}",
            info.id, info.created_at, key.session_id, key.created_at
        ));
    }
    Ok(info)
}

/// Whether `key` comes STRICTLY AFTER `after` in the merged order
/// (`created_at` descending, then `session_id` ascending, then `host_id`
/// ascending).
///
/// The Rust twin of the three-way disjunction [`HelmStore::scan_page`] hands
/// SQLite, and it exists because a COUNTING scan cannot use that predicate at
/// all: it has to see the rows before the cursor in order to count them. The
/// two spellings must agree exactly — a page whose resume test disagreed with
/// its own ORDER BY would skip or repeat rows, which is the one thing a
/// cursor may never do.
fn follows(key: &CacheKey, after: &CacheKey) -> bool {
    (
        std::cmp::Reverse(key.created_at),
        key.session_id.as_str(),
        key.host,
    ) > (
        std::cmp::Reverse(after.created_at),
        after.session_id.as_str(),
        after.host,
    )
}

/// One row's position in the cross-host merged order — the resume point
/// [`HelmStore::merged_page`] pages from.
///
/// All three components, in this order: `created_at` DESCENDING, then
/// `session_id` ascending, then `host_id` ascending. The third is what makes
/// the order TOTAL rather than merely usually-total (see [`apply_schema`]'s
/// version 3), and a resume point over a non-total order can skip or repeat
/// rows — the one thing a cursor must never do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    pub created_at: i64,
    pub session_id: String,
    pub host: HostId,
}

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
    /// A call named a [`HostId`] no row currently holds — including one
    /// that once existed and was removed.
    #[error("host {0} does not exist")]
    HostNotFound(HostId),
    /// A call tried to edit or remove the reserved local row through the
    /// ssh-host management API. The local row is not user management
    /// surface at all (PLAN_M6.md item 4): it is synthesized once by
    /// [`HelmStore::open`] and otherwise untouchable by every ssh-host
    /// MANAGEMENT mutation — but not by identity ([`HelmStore::record_first_contact`],
    /// [`HelmStore::adopt_identity`]) or cache
    /// ([`HelmStore::replace_host_sessions`]) operations, which the local
    /// row needs on the same terms as any other host (it "learns its
    /// identity the same way" — PLAN_M6.md item 3).
    #[error("the local host cannot be edited or removed")]
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
    /// How many times a committed write actually CHANGED a cached row — the
    /// token that says whether a count taken earlier still describes the data
    /// (see [`MatchingCount`]).
    ///
    /// Advanced INSIDE the [`Self::conn`] lock hold that committed the
    /// write, and read inside the lock hold that produced a page, which is
    /// what makes `(generation, page, total, matching)` mutually coherent by
    /// construction. Anything advanced after the lock is released — a fleet
    /// revision published by a caller that has already committed, say —
    /// cannot qualify a read, because a second write can land in the gap.
    ///
    /// "Changed" is decided by comparing STORED BYTES inside the writing
    /// transaction, never by a caller's opinion that it wrote something. A
    /// refresh writes back an identical row set every few seconds per host in
    /// a settled fleet, and a counter that moved for those would make every
    /// page of every walk recount — which is the cost this exists to avoid.
    /// Rows that are byte-identical support exactly the same counts, so
    /// standing still there is sound rather than merely convenient.
    ///
    /// Process-local and reset by a restart, deliberately: its only consumer
    /// is an in-memory cache that a restart empties too, so there is nothing
    /// a reused number could wrongly qualify. A durable counter would have to
    /// be written on the same transaction as every mutation and read back on
    /// every page, which is a row of contention bought for nothing.
    generation: Arc<AtomicU64>,
    /// How many times a read has actually walked the scope to COUNT matches
    /// — instrumentation for the tests that pin "one count per walk, one
    /// recount per invalidating write".
    ///
    /// A production counter rather than a test-only hook because the property
    /// it measures is the design (see [`MatchingCount`]), and a hook compiled
    /// only under `cfg(test)` would let the shape it guards drift in a build
    /// nobody tests. One relaxed increment per filtered read costs nothing
    /// against the scan it is counting.
    counting_passes: Arc<AtomicU64>,
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
///   to helm.db rather than to the wire. It is helm state by construction:
///   the supervisor owns the catalog and knows nothing about which profile
///   a particular user last picked, and a remembered default is per-HOST
///   because a profile id only means anything on the host that minted it.
/// - 6: `remembered_profiles.host_identity`, which makes that default
///   identity-bound AT REST rather than only at write time. A version-5 row
///   records no identity and therefore cannot be validated against the
///   install the host currently is — and since starter profile ids collide
///   across installs by construction, an unvalidatable default RESOLVES on a
///   successor install rather than merely dangling. The migration drops the
///   old rows for that reason; see [`HelmStore::remembered_profile`] for the
///   read-time check the column exists to serve.
/// - 7: PLAN_M7.md item 3 — web-token authentication and device sessions.
/// - 8: PLAN_M7.md item 4 — provenance for the remembered profile default,
///   so a completed drain can advance it without letting an older snapshot
///   overwrite a newer successful create.
/// - 9: a supervisor-local creation sequence for provenance ordering. Older
///   cached supervisors omit it, so the timestamp/id ordering remains the
///   compatibility fallback until a sequenced observation arrives.
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
        tx.execute_batch(
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
             -- new shape. HelmStore::cached_sessions/cached_sessions_all's
             -- skip-and-log read posture is the last line of defense for
             -- whatever this contract does not catch ahead of time — see
             -- those methods' own docs.
             CREATE TABLE session_cache (
                 host_id    INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
                 session_id TEXT NOT NULL,
                 -- Extracted from the stored SessionInfo JSON at write time
                 -- (HelmStore::replace_host_sessions) rather than re-parsed
                 -- from it on every read — see that method's and
                 -- cached_sessions's own docs for why ordering must never
                 -- depend on decoding every blob.
                 created_at INTEGER NOT NULL,
                 -- The durable, potentially STALE serialized SessionInfo
                 -- itself — see this table's own comment above for the
                 -- cross-upgrade format contract this column is bound by.
                 info_json  TEXT NOT NULL,
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
             CREATE UNIQUE INDEX session_cache_one_owner
                 ON session_cache (session_id);
             -- Serves the merged page query (HelmStore::merged_page): the
             -- full ordering key, in order, so a page is an index range
             -- scan with no sort step and no rows touched beyond the page.
             -- host_id is in the key even though session_id is already
             -- unique above, so the order stays TOTAL even against a
             -- database whose one-owner index is absent (a downgrade, a
             -- hand edit) -- a pagination cursor over a non-total order can
             -- skip or repeat rows, which is precisely what a resume point
             -- must never do.
             CREATE INDEX session_cache_order
                 ON session_cache (created_at DESC, session_id ASC, host_id ASC);
             -- Serves HelmStore::cached_sessions's per-host read
             -- separately: session_cache_order above has no host_id prefix,
             -- so \"WHERE host_id = ? ORDER BY created_at DESC, session_id
             -- ASC\" against it alone would still have to walk every row
             -- from every host to filter, not just this host's. Leading
             -- with host_id here turns that into an index range scan
             -- instead.
             CREATE INDEX session_cache_by_host_order
                 ON session_cache (host_id, created_at DESC, session_id ASC);
             -- The last profile a session was created from, per host
             -- (schema version 5). At most one row per host by
             -- construction: host_id IS the primary key, because
             -- \"remembered default\" is a single value and a table that
             -- could hold two would need a rule for which one wins.
             -- CASCADE for session_cache's reason: forgetting a host
             -- forgets everything the helm knew about it, in one statement.
             -- The profile id is NOT a foreign key to anything -- the
             -- catalog lives on the supervisor, so this side cannot
             -- validate it and deliberately does not try: a remembered
             -- default naming a profile that has since been deleted is a
             -- state the product HAS (SPEC.md's ask-don't-guess fallback),
             -- and the client resolves it against the catalog it is served
             -- in the same reply.
             -- host_identity is the identity the host was reporting when
             -- the default was recorded (NULL for a host that reports
             -- none), and it is what makes the row identity-bound AT REST
             -- rather than merely at write time. Starter profile ids
             -- collide across installs by construction, so a row that
             -- outlived the install it was recorded against would RESOLVE
             -- on the successor and be offered back as the user's own last
             -- choice. Adoption and retarget delete the row outright; this
             -- column is what refuses any row that escapes both.
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
             PRAGMA user_version = 9;",
        )
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
        // (`cached_sessions`/`merged_page`) SKIPS an undecodable row and
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
        // The remembered default becomes identity-bound at rest. A
        // version-5 row records only (host_id, profile_id), so there is
        // nothing in it to validate against the install the host currently
        // is — and "no identity recorded" cannot be told apart from "this
        // host reports no identity", which is a value the new column
        // legitimately stores.
        //
        // So the old rows GO rather than being carried forward with a NULL
        // that would validate against every identity-less host. The cost is
        // one create dialog per host asking which profile to use instead of
        // defaulting — exactly SPEC.md's ask-don't-guess fallback, which is
        // the safe direction — and the next profile-backed create restores
        // the default with an identity attached.
        //
        // Rebuilt rather than `ALTER TABLE ... ADD COLUMN`, so the stored
        // DDL is byte-identical to the fresh-create branch's (pinned by
        // `a_migrated_database_matches_a_freshly_created_one`).
        tx.execute_batch(
            "DROP TABLE remembered_profiles;
             CREATE TABLE remembered_profiles (
                 host_id       INTEGER PRIMARY KEY
                               REFERENCES hosts (id) ON DELETE CASCADE,
                 profile_id    TEXT NOT NULL,
                 host_identity TEXT
             ) STRICT;
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
            generation: Arc::new(AtomicU64::new(0)),
            counting_passes: Arc::new(AtomicU64::new(0)),
        })
    }

    /// How many times this store has walked a scope to count matches — see
    /// [`Self::counting_passes`].
    #[cfg(test)]
    pub fn counting_passes(&self) -> u64 {
        self.counting_passes.load(Ordering::Relaxed)
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

    /// Every registered host, local row included, ordered by [`HostId`] —
    /// which, thanks to `AUTOINCREMENT`, is also insertion order: the local
    /// row first (minted at the first-ever `open`), then ssh rows in the
    /// order they were added.
    ///
    /// A row whose `kind` fails [`HostKind::from_column`] FAILS this whole
    /// call rather than being skipped — the opposite posture from
    /// [`Self::cached_sessions`]/[`Self::cached_sessions_all`]'s
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
                    "SELECT id, kind, destination, remote_farhelm, remote_state_dir, \
                     host_identity FROM hosts ORDER BY id ASC",
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
                    ))
                })
                .context("querying hosts")?
                .collect::<Result<_, _>>()
                .context("reading host rows")?;
            raw.into_iter()
                .map(
                    |(id, kind, destination, remote_farhelm, remote_state_dir, host_identity)| {
                        Ok(HostRow {
                            id,
                            kind: HostKind::from_column(&kind)?,
                            destination,
                            remote_farhelm,
                            remote_state_dir,
                            host_identity,
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
            let conn = conn.lock().expect("helm db mutex poisoned");
            let inserted = conn
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
            Ok(conn.last_insert_rowid())
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
    /// ## What a retarget FORGETS
    ///
    /// The remembered default profile, whenever the destination actually
    /// moved. Retargeting points a registry row at a different install, and a
    /// profile id means nothing away from the supervisor that minted it —
    /// except that starter ids collide across installs by construction, so
    /// the id would RESOLVE on the new endpoint and be offered back as the
    /// user's own last choice. The identity binding alone does not cover this
    /// case: an identity-less host retargeted to another identity-less
    /// install matches `NULL` against `NULL` and the row would survive.
    ///
    /// A byte-identical destination update KEEPS it, deliberately: that is a
    /// caller re-affirming what the row already says (a resubmitted form, a
    /// reconcile), and forgetting a preference over a write that changed
    /// nothing would be a user-visible loss with no cause behind it.
    ///
    /// The learned identity and the session cache are NOT cleared here, and
    /// the asymmetry is the point: those are facts about an install, and the
    /// next handshake against the new endpoint decides what happens to them
    /// (a mismatch freezes, and adoption is what purges). A remembered
    /// default is a PREFERENCE, and there is no later moment at which anyone
    /// re-examines it.
    ///
    /// That forgetting is only complete if no write can land after it. This
    /// transaction fences its own readers, but the write it races is a
    /// `remember_profile_default` that has ALREADY passed its claim check and
    /// is about to commit — a claim the retarget invalidates only afterwards,
    /// through the manager's reconcile. Callers therefore hold the host's
    /// write lock across both (`crate::hosts::set_destination`, and
    /// `ConnectionManager::host_write_lock` for why that is the right lock).
    /// The identity binding cannot substitute: an identity-less host
    /// retargeted to another identity-less install matches `NULL` against
    /// `NULL` on every later read.
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
            // The current destination comes back with the kind, in the same
            // read and therefore the same transaction: "did this retarget
            // actually move the row" decides whether the remembered default
            // survives, and a second read for it could straddle another
            // writer.
            let current: Option<(String, Option<String>)> = tx
                .query_row(
                    "SELECT kind, destination FROM hosts WHERE id = ?1",
                    rusqlite::params![host],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .context("looking up host before updating its destination")?;
            let current = current
                .map(|(kind, destination)| {
                    HostKind::from_column(&kind).map(|kind| (kind, destination))
                })
                .transpose()?;
            match current {
                None => Err(anyhow::Error::new(HostStoreError::HostNotFound(host))),
                Some((HostKind::Local, _)) => {
                    Err(anyhow::Error::new(HostStoreError::LocalHostImmutable))
                }
                Some((HostKind::Ssh, previous)) => {
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
                    // Only when the row genuinely moved — see this method's
                    // docs for why a re-affirming write keeps the preference
                    // and a real retarget must not.
                    if previous.as_deref() != Some(destination.as_str()) {
                        tx.execute(
                            "DELETE FROM remembered_profiles WHERE host_id = ?1",
                            rusqlite::params![host],
                        )
                        .context("forgetting the retargeted host's remembered default profile")?;
                    }
                    tx.commit().context("committing destination update")?;
                    Ok(())
                }
            }
        })
        .await
        .context("update ssh destination task panicked")?
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
        let generation = Arc::clone(&self.generation);
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
                    // The `ON DELETE CASCADE` took this host's cached rows
                    // with it, so every matching count taken before now
                    // describes sessions that are gone. Bumped while the
                    // connection lock is still held — see
                    // [`Self::generation`].
                    generation.fetch_add(1, Ordering::Release);
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
    /// erasing everything the OLD install left behind — that host's
    /// `session_cache` rows and its remembered default profile — in the SAME
    /// transaction. PLAN_M6.md item 4's user-initiated adoption of an
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
    /// The REMEMBERED DEFAULT goes for a sharper version of the same reason,
    /// and it is the one an adoption is most likely to get wrong. Profile ids
    /// are minted per supervisor AND every fresh supervisor seeds the same
    /// starter profiles, so an id recorded against the superseded install
    /// does not merely go stale on the successor — it RESOLVES there, to a
    /// profile the user never chose, offered back as their own last choice.
    /// Purging costs one create dialog that asks instead of guessing, which
    /// is exactly SPEC.md's fallback.
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
        let generation = Arc::clone(&self.generation);
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
            tx.execute(
                "UPDATE hosts SET host_identity = ?2 WHERE id = ?1",
                rusqlite::params![host, new],
            )
            .context("adopting new host identity")?;
            tx.execute(
                "DELETE FROM session_cache WHERE host_id = ?1",
                rusqlite::params![host],
            )
            .context("purging the superseded identity's cached sessions")?;
            tx.execute(
                "DELETE FROM remembered_profiles WHERE host_id = ?1",
                rusqlite::params![host],
            )
            .context("purging the superseded identity's remembered default profile")?;
            tx.commit().context("committing identity adoption")?;
            generation.fetch_add(1, Ordering::Release);
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
    /// `created_at` is extracted from each `entry` and stored as its own
    /// column rather than left for a reader to re-parse from `info_json` —
    /// see [`Self::cached_sessions`]'s docs for why that split is load-
    /// bearing, not merely tidy.
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
    ) -> anyhow::Result<CacheReplacement> {
        let conn = Arc::clone(&self.conn);
        let generation = Arc::clone(&self.generation);
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
            // column of its own (the ordering key every page and cursor is
            // built on), so a row whose payload is unchanged while its
            // timestamp is repaired IS a change — and reporting otherwise
            // would starve the feed of exactly the reordering a client needs
            // to re-read for.
            //
            // ONE map, consumed as the rewrite goes: entries are removed as
            // they are matched, so this holds the host's cache once rather
            // than twice at the peak. What remains at the end is what
            // DISAPPEARED, which no per-row comparison of the new list could
            // notice on its own. The transient cost is one host's slice at
            // the refresh ceiling (`crate::manager::REFRESH_SESSION_CAP`
            // rows, `REFRESH_BYTE_CAP` bytes), which is the same data the
            // caller already holds in `entries`.
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
                        "INSERT INTO session_cache (host_id, session_id, created_at, info_json) \
                         VALUES (?1, ?2, ?3, ?4) \
                         ON CONFLICT (session_id) DO NOTHING",
                        rusqlite::params![host, entry.id, entry.created_at, json],
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
                                candidate.0,
                                candidate.1,
                                &candidate.2,
                                *creation_seq,
                                *created_at,
                                session_id,
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
            // A drain is the authoritative observation that a profile was
            // actually used. Carry its session ordering key beside the
            // preference so a delayed, older drain cannot roll the default
            // backward after a newer create or refresh has already landed.
            let mut default_changed = false;
            let remembered: Option<RememberedProfileRow> = tx
                .query_row(
                    "SELECT profile_id, host_identity, source_creation_seq, \
                            source_created_at, source_session_id \
                     FROM remembered_profiles WHERE host_id = ?1",
                    rusqlite::params![host],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()
                .context("reading remembered profile provenance")?;
            let source_disappeared = remembered
                .as_ref()
                .and_then(|(_, _, _, _, session_id)| session_id.as_deref())
                .is_some_and(|session_id| !present_session_ids.contains(session_id));
            if source_disappeared && newest_profile_source.is_none() {
                default_changed = tx
                    .execute(
                        "DELETE FROM remembered_profiles WHERE host_id = ?1",
                        rusqlite::params![host],
                    )
                    .context("clearing a remembered profile whose source disappeared")?
                    != 0;
            } else if let Some((creation_seq, created_at, session_id, profile_id)) =
                newest_profile_source
            {
                let advances = source_disappeared
                    || match &remembered {
                        None => true,
                        Some((
                            _,
                            stored_identity,
                            stored_seq,
                            Some(stored_at),
                            Some(stored_id),
                        )) if stored_identity.as_deref() == Some(identity.as_str()) => {
                            source_is_newer(
                                creation_seq,
                                created_at,
                                &session_id,
                                stored_seq.and_then(|seq| u64::try_from(seq).ok()),
                                *stored_at,
                                stored_id,
                            )
                        }
                        // A v7 -> v8 migrated preference has no source at
                        // all. The first post-upgrade drain cannot prove its
                        // newest SURVIVING session is newer than the session
                        // the user actually chose before upgrading: that
                        // source may already have been deleted. Keep the
                        // opaque preference until a direct create records a
                        // real source, after which ordinary drain ordering
                        // applies again.
                        Some((_, stored_identity, None, None, None))
                            if stored_identity.as_deref() == Some(identity.as_str()) =>
                        {
                            false
                        }
                        Some(_) => true,
                    };
                if advances {
                    default_changed = remembered.as_ref().is_none_or(
                        |(stored_profile, stored_identity, _, _, _)| {
                            stored_profile != &profile_id
                                || stored_identity.as_deref() != Some(identity.as_str())
                        },
                    );
                    tx.execute(
                        "INSERT INTO remembered_profiles (\
                             host_id, profile_id, host_identity, source_creation_seq, \
                             source_created_at, source_session_id\
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                         ON CONFLICT (host_id) DO UPDATE SET \
                             profile_id = excluded.profile_id, \
                             host_identity = excluded.host_identity, \
                             source_creation_seq = excluded.source_creation_seq, \
                             source_created_at = excluded.source_created_at, \
                             source_session_id = excluded.source_session_id",
                        rusqlite::params![
                            host,
                            profile_id,
                            identity,
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
            if changed {
                generation.fetch_add(1, Ordering::Release);
            }
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
    /// ingress point exactly as a drain's rows are, and an id this side
    /// cannot build a replayable cursor over must not enter the cache
    /// through either.
    ///
    /// Returns whether the stored row actually CHANGED — the same
    /// changed-only rule [`Self::replace_host_sessions`] answers, applied to
    /// one row. A retried create that re-records a byte-identical session is
    /// a successful write that invalidates nothing.
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
        let generation = Arc::clone(&self.generation);
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
            // everything except liveness — see
            // `crate::manager::merged_status` for why `Unknown` must not
            // erase a definite status, and why the previous value is kept
            // even at the cost of being briefly stale.
            let mut entry = entry;
            if let Some((_, _, previous)) = &claimed
                && let Ok(previous) = serde_json::from_str::<SessionInfo>(previous)
            {
                entry.status = crate::manager::merged_status(&previous.status, entry.status);
            }
            let entry = &entry;
            let json = serde_json::to_string(entry).context("serializing cached session")?;
            tx.execute(
                "INSERT INTO session_cache (host_id, session_id, created_at, info_json) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT (session_id) DO UPDATE SET \
                     created_at = excluded.created_at, info_json = excluded.info_json",
                rusqlite::params![host, entry.id, entry.created_at, json],
            )
            .context("seeding a cached session")?;
            tx.commit().context("committing cache seed")?;
            // Compared against BOTH stored halves this host already held,
            // AFTER the status merge above: the merge is what makes a
            // restart's `Unknown`-carrying reply a no-op for an unchanged
            // session, and comparing before it would report a change the row
            // does not actually show. The timestamp is in the comparison for
            // the same reason it is in the wholesale write's — it is the
            // ordering column, not a copy of something in the payload.
            let changed = match &claimed {
                Some((_, created_at, stored)) => {
                    *created_at != entry.created_at || stored.as_str() != json.as_str()
                }
                None => true,
            };
            if changed {
                generation.fetch_add(1, Ordering::Release);
            }
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
        let generation = Arc::clone(&self.generation);
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
            // says anything different than it did before — including the
            // generation, which is why the bump is conditional too.
            if removed > 0 {
                generation.fetch_add(1, Ordering::Release);
            }
            Ok(removed > 0)
        })
        .await
        .context("forget session task panicked")?
    }

    /// One host's cached sessions, in the wire order (`created_at`
    /// descending, `id` ascending — the same total order PLAN_M6.md item 1
    /// defines for the supervisor's own pagination) — the order a stale
    /// list is served in when a host is unreachable.
    ///
    /// Reads the `created_at`/`session_id` COLUMNS for ordering, never the
    /// parsed `info_json` — the whole reason [`Self::replace_host_sessions`]
    /// extracts them at write time. Re-deriving order from a parsed blob on
    /// every read would mean decoding every cached session just to sort
    /// them, on a path this store cannot know is cold (a helm restart with
    /// every host still down serves ONLY from this cache, per PLAN_M6.md's
    /// testing decisions) — the columns make this a plain indexed
    /// `ORDER BY` against `session_cache_by_host_order` instead (see
    /// [`apply_schema`]; the host_id-less `session_cache_order` index
    /// serves [`Self::cached_sessions_all`]'s cross-host read, not this
    /// one).
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
                    "SELECT session_id, info_json FROM session_cache \
                     WHERE host_id = ?1 ORDER BY created_at DESC, session_id ASC",
                )
                .context("preparing cached session query")?;
            let rows: Vec<(String, String)> = stmt
                .query_map(rusqlite::params![host], |r| Ok((r.get(0)?, r.get(1)?)))
                .context("querying cached sessions")?
                .collect::<Result<_, _>>()
                .context("reading cached session rows")?;
            Ok(rows
                .into_iter()
                .filter_map(|(session_id, json)| match serde_json::from_str(&json) {
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

    /// ONE PAGE of the cross-host merged order, resuming strictly after
    /// `after` — the substrate of PLAN_M6.md item 5's served session list.
    ///
    /// The whole point is that it is a PAGE: the resume predicate, the row
    /// limit, and the byte bound all apply during ONE indexed scan, so a
    /// poll reads and JSON-decodes only the rows it is about to return. The
    /// shape this replaced loaded and deserialized every session on every
    /// host on every poll, which made a full page walk quadratic in the
    /// fleet's size.
    ///
    /// `hosts` scopes the read to the hosts that currently have actors,
    /// which is what "the merged view" means everywhere else; a cache row
    /// whose host has none is not served and must not be counted (see
    /// [`Self::count_rows`], which takes the same scope for exactly that
    /// reason). An empty `hosts` is an empty answer rather than "all",
    /// deliberately: the degenerate reading is the dangerous one.
    ///
    /// ## Every scanned row comes back, decoded or not
    ///
    /// This is the fix for a real and permanent data-loss bug, so it is
    /// worth stating as a contract rather than as an implementation note. A
    /// row whose `info_json` no longer decodes is SKIPPED for display (the
    /// skip-and-log posture [`Self::cached_sessions`] documents) but is
    /// still reported here, as a [`ScannedRow`] with no payload — because
    /// its ORDERING KEY is what the caller's cursor has to advance past.
    /// Returning only decoded rows meant a poisoned row at a page boundary
    /// left the cursor pointing before it forever: the next page re-scanned
    /// the same poisoned row, skipped it again, and every row after it in
    /// the whole fleet became permanently unreachable. A fully poisoned
    /// page did the same thing more obviously — an empty page with no
    /// continuation, in the middle of a list.
    ///
    /// ## Two bounds, both applied while scanning
    ///
    /// `limit` bounds rows RETURNED — which, with no filter, is the same as
    /// rows scanned, poisoned ones included (otherwise a run of poisoned
    /// rows would make one request walk the whole table); see the filtering
    /// section below for what changes when a filter is set. `byte_budget`
    /// bounds the stored bytes carried, measured on the raw
    /// `info_json` this scan already holds rather than by re-serializing.
    /// The byte bound is a WORK bound: it stops this scan from decoding
    /// thousands of fat blobs the reply could never carry. The reply's own
    /// budget is the caller's, applied to the merged page (see
    /// `crate::aggregate::PAGE_BYTE_BUDGET`); the two use the same constant
    /// but answer different questions, and this one is deliberately the
    /// looser of the pair — it may over-deliver by a row, never under.
    ///
    /// At least one row is always scanned regardless of the byte bound: a
    /// single blob larger than the budget must still make progress, or the
    /// walk stalls on it forever.
    ///
    /// ## Filtering happens BEFORE the page cut
    ///
    /// A non-empty `filter` (PLAN_M6_75.md item 5) narrows what counts as a
    /// row of this page: `limit` and `byte_budget` then bound MATCHING rows,
    /// and non-matching ones are stepped over without consuming either. That
    /// ordering is the whole reason filtering is server-side at all — a
    /// client filtering the page it was handed would show "3 of 500" while
    /// hiding matches that sit past the cut, and no amount of paging would
    /// reconcile the two.
    ///
    /// The cost is stated rather than hidden: with a filter set, this scan
    /// is no longer bounded by `limit` rows. It walks the order until it has
    /// filled the page or run out of rows, so a filter that matches nothing
    /// reads every row in scope. That is inherent to answering "which rows
    /// match" over a payload SQLite is not indexing, and it is bounded in
    /// practice by the cache itself (`crate::manager::REFRESH_SESSION_CAP`
    /// per host). The unfiltered path keeps its `LIMIT` and is untouched.
    ///
    /// A row whose payload does not decode is TAKEN either way — the cursor
    /// contract above outranks the filter, since a row nobody can judge must
    /// still be a row the walk can get past — but it is never COUNTED as
    /// matching, because claiming a match for a payload this build cannot
    /// read would be inventing one.
    ///
    /// ## Counting rides along, when it is wanted
    ///
    /// With `count` set this same walk also answers "how many rows in scope
    /// match", and that fusion is the point rather than a convenience: the
    /// shape it replaced ran a counting scan and then a paging scan, so a
    /// zero-match `limit=1` request decoded the whole scope TWICE under the
    /// one mutex. One decode per row per request is the floor for an exact
    /// count, and this is it.
    ///
    /// Counting changes what the SQL may do, in one direction: a count has to
    /// see the rows BEFORE the resume point too, so the resume predicate
    /// moves out of the `WHERE` clause and into the loop. The page still
    /// contains exactly the rows that follow the cursor.
    ///
    /// Poisoned rows are reported only where the PAGE would have shown them.
    /// A count walks the whole scope, and one warning per unreadable row per
    /// keystroke in a search box is a log the user writes by typing.
    ///
    /// Takes a borrowed connection rather than `&self` because it is half of
    /// [`Self::merged_page`]'s single read and must run inside that read's
    /// transaction: a page fetched under its own lock hold could describe a
    /// different moment than the counts reported beside it.
    fn scan_page(
        conn: &Connection,
        hosts: &[HostId],
        after: Option<CacheKey>,
        limit: usize,
        byte_budget: usize,
        filter: &SessionFilter,
        count: bool,
    ) -> anyhow::Result<(CachePage, Option<u64>)> {
        if hosts.is_empty() {
            return Ok((CachePage::default(), count.then_some(0)));
        }
        // Built rather than constant because the host scope is an IN-list
        // whose arity varies; every value is still bound, never interpolated.
        let placeholders = host_placeholders(hosts.len(), 4);
        let resume = match (&after, count) {
            (None, _) | (Some(_), true) => "",
            // The three-way disjunction IS the strict-successor test for a
            // composite key, written out because SQLite has no row-value
            // comparison this code can rely on across every bundled version.
            // `created_at` is DESCENDING, so "after" means smaller.
            (Some(_), false) => {
                " AND (created_at < ?1                      OR (created_at = ?1 AND session_id > ?2)                      OR (created_at = ?1 AND session_id = ?2 AND host_id > ?3))"
            }
        };
        // One row past the limit, so "is there more" is answered by the scan
        // itself rather than by a second query that could disagree with it.
        // Neither a FILTERED scan nor a COUNTING one can use that bound — how
        // many rows either must read is exactly what it is trying to find out
        // — so they walk the order and stop themselves in the loop below.
        let fetch = limit.saturating_add(1);
        let bound_rows = if filter.is_empty() && !count {
            format!(" LIMIT {fetch}")
        } else {
            String::new()
        };
        let sql = format!(
            "SELECT host_id, session_id, created_at, info_json FROM session_cache \
             WHERE host_id IN ({placeholders}){resume} \
             ORDER BY created_at DESC, session_id ASC, host_id ASC{bound_rows}"
        );
        let mut stmt = conn
            .prepare(&sql)
            .context("preparing the merged cache page query")?;
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        // Bound 1..=3 unconditionally so the placeholder numbering in
        // `resume` is stable whether or not it is present; unused binds are
        // harmless, an off-by-one in the host list is not.
        let placeholder_key = CacheKey {
            created_at: 0,
            session_id: String::new(),
            host: 0,
        };
        let bind_key = after.as_ref().unwrap_or(&placeholder_key);
        params.push(Box::new(bind_key.created_at));
        params.push(Box::new(bind_key.session_id.clone()));
        params.push(Box::new(bind_key.host));
        for host in hosts {
            params.push(Box::new(*host));
        }
        let bound: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut query = stmt
            .query(bound.as_slice())
            .context("querying the merged cache page")?;

        let mut page = CachePage::default();
        let mut matching = 0u64;
        let mut bytes = 0usize;
        // Set once the page has taken everything it will take. The walk goes
        // on from there only while there is still counting to do.
        let mut page_closed = false;
        while let Some(row) = query.next().context("reading merged cache page rows")? {
            let host: HostId = row.get(0).context("reading a cached row's host")?;
            let session_id: String = row.get(1).context("reading a cached row's id")?;
            let created_at: i64 = row.get(2).context("reading a cached row's time")?;
            let json: String = row.get(3).context("reading a cached row's payload")?;
            let key = CacheKey {
                created_at,
                session_id,
                host,
            };
            // ONE validation predicate for both answers (see
            // [`usable_cached_session`]): a row this scan would refuse to
            // show must not be a row the count claims as a match, or "N
            // matching" would promise pages that can never display N rows.
            let usable = usable_cached_session(&key, &json);
            if count && usable.as_ref().is_ok_and(|info| filter.matches(host, info)) {
                matching = matching.saturating_add(1);
            }
            // The resume test, for the counting walk whose SQL carries none.
            if count && after.as_ref().is_some_and(|after| !follows(&key, after)) {
                continue;
            }
            if page_closed {
                if count {
                    continue;
                }
                break;
            }
            let info = match usable {
                Ok(info) => Some(info),
                Err(reason) => {
                    tracing::warn!(
                        host,
                        session_id = key.session_id.as_str(),
                        reason = reason.as_str(),
                        "skipping a cached session that cannot be trusted as this row"
                    );
                    None
                }
            };
            // Filtered out BEFORE the page's own cuts, so a non-matching row
            // costs neither a row slot nor a byte of the budget — that is
            // what "the predicate applies before the page cut" means
            // concretely. A row that could not be decoded is never filtered
            // out: nothing can judge it, and the cursor still has to get past
            // it.
            if let Some(info) = &info
                && !filter.matches(host, info)
            {
                continue;
            }
            // Both cuts leave the same mark: `more`, plus the key of the
            // first row a caller has NOT been shown. That key is the fence a
            // merge needs — with a filter set it is the first MATCHING row
            // withheld, which is the only fence that means anything to a
            // merge that would not show the others either — and the row
            // itself is not part of the page.
            let over_rows = page.rows.len() == limit;
            let over_bytes =
                !page.rows.is_empty() && bytes.saturating_add(json.len()) > byte_budget;
            if over_rows || over_bytes {
                page.more = true;
                page.frontier = Some(key);
                page_closed = true;
                if count {
                    continue;
                }
                break;
            }
            bytes = bytes.saturating_add(json.len());
            page.rows.push(ScannedRow { key, info });
        }
        Ok((page, count.then_some(matching)))
    }

    /// How many sessions the merged view holds across `hosts` — the `total`
    /// a page reports.
    ///
    /// A separate cheap query rather than a by-product of the page, because
    /// the page deliberately stops at its limit and therefore cannot know.
    /// `COUNT(*)` over an indexed IN-list touches no `info_json` at all,
    /// which is what keeps "how many are there" from costing what "show me
    /// all of them" used to.
    ///
    /// Counts ROWS, including any whose payload no longer decodes. That is a
    /// deliberate, documented divergence from the page and from the matching
    /// count, both of which skip them: a total is an answer about the fleet,
    /// and quietly shrinking it to hide a corrupt row would make "showing 4
    /// of 5" read as data loss rather than as the one unshowable entry it is.
    ///
    /// Borrows a connection rather than taking `&self`, so it runs inside
    /// [`Self::merged_page`]'s read transaction. There is deliberately no
    /// standalone async wrapper: production has exactly one reason to ask how
    /// big the merged view is — to answer `GET /api/sessions` — and that
    /// answer must come from the same moment as the page beside it.
    fn count_rows(conn: &Connection, hosts: &[HostId]) -> anyhow::Result<u64> {
        if hosts.is_empty() {
            return Ok(0);
        }
        let placeholders = host_placeholders(hosts.len(), 1);
        let sql = format!("SELECT COUNT(*) FROM session_cache WHERE host_id IN ({placeholders})");
        let params: Vec<Box<dyn rusqlite::ToSql>> = hosts
            .iter()
            .map(|host| Box::new(*host) as Box<dyn rusqlite::ToSql>)
            .collect();
        let bound: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        // `COUNT(*)` is non-negative by definition, so the widening below is
        // a widening rather than a clamp that could hide anything. rusqlite
        // has no `u64` column type (SQLite integers are signed), so the cast
        // is where the two type systems meet.
        let count: i64 = conn
            .query_row(&sql, bound.as_slice(), |r| r.get(0))
            .context("counting the merged cache")?;
        Ok(count as u64)
    }

    /// The page AND both of its counts, from ONE read — what
    /// `GET /api/sessions` is answered out of (PLAN_M6_75.md item 5).
    ///
    /// ## Why one read rather than three
    ///
    /// The three answers are a single claim about the fleet — "here are
    /// these rows, of N matching, of M sessions" — and taken separately they
    /// stop being one: a refresh committing between the page and the counts
    /// can produce `matching > total`, or counts describing rows the page
    /// does not contain, or a page whose rows were already deleted by the
    /// time the totals were taken. None of those is a crash; all of them
    /// reach the user as a list that visibly does not add up. One
    /// transaction, one lock hold, one moment.
    ///
    /// (This store has exactly one connection behind one mutex, so holding
    /// the lock would already serialize these reads against every writer.
    /// The transaction is still explicit, because that property is an
    /// implementation detail of this struct and the coherence requirement is
    /// not — a future connection pool must not silently reintroduce the
    /// split.)
    ///
    /// ## The two scopes are different, deliberately
    ///
    /// `scope` is the merged view: every host with an actor, which is what
    /// `total` counts. The PAGE and the MATCHING count are computed over
    /// `scope` intersected with the filter's host, so a host-filtered
    /// request never decodes another host's rows at all — while `total` goes
    /// on describing the whole fleet, because "N matching of M sessions" is
    /// a comparison against the fleet and not against the filter's own
    /// scope.
    ///
    /// ## Whether it COUNTS is decided here, inside the lock
    ///
    /// See [`MatchingCount`]. A caller that already holds a matching count
    /// names the generation it was taken at, and this read — holding the
    /// mutex, so no write can be in flight — compares that against the
    /// generation it actually finds. Only inside this hold is the comparison
    /// sound: a caller sampling anything beforehand can have a write land in
    /// the gap and pair an old count with new rows.
    ///
    /// A read that does count does so in the page's own scan
    /// ([`Self::scan_page`]), not in a second one.
    ///
    /// ## An unfiltered read makes NO matching claim
    ///
    /// Callers pass [`MatchingCount::Skip`] for an empty filter and
    /// [`MergedRead::matching`] comes back absent. The tempting shortcut —
    /// "unfiltered means everything matches, so report `total`" — is not
    /// true here: `total` counts rows including those whose payload cannot be
    /// trusted as that row, and the matching count deliberately excludes
    /// exactly those. Reporting `total` as a matching count would make an
    /// unshowable row count as a match only when nobody filtered.
    pub async fn merged_page(
        &self,
        scope: Vec<HostId>,
        after: Option<CacheKey>,
        limit: usize,
        byte_budget: usize,
        filter: SessionFilter,
        matching: MatchingCount,
    ) -> anyhow::Result<MergedRead> {
        let conn = Arc::clone(&self.conn);
        let store_generation = Arc::clone(&self.generation);
        let counting_passes = Arc::clone(&self.counting_passes);
        tokio::task::spawn_blocking(move || -> anyhow::Result<MergedRead> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            // Read-only and never committed: dropping it rolls back, which
            // is the correct end for a transaction that wrote nothing.
            let tx = conn
                .unchecked_transaction()
                .context("beginning the merged read transaction")?;
            // Sampled INSIDE the lock, with the rows below: that is the whole
            // basis on which a count and the data it describes are true of
            // each other. See [`HelmStore::generation`].
            let generation = store_generation.load(Ordering::Acquire);
            // `total` is the whole merged scope's, so it is taken FIRST and
            // the scope is then consumed rather than cloned: the unfiltered
            // case is the common one and its page scope is the same list.
            let total = Self::count_rows(&tx, &scope)?;
            // A host filter naming a host outside the merged view selects
            // nothing — not everything, which is what an empty IN-list would
            // silently mean if it reached the scan.
            let page_scope: Vec<HostId> = match filter.host_scope() {
                None => scope,
                Some(host) if scope.contains(&host) => vec![host],
                Some(_) => Vec::new(),
            };
            let count = match matching {
                MatchingCount::Skip => false,
                MatchingCount::Compute => true,
                MatchingCount::ComputeUnless(held) => held != generation,
            };
            if count {
                counting_passes.fetch_add(1, Ordering::Relaxed);
            }
            let (page, matching) =
                Self::scan_page(&tx, &page_scope, after, limit, byte_budget, &filter, count)?;
            Ok(MergedRead {
                page,
                total,
                matching,
                generation,
            })
        })
        .await
        .context("merged page task panicked")?
    }

    /// The PAGE alone, for the tests whose subject is the scan rather than
    /// the counts served beside it.
    ///
    /// Test-only because production has exactly one reason to read this
    /// table for a list — to answer `GET /api/sessions` — and that answer
    /// needs the counts in the same breath ([`Self::merged_page`]). Offering
    /// a page-only read to production code would be offering a way to
    /// reintroduce the split those counts were pulled into one transaction
    /// to close.
    #[cfg(test)]
    async fn cached_page(
        &self,
        hosts: Vec<HostId>,
        after: Option<CacheKey>,
        limit: usize,
        byte_budget: usize,
        filter: SessionFilter,
    ) -> anyhow::Result<CachePage> {
        Ok(self
            .merged_page(
                hosts,
                after,
                limit,
                byte_budget,
                filter,
                MatchingCount::Skip,
            )
            .await?
            .page)
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

    /// The profile a session was last created from on `host`, if any
    /// ever was (PLAN_M6_75.md item 5).
    ///
    /// Deliberately NOT validated against the host's catalog here, and the
    /// omission is the feature: the catalog lives on the supervisor, so
    /// this side could only check it by making a round trip that may fail
    /// or may be answered by a host that is currently down — and a
    /// remembered default naming a profile that has since been deleted is
    /// a state the product HAS. SPEC.md's rule is to ASK rather than guess
    /// when the last-used profile is gone, which needs the id to survive
    /// the deletion long enough to be recognized as missing. The profiles
    /// read serves this id beside the catalog in one reply, so a client has
    /// both facts in hand and can act on their disagreement — which is not
    /// a claim that the two were read atomically (they cannot be; one comes
    /// from this database and the other from a supervisor over the wire).
    /// No such claim is needed: the client's answer to any mismatch is the
    /// same single behavior, ask instead of guess.
    ///
    /// ## Validated against the host's identity, at READ time
    ///
    /// The stored row carries the identity it was recorded against, and a row
    /// whose identity is not the host's current one is answered as `None` —
    /// the same as never having had a default, which is exactly the state it
    /// describes.
    ///
    /// This is the third of three defences and the only one that covers a row
    /// nobody deleted. Adoption erases the default in its own transaction and
    /// a genuine retarget erases it in its; both are point-in-time actions,
    /// and neither can account for a row written by a request that was
    /// already in flight, a future path that moves a host some other way, or
    /// a database edited by hand. Since a starter profile id RESOLVES on the
    /// successor install rather than merely dangling, "probably fine" is not
    /// a safe posture — so the identity travels with the row and is checked
    /// every time it is read.
    ///
    /// One statement, joined against `hosts`, so the row and the identity it
    /// is judged against come from one moment. A `NULL` on both sides matches
    /// (a host that reports no identity may still have a remembered default);
    /// that is the one case this check cannot sharpen, and it is why the two
    /// deletions above exist rather than being left to this.
    pub async fn remembered_profile(&self, host: HostId) -> anyhow::Result<Option<String>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            conn.query_row(
                "SELECT remembered.profile_id \
                 FROM remembered_profiles AS remembered \
                 JOIN hosts ON hosts.id = remembered.host_id \
                 WHERE remembered.host_id = ?1 \
                   AND ((remembered.host_identity IS NULL AND hosts.host_identity IS NULL) \
                        OR remembered.host_identity = hosts.host_identity)",
                rusqlite::params![host],
                |r| r.get(0),
            )
            .optional()
            .context("reading a host's remembered default profile")
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
    pub async fn remember_profile_default(
        &self,
        host: HostId,
        identity: Option<&str>,
        profile_id: &str,
    ) -> anyhow::Result<bool> {
        self.remember_profile_default_with_source(host, identity, profile_id, None, None, None)
            .await
    }

    /// Remember a successful profile-backed create and its ordering key.
    pub async fn remember_profile_default_from_session(
        &self,
        host: HostId,
        identity: Option<&str>,
        profile_id: &str,
        source_creation_seq: Option<u64>,
        source_created_at: i64,
        source_session_id: &str,
    ) -> anyhow::Result<bool> {
        self.remember_profile_default_with_source(
            host,
            identity,
            profile_id,
            source_creation_seq,
            Some(source_created_at),
            Some(source_session_id),
        )
        .await
    }

    /// Write `profile_id` as `host`'s last-used profile, replacing
    /// whatever was there.
    ///
    /// Written both by a successful profile-backed create and by a completed
    /// session drain. Both observations mean a session was actually created
    /// from the profile; merely opening a picker does not. Their supervisor
    /// creation sequence decides chronology, with `(created_at, session id)`
    /// retained only for older peers that omit the additive sequence field.
    /// Returns whether the visible `(profile_id, host_identity)` pair
    /// changed, so the invalidation feed does not wake every client each time
    /// a user creates from the same profile twice in a row. `false` does not
    /// necessarily mean the candidate matched the stored provenance: it also
    /// means an out-of-order candidate was rejected as older. Callers must not
    /// treat it as proof that this observation became the remembered source.
    ///
    /// "Changed" means the whole `(profile_id, host_identity)` pair, compared
    /// NULL-safely, and the identity half is not bookkeeping. Consider the
    /// natural sequence: an identity-less host remembers P, the host later
    /// learns an identity, and the stored row — bound to `NULL` — stops being
    /// readable ([`Self::remembered_profile`] revalidates the binding, so the
    /// default silently disappears from every create dialog). The next
    /// profile-backed create on P REPAIRS it, rewriting the row against the
    /// learned identity, and the remembered default flips from absent back to
    /// P. Comparing the profile id alone calls that "unchanged" and publishes
    /// nothing, so the repair reaches no open client until something unrelated
    /// happens to bump.
    ///
    /// IDENTITY-BOUND, exactly like every session-cache write here, and for
    /// a sharper reason than symmetry: a profile id is minted per supervisor
    /// and the STARTER profiles every fresh supervisor seeds collide across
    /// installs by construction. So a create whose reply landed after the
    /// row was retargeted or adopted away would not merely record a stale
    /// preference — it could record an id that RESOLVES on the new host to a
    /// completely different profile, and the next create dialog would offer
    /// it as the user's own last choice. `identity` is what the caller
    /// believed this host was when it made the request: `Some` for an
    /// ordinary host, `None` for one that reports no identity at all (which
    /// must still match — a host that has since LEARNED one is not the host
    /// the caller was talking to either).
    ///
    /// The identity is also STORED beside the default, not merely checked on
    /// the way in, so the binding survives at rest and every read revalidates
    /// it (see [`Self::remembered_profile`]). Checking only the write leaves
    /// a row whose host has moved on since perfectly readable.
    ///
    /// [`HostStoreError::HostNotFound`] for an unregistered host rather
    /// than a silent no-op: the foreign key would refuse the insert anyway,
    /// and a typed refusal is what the REST edge maps to a 404. A stale
    /// identity is [`HostStoreError::IdentityMismatch`], with nothing
    /// written.
    async fn remember_profile_default_with_source(
        &self,
        host: HostId,
        identity: Option<&str>,
        profile_id: &str,
        source_creation_seq: Option<u64>,
        source_created_at: Option<i64>,
        source_session_id: Option<&str>,
    ) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        let identity = identity.map(str::to_string);
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
            // ONE statement for both facts this write is judged against: the
            // host's identity (the outer row — its absence IS the unknown
            // host) and the row being replaced (the LEFT JOIN — its columns
            // are NULL when there is no prior default). Two statements would
            // read the same transaction twice to answer one question.
            let known: Option<HostRememberedProfileRow> = tx
                .query_row(
                    "SELECT hosts.host_identity, remembered.profile_id, remembered.host_identity, \
                            remembered.source_creation_seq, remembered.source_created_at, \
                            remembered.source_session_id \
                     FROM hosts \
                     LEFT JOIN remembered_profiles AS remembered ON remembered.host_id = hosts.id \
                     WHERE hosts.id = ?1",
                    rusqlite::params![host],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                        ))
                    },
                )
                .optional()
                .context("checking the host a remembered default names")?;
            let Some((
                current,
                previous_profile,
                previous_identity,
                previous_creation_seq,
                previous_created_at,
                previous_session_id,
            )) = known
            else {
                return Err(anyhow::Error::new(HostStoreError::HostNotFound(host)));
            };
            if current.as_deref() != identity.as_deref() {
                return Err(anyhow::Error::new(HostStoreError::IdentityMismatch {
                    host,
                    expected: identity.unwrap_or_default(),
                    actual: current,
                }));
            }
            if let (Some(candidate_at), Some(candidate_id), Some(stored_at), Some(stored_id)) = (
                source_created_at,
                source_session_id.as_deref(),
                previous_created_at,
                previous_session_id.as_deref(),
            ) && previous_identity.as_deref() == identity.as_deref()
                && !source_is_newer(
                    source_creation_seq,
                    candidate_at,
                    candidate_id,
                    previous_creation_seq.and_then(|seq| u64::try_from(seq).ok()),
                    stored_at,
                    stored_id,
                )
            {
                tx.commit()
                    .context("committing an unchanged remembered default")?;
                return Ok(false);
            }
            // Both halves of the row, so an identity REPAIR under an unchanged
            // profile id counts as a change — see this method's docs.
            let changed = previous_profile.as_deref() != Some(profile_id.as_str())
                || previous_identity.as_deref() != identity.as_deref();
            tx.execute(
                "INSERT INTO remembered_profiles (\
                     host_id, profile_id, host_identity, source_creation_seq, \
                     source_created_at, source_session_id\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT (host_id) DO UPDATE SET profile_id = excluded.profile_id, \
                     host_identity = excluded.host_identity, \
                     source_creation_seq = excluded.source_creation_seq, \
                     source_created_at = excluded.source_created_at, \
                     source_session_id = excluded.source_session_id",
                rusqlite::params![
                    host,
                    profile_id,
                    identity,
                    stored_source_creation_seq,
                    source_created_at,
                    source_session_id
                ],
            )
            .context("remembering a host's default profile")?;
            tx.commit().context("committing the remembered default")?;
            Ok(changed)
        })
        .await
        .context("remember profile default task panicked")?
    }
}

/// `?4, ?5, ...` for an IN-list of `count` host ids starting at parameter
/// index `first`.
///
/// The host scope is the one part of these queries whose arity is not known
/// at compile time. Generating PLACEHOLDERS rather than values is what keeps
/// that from becoming string interpolation of data: every id is still bound
/// through rusqlite, and this function can only ever emit `?N`.
fn host_placeholders(count: usize, first: usize) -> String {
    (first..first + count)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ")
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

    /// A profile-backed fixture with the supervisor's strict chronology.
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

    /// Every cached session across `hosts`, walked page by page through the
    /// production paging API.
    ///
    /// Deliberately a WALK rather than one big page: it is how the tests
    /// below assert cross-host order and skip-and-log behavior, so making it
    /// follow real cursors means those assertions also pin that the resume
    /// predicate does not lose or repeat rows. A page size of two is small
    /// enough that every fixture here spans several pages.
    ///
    /// The cursor advances past SKIPPED rows as well as served ones, which
    /// is the contract [`ScannedRow`] exists for — a walker that resumed
    /// from the last DECODED row would re-scan a poisoned one forever.
    async fn walk_cached(store: &HelmStore, hosts: &[HostId]) -> Vec<CachedSession> {
        let mut all = Vec::new();
        let mut after: Option<CacheKey> = None;
        loop {
            let page = store
                .cached_page(
                    hosts.to_vec(),
                    after.clone(),
                    2,
                    usize::MAX,
                    SessionFilter::default(),
                )
                .await
                .expect("paged read");
            let Some(last) = page.rows.last() else { break };
            after = Some(last.key.clone());
            all.extend(page.rows.into_iter().filter_map(|row| {
                row.info.map(|info| CachedSession {
                    host: row.key.host,
                    info,
                })
            }));
            if !page.more {
                break;
            }
        }
        all
    }

    /// Every host id in the registry — the scope the paged reads take.
    async fn all_host_ids(store: &HelmStore) -> Vec<HostId> {
        store
            .list_hosts()
            .await
            .expect("list hosts")
            .into_iter()
            .map(|row| row.id)
            .collect()
    }

    // ---- Tracing capture, for the skip-and-log tests -------------------
    //
    // `cached_sessions`/`cached_sessions_all`'s skip-and-log posture (item
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
    /// (Safe for this DDL specifically: it contains no string literal that
    /// could hide a `--`.)
    fn schema_objects(conn: &Connection) -> Vec<(String, String)> {
        fn normalize(sql: &str) -> String {
            sql.lines()
                .map(|line| line.split("--").next().unwrap_or_default())
                .flat_map(str::split_whitespace)
                .collect::<Vec<_>>()
                .join(" ")
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
    /// `cached_sessions` (the per-host stale list), `merged_page` (the
    /// merged, paginated list), and `cached_session` (the stale detail view
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

        let page = store
            .cached_page(vec![host], None, 10, usize::MAX, SessionFilter::default())
            .await
            .expect("merged page");
        assert_eq!(page.rows.len(), 1);
        assert!(
            page.rows[0].info.is_some(),
            "the merged page must carry the row as DATA, not as a skipped hole"
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

    /// A migrated database and a freshly created one must end up with
    /// byte-identical schemas — the invariant that lets [`apply_schema`]'s
    /// version-0 branch create the final shape directly instead of
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

    /// The version-6 migration DROPS the remembered defaults it inherits.
    ///
    /// Spec: a database whose `remembered_profiles` rows predate the
    /// identity column comes up with no default for any host.
    ///
    /// Carrying them forward would have been the polite migration and is the
    /// unsafe one. A version-5 row records nothing about which install it was
    /// chosen on, and a `NULL` in the new column is a value that legitimately
    /// MEANS "a host reporting no identity" — so a preserved row would
    /// validate against every identity-less host, and starter profile ids
    /// collide across installs by construction. Dropping costs one create
    /// dialog that asks instead of defaulting, which is SPEC.md's own
    /// fallback and the direction that cannot be wrong.
    ///
    /// The v5 state is constructed by downgrading a real database rather than
    /// hand-building one, so the row this asserts about sits in the schema
    /// the previous release actually shipped.
    #[tokio::test]
    async fn the_identity_migration_forgets_defaults_it_cannot_validate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("helm.db");
        let host = {
            let store = HelmStore::open(&path).await.expect("create");
            let host = store.add_ssh_host("user@host", None, None).await.unwrap();
            store
                .remember_profile_default(host, None, "starter-claude")
                .await
                .unwrap();
            host
        };

        // Back to the shape version 5 shipped: no identity column, and a row
        // recorded under it.
        {
            let conn = Connection::open(&path).expect("reopen raw");
            conn.execute_batch(
                "DROP TABLE device_sessions;
                 DROP TABLE web_token;
                 DROP TABLE remembered_profiles;
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
            migrated.remembered_profile(host).await.unwrap(),
            None,
            "a default nothing can validate must be forgotten rather than served"
        );
        // And the host itself survives — this is a forgotten preference, not
        // a lost registry.
        assert!(
            migrated
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == host)
        );
    }

    /// A version-7 preference survives the provenance migration and stays
    /// opaque until a demonstrably new create establishes its source.
    ///
    /// The first post-upgrade drain may contain only older survivors because
    /// the session that actually established the preference was deleted
    /// before migration. Replacing from that drain would silently roll the
    /// default backward. A direct create observed after provenance support
    /// exists is the first event that can safely supersede it.
    #[tokio::test]
    async fn version_7_remembered_default_survives_until_a_new_create_establishes_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let host = {
            let store = HelmStore::open(&path).await.unwrap();
            host_with_identity(&store, "v7@host", "v7-identity").await
        };
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "DROP TABLE remembered_profiles;
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
        assert_eq!(
            store.remembered_profile(host).await.unwrap().as_deref(),
            Some("legacy-profile")
        );
        {
            let conn = store.conn.lock().unwrap();
            let provenance: (Option<i64>, Option<i64>, Option<String>) = conn
                .query_row(
                    "SELECT source_creation_seq, source_created_at, source_session_id
                     FROM remembered_profiles WHERE host_id = ?1",
                    rusqlite::params![host],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(provenance, (None, None, None));
        }
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
            )
            .await
            .unwrap();
        assert!(!replacement.default_changed);
        assert_eq!(
            store.remembered_profile(host).await.unwrap().as_deref(),
            Some("legacy-profile"),
            "a drain cannot prove its newest surviving source is newer than the migrated choice"
        );

        assert!(
            store
                .remember_profile_default_from_session(
                    host,
                    Some("v7-identity"),
                    "established-profile",
                    Some(2),
                    200,
                    "new-create",
                )
                .await
                .unwrap(),
            "a create observed after migration is demonstrably new"
        );
        assert_eq!(
            store.remembered_profile(host).await.unwrap().as_deref(),
            Some("established-profile")
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

    /// SPEC.md: the local host is never user management surface. Refusing
    /// its removal is what keeps it always-present, not merely
    /// usually-present.
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

    /// The same refusal on the update path — SPEC.md: the local host is not
    /// user management surface, and that must hold for every ssh-host
    /// MANAGEMENT mutation, not just removal. (`add_ssh_host` cannot even
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
            .replace_host_sessions(host, "identity-full", vec![session("s1", 100)])
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
            .replace_host_sessions(host, "cascade-identity", vec![session("s1", 100)])
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
            .replace_host_sessions(host, "identity-x", vec![session("s1", 100)])
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
            .replace_host_sessions(host, "identity-old", vec![session("s1", 100)])
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
            .replace_host_sessions(mismatched, "identity-old", vec![session("kept", 100)])
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
            .replace_host_sessions(host, "identity-old", vec![session("s1", 100)])
            .await
            .unwrap();
        store
            .replace_host_sessions(other, "other-identity", vec![session("s2", 200)])
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
            .replace_host_sessions(host, "identity-x", vec![session("s1", 100)])
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
            .replace_host_sessions(live, "live-identity", vec![session("s1", 100)])
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
            .replace_host_sessions(host, "identity-before", vec![session("s1", 100)])
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
            )
            .await
            .expect("seed the cache");

        let poisoned = vec![
            session("new-1", 300),
            session("dup", 400),
            session("dup", 500), // collides with the row just inserted above
        ];
        store
            .replace_host_sessions(host, "rollback-identity", poisoned)
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
                .replace_host_sessions(ghost, "any-identity", entries)
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
            .replace_host_sessions(host, "identity-before", vec![session("pre-adopt", 100)])
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
            .replace_host_sessions(host, "identity-after", vec![session("post-adopt", 200)])
            .await
            .expect("a fresh refresh under the NEW identity must succeed");

        // The delayed refresh: still carrying the identity the connection
        // observed BEFORE the adoption, arriving after the fact.
        let err = store
            .replace_host_sessions(host, "identity-before", vec![session("delayed", 300)])
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
            .replace_host_sessions(host, "overlap-identity", vec![session("s1", 100)])
            .await
            .unwrap();

        let mut newer = session("s1", 100);
        newer.title = "renamed".to_string();
        store
            .replace_host_sessions(host, "overlap-identity", vec![newer])
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
            .replace_host_sessions(host, "shrink-identity", vec![session("b", 200)])
            .await
            .unwrap();
        assert_eq!(
            cached_ids(&store, host).await,
            vec!["b"],
            "shrinking to one entry must drop the other two, not just add/update \"b\""
        );

        store
            .replace_host_sessions(host, "shrink-identity", vec![])
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
    /// caught. Checked for both the per-host read and the cross-host merge,
    /// since each has its own index ([`apply_schema`]'s
    /// `session_cache_by_host_order` and `session_cache_order`) and either
    /// could independently get the tiebreak wrong.
    #[tokio::test]
    async fn equal_created_at_ties_break_ascending_by_session_id() {
        let (_dir, store) = fresh_store().await;
        let a = host_with_identity(&store, "tie-a@host", "tie-a-identity").await;
        let b = host_with_identity(&store, "tie-b@host", "tie-b-identity").await;

        // Reverse-id insertion order on both hosts: if either read path
        // fell back to insertion/rowid order instead of sorting by
        // session_id, this would surface as "c", "b", "a" instead of the
        // expected ascending order.
        store
            .replace_host_sessions(
                a,
                "tie-a-identity",
                vec![session("c", 100), session("b", 100), session("a", 100)],
            )
            .await
            .unwrap();
        store
            .replace_host_sessions(b, "tie-b-identity", vec![session("z", 100)])
            .await
            .unwrap();

        assert_eq!(
            cached_ids(&store, a).await,
            vec!["a", "b", "c"],
            "per-host read must tie-break equal created_at ascending by session_id"
        );

        let all = walk_cached(&store, &all_host_ids(&store).await).await;
        let all_ids: Vec<&str> = all.iter().map(|c| c.info.id.as_str()).collect();
        assert_eq!(
            all_ids,
            vec!["a", "b", "c", "z"],
            "the cross-host merge must apply the same tiebreak GLOBALLY, not just \
             within each host's own rows"
        );
    }

    /// `cached_sessions_all` merges across hosts in the same wire order —
    /// the cross-host counterpart to
    /// `read_order_follows_the_extracted_columns_not_the_json`, and the
    /// first exercise of [`CachedSession`] tagging each row with its host.
    #[tokio::test]
    async fn cached_sessions_all_merges_hosts_in_wire_order() {
        let (_dir, store) = fresh_store().await;
        let a = host_with_identity(&store, "a@host", "a-identity").await;
        let b = host_with_identity(&store, "b@host", "b-identity").await;
        store
            .replace_host_sessions(a, "a-identity", vec![session("older", 100)])
            .await
            .unwrap();
        store
            .replace_host_sessions(b, "b-identity", vec![session("newer", 200)])
            .await
            .unwrap();

        let all = walk_cached(&store, &all_host_ids(&store).await).await;
        let seen: Vec<(HostId, &str)> = all.iter().map(|c| (c.host, c.info.id.as_str())).collect();
        assert_eq!(
            seen,
            vec![(b, "newer"), (a, "older")],
            "cross-host order must be created_at descending regardless of which host \
             each row belongs to"
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
    /// through `cached_sessions_all`'s merged read, with an entirely
    /// healthy second host present to prove one host's corruption cannot
    /// take down another host's rows in the shared cross-host list either.
    #[tokio::test]
    async fn cached_sessions_all_skips_a_poisoned_blob_and_serves_the_rest() {
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
                vec![session("cached-sessions-all-poisoned", 100)],
            )
            .await
            .unwrap();
        store
            .replace_host_sessions(
                healthy_host,
                "healthy-all-identity",
                vec![session("healthy-1", 200)],
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
                         WHERE session_id = 'cached-sessions-all-poisoned'",
                        [],
                    )
                    .unwrap();
            })
            .await
            .unwrap();
        }

        let all = walk_cached(&store, &all_host_ids(&store).await).await;
        let ids: Vec<&str> = all.iter().map(|c| c.info.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["healthy-1"],
            "the poisoned row must be skipped while the other host's row is still served"
        );

        let events = skip_warnings();
        let hit = events
            .iter()
            .find(|e| e.field("session_id") == Some("cached-sessions-all-poisoned"));
        let hit = hit.expect(
            "the skipped row must be logged via tracing::warn! naming its session id, \
             not just silently dropped from the returned Vec",
        );
        assert_eq!(
            hit.field("host"),
            Some(poisoned_host.to_string().as_str()),
            "the warning must name the host the poisoned row belongs to"
        );
    }

    /// A page whose LAST scanned row is poisoned must still advance the
    /// cursor past it — and a page where EVERY row is poisoned must too.
    ///
    /// This is the regression test for permanent, silent data loss, not a
    /// tidiness check. When continuation was derived from decoded rows, a
    /// poisoned row at a page boundary left the resume point before it
    /// forever: the next page re-scanned the same row, skipped it again,
    /// and every session after it in the whole fleet became unreachable
    /// through the API. The fully-poisoned case is the same bug wearing a
    /// more obvious costume — an empty page with no continuation, in the
    /// middle of a list.
    #[tokio::test]
    async fn a_page_ending_in_a_poisoned_row_still_advances_past_it() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "poison@host", "poison-identity").await;
        // Descending by created_at: newest first. The second and third are
        // poisoned below, so a two-row page ends on a poisoned row and a
        // later two-row page contains nothing BUT poisoned rows.
        store
            .replace_host_sessions(
                host,
                "poison-identity",
                vec![
                    session("good-1", 500),
                    session("bad-1", 400),
                    session("bad-2", 300),
                    session("good-2", 200),
                ],
            )
            .await
            .expect("seed the cache");
        for id in ["bad-1", "bad-2"] {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                conn.lock()
                    .unwrap()
                    .execute(
                        "UPDATE session_cache SET info_json = 'not valid json' \
                         WHERE session_id = ?1",
                        rusqlite::params![id],
                    )
                    .unwrap();
            })
            .await
            .unwrap();
        }

        // Page one ends ON a poisoned row.
        let first = store
            .cached_page(vec![host], None, 2, usize::MAX, SessionFilter::default())
            .await
            .expect("first page");
        assert_eq!(
            first.rows.len(),
            2,
            "the limit counts rows SCANNED, so a skipped row still consumes one"
        );
        assert_eq!(first.rows[1].key.session_id, "bad-1");
        assert!(
            first.rows[1].info.is_none(),
            "the poisoned row comes back with its key and no payload"
        );
        assert!(first.more, "two of four remain");

        // Page two is ENTIRELY poisoned — it must still advance.
        let second = store
            .cached_page(
                vec![host],
                Some(first.rows[1].key.clone()),
                1,
                usize::MAX,
                SessionFilter::default(),
            )
            .await
            .expect("second page");
        assert_eq!(second.rows.len(), 1);
        assert_eq!(second.rows[0].key.session_id, "bad-2");
        assert!(second.rows[0].info.is_none());
        assert!(second.more);

        // And the walk reaches the far side, which is the whole point.
        let third = store
            .cached_page(
                vec![host],
                Some(second.rows[0].key.clone()),
                2,
                usize::MAX,
                SessionFilter::default(),
            )
            .await
            .expect("third page");
        assert_eq!(
            third
                .rows
                .iter()
                .filter_map(|row| row.info.as_ref())
                .map(|info| info.id.as_str())
                .collect::<Vec<_>>(),
            vec!["good-2"],
            "the row after the poisoned run must be reachable"
        );
        assert!(!third.more, "and the order ends there");
    }

    /// The byte bound must stop the SCAN, not merely trim its result.
    ///
    /// The point is the decoding that never happens: a limit of five
    /// thousand against fat blobs meant five thousand JSON parses before any
    /// budget was consulted. A bound that only trimmed afterwards would
    /// leave that cost exactly where it was.
    #[tokio::test]
    async fn the_byte_bound_stops_the_scan_and_still_makes_progress() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "fat@host", "fat-identity").await;
        let fat = |id: &str, created_at: i64| {
            let mut info = session(id, created_at);
            info.title = "x".repeat(4096);
            info
        };
        store
            .replace_host_sessions(
                host,
                "fat-identity",
                vec![fat("f1", 300), fat("f2", 200), fat("f3", 100)],
            )
            .await
            .expect("seed the cache");

        let page = store
            .cached_page(vec![host], None, 10, 4_000, SessionFilter::default())
            .await
            .expect("bounded page");
        assert_eq!(
            page.rows.len(),
            1,
            "the scan stops at the byte bound rather than decoding all ten it was allowed"
        );
        assert!(page.more, "and says so, so the walk continues");

        // A single row larger than the whole budget must still be served,
        // or the walk stalls on it forever.
        let tiny = store
            .cached_page(vec![host], None, 10, 1, SessionFilter::default())
            .await
            .expect("degenerate budget");
        assert_eq!(tiny.rows.len(), 1, "at least one row always makes progress");
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
                .replace_host_sessions(local_id, "local-identity", vec![session("local-1", 100)])
                .await
                .unwrap();
            store
                .replace_host_sessions(ssh_id, "remote-identity", vec![session("remote-1", 200)])
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
            .replace_host_sessions(host, "identity-1", vec![session("s-1", 100)])
            .await
            .unwrap();
        assert!(
            first.changed,
            "filling an empty cache is a change by any reading"
        );

        let repeat = store
            .replace_host_sessions(host, "identity-1", vec![session("s-1", 100)])
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
            .replace_host_sessions(host, "identity-1", vec![flipped])
            .await
            .unwrap();
        assert!(changed.changed, "a status flip is a change");

        // And so is losing a session, which no per-row comparison of the
        // NEW list against itself would ever notice.
        let emptied = store
            .replace_host_sessions(host, "identity-1", Vec::new())
            .await
            .unwrap();
        assert!(emptied.changed, "a session disappearing is a change");
    }

    /// A rewrite that repairs the ORDERING COLUMN is a change, even though
    /// the payload beside it is untouched.
    ///
    /// `created_at` is a column of its own — the key every page, cursor and
    /// merge is built on — extracted from the payload at write time. The two
    /// can therefore disagree in a database this build did not write (a hand
    /// edit, a downgrade, an older writer), and the rewrite that repairs the
    /// disagreement MOVES the row in the merged order while leaving every
    /// byte of its payload alone. Comparing payloads only would report that
    /// as "nothing changed", and the feed would starve: the row would sit in
    /// its new position with no client ever told to look again.
    #[tokio::test]
    async fn a_repaired_ordering_column_counts_as_a_change() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;
        store
            .replace_host_sessions(host, "identity-1", vec![session("s-1", 100)])
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
            .replace_host_sessions(host, "identity-1", vec![session("s-1", 100)])
            .await
            .unwrap();
        assert!(
            repaired.changed,
            "the row moved in the merged order, so clients must be told to look again"
        );
        // And the write that follows it, with nothing left to repair, is a
        // no-op again — so this is a comparison rather than a permanent
        // "changed" latch.
        let settled = store
            .replace_host_sessions(host, "identity-1", vec![session("s-1", 100)])
            .await
            .unwrap();
        assert!(!settled.changed);
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

    /// The remembered default is per-host, replaceable, durable, and — like
    /// every other helm-owned fact about a host — forgotten with it.
    ///
    /// Durability is the point of storing it in helm.db at all rather than
    /// in memory: SPEC.md's create dialog defaults to the last-used profile,
    /// and a default that evaporated on every helm restart would send the
    /// user back to picking one by hand exactly when they had just
    /// established a habit.
    #[tokio::test]
    async fn a_remembered_default_is_per_host_replaceable_durable_and_cascades() {
        let (dir, store) = fresh_store().await;
        let local = all_host_ids(&store).await[0];
        let ssh = store.add_ssh_host("user@host", None, None).await.unwrap();

        assert_eq!(store.remembered_profile(local).await.unwrap(), None);
        // `None` throughout: neither of these hosts has ever reported an
        // identity, which is itself the value the write must match.
        assert!(
            store
                .remember_profile_default(local, None, "p-1")
                .await
                .unwrap(),
            "the first remembered default is a change"
        );
        assert!(
            !store
                .remember_profile_default(local, None, "p-1")
                .await
                .unwrap(),
            "creating from the same profile twice changes nothing observable"
        );
        assert!(
            store
                .remember_profile_default(local, None, "p-2")
                .await
                .unwrap()
        );
        assert_eq!(
            store.remembered_profile(local).await.unwrap(),
            Some("p-2".to_string()),
            "the latest choice replaces the previous one rather than accumulating"
        );
        assert_eq!(
            store.remembered_profile(ssh).await.unwrap(),
            None,
            "a default is per host: a profile id means nothing on another supervisor"
        );

        // Durable across a genuine reopen of the same file.
        drop(store);
        let reopened = HelmStore::open(&dir.path().join("helm.db"))
            .await
            .expect("reopen");
        assert_eq!(
            reopened.remembered_profile(local).await.unwrap(),
            Some("p-2".to_string())
        );

        // And forgotten with its host: removing a host forgets everything
        // the helm knew about it, in one statement (the same CASCADE the
        // session cache rides).
        reopened
            .remember_profile_default(ssh, None, "p-3")
            .await
            .unwrap();
        reopened.remove_ssh_host(ssh).await.unwrap();
        assert_eq!(reopened.remembered_profile(ssh).await.unwrap(), None);
    }

    /// REPAIRING a remembered default's identity binding — same profile id,
    /// newly learned identity — counts as a CHANGE.
    ///
    /// Spec: `remember_profile_default` compares the whole stored
    /// `(profile_id, host_identity)` pair, so writing the same id against an
    /// identity the row does not carry answers `true`.
    ///
    /// The sequence is ordinary rather than contrived, which is what makes the
    /// naive comparison dangerous: an identity-less host remembers P; the host
    /// later learns an identity, at which point the stored row stops being
    /// readable and the default silently vanishes from every create dialog;
    /// the next profile-backed create on P rewrites the binding and brings it
    /// back. Comparing the profile id alone calls that "no change", so the
    /// caller publishes no invalidation and the default's return reaches no
    /// open client — a create dialog somewhere else goes on offering nothing
    /// until something unrelated happens to wake it.
    #[tokio::test]
    async fn repairing_a_remembered_defaults_identity_binding_is_a_change() {
        let (_dir, store) = fresh_store().await;
        let host = store
            .add_ssh_host("user@learner", None, None)
            .await
            .unwrap();

        // Recorded while the host reported no identity at all.
        assert!(
            store
                .remember_profile_default(host, None, "starter-claude")
                .await
                .unwrap()
        );
        assert_eq!(
            store.remembered_profile(host).await.unwrap(),
            Some("starter-claude".to_string())
        );

        // The host's first successful hello teaches the registry an identity.
        // The stored row is bound to NULL, so it is no longer readable.
        store
            .record_first_contact(host, &dialed_as(&store, host).await, "identity-1")
            .await
            .unwrap();
        assert_eq!(
            store.remembered_profile(host).await.unwrap(),
            None,
            "a row bound to no identity is not this install's preference"
        );

        assert!(
            store
                .remember_profile_default(host, Some("identity-1"), "starter-claude")
                .await
                .unwrap(),
            "the same id under a different binding is a different row, and the default going from \
             absent back to present is exactly what other clients must be told about"
        );
        assert_eq!(
            store.remembered_profile(host).await.unwrap(),
            Some("starter-claude".to_string())
        );

        // And a genuinely idempotent write — same id, same binding — is still
        // no change, which is the property this must not have traded away.
        assert!(
            !store
                .remember_profile_default(host, Some("identity-1"), "starter-claude")
                .await
                .unwrap()
        );
    }

    /// ADOPTION erases the remembered default, and a write made against the
    /// superseded install is refused.
    ///
    /// This is not symmetry with the session-cache writes; it is the case
    /// that makes profiles different. Profile ids are minted per supervisor
    /// AND every fresh supervisor seeds the same starter profiles, so an id
    /// carried across an adoption does not merely dangle — it RESOLVES on the
    /// successor install, to a profile the user never chose, offered back as
    /// their own last choice. Both halves therefore have to hold: the stored
    /// row goes with the install it described, and a write still in flight
    /// across that moment is refused rather than stored.
    ///
    /// The purge is asserted from the same transaction that purges the
    /// session cache, because a follow-up call could be torn by a crash or
    /// observed half-done by a concurrent reader — and the half a reader
    /// would catch is exactly the one that resolves wrongly.
    #[tokio::test]
    async fn adoption_forgets_the_superseded_installs_remembered_default() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;

        assert!(
            store
                .remember_profile_default(host, Some("identity-1"), "starter-claude")
                .await
                .unwrap()
        );

        // The install this host points at was replaced, and the user adopted
        // the new identity. A create that was in flight across that moment
        // still carries the OLD one.
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
            store.remembered_profile(host).await.unwrap(),
            None,
            "the adopted install has no last-used profile, and the superseded install's id would \
             resolve here rather than dangle"
        );

        let error = store
            .remember_profile_default(host, Some("identity-1"), "starter-codex")
            .await
            .expect_err("a write made against the superseded install must not land");
        assert!(matches!(
            error.downcast_ref::<HostStoreError>(),
            Some(HostStoreError::IdentityMismatch { .. })
        ));
        assert_eq!(
            store.remembered_profile(host).await.unwrap(),
            None,
            "and nothing was written"
        );

        // A host that has LEARNED an identity is not the identity-less host
        // an earlier caller was talking to either, so `None` is refused too
        // rather than treated as "do not care".
        assert!(
            store
                .remember_profile_default(host, None, "starter-codex")
                .await
                .is_err()
        );
    }

    /// A default recorded against one install must never be served for
    /// another, even when the two mint the SAME id.
    ///
    /// Spec: after an adoption, `remembered_profile` answers `None` for a
    /// starter id both installs happen to define — the id resolving on the
    /// successor is precisely what makes carrying it forward dangerous rather
    /// than merely stale, and SPEC.md's rule is to ask rather than guess.
    ///
    /// Staged with a shared starter id specifically because the tempting
    /// wrong fix — "a stale default is harmless, it just will not be found in
    /// the catalog" — is exactly the assumption this breaks.
    #[tokio::test]
    async fn a_shared_starter_id_does_not_survive_an_adoption() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;
        // The id every fresh supervisor seeds, so it means something on both
        // sides of the adoption.
        store
            .remember_profile_default(host, Some("identity-1"), "starter-claude")
            .await
            .unwrap();

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
            store.remembered_profile(host).await.unwrap(),
            None,
            "an id that resolves on the new install must not be offered as the user's own last \
             choice there"
        );
        // And the new install can establish its own, which is what makes this
        // a purge rather than a permanent hole.
        store
            .remember_profile_default(host, Some("identity-2"), "starter-claude")
            .await
            .unwrap();
        assert_eq!(
            store.remembered_profile(host).await.unwrap(),
            Some("starter-claude".to_string())
        );
    }

    /// RETARGETING a host forgets its remembered default — including when
    /// neither install reports an identity at all.
    ///
    /// Spec: a destination change that actually moves the row deletes the
    /// default; a byte-identical destination update keeps it.
    ///
    /// The identity-less case is the one that needs its own staging. The
    /// identity binding cannot help there — `NULL` matches `NULL`, so two
    /// entirely different installs look alike to it — and starter profile ids
    /// collide by construction, so the default would resolve on whatever the
    /// row now points at. The re-affirming half is asserted beside it because
    /// a resubmitted form or an idempotent reconcile must not cost the user a
    /// preference over a write that changed nothing.
    #[tokio::test]
    async fn retargeting_forgets_the_remembered_default_even_with_no_identity() {
        let (_dir, store) = fresh_store().await;
        let host = store.add_ssh_host("user@first", None, None).await.unwrap();
        // No identity on either side of the move: the case the binding cannot
        // distinguish.
        store
            .remember_profile_default(host, None, "starter-claude")
            .await
            .unwrap();

        // Re-affirming the SAME destination keeps it.
        store
            .update_ssh_destination(host, "user@first")
            .await
            .unwrap();
        assert_eq!(
            store.remembered_profile(host).await.unwrap(),
            Some("starter-claude".to_string()),
            "a write that changed nothing must not cost a preference"
        );

        // A genuine retarget does not.
        store
            .update_ssh_destination(host, "user@second")
            .await
            .unwrap();
        assert_eq!(
            store.remembered_profile(host).await.unwrap(),
            None,
            "the row now points at a different install, whose starter ids collide with the old \
             one's by construction"
        );
    }

    /// A remembered default whose stored identity is not the host's current
    /// one is answered as `None` — the last defence, for a row neither the
    /// adoption purge nor the retarget purge deleted.
    ///
    /// Spec: `remembered_profile` validates the identity it stored against
    /// the identity the host currently holds, and refuses on disagreement.
    ///
    /// Staged by moving the HOST's identity directly rather than through
    /// adoption, because the point is a row that escaped both deletions — a
    /// write already in flight, a future path that moves a host some other
    /// way, a hand-edited database. Since a starter id resolves on the
    /// successor rather than dangling, "probably fine" is not a posture this
    /// can take.
    #[tokio::test]
    async fn a_remembered_default_from_a_superseded_identity_is_not_served() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;
        store
            .remember_profile_default(host, Some("identity-1"), "starter-claude")
            .await
            .unwrap();

        // The host's identity moves without the row being touched.
        {
            let conn = Arc::clone(&store.conn);
            tokio::task::spawn_blocking(move || {
                conn.lock()
                    .unwrap()
                    .execute(
                        "UPDATE hosts SET host_identity = 'identity-2' WHERE id = ?1",
                        rusqlite::params![host],
                    )
                    .unwrap();
            })
            .await
            .unwrap();
        }

        assert_eq!(
            store.remembered_profile(host).await.unwrap(),
            None,
            "a row recorded against an install this host no longer is must not be served"
        );
    }

    /// Remembering a default for a host that does not exist is a TYPED
    /// refusal, not a silent no-op and not a raw foreign-key message.
    ///
    /// The REST edge maps `HostNotFound` to a 404; without the typed value
    /// a caller would get a 500 for naming a host that was removed a moment
    /// ago, which is a normal race rather than a helm fault.
    #[tokio::test]
    async fn remembering_a_default_for_an_unknown_host_is_a_typed_not_found() {
        let (_dir, store) = fresh_store().await;
        let error = store
            .remember_profile_default(9_999, None, "p-1")
            .await
            .expect_err("an unregistered host cannot have a default");
        assert!(matches!(
            error.downcast_ref::<HostStoreError>(),
            Some(HostStoreError::HostNotFound(9_999))
        ));
    }

    /// A row the page refuses to SHOW must not be a row the count claims as
    /// a MATCH — the two share one validation predicate, and this is what
    /// that sharing buys.
    ///
    /// Without it the counts promise pages that cannot exist: "3 matching"
    /// against a walk that can only ever display 2, with the third
    /// permanently invisible and nothing to explain the gap. The two ways a
    /// row fails are both staged, because they took different code paths
    /// before they were unified: an undecodable payload, and one that
    /// decodes but disagrees with the columns it is filed under.
    ///
    /// The FLEET total still counts both, deliberately — see
    /// [`HelmStore::count_rows`]. A count of what is there and a count of
    /// what matches are different questions, and hiding a corrupt row from
    /// the first would make "showing 2 of 3" read as data loss rather than
    /// as the one unshowable entry it is.
    #[tokio::test]
    async fn an_unshowable_row_is_never_counted_as_a_match() {
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

        let read = store
            .merged_page(
                vec![host],
                None,
                10,
                usize::MAX,
                SessionFilter::default().title("keeper"),
                MatchingCount::Compute,
            )
            .await
            .expect("filtered read");
        assert_eq!(
            read.matching,
            Some(1),
            "only the row a page can actually show counts as a match"
        );
        assert_eq!(
            read.page
                .rows
                .iter()
                .filter(|row| row.info.is_some())
                .count(),
            1,
            "and the page shows exactly that one"
        );
        assert_eq!(
            read.total, 3,
            "while the fleet total counts every row, unshowable or not"
        );
    }

    /// The page and both counts describe ONE moment.
    ///
    /// The property is about a read that cannot interleave with a write, so
    /// what this can pin directly is the arithmetic that a torn read
    /// violates: `matching` never exceeds `total`, and both agree with the
    /// rows the same call returned. The structural half — one transaction,
    /// one lock hold — is enforced by `merged_page` being the only way to
    /// ask, which is why the page-only reader beside it is `#[cfg(test)]`.
    #[tokio::test]
    async fn one_read_answers_the_page_and_both_counts_coherently() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;
        store
            .replace_host_sessions(
                host,
                "identity-1",
                vec![
                    SessionInfo {
                        cwd: "/keep/one".to_string(),
                        ..session("s-1", 300)
                    },
                    SessionInfo {
                        cwd: "/keep/two".to_string(),
                        ..session("s-2", 200)
                    },
                    SessionInfo {
                        cwd: "/other".to_string(),
                        ..session("s-3", 100)
                    },
                ],
            )
            .await
            .expect("seed the cache");

        let read = store
            .merged_page(
                vec![host],
                None,
                1,
                usize::MAX,
                SessionFilter::default().directory("/keep"),
                MatchingCount::Compute,
            )
            .await
            .expect("filtered read");
        assert_eq!(read.total, 3);
        assert_eq!(read.matching, Some(2));
        assert!(read.matching.is_some_and(|matching| matching <= read.total));
        assert_eq!(read.page.rows.len(), 1, "the page cut is over the matches");
        assert!(read.page.more, "and there is another match beyond it");

        // An UNFILTERED read makes no matching claim at all — see
        // `merged_page`'s docs for why "no filter, so everything matches" is
        // not a truth this list can state.
        let unfiltered = store
            .merged_page(
                vec![host],
                None,
                10,
                usize::MAX,
                SessionFilter::default(),
                MatchingCount::Skip,
            )
            .await
            .expect("unfiltered read");
        assert_eq!(unfiltered.total, 3);
        assert_eq!(
            unfiltered.matching, None,
            "an unfiltered listing reports a fleet total and claims nothing about matching"
        );
    }

    /// A count is qualified by the store's GENERATION, and the comparison
    /// happens where no write can slip past it.
    ///
    /// Spec: `ComputeUnless(g)` answers `matching: None` — "the count you
    /// hold still stands" — exactly while the store's generation is still
    /// `g`, and recomputes otherwise. A committed change between the caller
    /// sampling a generation and the read running must therefore produce a
    /// FRESH count beside the new rows, never the old count beside them.
    ///
    /// This is the pairing the previous design could not make: the count rode
    /// in the client's cursor qualified by the fleet revision, which is
    /// published AFTER a write commits, so committed rows and an unmoved
    /// revision routinely coexisted. The ordering here is staged explicitly
    /// rather than raced — the write lands strictly between the sample and
    /// the read — because a property about a window is only pinned by a test
    /// that puts something in the window every time it runs.
    #[tokio::test]
    async fn a_count_cannot_be_paired_with_rows_committed_after_it() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;
        let matching = |cwd: &str, id: &str, at: i64| SessionInfo {
            cwd: cwd.to_string(),
            ..session(id, at)
        };
        store
            .replace_host_sessions(
                host,
                "identity-1",
                vec![
                    matching("/keep/one", "s-1", 300),
                    matching("/other", "s-2", 200),
                ],
            )
            .await
            .expect("seed the cache");

        // What a caller holds after a first page: a count, and the generation
        // it was taken at.
        let first = store
            .merged_page(
                vec![host],
                None,
                10,
                usize::MAX,
                SessionFilter::default().directory("/keep"),
                MatchingCount::Compute,
            )
            .await
            .expect("first page");
        assert_eq!(first.matching, Some(1));
        let sampled = first.generation;

        // Unchanged store: the held count is confirmed rather than recounted,
        // which is what makes a walk linear.
        let confirmed = store
            .merged_page(
                vec![host],
                None,
                10,
                usize::MAX,
                SessionFilter::default().directory("/keep"),
                MatchingCount::ComputeUnless(sampled),
            )
            .await
            .expect("second page");
        assert_eq!(
            confirmed.matching, None,
            "an unmoved generation means the caller's count still describes these rows"
        );
        assert_eq!(confirmed.generation, sampled);

        // The write that lands in the window. It commits strictly after the
        // generation above was sampled and strictly before the read below.
        store
            .replace_host_sessions(
                host,
                "identity-1",
                vec![
                    matching("/keep/one", "s-1", 300),
                    matching("/other", "s-2", 200),
                    matching("/keep/two", "s-3", 100),
                ],
            )
            .await
            .expect("commit a third session");

        let after = store
            .merged_page(
                vec![host],
                None,
                10,
                usize::MAX,
                SessionFilter::default().directory("/keep"),
                MatchingCount::ComputeUnless(sampled),
            )
            .await
            .expect("page after the write");
        assert_ne!(
            after.generation, sampled,
            "a committed change must move the generation, or nothing else here can work"
        );
        assert_eq!(
            after.matching,
            Some(2),
            "the stale generation must force a recount, so the count describes the rows beside it"
        );
        assert_eq!(after.page.rows.len(), 2, "and the page holds both of them");
    }

    /// A write that changed NOTHING must not move the generation.
    ///
    /// Spec: a refresh writing back a byte-identical row set leaves the
    /// generation where it was, so a walk in progress goes on reusing its
    /// count.
    ///
    /// This is what keeps the count cache useful at all rather than merely
    /// correct. Every connected host rewrites its whole list every few
    /// seconds, and in a settled fleet writes back exactly what was already
    /// there; a generation that moved for those would make every page of
    /// every walk recount, which is the cost the design exists to avoid.
    /// Sameness is judged on the STORED BYTES inside the writing
    /// transaction, so rows that support identical counts are recognized as
    /// such.
    #[tokio::test]
    async fn a_no_op_write_leaves_the_generation_alone() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;
        let entries = vec![session("s-1", 300), session("s-2", 200)];
        store
            .replace_host_sessions(host, "identity-1", entries.clone())
            .await
            .expect("seed the cache");
        let before = store
            .merged_page(
                vec![host],
                None,
                10,
                usize::MAX,
                SessionFilter::default().title("s-1"),
                MatchingCount::Compute,
            )
            .await
            .expect("first read")
            .generation;

        store
            .replace_host_sessions(host, "identity-1", entries)
            .await
            .expect("write the same list back");
        // And the same for the single-row paths, which have their own
        // changed-only comparisons.
        store
            .remember_session(host, "identity-1", &session("s-1", 300))
            .await
            .expect("re-seed an unchanged row");
        store
            .forget_session(host, "identity-1", "never-existed")
            .await
            .expect("forget a row that is not there");

        let after = store
            .merged_page(
                vec![host],
                None,
                10,
                usize::MAX,
                SessionFilter::default().title("s-1"),
                MatchingCount::ComputeUnless(before),
            )
            .await
            .expect("second read");
        assert_eq!(after.generation, before, "nothing observable changed");
        assert_eq!(
            after.matching, None,
            "so a held count is confirmed rather than recomputed"
        );
    }

    /// Counting and paging are ONE decode pass, and a walk pays for the count
    /// once.
    ///
    /// Spec: a filtered read that must count walks the scope exactly once —
    /// not once to count and again to page — and a walk whose later pages
    /// hand back a still-valid generation performs no counting pass at all.
    /// An invalidating write earns exactly one more.
    ///
    /// Instrumented rather than inferred from output, and that is the whole
    /// point of the test: an implementation that recounted on every page
    /// would produce identical numbers on every page, so nothing about the
    /// answers can tell the two apart. The counter is what can.
    #[tokio::test]
    async fn a_filtered_walk_counts_once_and_recounts_only_after_a_change() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "user@host", "identity-1").await;
        let entries: Vec<SessionInfo> = (0..6)
            .map(|index| SessionInfo {
                cwd: "/keep".to_string(),
                ..session(&format!("s-{index}"), 600 - index)
            })
            .collect();
        store
            .replace_host_sessions(host, "identity-1", entries.clone())
            .await
            .expect("seed the cache");

        let filter = SessionFilter::default().directory("/keep");
        let baseline = store.counting_passes();
        // Page one: nothing held, so this counts.
        let mut read = store
            .merged_page(
                vec![host],
                None,
                2,
                usize::MAX,
                filter.clone(),
                MatchingCount::Compute,
            )
            .await
            .expect("first page");
        assert_eq!(read.matching, Some(6));
        assert_eq!(
            store.counting_passes() - baseline,
            1,
            "one pass answers the page and the count together"
        );

        // The rest of the walk, each page resuming after the last and naming
        // the generation the count was taken at.
        let generation = read.generation;
        let mut pages = 1;
        while read.page.more {
            let after = read
                .page
                .rows
                .last()
                .map(|row| row.key.clone())
                .expect("a page that reports more has rows");
            read = store
                .merged_page(
                    vec![host],
                    Some(after),
                    2,
                    usize::MAX,
                    filter.clone(),
                    MatchingCount::ComputeUnless(generation),
                )
                .await
                .expect("a later page");
            assert_eq!(
                read.matching, None,
                "a later page of an unchanged walk must reuse the count, not recompute it"
            );
            pages += 1;
        }
        assert_eq!(pages, 3, "six matches at two per page is a three-page walk");
        assert_eq!(
            store.counting_passes() - baseline,
            1,
            "an unchanged walk counts exactly once, however many pages it takes"
        );

        // One invalidating write, then one recount — not one per page after
        // it either, since the fresh read hands back a new generation.
        store
            .replace_host_sessions(
                host,
                "identity-1",
                entries
                    .iter()
                    .cloned()
                    .chain([SessionInfo {
                        cwd: "/keep".to_string(),
                        ..session("s-6", 100)
                    }])
                    .collect(),
            )
            .await
            .expect("commit a seventh session");
        let recounted = store
            .merged_page(
                vec![host],
                None,
                2,
                usize::MAX,
                filter.clone(),
                MatchingCount::ComputeUnless(generation),
            )
            .await
            .expect("page after the write");
        assert_eq!(recounted.matching, Some(7));
        assert_eq!(
            store.counting_passes() - baseline,
            2,
            "exactly one recount for the change"
        );
        let _ = store
            .merged_page(
                vec![host],
                None,
                2,
                usize::MAX,
                filter,
                MatchingCount::ComputeUnless(recounted.generation),
            )
            .await
            .expect("and the walk is linear again");
        assert_eq!(store.counting_passes() - baseline, 2);
    }

    /// A host filter narrows the SQL scope, while the fleet total does not.
    ///
    /// Two properties in one read, and they pull in opposite directions:
    /// the page and the matching count must describe one host, and `total`
    /// must go on describing every host — otherwise "N matching of M" would
    /// compare a number against itself and always read as "all of them".
    #[tokio::test]
    async fn a_host_filter_narrows_the_page_but_not_the_fleet_total() {
        let (_dir, store) = fresh_store().await;
        let alpha = host_with_identity(&store, "user@alpha", "identity-alpha").await;
        let beta = host_with_identity(&store, "user@beta", "identity-beta").await;
        store
            .replace_host_sessions(alpha, "identity-alpha", vec![session("a-1", 300)])
            .await
            .unwrap();
        store
            .replace_host_sessions(
                beta,
                "identity-beta",
                vec![session("b-1", 200), session("b-2", 100)],
            )
            .await
            .unwrap();

        let read = store
            .merged_page(
                vec![alpha, beta],
                None,
                10,
                usize::MAX,
                SessionFilter::default().host(beta),
                MatchingCount::Compute,
            )
            .await
            .expect("host-filtered read");
        assert_eq!(read.total, 3, "the fleet is still three sessions");
        assert_eq!(read.matching, Some(2));
        assert_eq!(
            read.page
                .rows
                .iter()
                .map(|row| row.key.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["b-1", "b-2"]
        );

        // A host filter naming a host OUTSIDE the merged view selects
        // nothing — not everything, which is what an unguarded empty
        // IN-list would quietly mean.
        let outside = store
            .merged_page(
                vec![alpha],
                None,
                10,
                usize::MAX,
                SessionFilter::default().host(beta),
                MatchingCount::Compute,
            )
            .await
            .expect("out-of-scope host filter");
        assert_eq!(outside.matching, Some(0));
        assert!(outside.page.rows.is_empty());
        assert_eq!(outside.total, 1, "and the fleet total is unaffected by it");
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

    /// Parent matching happens during the merged scan, so a nonmatching row
    /// before or between children cannot consume the page limit or distort
    /// the fleet-wide matching count.
    #[tokio::test]
    async fn parent_filter_precedes_pagination_and_participates_in_the_count() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "parent@host", "parent-identity").await;
        let child = |id: &str, created_at| SessionInfo {
            parent: Some("root-session".to_string()),
            ..session(id, created_at)
        };
        store
            .replace_host_sessions(
                host,
                "parent-identity",
                vec![
                    session("unrelated-newest", 400),
                    child("child-a", 300),
                    session("unrelated-middle", 200),
                    child("child-b", 100),
                ],
            )
            .await
            .unwrap();
        let filter = SessionFilter::default().parent("root-session");
        let first = store
            .merged_page(
                vec![host],
                None,
                1,
                usize::MAX,
                filter.clone(),
                MatchingCount::Compute,
            )
            .await
            .unwrap();
        assert_eq!(first.total, 4);
        assert_eq!(first.matching, Some(2));
        assert_eq!(first.page.rows[0].key.session_id, "child-a");
        assert!(first.page.more);
        let second = store
            .merged_page(
                vec![host],
                Some(first.page.rows[0].key.clone()),
                1,
                usize::MAX,
                filter.clone(),
                MatchingCount::ComputeUnless(first.generation),
            )
            .await
            .unwrap();
        assert_eq!(second.page.rows[0].key.session_id, "child-b");
        assert!(!second.page.more);
        assert_eq!(second.matching, None, "the bound count is reused");
        assert_ne!(
            filter.fingerprint(),
            SessionFilter::default()
                .parent("another-root")
                .fingerprint(),
            "a cursor and count cannot cross parent filters"
        );
    }

    /// Completed drains converge the remembered default to their newest
    /// profile-backed source and never let an older snapshot roll it back.
    #[tokio::test]
    async fn drain_convergence_advances_only_to_newer_profile_provenance() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "profiles@host", "profile-identity").await;
        store
            .remember_profile_default_from_session(
                host,
                Some("profile-identity"),
                "profile-old",
                Some(2),
                200,
                "old-source",
            )
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
            )
            .await
            .unwrap();
        assert!(advanced.default_changed);
        assert_eq!(
            store.remembered_profile(host).await.unwrap().as_deref(),
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
            )
            .await
            .unwrap();
        assert!(!delayed.default_changed);
        assert_eq!(
            store.remembered_profile(host).await.unwrap().as_deref(),
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
            )
            .await
            .unwrap();
        assert!(!no_rollback.default_changed);
        assert_eq!(
            store.remembered_profile(host).await.unwrap().as_deref(),
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
            )
            .await
            .unwrap();
        assert!(retreated.default_changed);
        assert_eq!(
            store.remembered_profile(host).await.unwrap().as_deref(),
            Some("profile-older"),
            "when the remembered source disappears, the remaining newest source wins"
        );

        let cleared = store
            .replace_host_sessions(host, "profile-identity", Vec::new())
            .await
            .unwrap();
        assert!(cleared.default_changed);
        assert_eq!(store.remembered_profile(host).await.unwrap(), None);
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
            )
            .await
            .unwrap();
        assert_eq!(
            store.remembered_profile(host).await.unwrap().as_deref(),
            Some("profile-a")
        );
    }

    /// Direct create replies can arrive out of order; their sequence, not
    /// arrival order, decides the remembered default.
    #[tokio::test]
    async fn direct_remembered_default_refuses_out_of_order_sources() {
        let (_dir, store) = fresh_store().await;
        let host = host_with_identity(&store, "direct@host", "direct-identity").await;
        assert!(
            store
                .remember_profile_default_from_session(
                    host,
                    Some("direct-identity"),
                    "new",
                    Some(10),
                    100,
                    "new-source",
                )
                .await
                .unwrap()
        );
        assert!(
            !store
                .remember_profile_default_from_session(
                    host,
                    Some("direct-identity"),
                    "old",
                    Some(9),
                    200,
                    "old-source",
                )
                .await
                .unwrap()
        );
        assert_eq!(
            store.remembered_profile(host).await.unwrap().as_deref(),
            Some("new")
        );
    }

    /// A filter's canonical encoding distinguishes filters that select
    /// differently, including the shapes a delimiter-joined encoding would
    /// confuse.
    ///
    /// A collision here is not cosmetic. The encoding is what every derived
    /// identity of a filter is built from — the digest a cursor is bound to,
    /// and the key the matching count is cached under — so two different
    /// filters sharing one would let a cursor replay across them and let one
    /// filter's count be reported as another's. The field values are user
    /// text, so the encoding has to survive a user writing the separator.
    ///
    /// Asserted on the encoding rather than on the digest because the digest
    /// is keyed with a process-random key: it says nothing readable about WHY
    /// two filters differ, and this is the property that has to be readable.
    #[test]
    fn a_filter_fingerprint_cannot_be_forged_by_field_content() {
        let plain = SessionFilter::default().title("x").status("running");
        // The same two dimensions, and a title that spells out what a
        // delimiter-joined encoding would emit for the pair.
        let forged = SessionFilter::default().title("x|s=running");
        assert_ne!(plain.fingerprint(), forged.fingerprint());

        // Adjacent fields must not be able to borrow each other's text.
        assert_ne!(
            SessionFilter::default()
                .title("ab")
                .profile("c")
                .fingerprint(),
            SessionFilter::default()
                .title("a")
                .profile("bc")
                .fingerprint()
        );

        // Same filter, same fingerprint — a walk's later pages must be able
        // to recognize their own.
        assert_eq!(
            SessionFilter::default().directory("/srv").fingerprint(),
            SessionFilter::default().directory("/srv").fingerprint()
        );
        // And an absent dimension is distinguishable from an empty one.
        assert_ne!(
            SessionFilter::default().fingerprint(),
            SessionFilter::default().title("").fingerprint()
        );
        assert_ne!(
            SessionFilter::default().fingerprint(),
            SessionFilter::default()
                .include_archived(true)
                .fingerprint(),
            "a cursor minted with archived rows hidden must not replay after the toggle changes"
        );

        // The digest inherits both properties — it is what actually travels,
        // so a distinction the encoding makes and the digest loses would be
        // no distinction at all.
        assert_ne!(plain.digest(), forged.digest());
        assert_eq!(
            SessionFilter::default().directory("/srv").digest(),
            SessionFilter::default().directory("/srv").digest(),
            "a walk's later pages must recognize their own cursors"
        );
        assert_eq!(
            plain.digest().len(),
            16,
            "fixed-size, so a cursor cannot grow with the search box"
        );
    }

    /// The filter both halves of the cross-process digest test speak about.
    ///
    /// Every dimension is set, and every value is a constant: the point of the
    /// test is that two processes handed the SAME filter still disagree, so
    /// nothing here may vary between them.
    fn probe_filter() -> SessionFilter {
        SessionFilter::default()
            .host(7)
            .directory("/srv/work")
            .title("nightly")
            .profile("starter-claude")
            .status("running")
    }

    /// Print this process's digest for [`probe_filter`], one line, and exit.
    ///
    /// Not a test: it is the CHILD half of
    /// [`a_filter_digest_belongs_to_the_process_that_minted_it`], which
    /// re-executes this binary to obtain a digest minted under a genuinely
    /// different process key. `#[ignore]` is what keeps an ordinary run from
    /// executing it, and the parent passes `--ignored` to get it back.
    ///
    /// A subprocess rather than something cheaper because the key is a
    /// `OnceLock` minted once per process (see [`SessionFilter::digest`]) —
    /// there is no in-process way to obtain a second one, and a test that
    /// reached for one would be testing a seam the product does not have.
    #[test]
    #[ignore = "the child half of a_filter_digest_belongs_to_the_process_that_minted_it"]
    fn digest_probe() {
        println!("FH-DIGEST {}", probe_filter().digest());
    }

    /// A filter digest belongs to the PROCESS that minted it: a fresh helm
    /// computes a different one for the same filter, and therefore refuses
    /// every cursor the previous one issued.
    ///
    /// Spec: two fresh processes digesting [`probe_filter`] produce three
    /// distinct values between them and this one, none matching any other.
    ///
    /// The refusal is what this is really about. `crate::aggregate`'s page
    /// walk compares a cursor's carried binding against the digest of the
    /// filter the request actually names, and answers 400 when they differ —
    /// so "the digests differ" IS "the cursor is rejected", in both directions
    /// at once, since the comparison is symmetric. What would break it is a
    /// constant key: cursors would then be minted by anyone, off-line, and a
    /// helm would resume a walk under a binding it never issued. That failure
    /// is invisible in-process, which is why this pays for two subprocesses.
    ///
    /// The counterpart property — that ONE process recognizes its own filters
    /// — is pinned in `a_filter_fingerprint_cannot_be_forged_by_field_content`
    /// above; without it, this test would pass on a digest that was simply
    /// random per call and no walk could ever continue.
    #[test]
    fn a_filter_digest_belongs_to_the_process_that_minted_it() {
        /// Run this test binary again, in a fresh process, and read the
        /// digest it prints.
        fn digest_from_a_fresh_process() -> String {
            let exe = std::env::current_exe().expect("a test binary knows its own path");
            let run = std::process::Command::new(&exe)
                .args([
                    "--exact",
                    "store::tests::digest_probe",
                    "--ignored",
                    "--nocapture",
                ])
                .output()
                .unwrap_or_else(|error| panic!("re-running {exe:?}: {error}"));
            let text = String::from_utf8_lossy(&run.stdout).into_owned();
            assert!(
                run.status.success(),
                "the digest probe must run cleanly: {text}{}",
                String::from_utf8_lossy(&run.stderr)
            );
            text.lines()
                .find_map(|line| line.strip_prefix("FH-DIGEST "))
                .map(str::to_string)
                .unwrap_or_else(|| panic!("the probe must print exactly one digest line: {text}"))
        }

        let mine = probe_filter().digest();
        let first = digest_from_a_fresh_process();
        let second = digest_from_a_fresh_process();

        assert_ne!(
            first, second,
            "two fresh helms must not agree on a filter's digest, or a cursor minted by one would \
             resume a walk in the other"
        );
        assert_ne!(
            first, mine,
            "and neither of them agrees with this process, which is what makes a restarted helm \
             refuse the cursors it handed out before"
        );
        assert_ne!(second, mine);
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
}
