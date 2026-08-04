//! The helm connection manager: one connection actor per registry host
//! (PLAN_M6.md item 4).
//!
//! M1's helm held exactly one `SupervisorClient`, chosen by argv, with no
//! reconnection and no notion of a host beyond "the one we dialed". This
//! module is that transport generalized: every row in the registry — the
//! reserved local row included — gets an independent actor owning its own
//! transport connection, its own reconnect state machine, and its own slice
//! of the session cache. Everything a host's CONNECTION can be is one of
//! the six [`HostState`] variants PLAN_M6.md item 4 fixes (plus
//! [`HostState::Retired`], which describes this side rather than the host),
//! and every transition between them is logged, because SPEC.md requires
//! per-host connection state to be *always visible* and its reconnection
//! trail to be *available*.
//!
//! ## Why `manager.rs` and not `hosts.rs`
//!
//! [`crate::store`] already owns the vocabulary of a *host* — `HostRow`,
//! `HostId`, `HostKind`, the registry table itself. A second module named
//! for the same noun would leave a reader guessing which of the two owns,
//! say, "what is this host's identity" (the store: it is the durable
//! record) versus "what is this host doing right now" (here: it is
//! runtime). PLAN_M6.md item 4 names the thing being built — the helm
//! *connection manager* — and naming the module after the mechanism rather
//! than the noun keeps that split legible from the file list alone.
//!
//! ## What this module is NOT wired to yet
//!
//! Nothing user-visible. PLAN_M6.md's order of work puts aggregation, REST
//! host management, and session-op routing in the NEXT PR; the helm's
//! serving path today is still `AppState`'s single client, untouched. This
//! module is built, tested, and exported so that PR has a substrate to
//! wire, not a design to invent. The public surface is shaped for exactly
//! those consumers: [`ConnectionManager::snapshots`] is the hosts list REST
//! will render, [`ConnectionManager::client`] is how a session operation
//! finds its host's live connection, and [`ConnectionManager::adopt`] is
//! the identity-mismatch resolution verb.
//!
//! ## The shape of an actor
//!
//! One task per host, looping forever over the same three questions:
//! should this entry connect at all (duplicate/mismatch freezes say no),
//! can it connect (the backoff-then-re-probe ladder), and — once connected
//! — keeping its cache fresh until the connection dies. An actor is the
//! only thing that OPENS or drives that host's connection, and the only
//! thing that writes its cache in the ordinary course of events, which is
//! what makes a flapping host's behavior a local property rather than
//! something that has to be reasoned about against every other host at
//! once. Two user decisions reach in from outside and are worth naming
//! rather than leaving as exceptions to an over-broad claim:
//! [`ConnectionManager::adopt`] purges a host's cache rows through the
//! store, and [`ConnectionManager::sync_registry`] retires an actor's
//! published connection when the row it was dialing is edited. Both are
//! transactional or lock-held at the point they happen, and both hand the
//! actor back the work of reconnecting.
//!
//! Actors are cancel-driven: [`ConnectionManager::shutdown`] and a host's
//! removal both abort the task outright. That is safe because every piece
//! of state an actor mutates outside its own memory is a single
//! transactional [`HelmStore`] call — an abort can lose the fact that a
//! refresh was in flight, never leave one half-applied.

use crate::client::{SupervisorClient, SupervisorError};
use crate::store::{
    DialedAs, FirstContactOutcome, HelmStore, HostId, HostKind, HostRow, HostStoreError,
};
use anyhow::Context as _;
use farhelm_proto::io::VersionSkew;
use farhelm_proto::{ErrorKind, SessionInfo};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::watch;
use tracing::{Instrument, debug, info, info_span, warn};

// ---- Cadences -------------------------------------------------------
//
// The user's settled answer for M6 was "snappy" (PLAN_M6.md item 4, user
// decision 2026-08-04), and every number below is a consequence of one
// requirement: a host that comes back must be noticed within about a
// minute of returning, without a healthy fleet paying for that in constant
// polling. These are implementation choices rather than product promises —
// SPEC.md's Errors section owns the CONTRACT (bounded retries, then
// periodic re-probing, phase visible) and says nothing about the values —
// so they are recorded in SPEC_impl.md's helm-internals section alongside
// the rest of the helm's settled internals.

/// The active-retry ladder: how long to wait BEFORE each retry, after the
/// immediate first attempt.
///
/// Six delays summing to sixty seconds, so the active window is `1 + 6`
/// attempts spread over about a minute (t = 0, 1, 3, 7, 15, 30, 60
/// seconds). The early entries are what make a momentary blip — an ssh
/// ControlMaster expiring, a supervisor restarted by hand — recover in
/// about a second rather than at the re-probe cadence; the doubling is
/// what keeps a host that is genuinely gone from being hammered for the
/// whole minute.
///
/// Deliberately a list rather than a base-and-multiplier: the tail is
/// hand-capped (15, 30 — not 16, 32) so the ladder lands on a round minute,
/// and a reader comparing this against PLAN_M6.md's prose should see the
/// same six numbers, not have to evaluate a formula to check.
pub const CONNECT_BACKOFF: [Duration; 6] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(15),
    Duration::from_secs(30),
];

/// How often an unreachable, version-skewed, or duplicate host is
/// re-probed once its active-retry window is spent — forever, never
/// escalating to a give-up.
///
/// Forty-five seconds is the "within about a minute" promise made
/// affordable: worst case a returning host waits one full period, which
/// stacked on the active window's tail still lands inside the minute a
/// user was told to expect, while a fleet of down hosts costs a little over
/// one connection attempt per host per minute.
///
/// The SAME cadence deliberately serves three different-looking states.
/// Version skew re-probes on it because the fix is a binary upgrade on the
/// host, which the helm cannot observe any other way — so an upgraded host
/// resurfaces by itself, alone, with no user action (PLAN_M6.md item 4).
/// Duplicate entries re-probe on it not to dial anything (a duplicate
/// connects nothing) but to re-ask the registry whether they are still
/// duplicates, so removing the twin unsticks the entry on the same
/// timescale everything else recovers on.
pub const REPROBE_INTERVAL: Duration = Duration::from_secs(45);

/// How often a CONNECTED host's session list is drained into the cache.
///
/// Matched to `farhelm-ui`'s own `POLL_INTERVAL_MS` (three seconds), which
/// is the freshness the single-host path already delivers today: the
/// browser polls the helm every three seconds and the helm answers with a
/// live round trip. Multi-host aggregation moves that round trip off the
/// request path and behind this cadence, so keeping the number identical
/// is what stops the visible list from getting *staler* than it is now as
/// a side effect of gaining hosts. M6.75's push channel is what eventually
/// retires polling on both sides.
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(3);

/// Hard ceiling on pages one [`drain_sessions`] walk will follow.
///
/// Not a tuning knob — a termination guarantee. The walk's loop condition
/// is a value the PEER supplies (`next_cursor`), so a supervisor that is
/// buggy, or malicious, or merely racing its own session churn in a way
/// nobody anticipated, could hand back a cursor forever and park this
/// actor in an unbounded loop. This bound is what catches the pathological
/// shape [`REFRESH_SESSION_CAP`] cannot: a peer that answers every page
/// with zero sessions and a fresh cursor never accumulates anything to
/// count, so only a page ceiling ends it.
pub const REFRESH_PAGE_LIMIT: usize = 1_000;

/// Hard ceiling on sessions one [`drain_sessions`] walk will accumulate.
///
/// The memory half of the same termination guarantee: pages are bounded
/// above, but a peer serving full pages could still hand this actor
/// half a million `SessionInfo`s before [`REFRESH_PAGE_LIMIT`] fired, and
/// they are all held at once because the cache is replaced wholesale.
///
/// Ten times the supervisor's own default page cap
/// (`LIST_SESSION_CAP`, 500), i.e. ten full pages: honest headroom rather
/// than a snug fit, since an ordinary host's whole list arrives in ONE page
/// and even an implausibly busy fleet is nowhere near this. A host that
/// exceeds it gets a failed refresh with its previous cache preserved (see
/// [`HostActor::refresh_once`]) — the same treatment as any other refusal,
/// which is the point: a walk that will not terminate is a broken refresh,
/// not a reason to lose what the host last told us.
pub const REFRESH_SESSION_CAP: usize = 5_000;

/// How long ONE connection attempt — the dial AND the hello it must
/// complete — may take before it counts as failed.
///
/// Without a bound here the whole ladder can be parked by a transport that
/// accepts a connection and then says nothing: ssh to a host whose network
/// blackholes after the TCP handshake, a wedged remote proxy, a supervisor
/// stuck before its own hello. The retry ladder's timing promise (a host
/// that comes back is noticed within about a minute) is not a promise the
/// code can keep if a single attempt can outlive the entire window.
///
/// Twenty seconds is chosen against the slowest LEGITIMATE attempt: a cold
/// ssh connection doing full key exchange and authentication over a bad
/// link, plus the remote proxy dialing its own supervisor socket. Anything
/// slower than that is indistinguishable from stuck, and treating it as a
/// failed attempt costs only a retry — expiry is an ordinary failure, so
/// the ladder and the re-probe cadence carry on exactly as they would for a
/// refused connection.
pub const CONNECT_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(20);

/// How long one cache refresh may take before the CONNECTION is torn down.
///
/// A connected host that stops answering `ListSessions` — without closing
/// the connection, so nothing EOFs — would otherwise park `serve` forever:
/// no refresh completes, no loss is detected, and the host reads as
/// healthily connected while it answers nothing. Expiry drops the
/// connection so the actor re-enters its normal loss handling, where a
/// genuinely wedged peer at least surfaces as unreachable.
///
/// Deliberately much larger than [`REFRESH_INTERVAL`] and larger than
/// [`CONNECT_ATTEMPT_TIMEOUT`]: a refresh may legitimately walk several
/// pages, and each page costs the supervisor a whole-host capture sweep
/// (see [`drain_sessions`]). Thirty seconds is well past any plausible
/// honest walk while still bounded by something a user would notice.
pub const REFRESH_TIMEOUT: Duration = Duration::from_secs(30);

/// The cadences an actor runs on and the deadlines it holds a peer to,
/// injectable so tests do not have to wait out production timescales.
///
/// Two kinds of number, and the distinction matters when picking values
/// for a test: the CADENCES say how often this side acts, and shortening
/// one only makes a test faster; the DEADLINES
/// ([`Self::attempt_timeout`], [`Self::refresh_timeout`]) say how long a
/// peer may say nothing before it is treated as broken, and shortening one
/// can turn a merely slow host into a failing one. Tests against real
/// processes therefore shorten the cadences and leave the deadlines alone.
///
/// The repo's established discipline for time in tests is tokio's virtual
/// clock (`#[tokio::test(start_paused = true)]` plus `tokio::time::advance`
/// — see `farhelm-supervisor`'s connection and terminal tests), and the
/// state-machine tests below use exactly that AGAINST THE PRODUCTION
/// VALUES: a paused clock makes asserting "the fourth retry happens at
/// t=7s, not t=6s" free and exact, so the real ladder is what gets pinned
/// rather than a test-only stand-in for it.
///
/// This struct exists for the OTHER kind of test — the ones driving real
/// ssh and real unix sockets, where a real supervisor process and a real
/// scheduler are in the loop and the virtual clock is therefore
/// unavailable. Those need a short re-probe so that a legitimately slow
/// startup does not read as a hang, and they say so explicitly here rather
/// than sleeping through a production forty-five seconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cadence {
    /// Waits before each active retry; see [`CONNECT_BACKOFF`]. An empty
    /// ladder is legal and means "one attempt, then straight to
    /// re-probing".
    pub connect_backoff: Vec<Duration>,
    /// See [`REPROBE_INTERVAL`].
    pub reprobe: Duration,
    /// See [`REFRESH_INTERVAL`].
    pub refresh: Duration,
    /// See [`CONNECT_ATTEMPT_TIMEOUT`].
    pub attempt_timeout: Duration,
    /// See [`REFRESH_TIMEOUT`].
    pub refresh_timeout: Duration,
}

impl Default for Cadence {
    fn default() -> Self {
        Cadence {
            connect_backoff: CONNECT_BACKOFF.to_vec(),
            reprobe: REPROBE_INTERVAL,
            refresh: REFRESH_INTERVAL,
            attempt_timeout: CONNECT_ATTEMPT_TIMEOUT,
            refresh_timeout: REFRESH_TIMEOUT,
        }
    }
}

// ---- The connection-state taxonomy ----------------------------------

/// Why a host is unreachable, to the extent this side can tell.
///
/// Only one distinction is drawn, and only because the REMEDY differs. For
/// every other cause the honest answer is the transport's own error text,
/// which the state carries verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnreachableCause {
    /// The reserved local row's supervisor is not running: nothing is
    /// listening on the unix socket in the helm's own state directory.
    ///
    /// Distinguished because this is the one unreachable host the user can
    /// fix with a command on the machine they are already sitting at, and
    /// PLAN_M6.md is explicit that a registered destination with no
    /// supervisor gets a manual-path hint and never an offer to install
    /// (provisioning is M7's). The hint TEXT is the UI's, next PR; what
    /// this variant guarantees is that the manager distinguishes the case
    /// honestly instead of collapsing it into a generic connection
    /// failure the UI could only guess at.
    LocalSupervisorNotRunning,
    /// Anything else: ssh refused, the host is down, the remote proxy
    /// exited, DNS failed. Undifferentiated on purpose — the error string
    /// beside it is more informative than any classification this side
    /// could invent, and ssh's own stderr reaches the operator directly.
    TransportFailure,
}

/// What one refresh means for the CONNECTION, as distinct from what it
/// means for the cache.
///
/// The two are usually independent — a failed refresh is emphatically not a
/// lost connection, which is why [`RefreshHealth`] lives inside
/// [`HostState::Connected`] — but two failures are also verdicts on the
/// connection itself: a peer that stopped answering at all, and a write
/// refused because this connection's identity is no longer the row's.
/// Continuing to serve on either would mean a host that reads as connected
/// while every refresh it makes is doomed identically.
struct RefreshStep {
    health: RefreshHealth,
    /// `Some(reason)` tears the connection down and re-enters the connect
    /// path; the reason is log text, not a state (the state the user sees
    /// is whatever the reconnection produces).
    end_connection: Option<&'static str>,
}

/// How a connected host's most recent cache refresh went.
///
/// Lives inside [`HostState::Connected`] rather than beside it because a
/// failed refresh does NOT disconnect the host: the connection is fine,
/// one list walk was not. Collapsing the two would make a host that is
/// answering perfectly well look unreachable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefreshHealth {
    /// Connected, but the first refresh has not completed yet.
    Pending,
    /// The last refresh drained cleanly.
    ///
    /// "Drained", not "cached": a host with no identity to bind a cache
    /// write to reaches this variant too, having listed its sessions live
    /// and written nothing (see [`HostState::Connected`]'s `identity`).
    /// The count below is real either way — it is what the walk returned —
    /// but it is not a claim that this host's cache slice now holds that
    /// many rows.
    Ok {
        /// How many sessions the walk returned in total, across pages.
        sessions: usize,
    },
    /// The last refresh failed; the PREVIOUS cache is still in place. See
    /// [`HostActor::refresh_once`] for why nothing is wiped on failure.
    Failed { error: String },
}

/// Everything a registry entry's connection can be, and nothing else —
/// the taxonomy PLAN_M6.md item 4 fixes — plus one variant that is about
/// this side instead ([`Self::Retired`], whose own docs say why it is here
/// rather than absent).
///
/// The six connection variants are not a status enum bolted onto a
/// connection; they
/// ARE the connection's state machine, and three of them (`VersionSkew`,
/// `IdentityMismatch`, `Duplicate`) exist specifically because collapsing
/// them into "unreachable" would destroy the only information that makes
/// the situation fixable. SPEC.md's rule that errors be *actionable*, not
/// merely diagnostic, is what forces each of those three to carry its own
/// evidence rather than a shared error string.
///
/// Each variant's own docs give the transition rules INTO and OUT OF it;
/// [`HostActor::run`] is the one place that performs them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostState {
    /// Inside the active-retry window: attempts are being made right now,
    /// on the [`Cadence::connect_backoff`] ladder.
    ///
    /// Entered at startup, after any connection loss, and whenever a
    /// freeze is resolved. Left for `Connected` on a successful hello, for
    /// one of the three refusal states when the hello is answered but
    /// refused, or for `Unreachable` once the ladder is spent.
    Connecting {
        /// 0 for the immediate first attempt, then 1.. for each ladder
        /// step — the number the diagnostic trail reports and the UI's
        /// "retrying" phase can render progress from.
        attempt: u32,
        /// Why the previous attempt failed; `None` before the first one
        /// has been made.
        last_error: Option<String>,
    },
    /// The active window is spent and background re-probing has taken
    /// over, at [`Cadence::reprobe`], forever.
    ///
    /// SPEC.md's "a host that comes back overnight resurfaces by itself"
    /// is this state: there is no terminal give-up, and no user action is
    /// required to leave it.
    Unreachable {
        cause: UnreachableCause,
        /// The last connection failure verbatim, including the ssh or
        /// socket-level context that names the actual problem.
        last_error: String,
    },
    /// A hello succeeded and its identity question was settled. The host's
    /// sessions refresh on [`Cadence::refresh`] until the connection dies.
    Connected {
        /// The identity this host reported, as now recorded. `None` only
        /// for a supervisor that reported none at all (see
        /// [`crate::client::PeerHello`]) — a host in that state serves
        /// live but writes NO cache, because the cache's identity binding
        /// has nothing to bind to.
        identity: Option<String>,
        /// The supervisor's build version from its hello — the value that
        /// makes "which side is old" answerable in a support conversation.
        build_version: String,
        last_refresh: RefreshHealth,
    },
    /// The hello was answered and REFUSED: the peer speaks a protocol
    /// version this helm cannot talk to.
    ///
    /// A state of its own, never folded into `Unreachable`, because the
    /// two call for opposite responses — waiting does not fix skew, and
    /// upgrading a binary does not fix an unplugged network cable. Carries
    /// both versions (so the user can see WHICH side is behind) and the
    /// remediation text (so SPEC.md's actionable-error rule is satisfied
    /// by the state itself rather than by whatever prose a renderer
    /// improvises). Re-probed on [`Cadence::reprobe`], so an upgraded host
    /// resurfaces on its own with no user action here.
    VersionSkew {
        peer_protocol: u32,
        peer_build: String,
        our_protocol: u32,
        our_build: String,
        /// What the user should actually do. Held as data rather than
        /// derived at render time so every surface — REST body, log line,
        /// UI chip — says the same thing.
        remediation: String,
    },
    /// FROZEN: the destination answered with an identity that is not the
    /// one on record for this entry.
    ///
    /// Nothing is connected and nothing is written (the store's
    /// first-contact API refuses the write itself — see
    /// [`HelmStore::record_first_contact`]), because SPEC.md forbids
    /// silently merging two installs. The ONLY exits are user decisions:
    /// [`ConnectionManager::adopt`] (accept the new identity, purging the
    /// old one's cache) or fixing the destination and calling
    /// [`ConnectionManager::retry_now`]. Deliberately NOT re-probed: a
    /// timer cannot resolve a question only a human can answer, and
    /// re-probing would churn the log with a decision nobody made.
    IdentityMismatch {
        /// The identity this registry row already holds.
        recorded: String,
        /// The identity the destination just reported.
        reported: String,
    },
    /// This entry reaches an identity that ANOTHER registry entry already
    /// holds — two rows, one host.
    ///
    /// Connects nothing while it stays one, so the host appears exactly
    /// once (under `twin`, whose actor is the one that owns it) while this
    /// entry remains visible as something the user must resolve by editing
    /// or removing it. That is how SPEC.md's shown-once rule and the
    /// user's ability to fix a mis-typed destination coexist.
    ///
    /// Entered on the STORE's answer, not on a check this side made first:
    /// the identity claim is resolved inside the same transaction that
    /// would have recorded it (see [`HelmStore::record_first_contact`]), so
    /// two entries racing one host cannot both believe they won.
    ///
    /// Re-evaluated on [`Cadence::reprobe`] against the registry — a
    /// REGISTRY read, not a dial: if the twin no longer holds the identity
    /// (it was removed, or adopted a different one) this entry stops being
    /// a duplicate and goes back to connecting, unaided. A registry that
    /// cannot be READ leaves the freeze in place, since "I could not check"
    /// is not "there is no twin". The other exit is the user's: editing
    /// this entry's destination reconnects it (see
    /// [`ConnectionManager::sync_registry`]), which is how a duplicate
    /// resolves while its twin stays exactly where it is.
    ///
    /// "Connects nothing" is precise about one boundary worth stating: an
    /// identity is only knowable from a hello, so discovering the
    /// collision costs exactly one connection, which is then dropped
    /// immediately and never reopened while this state holds. The same is
    /// true of [`Self::IdentityMismatch`]. There is no way to learn who is
    /// at the other end without asking.
    Duplicate {
        /// The registry row that already owns this identity.
        twin: HostId,
        /// The shared identity, so a renderer can name the collision
        /// without a second lookup.
        identity: String,
    },
    /// This entry has no actor any more: the task that owned its
    /// connection finished, and nothing here will attempt anything until
    /// the next [`ConnectionManager::sync_registry`] gives it a new one.
    ///
    /// Not a connection state like the six above — it describes THIS SIDE,
    /// not the host — and it exists because the alternative is a lie that
    /// never expires. An actor's last published status stays registered
    /// after the actor is gone, so a task that panicked mid-connection
    /// would leave the entry reading `Connected`, with a live-looking
    /// client, forever: every session operation routed there would fail in
    /// a way the hosts list flatly contradicts. Publishing this instead
    /// costs the same operation an honest refusal.
    ///
    /// Reached two ways, and `reason` is what tells them apart: an actor
    /// that RETIRED itself (its registry row was removed out from under it)
    /// and one that PANICKED. Neither is re-probed — an actor is not
    /// something a timer can restart — but both are visible, which is the
    /// whole point.
    Retired { reason: String },
}

impl HostState {
    /// A short, stable label for this state's PHASE — what the diagnostic
    /// trail logs on a transition and what a UI chip is keyed off.
    ///
    /// Stable across changes to a variant's payload on purpose: a refresh
    /// failure or a further retry within the same phase must not read as a
    /// phase change, in the log or anywhere else. [`HostActor::set_state`]
    /// is what relies on that to keep the transition trail meaningful
    /// rather than one line per refresh tick.
    pub fn phase(&self) -> &'static str {
        match self {
            HostState::Connecting { .. } => "connecting",
            HostState::Unreachable { .. } => "unreachable-reprobing",
            HostState::Connected { .. } => "connected",
            HostState::VersionSkew { .. } => "version-skew",
            HostState::IdentityMismatch { .. } => "identity-mismatch",
            HostState::Duplicate { .. } => "duplicate",
            HostState::Retired { .. } => "retired",
        }
    }

    /// Whether a session operation may be routed to this host right now.
    ///
    /// The next PR refuses operations against any non-connected host with
    /// the host's state named in the error (PLAN_M6.md item 5): unreachable
    /// is not special, it is just the common case. Expressed here, once, so
    /// that adding a seventh state cannot silently become "routable by
    /// default" at some call site that forgot about it.
    pub fn is_connected(&self) -> bool {
        matches!(self, HostState::Connected { .. })
    }
}

/// One host's registry facts plus its live state, as
/// [`ConnectionManager::snapshots`] hands them out.
///
/// The registry fields are copied into the snapshot rather than left for a
/// caller to re-read from the store, because the pair has to be coherent:
/// a hosts list that showed a freshly-edited destination beside the state
/// of the connection to the OLD one would be worse than either alone.
///
/// That coherence is upheld at the one place it could be broken, and only
/// because it is upheld there: [`ConnectionManager::sync_registry`] swaps
/// the row and publishes the row's new, non-connected state inside a single
/// hold of the actor map's lock — the same lock every snapshot is taken
/// under — and the actor whose connection is being retired publishes
/// nothing further about it (see [`HostActor::serve`]). Without both
/// halves, the window between "the edit landed" and "the actor noticed" is
/// exactly a window in which this type lies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostSnapshot {
    pub id: HostId,
    pub kind: HostKind,
    /// `None` for the reserved local row, always `Some` for an ssh row.
    pub destination: Option<String>,
    pub state: HostState,
}

