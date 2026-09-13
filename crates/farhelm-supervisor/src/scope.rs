//! Per-launch systemd transient scopes: the Linux cgroup hardening that
//! sits ON TOP of M2's process-tree sweep (PLAN_M3.md item 10).
//!
//! # What this buys — and the guarantee it does NOT make
//!
//! The sweep in `service::kill_process_tree` finds a session's descendants
//! two ways — the pane's PPID closure and a scan of `/proc/*/environ` for
//! `FARHELM_SESSION_ID` — and lore/2026-07-27-m2-process-tree-stop.md
//! records the one shape both miss: a descendant that double-forked (so no
//! PPID walk reaches it) AND `exec`'d with a scrubbed environment (so the
//! marker scan cannot see it either). That shape is what this closes, and
//! it is the shape ACCIDENTAL daemonization produces: a dev server, an MCP
//! server, a build watcher. Cgroup membership is inherited across fork and
//! exec by the kernel, so a process that merely forgets its ancestry and
//! its environment is still in the cgroup and still dies with it.
//!
//! **The guarantee stops at accidental daemonization, and that boundary is
//! real rather than cautious wording.** A descendant that deliberately runs
//! `systemd-run --user --scope` on ITSELF migrates into a sibling unit
//! under the same user manager; with its marker also scrubbed it is then
//! invisible to the cgroup kill (wrong unit) and to the marker scan (no
//! marker) alike. This was reproduced during review, not theorized. Nothing
//! at this layer can prevent it: containing a process that can talk to the
//! user manager needs a DELEGATION boundary — a parent slice this
//! supervisor owns, with the session's units confined inside it and the
//! manager refusing migrations out — which v1 does not build and SPEC.md
//! does not promise. An agent's descendants run with the user's own
//! privileges by design (that is what the tool is FOR), so a descendant
//! determined to outlive its session can always arrange to; the honest
//! claim is that stop reaps what a normal program leaves behind, not that
//! it contains an adversary.
//!
//! It is also hardening, never a replacement. SPEC_impl.md's
//! belt-and-suspenders rule stands: the sweep runs AFTER every scope kill,
//! and where no user manager exists (CI containers, and any host whose user
//! manager is broken) the sweep is the whole mechanism, exactly as it was
//! in M2. Nothing here is allowed to make stop weaker than M2's guarantee.
//!
//! # Why the wrapper goes where it goes
//!
//! `systemd-run --user --scope` REPLACES ITSELF with the command it is
//! given. Verified empirically on systemd 255, and the audit claim is
//! exactly this and no more: the launched process is a direct child of the
//! invoking shell with no `systemd-run` left in the tree, the command's
//! exit status propagates unchanged, and the working directory and
//! environment are inherited — which is what the three observers that read
//! those facts need (`pane_process` liveness, `pane_dead_status` exit
//! codes, and the sweep's PPID closure). It is NOT a claim that a scoped
//! process is indistinguishable from an unscoped one in general: cgroup
//! membership is plainly visible from inside (`/proc/self/cgroup`), and
//! that visibility is exactly what makes the escape above possible.
//!
//! # Unit naming
//!
//! One scope per LAUNCH, named from the session id and its generation
//! ([`unit_name`]) for the same reason launch specs and sentinels are
//! (`store::StoredSession::generation`): the name of a past run's artifact
//! must never collide with the current run's. The name is DERIVED at every
//! use rather than stored, so nothing the database holds can aim a kill at
//! a unit that does not belong to the session being stopped; the store
//! records only the boolean selection
//! (`store::StoredSession::launch_scoped`).

use anyhow::Context;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// How long any single `systemd-run`/`systemctl` QUERY may take before this
/// module gives up on it and falls back.
///
/// These are D-Bus round trips to the user manager, normally a few
/// milliseconds. The bound exists for the pathological case — a wedged or
/// overloaded manager — where waiting indefinitely would convert a
/// hardening feature into a hang on the stop path a user is waiting on.
/// Timing out is never fatal: the probe reports "unavailable" and stop
/// falls through to the sweep it was always going to run anyway.
///
/// The LAUNCH wrapper is the deliberate exception and is not run through
/// here at all: `systemd-run --scope` `exec`s into the agent, so its
/// "invocation" lasts exactly as long as the session does. Bounding it
/// would mean killing the agent.
const SYSTEMCTL_TIMEOUT: Duration = Duration::from_secs(5);

/// Total time the availability probe may spend before reporting failure.
///
/// Covers the whole create → show → kill → gone sequence, so a manager that
/// accepts a unit and then stops answering cannot stall supervisor startup
/// (or a first create) indefinitely.
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Poll interval while waiting for a unit to appear or disappear.
const UNIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Prefix every farhelm-owned transient scope carries — the per-launch
/// units and the availability probe's own throwaway unit alike — so a unit
/// found on a host is recognizably ours.
const UNIT_PREFIX: &str = "farhelm-";

/// The transient scope unit name for one launch of one session, or `None`
/// for a session id that cannot safely name a unit.
///
/// Generation-scoped, so a stale unit from a previous run can never be
/// mistaken for the current one; see the module docs.
///
/// The `None` arm is a real invariant check, not defensive decoration. Any
/// mapping from arbitrary ids onto unit names is non-injective (systemd's
/// name charset is smaller than a string's), and two live sessions sharing
/// a unit name would mean one session's stop killing the other's agent. So
/// rather than sanitize, this REFUSES anything that is not a plain
/// lowercase hyphenated UUID — the shape `service::new_session_identity`
/// mints for every session — which is injective by construction. An id
/// outside that shape can only come from a hand-edited or foreign database;
/// it selects the fallback (the sweep alone, exactly M2) rather than a name
/// that might belong to somebody else.
pub fn unit_name(session_id: &str, generation: i64) -> Option<String> {
    is_uuid_shaped(session_id).then(|| format!("{UNIT_PREFIX}{session_id}-{generation}.scope"))
}

/// The transient scope unit name for one TERMINAL TAB (PLAN_M4.md item 2),
/// or `None` when either id cannot safely name a unit.
///
/// A tab's scope is keyed by (session, tab) rather than by generation,
/// because a tab is not a launch of the session: it has no generation
/// column, it survives the agent's own restarts, and its lifetime is
/// bounded by exactly one open/close pair. The tab id is minted per open
/// and never reused, which gives this name the same
/// unique-over-time property [`unit_name`]'s generation suffix gives
/// launches.
///
/// Both ids go through the same UUID-shape refusal `unit_name` documents,
/// and for the same reason — plus one this name has and that one does not:
/// a tab id is read back out of a tmux WINDOW MARKER, and any process that
/// inherited `TMUX` can write window options on the private server. A unit
/// name derived from an id this supervisor did not mint could aim a kill
/// at something else entirely, so an unrecognized shape selects the same
/// fallback a missing user manager does — the marker sweep alone.
///
/// The `-tab-` infix keeps this namespace disjoint from `unit_name`'s
/// `<session>-<generation>` shape without needing a second prefix: a
/// generation is always digits, so no launch unit can ever spell `tab`.
pub fn tab_unit_name(session_id: &str, tab_id: &str) -> Option<String> {
    (is_uuid_shaped(session_id) && is_uuid_shaped(tab_id))
        .then(|| format!("{UNIT_PREFIX}{session_id}-tab-{tab_id}.scope"))
}

/// The glob every one of a session's TAB scopes matches, or `None` for a
/// session id that cannot safely name a unit.
///
/// Exists so a teardown can find a session's tab scopes WITHOUT asking
/// tmux (`ScopeManager::units_matching`). That independence is the point:
/// a delete whose tmux server has already died would otherwise have no
/// tab ids to derive names from, and an environment-scrubbed tab daemon —
/// reachable only through its cgroup — would outlive a delete that
/// reported success. The manager knows its own units regardless of what
/// tmux is doing.
///
/// Anchored on the same `-tab-` infix [`tab_unit_name`] builds, so it can
/// never match a LAUNCH unit of the same session (a generation is always
/// digits).
pub fn tab_unit_glob(session_id: &str) -> Option<String> {
    is_uuid_shaped(session_id).then(|| format!("{UNIT_PREFIX}{session_id}-tab-*.scope"))
}

