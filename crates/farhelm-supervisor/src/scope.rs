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
    /// A test double: availability is fixed, nothing is actually signaled,
    /// and every operation is reported to `sink`. `kills_fail` stands in for
    /// the manager that is THERE but not working — the case stop must
    /// survive without losing anything M2 guaranteed. `vanishes_after`
    /// makes `exists` report the unit gone once that many existence checks
    /// have been answered, which is how the post-kill confirmation's two
    /// outcomes (converged, and timed out) are both reachable in a test.
    #[cfg(test)]
    Fake {
        available: bool,
        kills_fail: bool,
        vanishes_after: Option<usize>,
        exists_calls: std::sync::atomic::AtomicUsize,
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
/// record the fallback.
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
    tools: tokio::sync::OnceCell<Option<Tools>>,
}

impl std::fmt::Debug for ScopeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match self.mode {
            Mode::Systemd => "systemd",
            Mode::Disabled => "disabled",
            #[cfg(test)]
            Mode::Fake { .. } => "fake",
        };
        f.debug_struct("ScopeManager")
            .field("mode", &mode)
            .field("probed", &self.tools.get().map(Option::is_some))
            .finish()
    }
}

impl ScopeManager {
    /// The production manager: talks to the host's real systemd user
    /// instance, if it has one.
    pub fn systemd() -> ScopeManager {
        ScopeManager {
            mode: Mode::Systemd,
            tools: tokio::sync::OnceCell::new(),
        }
    }

    /// A manager that is never available — the injected fallback, see
    /// [`Mode::Disabled`].
    pub fn disabled() -> ScopeManager {
        ScopeManager {
            mode: Mode::Disabled,
            tools: tokio::sync::OnceCell::new(),
        }
    }

    /// A test double reporting every operation to `sink`; see [`ScopeOp`].
    #[cfg(test)]
    pub fn fake(available: bool, sink: ScopeOpSink) -> ScopeManager {
        ScopeManager::fake_with(available, false, None, sink)
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
        ScopeManager::fake_with(true, true, None, sink)
    }

    /// A test double whose unit disappears after `vanishes_after` existence
    /// checks — the knob the post-kill confirmation needs, since "the cgroup
    /// emptied" and "it never emptied" are otherwise indistinguishable to a
    /// caller.
    #[cfg(test)]
    pub fn fake_vanishing(vanishes_after: usize, sink: ScopeOpSink) -> ScopeManager {
        ScopeManager::fake_with(true, false, Some(vanishes_after), sink)
    }

    #[cfg(test)]
    fn fake_with(
        available: bool,
        kills_fail: bool,
        vanishes_after: Option<usize>,
        sink: ScopeOpSink,
    ) -> ScopeManager {
        ScopeManager {
            mode: Mode::Fake {
                available,
                kills_fail,
                vanishes_after,
                exists_calls: std::sync::atomic::AtomicUsize::new(0),
                sink,
            },
            tools: tokio::sync::OnceCell::new(),
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
        match &self.mode {
            Mode::Disabled => false,
            #[cfg(test)]
            Mode::Fake {
                available, sink, ..
            } => {
                // Routed through the same `OnceCell` the real path uses, so
                // the caching contract is what a test observes.
                self.tools
                    .get_or_init(|| async {
                        sink(&ScopeOp::Probe);
                        None
                    })
                    .await;
                *available
            }
            Mode::Systemd => self.tools().await.is_some(),
        }
    }

    /// The probed tools, or `None` when this host has no usable manager.
    async fn tools(&self) -> Option<&Tools> {
        self.tools
            .get_or_init(|| async {
                let tools = probe_systemd().await;
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
            })
            .await
            .as_ref()
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
            Mode::Systemd => self.tools().await?,
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
                exists_calls,
                sink,
                ..
            } => {
                sink(&ScopeOp::Exists(unit.to_string()));
                let seen = exists_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(*available && vanishes_after.is_none_or(|after| seen < after))
            }
            Mode::Systemd => {
                let tools = self.tools().await.ok_or_else(|| {
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
    /// tmux-independent half of a session teardown (see
    /// [`tab_unit_glob`]).
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
                available, sink, ..
            } => {
                sink(&ScopeOp::List(pattern.to_string()));
                // A fake owns no real units; the availability flag is what
                // a test is asserting against here.
                let _ = available;
                Ok(Vec::new())
            }
            Mode::Systemd => {
                let tools = self.tools().await.ok_or_else(|| {
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
    pub async fn kill(&self, unit: &str, signal: &str) -> anyhow::Result<()> {
        match &self.mode {
            Mode::Disabled => anyhow::bail!("no systemd user manager to kill scope {unit} with"),
            #[cfg(test)]
            Mode::Fake {
                kills_fail, sink, ..
            } => {
                sink(&ScopeOp::Kill {
                    unit: unit.to_string(),
                    signal: signal.to_string(),
                });
                if *kills_fail {
                    anyhow::bail!("injected failure sending {signal} to scope {unit}");
                }
                Ok(())
            }
            Mode::Systemd => {
                let tools = self.tools().await.ok_or_else(|| {
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
            tracing::debug!(
                error = %format!("{e:#}"),
                "the systemd user-manager probe failed; selecting the process-tree sweep alone"
            );
            None
        }
        Err(_) => {
            tracing::debug!(
                "the systemd user-manager probe did not finish within {PROBE_TIMEOUT:?}; \
                 selecting the process-tree sweep alone"
            );
            None
        }
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
async fn probe_round_trip(systemd_run: &Path, systemctl: &Path) -> anyhow::Result<bool> {
    match probe_once(systemd_run, systemctl, true).await {
        Ok(()) => Ok(true),
        Err(with_flag) => match probe_once(systemd_run, systemctl, false).await {
            Ok(()) => Ok(false),
            Err(without_flag) => Err(without_flag.context(format!(
                "and the same probe with --expand-environment=no failed too ({with_flag:#})"
            ))),
        },
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
        .map_err(|_| {
            anyhow::anyhow!(
                "the systemd user manager did not answer within {:?}",
                SYSTEMCTL_TIMEOUT
            )
        })?
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
    #[test]
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
    #[test]
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
    #[test]
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
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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
}