// ---- The transport seam ---------------------------------------------

/// One opened connection's reader/writer pair, type-erased.
///
/// Boxed `dyn` rather than generics because the manager holds a
/// heterogeneous set of actors behind one trait object: the local row
/// speaks over a unix socket, ssh rows over an exec channel, and a test
/// peer over an in-memory duplex. `SupervisorClient` is already
/// transport-blind by construction (SPEC_impl.md's transport section), so
/// erasing the type here costs nothing it was relying on.
pub type TransportPair = (
    Box<dyn AsyncRead + Send + Unpin>,
    Box<dyn AsyncWrite + Send + Unpin>,
);

/// How an actor opens a FRESH connection to its host.
///
/// One method, called once per connection attempt, returning a brand-new
/// pair every time — there is no reuse and no pooling, because a reconnect
/// after a loss must not be able to hand back the corpse of the connection
/// that just died.
///
/// A trait rather than a concrete enum so tests inject scripted supervisor
/// peers over `tokio::io::duplex` without a real process, a real socket,
/// or a real ssh anywhere in the loop — which is what makes the full
/// state machine (backoff timing, skew, identity, duplicates) testable
/// under a paused clock at all.
///
/// The returned future is boxed by hand rather than declared with `async
/// fn`: an `async fn` in a trait is not dyn-compatible, and dyn dispatch
/// is the entire point here.
pub trait HostTransport: Send + Sync + 'static {
    /// Open a connection for `host`. The row is passed whole (not just its
    /// destination) because an ssh row's `remote_farhelm` and
    /// `remote_state_dir` are part of how it is reached, and a test
    /// transport keys off `id`.
    fn connect<'a>(
        &'a self,
        host: &'a HostRow,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<TransportPair>> + Send + 'a>>;
}

/// The production transport: unix socket for the reserved local row, the
/// user's own `ssh` running `farhelm internal stdio` for an ssh row.
///
/// The exact two paths `connect_supervisor` in `lib.rs` has taken since
/// M1, generalized from "whatever argv said" to "whatever this row says".
/// The ssh argv itself is still built by [`crate::ssh_args`] — shared
/// rather than reimplemented, because its quoting rules are the subtlest
/// correctness surface in the transport and two copies would eventually
/// disagree.
pub struct SystemTransport {
    /// The helm's own state directory: where the local supervisor's socket
    /// lives, and where ssh ControlMaster sockets are kept.
    state_dir: std::path::PathBuf,
}

impl SystemTransport {
    /// `state_dir` must be the helm's OWN state directory, already
    /// established at `0700` by its caller — it is both where the local
    /// supervisor's socket is looked for and where ssh ControlMaster
    /// sockets are written, and neither is a location this type is free to
    /// choose for itself.
    pub fn new(state_dir: impl Into<std::path::PathBuf>) -> SystemTransport {
        SystemTransport {
            state_dir: state_dir.into(),
        }
    }
}

/// Marker attached to a local-row dial that failed because nothing is
/// listening — the evidence [`HostActor`] classifies
/// [`UnreachableCause::LocalSupervisorNotRunning`] from.
///
/// A typed payload rather than a string match on the error, for the same
/// reason `farhelm_proto::io::ClosedBeforeHello` is one: the message text
/// is a diagnostic for humans and must stay free to change, while a state
/// machine reading it would silently break on a rewording.
#[derive(Debug, thiserror::Error)]
#[error("no supervisor is running on this machine")]
pub struct LocalSupervisorNotRunning;

impl HostTransport for SystemTransport {
    fn connect<'a>(
        &'a self,
        host: &'a HostRow,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<TransportPair>> + Send + 'a>> {
        Box::pin(async move {
            match host.kind {
                HostKind::Local => {
                    let stream = farhelm_supervisor::service::connect(&self.state_dir)
                        .await
                        .map_err(classify_local_dial)?;
                    let (r, w) = tokio::io::split(stream);
                    Ok((
                        Box::new(r) as Box<dyn AsyncRead + Send + Unpin>,
                        Box::new(w) as Box<dyn AsyncWrite + Send + Unpin>,
                    ))
                }
                HostKind::Ssh => {
                    let dest = host.destination.as_deref().context(
                        "an ssh registry row has no destination; the schema's CHECK constraint \
                         should have made this impossible",
                    )?;
                    let control_path = self.state_dir.join("ssh-cm-%C");
                    let mut cmd = tokio::process::Command::new("ssh");
                    cmd.args(crate::ssh_args(
                        dest,
                        &control_path,
                        host.remote_farhelm.as_deref().unwrap_or("farhelm"),
                        host.remote_state_dir.as_deref(),
                    )?);
                    // stderr is PIPED and relayed as tracing events, not
                    // inherited. Inheriting is what the M1 single-host path
                    // does, and it was defensible there: one connection the
                    // user started by hand, whose ssh diagnostics belong on
                    // the terminal they are watching. Here the far end is a
                    // registered host running a command the helm chose but
                    // the REMOTE side controls the output of — and anything
                    // written to an inherited stderr reaches the operator's
                    // terminal as raw bytes, unbounded, with escape
                    // sequences intact. A remote that repaints the screen,
                    // hides the cursor, or simply never stops writing would
                    // be doing it to the helm's own console. Relaying
                    // instead keeps ssh's genuinely actionable diagnostics
                    // (auth failure, unresolvable host) while making them
                    // bounded, escaped, and attributable to a host.
                    let mut child = cmd
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .kill_on_drop(true)
                        .spawn()
                        .context("spawning ssh")?;
                    let stdout = child.stdout.take().expect("piped stdout");
                    let stdin = child.stdin.take().expect("piped stdin");
                    let stderr = child.stderr.take().expect("piped stderr");
                    // Drained continuously rather than at exit: a pipe
                    // nobody reads fills up, and a full stderr blocks the
                    // ssh child's writes — which for a chatty remote means
                    // the transport wedges for reasons no log line would
                    // ever explain.
                    tokio::spawn(relay_ssh_stderr(host.id, dest.to_string(), stderr));
                    Ok((
                        Box::new(SshChannel {
                            stdout,
                            _child: child,
                        }) as Box<dyn AsyncRead + Send + Unpin>,
                        Box::new(stdin) as Box<dyn AsyncWrite + Send + Unpin>,
                    ))
                }
            }
        })
    }
}

/// An ssh exec channel's read half WITH the ssh child that produces it,
/// so the child's lifetime is the transport's lifetime and nothing else.
///
/// The child used to be parked in a detached `wait()` task instead, which
/// reaped it but did not own it: closing the pipes only asks ssh to exit,
/// and an ssh (or a remote proxy) that ignores EOF simply kept running —
/// past a cancelled attempt, past a torn-down actor, one survivor per
/// retry for a host that keeps failing late in the handshake. Holding the
/// `Child` here makes teardown structural: dropping the transport pair —
/// which is what a cancelled attempt, a lost connection, and a reconfigured
/// row all do — drops this value, and tokio's `kill_on_drop` both signals
/// the child and hands it to the runtime's orphan reaper, so there is
/// nothing left to leak and no zombie to collect by hand.
///
/// Only the read half is wrapped: the pair is created and dropped
/// together, so one anchor is enough, and the writer stays a plain
/// `ChildStdin` whose close is what asks ssh to exit politely first.
///
/// The child's STDERR is not here — it is piped and drained by a task of
/// its own (see [`relay_ssh_stderr`]), which ends when the child's stderr
/// closes, i.e. when the child this value owns exits.
struct SshChannel {
    stdout: tokio::process::ChildStdout,
    _child: tokio::process::Child,
}

impl AsyncRead for SshChannel {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

/// Longest error text this side will log or keep in a [`HostState`].
///
/// Every string that reaches a state or a log line from OUTSIDE has passed
/// through the peer at least partly: a supervisor's `Error.message` is
/// wholly its own text, and a transport failure's chain can carry remote
/// prose too. The wire's frame cap is measured in megabytes, and a
/// connected host is refreshed every few seconds — so an unbounded copy of
/// the peer's message is both a log flood and a per-host retention leak
/// that lasts as long as the state does.
///
/// A kilobyte is far past any real diagnostic (the supervisor's own
/// refusals are one sentence) and far below anything that matters as
/// retention.
const PEER_TEXT_CAP: usize = 1024;

/// Bound and escape a string that came from, or through, a peer before it
/// is logged or retained in a [`HostState`].
///
/// Two separate jobs, both mandatory. The BOUND is about memory and log
/// volume (see [`PEER_TEXT_CAP`]). The ESCAPING is about where this text
/// ends up: an operator's terminal, via a log line, and a UI, via a state —
/// and a peer that embeds control bytes can otherwise repaint a console or
/// hide the very line describing what it did. `Debug`'s escaping is the
/// same defense the supervisor applies to tmux's exit reasons, chosen there
/// for the same reason and reused here rather than re-invented.
///
/// Truncation is marked rather than silent: "the host said this" and "the
/// host said this and kept going" are different diagnostics.
fn peer_text(text: &str) -> String {
    let truncated = text.len() > PEER_TEXT_CAP;
    // Split on a char boundary — `text` is a Rust `String`, so a byte
    // index in the middle of a multi-byte char would panic.
    let end = (0..=PEER_TEXT_CAP.min(text.len()))
        .rev()
        .find(|end| text.is_char_boundary(*end))
        .unwrap_or(0);
    let escaped = format!("{:?}", &text[..end]);
    if truncated {
        format!("{escaped} (truncated)")
    } else {
        escaped
    }
}

/// How many times one repeated failure is logged before this actor falls
/// silent about it.
///
/// A host that is down produces the same failure at every re-probe,
/// forever; a peer that answers every refresh with an error produces one
/// every few seconds. Neither is news after the first time, and both would
/// otherwise be an unbounded log the peer's own behavior is writing. The
/// count is not lost — the next DIFFERENT failure reports how many were
/// suppressed — so nothing becomes invisible, it merely stops repeating.
const REPEATED_FAILURE_LOG_LIMIT: u64 = 3;

/// One failure text and how many times it has repeated since it was last
/// reported — the state behind [`HostActor::note_failure`].
#[derive(Default)]
struct RepeatedFailure {
    text: String,
    seen: u64,
    suppressed: u64,
}

/// Longest single line of an ssh child's stderr this side will relay.
///
/// Not a formatting preference: the far side decides how long a "line" is,
/// and a remote that writes megabytes without a newline would otherwise be
/// choosing this process's memory usage. A truncated line is marked as
/// such, so an operator can tell "ssh said this" from "ssh said this and
/// more".
const SSH_STDERR_LINE_CAP: usize = 512;

/// How many stderr lines one ssh child may have relayed before the rest are
/// dropped.
///
/// A per-CHILD budget rather than a rate: an ssh that has said two hundred
/// things has said everything diagnostic it was going to say, and the
/// remainder is either a loop or an attack on the log. The actor makes a
/// new child per attempt, so a genuinely chatty host still gets a fresh
/// budget on every reconnect rather than going permanently quiet.
const SSH_STDERR_LINE_BUDGET: usize = 200;

/// Relay one ssh child's stderr into the tracing trail, bounded and
/// escaped, and attributed to the host it came from.
///
/// Every line is `Debug`-formatted, which is the same defense the
/// supervisor applies to tmux's exit reasons and for the same reason: this
/// is text a REMOTE party influences, arriving at a log an operator reads
/// in a terminal emulator. `Display` would replay control bytes verbatim —
/// cursor moves, screen clears, an OSC sequence retitling the window —
/// while `Debug` escapes them into something legible and inert.
///
/// Ends when the child's stderr closes, which is when the child exits;
/// there is no separate lifetime to manage, and the task cannot outlive
/// the transport that spawned it by more than the child's own teardown.
async fn relay_ssh_stderr(host: HostId, destination: String, stderr: tokio::process::ChildStderr) {
    use tokio::io::AsyncBufReadExt as _;

    // `split`, not `lines`: stderr from a remote is arbitrary bytes, and
    // the `lines` adapter would fail the whole stream on the first
    // non-UTF-8 one. Lossy conversion keeps a mostly-text diagnostic
    // readable instead of discarding it.
    let mut lines = tokio::io::BufReader::new(stderr).split(b'\n');
    let mut relayed = 0usize;
    while let Ok(Some(line)) = lines.next_segment().await {
        relayed += 1;
        // Past the budget the loop keeps READING and stops logging. It
        // must not stop reading: closing this pipe early makes the child's
        // next stderr write fail, which for ssh means the transport dies
        // because the remote was talkative — a far worse outcome than a
        // quiet log.
        match relayed.cmp(&SSH_STDERR_LINE_BUDGET) {
            std::cmp::Ordering::Greater => continue,
            std::cmp::Ordering::Equal => {
                warn!(
                    host,
                    destination = destination.as_str(),
                    budget = SSH_STDERR_LINE_BUDGET,
                    "the ssh child for this host has said enough; dropping the rest of its \
                     stderr for this connection"
                );
                continue;
            }
            std::cmp::Ordering::Less => {}
        }
        let truncated = line.len() > SSH_STDERR_LINE_CAP;
        let text = String::from_utf8_lossy(&line[..line.len().min(SSH_STDERR_LINE_CAP)]);
        warn!(
            host,
            destination = destination.as_str(),
            truncated,
            // Debug-formatted deliberately; see this function's own docs.
            message = ?text,
            "ssh reported a problem for this host"
        );
    }
}

/// Tag a failed local-socket dial that means "no supervisor here" with
/// [`LocalSupervisorNotRunning`], leaving every other failure untouched.
///
/// The two `io::ErrorKind`s below are the same pair
/// `farhelm_supervisor::service::connect` itself keys its remedy message
/// off — nothing is listening on the socket, or there is no socket file at
/// all. That duplication is deliberate and narrow: this side needs the
/// answer as a TYPE (a state-machine input), that side needs it as prose
/// (an operator's remedy), and re-deriving it here from the kinds is
/// cheaper and clearer than either parsing that prose or reshaping a
/// public API for one caller. Every other kind — permission denied, a
/// non-directory path component — keeps its original error, because
/// "start a supervisor" would be wrong advice for all of them.
fn classify_local_dial(error: anyhow::Error) -> anyhow::Error {
    let refused = error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(|io| {
            matches!(
                io.kind(),
                std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
            )
        });
    if refused {
        error.context(LocalSupervisorNotRunning)
    } else {
        error
    }
}

// ---- Session refresh -------------------------------------------------

/// Walk a supervisor's paginated session list to exhaustion, following
/// `next_cursor` until it is absent, and return every entry in order.
///
/// The helm's aggregation is deliberately two decoupled cursor layers
/// (PLAN_M6.md item 5): this walk drains a host into local storage, and
/// the REST edge paginates over that local data. Composing the two instead
/// — turning one browser page fetch into N live host round trips — would
/// break the moment any host flapped mid-walk. So this runs on a cadence,
/// off the request path, and its cost is paid once per host per refresh
/// rather than once per viewer.
///
/// **The page count is not free, and not just in round trips.** The
/// supervisor's conversation-capture sweep (`core::capture_pass`) rides
/// the `ListSessions` handler, so it runs once PER PAGE — a walk that takes
/// four pages performs four whole-host scans, not one. That is why every
/// caller passes `limit: None` and lets the supervisor apply its own
/// default cap (`LIST_SESSION_CAP`, 500): at that size an ordinary host's
/// entire list arrives in a single page, so the multiplication is
/// hypothetical for everyone except the pathological fleet that pagination
/// exists for in the first place. Shrinking the limit to make pages
/// "nicer" would multiply that scan for every host, every refresh.
///
/// **Everything about this walk's termination is the peer's to decide, so
/// none of it is trusted.** Three independent bounds, because the ways a
/// walk can fail to end are independent: [`REFRESH_PAGE_LIMIT`] on pages
/// followed, [`REFRESH_SESSION_CAP`] on entries accumulated, and a refusal
/// to follow a `next_cursor` identical to the one that produced it (the
/// cheapest non-termination there is to write by accident — a supervisor
/// that echoes back the cursor it was handed, which alone would spin
/// forever inside both other bounds while collecting duplicates). Any of
/// them tripping is an ordinary failed refresh: the caller keeps the
/// previous cache (see [`HostActor::refresh_once`]) rather than trusting a
/// walk that did not finish.
pub async fn drain_sessions(client: &SupervisorClient) -> anyhow::Result<Vec<SessionInfo>> {
    let mut collected: Vec<SessionInfo> = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..REFRESH_PAGE_LIMIT {
        // Kept so the reply's own `next_cursor` can be compared against
        // the cursor that produced it.
        let used = cursor.clone();
        let page = client
            .list_sessions_page(cursor.take(), None)
            .await
            .context("listing a page of the host's sessions")?;
        collected.extend(page.sessions);
        if collected.len() > REFRESH_SESSION_CAP {
            anyhow::bail!(
                "the host's session list exceeded {REFRESH_SESSION_CAP} entries; refusing to keep \
                 draining it"
            );
        }
        match page.next_cursor {
            None => {
                // `total` is the supervisor's own count before any page
                // cut, so a completed walk should have collected exactly
                // that many. A mismatch is not fatal — sessions really do
                // come and go between pages, and the cache is last-known
                // data either way — but it is the only signal that would
                // distinguish a benign race from a pagination bug quietly
                // dropping entries, so it is said out loud.
                if collected.len() as u64 != page.total {
                    warn!(
                        drained = collected.len(),
                        reported_total = page.total,
                        "the host's session walk finished with a different count than the host \
                         reported; caching what was drained"
                    );
                }
                return Ok(collected);
            }
            Some(next) if used.as_deref() == Some(next.as_str()) => {
                anyhow::bail!(
                    "the host's session list handed back the same cursor it was given \
                     ({next:?}); refusing to follow it in a circle"
                );
            }
            Some(next) => cursor = Some(next),
        }
    }
    anyhow::bail!(
        "the host's session list did not terminate within {REFRESH_PAGE_LIMIT} pages; refusing to \
         keep following its cursor"
    )
}

// ---- The manager -----------------------------------------------------

/// What one actor publishes about itself, and the only channel through
/// which anything outside the actor observes it.
///
/// State and client travel TOGETHER in one `watch` rather than in two
/// places, because the pair has an invariant: the client is `Some` exactly
/// while the state is `Connected`. Split across two cells, an observer
/// could catch the moment after a connection died but before the state
/// caught up and route an operation onto a corpse — precisely the failure
/// PLAN_M6.md item 5 refuses by naming the host's state in the error.
#[derive(Clone)]
struct ActorStatus {
    state: HostState,
    client: Option<Arc<SupervisorClient>>,
}

/// An out-of-band request from the manager to one actor: stop whatever you
/// are waiting on (or doing), and start again from the row-reload
/// boundary.
///
/// One mechanism for what used to be two half-mechanisms. A `retry_now`
/// that only shortened a sleep left the actor dialing the `HostRow` it had
/// captured when the phase began, so a whole retry ladder could burn
/// against a destination the user had already edited away; and a
/// connected actor's retry did nothing at all beyond skipping one refresh
/// tick. Returning to the boundary — reload the row, re-evaluate the
/// freezes, dial what the registry says NOW — is the only interpretation
/// that is correct for every state at once.
///
/// `revision` exists so a request is never lost between waits (a `watch`
/// receiver that was not waiting still sees the change on its next check),
/// and so two requests that arrive together collapse into one restart
/// rather than two.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Nudge {
    /// Monotonic; only its CHANGES are meaningful.
    revision: u64,
    /// Whether this request earns a fresh active-retry window on top of
    /// the restart.
    ///
    /// A plain retry against an unreachable host must stay a single probe
    /// — the user asking "try now" is not evidence the host is back, and
    /// unfolding a whole ladder per click is exactly the hammering the
    /// two-regime design avoids. A registry EDIT is different: it changes
    /// what is being dialed, which is the same kind of event as a freeze
    /// being resolved, and it gets the ladder for the same reason.
    fresh_window: bool,
}

/// The manager's handle on one running actor.
struct ActorHandle {
    /// Registry facts as of the last [`ConnectionManager::sync_registry`],
    /// so a snapshot needs no store round trip.
    row: HostRow,
    /// The SENDER, shared with the actor rather than merely subscribed to:
    /// a reconfiguring [`ConnectionManager::sync_registry`] must be able to
    /// publish the row's new, non-connected state in the same lock hold
    /// that swaps `row`, or a snapshot taken in between would pair a
    /// freshly edited destination with the old connection's `Connected`
    /// state — precisely the incoherence [`HostSnapshot`] promises callers
    /// they will never see.
    status: Arc<watch::Sender<ActorStatus>>,
    /// Interrupts whatever the actor is waiting on or doing — a backoff
    /// sleep, a re-probe wait, a freeze, or a live connection — so a user
    /// decision takes effect at once instead of at the next tick. See
    /// [`Nudge`].
    nudge: watch::Sender<Nudge>,
    task: tokio::task::JoinHandle<()>,
}

/// One connection actor per registry host, plus the entry points a user
/// decision arrives through.
///
/// Cheap to clone-by-`Arc` and safe to share: every method takes `&self`,
/// and the only mutable state is the actor map behind a short-held std
/// mutex (never across an await).
pub struct ConnectionManager {
    store: HelmStore,
    transport: Arc<dyn HostTransport>,
    cadence: Cadence,
    /// Serializes [`Self::sync_registry`] end to end — the registry READ
    /// included, which is why it cannot be the actor-map mutex (that one is
    /// std, and is deliberately never held across an await).
    ///
    /// Reconciling against a stale read is not a cosmetic race: two
    /// concurrent syncs can interleave so that the one holding the OLDER
    /// row set writes last, resurrecting an actor for a host that has been
    /// removed — an actor nothing will ever stop, dialing a host that is
    /// gone. Serializing the read with the reconcile makes that
    /// unconstructible rather than unlikely.
    reconcile: tokio::sync::Mutex<()>,
    actors: Mutex<ActorMap>,
}

/// The running actors, plus the one bit that can retire the whole set.
///
/// The flag lives INSIDE the guarded value rather than beside it as an
/// atomic, because its correctness is entirely about being read and written
/// under the same lock as the map: [`ConnectionManager::shutdown`] drains
/// and sets it in one hold, and [`ConnectionManager::sync_registry`] checks
/// it in the hold it does its insertions in. Split apart, the two can
/// interleave so that a reconcile which read the registry BEFORE the
/// shutdown repopulates the map after it — leaving actors nothing will ever
/// stop, running against a manager that is already gone.
#[derive(Default)]
struct ActorMap {
    /// Keyed by [`HostId`], which is never recycled (see that type's docs)
    /// — so a removed-then-re-added destination gets a genuinely new actor
    /// rather than inheriting the old one's state.
    actors: HashMap<HostId, ActorHandle>,
    /// Set once, by [`ConnectionManager::shutdown`]; never cleared. A
    /// manager is shut down for good — the alternative, a manager that can
    /// be revived, would need an answer for every actor that was mid-dial
    /// when it stopped, and nothing wants one.
    shut_down: bool,
}

/// One host's live state and its live connection, read TOGETHER.
///
/// The pair has an invariant — the client is `Some` exactly while the state
/// is [`HostState::Connected`] — and that invariant is only observable if
/// both are read from one borrow of the actor's published status. Two
/// separate reads can straddle a transition and hand a caller a fresh
/// `Connected` state beside the `None` client from the moment after the
/// connection died (or the reverse), which is exactly the pairing session
/// routing must never see: it would report a host as connected and then
/// refuse the operation, or route onto a corpse.
#[derive(Clone)]
pub struct HostStatus {
    pub state: HostState,
    /// `None` in every state except [`HostState::Connected`]. Not an error
    /// case to paper over: it is the "refuse with the host's state named"
    /// path PLAN_M6.md item 5 specifies, and `state` beside it is what says
    /// WHICH non-connected state it was.
    pub client: Option<Arc<SupervisorClient>>,
}