/// Whether `unit` names a terminal tab's scope (as [`tab_unit_name`]
/// builds it) rather than a launch scope.
///
/// The teardown asks so it can hang up a tab's shell as well as terminate
/// it (see `service::sweep::kill_scope`). Anchored on the `-tab-` infix
/// within the farhelm prefix, which no launch unit can spell because a
/// generation is always digits.
pub fn is_tab_unit(unit: &str) -> bool {
    unit.strip_prefix(UNIT_PREFIX)
        .and_then(|rest| rest.strip_suffix(".scope"))
        .is_some_and(|body| body.contains("-tab-"))
}

/// The glob every generation of a session's LAUNCH scope matches, or `None`
/// for a session id that cannot safely name a unit.
///
/// Delete and archive ask the manager for these names because the row only
/// records its current generation. A prior generation can still contain a
/// daemon after its portable sweep reported clean, and tmux has no record of
/// that scope once the old pane is gone. The `[0-9]*` suffix is deliberately
/// narrower than `*`: launch generations are digits, so this cannot match a
/// tab scope's `-tab-` infix.
pub fn launch_unit_glob(session_id: &str) -> Option<String> {
    is_uuid_shaped(session_id).then(|| format!("{UNIT_PREFIX}{session_id}-[0-9]*.scope"))
}

/// Whether `id` is a plain lowercase hyphenated UUID (8-4-4-4-12 hex).
///
/// Deliberately stricter than "parses as a UUID": accepting uppercase or
/// braced forms would reintroduce two spellings of one id, and the only
/// producer this needs to accept is `uuid::Uuid::new_v4().to_string()`.
///
/// Public because it doubles as the ACCEPTANCE test for a tab id read back
/// out of a tmux window marker (`service.rs`'s tab rediscovery): the
/// marker is writable by anything that inherited `TMUX`, so a value that
/// is not the shape this supervisor mints is treated as not a tab at all,
/// rather than carried on into an attachment key, an error message, or a
/// derived unit name.
pub fn is_uuid_shaped(id: &str) -> bool {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut parts = id.split('-');
    for len in GROUPS {
        match parts.next() {
            Some(part)
                if part.len() == len
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) => {}
            _ => return false,
        }
    }
    parts.next().is_none()
}

/// One operation this module performed, reported to a test's sink.
///
/// Exists so a test can pin the ORDER of stop's two mechanisms — the scope
/// kill and the backstop sweep — which is otherwise invisible from outside:
/// both leave the same end state (nothing running), so only observing what
/// is still alive at the moment of a scope call can tell "scope, then
/// sweep" from "sweep, then scope".
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeOp {
    /// The availability probe ran.
    Probe,
    /// Existence of a unit was checked.
    Exists(String),
    /// Units matching a pattern were enumerated.
    List(String),
    /// A signal was sent to a unit's whole cgroup.
    Kill { unit: String, signal: String },
}

/// Where a [`ScopeManager::fake`] reports its operations.
#[cfg(test)]
pub type ScopeOpSink = std::sync::Arc<dyn Fn(&ScopeOp) + Send + Sync>;

/// How a [`ScopeManager`] talks to the world.
///
/// The test double is many times the size of the two production variants
/// (it carries scripted answers and an operation log), which trips
/// `large_enum_variant` only in test builds. One `Mode` lives inside one
/// `ScopeManager` per supervisor, so the size difference costs nothing
/// worth boxing for.
#[cfg_attr(test, allow(clippy::large_enum_variant))]
enum Mode {
    /// The real `systemd-run`/`systemctl --user` pair.
    Systemd,
    /// No manager, unconditionally. Production never selects this; it is
    /// how a test pins the FALLBACK path on a host that does happen to have
    /// a user manager, which is the only way CI's proof and this repo's
    /// development hosts can run the same assertions. Reachable from
    /// outside the crate (unlike the fakes below) because the integration
    /// suite lives in another crate.
    Disabled,
    /// A test double: availability is scripted by probe, nothing is actually signaled,
    /// and every operation is reported to `sink`. `kills_fail` stands in for
    /// the manager that is THERE but not working — the case stop must
    /// survive without losing anything M2 guaranteed. `vanishes_after`
    /// makes `exists` report each unit gone once that many checks for that
    /// unit have been answered, which is how the post-kill confirmation's
    /// two outcomes (converged, and timed out) are both reachable in a test.
    #[cfg(test)]
    Fake {
        probe_answers: std::sync::Mutex<std::collections::VecDeque<bool>>,
        available: std::sync::atomic::AtomicBool,
        kills_fail: bool,
        vanishes_after: Option<usize>,
        /// Report a unit gone once this signal (`"SIGTERM"` or `"SIGKILL"`)
        /// has been sent to it — an agent that exits politely, or one that
        /// dies only to the unignorable signal — expressed by EVENT rather
        /// than by check count so a test does not depend on how many times
        /// the teardown happens to ask in between.
        vanishes_after_signal: Option<&'static str>,
        /// Every `(unit, signal)` pair `kill` has been asked to send, whether
        /// or not it reported success.
        signalled: std::sync::Mutex<std::collections::HashSet<(String, String)>>,
        exists_calls: std::sync::Mutex<std::collections::HashMap<String, usize>>,
        matching_units: Vec<String>,
        sink: ScopeOpSink,
    },
}

/// The user-manager binaries this supervisor will use, resolved once to
/// ABSOLUTE paths.
///
/// Resolved rather than invoked by bare name, and this is a trust property
/// rather than tidiness: the probe establishes that a specific
/// `systemd-run` works, and every later invocation must be that same
/// binary. Bare names are resolved against `$PATH` at each spawn, and a
/// session's login shell — which SPEC.md's environment contract invites the
/// user to configure freely — can prepend a directory containing a
/// `systemd-run` of its own between the probe and the launch. Absolute
/// paths make the probe's verdict describe the binary that actually runs.
struct Tools {
    systemd_run: PathBuf,
    systemctl: PathBuf,
    /// Whether this systemd accepts `--expand-environment=no`.
    ///
    /// Added in systemd 254; newer systemd expands `$`-references in a
    /// command's argv BY DEFAULT, which would mangle a perfectly legal
    /// working directory or agent path containing a dollar sign. Where the
    /// flag exists it is always passed; where it does not, the systemd is
    /// old enough that expansion is not the default either, so omitting it
    /// is equally safe. Probed rather than version-sniffed — the question is
    /// whether THIS binary takes the flag.
    expand_environment_flag: bool,
}

/// This supervisor's access to a systemd user manager, and its cached
/// verdict on whether one is usable.
///
/// The verdict is cached because it is a property of the host's user
/// session rather than of a moment (PLAN_M3.md item 10's "probe once,
/// cache"), and because probing means creating and tearing down a real
/// transient unit — cheap, but not free, and not something to repeat on
/// every launch. Each LAUNCH still makes and records its own selection from
/// that verdict, which is what lets one session run under a scope and a
/// later one (after a restart on a host that lost its manager) honestly
/// record the fallback. There is one narrow exception: when teardown holds
/// a unit the durable row says was scoped, a cached negative verdict gets
/// one re-probe. That evidence proves a manager existed when the launch ran,
/// so treating an old transient failure as stronger evidence would discard
/// the one handle on an environment-scrubbed descendant. A second negative
/// is final until the next supervisor process.
///
/// Residual, accepted and documented rather than engineered around: a user
/// manager that dies DURING a supervisor's lifetime leaves the cached
/// verdict saying "available", and the next launch's `systemd-run` then
/// fails inside the pane. That failure is CLASSIFIED rather than silent —
/// `service.rs`'s `wrapper_failure_detail` recognizes the unconsumed-spec
/// shape it leaves and reports the session as `error` — but it is still a
/// failed launch. The next supervisor start re-probes. The alternative
/// (probing before every launch) buys a narrow window at the cost of a
/// D-Bus round trip on the create path, and a manager disappearing under a
/// live user session is not a failure mode this tool needs to be robust to.
pub struct ScopeManager {
    mode: Mode,
    verdict: tokio::sync::Mutex<Verdict>,
    verdict_changed: tokio::sync::Notify,
}

/// One process's cached user-manager answer.
///
/// Tools are shared by `Arc` so callers can run `systemctl` without holding
/// the verdict mutex. `Probing` only coordinates concurrent callers: it does
/// not make another manager query possible beyond the initial probe and the
/// one permitted re-probe after a negative result.
enum Verdict {
    Unprobed,
    Probing,
    Usable(Option<Arc<Tools>>),
    Unusable { reprobed: bool },
}

