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
//! This module is STORAGE ONLY (PLAN_M6.md's order of work splits it from
//! the connection manager that will call it): schema, types, and CRUD, with
//! nothing in the helm's serving path wired to it yet. SPEC_impl.md's
//! "Helm internals" section carries the settled data model this schema
//! implements.
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
//! - **Concurrent-open safety despite the single-helm rule.** SPEC.md's
//!   "exactly one helm runs at a time" is an operating assumption this
//!   crate does not enforce anywhere — no lock file, no pid check, nothing
//!   stops a second helm process from ever being started by mistake.
//!   [`ensure_local_row`]'s conditional `ON CONFLICT` and
//!   [`HelmStore::add_ssh_host`]'s own conditional insert hold regardless: a
//!   second process racing the first over the same file converges on one
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

use anyhow::Context;
use farhelm_proto::SessionInfo;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
const SCHEMA_VERSION: i64 = 2;

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
    /// coat. `crate::ssh_args` already terminates ssh's option parsing
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
fn destination_is_usable(destination: &str) -> bool {
    !destination.is_empty() && !destination.starts_with('-')
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
             -- Serves HelmStore::cached_sessions_all's cross-host merge —
             -- the one read that legitimately spans every host, so it wants
             -- exactly this order with no host_id prefix to skip past.
             CREATE INDEX session_cache_order
                 ON session_cache (created_at DESC, session_id ASC);
             -- Serves HelmStore::cached_sessions's per-host read
             -- separately: session_cache_order above has no host_id column,
             -- so \"WHERE host_id = ? ORDER BY created_at DESC, session_id
             -- ASC\" against it alone would still have to walk every row
             -- from every host to filter, not just this host's. Leading
             -- with host_id here turns that into an index range scan
             -- instead.
             CREATE INDEX session_cache_by_host_order
                 ON session_cache (host_id, created_at DESC, session_id ASC);
             PRAGMA user_version = 2;",
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
            let kind: Option<String> = tx
                .query_row(
                    "SELECT kind FROM hosts WHERE id = ?1",
                    rusqlite::params![host],
                    |r| r.get(0),
                )
                .optional()
                .context("looking up host before updating its destination")?;
            let kind = kind.map(|k| HostKind::from_column(&k)).transpose()?;
            match kind {
                None => Err(anyhow::Error::new(HostStoreError::HostNotFound(host))),
                Some(HostKind::Local) => {
                    Err(anyhow::Error::new(HostStoreError::LocalHostImmutable))
                }
                Some(HostKind::Ssh) => {
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
    /// purging that host's `session_cache` rows in the SAME transaction —
    /// PLAN_M6.md item 4's user-initiated adoption of an identity-mismatched
    /// host (SPEC.md: the helm never silently merges; this is the explicit
    /// acknowledgment that performs the merge the user chose after seeing
    /// [`FirstContactOutcome::Mismatch`]).
    ///
    /// The purge is IN the transaction, never a follow-up call: a different
    /// identity at a known destination means the install that produced the
    /// OLD identity is gone, so its cached sessions describe agents that no
    /// longer exist behind this [`HostId`] — carrying them forward under the
    /// NEW identity would misattribute a dead install's history to a live
    /// one. A separate follow-up call could leave the two writes torn by a
    /// crash or a concurrent reader between them; one transaction cannot.
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
    pub async fn replace_host_sessions(
        &self,
        host: HostId,
        identity: &str,
        entries: Vec<SessionInfo>,
    ) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        let identity = identity.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
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
            tx.execute(
                "DELETE FROM session_cache WHERE host_id = ?1",
                rusqlite::params![host],
            )
            .context("clearing the stale cache")?;
            for entry in &entries {
                let json = serde_json::to_string(entry).context("serializing cached session")?;
                tx.execute(
                    "INSERT INTO session_cache (host_id, session_id, created_at, info_json) \
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![host, entry.id, entry.created_at, json],
                )
                .context("inserting cached session")?;
            }
            tx.commit().context("committing cache replace")?;
            Ok(())
        })
        .await
        .context("replace host sessions task panicked")?
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

    /// The cached sessions of EVERY host, merged into one wire-ordered
    /// list — the persistence half of PLAN_M6.md item 5's cross-host
    /// aggregation, which this same module's tests pin in isolation ahead
    /// of the connection manager PR that will actually call it at serving
    /// time. See [`CachedSession`]'s own docs for why the host id rides
    /// alongside each entry here but not in [`Self::cached_sessions`].
    ///
    /// Shares [`Self::cached_sessions`]'s skip-and-log posture on an
    /// undecodable `info_json` — see that method's own docs for the
    /// reasoning, which applies identically here since this is the same
    /// last-known DISPLAY data, merely merged across hosts instead of
    /// scoped to one.
    pub async fn cached_sessions_all(&self) -> anyhow::Result<Vec<CachedSession>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<CachedSession>> {
            let conn = conn.lock().expect("helm db mutex poisoned");
            let mut stmt = conn
                .prepare(
                    "SELECT host_id, session_id, info_json FROM session_cache \
                     ORDER BY created_at DESC, session_id ASC",
                )
                .context("preparing cross-host cached session query")?;
            let rows: Vec<(HostId, String, String)> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .context("querying cached sessions across hosts")?
                .collect::<Result<_, _>>()
                .context("reading cached session rows")?;
            Ok(rows
                .into_iter()
                .filter_map(
                    |(host, session_id, json)| match serde_json::from_str(&json) {
                        Ok(info) => Some(CachedSession { host, info }),
                        Err(error) => {
                            tracing::warn!(
                                host,
                                session_id = session_id.as_str(),
                                error = %error,
                                "skipping a cached session whose info_json no longer decodes"
                            );
                            None
                        }
                    },
                )
                .collect())
        })
        .await
        .context("cached sessions all task panicked")?
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
            id: id.to_string(),
            title: id.to_string(),
            created_at,
            cwd: format!("/{id}"),
            invocation: "agent".to_string(),
            status: farhelm_proto::SessionStatus::Alive,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
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
        crate::test_capture::matching(
            &crate::test_capture::install(),
            "info_json no longer decodes",
        )
    }

    // ---- Schema and the version mechanism ----------------------------

    /// A fresh database must come up on `user_version` 1 with the reserved
    /// local row already present — the two invariants every other test in
    /// this module assumes without re-checking.
    #[tokio::test]
    async fn fresh_open_creates_version_1_with_the_local_row_present() {
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

    /// The migration-fixture scaffold `apply_schema`'s own docs promise:
    /// even though version 1 (this PR) has no migration step to pin, the
    /// REFUSAL half of the version mechanism — a database claiming a
    /// version this build does not understand — is exercised exactly like
    /// the supervisor store's `open_refuses_an_unrecognized_schema_version`,
    /// by planting a raw `user_version` directly with rusqlite rather than
    /// through `HelmStore::open`. The day a version 2 migration lands, its
    /// test plants a version-1 fixture the same way this plants a
    /// too-new one.
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
            let cached = serde_json::to_string(&session("ghost", 100)).unwrap();
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
            conn.execute(
                "INSERT INTO session_cache (host_id, session_id, created_at, info_json)
                 SELECT id, 'ghost', 100, ?1 FROM hosts WHERE host_identity IS NOT NULL",
                rusqlite::params![cached],
            )
            .expect("give every identified row a cache");
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
    /// neither does an empty one. `crate::ssh_args`' terminator placement
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

        let all = store.cached_sessions_all().await.unwrap();
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

        let all = store.cached_sessions_all().await.unwrap();
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

        let all = store
            .cached_sessions_all()
            .await
            .expect("a poisoned row on one host must not fail the whole cross-host read");
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
}