/// Aborts a task when dropped — the piece that makes one spawned task's
/// cancellation propagate to another it supervises.
///
/// A `JoinHandle` does NOT abort its task when dropped, so a supervisor
/// task that is itself aborted would otherwise leave the task it was
/// watching running with nobody watching it — the exact leak the
/// supervision exists to prevent, reintroduced one level up.
struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Whether an edit changed how this host is REACHED, as opposed to
/// anything else a row carries.
///
/// The three fields below are exactly the transport's inputs (see
/// [`HostTransport::connect`]), so a change to any of them means the live
/// connection — if there is one — is to the wrong place, or reaches the
/// right place a different way. Identity is deliberately not among them: a
/// row's learned identity changing is an adoption, which has its own path.
fn reconfigured(before: &HostRow, after: &HostRow) -> bool {
    before.destination != after.destination
        || before.remote_farhelm != after.remote_farhelm
        || before.remote_state_dir != after.remote_state_dir
}

impl ConnectionManager {
    /// Build a manager over `store` and start an actor for every host
    /// already registered.
    ///
    /// Returns as soon as the actors are SPAWNED, not once they have
    /// connected: a down host must not delay the helm's startup. Every host
    /// has an observable state from the moment this returns, and each was
    /// PUBLISHED as `Connecting` initially — but an actor runs concurrently
    /// with this call returning, so a fast host may already have moved on
    /// by the time a caller looks. Callers wanting a particular state must
    /// wait for it ([`Self::wait_for_state`]) rather than assume the
    /// initial one is still current. That is the same ordering `run()`
    /// already relies on for the listener — nothing user-visible waits on a
    /// host being up.
    pub async fn start(
        store: HelmStore,
        transport: Arc<dyn HostTransport>,
        cadence: Cadence,
    ) -> anyhow::Result<Arc<ConnectionManager>> {
        let manager = Arc::new(ConnectionManager {
            store,
            transport,
            cadence,
            reconcile: tokio::sync::Mutex::new(()),
            actors: Mutex::new(ActorMap::default()),
        });
        manager.sync_registry().await?;
        Ok(manager)
    }

    /// Reconcile the running actors against the registry: start one for
    /// every row that has none, stop the ones whose row is gone, and
    /// refresh the registry facts snapshots are rendered from.
    ///
    /// Called at startup and, in the next PR, after every host add, edit,
    /// and remove. Idempotent, so a caller that is unsure whether anything
    /// changed can simply call it.
    ///
    /// An EDITED row keeps its actor but NOT its connection: a change to
    /// any of the fields that decide how the host is reached
    /// ([`reconfigured`]) tears the current connection down, publishes a
    /// non-connected state alongside the new row, and hands the actor a
    /// fresh active-retry window. All three parts are load-bearing. Keeping
    /// the old connection — the behavior this replaced — means an edit
    /// silently does nothing until the connection happens to drop, so a
    /// user fixing a wrong destination sees their fix not take; publishing
    /// the state here rather than leaving it to the actor is what keeps
    /// [`HostSnapshot`]'s new-row/old-state pairing impossible; and the
    /// fresh window is the same rule freeze resolution already follows,
    /// generalized — an explicit user action deserves the ladder rather
    /// than a re-probe wait.
    ///
    /// Serialized against itself end to end (see [`Self::reconcile`]), so
    /// no two callers can reconcile from different reads of the registry.
    pub async fn sync_registry(&self) -> anyhow::Result<()> {
        let _reconcile = self.reconcile.lock().await;
        let rows = self.store.list_hosts().await?;
        let mut map = self.actors.lock().expect("actor map mutex poisoned");
        // Checked in the same hold the insertions below happen in, which is
        // what makes "a reconcile cannot outlive a shutdown" true by
        // construction rather than by timing: this call may well have read
        // the registry BEFORE the shutdown drained the map.
        if map.shut_down {
            info!("the connection manager is shut down; skipping registry reconciliation");
            return Ok(());
        }
        let live: std::collections::HashSet<HostId> = rows.iter().map(|row| row.id).collect();
        map.actors.retain(|id, handle| {
            if live.contains(id) {
                return true;
            }
            // The row is gone. Aborting is safe mid-anything: see the
            // module docs on why an actor has no state that can be left
            // half-applied. Its cache rows are already gone too, cascaded
            // by the store's own delete.
            info!(
                host = *id,
                "stopping the connection actor for a removed host"
            );
            handle.task.abort();
            false
        });
        for row in rows {
            match map.actors.entry(row.id) {
                std::collections::hash_map::Entry::Occupied(mut existing) => {
                    let handle = existing.get_mut();
                    if reconfigured(&handle.row, &row) {
                        // Carries `from`/`to` because this line IS the
                        // phase transition for an edited host: the actor's
                        // own transition log would see the phase already
                        // set to `connecting` by the publish below and say
                        // nothing, so without these fields the reconnection
                        // trail would show a host moving from connected to
                        // connected with no account of what happened in
                        // between.
                        info!(
                            host = row.id,
                            kind = ?row.kind,
                            destination = display_destination(&row),
                            from = handle.status.borrow().state.phase(),
                            to = "connecting",
                            "the host's connection settings changed; reconnecting to the edited \
                             configuration"
                        );
                        handle.row = row;
                        // The published client goes with the old row.
                        // Dropping the manager's copy is not what tears the
                        // connection down (the actor holds one too) — the
                        // nudge below is — but it must not remain routable
                        // for the interval in between, since a session
                        // operation sent onto it would reach the host the
                        // user just stopped pointing at.
                        handle.status.send_replace(ActorStatus {
                            state: HostState::Connecting {
                                attempt: 0,
                                last_error: None,
                            },
                            client: None,
                        });
                        handle.nudge.send_modify(|nudge| {
                            nudge.revision += 1;
                            nudge.fresh_window = true;
                        });
                    } else {
                        handle.row = row;
                    }
                }
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(self.spawn_actor(row));
                }
            }
        }
        Ok(())
    }

    /// Start one actor and hand back the manager's handle on it.
    fn spawn_actor(&self, row: HostRow) -> ActorHandle {
        let status = Arc::new(watch::Sender::new(ActorStatus {
            state: HostState::Connecting {
                attempt: 0,
                last_error: None,
            },
            client: None,
        }));
        let (nudge_tx, nudge_rx) = watch::channel(Nudge::default());
        let actor = HostActor {
            id: row.id,
            store: self.store.clone(),
            transport: Arc::clone(&self.transport),
            cadence: self.cadence.clone(),
            status: Arc::clone(&status),
            destination: Mutex::new(display_destination(&row)),
            last_failure: Mutex::new(RepeatedFailure::default()),
        };
        // One span per actor, entered for the task's whole life: SPEC.md
        // wants a reconnection trail whose every line names the host, and
        // a span is how that becomes a property of the code's structure
        // rather than a field every call site must remember to attach.
        //
        // The DESTINATION is deliberately not in the span even though it
        // reads like span-shaped context: a span's fields are fixed when it
        // is created, so a destination recorded here would keep naming the
        // host's original address for the actor's whole life — including in
        // the very lines describing the reconnection to its NEW one. The
        // actor attaches it per event instead (see
        // [`HostActor::destination`]).
        let span = info_span!("host", host = row.id, kind = ?row.kind);
        let host = row.id;
        let kind = row.kind;
        let destination = display_destination(&row);
        let actor_task = tokio::spawn(actor.run(row.clone(), nudge_rx).instrument(span));
        // The handle the manager holds is a SUPERVISOR of the actor, not
        // the actor itself. Nothing else watches a spawned task, so an
        // actor that ends — by panicking, or by retiring itself when its
        // row disappears — would otherwise leave its last published status
        // registered forever, complete with a client nobody is serving.
        //
        // Aborting the supervisor aborts the actor with it: the guard below
        // is dropped when this task's future is, which is exactly what
        // cancellation does.
        let supervised = Arc::clone(&status);
        let task = tokio::spawn(async move {
            let _abort_actor = AbortOnDrop(actor_task.abort_handle());
            let reason = match actor_task.await {
                Ok(()) => "the connection actor stopped because its registry row is gone",
                // A cancelled actor is this manager stopping it on purpose
                // (shutdown, or the row's removal), and the handle goes
                // with it — there is nobody left to publish to.
                Err(error) if error.is_cancelled() => return,
                Err(_) => "the connection actor panicked; this entry is not being attempted",
            };
            warn!(
                host,
                kind = ?kind,
                destination = destination.as_str(),
                reason,
                "a host connection actor finished; retiring the entry"
            );
            // The client goes with the state, in one publish: an entry with
            // no actor must not stay routable for even the moment it would
            // take to clear them separately.
            supervised.send_replace(ActorStatus {
                state: HostState::Retired {
                    reason: reason.to_string(),
                },
                client: None,
            });
        });
        ActorHandle {
            row,
            status,
            nudge: nudge_tx,
            task,
        }
    }

    /// Every host's registry facts plus its live connection state, ordered
    /// by [`HostId`] — the local row first, then ssh rows in registration
    /// order, matching [`HelmStore::list_hosts`].
    ///
    /// A point-in-time copy, never a live view: a caller rendering a hosts
    /// list wants one coherent picture, not a set of cells that can change
    /// underneath it mid-render.
    pub fn snapshots(&self) -> Vec<HostSnapshot> {
        let map = self.actors.lock().expect("actor map mutex poisoned");
        let mut out: Vec<HostSnapshot> = map
            .actors
            .values()
            .map(|handle| HostSnapshot {
                id: handle.row.id,
                kind: handle.row.kind,
                destination: handle.row.destination.clone(),
                state: handle.status.borrow().state.clone(),
            })
            .collect();
        out.sort_by_key(|snapshot| snapshot.id);
        out
    }

    /// One host's live state AND its live connection, from a single read.
    ///
    /// The accessor a session operation uses (PLAN_M6.md item 5): route to
    /// `client` if it is `Some`, otherwise refuse naming `state`. Doing
    /// that from two separate reads is the bug this exists to make
    /// unavailable — the two would be read either side of a transition, so
    /// a caller could see `Connected` beside `None` (refusing an operation
    /// against a host that is up) or a stale client beside a non-connected
    /// state (routing onto a connection that is already gone). See
    /// [`HostStatus`].
    ///
    /// `None` means no actor is running for that id.
    pub fn status(&self, host: HostId) -> Option<HostStatus> {
        let map = self.actors.lock().expect("actor map mutex poisoned");
        let published = map.actors.get(&host)?.status.borrow();
        Some(HostStatus {
            state: published.state.clone(),
            client: published.client.clone(),
        })
    }

    /// One host's live state, or `None` if no actor is running for that id.
    ///
    /// For callers that want the state ALONE — a hosts list, a log line, a
    /// test assertion. Anything that also needs the connection must use
    /// [`Self::status`] instead of pairing this with a second read.
    pub fn state(&self, host: HostId) -> Option<HostState> {
        self.status(host).map(|status| status.state)
    }

    /// Wait until `host`'s state satisfies `predicate`, and return it.
    ///
    /// Exists because the interesting properties of this module are
    /// TRANSITIONS, and polling for them is how a test becomes either slow
    /// or flaky. Public rather than test-only because the next PR's
    /// event-stream work needs exactly this shape, and because an
    /// `adopt`-then-observe caller should not have to invent it.
    ///
    /// `None` means no actor is running for `host`, or its actor died —
    /// never a timeout, which is the caller's to impose.
    pub async fn wait_for_state(
        &self,
        host: HostId,
        mut predicate: impl FnMut(&HostState) -> bool,
    ) -> Option<HostState> {
        let mut status = {
            let map = self.actors.lock().expect("actor map mutex poisoned");
            map.actors.get(&host)?.status.subscribe()
        };
        let matched = status
            .wait_for(|status| predicate(&status.state))
            .await
            .ok()?;
        Some(matched.state.clone())
    }

    /// Accept the identity a mismatched host is reporting, purging the old
    /// identity's cached sessions, and reconnect.
    ///
    /// The user decision half of SPEC.md's never-silently-merge rule: the
    /// helm refuses to merge two installs on its own, and this is the
    /// explicit acknowledgment that performs the merge the user chose.
    ///
    /// Refuses unless the host is currently in
    /// [`HostState::IdentityMismatch`], and passes that state's `recorded`
    /// value as the store's compare-and-swap expectation rather than
    /// re-reading it. Both matter: adopting a host that is not mismatched
    /// is a caller bug, and re-reading would open a window where a
    /// concurrent change is adopted instead of the one the user was
    /// actually shown.
    ///
    /// The row's dialed configuration is captured in the same lock hold as
    /// the state, and handed to the store: an adoption is only meaningful
    /// for the configuration the mismatch was observed under, and the store
    /// refuses it otherwise (see [`HelmStore::adopt_identity`]). So is a
    /// rival's claim on the identity being adopted, which is why this can
    /// fail even though the state it was called on was current.
    pub async fn adopt(&self, host: HostId) -> anyhow::Result<()> {
        let (state, row) = {
            let map = self.actors.lock().expect("actor map mutex poisoned");
            let handle = map
                .actors
                .get(&host)
                .with_context(|| format!("no connection actor is running for host {host}"))?;
            (handle.status.borrow().state.clone(), handle.row.clone())
        };
        let HostState::IdentityMismatch { recorded, reported } = state else {
            anyhow::bail!(
                "host {host} is {}, not awaiting an identity decision; nothing to adopt",
                state.phase()
            );
        };
        self.store
            .adopt_identity(host, &DialedAs::of(&row), &recorded, &reported)
            .await
            .with_context(|| format!("adopting host {host}'s new identity"))?;
        // Logged with the actor's own metadata — host, kind, destination —
        // because this decision happens on the MANAGER, outside the actor's
        // span, and an adoption line with no host context would be the one
        // hole in the identity trail SPEC_impl.md claims falls out of the
        // span discipline.
        info!(
            host,
            kind = ?row.kind,
            destination = display_destination(&row),
            superseded = recorded.as_str(),
            adopted = reported.as_str(),
            "adopted a new host identity; the superseded identity's cached sessions were purged"
        );
        self.retry_now(host);
        Ok(())
    }

    /// Reconnect `host` now: drop whatever connection it has, reload its
    /// row, and attempt it again from scratch.
    ///
    /// A real reconnect, not merely an early wake-up. The wake-only version
    /// of this was a bug in three directions at once — a connected host
    /// skipped one refresh tick and nothing else, a host mid-ladder went on
    /// dialing the row it had captured when the ladder began, and a caller
    /// reading the doc could reasonably expect either — so the contract is
    /// settled here as the strongest of the three: after this call the
    /// actor is back at its row-reload boundary, and everything it does
    /// next is against the registry as it stands now.
    ///
    /// What it does NOT do is grant a fresh active-retry window (see
    /// [`Nudge::fresh_window`]): an unreachable host asked to retry makes
    /// ONE attempt and returns to its re-probe cadence, because a user
    /// clicking retry is not evidence the host is back. A registry edit,
    /// which is such evidence, goes through [`Self::sync_registry`]
    /// instead. Resolving a freeze still earns the ladder — that decision
    /// lives in the actor, where the freeze does.
    ///
    /// A no-op for a host with no actor.
    pub fn retry_now(&self, host: HostId) {
        let map = self.actors.lock().expect("actor map mutex poisoned");
        if let Some(handle) = map.actors.get(&host) {
            handle.nudge.send_modify(|nudge| {
                nudge.revision += 1;
                // Explicitly cleared rather than left alone: the value is
                // retained between sends, so a previous reconfigure's flag
                // would otherwise still be riding along.
                nudge.fresh_window = false;
            });
        }
    }

    /// Stop every actor and drop every connection, permanently.
    ///
    /// There is no drain and no graceful phase, deliberately: SPEC.md's
    /// durability promise is that killing the helm does nothing to any
    /// session, so there is nothing an orderly shutdown would protect.
    /// Aborting is enough (see the module docs).
    ///
    /// TERMINAL, and that is what makes it safe against a concurrent
    /// [`Self::sync_registry`]. A reconcile that read the registry a moment
    /// before this call would otherwise repopulate the map right after it
    /// was drained, leaving actors nothing will ever stop; the flag set
    /// here is checked in the same lock hold that reconciliation does its
    /// insertions in, so the loser of that race becomes a no-op instead of
    /// a leak.
    ///
    /// A poisoned mutex is stepped over rather than panicked on, unlike
    /// every other lock hold in this module. This is the one method
    /// [`Drop`] calls, and a panic there during an unwind aborts the
    /// process — so "some other thread panicked while holding this lock"
    /// must not be able to turn a manager going out of scope into a crash.
    /// The recovered guard is safe to use for what this does: the map's
    /// entries are handles, not a half-updated invariant.
    pub fn shutdown(&self) {
        let mut map = match self.actors.lock() {
            Ok(map) => map,
            Err(poisoned) => poisoned.into_inner(),
        };
        map.shut_down = true;
        for (_, handle) in map.actors.drain() {
            handle.task.abort();
        }
    }
}

impl Drop for ConnectionManager {
    /// Actors are `tokio::spawn`ed, so they outlive this value unless
    /// something aborts them. Without this, dropping a manager — which
    /// every test does, and which the desktop app will do when it tears
    /// the embedded helm down — would leave a task per host running
    /// forever against a store nobody reads.
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ---- The actor -------------------------------------------------------

/// One host's connection actor: the loop that owns its transport
/// connection, its state machine, and its cache slice.
struct HostActor {
    id: HostId,
    store: HelmStore,
    transport: Arc<dyn HostTransport>,
    cadence: Cadence,
    status: Arc<watch::Sender<ActorStatus>>,
    /// The destination this actor is currently working against, in display
    /// form, refreshed every time the row is reloaded.
    ///
    /// Held here rather than read off the row at each call site because
    /// every diagnostic this actor writes has to name it — SPEC.md wants a
    /// reconnection trail, and a trail that cannot say WHERE the failing
    /// attempts were going is not one — while the row itself is owned by
    /// [`Self::run`]'s stack and the logging happens several frames down.
    destination: Mutex<String>,
    /// The last failure this actor logged, and how many identical ones it
    /// has swallowed since — see [`Self::note_failure`].
    last_failure: Mutex<RepeatedFailure>,
}

/// What [`HostActor::reload_row`] found, distinguishing "removed" from
/// "could not tell" — see that method for why two negatives are not one.
enum RowStatus {
    Present(HostRow),
    /// The registry no longer holds this id: the actor retires.
    Removed,
    /// The registry could not be read at all: the caller keeps the row it
    /// already had until the next successful reload.
    Unknown,
}

/// How a host is named in a log line: its destination, or `<local>` for
/// the reserved local row, which has none by construction.
fn display_destination(row: &HostRow) -> String {
    row.destination.as_deref().unwrap_or("<local>").to_string()
}

/// Wait for the manager's next [`Nudge`].
///
/// A free function rather than a method so it can be selected against
/// futures that borrow the actor — which is most of them.
///
/// A dropped sender parks forever instead of returning: it means this
/// actor's handle is gone, so an abort is already on its way, and the
/// alternative (returning immediately, forever) would spin the loop hot in
/// the window before that abort lands.
async fn next_nudge(nudge: &mut watch::Receiver<Nudge>) -> Nudge {
    match nudge.changed().await {
        Ok(()) => *nudge.borrow_and_update(),
        Err(_) => std::future::pending().await,
    }
}

/// Take a nudge that has ALREADY arrived, without waiting for one.
///
/// The seam that keeps a state publication from crossing a
/// reconfiguration: work that was in flight when the manager retargeted
/// the row must not publish its result, because the manager has already
/// published the new row's state and the actor's would overwrite it with
/// the old connection's.
fn taken_nudge(nudge: &mut watch::Receiver<Nudge>) -> Option<Nudge> {
    // An `Err` means the sender is gone — this actor's handle was dropped
    // and its abort is on the way — which is not a pending request, so it
    // reads the same as "nothing arrived".
    nudge
        .has_changed()
        .is_ok_and(|changed| changed)
        .then(|| *nudge.borrow_and_update())
}

/// How one connection attempt ended — the actor's whole decision surface,
/// named so [`HostActor::run`]'s loop reads as the state machine it is
/// rather than as nested error handling.
enum AttemptOutcome {
    /// Hello succeeded and the identity question was settled.
    Connected {
        client: Arc<SupervisorClient>,
        identity: Option<String>,
        build_version: String,
    },
    /// The peer answered and refused: protocol skew.
    Skew(VersionSkew),
    /// The peer answered with an identity that is not this row's.
    Mismatch { recorded: String, reported: String },
    /// The peer's identity belongs to another registry row.
    Duplicate { twin: HostId, identity: String },
    /// The transport never got as far as a settled hello.
    Failed {
        cause: UnreachableCause,
        error: String,
    },
    /// The manager interrupted this phase (a registry edit, or an explicit
    /// retry): nothing was settled and nothing should be published — the
    /// loop returns to its row-reload boundary instead.
    ///
    /// Produced only by [`HostActor::connect_phase`], never by a single
    /// attempt: an attempt either answers or fails.
    Interrupted { fresh_window: bool },
}

impl HostActor {
    /// Run this host's connection until the task is aborted, or until this
    /// entry's registry row disappears.
    ///
    /// Two exits, and the second is a backstop rather than the normal path:
    /// removal ordinarily goes through
    /// [`ConnectionManager::sync_registry`], which aborts the task
    /// outright, but an actor that outlived its own row retires itself
    /// instead of dialing a host the registry no longer knows. Its
    /// supervisor publishes [`HostState::Retired`] either way, so an entry
    /// whose actor is gone never keeps claiming the connection it had.
    ///
    /// The outer loop is the state machine's top level and every iteration
    /// answers the same question in the same order: may this entry connect
    /// at all, can it, and — once it has — keep its cache fresh until the
    /// connection dies. Structuring it as a loop rather than as mutually
    /// recursive per-state functions is what keeps every path back to
    /// "start over" identical, including the ones a future state would add.
    ///
    /// `row` is re-read from the store at the top of each pass, BEFORE
    /// anything else including the duplicate freeze, and never cached
    /// across one: a destination edit must take effect on the next
    /// attempt — for a frozen entry as much as for a healthy one, which is
    /// how editing a duplicate's destination can resolve it while its twin
    /// is still there — and a row that has DISAPPEARED ends the actor. That
    /// last case is a backstop rather than the normal path — removal goes
    /// through [`ConnectionManager::sync_registry`], which aborts the task
    /// outright — but it means a removal this actor somehow outlived
    /// cannot leave it dialing a host the registry no longer knows.
    ///
    /// `active` tracks which of the two regimes SPEC.md's Errors section
    /// describes this pass belongs to, and getting it wrong is the bug
    /// this comment exists to prevent: without it, every re-probe would
    /// unfold a whole fresh sixty-second retry ladder, so a host that has
    /// been down for an hour would be dialed roughly every seven seconds
    /// forever rather than every forty-five — the exact "hammering a dead
    /// host" behavior the two-regime design exists to avoid. A fresh
    /// active window is granted only where something actually CHANGED:
    /// startup, a connection that was up and was lost, the resolution of a
    /// freeze, and a registry edit (which is the same kind of event, and is
    /// the one that arrives as a [`Nudge`] asking for it). A re-probe that
    /// fails simply re-probes again, and so does a plain user retry.
    ///
    /// `nudge` is the manager's interrupt, and every wait in every phase
    /// races it — including the dial itself. Whatever it interrupts, the
    /// answer is the same: come back HERE, to the top of the loop, and
    /// decide again from the registry as it now stands. That is what makes
    /// "the edit took effect" true for an actor mid-ladder, mid-dial,
    /// mid-refresh, or frozen, without each of those states needing its own
    /// notion of what an edit means.
    async fn run(self, mut row: HostRow, mut nudge: watch::Receiver<Nudge>) {
        let mut active = true;
        loop {
            match self.reload_row().await {
                RowStatus::Present(fresh) => row = fresh,
                RowStatus::Removed => {
                    info!("registry row is gone; the connection actor is stopping");
                    return;
                }
                RowStatus::Unknown => {}
            }
            *self.destination.lock().expect("destination mutex poisoned") =
                display_destination(&row);

            // A duplicate freeze is re-evaluated BEFORE anything is
            // dialed, so an entry that is still a twin of another never
            // opens a connection at all (PLAN_M6.md item 4: a duplicate
            // entry connects nothing while it stays one). The check is a
            // registry read, not a network round trip — and it happens
            // AFTER the row reload above, so an entry whose destination was
            // edited dials the new one on this pass rather than staying
            // frozen against an answer the old one gave.
            //
            // The borrow is scoped to its own binding on purpose: a
            // `watch::Ref` is not `Send`, so holding one across the await
            // below would make this whole task unspawnable.
            let frozen_as_duplicate = match &self.status.borrow().state {
                HostState::Duplicate { identity, .. } => Some(identity.clone()),
                _ => None,
            };
            if let Some(identity) = frozen_as_duplicate {
                match self.twin_holding(&identity).await {
                    Ok(Some(twin)) => {
                        self.set_state(HostState::Duplicate { twin, identity });
                        active = self.hold(&mut nudge, self.cadence.reprobe).await;
                        continue;
                    }
                    Ok(None) => {
                        // The collision is gone — the twin was removed, or
                        // adopted a different identity. That is a state
                        // change, so this entry earns a fresh active window
                        // rather than being made to wait out re-probes for
                        // a host that is probably fine.
                        info!(
                            destination = %self.destination(),
                            "no longer a duplicate of another entry; resuming connection attempts"
                        );
                        active = true;
                    }
                    Err(error) => {
                        // A registry that cannot be read is a FAILED check,
                        // never a "no twin found": answering the latter
                        // would connect an entry on the strength of a
                        // database hiccup, which is precisely the outcome
                        // the duplicate freeze exists to prevent. The
                        // freeze is retained and re-checked next pass.
                        warn!(
                            error = %error,
                            destination = %self.destination(),
                            "could not re-check whether this entry is still a duplicate; \
                             keeping the freeze"
                        );
                        active = self.hold(&mut nudge, self.cadence.reprobe).await;
                        continue;
                    }
                }
            }

            // An empty ladder is one attempt and no retries — exactly what
            // a background re-probe is. See this function's own docs on
            // `active`.
            let ladder: &[Duration] = if active {
                &self.cadence.connect_backoff
            } else {
                &[]
            };
            match self.connect_phase(&row, ladder, active, &mut nudge).await {
                AttemptOutcome::Connected {
                    client,
                    identity,
                    build_version,
                } => {
                    self.serve(client, identity, build_version, &mut nudge)
                        .await;
                    // A connection that WAS up and was lost earns a fresh
                    // active window: a supervisor restarted by hand is
                    // back within a second or two, and that is precisely
                    // the case the early ladder steps exist for. A
                    // connection dropped because the row was retargeted
                    // earns it for the same reason — the new destination
                    // deserves the same first-attempt urgency a returning
                    // host gets.
                    active = true;
                }
                AttemptOutcome::Skew(skew) => {
                    // The outcome is owned here, so its strings move into
                    // the state rather than being copied into it.
                    self.set_state(HostState::VersionSkew {
                        peer_protocol: skew.peer_protocol,
                        peer_build: skew.peer_build,
                        our_protocol: skew.our_protocol,
                        our_build: skew.our_build,
                        remediation: skew_remediation(&row),
                    });
                    active = self.hold(&mut nudge, self.cadence.reprobe).await;
                }
                AttemptOutcome::Mismatch { recorded, reported } => {
                    self.set_state(HostState::IdentityMismatch { recorded, reported });
                    // Frozen with NO timer: only a user decision
                    // (adopt, or a fixed destination) can resolve this,
                    // and re-probing would churn the trail with a
                    // question nobody answered. See
                    // `HostState::IdentityMismatch`.
                    next_nudge(&mut nudge).await;
                    // Whatever nudged this is a user decision, so the retry
                    // it asked for happens immediately and with a full
                    // active window behind it — regardless of whether the
                    // nudge itself asked for one.
                    active = true;
                }
                AttemptOutcome::Duplicate { twin, identity } => {
                    self.set_state(HostState::Duplicate { twin, identity });
                    active = self.hold(&mut nudge, self.cadence.reprobe).await;
                }
                AttemptOutcome::Failed { cause, error } => {
                    self.set_state(HostState::Unreachable {
                        cause,
                        last_error: error,
                    });
                    active = self.hold(&mut nudge, self.cadence.reprobe).await;
                }
                AttemptOutcome::Interrupted { fresh_window } => {
                    // Nothing was settled and nothing was published: start
                    // the pass again against whatever the registry says
                    // now. The regime is preserved unless the nudge asked
                    // for the ladder, so a user's retry against an
                    // unreachable host stays a single probe.
                    active = active || fresh_window;
                }
            }
        }
    }