impl std::fmt::Debug for ScopeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match self.mode {
            Mode::Systemd => "systemd",
            Mode::Disabled => "disabled",
            #[cfg(test)]
            Mode::Fake { .. } => "fake",
        };
        f.debug_struct("ScopeManager").field("mode", &mode).finish()
    }
}

impl ScopeManager {
    /// The production manager: talks to the host's real systemd user
    /// instance, if it has one.
    pub fn systemd() -> ScopeManager {
        ScopeManager {
            mode: Mode::Systemd,
            verdict: tokio::sync::Mutex::new(Verdict::Unprobed),
            verdict_changed: tokio::sync::Notify::new(),
        }
    }

    /// A manager that is never available — the injected fallback, see
    /// [`Mode::Disabled`].
    pub fn disabled() -> ScopeManager {
        ScopeManager {
            mode: Mode::Disabled,
            verdict: tokio::sync::Mutex::new(Verdict::Unprobed),
            verdict_changed: tokio::sync::Notify::new(),
        }
    }

    /// A test double reporting every operation to `sink`; see [`ScopeOp`].
    #[cfg(test)]
    pub fn fake(available: bool, sink: ScopeOpSink) -> ScopeManager {
        ScopeManager::fake_with(vec![available], false, None, Vec::new(), sink)
    }

    /// A fake whose first probe and permitted re-probe answer independently.
    #[cfg(test)]
    pub(crate) fn fake_reprobing(first: bool, second: bool, sink: ScopeOpSink) -> ScopeManager {
        ScopeManager::fake_with(vec![first, second], false, None, Vec::new(), sink)
    }

    /// A re-probing fake whose kill confirmation can observe a gone unit.
    #[cfg(test)]
    pub(crate) fn fake_reprobing_vanishing(
        first: bool,
        second: bool,
        vanishes_after: usize,
        sink: ScopeOpSink,
    ) -> ScopeManager {
        ScopeManager::fake_with(
            vec![first, second],
            false,
            Some(vanishes_after),
            Vec::new(),
            sink,
        )
    }

    /// A fake manager that lists `matching_units` for every requested glob.
    #[cfg(test)]
    pub(crate) fn fake_with_matching_units(
        available: bool,
        matching_units: Vec<String>,
        sink: ScopeOpSink,
    ) -> ScopeManager {
        ScopeManager::fake_with(vec![available], false, None, matching_units, sink)
    }

    /// A listing fake whose returned units disappear during kill confirmation.
    #[cfg(test)]
    pub(crate) fn fake_with_matching_units_vanishing(
        available: bool,
        matching_units: Vec<String>,
        vanishes_after: usize,
        sink: ScopeOpSink,
    ) -> ScopeManager {
        ScopeManager::fake_with(
            vec![available],
            false,
            Some(vanishes_after),
            matching_units,
            sink,
        )
    }

    /// A test double for a manager that is present and answers questions but
    /// cannot actually kill anything.
    ///
    /// Its own constructor rather than a flag at every call site because it
    /// stands for a distinct claim: item 10 says absence of a manager never
    /// degrades stop below M2, and this is the other half of that —
    /// PRESENCE of a broken one must not either. A stop against this manager
    /// must still succeed on the sweep's own verdict.
    #[cfg(test)]
    pub fn fake_failing_kills(sink: ScopeOpSink) -> ScopeManager {
        ScopeManager::fake_with(vec![true], true, None, Vec::new(), sink)
    }

    /// A test double whose unit disappears after `vanishes_after` existence
    /// checks — the knob the post-kill confirmation needs, since "the cgroup
    /// emptied" and "it never emptied" are otherwise indistinguishable to a
    /// caller.
    #[cfg(test)]
    pub fn fake_vanishing(vanishes_after: usize, sink: ScopeOpSink) -> ScopeManager {
        ScopeManager::fake_with(vec![true], false, Some(vanishes_after), Vec::new(), sink)
    }

    /// Every kill REPORTS failure, yet the unit retires as soon as a
    /// `SIGKILL` has been sent to it.
    ///
    /// This is the systemd 255 shape `service::sweep::kill_scope` has to
    /// get right — `systemctl kill` exiting non-zero after a SIGKILL that
    /// killed everything in a multithreaded scope — and neither existing
    /// knob alone can express it: [`fake_failing_kills`](Self::fake_failing_kills)
    /// models a manager whose kills do nothing, and
    /// [`fake_vanishing`](Self::fake_vanishing) one whose kills work and
    /// say so. The distinction a caller must draw is between the report
    /// and the outcome, which is exactly what this fixture separates.
    #[cfg(test)]
    pub fn fake_failing_kills_vanishing_after_sigkill(sink: ScopeOpSink) -> ScopeManager {
        ScopeManager::fake_with(vec![true], true, None, Vec::new(), sink)
            .vanishing_after_signal("SIGKILL")
    }

    /// A test double whose kills succeed and whose units retire once
    /// `signal` (`"SIGTERM"` or `"SIGKILL"`) has been sent to them: an agent
    /// that exits politely, or one that dies only to the unignorable signal.
    /// Keyed on the event rather than a check count because the SIGTERM
    /// grace polls existence a timing-dependent number of times.
    #[cfg(test)]
    pub fn fake_vanishing_after_signal(signal: &'static str, sink: ScopeOpSink) -> ScopeManager {
        ScopeManager::fake_with(vec![true], false, None, Vec::new(), sink)
            .vanishing_after_signal(signal)
    }

    /// Make a fake's units retire once `signal` has been sent to them; see
    /// the `vanishes_after_signal` field.
    #[cfg(test)]
    fn vanishing_after_signal(mut self, signal: &'static str) -> ScopeManager {
        if let Mode::Fake {
            vanishes_after_signal,
            ..
        } = &mut self.mode
        {
            *vanishes_after_signal = Some(signal);
        }
        self
    }

    #[cfg(test)]
    fn fake_with(
        probe_answers: Vec<bool>,
        kills_fail: bool,
        vanishes_after: Option<usize>,
        matching_units: Vec<String>,
        sink: ScopeOpSink,
    ) -> ScopeManager {
        ScopeManager {
            mode: Mode::Fake {
                probe_answers: std::sync::Mutex::new(probe_answers.into()),
                available: std::sync::atomic::AtomicBool::new(false),
                kills_fail,
                vanishes_after,
                vanishes_after_signal: None,
                signalled: std::sync::Mutex::new(std::collections::HashSet::new()),
                exists_calls: std::sync::Mutex::new(std::collections::HashMap::new()),
                matching_units,
                sink,
            },
            verdict: tokio::sync::Mutex::new(Verdict::Unprobed),
            verdict_changed: tokio::sync::Notify::new(),
        }
    }

    /// Whether a usable systemd user manager exists, probed at most once.
    ///
    /// "Usable" is deliberately a FUNCTIONAL question about the WHOLE
    /// interface, not a `which systemd-run` question: a container image can
    /// carry the binaries with no user manager behind them, `systemctl` can
    /// be absent where `systemd-run` is present, and a manager can accept a
    /// unit while refusing the `kill` that stop depends on — all of which
    /// look like success to a path lookup and fail at the one moment a wrong
    /// answer is expensive. So the probe runs a real transient scope, looks
    /// it up, kills it, and confirms it went away, believing only the exit
    /// statuses it saw.
    ///
    /// Any failure selects the fallback SILENTLY as far as the user is
    /// concerned — a host without a user manager is not misconfigured, it is
    /// just a host — but never silently as far as the record goes: the
    /// verdict is logged once here and every launch records its own
    /// selection durably.
    pub async fn available(&self) -> bool {
        self.tools(false).await.is_some()
    }

    /// Re-check a cached negative verdict once for durable teardown evidence.
    ///
    /// A positive verdict stays cached even if its manager dies later; that
    /// accepted residual is documented on [`ScopeManager`]. A second negative
    /// also stays final, so repeated teardown work cannot turn into repeated
    /// manager probes.
    pub async fn reprobe(&self) -> bool {
        self.tools(true).await.is_some()
    }

    /// Return the cached tools, probing once and re-probing one negative only
    /// when `allow_reprobe` says the caller holds durable scope evidence.
    async fn tools(&self, allow_reprobe: bool) -> Option<Arc<Tools>> {
        if matches!(&self.mode, Mode::Disabled) {
            return None;
        }

        let notified = self.verdict_changed.notified();
        tokio::pin!(notified);
        loop {
            // Register before observing `Probing`, while the mutex still
            // orders us against the probe owner. `notify_waiters` otherwise
            // has no stored permit and could fire in the gap before await.
            notified.as_mut().enable();
            let should_probe = {
                let mut verdict = self.verdict.lock().await;
                match &*verdict {
                    Verdict::Usable(tools) => return tools.clone(),
                    Verdict::Unusable { reprobed } if !allow_reprobe || *reprobed => return None,
                    Verdict::Unprobed => {
                        *verdict = Verdict::Probing;
                        Some(false)
                    }
                    Verdict::Unusable { .. } => {
                        *verdict = Verdict::Probing;
                        Some(true)
                    }
                    Verdict::Probing => None,
                }
            };
            let Some(reprobed) = should_probe else {
                notified.as_mut().await;
                notified.set(self.verdict_changed.notified());
                continue;
            };
            let tools = self.probe().await;
            let available = tools.is_some();
            {
                let mut verdict = self.verdict.lock().await;
                *verdict = if available {
                    Verdict::Usable(tools)
                } else {
                    Verdict::Unusable { reprobed }
                };
            }
            self.verdict_changed.notify_waiters();
        }
    }

    /// Probe the mode without retaining the verdict mutex across process I/O.
    async fn probe(&self) -> Option<Arc<Tools>> {
        match &self.mode {
            Mode::Disabled => None,
            #[cfg(test)]
            Mode::Fake {
                probe_answers,
                available,
                sink,
                ..
            } => {
                sink(&ScopeOp::Probe);
                let answer = probe_answers.lock().unwrap().pop_front().unwrap_or(false);
                available.store(answer, std::sync::atomic::Ordering::SeqCst);
                answer.then(|| {
                    Arc::new(Tools {
                        systemd_run: PathBuf::new(),
                        systemctl: PathBuf::new(),
                        expand_environment_flag: false,
                    })
                })
            }
            Mode::Systemd => {
                let tools = probe_systemd().await.map(Arc::new);
                match &tools {
                    Some(tools) => tracing::info!(
                        systemd_run = %tools.systemd_run.display(),
                        systemctl = %tools.systemctl.display(),
                        expand_environment_flag = tools.expand_environment_flag,
                        "systemd user manager is usable; launches will run in their own \
                         transient scope, with the process-tree sweep as backstop"
                    ),
                    None => tracing::info!(
                        "no usable systemd user manager; launches will rely on the \
                         process-tree sweep alone (M2 behavior)"
                    ),
                }
                tools
            }
        }
    }

    /// The argv prefix that wraps a launch in `unit`, or `None` when this
    /// host has no usable manager.
    ///
    /// Every element is load-bearing:
    ///
    /// - the ABSOLUTE `systemd-run` path, so the login shell's `$PATH`
    ///   cannot substitute the binary the probe approved (see [`Tools`]);
    /// - `--user` puts the scope under this user's own manager, the only one
    ///   an unprivileged supervisor can create units in;
    /// - `--scope` is what makes `systemd-run` `exec` in place instead of
    ///   spawning a service under the manager; a service would run detached
    ///   from the pane entirely and lose the terminal;
    /// - `--collect` has systemd garbage-collect the unit once it goes
    ///   inactive, including after a failure, so a host does not accumulate
    ///   dead scope units for every session ever launched;
    /// - `--quiet` suppresses the "Running scope as unit ..." banner, which
    ///   would otherwise be the first thing the user sees in their terminal;
    /// - `--expand-environment=no`, where supported, keeps a `$` in a path
    ///   from being expanded away (see [`Tools::expand_environment_flag`]);
    /// - `--unit` pins the generation-scoped name stop later derives;
    ///   without it systemd invents a random name nothing could find again;
    /// - `--` stops flag parsing before the shim's own argv.
    pub async fn launch_prefix(&self, unit: &str) -> Option<Vec<String>> {
        let tools = match &self.mode {
            Mode::Systemd => self.tools(false).await?,
            // A fake never launches anything; a disabled manager has nothing
            // to wrap with. Both answer honestly rather than handing back a
            // prefix naming a binary they never probed.
            _ => return None,
        };
        let mut prefix = vec![
            tools.systemd_run.to_string_lossy().into_owned(),
            "--user".to_string(),
            "--scope".to_string(),
            "--collect".to_string(),
            "--quiet".to_string(),
        ];
        if tools.expand_environment_flag {
            prefix.push("--expand-environment=no".to_string());
        }
        prefix.push(format!("--unit={unit}"));
        prefix.push("--".to_string());
        Some(prefix)
    }

    /// Whether `unit` is still known to the manager.
    ///
    /// Checked before every use of a unit name, because the name is
    /// re-derived from durable state (id + generation) rather than
    /// remembered from the launch: a supervisor restart, a `--collect`
    /// garbage collection after the agent exited on its own, or a manual
    /// `systemctl --user stop` can all have removed it. A gone unit is not an
    /// error — it is a session whose stop is sweep-only — so this is `false`
    /// rather than a failure in that case, and a failure ONLY when the
    /// manager could not be asked at all.
    pub async fn exists(&self, unit: &str) -> anyhow::Result<bool> {
        match &self.mode {
            Mode::Disabled => Ok(false),
            #[cfg(test)]
            Mode::Fake {
                available,
                vanishes_after,
                vanishes_after_signal,
                signalled,
                exists_calls,
                sink,
                ..
            } => {
                sink(&ScopeOp::Exists(unit.to_string()));
                let mut calls = exists_calls
                    .lock()
                    .expect("fake scope existence log poisoned");
                let seen = calls.entry(unit.to_string()).or_insert(0);
                let prior = *seen;
                *seen += 1;
                let retired_by_signal = vanishes_after_signal.is_some_and(|signal| {
                    signalled
                        .lock()
                        .expect("fake scope signal log poisoned")
                        .contains(&(unit.to_string(), signal.to_string()))
                });
                Ok(available.load(std::sync::atomic::Ordering::SeqCst)
                    && vanishes_after.is_none_or(|after| prior < after)
                    && !retired_by_signal)
            }
            Mode::Systemd => {
                let tools = self.tools(false).await.ok_or_else(|| {
                    anyhow::anyhow!("no systemd user manager to ask about {unit}")
                })?;
                let out = run_with_timeout(
                    tokio::process::Command::new(&tools.systemctl)
                        .arg("--user")
                        .arg("show")
                        .arg("-p")
                        .arg("LoadState")
                        .arg("--value")
                        .arg(unit),
                )
                .await
                .with_context(|| format!("asking the user manager about scope {unit}"))?;
                if !out.status.success() {
                    anyhow::bail!(
                        "systemctl --user show {unit} exited {:?}: {}",
                        out.status.code(),
                        String::from_utf8_lossy(&out.stderr).trim()
                    );
                }
                // `not-found` is systemd's own word for a unit it has no
                // record of; anything else (loaded, masked, ...) means the
                // manager still has state for this name.
                Ok(String::from_utf8_lossy(&out.stdout).trim() != "not-found")
            }
        }
    }