    /// One connection window: an immediate attempt, then one per `ladder`
    /// step, stopping at the first attempt that settles the question
    /// either way.
    ///
    /// "Settles" is broader than "succeeds". A transport failure is worth
    /// retrying — the host may be booting — but a REFUSAL is not: skew,
    /// an identity mismatch, and a duplicate are all answers, and retrying
    /// them six more times over a minute would only produce six identical
    /// answers while delaying the state the user needs to see. So those
    /// three return immediately and the ladder is spent only on failures.
    ///
    /// `ladder` rather than [`Cadence::connect_backoff`] directly, because
    /// this function serves both regimes: the active window passes the
    /// real ladder, a background re-probe passes an empty one and thereby
    /// makes exactly one attempt. See [`Self::run`]'s docs.
    ///
    /// `active` also governs whether [`HostState::Connecting`] is
    /// PUBLISHED. A background re-probe deliberately leaves the host's
    /// existing state alone while it dials: an entry that has been
    /// unreachable overnight must read as unreachable, not flicker into
    /// "connecting" and back every forty-five seconds — which would both
    /// mislead the UI and drown the reconnection trail (SPEC.md's actual
    /// requirement) in two phase-transition lines per probe per host. The
    /// probe's outcome is what changes the state, if anything does.
    /// Every wait AND every attempt in this phase is racing the manager's
    /// nudge, which is what stops an edit from being ignored for a whole
    /// window: without it a ladder started against the old destination
    /// would keep dialing it for up to a minute after the user retargeted
    /// the row, and a hung dial would ignore the edit for as long as
    /// [`Cadence::attempt_timeout`] allows. Losing that race cancels the
    /// attempt in flight — which, for the ssh transport, kills the child
    /// (see [`SshChannel`]) rather than leaving it behind.
    async fn connect_phase(
        &self,
        row: &HostRow,
        ladder: &[Duration],
        active: bool,
        nudge: &mut watch::Receiver<Nudge>,
    ) -> AttemptOutcome {
        let mut last: Option<AttemptOutcome> = None;
        for attempt in 0..=ladder.len() as u32 {
            if attempt > 0 {
                let wait = ladder[attempt as usize - 1];
                if active {
                    self.set_state(HostState::Connecting {
                        attempt,
                        last_error: match &last {
                            Some(AttemptOutcome::Failed { error, .. }) => Some(error.clone()),
                            _ => None,
                        },
                    });
                }
                if let Some(nudge) = self.wait_or_nudge(nudge, wait).await {
                    return AttemptOutcome::Interrupted {
                        fresh_window: nudge.fresh_window,
                    };
                }
            } else if active {
                self.set_state(HostState::Connecting {
                    attempt: 0,
                    last_error: None,
                });
            }
            debug!(
                attempt,
                destination = %self.destination(),
                "opening a connection to the host supervisor"
            );
            let outcome = tokio::select! {
                nudge = next_nudge(nudge) => AttemptOutcome::Interrupted {
                    fresh_window: nudge.fresh_window,
                },
                outcome = self.deadlined_attempt(row) => outcome,
            };
            match &outcome {
                AttemptOutcome::Failed { error, .. } => {
                    // A host that is down says the same thing at every
                    // re-probe, forever; the trail wants the failure, not a
                    // line per probe for the rest of the process's life.
                    if let Some(suppressed) = self.note_failure(error) {
                        warn!(
                            attempt,
                            destination = %self.destination(),
                            error = error.as_str(),
                            suppressed,
                            "host connection attempt failed"
                        );
                    }
                    last = Some(outcome);
                }
                _ => return outcome,
            }
        }
        last.expect("the loop always runs at least the immediate attempt")
    }

    /// One attempt, bounded by [`Cadence::attempt_timeout`].
    ///
    /// Expiry is an ORDINARY failed attempt, not a state of its own: the
    /// ladder steps, the re-probe cadence, and the unreachable state all
    /// behave exactly as they do for a refused connection, which is the
    /// whole point — a peer that accepts a connection and then says nothing
    /// must cost this actor no more than one that refuses outright. The
    /// message says which of the two happened, since "connection refused"
    /// and "connected, never spoke" send an operator to different places.
    async fn deadlined_attempt(&self, row: &HostRow) -> AttemptOutcome {
        match tokio::time::timeout(self.cadence.attempt_timeout, self.attempt(row)).await {
            Ok(outcome) => outcome,
            Err(_) => AttemptOutcome::Failed {
                // Never the local-supervisor classification: a timeout
                // means something ANSWERED the dial and then stalled, which
                // is not the "nothing is listening" case that hint exists
                // for.
                cause: UnreachableCause::TransportFailure,
                error: format!(
                    "the connection attempt did not complete within {:?} (the transport opened \
                     but the supervisor's hello never arrived, or the dial itself never returned)",
                    self.cadence.attempt_timeout
                ),
            },
        }
    }

    /// One connection attempt, from opening the transport through settling
    /// the identity question — everything that has to succeed before a
    /// host counts as connected.
    ///
    /// Identity is settled HERE, on the connect path, and not on a later
    /// refresh, because the hello is the only place an identity crosses
    /// the wire: a host whose identity changed is necessarily a different
    /// connection, so there is no window in which a live connection's
    /// identity could drift out from under this decision.
    ///
    /// The row's configuration is captured BEFORE the dial and carried into
    /// the identity write, so a hello that crosses the wire while the user
    /// is retargeting the row cannot commit the old endpoint's identity
    /// under the new one (see [`DialedAs`]).
    async fn attempt(&self, row: &HostRow) -> AttemptOutcome {
        let dialed = DialedAs::of(row);
        let (reader, writer) = match self.transport.connect(row).await {
            Ok(pair) => pair,
            Err(error) => return failure(row, error),
        };
        let client = match SupervisorClient::start(reader, writer).await {
            Ok(client) => client,
            Err(error) => {
                // The skew refusal reaches this side as an io::Error
                // payload on the handshake failure; recovering it as a
                // TYPE rather than by matching prose is what makes both
                // versions available to the state below. See
                // `farhelm_proto::io::VersionSkew`.
                if let Some(skew) = error
                    .chain()
                    .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
                    .find_map(VersionSkew::cause_of)
                {
                    // The payload's own `Display` rides along, not just the
                    // fields recovered from it: that text is the refusal
                    // exactly as the handshake words it — the same wording
                    // sent to the peer and shown by the single-host path —
                    // and `farhelm_proto::io::VersionSkew`'s docs promise
                    // an operator reads it on the helm's stderr. Recovering
                    // the numbers as a TYPE is what the state machine needs;
                    // it is not a reason to withhold the sentence a human
                    // needs.
                    warn!(
                        peer_protocol = skew.peer_protocol,
                        peer_build = skew.peer_build.as_str(),
                        our_protocol = skew.our_protocol,
                        destination = %self.destination(),
                        skew = %skew,
                        "the host's supervisor refused the hello: protocol version skew"
                    );
                    return AttemptOutcome::Skew(skew.clone());
                }
                // An ssh channel that closed before a single byte of
                // protocol is the "no supervisor over there" shape, and
                // the M1 annotation is what turns it into advice; reused
                // rather than re-derived so both paths say the same thing.
                let error = match (&row.kind, row.destination.as_deref()) {
                    (HostKind::Ssh, Some(dest)) => crate::annotate_ssh_handshake_eof(
                        error,
                        dest,
                        row.remote_state_dir.as_deref(),
                    ),
                    _ => error,
                };
                return failure(row, error);
            }
        };

        // The two fields, not the whole hello: the borrow has to end before
        // `client` can be moved into an outcome below, and cloning one
        // struct to then clone a field out of it again was a copy nobody
        // wanted.
        let (reported_identity, build_version) = {
            let hello = client.peer_hello();
            (hello.host_identity.clone(), hello.build_version.clone())
        };
        let Some(identity) = reported_identity else {
            // A supervisor with no identity to report. NOT an old
            // supervisor: one predating PLAN_M6.md item 2 speaks an older
            // protocol version and is refused at the hello gate long
            // before this, so what actually lands here is a construction
            // with no standing to mint one (a test peer, an embedded
            // supervisor built without a state directory to claim from).
            // Connected and usable — sessions still list, terminals still
            // attach — but NOT cacheable, because the cache's identity
            // binding is what protects a stale refresh from landing on the
            // wrong install and there is nothing here to bind to. Warned
            // rather than silently degraded: a host that never caches would
            // otherwise show an empty stale list with no explanation.
            warn!(
                build_version = build_version.as_str(),
                destination = %self.destination(),
                "the host's supervisor reported no identity; connecting without a session cache"
            );
            return AttemptOutcome::Connected {
                client,
                identity: None,
                build_version,
            };
        };

        // Duplicate detection is the STORE's answer, resolved in the same
        // transaction as the write it would have made. This used to be a
        // separate "does another row hold this identity" query here,
        // followed by a record — a check-then-write whose TOCTOU cost the
        // user the whole host: two actors reaching one new supervisor both
        // saw no twin, both recorded, and at the next helm start each saw
        // the other as its twin and BOTH froze, so a live host appeared
        // nowhere at all.
        //
        // Every non-`Recorded` answer below DROPS `client`, which tears the
        // connection down: that is what "a duplicate entry connects
        // nothing" means in practice, given that an identity can only be
        // learned by saying hello once. The same applies to the mismatch
        // arm.
        match self
            .store
            .record_first_contact(self.id, &dialed, &identity)
            .await
        {
            Ok(FirstContactOutcome::Recorded) => {
                info!(
                    identity = identity.as_str(),
                    build_version = build_version.as_str(),
                    destination = %self.destination(),
                    "host identity confirmed at hello"
                );
                AttemptOutcome::Connected {
                    client,
                    identity: Some(identity),
                    build_version,
                }
            }
            Ok(FirstContactOutcome::Mismatch { recorded, reported }) => {
                // Nothing was written — the store's API guarantees that,
                // not this call site's discipline — so freezing here
                // leaves the registry exactly as the user last left it.
                warn!(
                    recorded = recorded.as_str(),
                    reported = reported.as_str(),
                    destination = %self.destination(),
                    "the destination reports a different host identity; freezing until the user \
                     adopts it or fixes the destination"
                );
                AttemptOutcome::Mismatch { recorded, reported }
            }
            Ok(FirstContactOutcome::Collision { owner }) => {
                warn!(
                    twin = owner,
                    identity = identity.as_str(),
                    destination = %self.destination(),
                    "this entry reaches a host another entry already owns; connecting nothing"
                );
                AttemptOutcome::Duplicate {
                    twin: owner,
                    identity,
                }
            }
            Ok(FirstContactOutcome::StaleAttempt { current }) => {
                // The row was retargeted while this handshake was in
                // flight, so the identity in hand describes the endpoint
                // the row USED to name. Treated as a failed attempt: there
                // is nothing wrong with the host, and the very next pass
                // dials what the registry says now.
                warn!(
                    identity = identity.as_str(),
                    dialed = ?dialed.destination,
                    current = ?current.destination,
                    "the host was reconfigured while this handshake was in flight; discarding \
                     what the previous configuration reported"
                );
                AttemptOutcome::Failed {
                    cause: UnreachableCause::TransportFailure,
                    error: "the host was reconfigured while this connection attempt was in flight"
                        .to_string(),
                }
            }
            Err(error) => {
                // A storage failure is not a connection failure, but the
                // honest response is the same: this host has no settled
                // identity, so it must not serve or cache under one.
                failure(row, error.context("recording the host's identity"))
            }
        }
    }

    /// The connected phase: refresh this host's cache on
    /// [`Cadence::refresh`] until the connection dies, then return so the
    /// caller can start a fresh active-retry window.
    ///
    /// The refresh tick and the connection's death are raced rather than
    /// polled, so an idle host's outage is noticed the moment it happens
    /// instead of at the next tick. That matters for the reconnect clock:
    /// the active-retry window is supposed to start at the LOSS, and a
    /// host that goes down one second after a refresh would otherwise
    /// spend most of a cadence looking healthy.
    /// Four things end this phase, and each is a real event rather than a
    /// timeout on a healthy connection: the peer closing, a refresh that
    /// expired or was refused as identity-stale (see [`RefreshStep`]), and
    /// a nudge from the manager (a registry edit, or an explicit retry).
    /// The nudge case cancels an in-flight refresh and publishes nothing
    /// further about this connection, because by then the manager has
    /// already published the new row's state and anything published here
    /// would describe a connection to the OLD one.
    async fn serve(
        &self,
        client: Arc<SupervisorClient>,
        identity: Option<String>,
        build_version: String,
        nudge: &mut watch::Receiver<Nudge>,
    ) {
        let connected_at = tokio::time::Instant::now();
        self.publish(
            HostState::Connected {
                identity: identity.clone(),
                build_version: build_version.clone(),
                last_refresh: RefreshHealth::Pending,
            },
            Some(Arc::clone(&client)),
        );
        let mut ended = "the peer closed the connection";
        loop {
            let step = tokio::select! {
                _ = next_nudge(nudge) => {
                    ended = "the host was reconfigured or an immediate retry was requested";
                    break;
                }
                step = self.refresh_once(&client, identity.as_deref()) => step,
            };
            // A nudge that landed while the refresh was finishing means the
            // same thing as one that arrived a moment earlier; publishing
            // this connection's health now would overwrite the state the
            // manager just published for the edited row.
            if taken_nudge(nudge).is_some() {
                ended = "the host was reconfigured or an immediate retry was requested";
                break;
            }
            let end_connection = step.end_connection;
            self.publish(
                HostState::Connected {
                    identity: identity.clone(),
                    build_version: build_version.clone(),
                    last_refresh: step.health,
                },
                Some(Arc::clone(&client)),
            );
            if let Some(reason) = end_connection {
                ended = reason;
                break;
            }
            tokio::select! {
                _ = client.closed() => break,
                _ = tokio::time::sleep(self.cadence.refresh) => {}
                nudge = next_nudge(nudge) => {
                    let _ = nudge;
                    ended = "the host was reconfigured or an immediate retry was requested";
                    break;
                }
            }
        }
        info!(
            connected_for_secs = connected_at.elapsed().as_secs(),
            destination = %self.destination(),
            reason = ended,
            "dropped the connection to the host supervisor; re-entering active retries"
        );
        // Dropping the last handle is what actually tears the transport
        // down (see `SupervisorClient`'s task-weakness discipline), so the
        // client must leave the published status before the next attempt
        // opens a second connection to the same host.
        self.publish(
            HostState::Connecting {
                attempt: 0,
                last_error: None,
            },
            None,
        );
    }

    /// Drain this host's session list and replace its cache slice, exactly
    /// once.
    ///
    /// **A failed refresh keeps the previous cache.** Nothing is cleared,
    /// and the store is not even asked to write. The cache's whole job is
    /// to answer "what did this host have, last we knew" while the host is
    /// unavailable, so wiping it on a failed refresh would destroy the
    /// answer at precisely the moment it becomes the only one available —
    /// and would make a transient failure (the pagination `Internal` error
    /// a single over-large record produces, a connection dying mid-walk)
    /// indistinguishable from "this host genuinely has no sessions". The
    /// failure is recorded in [`RefreshHealth::Failed`] instead, where it
    /// is visible without being destructive.
    ///
    /// A host with no identity writes NO cache at all: see
    /// [`HostState::Connected`]'s `identity` field.
    ///
    /// An identity that has moved on since the connection came up — the
    /// user adopted a new one while this walk was in flight — comes back
    /// from the store as a refused write, which is exactly the delayed
    /// stale refresh the binding exists to catch. That one ENDS the
    /// connection rather than merely recording a failure: this actor is
    /// talking to an install the registry no longer associates with this
    /// row, so every later refresh would be refused identically and the
    /// host would sit there looking connected while caching nothing. Going
    /// back through the connect path is what re-asks the identity question
    /// against the row as it now stands.
    ///
    /// The walk is bounded by [`Cadence::refresh_timeout`], and expiry ends
    /// the connection too — see that field for why a peer that answers
    /// nothing must not be allowed to park this loop. The previous cache
    /// survives both, like any other failed refresh.
    async fn refresh_once(&self, client: &SupervisorClient, identity: Option<&str>) -> RefreshStep {
        let drained = tokio::time::timeout(self.cadence.refresh_timeout, drain_sessions(client));
        let entries = match drained.await {
            Err(_) => {
                warn!(
                    timeout_secs = self.cadence.refresh_timeout.as_secs(),
                    destination = %self.destination(),
                    "the host stopped answering its session list; dropping the connection and \
                     keeping the previous cache"
                );
                // Cancelling the walk drops the request future, and the
                // connection is dropped right behind it, so no pending
                // request outlives this: the client's whole state goes when
                // the actor stops holding it.
                return RefreshStep {
                    // This side's own words, so nothing to sanitize — but
                    // the same shape as every other retained failure.
                    health: RefreshHealth::Failed {
                        error: format!(
                            "the host did not answer its session list within {:?}",
                            self.cadence.refresh_timeout
                        ),
                    },
                    end_connection: Some("the host stopped answering its session list"),
                };
            }
            Ok(Ok(entries)) => entries,
            Ok(Err(error)) => {
                let internal = error
                    .downcast_ref::<SupervisorError>()
                    .is_some_and(|e| e.kind == ErrorKind::Internal);
                // The message is the PEER's, in full: a supervisor's
                // `Error.message` is whatever it chose to send, and this
                // one is both logged and retained in the connected state
                // until the next refresh replaces it. Bounded and escaped
                // once, here, so neither copy carries megabytes or control
                // bytes (see `peer_text`), and rate-limited so a host
                // erroring on every tick cannot write the log itself.
                let error = peer_text(&format!("{error:#}"));
                if let Some(suppressed) = self.note_failure(&error) {
                    warn!(
                        error = error.as_str(),
                        supervisor_internal = internal,
                        destination = %self.destination(),
                        suppressed,
                        "refreshing the host's session list failed; keeping the previous cache"
                    );
                }
                return RefreshStep {
                    health: RefreshHealth::Failed { error },
                    end_connection: None,
                };
            }
        };
        let Some(identity) = identity else {
            // Live-only: the walk succeeded, so the count is real, but
            // there is nothing to bind a cache write to. See this method's
            // own docs.
            return RefreshStep {
                health: RefreshHealth::Ok {
                    sessions: entries.len(),
                },
                end_connection: None,
            };
        };
        let sessions = entries.len();
        match self
            .store
            .replace_host_sessions(self.id, identity, entries)
            .await
        {
            Ok(()) => {
                debug!(sessions, "replaced the host's cached session list");
                RefreshStep {
                    health: RefreshHealth::Ok { sessions },
                    end_connection: None,
                }
            }
            Err(error) => {
                let superseded = error
                    .downcast_ref::<HostStoreError>()
                    .is_some_and(|e| matches!(e, HostStoreError::IdentityMismatch { .. }));
                // This one is the STORE's own text rather than a peer's,
                // but it is normalized identically: one shape of retained
                // failure string is easier to reason about than two, and a
                // store error can quote an identity the peer supplied.
                let error = peer_text(&format!("{error:#}"));
                if let Some(suppressed) = self.note_failure(&error) {
                    warn!(
                        error = error.as_str(),
                        identity_superseded = superseded,
                        destination = %self.destination(),
                        suppressed,
                        "caching the host's session list failed; keeping the previous cache"
                    );
                }
                RefreshStep {
                    health: RefreshHealth::Failed { error },
                    end_connection: superseded
                        .then_some("this connection's identity is no longer the row's"),
                }
            }
        }
    }