    /// Every unit this manager knows whose name matches `pattern` — the
    /// tmux-independent half of a session teardown (see [`tab_unit_glob`]
    /// and [`launch_unit_glob`]).
    ///
    /// `Ok(vec![])` means the manager answered and has none, which is the
    /// ordinary case and a real answer. An `Err` means the manager could
    /// not be asked, which a teardown must NOT flatten into "there are
    /// none": the whole reason this exists is the case where tmux has
    /// already died and the cgroup is the only remaining handle on a
    /// scrubbed daemon.
    ///
    /// `--all` is deliberate: a scope whose processes have all exited is
    /// still worth naming (it costs one no-op kill), while omitting it
    /// would hide units in states this code has no reason to enumerate.
    /// `--plain --no-legend` strips systemd's table decoration so the
    /// first field of each line is the unit name and nothing else.
    pub async fn units_matching(&self, pattern: &str) -> anyhow::Result<Vec<String>> {
        match &self.mode {
            Mode::Disabled => Ok(Vec::new()),
            #[cfg(test)]
            Mode::Fake {
                matching_units,
                sink,
                ..
            } => {
                sink(&ScopeOp::List(pattern.to_string()));
                Ok(matching_units
                    .iter()
                    .filter(|unit| unit_matches_pattern(pattern, unit))
                    .cloned()
                    .collect())
            }
            Mode::Systemd => {
                let tools = self.tools(false).await.ok_or_else(|| {
                    anyhow::anyhow!("no systemd user manager to list units matching {pattern}")
                })?;
                let out = run_with_timeout(
                    tokio::process::Command::new(&tools.systemctl)
                        .arg("--user")
                        .arg("list-units")
                        .arg("--all")
                        .arg("--plain")
                        .arg("--no-legend")
                        .arg("--type=scope")
                        .arg(pattern),
                )
                .await
                .with_context(|| format!("listing user units matching {pattern}"))?;
                if !out.status.success() {
                    anyhow::bail!(
                        "systemctl --user list-units {pattern} exited {:?}: {}",
                        out.status.code(),
                        String::from_utf8_lossy(&out.stderr).trim()
                    );
                }
                Ok(String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .filter_map(|line| line.split_whitespace().next())
                    .filter(|name| name.starts_with(UNIT_PREFIX))
                    .map(str::to_string)
                    .collect())
            }
        }
    }

    /// Send `signal` to EVERY process in `unit`'s cgroup.
    ///
    /// The whole point of the mechanism: `systemctl kill` with the default
    /// `--kill-whom=all` reaches every member of the scope, including the
    /// double-forked, environment-scrubbed descendant the sweep provably
    /// cannot find. `signal` is a systemd signal name (`SIGTERM`, `SIGKILL`).
    ///
    /// A unit that is already gone reports an error here, which callers
    /// treat as information rather than failure — see
    /// `service::reap_process_tree`, where nothing about the scope is ever
    /// allowed to fail a stop the sweep afterwards confirms.
    ///
    /// More generally, an `Err` from this call is a REPORT, not the
    /// outcome: `systemctl kill` can exit non-zero after delivering the
    /// signal to everything in the cgroup (systemd 255 does exactly that
    /// for SIGKILL against a scope holding a multithreaded process, see
    /// `service::sweep::kill_scope`'s docs). Callers that need to know
    /// whether the kill WORKED ask [`exists`](Self::exists) afterwards
    /// and let the unit's disappearance decide.
    pub async fn kill(&self, unit: &str, signal: &str) -> anyhow::Result<()> {
        match &self.mode {
            Mode::Disabled => anyhow::bail!("no systemd user manager to kill scope {unit} with"),
            #[cfg(test)]
            Mode::Fake {
                kills_fail,
                signalled,
                sink,
                ..
            } => {
                sink(&ScopeOp::Kill {
                    unit: unit.to_string(),
                    signal: signal.to_string(),
                });
                // Recorded BEFORE the failure report, deliberately: the
                // `vanishes_after_signal` shape is a kill that worked while
                // its report said otherwise, so the event must count even
                // when this call is about to return an error.
                signalled
                    .lock()
                    .expect("fake scope signal log poisoned")
                    .insert((unit.to_string(), signal.to_string()));
                if *kills_fail {
                    anyhow::bail!("injected failure sending {signal} to scope {unit}");
                }
                Ok(())
            }
            Mode::Systemd => {
                let tools = self.tools(false).await.ok_or_else(|| {
                    anyhow::anyhow!("no systemd user manager to kill scope {unit} with")
                })?;
                let out = run_with_timeout(
                    tokio::process::Command::new(&tools.systemctl)
                        .arg("--user")
                        .arg("kill")
                        .arg(format!("--signal={signal}"))
                        .arg(unit),
                )
                .await
                .with_context(|| format!("sending {signal} to scope {unit}"))?;
                if !out.status.success() {
                    anyhow::bail!(
                        "systemctl --user kill --signal={signal} {unit} exited {:?}: {}",
                        out.status.code(),
                        String::from_utf8_lossy(&out.stderr).trim()
                    );
                }
                Ok(())
            }
        }
    }
}

/// Match the two session-scope glob shapes the fake needs to model.
///
/// This is deliberately not a general systemd glob implementation. Production
/// passes its pattern straight to `systemctl`; the fake only needs to keep the
/// test boundary honest by distinguishing the tab `*` suffix from the launch
/// `[0-9]*` suffix that must reject `-tab-` names.
#[cfg(test)]
fn unit_matches_pattern(pattern: &str, unit: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("[0-9]*.scope") {
        return unit
            .strip_prefix(prefix)
            .and_then(|suffix| suffix.strip_suffix(".scope"))
            .is_some_and(|generation| {
                !generation.is_empty() && generation.bytes().all(|b| b.is_ascii_digit())
            });
    }
    if let Some(prefix) = pattern.strip_suffix("*.scope") {
        return unit
            .strip_prefix(prefix)
            .and_then(|suffix| suffix.strip_suffix(".scope"))
            .is_some_and(|suffix| !suffix.is_empty());
    }
    unit == pattern
}

/// Resolve `name` to an absolute path by walking `$PATH`, or `None`.
///
/// Only regular-file candidates with an execute bit are accepted, and only
/// ABSOLUTE `$PATH` entries are considered: a relative entry (including the
/// empty string, which POSIX reads as the current directory) would resolve
/// against whatever directory the supervisor happens to be running in.
fn resolve_program(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if !dir.is_absolute() {
            continue;
        }
        let candidate = dir.join(name);
        let Ok(meta) = std::fs::metadata(&candidate) else {
            continue;
        };
        if meta.is_file() && meta.permissions().mode() & 0o111 != 0 {
            return Some(candidate);
        }
    }
    None
}

/// Exercise the whole user-manager interface once and report the tools, or
/// `None` if any step failed.
///
/// The sequence is create → show → kill → gone, because those are exactly
/// the four things a launch and a stop later depend on, and each can fail
/// independently of the others: a manager that creates units but refuses
/// `kill` would otherwise be discovered only when a user's stop silently did
/// nothing. The probe's own unit is NAMED, like every other farhelm scope,
/// so a leftover is recognizable; `--collect` disposes of it either way.
///
/// `/bin/sh` is the probe command. That is a Linux deployment assumption
/// rather than a portability claim — POSIX standardizes the name `sh`, not
/// the path — and a safe one here, because this whole module only ever
/// matters on a host running a systemd user manager.
async fn probe_systemd() -> Option<Tools> {
    let systemd_run = resolve_program("systemd-run")?;
    let systemctl = resolve_program("systemctl")?;
    match tokio::time::timeout(PROBE_TIMEOUT, probe_round_trip(&systemd_run, &systemctl)).await {
        Ok(Ok(expand_environment_flag)) => Some(Tools {
            systemd_run,
            systemctl,
            expand_environment_flag,
        }),
        Ok(Err(e)) => {
            // warn!, not debug!: this is production silently losing cgroup
            // containment and falling back to the sweep alone, which is
            // exactly the kind of degradation that must not be invisible.
            // The named shape tells an operator what to do about it — a
            // "timeout" points at an overloaded manager, a "retry_failed"
            // at both attempts failing (read the attached error for which
            // step; a healthy old systemd rejects the flag and then
            // SUCCEEDS unflagged, producing no warning at all), and a
            // bare "error" at something this module did not anticipate.
            tracing::warn!(
                error = %format!("{e:#}"),
                shape = describe_probe_failure(&e),
                "the systemd user-manager probe failed; selecting the process-tree sweep alone"
            );
            None
        }
        Err(_) => {
            tracing::warn!(
                shape = "timeout",
                "the systemd user-manager probe did not finish within {PROBE_TIMEOUT:?}; \
                 selecting the process-tree sweep alone"
            );
            None
        }
    }
}

/// Marks a [`run_with_timeout`] failure whose cause was its own
/// [`SYSTEMCTL_TIMEOUT`] elapsing, as distinct from the query completing and
/// reporting a real failure (a non-zero exit, a rejected flag).
///
/// Carried purely as an entry in the `anyhow::Error` cause chain so
/// [`classify_probe_failure`] can downcast for it: "the manager is slow" and
/// "the manager said no" are the two shapes [`should_retry_without_flag`]
/// must tell apart, and a chain-walk is the only way to find this cause
/// again after [`probe_once`] has wrapped it in several layers of
/// `.context(...)`.
#[derive(Debug)]
struct QueryTimedOut;

impl std::fmt::Display for QueryTimedOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the systemd user manager did not answer within {SYSTEMCTL_TIMEOUT:?}"
        )
    }
}

impl std::error::Error for QueryTimedOut {}

/// Wraps a `probe_once` failure that happened on the RETRIED (unflagged)
/// attempt, once the flagged attempt had ALSO already failed — i.e. the
/// fallback the flag retry exists for was actually exercised and still came
/// up empty, rather than the flagged attempt failing without ever being
/// retried (see [`ProbeFailureShape::Timeout`]).
///
/// A plain `.context(...)` cannot be used for this marker the way
/// [`QueryTimedOut`] is used: `Context::context`'s argument only supplies
/// `Display` text for the NEW wrapping error, it is not itself pushed onto
/// the cause chain as a distinct, downcastable link. Wrapping the retried
/// error directly and forwarding `source()` to it is what keeps this type
/// visible to [`describe_probe_failure`]'s chain walk while leaving the
/// original message intact for humans (`Display` just forwards to it).
#[derive(Debug)]
struct BothProbeAttemptsFailed(anyhow::Error);

impl std::fmt::Display for BothProbeAttemptsFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for BothProbeAttemptsFailed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.0 as &(dyn std::error::Error + 'static))
    }
}

/// Why one `probe_once` attempt failed, coarse enough to be exactly what
/// [`should_retry_without_flag`] needs and no more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeFailureShape {
    /// A per-query [`SYSTEMCTL_TIMEOUT`] elapsed: the manager is slow to
    /// answer, which says nothing about whether it would have accepted
    /// `--expand-environment=no`.
    Timeout,
    /// Any other failure — a non-zero exit, a spawn error, or the flag
    /// rejection the retry in [`probe_round_trip`] exists to route around.
    Error,
}

/// Classify a `probe_once` failure by walking its cause chain for the
/// [`QueryTimedOut`] marker, so callers can react to "the manager is slow"
/// differently from every other failure shape.
fn classify_probe_failure(err: &anyhow::Error) -> ProbeFailureShape {
    if err.chain().any(|cause| cause.is::<QueryTimedOut>()) {
        ProbeFailureShape::Timeout
    } else {
        ProbeFailureShape::Error
    }
}

/// Whether [`probe_round_trip`] should retry `probe_once` without
/// `--expand-environment=no`, given that the flagged attempt failed with
/// `shape`.
///
/// Pulled out as a pure function because this decision is the crux of the
/// fix: a flag rejection is exactly the case the retry exists for, but a
/// [`ProbeFailureShape::Timeout`] means the manager just proved itself slow
/// — retrying would spend what is left of [`PROBE_TIMEOUT`] on a second
/// attempt against that SAME overloaded manager, for strictly worse odds
/// (less budget) and no new information. Letting the probe fail fast there
/// is more honest than a doomed second round trip.
fn should_retry_without_flag(shape: ProbeFailureShape) -> bool {
    !matches!(shape, ProbeFailureShape::Timeout)
}

/// Label a failed probe for the operator log, distinguishing the shapes
/// that call for different reactions:
///
/// - `"timeout"` — the manager is slow, possibly just under load; a retry
///   would not have helped (see [`should_retry_without_flag`]).
/// - `"retry_failed"` — both the flagged and unflagged attempts failed, the
///   scenario the flag's fallback exists to catch (a systemd too old for
///   `--expand-environment=no`, or a manager broken outright).
/// - `"error"` — the sole attempt failed for some other reason without a
///   retry. Not reachable under today's always-retry-on-non-timeout policy;
///   kept so this function stays correct if that policy ever narrows.
fn describe_probe_failure(err: &anyhow::Error) -> &'static str {
    let timed_out = err.chain().any(|cause| cause.is::<QueryTimedOut>());
    let retried = err
        .chain()
        .any(|cause| cause.is::<BothProbeAttemptsFailed>());
    match (timed_out, retried) {
        (true, _) => "timeout",
        (false, true) => "retry_failed",
        (false, false) => "error",
    }
}

/// One full create/show/kill/gone round trip, reporting whether
/// `--expand-environment=no` was accepted.
///
/// The flag is tried FIRST and the whole round trip repeated without it on
/// failure, rather than probed separately: "does this binary accept the
/// flag" and "does this manager work" are the same experiment, and a systemd
/// too old for the flag is also too old to expand argv by default (see
/// [`Tools::expand_environment_flag`]), so dropping it there is safe.
///
/// The retry is skipped for a [`ProbeFailureShape::Timeout`]
/// ([`should_retry_without_flag`]): a manager that just took longer than
/// [`SYSTEMCTL_TIMEOUT`] to answer has not told us anything about the flag,
/// and a second attempt against it would only spend down [`PROBE_TIMEOUT`]
/// for nothing.
async fn probe_round_trip(systemd_run: &Path, systemctl: &Path) -> anyhow::Result<bool> {
    let with_flag = match probe_once(systemd_run, systemctl, true).await {
        Ok(()) => return Ok(true),
        Err(e) => e,
    };
    if !should_retry_without_flag(classify_probe_failure(&with_flag)) {
        return Err(with_flag);
    }
    match probe_once(systemd_run, systemctl, false).await {
        Ok(()) => Ok(false),
        // Wrapped in `BothProbeAttemptsFailed` (see its doc comment for why
        // a plain `.context(...)` will not do) purely so
        // `describe_probe_failure` can name this "retry_failed" rather than a
        // less informative "error"; the message text is unchanged.
        Err(without_flag) => Err(anyhow::Error::new(BothProbeAttemptsFailed(without_flag))
            .context(format!(
                "and the same probe with --expand-environment=no failed too ({with_flag:#})"
            ))),
    }
}

/// Create a throwaway scope holding a sleeping shell, confirm the manager
/// reports it, kill it, and confirm it went away.
async fn probe_once(
    systemd_run: &Path,
    systemctl: &Path,
    expand_environment_flag: bool,
) -> anyhow::Result<()> {
    let unit = format!("{UNIT_PREFIX}probe-{}.scope", uuid::Uuid::new_v4());
    let mut cmd = tokio::process::Command::new(systemd_run);
    cmd.arg("--user")
        .arg("--scope")
        .arg("--collect")
        .arg("--quiet");
    if expand_environment_flag {
        cmd.arg("--expand-environment=no");
    }
    // `kill_on_drop` matters here specifically: every `?` below abandons
    // this child, and without it a failed probe would leave a sleeping shell
    // behind for the rest of the supervisor's life.
    let mut child = cmd
        .arg(format!("--unit={unit}"))
        .arg("--")
        .arg("/bin/sh")
        .arg("-c")
        .arg("sleep 30")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("running {}", systemd_run.display()))?;

    let result = async {
        wait_for_unit(systemctl, &unit, true)
            .await
            .context("the probe scope never became visible to the user manager")?;
        let killed = run_with_timeout(
            tokio::process::Command::new(systemctl)
                .arg("--user")
                .arg("kill")
                .arg("--signal=SIGKILL")
                .arg(&unit),
        )
        .await
        .context("killing the probe scope")?;
        if !killed.status.success() {
            anyhow::bail!(
                "systemctl --user kill on the probe scope exited {:?}: {}",
                killed.status.code(),
                String::from_utf8_lossy(&killed.stderr).trim()
            );
        }
        wait_for_unit(systemctl, &unit, false)
            .await
            .context("the probe scope was killed but never went away")
    }
    .await;

    // Reaped either way: on success the kill already ended it, and on failure
    // this is what actually stops it.
    let _ = child.kill().await;
    let _ = child.wait().await;
    result
}