    /// The registry row holding `identity`, if any row OTHER than this one
    /// does.
    ///
    /// A store failure PROPAGATES rather than answering "no twin known".
    /// Answering the latter — the shape this had — turns a database hiccup
    /// into a decision to connect an entry the registry might well say is a
    /// duplicate, which is the exact outcome the freeze exists to prevent;
    /// a failed check is a failed check, and the caller retains whatever
    /// freeze it already had and asks again next pass.
    ///
    /// Only the DUPLICATE state's re-evaluation calls this now. The
    /// once-per-attempt "does another row hold this identity" pre-check is
    /// gone: the store resolves that inside the transaction that would have
    /// written (see [`HelmStore::record_first_contact`]), and a row's own
    /// stored identity can no longer belong to another row at all, because
    /// the schema refuses to hold two such rows.
    async fn twin_holding(&self, identity: &str) -> anyhow::Result<Option<HostId>> {
        let rows = self
            .store
            .list_hosts()
            .await
            .context("reading the registry to re-check a duplicate host")?;
        Ok(rows
            .into_iter()
            .find(|row| row.id != self.id && row.host_identity.as_deref() == Some(identity))
            .map(|row| row.id))
    }

    /// This actor's registry row as it stands right now.
    ///
    /// Three answers, not two, and the third is the point: "the row is
    /// gone" stops the actor, so a transient database read failure must
    /// never be able to produce it. [`RowStatus::Unknown`] keeps the
    /// caller on the row it already had — at worst one edit stale, and
    /// self-correcting on the next pass — rather than retiring a host
    /// because SQLite hiccupped.
    async fn reload_row(&self) -> RowStatus {
        match self.store.list_hosts().await {
            Ok(rows) => match rows.into_iter().find(|row| row.id == self.id) {
                Some(row) => RowStatus::Present(row),
                None => RowStatus::Removed,
            },
            Err(error) => {
                warn!(error = %error, "could not re-read the registry row; keeping the last one");
                RowStatus::Unknown
            }
        }
    }

    /// Publish a state with no client attached — the shape every
    /// non-connected state has.
    ///
    /// Split from [`Self::publish`] purely so the overwhelmingly common
    /// call reads as one argument rather than a `None` at every site.
    fn set_state(&self, state: HostState) {
        self.publish(state, None);
    }

    /// Publish this actor's state and its live client together, logging
    /// the transition when the PHASE changed.
    ///
    /// The phase check is what keeps the trail readable: a connected
    /// host republishes its status on every refresh tick, and logging each
    /// of those as a transition would bury the handful of lines that
    /// describe what actually happened to the host. Per-event detail
    /// (which attempt, which identity, why a refresh failed) is logged at
    /// the point of decision instead, where the evidence is in hand.
    fn publish(&self, state: HostState, client: Option<Arc<SupervisorClient>>) {
        let previous = self.status.borrow().state.phase();
        if previous != state.phase() {
            info!(
                from = previous,
                to = state.phase(),
                destination = %self.destination(),
                "host connection phase changed"
            );
        }
        // `send_replace`, never `send`: a `watch` send is a NO-OP when the
        // channel has no receivers, and this channel legitimately has none
        // most of the time — the manager holds the sender and only
        // subscribes while something is actively waiting. With `send` the
        // published status would silently stop advancing whenever nobody
        // happened to be watching, which is worse than it sounds: the
        // retained value keeps a strong `Arc<SupervisorClient>` alive, so a
        // connection the actor believed it had dropped would stay open and
        // its peer would never see the close. The old status is returned
        // and dropped here, which is what actually releases it.
        let _previous = self.status.send_replace(ActorStatus { state, client });
    }

    /// Sleep for `wait`, or return the [`Nudge`] that cut it short.
    ///
    /// A `watch` rather than a `Notify`, so a nudge that arrives in the gap
    /// between two waits is STORED rather than lost — a user decision must
    /// not be dropped because it landed a microsecond before the actor got
    /// around to waiting — and so the request's own terms (see
    /// [`Nudge::fresh_window`]) survive with it, which a bare notification
    /// could not carry.
    async fn wait_or_nudge(
        &self,
        nudge: &mut watch::Receiver<Nudge>,
        wait: Duration,
    ) -> Option<Nudge> {
        tokio::select! {
            _ = tokio::time::sleep(wait) => None,
            nudge = next_nudge(nudge) => Some(nudge),
        }
    }

    /// Wait out a background interval and report whether the next pass runs
    /// in the ACTIVE regime.
    ///
    /// The one place the two regimes are chosen between after a settled
    /// non-connected outcome, so the rule reads in one line: an interval
    /// that simply elapsed means keep re-probing, and one cut short by a
    /// nudge means the ladder only if that nudge asked for it (a registry
    /// edit does; a plain retry does not — see [`Nudge::fresh_window`]).
    async fn hold(&self, nudge: &mut watch::Receiver<Nudge>, wait: Duration) -> bool {
        self.wait_or_nudge(nudge, wait)
            .await
            .is_some_and(|nudge| nudge.fresh_window)
    }

    /// Whether this failure is worth a log line, and how many identical
    /// ones were suppressed since the last one that was.
    ///
    /// `Some(suppressed)` means log it; `None` means stay quiet. The
    /// suppression is deliberately keyed on the TEXT rather than on a
    /// timer: a host that is down repeats one message forever, a peer
    /// erroring on every refresh repeats one message every few seconds, and
    /// in both cases the fourth identical line has told the operator
    /// nothing the first three did not. A CHANGED failure always logs, and
    /// carries the count of what it displaced, so a run of suppressed lines
    /// is visible as a number rather than as a silence.
    fn note_failure(&self, text: &str) -> Option<u64> {
        let mut last = self.last_failure.lock().expect("failure mutex poisoned");
        if last.text == text {
            last.seen += 1;
            if last.seen <= REPEATED_FAILURE_LOG_LIMIT {
                return Some(0);
            }
            last.suppressed += 1;
            return None;
        }
        let suppressed = last.suppressed;
        *last = RepeatedFailure {
            text: text.to_string(),
            seen: 1,
            suppressed: 0,
        };
        Some(suppressed)
    }

    /// This actor's current destination, for the log line being written.
    fn destination(&self) -> String {
        self.destination
            .lock()
            .expect("destination mutex poisoned")
            .clone()
    }
}

/// Build the [`AttemptOutcome::Failed`] for `error`, classifying the local
/// row's "nothing is listening" case (see [`UnreachableCause`]).
///
/// `{error:#}` rather than `{error}`: an `anyhow` chain's top layer is
/// usually the least specific ("spawning ssh"), and the alternate form is
/// what keeps the actual cause in the state the user is shown.
fn failure(row: &HostRow, error: anyhow::Error) -> AttemptOutcome {
    // `anyhow::Error::downcast_ref`, not a `chain()` walk: the marker is
    // attached with `.context(..)`, which wraps it in anyhow's own
    // `ContextError` — so the chain's entries are that wrapper, and only
    // anyhow's specialized downcast looks INSIDE it for the context value.
    // A `chain().any(|c| c.is::<..>())` compiles, always answers false,
    // and would silently reduce this to the generic transport case.
    let cause = if row.kind == HostKind::Local
        && error.downcast_ref::<LocalSupervisorNotRunning>().is_some()
    {
        UnreachableCause::LocalSupervisorNotRunning
    } else {
        UnreachableCause::TransportFailure
    };
    AttemptOutcome::Failed {
        cause,
        // Bounded and escaped: an anyhow chain from a dial can carry the
        // peer's own words (an ssh channel's annotated EOF, a supervisor's
        // refusal), and this string is both logged and RETAINED in the
        // unreachable state a UI renders. See `peer_text`.
        error: peer_text(&format!("{error:#}")),
    }
}

/// The remediation text a version-skewed host carries.
///
/// SPEC.md demands errors be actionable, not merely diagnostic, and the
/// action here is always the same one: update the farhelm binary on the
/// host. The destination is named because a user staring at a hosts list
/// needs to know WHICH machine to go to, and the local row is named as
/// such because "update the binary on <local>" would be nonsense.
fn skew_remediation(row: &HostRow) -> String {
    match row.destination.as_deref() {
        Some(dest) => format!(
            "update the farhelm binary on {dest} (or this helm) so the two speak the same protocol \
             version"
        ),
        None => "update the farhelm binary on this machine so the helm and its local supervisor \
                 speak the same protocol version"
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::io::{FrameReader, FrameWriter, parse_control};
    use farhelm_proto::{ControlMsg, PROTOCOL_VERSION, RestartOffer, SessionStatus};
    use tokio::sync::broadcast;

    // ---- Scripted supervisor peers ---------------------------------
    //
    // The state machine's interesting behavior is entirely in WHEN it
    // dials, WHAT the peer says, and what it does with the answer — none
    // of which needs a real process, a real socket, or a real ssh. So this
    // section reuses the peer pattern `client.rs`'s own tests established
    // (`tokio::io::duplex` plus a task playing the supervisor's side of
    // the handshake) and puts it behind a [`HostTransport`], which is the
    // seam that lets an actor be driven without any of the above.
    //
    // Every timing test below runs on tokio's virtual clock
    // (`#[tokio::test(start_paused = true)]`, the discipline
    // `farhelm-supervisor`'s connection and terminal tests established)
    // AGAINST THE PRODUCTION CADENCES. That combination is deliberate: a
    // shortened test-only ladder would pin that the code follows *a*
    // ladder, while the virtual clock makes it free to pin that it follows
    // *the* ladder, down to the exact second each retry lands on.

    /// What a scripted peer does when dialed, mutable between attempts so
    /// a test can bring a host up, take it down, or upgrade it mid-run.
    #[derive(Clone)]
    struct Script {
        /// `false` makes the DIAL itself fail — the host is down, and no
        /// peer is spawned at all.
        reachable: bool,
        protocol: u32,
        build: String,
        /// The identity the peer's hello reports. `None` models a
        /// supervisor with none to report.
        identity: Option<String>,
        /// What the peer's session list contains, served in
        /// `page_size` chunks so a walk really does have to follow
        /// cursors.
        sessions: Vec<SessionInfo>,
        page_size: usize,
        /// When set, every `ListSessions` is refused with this message as
        /// an `Internal` error — the shape a record too large to ship
        /// produces on a real supervisor (`build_list_reply`'s refusal,
        /// mapped by `handle_list_sessions`).
        list_error: Option<String>,
        /// The peer accepts the connection and then says NOTHING — no
        /// hello, ever. A dial that succeeds followed by a silence is a
        /// real failure mode (a network that blackholes after the TCP
        /// handshake, a wedged remote proxy) and it is the one the
        /// per-attempt deadline exists for, because it is indistinguishable
        /// from a healthy but slow host until a clock says otherwise.
        silent_hello: bool,
        /// The peer completes the handshake and then never answers a
        /// `ListSessions`. The connected counterpart of `silent_hello`, and
        /// the failure the refresh deadline exists for.
        silent_list: bool,
        /// The peer drops the connection after serving this many pages,
        /// mid-walk — a supervisor killed, or an ssh channel dying, while
        /// the helm was halfway through draining it.
        close_after_pages: Option<usize>,
        /// The TRANSPORT panics when this host is dialed, standing in for
        /// any bug that takes an actor's task down — there is no honest
        /// smaller stand-in, since a panic is exactly what an unexpected
        /// bug is.
        panic_on_dial: bool,
    }

    impl Default for Script {
        fn default() -> Self {
            Script {
                reachable: true,
                protocol: PROTOCOL_VERSION,
                build: "peer-build".to_string(),
                identity: Some("identity-a".to_string()),
                sessions: Vec::new(),
                page_size: 500,
                list_error: None,
                silent_hello: false,
                silent_list: false,
                close_after_pages: None,
                panic_on_dial: false,
            }
        }
    }

    /// One `ListSessions` exactly as it arrived — the request side of the
    /// wire, which the state assertions elsewhere cannot see.
    ///
    /// Recorded because two of the drain's contracts are entirely about
    /// what is SENT: that every page is requested with `limit: None` (so
    /// the supervisor applies its own cap, which is what keeps its
    /// per-page capture sweep from being multiplied — see
    /// [`drain_sessions`]), and that a walk really advances its cursor
    /// rather than re-asking for the same page.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ListRequest {
        host: HostId,
        cursor: Option<String>,
        limit: Option<u32>,
    }

    /// One recorded dial: which host, WHERE it was pointed, and when.
    ///
    /// The destination is recorded because it is the only observable that
    /// distinguishes "reconnected" from "reconnected to the edited
    /// address": states and identities look identical either way, so a
    /// reconfiguration test asserting on them alone would pass against a
    /// manager that reconnected to the old destination forever.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Dial {
        host: HostId,
        destination: Option<String>,
        at: Duration,
    }

    /// A [`HostTransport`] serving one [`Script`] per host, recording every
    /// dial with the virtual time it happened at.
    ///
    /// The attempt log is the primary observable for the cadence tests:
    /// asserting on the TIMES OF DIALS pins the ladder itself, whereas
    /// asserting only on states would pass equally for a machine that
    /// reached `Unreachable` by any schedule at all.
    struct ScriptedTransport {
        /// `Arc`-wrapped so a spawned peer can keep reading it: a peer
        /// answers each `ListSessions` from the CURRENT script, not from
        /// a snapshot taken when it was dialed. Without that, a test that
        /// breaks a healthy host's list mid-connection (the failed-refresh
        /// case) would silently keep getting the old, working answers and
        /// hang waiting for a failure that can never arrive.
        scripts: Arc<Mutex<HashMap<HostId, Script>>>,
        /// Sender-side of the attempt log. A `watch` rather than a plain
        /// vector so a test can WAIT for the Nth dial instead of polling
        /// for it, which under a virtual clock is the difference between
        /// deterministic and hung.
        attempts: watch::Sender<Vec<Dial>>,
        /// Every `ListSessions` any peer has received, in arrival order.
        /// See [`ListRequest`].
        requests: Arc<Mutex<Vec<ListRequest>>>,
        origin: tokio::time::Instant,
        /// Broadcast to every live peer, which then drops its half of the
        /// duplex — the client sees EOF, exactly as it would when an ssh
        /// child dies or a supervisor is killed.
        kill: broadcast::Sender<()>,
        /// Every peer that has EXITED, in order, as the far side observing
        /// its connection being closed.
        ///
        /// The only way to assert the helm-side half of teardown from the
        /// peer's point of view: a manager that stopped an actor but leaked
        /// its client would leave the peer's read loop parked forever, and
        /// nothing observable on the helm side would say so.
        closures: watch::Sender<Vec<HostId>>,
    }

    impl ScriptedTransport {
        fn new() -> Arc<ScriptedTransport> {
            let (attempts, _) = watch::channel(Vec::new());
            let (kill, _) = broadcast::channel(8);
            Arc::new(ScriptedTransport {
                scripts: Arc::new(Mutex::new(HashMap::new())),
                attempts,
                requests: Arc::new(Mutex::new(Vec::new())),
                origin: tokio::time::Instant::now(),
                kill,
                closures: watch::Sender::new(Vec::new()),
            })
        }

        /// Wait until at least `n` of `host`'s peers have seen their
        /// connection close.
        async fn wait_for_closures(&self, host: HostId, n: usize) {
            let mut rx = self.closures.subscribe();
            let _ = rx
                .wait_for(|log| log.iter().filter(|id| **id == host).count() >= n)
                .await;
        }

        /// Every `ListSessions` one host's peers have received so far.
        fn requests(&self, host: HostId) -> Vec<ListRequest> {
            self.requests
                .lock()
                .expect("request log mutex")
                .iter()
                .filter(|request| request.host == host)
                .cloned()
                .collect()
        }

        fn set_script(&self, host: HostId, script: Script) {
            self.scripts
                .lock()
                .expect("script mutex")
                .insert(host, script);
        }

        /// Mutate one host's script in place — the "the peer was upgraded"
        /// / "the host came back" move the recovery tests make.
        fn edit(&self, host: HostId, edit: impl FnOnce(&mut Script)) {
            let mut scripts = self.scripts.lock().expect("script mutex");
            edit(scripts.entry(host).or_default());
        }

        /// Every dial of `host` so far, as virtual time since this
        /// transport was created.
        fn attempts(&self, host: HostId) -> Vec<Duration> {
            self.dials(host).into_iter().map(|dial| dial.at).collect()
        }

        /// Every dial of `host` so far, whole.
        fn dials(&self, host: HostId) -> Vec<Dial> {
            self.attempts
                .borrow()
                .iter()
                .filter(|dial| dial.host == host)
                .cloned()
                .collect()
        }

        /// The destination each dial of `host` was pointed at, in order.
        fn dialed_destinations(&self, host: HostId) -> Vec<String> {
            self.dials(host)
                .into_iter()
                .map(|dial| dial.destination.unwrap_or_else(|| "<local>".to_string()))
                .collect()
        }

        /// Wait until `host` has been dialed at least `n` times, then
        /// return the whole log for it.
        async fn wait_for_attempts(&self, host: HostId, n: usize) -> Vec<Duration> {
            let mut rx = self.attempts.subscribe();
            let _ = rx
                .wait_for(|log| log.iter().filter(|dial| dial.host == host).count() >= n)
                .await;
            self.attempts(host)
        }

        /// Kill every live peer connection.
        fn kill_connections(&self) {
            let _ = self.kill.send(());
        }
    }

    impl HostTransport for ScriptedTransport {
        fn connect<'a>(
            &'a self,
            host: &'a HostRow,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<TransportPair>> + Send + 'a>> {
            let id = host.id;
            let destination = host.destination.clone();
            Box::pin(async move {
                let at = self.origin.elapsed();
                self.attempts.send_modify(|log| {
                    log.push(Dial {
                        host: id,
                        destination,
                        at,
                    })
                });
                let script = self
                    .scripts
                    .lock()
                    .expect("script mutex")
                    .get(&id)
                    .cloned()
                    .unwrap_or_default();
                // Panicked with no lock held: the script map's guard is a
                // temporary of the statement above, so this cannot poison
                // a mutex the rest of the fixture still needs.
                assert!(!script.panic_on_dial, "scripted panic dialing host {id}");
                if !script.reachable {
                    anyhow::bail!("scripted host {id} is down");
                }
                let (ours, theirs) = tokio::io::duplex(256 * 1024);
                let kill = self.kill.subscribe();
                let peer = PeerContext {
                    scripts: Arc::clone(&self.scripts),
                    requests: Arc::clone(&self.requests),
                    closures: self.closures.clone(),
                    id,
                };
                tokio::spawn(run_peer(theirs, script, peer, kill));
                let (r, w) = tokio::io::split(ours);
                Ok((
                    Box::new(r) as Box<dyn AsyncRead + Send + Unpin>,
                    Box::new(w) as Box<dyn AsyncWrite + Send + Unpin>,
                ))
            })
        }
    }

    /// Play the supervisor's side of one connection until the helm hangs
    /// up or the test kills it.
    ///
    /// Writes its hello WITHOUT first waiting for the helm's, matching the
    /// protocol's crossing-hellos rule (`io::handshake`'s own docs) — a
    /// peer that waited would deadlock against a helm that is also
    /// waiting.
    ///
    /// Two script sources on purpose. `hello` is the snapshot taken when
    /// this connection was dialed, because a hello really is exchanged
    /// once and cannot change afterwards; `scripts` is the LIVE map, read
    /// per request, because a test that changes how a host answers must
    /// affect the connection that is already open (see
    /// [`ScriptedTransport::scripts`]).
    /// The shared state one scripted peer reads and writes: the live script
    /// map, the request log, the closure log, and which host it is playing.
    ///
    /// Bundled rather than passed as four parameters because they travel
    /// together everywhere and a peer that received them individually would
    /// invite exactly the mix-up (one host's script, another's log) the
    /// tests would then have to debug.
    #[derive(Clone)]
    struct PeerContext {
        scripts: Arc<Mutex<HashMap<HostId, Script>>>,
        requests: Arc<Mutex<Vec<ListRequest>>>,
        closures: watch::Sender<Vec<HostId>>,
        id: HostId,
    }

    impl Drop for PeerContext {
        /// A peer's exit — for ANY reason: the helm hung up, the test
        /// killed it, the script told it to close mid-walk — is what the
        /// far side of a closed connection looks like. Recorded on drop
        /// rather than at each return so no path can forget to.
        fn drop(&mut self) {
            let id = self.id;
            self.closures.send_modify(|log| log.push(id));
        }
    }

    async fn run_peer(
        io: tokio::io::DuplexStream,
        hello: Script,
        peer: PeerContext,
        mut kill: broadcast::Receiver<()>,
    ) {
        // Copied out rather than borrowed from `peer`: the context itself
        // must stay owned by this function for its whole body, since its
        // DROP is what records the closure this peer's exit represents.
        let (id, scripts, requests) = (
            peer.id,
            Arc::clone(&peer.scripts),
            Arc::clone(&peer.requests),
        );
        let (r, w) = tokio::io::split(io);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        if hello.silent_hello {
            // Hold the connection OPEN and say nothing: dropping the
            // duplex would EOF the helm's reader and produce an ordinary
            // failed dial, which is the opposite of what this models. The
            // kill signal is still honored so a test can tear the peer down
            // deliberately.
            let _ = kill.recv().await;
            return;
        }
        let greeting = ControlMsg::Hello {
            protocol_version: hello.protocol,
            build_version: hello.build.clone(),
            role: "supervisor".to_string(),
            host_identity: hello.identity.clone(),
        };
        if writer.write_control(&greeting).await.is_err() {
            return;
        }
        // The helm's own hello — or, on skew, its refusal, after which it
        // hangs up and the loop below simply ends.
        if !matches!(reader.read_frame().await, Ok(Some(_))) {
            return;
        }
        let mut pages_served = 0usize;
        loop {
            let frame = tokio::select! {
                _ = kill.recv() => return,
                frame = reader.read_frame() => frame,
            };
            let Ok(Some(frame)) = frame else { return };
            let Ok(msg) = parse_control(&frame) else {
                return;
            };
            if let ControlMsg::ListSessions {
                req_id,
                cursor,
                limit,
            } = msg
            {
                requests
                    .lock()
                    .expect("request log mutex")
                    .push(ListRequest {
                        host: id,
                        cursor: cursor.clone(),
                        limit,
                    });
                let current = scripts
                    .lock()
                    .expect("script mutex")
                    .get(&id)
                    .cloned()
                    .unwrap_or_default();
                if current.silent_list {
                    // Received, never answered — the connection stays
                    // healthy at the transport level, which is exactly what
                    // makes this invisible without a deadline.
                    continue;
                }
                if current.close_after_pages.is_some_and(|n| pages_served >= n) {
                    // Returning drops this peer's half of the duplex: the
                    // helm sees EOF mid-walk, as it would if the supervisor
                    // had been killed between two pages.
                    return;
                }
                if writer
                    .write_control(&list_reply(&current, req_id, cursor))
                    .await
                    .is_err()
                {
                    return;
                }
                pages_served += 1;
            }
        }
    }

    /// One page of a scripted peer's session list.
    ///
    /// The cursor is the index of the next entry, rendered as a string.
    /// Opaque to the helm by contract, so any encoding will do — and a
    /// transparent one keeps the test's own arithmetic checkable at a
    /// glance.
    fn list_reply(script: &Script, req_id: u64, cursor: Option<String>) -> ControlMsg {
        if let Some(message) = &script.list_error {
            return ControlMsg::Error {
                req_id,
                message: message.clone(),
                kind: ErrorKind::Internal,
            };
        }
        let start: usize = cursor
            .as_deref()
            .map(|c| c.parse().expect("the test's own cursor round-trips"))
            .unwrap_or(0);
        let end = (start + script.page_size).min(script.sessions.len());
        ControlMsg::SessionList {
            req_id,
            sessions: script.sessions[start..end].to_vec(),
            total: script.sessions.len() as u64,
            next_cursor: (end < script.sessions.len()).then(|| end.to_string()),
        }
    }