/// Poll `systemctl show` until `unit`'s existence matches `want`.
///
/// Unbounded on purpose: [`probe_systemd`]'s own [`PROBE_TIMEOUT`] bounds the
/// whole sequence, and a second bound per step would only be a second number
/// to keep in agreement with the first.
async fn wait_for_unit(systemctl: &Path, unit: &str, want: bool) -> anyhow::Result<()> {
    loop {
        let out = run_with_timeout(
            tokio::process::Command::new(systemctl)
                .arg("--user")
                .arg("show")
                .arg("-p")
                .arg("LoadState")
                .arg("--value")
                .arg(unit),
        )
        .await?;
        if !out.status.success() {
            anyhow::bail!(
                "systemctl --user show exited {:?}: {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        if (String::from_utf8_lossy(&out.stdout).trim() != "not-found") == want {
            return Ok(());
        }
        tokio::time::sleep(UNIT_POLL_INTERVAL).await;
    }
}

/// Run `cmd` to completion under [`SYSTEMCTL_TIMEOUT`], killing it and
/// erroring out past the bound.
///
/// Every QUERY into the user manager goes through here: a wedged manager must
/// not be able to hang a stop, and `Command::output` on its own has no bound
/// at all. The launch wrapper deliberately does not (see
/// [`SYSTEMCTL_TIMEOUT`]).
///
/// The timeout branch's error carries the [`QueryTimedOut`] marker rather
/// than being an anonymous string: [`probe_round_trip`] needs to tell "this
/// query merely took too long" apart from every other failure shape, and it
/// can only do that by downcasting for something more specific than text.
async fn run_with_timeout(
    cmd: &mut tokio::process::Command,
) -> anyhow::Result<std::process::Output> {
    // `kill_on_drop` is what makes the timeout real: without it the child
    // outlives the abandoned future and keeps holding its pipes open.
    let child = cmd
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .output();
    tokio::time::timeout(SYSTEMCTL_TIMEOUT, child)
        .await
        // `QueryTimedOut` rather than a bare `anyhow::anyhow!`: its identity
        // (not just its text) is what lets `classify_probe_failure` tell a
        // timeout apart from every other failure shape further up the call
        // chain.
        .map_err(|_| anyhow::Error::new(QueryTimedOut))?
        .context("running a systemd user-manager command")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const UUID_A: &str = "2b1f0e4c-0000-4000-8000-000000000001";

    /// Unit names must be generation-scoped and may only ever be derived from
    /// a UUID-shaped id. Both halves are load-bearing: a name shared across
    /// generations would let a stop signal a PREVIOUS run's scope (the same
    /// class of bug the launch spec's generation suffix excludes), and any
    /// sanitizing of a non-UUID id would be non-injective — two live sessions
    /// could map onto one unit, so stopping one would kill the other's agent.
    /// Refusing is what makes that unconstructible.
    #[farhelm_testtrace::test]
    fn unit_names_are_generation_scoped_and_only_ever_derived_from_uuids() {
        assert_eq!(
            unit_name(UUID_A, 0).as_deref(),
            Some("farhelm-2b1f0e4c-0000-4000-8000-000000000001-0.scope")
        );
        assert_ne!(unit_name(UUID_A, 0), unit_name(UUID_A, 1));
        // Everything that is not the exact shape `new_session_identity` mints
        // selects the fallback rather than a name that might collide.
        for bad in [
            "s1",
            "a/b c@d",
            "2B1F0E4C-0000-4000-8000-000000000001",
            "{2b1f0e4c-0000-4000-8000-000000000001}",
            "2b1f0e4c00004000800000000000001",
            "2b1f0e4c-0000-4000-8000-000000000001-extra",
            "",
        ] {
            assert_eq!(unit_name(bad, 0), None, "{bad:?} must not name a unit");
        }
    }

    /// A tab's unit name must be keyed by BOTH ids and must refuse either
    /// one that is not the shape this supervisor mints.
    ///
    /// The tab id is the half that matters most, and for a reason the
    /// session id does not share: it is read back out of a tmux window
    /// option, and anything that inherited `TMUX` inside a pane can write
    /// one. A name derived from an attacker-chosen (or merely corrupted)
    /// marker would aim a `systemctl kill` at whatever unit that marker
    /// spelled. Refusing is what makes that unconstructible, exactly as
    /// for [`unit_name`].
    #[farhelm_testtrace::test]
    fn tab_unit_names_are_keyed_by_both_ids_and_refuse_anything_else() {
        const UUID_B: &str = "9c3d5a71-0000-4000-8000-0000000000ff";
        assert_eq!(
            tab_unit_name(UUID_A, UUID_B).as_deref(),
            Some(
                "farhelm-2b1f0e4c-0000-4000-8000-000000000001-tab-\
                 9c3d5a71-0000-4000-8000-0000000000ff.scope"
            )
        );
        // Two tabs of one session, and one tab id under two sessions, must
        // all be distinct names — a collision either way would have one
        // close kill another terminal's processes.
        assert_ne!(tab_unit_name(UUID_A, UUID_B), tab_unit_name(UUID_A, UUID_A));
        assert_ne!(tab_unit_name(UUID_A, UUID_B), tab_unit_name(UUID_B, UUID_B));
        // Disjoint from the LAUNCH namespace: a generation is always
        // digits, so no launch unit can spell `-tab-`.
        assert_ne!(tab_unit_name(UUID_A, UUID_B), unit_name(UUID_A, 0));
        for bad in ["", "not-a-uuid", "../../etc", "9c3d5a71 0000 4000 8000"] {
            assert_eq!(
                tab_unit_name(UUID_A, bad),
                None,
                "tab id {bad:?} must not name a unit"
            );
            assert_eq!(
                tab_unit_name(bad, UUID_B),
                None,
                "session id {bad:?} must not name a unit"
            );
        }
    }

    /// Real UUIDs from the real minter must always be nameable — the other
    /// direction of the invariant above, and the one whose failure would
    /// silently disable the whole feature rather than announce itself.
    #[farhelm_testtrace::test]
    fn every_minted_session_id_can_name_a_unit() {
        for _ in 0..64 {
            let id = uuid::Uuid::new_v4().to_string();
            assert!(unit_name(&id, 0).is_some(), "{id} must name a unit");
        }
    }

    /// A disabled manager must be inert in every direction: unavailable, no
    /// unit ever exists, no prefix, and a kill through it is an error rather
    /// than a silent success. This is the shape CI's whole fallback proof
    /// rests on — if `disabled()` ever reported a unit as existing, the
    /// fallback tests would start exercising the scope path without saying so.
    #[farhelm_testtrace::test]
    async fn a_disabled_manager_reports_nothing_and_refuses_to_kill() {
        let scopes = ScopeManager::disabled();
        assert!(!scopes.available().await);
        assert!(!scopes.exists("farhelm-x-0.scope").await.unwrap());
        assert!(scopes.launch_prefix("farhelm-x-0.scope").await.is_none());
        assert!(scopes.kill("farhelm-x-0.scope", "SIGTERM").await.is_err());
    }

    /// The availability verdict is cached, which PLAN_M3.md item 10 asks for
    /// explicitly ("probe once, cache"). Pinned through the fake's sink
    /// because caching is otherwise invisible: a second probe would return the
    /// same answer and look identical.
    #[farhelm_testtrace::test]
    async fn availability_is_probed_at_most_once() {
        let ops = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = {
            let ops = Arc::clone(&ops);
            Arc::new(move |op: &ScopeOp| ops.lock().unwrap().push(op.clone())) as ScopeOpSink
        };
        let scopes = ScopeManager::fake(true, sink);
        assert!(scopes.available().await);
        assert!(scopes.available().await);
        assert!(scopes.available().await);
        assert_eq!(*ops.lock().unwrap(), vec![ScopeOp::Probe]);
    }

    /// A negative verdict is normally final, but durable teardown evidence
    /// gets one chance to distinguish a startup race from a truly absent
    /// manager. The sink is the observable that matters: availability alone
    /// cannot tell one fresh probe from a cached answer.
    #[farhelm_testtrace::test]
    async fn a_negative_verdict_reprobes_once_and_can_become_usable() {
        let ops = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = {
            let ops = Arc::clone(&ops);
            Arc::new(move |op: &ScopeOp| ops.lock().unwrap().push(op.clone())) as ScopeOpSink
        };
        let scopes = ScopeManager::fake_reprobing(false, true, sink);

        assert!(
            !scopes.available().await,
            "the first probe must stay negative"
        );
        assert!(
            scopes.reprobe().await,
            "the permitted re-probe must use its fresh answer"
        );
        assert!(
            scopes.available().await,
            "a positive re-probe must be cached"
        );
        assert_eq!(
            *ops.lock().unwrap(),
            vec![ScopeOp::Probe, ScopeOp::Probe],
            "only the initial probe and the one durable-evidence re-probe may run"
        );
    }

    /// A second negative answer closes the re-probe door for this process.
    /// Without that bound, every inherited scoped row could turn a manager
    /// outage into another full probe round trip during teardown.
    #[farhelm_testtrace::test]
    async fn a_still_negative_reprobe_is_final_for_this_process() {
        let ops = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = {
            let ops = Arc::clone(&ops);
            Arc::new(move |op: &ScopeOp| ops.lock().unwrap().push(op.clone())) as ScopeOpSink
        };
        let scopes = ScopeManager::fake_reprobing(false, false, sink);

        assert!(!scopes.available().await);
        assert!(!scopes.reprobe().await);
        assert!(!scopes.reprobe().await);
        assert_eq!(*ops.lock().unwrap(), vec![ScopeOp::Probe, ScopeOp::Probe]);
    }

    /// A usable manager never gets a speculative health check. The residual
    /// where it dies later is intentional: only a cached negative can be
    /// stale in a way durable launch evidence is allowed to revisit.
    #[farhelm_testtrace::test]
    async fn a_positive_verdict_is_never_reprobed() {
        let ops = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = {
            let ops = Arc::clone(&ops);
            Arc::new(move |op: &ScopeOp| ops.lock().unwrap().push(op.clone())) as ScopeOpSink
        };
        let scopes = ScopeManager::fake_reprobing(true, false, sink);

        assert!(scopes.available().await);
        assert!(scopes.reprobe().await);
        assert_eq!(*ops.lock().unwrap(), vec![ScopeOp::Probe]);
    }

    /// The launch glob may find prior generations, but it must not reach tab
    /// scopes. Both shapes share the session prefix, so this test exercises
    /// the fake's bracket-class matcher instead of merely comparing strings.
    #[farhelm_testtrace::test]
    async fn launch_scope_glob_refuses_bad_ids_and_excludes_tab_names() {
        let previous = unit_name(UUID_A, 3).expect("the fixture UUID names a launch scope");
        let tab = tab_unit_name(UUID_A, UUID_A).expect("the fixture UUIDs name a tab scope");
        let scopes = ScopeManager::fake_with_matching_units(
            true,
            vec![previous.clone(), tab],
            Arc::new(|_| {}),
        );
        let glob = launch_unit_glob(UUID_A).expect("the fixture UUID names a glob");

        assert_eq!(scopes.units_matching(&glob).await.unwrap(), vec![previous]);
        assert_eq!(launch_unit_glob("not-a-uuid"), None);
    }

    /// The wrapper's flags are the contract with the launch chain, and several
    /// are silently load-bearing (`--quiet` keeps the banner out of the user's
    /// terminal, `--collect` keeps dead units from accumulating, `--` stops
    /// flag parsing before the shim's argv, and the absolute path is what a
    /// hostile `$PATH` must not be able to redirect). Pinned as a whole
    /// against the REAL probe, so it also proves the prefix names a binary
    /// that actually exists.
    ///
    /// Skipped loudly without a user manager: there is nothing to probe, and
    /// asserting on a hand-built prefix would only test the test.
    #[farhelm_testtrace::test]
    async fn the_probed_launch_prefix_pins_every_load_bearing_flag() {
        let scopes = ScopeManager::systemd();
        if !scopes.available().await {
            eprintln!(
                "SKIPPED the_probed_launch_prefix_pins_every_load_bearing_flag: no usable \
                 systemd user manager on this host"
            );
            return;
        }
        let prefix = scopes
            .launch_prefix("farhelm-x-0.scope")
            .await
            .expect("an available manager must yield a prefix");
        assert!(
            Path::new(&prefix[0]).is_absolute() && Path::new(&prefix[0]).exists(),
            "the prefix must name the probed binary by absolute path, got {:?}",
            prefix[0]
        );
        assert!(prefix[0].ends_with("systemd-run"));
        assert_eq!(&prefix[1..5], ["--user", "--scope", "--collect", "--quiet"]);
        assert_eq!(prefix[prefix.len() - 2], "--unit=farhelm-x-0.scope");
        assert_eq!(prefix[prefix.len() - 1], "--");
    }

    /// A dollar sign in an argument must survive the wrapper. systemd ≥254
    /// expands `$`-references in a command's argv by default, so a perfectly
    /// legal working directory or agent path containing one would otherwise be
    /// mangled into something else — or into nothing at all, which is how a
    /// launch fails with no useful message.
    ///
    /// Run against the REAL binary because the whole question is what THIS
    /// systemd does with the flag; skipped loudly where there is none.
    #[farhelm_testtrace::test]
    async fn a_dollar_sign_in_an_argument_survives_the_wrapper() {
        let scopes = ScopeManager::systemd();
        if !scopes.available().await {
            eprintln!(
                "SKIPPED a_dollar_sign_in_an_argument_survives_the_wrapper: no usable systemd \
                 user manager on this host"
            );
            return;
        }
        let unit = format!("{UNIT_PREFIX}test-{}.scope", uuid::Uuid::new_v4());
        let prefix = scopes.launch_prefix(&unit).await.expect("prefix");
        let out = tokio::process::Command::new(&prefix[0])
            .args(&prefix[1..])
            .arg("/bin/sh")
            .arg("-c")
            .arg("printf %s \"$1\"")
            .arg("sh")
            .arg("/tmp/lit$HOME/x")
            .output()
            .await
            .expect("running the wrapped command");
        assert!(
            out.status.success(),
            "the wrapped command must run at all: {out:?}"
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            "/tmp/lit$HOME/x",
            "the wrapper must not expand a literal $ in an argument"
        );
    }

    /// The whole point of this fix: a per-query timeout must not trigger
    /// the `--expand-environment=no` fallback retry, because retrying
    /// spends what is left of `PROBE_TIMEOUT` against a manager that JUST
    /// proved itself slow, for strictly worse odds. Every other failure
    /// shape keeps retrying, unchanged from before this fix.
    #[farhelm_testtrace::test]
    fn only_a_timeout_shape_skips_the_flag_retry() {
        assert!(
            !should_retry_without_flag(ProbeFailureShape::Timeout),
            "a timeout must not spend remaining budget on a doomed retry"
        );
        assert!(
            should_retry_without_flag(ProbeFailureShape::Error),
            "a non-timeout failure must still get the flag-rejection retry"
        );
    }

    /// `classify_probe_failure` is what `probe_round_trip` actually calls to
    /// make the retry decision above, so its downcast has to survive being
    /// buried under the layers of `.context(...)` every real probe error
    /// picks up on the way out (`probe_once`'s "the probe scope never became
    /// visible..." and friends). A chain walk rather than a top-level check
    /// is what makes that survive.
    #[farhelm_testtrace::test]
    fn classifies_a_wrapped_timeout_marker_regardless_of_context_depth() {
        let bare = anyhow::Error::new(QueryTimedOut);
        assert_eq!(classify_probe_failure(&bare), ProbeFailureShape::Timeout);

        let wrapped = anyhow::Error::new(QueryTimedOut)
            .context("killing the probe scope")
            .context("the outer probe_once call");
        assert_eq!(
            classify_probe_failure(&wrapped),
            ProbeFailureShape::Timeout,
            "a timeout buried under unrelated context must still classify as Timeout"
        );

        let ordinary = anyhow::anyhow!("systemctl --user kill exited 1: unit not loaded")
            .context("killing the probe scope");
        assert_eq!(classify_probe_failure(&ordinary), ProbeFailureShape::Error);
    }

    /// `describe_probe_failure` drives the operator-facing log field, so its
    /// three labels each need their own pinned case: a lone timeout must
    /// read "timeout" even without a retry ever having been attempted, two
    /// failed attempts must read "retry_failed" (the scenario the flag retry
    /// exists for), and a single non-timeout failure defaults to "error".
    #[farhelm_testtrace::test]
    fn describes_each_probe_failure_shape_by_its_own_marker() {
        let lone_timeout =
            anyhow::Error::new(QueryTimedOut).context("the probe scope never became visible");
        assert_eq!(describe_probe_failure(&lone_timeout), "timeout");

        let both_failed = anyhow::Error::new(BothProbeAttemptsFailed(anyhow::anyhow!(
            "systemctl --user kill exited 1"
        )))
        .context("and the same probe with --expand-environment=no failed too (...)");
        assert_eq!(describe_probe_failure(&both_failed), "retry_failed");

        let lone_error = anyhow::anyhow!("running /usr/bin/systemd-run: permission denied");
        assert_eq!(describe_probe_failure(&lone_error), "error");
    }
}