    /// A minimal, distinct session — mirrors `store.rs`'s helper of the
    /// same shape, since both modules need "a session that round-trips"
    /// and neither needs the other's field coverage.
    fn session(id: &str, created_at: i64) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            title: id.to_string(),
            created_at,
            cwd: format!("/{id}"),
            invocation: "agent".to_string(),
            status: SessionStatus::Alive,
            annotation: None,
            restart_offer: RestartOffer::default(),
            tabs: Vec::new(),
        }
    }

    /// A store, a scripted transport, and a manager over both.
    ///
    /// The `TempDir` guard must outlive the store in every caller —
    /// dropping it deletes the database out from under an open connection.
    struct Fixture {
        _dir: tempfile::TempDir,
        store: HelmStore,
        transport: Arc<ScriptedTransport>,
        manager: Arc<ConnectionManager>,
    }

    /// Open a store, let `setup` register whatever hosts and scripts the
    /// test needs, and start a manager over the result.
    ///
    /// `setup` runs BEFORE the manager starts on purpose: an actor that is
    /// already dialing while a test is still writing rows would make
    /// several of the tests below race their own fixtures.
    ///
    /// The reserved local row is scripted UNREACHABLE by default. It exists
    /// in every registry and therefore gets an actor in every test, while
    /// almost no test is about it — and a local row that connected would
    /// interleave its own dials, hellos and refreshes into whatever the
    /// test is actually watching. A test that cares about the local row
    /// overrides the script in `setup` like any other.
    async fn fixture<F, Fut>(cadence: Cadence, setup: F) -> Fixture
    where
        F: FnOnce(HelmStore, Arc<ScriptedTransport>) -> Fut,
        Fut: Future<Output = ()>,
    {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = HelmStore::open(&dir.path().join("helm.db"))
            .await
            .expect("open store");
        let transport = ScriptedTransport::new();
        transport.set_script(
            local_id(&store).await,
            Script {
                reachable: false,
                ..Script::default()
            },
        );
        setup(store.clone(), Arc::clone(&transport)).await;
        let manager = ConnectionManager::start(
            store.clone(),
            Arc::clone(&transport) as Arc<dyn HostTransport>,
            cadence,
        )
        .await
        .expect("start manager");
        Fixture {
            _dir: dir,
            store,
            transport,
            manager,
        }
    }

    /// The reserved local row's id, which `HelmStore::open` mints first
    /// and `AUTOINCREMENT` therefore fixes at 1.
    async fn local_id(store: &HelmStore) -> HostId {
        store.list_hosts().await.expect("list hosts")[0].id
    }

    /// Seed a host's identity the way a successful first contact would
    /// have, reading the row's dialed configuration back out of the store
    /// first.
    ///
    /// Every fixture that wants "this host is already known" goes through
    /// here rather than calling the store directly, because the identity
    /// writers now refuse an attempt whose configuration does not match the
    /// row (see [`DialedAs`]) — and a test seeding a plausible-looking
    /// configuration by hand would be pinning its own guess instead of the
    /// registry's answer.
    async fn record_contact(store: &HelmStore, host: HostId, identity: &str) {
        let row = store
            .list_hosts()
            .await
            .expect("list hosts")
            .into_iter()
            .find(|row| row.id == host)
            .expect("host row");
        let outcome = store
            .record_first_contact(host, &DialedAs::of(&row), identity)
            .await
            .expect("record first contact");
        assert_eq!(
            outcome,
            FirstContactOutcome::Recorded,
            "fixtures seed identities on rows that have none yet"
        );
    }

    /// One host's live connection, from the atomic status read — the
    /// client half of [`ConnectionManager::status`], for assertions that
    /// only care about that half.
    fn status_client(manager: &ConnectionManager, host: HostId) -> Option<Arc<SupervisorClient>> {
        manager.status(host).and_then(|status| status.client)
    }

    /// A host's cached session ids, in wire order — the terse form of
    /// "exactly these sessions, in this order".
    async fn cached_ids(store: &HelmStore, host: HostId) -> Vec<String> {
        store
            .cached_sessions(host)
            .await
            .expect("cached sessions")
            .into_iter()
            .map(|info| info.id)
            .collect()
    }

    /// One host's recorded identity, straight from the registry — the
    /// independent check that a state machine decision did (or did not)
    /// reach durable storage.
    async fn recorded_identity(store: &HelmStore, host: HostId) -> Option<String> {
        store
            .list_hosts()
            .await
            .expect("list hosts")
            .into_iter()
            .find(|row| row.id == host)
            .expect("host row")
            .host_identity
    }

    /// Attempt timestamps as whole seconds, for readable assertions
    /// against the ladder. Exact rather than approximate: on tokio's
    /// virtual clock a sleep expires at precisely its deadline, so a
    /// rounding tolerance here would only hide a real drift.
    fn seconds(attempts: &[Duration]) -> Vec<u64> {
        attempts.iter().map(|d| d.as_secs()).collect()
    }

    // ---- Cadences ---------------------------------------------------

    /// The whole point of the active-retry ladder: a host that is down
    /// must be dialed on PLAN_M6.md item 4's exact schedule — immediately,
    /// then at 1, 2, 4, 8, 15 and 30 second intervals, about a minute in
    /// total — and must then fall back to a steady forty-five-second
    /// re-probe forever, never to a give-up and never to another full
    /// ladder.
    ///
    /// The last clause is the regression this test exists for. Re-running
    /// the ladder on every re-probe (the shape the code has if `run`'s
    /// `active` flag is dropped) still reaches `Unreachable` and still
    /// recovers, so a state-only assertion passes — while the host is
    /// actually being dialed every few seconds forever, which is exactly
    /// the hammering the bounded-then-periodic contract exists to prevent.
    /// Only the dial TIMES catch it.
    #[tokio::test(start_paused = true)]
    async fn active_retries_follow_the_ladder_then_settle_into_reprobing() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("down.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    reachable: false,
                    ..Script::default()
                },
            );
            // The local row is never dialed in this test; leave it down
            // too so it cannot interleave a connection.
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;

        // Seven dials: the immediate one plus the six ladder steps.
        let active = fixture.transport.wait_for_attempts(host, 7).await;
        assert_eq!(
            seconds(&active),
            vec![0, 1, 3, 7, 15, 30, 60],
            "the active window must be the immediate attempt plus CONNECT_BACKOFF's six steps"
        );

        let state = fixture
            .manager
            .wait_for_state(host, |state| matches!(state, HostState::Unreachable { .. }))
            .await
            .expect("actor is running");
        assert!(
            matches!(
                state,
                HostState::Unreachable {
                    cause: UnreachableCause::TransportFailure,
                    ..
                }
            ),
            "a down ssh host is a plain transport failure, not the local-supervisor case: {state:?}"
        );

        // Two more re-probes, each exactly one dial, forty-five seconds
        // apart — not a fresh ladder.
        let reprobing = fixture.transport.wait_for_attempts(host, 9).await;
        assert_eq!(
            seconds(&reprobing),
            vec![0, 1, 3, 7, 15, 30, 60, 105, 150],
            "re-probes must be single attempts on the 45s cadence, never repeated ladders"
        );
    }

    /// A host that comes back while it is being re-probed must reconnect
    /// on its own, with no user action — SPEC.md's "a host that comes back
    /// overnight resurfaces by itself", pinned at the cadence it promises.
    #[tokio::test(start_paused = true)]
    async fn a_returning_host_reconnects_on_the_reprobe_cadence() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("flaky.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    reachable: false,
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;

        fixture
            .manager
            .wait_for_state(host, |state| matches!(state, HostState::Unreachable { .. }))
            .await
            .expect("actor is running");
        fixture
            .transport
            .edit(host, |script| script.reachable = true);

        let state = fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");
        let HostState::Connected { identity, .. } = state else {
            unreachable!("filtered on is_connected");
        };
        assert_eq!(identity.as_deref(), Some("identity-a"));
        // The recovery rode the next scheduled probe, not a new ladder:
        // the eighth dial is the first one after the active window.
        assert_eq!(seconds(&fixture.transport.attempts(host))[7], 105);
    }

    /// A connection that was UP and is lost gets a fresh active window
    /// rather than being made to wait out a re-probe.
    ///
    /// This is the asymmetry `run`'s `active` flag encodes, and it matters
    /// for the most common real failure there is: a supervisor restarted
    /// by hand is back within a second or two, and forty-five seconds of
    /// visible downtime for it would be a papercut of the manager's own
    /// making.
    #[tokio::test(start_paused = true)]
    async fn a_lost_connection_re_enters_the_active_window_immediately() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("blip.example", None, None)
                .await
                .unwrap();
            transport.set_script(host, Script::default());
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");
        assert_eq!(seconds(&fixture.transport.attempts(host)), vec![0]);

        // Take the peer away, but let the next dial succeed: the point is
        // WHEN the redial happens, not that it fails.
        fixture.transport.kill_connections();

        let attempts = fixture.transport.wait_for_attempts(host, 2).await;
        assert_eq!(
            seconds(&attempts),
            vec![0, 0],
            "a redial after a loss must be immediate, not a re-probe away"
        );
        fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");
    }

    /// A dial that CONNECTS and then says nothing must cost one attempt,
    /// not the whole window.
    ///
    /// Without a per-attempt deadline this is the worst of the failure
    /// modes: the transport succeeds, so nothing errors, and the actor
    /// parks inside a single attempt forever — no ladder, no re-probe, no
    /// unreachable state, and a host that reads as "connecting" until the
    /// helm restarts. The timing assertion is the whole test: each attempt
    /// must expire after exactly [`CONNECT_ATTEMPT_TIMEOUT`] and the ladder
    /// must then step as usual, so the dials land at the deadline plus each
    /// backoff rather than at the backoffs alone.
    #[tokio::test(start_paused = true)]
    async fn a_peer_that_accepts_and_never_says_hello_still_advances_the_ladder() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("mute.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    silent_hello: true,
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;

        let attempts = fixture.transport.wait_for_attempts(host, 7).await;
        assert_eq!(
            seconds(&attempts),
            vec![0, 21, 43, 67, 95, 130, 180],
            "each attempt must expire at the 20s deadline and the ladder step from there"
        );
        let state = fixture
            .manager
            .wait_for_state(host, |state| matches!(state, HostState::Unreachable { .. }))
            .await
            .expect("actor is running");
        assert!(
            matches!(&state, HostState::Unreachable { last_error, .. }
                if last_error.contains("did not complete")),
            "the state must say the attempt timed out rather than blaming the network: {state:?}"
        );
    }

    // ---- Version skew ------------------------------------------------

    /// A supervisor speaking another protocol version must land in its own
    /// state carrying BOTH versions and a remediation — never in
    /// `Unreachable`, where the user would be told to check the network
    /// about a problem only a binary upgrade fixes (SPEC.md: actionable,
    /// not merely diagnostic).
    ///
    /// The recovery half is what makes the state worth having: once the
    /// host is upgraded, the same forty-five-second re-probe that serves
    /// unreachable hosts brings it back with no user action at all, so an
    /// upgraded host resurfaces alone.
    #[tokio::test(start_paused = true)]
    async fn version_skew_names_both_versions_and_recovers_when_the_host_upgrades() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store.add_ssh_host("old.example", None, None).await.unwrap();
            transport.set_script(
                host,
                Script {
                    protocol: PROTOCOL_VERSION - 1,
                    build: "0.0.1-old".to_string(),
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;

        let state = fixture
            .manager
            .wait_for_state(host, |state| matches!(state, HostState::VersionSkew { .. }))
            .await
            .expect("actor is running");
        let HostState::VersionSkew {
            peer_protocol,
            peer_build,
            our_protocol,
            remediation,
            ..
        } = &state
        else {
            unreachable!("filtered on VersionSkew");
        };
        assert_eq!(*peer_protocol, PROTOCOL_VERSION - 1);
        assert_eq!(peer_build, "0.0.1-old");
        assert_eq!(*our_protocol, PROTOCOL_VERSION);
        assert!(
            remediation.contains("old.example") && remediation.contains("update"),
            "the remediation must name the host to update: {remediation:?}"
        );
        // A refusal is an answer, so it must not have spent the ladder on
        // six identical re-refusals before reporting itself.
        assert_eq!(seconds(&fixture.transport.attempts(host)), vec![0]);

        // Nothing was ever recorded for a host we refused to talk to.
        assert_eq!(recorded_identity(&fixture.store, host).await, None);

        fixture
            .transport
            .edit(host, |script| script.protocol = PROTOCOL_VERSION);
        fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");
        assert_eq!(
            seconds(&fixture.transport.attempts(host)),
            vec![0, 45],
            "a skewed host must be re-probed on the same 45s cadence an unreachable one is"
        );
    }

    // ---- Identity ----------------------------------------------------

    /// First contact records the identity the host reported, and the
    /// connected state carries it — the precondition every cache write
    /// downstream is bound to.
    #[tokio::test(start_paused = true)]
    async fn a_first_contact_records_the_hosts_identity() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store.add_ssh_host("new.example", None, None).await.unwrap();
            // Asserted HERE, before the manager exists, rather than after
            // it starts: once an actor is running, "nothing is recorded
            // yet" is a claim about a race — the first contact this test is
            // about may already have landed — and a passing assertion would
            // mean only that this thread got there first.
            assert_eq!(
                recorded_identity(&store, host).await,
                None,
                "a freshly added host starts with no identity"
            );
            transport.set_script(
                host,
                Script {
                    identity: Some("fresh-identity".to_string()),
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;

        let state = fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");
        assert!(
            matches!(
                &state,
                HostState::Connected { identity, build_version, .. }
                    if identity.as_deref() == Some("fresh-identity")
                        && build_version == "peer-build"
            ),
            "{state:?}"
        );
        assert_eq!(
            recorded_identity(&fixture.store, host).await,
            Some("fresh-identity".to_string()),
            "first contact must reach durable storage, not just the in-memory state"
        );
    }

    /// A destination reporting a DIFFERENT identity than the one on record
    /// freezes the host and writes nothing — SPEC.md's never-silently-merge
    /// rule, observed from both sides at once.
    ///
    /// Three separate claims, each of which has failed independently in
    /// designs of this shape: the state carries BOTH identities (so the
    /// user can see what the choice is between), the registry is
    /// untouched (so declining to decide costs nothing), and the actor
    /// makes no further attempts (so a freeze is really frozen, not a slow
    /// retry loop that would re-ask a question only a human can answer).
    #[tokio::test(start_paused = true)]
    async fn an_identity_mismatch_freezes_the_host_and_writes_nothing() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("recycled.example", None, None)
                .await
                .unwrap();
            record_contact(&store, host, "identity-original").await;
            transport.set_script(
                host,
                Script {
                    identity: Some("identity-reinstalled".to_string()),
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;

        let state = fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(state, HostState::IdentityMismatch { .. })
            })
            .await
            .expect("actor is running");
        assert_eq!(
            state,
            HostState::IdentityMismatch {
                recorded: "identity-original".to_string(),
                reported: "identity-reinstalled".to_string(),
            }
        );
        assert_eq!(
            recorded_identity(&fixture.store, host).await,
            Some("identity-original".to_string()),
            "a mismatch must leave the registry exactly as the user last left it"
        );
        assert!(
            status_client(&fixture.manager, host).is_none(),
            "a frozen host must hold no connection"
        );

        // Frozen means frozen: well past several re-probe intervals, the
        // dial count has not moved.
        tokio::time::advance(REPROBE_INTERVAL * 5).await;
        tokio::task::yield_now().await;
        assert_eq!(
            fixture.transport.attempts(host).len(),
            1,
            "an identity mismatch must not be re-probed; only a user decision resolves it"
        );
    }

    /// Adopting the reported identity is the user's explicit
    /// acknowledgment, and it must do all three things at once: swap the
    /// identity, purge the dead install's cached sessions, and reconnect
    /// — without the user having to do anything else.
    #[tokio::test(start_paused = true)]
    async fn adopting_a_mismatched_identity_purges_the_old_cache_and_reconnects() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("recycled.example", None, None)
                .await
                .unwrap();
            record_contact(&store, host, "identity-original").await;
            // The dead install's sessions, which adoption must not carry
            // forward under the new identity.
            store
                .replace_host_sessions(
                    host,
                    "identity-original",
                    vec![session("ghost", 100), session("phantom", 90)],
                )
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    identity: Some("identity-reinstalled".to_string()),
                    sessions: vec![session("live", 200)],
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(state, HostState::IdentityMismatch { .. })
            })
            .await
            .expect("actor is running");

        fixture.manager.adopt(host).await.expect("adopt");

        let state = fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");
        assert!(
            matches!(&state, HostState::Connected { identity, .. }
                if identity.as_deref() == Some("identity-reinstalled")),
            "{state:?}"
        );
        assert_eq!(
            recorded_identity(&fixture.store, host).await,
            Some("identity-reinstalled".to_string())
        );
        let cached = fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Ok { .. },
                        ..
                    }
                )
            })
            .await;
        assert!(cached.is_some());
        let ids: Vec<String> = fixture
            .store
            .cached_sessions(host)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(
            ids,
            vec!["live".to_string()],
            "the dead install's cached sessions must be gone, not merged with the new one's"
        );
    }

    /// Adoption is refused for a host that is not actually awaiting a
    /// decision. A caller that adopts on the wrong host would otherwise
    /// perform a compare-and-swap against whatever the state happened to
    /// be, which is exactly the silent merge the whole mechanism exists to
    /// prevent.
    #[tokio::test(start_paused = true)]
    async fn adopting_a_host_that_is_not_mismatched_is_refused() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("fine.example", None, None)
                .await
                .unwrap();
            transport.set_script(host, Script::default());
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");

        let error = fixture.manager.adopt(host).await.expect_err("must refuse");
        assert!(
            error.to_string().contains("connected"),
            "the refusal must name the state the host is actually in: {error:#}"
        );
    }

    // ---- Registry edits take effect ------------------------------------

    /// Editing a CONNECTED host's destination must actually retarget it:
    /// the live connection goes, and the next dial is pointed at the new
    /// address.
    ///
    /// The behavior this replaces did the opposite — an edited row kept its
    /// connection until it happened to drop on its own — so a user fixing a
    /// wrong address saw their fix silently do nothing, for as long as the
    /// wrong host stayed up. Asserting on the dialed DESTINATIONS rather
    /// than on states is what makes the difference visible: a manager that
    /// merely reconnected to the old address would satisfy every
    /// state-shaped assertion here.
    #[tokio::test(start_paused = true)]
    async fn editing_a_connected_hosts_destination_reconnects_to_the_new_one() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("before.example", None, None)
                .await
                .unwrap();
            transport.set_script(host, Script::default());
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");
        // The peer at the NEW address is a different machine, and says so
        // — which is how the reconnection proves it reached it. (No client
        // handle is held anywhere in this test: one would keep the old
        // connection alive and hide exactly the teardown being asserted.)
        fixture
            .transport
            .edit(host, |script| script.build = "after-build".to_string());

        fixture
            .store
            .update_ssh_destination(host, "after.example")
            .await
            .expect("retarget the host");
        fixture.manager.sync_registry().await.unwrap();

        fixture.transport.wait_for_closures(host, 1).await;
        fixture.transport.wait_for_attempts(host, 2).await;
        assert_eq!(
            fixture.transport.dialed_destinations(host),
            vec!["before.example", "after.example"],
            "the edit must be dialed, not merely recorded"
        );
        let state = fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(state, HostState::Connected { build_version, .. }
                    if build_version == "after-build")
            })
            .await
            .expect("actor is running");
        assert!(
            state.is_connected(),
            "the host must end up connected to the edited destination: {state:?}"
        );
    }

    /// `retry_now` against a connected host is a RECONNECT, not a nudged
    /// refresh.
    ///
    /// This was the ambiguity in the old contract: the call only shortened
    /// the refresh sleep, so a user asking a connected-but-misbehaving host
    /// to retry got one early poll on the same connection — which is
    /// precisely the thing that was already not working. One dial before
    /// and two after is the whole assertion.
    #[tokio::test(start_paused = true)]
    async fn retry_now_on_a_connected_host_reconnects_rather_than_refreshing() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("live.example", None, None)
                .await
                .unwrap();
            transport.set_script(host, Script::default());
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");
        assert_eq!(fixture.transport.attempts(host).len(), 1);
        // A distinguishable hello on the far side of the retry: the state's
        // build version is what proves a NEW handshake happened rather than
        // the old connection being polled again. No client handle is held
        // here — holding one would keep the connection this test is trying
        // to observe being dropped.
        fixture
            .transport
            .edit(host, |script| script.build = "after-retry".to_string());

        fixture.manager.retry_now(host);
        fixture.transport.wait_for_closures(host, 1).await;
        fixture.transport.wait_for_attempts(host, 2).await;
        let state = fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(state, HostState::Connected { build_version, .. }
                    if build_version == "after-retry")
            })
            .await
            .expect("actor is running");
        assert!(
            state.is_connected(),
            "a retry on a connected host must produce a genuinely new connection: {state:?}"
        );
    }

    /// `retry_now` against an UNREACHABLE host is a single probe, and
    /// leaves the displayed state alone while it makes it.
    ///
    /// The other half of the contract, and the reason the two are not the
    /// same call: a user clicking retry is not evidence the host is back,
    /// so unfolding a fresh sixty-second ladder per click would let the UI
    /// hammer a dead host on demand. The state must also not flicker into
    /// "connecting" for the duration — an entry that has been down for
    /// hours reads as down, which is the same rule background re-probing
    /// follows.
    #[tokio::test(start_paused = true)]
    async fn retry_now_on_an_unreachable_host_is_a_single_probe() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("down.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    reachable: false,
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        let settled = fixture
            .manager
            .wait_for_state(host, |state| matches!(state, HostState::Unreachable { .. }))
            .await
            .expect("actor is running");
        // The whole active window, and nothing after it yet.
        assert_eq!(fixture.transport.attempts(host).len(), 7);

        fixture.manager.retry_now(host);
        let attempts = fixture.transport.wait_for_attempts(host, 8).await;
        assert_eq!(
            seconds(&attempts)[7],
            60,
            "the retry must be dialed immediately, at the moment it was asked for"
        );
        assert_eq!(
            fixture.manager.state(host),
            Some(settled),
            "a retry must not disturb what the user is being shown"
        );

        // ...and the cadence it returns to is the ordinary re-probe, not a
        // ladder the retry smuggled in.
        let attempts = fixture.transport.wait_for_attempts(host, 9).await;
        assert_eq!(
            seconds(&attempts)[8],
            105,
            "the next probe must be one full re-probe interval later"
        );
    }

    /// The freeze-resolution ladder, reached through a registry EDIT rather
    /// than through adoption: a mismatched entry whose destination is
    /// corrected gets the full active window at its new address.
    ///
    /// The replacement is deliberately DOWN, because that is the only way
    /// to observe the window at all — a host that answers settles on the
    /// first attempt and proves nothing about what the other six would have
    /// done. What this pins is that an edit is treated as the user action
    /// it is: the same ladder a resolved freeze earns, not the single probe
    /// a background re-check would make.
    #[tokio::test(start_paused = true)]
    async fn fixing_a_mismatched_hosts_destination_runs_the_full_ladder() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("recycled.example", None, None)
                .await
                .unwrap();
            record_contact(&store, host, "identity-original").await;
            transport.set_script(
                host,
                Script {
                    identity: Some("identity-someone-else".to_string()),
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(state, HostState::IdentityMismatch { .. })
            })
            .await
            .expect("actor is running");

        // The user fixes the address. The machine at the NEW one is down —
        // and, being a different machine, no longer answers with the
        // identity that caused the mismatch.
        fixture.transport.edit(host, |script| {
            script.reachable = false;
            script.identity = Some("identity-original".to_string());
        });
        fixture
            .store
            .update_ssh_destination(host, "corrected.example")
            .await
            .expect("correct the destination");
        fixture.manager.sync_registry().await.unwrap();

        let attempts = fixture.transport.wait_for_attempts(host, 8).await;
        assert_eq!(
            seconds(&attempts),
            vec![0, 0, 1, 3, 7, 15, 30, 60],
            "the corrected destination must get a whole fresh active window, not one probe"
        );
        assert_eq!(
            fixture.transport.dialed_destinations(host)[1..],
            vec!["corrected.example"; 7],
            "every attempt after the edit must go to the corrected destination"
        );
        let state = fixture
            .manager
            .wait_for_state(host, |state| matches!(state, HostState::Unreachable { .. }))
            .await
            .expect("actor is running");
        assert!(matches!(state, HostState::Unreachable { .. }));
    }

    // ---- Duplicates ---------------------------------------------------

    /// Two registry entries reaching one host: the second must connect
    /// NOTHING and record nothing, so the host appears exactly once (under
    /// the entry that already owns it) while the duplicate entry stays
    /// visible as something to resolve.
    ///
    /// The twin is pre-recorded rather than raced into existence, so the
    /// test pins the detection rather than which of two concurrent actors
    /// happened to win.
    #[tokio::test(start_paused = true)]
    async fn a_second_entry_reaching_a_known_identity_is_a_duplicate() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let first = store
                .add_ssh_host("host.example", None, None)
                .await
                .unwrap();
            record_contact(&store, first, "shared").await;
            let second = store
                .add_ssh_host("host.example.via-vpn", None, None)
                .await
                .unwrap();
            // The twin is deliberately unreachable: this test is about the
            // SECOND entry's decision, and a live twin would only add a
            // second connection to reason about.
            transport.set_script(
                first,
                Script {
                    reachable: false,
                    ..Script::default()
                },
            );
            transport.set_script(
                second,
                Script {
                    identity: Some("shared".to_string()),
                    ..Script::default()
                },
            );
        })
        .await;
        let rows = fixture.store.list_hosts().await.unwrap();
        let (first, second) = (rows[1].id, rows[2].id);

        let state = fixture
            .manager
            .wait_for_state(second, |state| matches!(state, HostState::Duplicate { .. }))
            .await
            .expect("actor is running");
        assert_eq!(
            state,
            HostState::Duplicate {
                twin: first,
                identity: "shared".to_string(),
            }
        );
        assert!(
            status_client(&fixture.manager, second).is_none(),
            "a duplicate entry must hold no connection"
        );
        assert_eq!(
            recorded_identity(&fixture.store, second).await,
            None,
            "a duplicate must not record the identity; that would make the collision durable"
        );
    }

    /// Removing the twin resolves the duplicate by itself, on the re-probe
    /// cadence — the "edit it or remove it" resolution actually working
    /// end to end, rather than leaving the surviving entry stuck in a
    /// state whose cause is gone.
    #[tokio::test(start_paused = true)]
    async fn a_duplicate_resolves_itself_once_the_twin_is_gone() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let first = store
                .add_ssh_host("host.example", None, None)
                .await
                .unwrap();
            record_contact(&store, first, "shared").await;
            let second = store
                .add_ssh_host("host.example.via-vpn", None, None)
                .await
                .unwrap();
            transport.set_script(
                first,
                Script {
                    reachable: false,
                    ..Script::default()
                },
            );
            transport.set_script(
                second,
                Script {
                    identity: Some("shared".to_string()),
                    ..Script::default()
                },
            );
        })
        .await;
        let rows = fixture.store.list_hosts().await.unwrap();
        let (first, second) = (rows[1].id, rows[2].id);
        fixture
            .manager
            .wait_for_state(second, |state| matches!(state, HostState::Duplicate { .. }))
            .await
            .expect("actor is running");

        fixture.store.remove_ssh_host(first).await.unwrap();
        fixture.manager.sync_registry().await.unwrap();

        let state = fixture
            .manager
            .wait_for_state(second, HostState::is_connected)
            .await
            .expect("actor is running");
        assert!(
            matches!(&state, HostState::Connected { identity, .. }
                if identity.as_deref() == Some("shared")),
            "{state:?}"
        );
        assert_eq!(
            recorded_identity(&fixture.store, second).await,
            Some("shared".to_string()),
            "the surviving entry becomes the host's entry, identity and all"
        );
    }

    /// A duplicate that STAYS one must keep re-checking the registry and
    /// must never dial again while it does.
    ///
    /// "Connects nothing while it stays one" is the whole content of the
    /// duplicate state, and it is a claim about the network, not about the
    /// state label: discovering the collision costs exactly one connection,
    /// and everything after that is a registry read. A re-probe that dialed
    /// would mean two entries holding connections to one host — the
    /// shown-once rule broken in the one place it is hardest to notice,
    /// since the extra connection is invisible in the state.
    ///
    /// The dial count is pinned across several re-probe intervals, which is
    /// what distinguishes "not dialing" from "not dialing yet".
    #[tokio::test(start_paused = true)]
    async fn an_unresolved_duplicate_rechecks_the_registry_without_redialing() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let first = store
                .add_ssh_host("owner.example", None, None)
                .await
                .unwrap();
            record_contact(&store, first, "shared").await;
            let second = store
                .add_ssh_host("owner.example.via-vpn", None, None)
                .await
                .unwrap();
            transport.set_script(
                first,
                Script {
                    reachable: false,
                    ..Script::default()
                },
            );
            transport.set_script(
                second,
                Script {
                    identity: Some("shared".to_string()),
                    ..Script::default()
                },
            );
        })
        .await;
        let second = fixture.store.list_hosts().await.unwrap()[2].id;
        fixture
            .manager
            .wait_for_state(second, |state| matches!(state, HostState::Duplicate { .. }))
            .await
            .expect("actor is running");
        assert_eq!(
            fixture.transport.attempts(second).len(),
            1,
            "learning the identity costs exactly one connection"
        );

        tokio::time::advance(REPROBE_INTERVAL * 5).await;
        tokio::task::yield_now().await;

        assert_eq!(
            fixture.transport.attempts(second).len(),
            1,
            "a duplicate must re-ask the REGISTRY, never the host"
        );
        assert!(
            matches!(
                fixture.manager.state(second),
                Some(HostState::Duplicate { .. })
            ),
            "and it must still be a duplicate, since the twin is still there"
        );
        assert!(
            status_client(&fixture.manager, second).is_none(),
            "a duplicate holds no connection at any point in that loop"
        );
    }

    /// A duplicate resolved by EDITING it rather than by removing its twin:
    /// the twin stays exactly where it is, and the duplicate entry is
    /// pointed somewhere else.
    ///
    /// This is the other half of "edit it or remove it", and it was the
    /// broken half: a duplicate actor re-evaluated its freeze before
    /// reloading its row, so an edited duplicate went on re-checking a
    /// question about the destination it no longer pointed at, forever,
    /// while its twin lived. Reloading the row first — and treating the
    /// edit as a reconfiguration that clears the freeze — is what makes the
    /// resolution the user was offered actually work.
    ///
    /// The scripted peer's identity is changed together with the
    /// destination, which is what "this address is a different machine"
    /// means for a transport keyed by row rather than by address.
    #[tokio::test(start_paused = true)]
    async fn editing_a_duplicates_destination_resolves_it_while_the_twin_lives() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let first = store
                .add_ssh_host("host.example", None, None)
                .await
                .unwrap();
            record_contact(&store, first, "shared").await;
            let second = store
                .add_ssh_host("host.example.via-vpn", None, None)
                .await
                .unwrap();
            // The twin is up and stays up for the whole test: the point is
            // that the duplicate resolves WITHOUT it going away.
            transport.set_script(
                first,
                Script {
                    identity: Some("shared".to_string()),
                    ..Script::default()
                },
            );
            transport.set_script(
                second,
                Script {
                    identity: Some("shared".to_string()),
                    ..Script::default()
                },
            );
        })
        .await;
        let rows = fixture.store.list_hosts().await.unwrap();
        let (first, second) = (rows[1].id, rows[2].id);
        fixture
            .manager
            .wait_for_state(second, |state| matches!(state, HostState::Duplicate { .. }))
            .await
            .expect("actor is running");

        fixture.transport.edit(second, |script| {
            script.identity = Some("its-own-identity".to_string())
        });
        fixture
            .store
            .update_ssh_destination(second, "the-other-machine.example")
            .await
            .expect("point the duplicate somewhere else");
        fixture.manager.sync_registry().await.unwrap();

        let state = fixture
            .manager
            .wait_for_state(second, HostState::is_connected)
            .await
            .expect("actor is running");
        assert!(
            matches!(&state, HostState::Connected { identity, .. }
                if identity.as_deref() == Some("its-own-identity")),
            "the edited entry must connect to its new destination: {state:?}"
        );
        assert_eq!(
            recorded_identity(&fixture.store, first).await,
            Some("shared".to_string()),
            "the twin must be entirely undisturbed by the other entry's edit"
        );
        assert!(
            fixture
                .manager
                .state(first)
                .is_some_and(|state| state.is_connected()),
            "the twin must still be connected"
        );
    }

    // ---- Session refresh ----------------------------------------------

    /// A refresh drains the supervisor's pagination to exhaustion and
    /// replaces the whole cache slice — PLAN_M6.md item 5's
    /// drain-then-replace, with the walk really crossing page boundaries
    /// rather than fitting in one.
    #[tokio::test(start_paused = true)]
    async fn a_refresh_drains_every_page_into_the_cache() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("busy.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    // Five sessions in pages of two: three pages, the last
                    // one short, so both the "follow the cursor" and the
                    // "stop when it is absent" halves are exercised.
                    sessions: (0..5).map(|n| session(&format!("s{n}"), 100 - n)).collect(),
                    page_size: 2,
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;

        let state = fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Ok { .. },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");
        assert!(
            matches!(
                state,
                HostState::Connected {
                    last_refresh: RefreshHealth::Ok { sessions: 5 },
                    ..
                }
            ),
            "{state:?}"
        );
        let ids: Vec<String> = fixture
            .store
            .cached_sessions(host)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(
            ids,
            vec!["s0", "s1", "s2", "s3", "s4"],
            "every page must land in the cache, in the wire order"
        );
    }

    /// A failed refresh must NOT wipe the cache.
    ///
    /// This is the difference between "we could not reach the list just
    /// now" and "this host genuinely has no sessions", and the stale list
    /// is exactly what a user falls back to when a host is having trouble
    /// — so wiping on failure would destroy the data at the moment it
    /// becomes the only data there is. The failure modelled here is the
    /// real one PLAN_M6.md names: the supervisor's pagination `Internal`
    /// refusal for a record too large to ship.
    #[tokio::test(start_paused = true)]
    async fn a_failed_refresh_keeps_the_previous_cache() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store.add_ssh_host("fat.example", None, None).await.unwrap();
            transport.set_script(
                host,
                Script {
                    sessions: vec![session("kept-a", 200), session("kept-b", 100)],
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Ok { .. },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");

        fixture.transport.edit(host, |script| {
            script.list_error = Some("a session record is too large to send".to_string());
        });

        let state = fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Failed { .. },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");
        assert!(
            matches!(&state, HostState::Connected { last_refresh: RefreshHealth::Failed { error }, .. }
                if error.contains("too large")),
            "the refusal's own words must survive into the state: {state:?}"
        );
        assert!(
            state.is_connected(),
            "a failed refresh is not a lost connection"
        );
        let ids: Vec<String> = fixture
            .store
            .cached_sessions(host)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(
            ids,
            vec!["kept-a", "kept-b"],
            "the previous cache must survive a failed refresh untouched"
        );
    }

    /// The race the store's identity-bound cache write was built for, run
    /// end to end through the manager: a refresh produced under an
    /// identity that has since been superseded must be REFUSED, not
    /// allowed to repopulate the cache an adoption just purged — and the
    /// CONNECTION that produced it must then go.
    ///
    /// The teardown half is the regression this test exists for. A
    /// connection whose identity is no longer the row's cannot ever cache
    /// again: every refresh it makes will be refused for the same reason,
    /// so leaving it up produces a host that reads as healthily connected
    /// while its stale list silently stops advancing, forever. Dropping it
    /// re-asks the identity question against the row as it now stands,
    /// which here is a genuine mismatch (the peer still reports the
    /// superseded identity) and surfaces as the freeze a user can act on.
    ///
    /// Driving it through the manager rather than the store alone is the
    /// point — the store's own test proves the check exists, this one
    /// proves the connection actor is actually subject to it, which is
    /// where a real deployment would find out otherwise.
    #[tokio::test(start_paused = true)]
    async fn a_refresh_under_a_superseded_identity_tears_the_connection_down() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("adopted.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    identity: Some("identity-old".to_string()),
                    sessions: vec![session("stale", 100)],
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Ok { .. },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");
        assert_eq!(fixture.store.cached_sessions(host).await.unwrap().len(), 1);

        // The user adopts a new identity while the connection above is
        // still live and still refreshing under the old one. The purge is
        // part of that same transaction.
        let row = fixture.store.list_hosts().await.unwrap()[1].clone();
        fixture
            .store
            .adopt_identity(host, &DialedAs::of(&row), "identity-old", "identity-new")
            .await
            .unwrap();
        assert!(
            fixture
                .store
                .cached_sessions(host)
                .await
                .unwrap()
                .is_empty()
        );

        // The refused write ends the connection, and the reconnection
        // behind it re-asks the identity question — which the peer answers
        // with the identity the user just superseded, so the host lands in
        // the mismatch freeze rather than back in a doomed Connected.
        // Waiting on THAT rather than on the intervening failed-refresh
        // state is deliberate: the failure is published, but a `watch`
        // keeps only the newest value, so asserting on an intermediate one
        // would be asserting on a race.
        let state = fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(state, HostState::IdentityMismatch { .. })
            })
            .await
            .expect("actor is running");
        assert_eq!(
            state,
            HostState::IdentityMismatch {
                recorded: "identity-new".to_string(),
                reported: "identity-old".to_string(),
            },
            "the reconnection must surface the mismatch the adoption created"
        );
        assert!(
            status_client(&fixture.manager, host).is_none(),
            "the connection that can no longer cache must not stay routable"
        );
        assert!(
            fixture
                .store
                .cached_sessions(host)
                .await
                .unwrap()
                .is_empty(),
            "the superseded identity's refresh must not repopulate the purged cache"
        );
    }

    /// A supervisor that reports no identity at all — a construction with
    /// no standing to mint one, not an old build, which the protocol
    /// version gate refuses before identity is ever discussed — is still
    /// usable: it connects and its list still walks, but it writes no
    /// cache, because there is nothing to bind a cache write to.
    ///
    /// Worth pinning because the tempting alternative, synthesizing an
    /// identity so the cache "works", would make every such host look
    /// permanently identified under a value the host itself has never
    /// heard of.
    #[tokio::test(start_paused = true)]
    async fn a_host_reporting_no_identity_connects_but_caches_nothing() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("ancient.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    identity: None,
                    sessions: vec![session("a", 100)],
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;

        let state = fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Ok { .. },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");
        assert!(
            matches!(&state, HostState::Connected { identity: None, .. }),
            "{state:?}"
        );
        assert_eq!(recorded_identity(&fixture.store, host).await, None);
        assert!(
            fixture
                .store
                .cached_sessions(host)
                .await
                .unwrap()
                .is_empty(),
            "an unidentified host must not write cache rows nothing could ever validate"
        );
    }

    /// A connected host that stops answering `ListSessions` — without
    /// closing anything — must not park the actor.
    ///
    /// The nastiest of the stalls, because every layer below looks healthy:
    /// the transport is open, the peer is reading, nothing errors, and the
    /// host reads as connected while its cached list quietly stops
    /// advancing forever. Expiry drops the connection so the ordinary loss
    /// path runs, and the previous cache — the whole reason the cache
    /// exists — is left exactly as it was.
    #[tokio::test(start_paused = true)]
    async fn a_host_that_stops_answering_its_list_is_dropped_and_keeps_its_cache() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("mute-list.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    sessions: vec![session("kept", 100)],
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Ok { .. },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");

        // The peer goes silent: it still reads, it still holds the
        // connection open, it simply never answers again.
        fixture
            .transport
            .edit(host, |script| script.silent_list = true);

        // The connection is dropped at the refresh deadline and the actor
        // re-enters its connect path, which dials again.
        fixture.transport.wait_for_closures(host, 1).await;
        let attempts = fixture.transport.wait_for_attempts(host, 2).await;
        assert_eq!(
            seconds(&attempts)[1] - seconds(&attempts)[0],
            REFRESH_TIMEOUT.as_secs() + REFRESH_INTERVAL.as_secs(),
            "the redial must follow the refresh deadline, not some other clock"
        );
        let ids: Vec<String> = fixture
            .store
            .cached_sessions(host)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(
            ids,
            vec!["kept"],
            "a host that went silent must keep the list it last gave us"
        );
    }

    /// A connection lost MID-WALK, between two pages, must leave the whole
    /// previous cache intact — not the pages that happened to arrive.
    ///
    /// The wholesale-replacement contract makes this a real risk rather
    /// than a theoretical one: a walk that wrote what it had so far would
    /// replace a complete list with a partial one, and the resulting stale
    /// view would be missing exactly the sessions that were listed last.
    #[tokio::test(start_paused = true)]
    async fn a_connection_lost_mid_walk_keeps_the_whole_previous_cache() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store.add_ssh_host("cut.example", None, None).await.unwrap();
            transport.set_script(
                host,
                Script {
                    sessions: vec![session("a", 300), session("b", 200), session("c", 100)],
                    page_size: 1,
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Ok { .. },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");

        // From here on the peer serves ONE page and then dies, so every
        // later walk is cut after its first page — with two more pages
        // still owed.
        fixture
            .transport
            .edit(host, |script| script.close_after_pages = Some(1));

        fixture.transport.wait_for_attempts(host, 2).await;
        fixture
            .manager
            .wait_for_state(host, |state| !state.is_connected())
            .await
            .expect("actor is running");
        let ids: Vec<String> = fixture
            .store
            .cached_sessions(host)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(
            ids,
            vec!["a", "b", "c"],
            "a walk cut between pages must leave the previous cache whole, not truncated to \
             what arrived"
        );
    }

    /// A list that grows past [`REFRESH_SESSION_CAP`] is a refused refresh
    /// like any other: the cache the host last gave us survives.
    ///
    /// Pinned through the actor rather than only at [`drain_sessions`]
    /// because the bound's WHOLE value is what it does to the connection —
    /// a walk that gave up but then wiped the cache, or dropped the
    /// connection, would satisfy the drain's own test and still be wrong
    /// here.
    #[tokio::test(start_paused = true)]
    async fn a_refresh_past_the_session_ceiling_keeps_the_previous_cache() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("huge.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    sessions: vec![session("kept", 100)],
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Ok { .. },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");

        // One session past the ceiling, served in the supervisor's own
        // default page size — i.e. eleven near-budget pages, the realistic
        // shape rather than one implausible mega-page.
        fixture.transport.edit(host, |script| {
            script.sessions = (0..=REFRESH_SESSION_CAP)
                .map(|n| session(&format!("many-{n}"), 100))
                .collect();
            script.page_size = 500;
        });

        let state = fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Failed { .. },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");
        assert!(
            matches!(&state, HostState::Connected { last_refresh: RefreshHealth::Failed { error }, .. }
                if error.contains(&REFRESH_SESSION_CAP.to_string())),
            "the refusal must name the ceiling: {state:?}"
        );
        assert!(
            state.is_connected(),
            "exceeding the ceiling is a failed refresh, not a lost connection"
        );
        let ids: Vec<String> = fixture
            .store
            .cached_sessions(host)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, vec!["kept"], "the previous cache must survive intact");
    }

    /// A live host's list is replaced WHOLESALE on the next refresh tick,
    /// and not before it.
    ///
    /// Two claims, and the second is the one that needs a paused clock: a
    /// cache that updated the moment the far side changed would mean
    /// something other than the refresh cadence is writing it, and a cache
    /// that never dropped the sessions missing from the new list would mean
    /// the replacement is really an upsert. Sessions that VANISH are what
    /// separates the two.
    #[tokio::test(start_paused = true)]
    async fn a_periodic_refresh_replaces_the_cached_list_wholesale() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("churn.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    sessions: vec![session("gone-soon", 200), session("stays", 100)],
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Ok { sessions: 2 },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");

        fixture.transport.edit(host, |script| {
            script.sessions = vec![session("stays", 100), session("brand-new", 50)]
        });
        assert_eq!(
            cached_ids(&fixture.store, host).await,
            vec!["gone-soon", "stays"],
            "the cache must not change until a refresh actually runs"
        );

        // The tick lands at the refresh cadence; waiting for the CONTENT
        // rather than for a state is what makes this immune to which
        // publish happened to be observed (both lists are two entries long,
        // so the states are indistinguishable).
        let deadline = tokio::time::Instant::now() + REFRESH_INTERVAL * 4;
        loop {
            if cached_ids(&fixture.store, host).await == vec!["stays", "brand-new"] {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the next refresh tick must replace the whole list, got {:?}",
                cached_ids(&fixture.store, host).await
            );
            tokio::time::sleep(REFRESH_INTERVAL / 2).await;
        }
    }

    /// Every page of a walk must be requested with NO limit, and with a
    /// cursor that actually advances.
    ///
    /// Both are contracts about what goes OUT, which no state assertion can
    /// see. The limit matters beyond tidiness: the supervisor's
    /// conversation-capture sweep rides its `ListSessions` handler, so it
    /// runs once per page — a helm that asked for small pages would
    /// multiply whole-host scans on every host on every refresh (see
    /// [`drain_sessions`]).
    #[tokio::test(start_paused = true)]
    async fn the_drain_asks_for_unlimited_pages_and_advances_its_cursor() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("paged.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    sessions: (0..5).map(|n| session(&format!("s{n}"), 100 - n)).collect(),
                    page_size: 2,
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Ok { sessions: 5 },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");

        let requests = fixture.transport.requests(host);
        assert_eq!(
            requests[..3]
                .iter()
                .map(|r| r.cursor.as_deref())
                .collect::<Vec<_>>(),
            vec![None, Some("2"), Some("4")],
            "one walk must follow the peer's cursors forward: {requests:?}"
        );
        assert!(
            requests.iter().all(|request| request.limit.is_none()),
            "every page must be requested without a limit: {requests:?}"
        );
    }

    // ---- Registry reconciliation and the local row ---------------------

    /// `sync_registry` is the primary reconciliation path — the one a
    /// caller invokes after any registry change — and it must decide which
    /// actors exist: a newly added host gains one, a removed host loses
    /// one, and snapshots come back in registry order with the local row
    /// first. (An actor can also retire itself when it finds its own row
    /// gone, which is the backstop `HostActor::run` documents; that path is
    /// not what this pins.)
    #[tokio::test(start_paused = true)]
    async fn sync_registry_starts_and_stops_actors_to_match_the_registry() {
        let fixture = fixture(Cadence::default(), |_store, transport| async move {
            transport.set_script(
                1,
                Script {
                    reachable: false,
                    ..Script::default()
                },
            );
        })
        .await;
        let local = local_id(&fixture.store).await;
        assert_eq!(
            fixture
                .manager
                .snapshots()
                .into_iter()
                .map(|s| (s.id, s.kind, s.destination))
                .collect::<Vec<_>>(),
            vec![(local, HostKind::Local, None)],
            "a fresh registry has exactly the reserved local row"
        );

        let added = fixture
            .store
            .add_ssh_host("added.example", None, None)
            .await
            .unwrap();
        fixture.transport.set_script(
            added,
            Script {
                reachable: false,
                ..Script::default()
            },
        );
        fixture.manager.sync_registry().await.unwrap();
        assert_eq!(
            fixture
                .manager
                .snapshots()
                .into_iter()
                .map(|s| (s.id, s.destination))
                .collect::<Vec<_>>(),
            vec![(local, None), (added, Some("added.example".to_string()))]
        );

        fixture.store.remove_ssh_host(added).await.unwrap();
        fixture.manager.sync_registry().await.unwrap();
        assert!(fixture.manager.state(added).is_none());
        assert_eq!(fixture.manager.snapshots().len(), 1);
    }

    /// The local row with no supervisor running must be honestly
    /// distinguished from a generic unreachable host, because it is the
    /// one case whose remedy is a command on the machine the user is
    /// already sitting at (PLAN_M6.md: a manual-path hint, never an offer
    /// to install — provisioning is M7's).
    ///
    /// Runs against the REAL [`SystemTransport`] and a genuinely empty
    /// state directory, not a scripted peer: the whole claim is about how
    /// a real unix-socket dial failure is classified, which a script could
    /// only assert about itself.
    #[tokio::test]
    async fn the_local_row_with_no_supervisor_says_so() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = HelmStore::open(&dir.path().join("helm.db"))
            .await
            .expect("open store");
        let manager = ConnectionManager::start(
            store.clone(),
            Arc::new(SystemTransport::new(dir.path())),
            Cadence {
                // No ladder and a short re-probe: this test is about the
                // classification, and there is nothing to wait for. The
                // deadlines stay at their production values — nothing here
                // is supposed to reach them, and a short one would only be
                // able to mask the classification with a timeout.
                connect_backoff: Vec::new(),
                reprobe: Duration::from_millis(50),
                refresh: Duration::from_millis(50),
                ..Cadence::default()
            },
        )
        .await
        .expect("start manager");
        let local = local_id(&store).await;

        let state = tokio::time::timeout(
            Duration::from_secs(10),
            manager.wait_for_state(local, |state| {
                matches!(state, HostState::Unreachable { .. })
            }),
        )
        .await
        .expect("the local row must reach unreachable promptly")
        .expect("actor is running");
        assert!(
            matches!(
                &state,
                HostState::Unreachable {
                    cause: UnreachableCause::LocalSupervisorNotRunning,
                    ..
                }
            ),
            "an absent local socket must be classified as the manual-start case: {state:?}"
        );
    }

    /// `sync_registry`'s reconciliation must reach the ACTORS, not just the
    /// map they are listed in.
    ///
    /// The map-level version of this claim (an entry appears, an entry
    /// disappears) is already pinned above, and it is the weaker half: an
    /// added actor that never dialed and a removed actor that kept dialing
    /// would both satisfy it. This one watches the transport instead — the
    /// added host is really connected to, and the removed host's peer
    /// really sees its connection close and is never dialed again.
    #[tokio::test(start_paused = true)]
    async fn sync_registry_starts_and_stops_the_actors_work_not_just_their_entries() {
        let fixture = fixture(Cadence::default(), |_store, _transport| async {}).await;

        let added = fixture
            .store
            .add_ssh_host("arrives.example", None, None)
            .await
            .unwrap();
        fixture.transport.set_script(added, Script::default());
        fixture.manager.sync_registry().await.unwrap();

        fixture
            .manager
            .wait_for_state(added, HostState::is_connected)
            .await
            .expect("the added host must get a working actor");
        let dials_while_registered = fixture.transport.attempts(added).len();

        fixture.store.remove_ssh_host(added).await.unwrap();
        fixture.manager.sync_registry().await.unwrap();

        // The far side observes the close — the only evidence that the
        // connection went away rather than merely being forgotten about.
        fixture.transport.wait_for_closures(added, 1).await;
        assert!(fixture.manager.state(added).is_none());
        // Well past several re-probe intervals, nothing further is dialed.
        tokio::time::advance(REPROBE_INTERVAL * 3).await;
        tokio::task::yield_now().await;
        assert_eq!(
            fixture.transport.attempts(added).len(),
            dials_while_registered,
            "a removed host's actor must stop attempting, not merely stop being listed"
        );
    }

    /// Shutting the manager down must stop every actor and drop every
    /// connection — and dropping the manager must do the same, since that
    /// is what the desktop app's teardown and every test's scope exit
    /// actually do.
    ///
    /// Actors are `tokio::spawn`ed, so nothing about going out of scope
    /// stops them on its own: without the explicit teardown they would keep
    /// dialing, keep writing to a store nobody reads, and keep holding
    /// connections open against hosts that have no manager left to serve.
    #[tokio::test(start_paused = true)]
    async fn dropping_the_manager_stops_every_actor_and_closes_its_connections() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("shutdown.example", None, None)
                .await
                .unwrap();
            transport.set_script(host, Script::default());
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");
        let dials_while_running = fixture.transport.attempts(host).len();

        let Fixture {
            manager,
            transport,
            _dir,
            store,
        } = fixture;
        drop(manager);

        transport.wait_for_closures(host, 1).await;
        tokio::time::advance(REPROBE_INTERVAL * 3).await;
        tokio::task::yield_now().await;
        assert_eq!(
            transport.attempts(host).len(),
            dials_while_running,
            "a dropped manager must leave no actor still dialing"
        );
        // Held to the end: dropping the store's tempdir out from under a
        // still-running actor would be a different failure than the one
        // this test is about.
        drop(store);
    }

    /// An actor whose task DIES must not leave its last published status
    /// standing — least of all a `Connected` one with a live-looking
    /// client.
    ///
    /// Nothing else watches a spawned task, so without supervision a
    /// panicked actor leaves the entry claiming a connection that no longer
    /// has anyone driving it: session operations route onto it, the hosts
    /// list shows it as healthy, and no timer ever corrects either, because
    /// every mechanism that would is inside the task that died. The
    /// retired state is a worse-looking answer and a truthful one.
    #[tokio::test(start_paused = true)]
    async fn a_panicking_actor_is_retired_rather_than_left_claiming_its_host() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("doomed.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    panic_on_dial: true,
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;

        let state = fixture
            .manager
            .wait_for_state(host, |state| matches!(state, HostState::Retired { .. }))
            .await
            .expect("the actor's supervisor must publish for it");
        assert!(
            matches!(&state, HostState::Retired { reason } if reason.contains("panicked")),
            "the reason must say what happened: {state:?}"
        );
        assert!(
            status_client(&fixture.manager, host).is_none(),
            "a host with no actor must not stay routable"
        );
        assert!(
            !state.is_connected(),
            "a retired entry is not a connected one"
        );
        // Nothing restarts it on a timer: an actor is not something a
        // re-probe can bring back.
        let dials = fixture.transport.attempts(host).len();
        tokio::time::advance(REPROBE_INTERVAL * 4).await;
        tokio::task::yield_now().await;
        assert_eq!(fixture.transport.attempts(host).len(), dials);
    }

    /// The status pair — state and client — must be coherent at EVERY
    /// instant, including the instants inside a transition.
    ///
    /// The pairing is the contract session routing is built on: `Some`
    /// client exactly while `Connected`. Read from two separate borrows it
    /// would hold only between transitions, and a caller sampling across
    /// one could see a `Connected` state beside a `None` client (refusing
    /// an operation against a live host) or a live client beside a
    /// non-connected state (routing onto a corpse). This samples hard
    /// across a real connection loss and a real reconnection and asserts
    /// the invariant on every sample.
    #[tokio::test(start_paused = true)]
    async fn the_status_pair_is_coherent_across_a_transition() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("flapping.example", None, None)
                .await
                .unwrap();
            transport.set_script(host, Script::default());
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");

        // Kill the connection and sample continuously while the actor
        // notices, publishes, redials and reconnects.
        fixture.transport.kill_connections();
        let mut saw_connected = false;
        let mut saw_disconnected = false;
        for _ in 0..500 {
            let status = fixture.manager.status(host).expect("actor is running");
            assert_eq!(
                status.client.is_some(),
                status.state.is_connected(),
                "a client must be published exactly while the state is connected: {:?}",
                status.state
            );
            if status.state.is_connected() {
                saw_connected = true;
            } else {
                saw_disconnected = true;
            }
            tokio::task::yield_now().await;
        }
        assert!(
            saw_connected && saw_disconnected,
            "the sampling must actually straddle the transition (connected: {saw_connected}, \
             disconnected: {saw_disconnected})"
        );
    }

    /// A shutdown must be TERMINAL against a reconciliation that is already
    /// in flight.
    ///
    /// `sync_registry` reads the registry before it touches the actor map,
    /// and that read is an await — so a shutdown can land in the middle of
    /// it, drain the map, and then have the reconcile repopulate it from
    /// rows it read while the manager was still alive. The actors that
    /// creates are unreachable and unstoppable: the map they would be
    /// stopped through has already been drained, and the manager they
    /// belong to is on its way out.
    ///
    /// The `yield_now` is the barrier: it lets the spawned reconcile get as
    /// far as its store read before the shutdown runs, which is exactly the
    /// interleaving that used to lose.
    #[tokio::test(start_paused = true)]
    async fn a_shutdown_beats_a_reconciliation_that_is_already_running() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("racing.example", None, None)
                .await
                .unwrap();
            transport.set_script(host, Script::default());
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");
        let dials = fixture.transport.attempts(host).len();

        let manager = Arc::clone(&fixture.manager);
        let reconciling = tokio::spawn(async move { manager.sync_registry().await });
        tokio::task::yield_now().await;
        fixture.manager.shutdown();
        reconciling
            .await
            .expect("the reconcile task must not panic")
            .expect("a reconcile after shutdown is a no-op, not an error");

        assert!(
            fixture.manager.snapshots().is_empty(),
            "a reconcile that lost to a shutdown must not repopulate the map"
        );
        // And a plain call afterwards is a no-op too, for the same reason.
        fixture.manager.sync_registry().await.unwrap();
        assert!(fixture.manager.snapshots().is_empty());
        tokio::time::advance(REPROBE_INTERVAL * 3).await;
        tokio::task::yield_now().await;
        assert_eq!(
            fixture.transport.attempts(host).len(),
            dials,
            "no resurrected actor may still be dialing"
        );
    }

    /// A peer's own error text is BOUNDED and ESCAPED before it is logged
    /// or retained, and a peer that repeats itself cannot write the log.
    ///
    /// The wire's frame cap is measured in megabytes and a connected host
    /// refreshes every few seconds, so an unfiltered `Error.message` is
    /// three problems at once: a per-host retention leak (the state holds
    /// it until the next refresh), a log flood, and — since a log line
    /// lands in an operator's terminal emulator — a way for a remote party
    /// to emit escape sequences into it.
    #[tokio::test(start_paused = true)]
    async fn a_hostile_error_message_is_bounded_escaped_and_not_repeated() {
        let captured = crate::test_capture::install();
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("shouty.example", None, None)
                .await
                .unwrap();
            transport.set_script(
                host,
                Script {
                    // Screen-clearing escapes, then far more text than any
                    // diagnostic could need.
                    list_error: Some(format!("\u{1b}[2J\u{1b}[H{}", "x".repeat(200_000))),
                    ..Script::default()
                },
            );
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;

        let state = fixture
            .manager
            .wait_for_state(host, |state| {
                matches!(
                    state,
                    HostState::Connected {
                        last_refresh: RefreshHealth::Failed { .. },
                        ..
                    }
                )
            })
            .await
            .expect("actor is running");
        let HostState::Connected {
            last_refresh: RefreshHealth::Failed { error },
            ..
        } = &state
        else {
            unreachable!("filtered on a failed refresh");
        };
        assert!(
            error.len() < PEER_TEXT_CAP * 2,
            "the retained failure must be bounded, got {} bytes",
            error.len()
        );
        assert!(
            !error.contains('\u{1b}'),
            "no raw escape byte may survive into the retained state: {error:?}"
        );
        assert!(
            error.contains("truncated"),
            "a cut message must say so: {error:?}"
        );

        // Many more failing refreshes, all identical: the log must not
        // grow with them.
        tokio::time::advance(REFRESH_INTERVAL * 20).await;
        tokio::task::yield_now().await;
        let logged = crate::test_capture::matching(&captured, "refreshing the host's session list")
            .into_iter()
            .filter(|event| event.field("destination") == Some("shouty.example"))
            .count() as u64;
        assert!(
            logged <= REPEATED_FAILURE_LOG_LIMIT,
            "an identical failure must stop being logged, got {logged} lines"
        );
    }

    /// `peer_text`'s two jobs, at the unit level: bound the length, and
    /// escape what a terminal would otherwise act on.
    #[test]
    fn peer_text_bounds_and_escapes() {
        let short = peer_text("connection refused");
        assert!(
            short.contains("connection refused") && !short.contains("truncated"),
            "an ordinary message must survive intact: {short}"
        );

        let hostile = peer_text("\u{1b}[2Jgone\u{7}");
        assert!(
            !hostile.contains('\u{1b}') && !hostile.contains('\u{7}'),
            "control bytes must not survive: {hostile:?}"
        );
        assert!(hostile.contains("gone"), "the readable part must survive");

        let long = peer_text(&"x".repeat(PEER_TEXT_CAP * 4));
        assert!(long.len() < PEER_TEXT_CAP * 2, "must be bounded");
        assert!(long.contains("truncated"), "a cut must be marked");

        // A multi-byte character straddling the cap must not panic, and
        // must not produce invalid text.
        let multibyte = peer_text(&"é".repeat(PEER_TEXT_CAP));
        assert!(multibyte.contains("truncated"));
    }

    // ---- The diagnostic trail -----------------------------------------

    /// SPEC.md requires a reconnection trail, and a trail is only usable if
    /// every line says WHICH host it is about and WHERE that host is —
    /// including after the answer to "where" changes.
    ///
    /// Two failures this pins. The destination used to be a SPAN field
    /// fixed when the actor was created, so every line about a retargeted
    /// host named the address it no longer used — worst of all in the very
    /// lines describing the reconnection to its new one. And phase
    /// transitions must be logged only when the phase actually CHANGES: a
    /// connected host republishes its status on every refresh tick, and a
    /// line per tick would bury the handful that describe what happened to
    /// the host.
    ///
    /// The capture is process-global and shared with every other test in
    /// this binary (see [`crate::test_capture`]), so this filters on a
    /// destination no other test uses.
    #[tokio::test(start_paused = true)]
    async fn the_reconnection_trail_names_the_host_and_its_current_destination() {
        let captured = crate::test_capture::install();
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            let host = store
                .add_ssh_host("trail-before.example", None, None)
                .await
                .unwrap();
            transport.set_script(host, Script::default());
        })
        .await;
        let host = fixture.store.list_hosts().await.unwrap()[1].id;
        fixture
            .manager
            .wait_for_state(host, HostState::is_connected)
            .await
            .expect("actor is running");

        // Several refresh ticks, each of which republishes the connected
        // status without changing the phase.
        tokio::time::advance(REFRESH_INTERVAL * 5).await;
        tokio::task::yield_now().await;

        fixture
            .store
            .update_ssh_destination(host, "trail-after.example")
            .await
            .expect("retarget");
        fixture.manager.sync_registry().await.unwrap();
        fixture
            .manager
            .wait_for_state(host, |state| matches!(state, HostState::Connected { .. }))
            .await
            .expect("actor is running");

        let transitions = crate::test_capture::matching(&captured, "host connection phase changed");
        let ours: Vec<_> = transitions
            .iter()
            .filter(|event| {
                event
                    .field("destination")
                    .is_some_and(|destination| destination.starts_with("trail-"))
            })
            .collect();
        assert!(
            ours.iter().all(
                |event| event.field("host") == Some(host.to_string().as_str())
                    && event.field("kind") == Some("Ssh")
            ),
            "every line must carry the host span's context: {ours:?}"
        );
        let phases: Vec<(Option<&str>, Option<&str>)> = ours
            .iter()
            .map(|event| (event.field("from"), event.field("to")))
            .collect();
        assert_eq!(
            phases,
            vec![
                (Some("connecting"), Some("connected")),
                (Some("connecting"), Some("connected")),
            ],
            "one line per real phase change — not one per refresh tick: {ours:?}"
        );
        assert_eq!(
            ours.iter()
                .map(|event| event.field("destination"))
                .collect::<Vec<_>>(),
            vec![Some("trail-before.example"), Some("trail-after.example")],
            "the reconnection's own line must name the destination it is reconnecting TO, not \
             the one the actor was created with"
        );

        // The connected → connecting step of that trail is the EDIT's own
        // line, which is where it belongs: it is the only line that can
        // also say why the connection went away.
        let edits = crate::test_capture::matching(&captured, "connection settings changed");
        let ours: Vec<_> = edits
            .iter()
            .filter(|event| event.field("destination") == Some("trail-after.example"))
            .collect();
        assert_eq!(ours.len(), 1, "exactly one edit happened: {ours:?}");
        assert_eq!(
            (ours[0].field("from"), ours[0].field("to")),
            (Some("connected"), Some("connecting")),
            "the edit must record the transition it performed: {ours:?}"
        );
    }

    // ---- Classification, without a state machine around it -------------

    /// The local row's "no supervisor is running here" hint must be
    /// attached for exactly the two dial failures that mean it, and for no
    /// others — table-driven, because the failure modes are a set and a
    /// test that checked one of them would leave the rest to be discovered
    /// by a user being told to start a supervisor that is already running.
    ///
    /// Both a bare error and a CONTEXT-WRAPPED one are checked for each
    /// kind: the marker is attached with `.context(..)`, and recovering it
    /// requires anyhow's own downcast rather than a `chain()` walk — a
    /// distinction that compiles either way and silently answers "no" if
    /// gotten wrong (see [`failure`]'s own comment).
    #[test]
    fn local_dial_failures_are_classified_by_kind_including_wrapped_ones() {
        use std::io::ErrorKind;

        let local = HostRow {
            id: 1,
            kind: HostKind::Local,
            destination: None,
            remote_farhelm: None,
            remote_state_dir: None,
            host_identity: None,
        };
        let cases = [
            (
                ErrorKind::NotFound,
                UnreachableCause::LocalSupervisorNotRunning,
            ),
            (
                ErrorKind::ConnectionRefused,
                UnreachableCause::LocalSupervisorNotRunning,
            ),
            (
                ErrorKind::PermissionDenied,
                UnreachableCause::TransportFailure,
            ),
            (ErrorKind::NotADirectory, UnreachableCause::TransportFailure),
        ];
        for (kind, expected) in cases {
            for wrapped in [false, true] {
                let raw = anyhow::Error::new(std::io::Error::new(kind, "planted"));
                // The real dial path wraps its io error in context before
                // this ever sees it; both shapes must classify the same.
                let error = if wrapped {
                    raw.context("connecting to the local supervisor socket")
                } else {
                    raw
                };
                let outcome = failure(&local, classify_local_dial(error));
                let AttemptOutcome::Failed { cause, .. } = outcome else {
                    panic!("a dial failure must be a failed attempt");
                };
                assert_eq!(
                    cause, expected,
                    "{kind:?} (wrapped: {wrapped}) must classify as {expected:?}"
                );
            }
        }
    }

    /// The same classification must NOT fire for an ssh row, whatever the
    /// io error says: "start a supervisor on this machine" is advice about
    /// the wrong machine entirely when the host is remote.
    #[test]
    fn a_remote_rows_refused_dial_is_never_the_local_supervisor_hint() {
        let remote = HostRow {
            id: 2,
            kind: HostKind::Ssh,
            destination: Some("remote.example".to_string()),
            remote_farhelm: None,
            remote_state_dir: None,
            host_identity: None,
        };
        let error = classify_local_dial(anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "planted",
        )));
        let AttemptOutcome::Failed { cause, .. } = failure(&remote, error) else {
            panic!("a dial failure must be a failed attempt");
        };
        assert_eq!(cause, UnreachableCause::TransportFailure);
    }

    /// A version-skewed LOCAL row's remediation must name THIS machine.
    ///
    /// The local row has no destination, so the remote-shaped sentence
    /// ("update the farhelm binary on <dest>") would render as nonsense at
    /// best and as advice about a machine that does not exist at worst —
    /// and skew is precisely the state SPEC.md requires to carry an
    /// actionable remedy rather than a diagnosis.
    #[tokio::test(start_paused = true)]
    async fn a_skewed_local_row_is_told_to_update_this_machine() {
        let fixture = fixture(Cadence::default(), |store, transport| async move {
            transport.set_script(
                local_id(&store).await,
                Script {
                    protocol: PROTOCOL_VERSION - 1,
                    build: "0.0.1-old-local".to_string(),
                    ..Script::default()
                },
            );
        })
        .await;
        let local = local_id(&fixture.store).await;

        let state = fixture
            .manager
            .wait_for_state(local, |state| {
                matches!(state, HostState::VersionSkew { .. })
            })
            .await
            .expect("actor is running");
        let HostState::VersionSkew { remediation, .. } = &state else {
            unreachable!("filtered on VersionSkew");
        };
        assert!(
            remediation.contains("this machine") && !remediation.contains("<local>"),
            "the local row's remedy must name this machine in prose, not a placeholder \
             destination: {remediation:?}"
        );
    }

    // ---- The drain primitive on its own --------------------------------

    /// A peer whose session list never ends, written inline rather than via
    /// [`ScriptedTransport`] because an endless list is the one thing that
    /// transport cannot express.
    ///
    /// `sessions_per_page` and the cursor's shape are the two knobs the
    /// bounds below are separated by: a peer that advances its cursor
    /// forever is caught by a different guard than one that hands back the
    /// cursor it was given, and a peer serving full pages is caught by a
    /// different guard than one serving thin ones.
    async fn endless_list_client(
        sessions_per_page: usize,
        repeat_the_cursor: bool,
    ) -> Arc<SupervisorClient> {
        let (ours, theirs) = tokio::io::duplex(1 << 20);
        tokio::spawn(async move {
            let (r, w) = tokio::io::split(theirs);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            let _ = writer.write_control(&ControlMsg::hello("supervisor")).await;
            let _ = reader.read_frame().await;
            let mut issued: u64 = 0;
            while let Ok(Some(frame)) = reader.read_frame().await {
                let Ok(ControlMsg::ListSessions { req_id, cursor, .. }) = parse_control(&frame)
                else {
                    continue;
                };
                issued += 1;
                let next = if repeat_the_cursor {
                    // Echoing the cursor back is the accidental
                    // non-termination this models: a supervisor that
                    // forgets to advance its own resume point.
                    cursor.unwrap_or_else(|| "0".to_string())
                } else {
                    issued.to_string()
                };
                let reply = ControlMsg::SessionList {
                    req_id,
                    sessions: (0..sessions_per_page)
                        .map(|n| session(&format!("s{issued}-{n}"), 1))
                        .collect(),
                    total: u64::MAX,
                    // Never absent: the peer claims there is always more.
                    next_cursor: Some(next),
                };
                if writer.write_control(&reply).await.is_err() {
                    return;
                }
            }
        });
        let (r, w) = tokio::io::split(ours);
        SupervisorClient::start(r, w).await.expect("client")
    }

    /// [`drain_sessions`] must terminate against a peer that keeps issuing
    /// FRESH cursors, rather than following them forever.
    ///
    /// One session per page, so the walk is stopped by the page ceiling
    /// long before it has accumulated enough entries for the session cap to
    /// have anything to say — which is exactly the shape that bound exists
    /// for, and the shape a count-based bound alone would never catch.
    ///
    /// Pinned directly rather than through an actor because the bound is a
    /// safety property, not a behavior: it should hold for every caller
    /// this function ever gains, including ones that do not exist yet.
    #[tokio::test]
    async fn draining_a_peer_that_never_exhausts_its_cursor_gives_up() {
        let client = endless_list_client(1, false).await;
        let error = drain_sessions(&client)
            .await
            .expect_err("an unterminating cursor must be given up on, not followed forever");
        assert!(error.to_string().contains("did not terminate"), "{error:#}");
    }

    /// The cheapest non-termination there is: a peer that hands back the
    /// cursor it was given. Both other bounds would eventually stop it, but
    /// only after following the same page hundreds of times and
    /// accumulating hundreds of copies of the same sessions — so the walk
    /// refuses the second use of one cursor outright, which is also the
    /// only one of the three bounds that can name the actual bug.
    #[tokio::test]
    async fn draining_a_peer_that_repeats_one_cursor_aborts_at_once() {
        let client = endless_list_client(1, true).await;
        let error = drain_sessions(&client)
            .await
            .expect_err("a repeated cursor must abort the walk");
        assert!(
            error.to_string().contains("same cursor"),
            "the error must name what the peer did: {error:#}"
        );
    }

    /// The memory half of the bound: full pages with advancing cursors stay
    /// under the page ceiling indefinitely while the collected `Vec` grows
    /// without limit, so only the session cap ends it.
    ///
    /// Pages of 500 are the supervisor's own default cap
    /// (`LIST_SESSION_CAP`), i.e. the largest page a real host ever sends —
    /// the walk must give up a little past ten of them, not after some
    /// number of round trips that happens to be small.
    #[tokio::test]
    async fn draining_a_peer_serving_full_pages_forever_stops_at_the_session_cap() {
        let client = endless_list_client(500, false).await;
        let error = drain_sessions(&client)
            .await
            .expect_err("an endless list must be given up on before it exhausts memory");
        assert!(
            error.to_string().contains(&REFRESH_SESSION_CAP.to_string()),
            "the error must name the ceiling it hit: {error:#}"
        );
    }
}
