//! Process-tree sweep, reap, and systemd-scope-kill machinery.
//!
//! Everything here answers one question honestly: is anything still
//! running under a session (or one of its tabs) that a stop, restart, or
//! delete needs to be sure is gone before it reports success? The
//! process-table sweep is the portable backstop that always RUNS (it can
//! still miss a descendant that scrubbed its markers — see the marker
//! docs below); the systemd scope kill (`kill_scope`/`reap_process_tree`)
//! closes that residual gap where a user manager exists. See
//! `reap_process_tree`'s own docs for how the two combine, and
//! PLAN_M3.md's process-tree-ownership section for why neither alone was
//! ever enough.
//!
//! "Portable" is now literal rather than aspirational. Every read of the
//! process table goes through [`crate::procs`] — `/proc` on Linux,
//! `sysctl` on macOS — and NOTHING in this module knows which. The PPID
//! closure, the marker union, start-time validation, the escalation, and
//! the confirmation are one implementation on both platforms, because
//! what stop promises a user must not vary by host.

use super::core::{SessionEntry, Supervisor};
use super::status::dead_pane_exit_code;
use crate::procs::{self, ProcessState};
use crate::store::Transition;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Grace period between `SIGTERM` and the SIGSTOP-quiesce step in
/// [`kill_process_tree`]. Long enough for a well-behaved agent (or an
/// MCP/dev-server child) to run its own shutdown hooks; short enough that
/// `stop`/`delete` still feel immediate to a human waiting on them.
const KILL_GRACE: Duration = Duration::from_millis(500);

/// Bounds how many SIGSTOP-and-re-enumerate rounds
/// [`kill_process_tree`]'s quiesce fixpoint runs before giving up on
/// convergence. Stopped processes cannot fork, so each round can only
/// discover pids that forked in the brief window before THIS round's
/// SIGSTOP landed; the set shrinks fast in practice, and this cap is a
/// backstop against a pathological fork storm rather than an expected
/// ceiling — exhausting it without converging is itself a reported sweep
/// failure (see that function's docs), not a silent "close enough".
const MAX_QUIESCE_PASSES: usize = 5;

/// Total time [`kill_process_tree`] polls for its SIGKILLed pids to
/// actually disappear before giving up and reporting survivors as a sweep
/// failure. `SIGKILL` cannot be blocked or ignored, so this is generous
/// purely for scheduler noise (a loaded host taking a moment to actually
/// schedule the kill), not because the kernel might refuse.
const KILL_CONFIRM_TIMEOUT: Duration = Duration::from_secs(2);

/// Poll interval within [`KILL_CONFIRM_TIMEOUT`].
const KILL_CONFIRM_STEP: Duration = Duration::from_millis(50);

/// Every farhelm marker one process's environment carries, decided in a
/// SINGLE pass over the process's environment block.
///
/// One pass rather than four because this is read for every same-uid
/// process on the host on every sweep round, and because four independent
/// reads would describe four different instants of a process that can
/// `exec` in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct EnvironMarkers {
    /// An exact `FARHELM_SESSION_ID=<session>` entry for the session being
    /// swept. Every farhelm-launched process of that session has this, and
    /// it is the only marker that has existed since M2 — which is why the
    /// legacy bucket below can lean on it alone.
    session: bool,
    /// An exact `FARHELM_AGENT_ID=<session>` entry: this process belongs
    /// to THIS session's agent launch.
    agent: bool,
    /// Any non-empty `FARHELM_AGENT_ID` entry, whatever session it names.
    ///
    /// Distinct from `agent` because it answers a different question: not
    /// "is this ours" but "has something already claimed this process for
    /// an agent". A process carrying another session's agent marker is not
    /// a pre-marker legacy process, so the legacy bucket must not claim it.
    any_agent: bool,
    /// An exact `FARHELM_TAB_ID=<tab>` entry for the tab being swept.
    /// Meaningful only when a tab is named.
    tab: bool,
    /// Any `FARHELM_TAB_ID` entry whose value is a complete minted-shaped
    /// id — the same "has something already claimed this" role `any_agent`
    /// plays. An empty or malformed value is deliberately NOT a claim: it
    /// would otherwise let anything that exports `FARHELM_TAB_ID=` shield
    /// itself from the legacy bucket.
    any_tab: bool,
}

/// Scan one process's raw environment for [`EnvironMarkers`].
///
/// Split out from [`environ_marker_verdict`] purely so this matching logic
/// is unit-testable against constructed byte buffers, without a real
/// process or a real process table behind it.
///
/// Every match is against a complete NUL-delimited entry, never a
/// substring: `environ` packs `KEY=VALUE\0KEY=VALUE\0...`, so a substring
/// match would count `FARHELM_SESSION_ID=abc-1` as containing session
/// `abc`, or misfire on an unrelated variable that merely embeds the same
/// text (`OTHER=FARHELM_SESSION_ID=abc`). The "any" forms additionally
/// require the `=`, so a variable merely PREFIXED with a marker's name
/// (`FARHELM_TAB_ID_ALT=...`) is not that marker.
fn environ_markers(environ: &[u8], session_id: &str, tab_id: Option<&str>) -> EnvironMarkers {
    let session_marker = format!("{}={session_id}", crate::launch::SESSION_ID_ENV_VAR);
    let agent_marker = format!("{}={session_id}", crate::launch::AGENT_ID_ENV_VAR);
    let agent_prefix = format!("{}=", crate::launch::AGENT_ID_ENV_VAR);
    let tab_prefix = format!("{}=", crate::launch::TAB_ID_ENV_VAR);
    let tab_marker = tab_id.map(|id| format!("{tab_prefix}{id}"));
    let mut found = EnvironMarkers::default();
    for entry in environ.split(|&b| b == 0) {
        if entry == session_marker.as_bytes() {
            found.session = true;
        }
        if entry == agent_marker.as_bytes() {
            found.agent = true;
        }
        if let Some(value) = entry.strip_prefix(agent_prefix.as_bytes())
            && !value.is_empty()
        {
            found.any_agent = true;
        }
        if let Some(value) = entry.strip_prefix(tab_prefix.as_bytes())
            && std::str::from_utf8(value).is_ok_and(crate::scope::is_uuid_shaped)
        {
            found.any_tab = true;
        }
        if tab_marker
            .as_deref()
            .is_some_and(|marker| entry == marker.as_bytes())
        {
            found.tab = true;
        }
    }
    found
}

/// Which of a session's processes one sweep claims — the marker split
/// PLAN_M4.md item 2 forces once tabs exist.
///
/// Before tabs, "the session's processes" was one set and the sweep was
/// keyed on the session's environment marker alone. Tabs break that in
/// three directions at once: SPEC.md says stopping or restarting the agent
/// leaves a tab's shell running, deleting or archiving takes everything
/// down, and closing ONE tab must reach that tab and nothing else. Those
/// are three different sets over the same marked processes, so the sweep
/// has to be told which one it is running.
///
/// ## Selection is POSITIVE, and why that matters
///
/// Each kind of launch marks itself — `FARHELM_AGENT_ID` for an agent,
/// `FARHELM_TAB_ID` for a tab — and each sweep names the marker it wants
/// rather than subtracting the marker it does not. An earlier cut did the
/// opposite (sweep the session marker, subtract anything wearing a tab
/// marker) and had a hole that appears the moment farhelm supervises
/// farhelm: an inner agent launched by a supervisor running inside
/// somebody's tab inherits that tab's marker, so exclusion filed it as a
/// tab process and its own session's stop never reaped it. See
/// [`crate::launch::AGENT_ID_ENV_VAR`] for the full argument and for the
/// scrub at each launch boundary that keeps an ambient marker from
/// crossing into a launch of the opposite kind.
///
/// ## The legacy bucket
///
/// Positive agent marking alone would silently stop reaching daemons left
/// by sessions launched BEFORE this build: they carry the session marker
/// and neither kind marker, so nothing would claim them and stop would
/// quietly get weaker on every host that upgraded. `AgentOnly` therefore
/// claims that exact shape as a third root set.
///
/// ## What the markers do NOT deliver
///
/// A process that scrubs its whole environment is invisible to every
/// variant here, and so is one that selectively scrubs exactly ONE farhelm
/// marker — a tab descendant that drops `FARHELM_TAB_ID` while keeping
/// `FARHELM_SESSION_ID` lands in the legacy bucket and is reaped by a
/// stop that was supposed to leave tabs alone. Both are the recorded
/// adversarial case: SPEC_impl.md's cgroup section says plainly that the
/// guarantee stops at ACCIDENTAL daemonization and that containing a
/// process determined to escape needs cgroups (and, past that, a
/// delegation boundary v1 does not build). Nothing here is claimed to
/// authenticate a marker's provenance; the markers describe what a
/// well-behaved process tree looks like, and the per-launch scopes are the
/// answer where that is not enough.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SweepTarget {
    /// The agent and everything under it: what `StopSession` and the
    /// restart's pre-relaunch reap claim. Three root sets — the agent
    /// pane's PPID closure (supplied by the caller), this session's agent
    /// marker, and the legacy bucket.
    AgentOnly,
    /// Every process carrying the session marker, tabs included: what
    /// `DeleteSession` (and, later, archive) claims.
    WholeSession,
    /// One tab's own processes: what `CloseTab` claims. Selection requires
    /// the session marker AND that tab's exact minted id — never a tab
    /// marker on its own, which would let a process of an unrelated
    /// session be reaped by a close it has nothing to do with.
    Tab(String),
}

impl SweepTarget {
    /// The tab id whose marker this sweep selects on, if any.
    fn selects_tab(&self) -> Option<&str> {
        match self {
            SweepTarget::Tab(id) => Some(id.as_str()),
            _ => None,
        }
    }

    /// Whether `markers` put this process in the sweep's root set.
    ///
    /// The session marker is required by every variant, which is what
    /// keeps one session's sweep from ever reaching another's processes
    /// however their other markers read.
    fn claims(&self, markers: &EnvironMarkers) -> bool {
        if !markers.session {
            return false;
        }
        match self {
            SweepTarget::AgentOnly => markers.agent || (!markers.any_agent && !markers.any_tab),
            SweepTarget::WholeSession => true,
            SweepTarget::Tab(_) => markers.tab,
        }
    }
}

/// Whether `pid`'s environment puts it in `target`'s root set for
/// `session_id` — the environment [`crate::launch::SESSION_ID_ENV_VAR`]
/// sets at launch and every descendant inherits automatically,
/// transitively, across any number of forks and execs, UNLESS a process
/// along the way deliberately scrubs or replaces its own environment
/// (`env -i`, an `exec` with an explicit empty/rebuilt envp, and the like)
/// — that residual is the one [`reap_process_tree`]'s cgroup kill closes,
/// alongside the reparented-daemon case documented on
/// [`kill_process_tree`], and remains open wherever no systemd user
/// manager exists.
///
/// A process belonging to a different user makes its environment
/// unreadable on both platforms — that failure is silently treated as "no
/// marker", which is exactly the "same-user" scoping this scan is
/// supposed to have; no separate uid check is needed because the kernel
/// already enforces it (the enumeration additionally restricts itself to
/// same-euid pids before this is ever called — see [`crate::procs`] — so
/// this scoping is belt and braces, not the only place it happens). A
/// SAME-uid process escapes this scan too if it has called
/// `prctl(PR_SET_DUMPABLE, 0)` (directly, or via a setuid/setgid exec,
/// which clears dumpability as a kernel security measure): `environ`
/// requires `PTRACE_MODE_READ`-equivalent access even from the owning
/// user once a process is non-dumpable, so it becomes just as unreadable
/// as another user's. That is the same accepted-residual bucket as the
/// environment-scrubbing case above — a legitimate hardening choice by
/// the target process defeats the marker scan, and only cgroups (or
/// running as root) close it. Unlike [`procs::read_process`], this has no
/// error path at all: every failure mode here (gone, permission,
/// non-dumpable) is routine and expected for a host-wide scan of processes
/// this sweep does not necessarily own.
///
/// macOS 26+ adds one more residual to that bucket, and it is the OS's
/// choice rather than the target's: the kernel withholds the environment
/// region of `KERN_PROCARGS2` for Apple PLATFORM binaries even from a
/// same-uid parent, so a reparented descendant exec'd into `/bin/sh` (or
/// any other platform binary) reads as marker-less there no matter what
/// its environment really holds. The PPID closure still finds such a
/// process while it remains in the pane's tree; once reparented it is
/// unreachable by this scan on that OS. The recorded follow-up is a
/// session-id membership channel (tmux panes are session leaders, and a
/// SID survives fork, exec, and reparenting), deliberately deferred —
/// see `procs::read_environ`'s docs for the observation itself.
///
/// What is scanned is the EXEC-TIME environment and only that, identically
/// on both platforms ([`crate::procs::read_environ`]'s contract): a
/// process cannot be claimed for the text on its command line, which on
/// macOS arrives in the same kernel buffer and is deliberately dropped
/// before it gets here.
///
/// An unreadable environment yielding "not claimed" is the SAFE
/// direction: a sweep never signals a process it could not identify.
fn environ_marker_verdict(pid: u32, session_id: &str, target: &SweepTarget) -> bool {
    let Some(bytes) = procs::read_environ(pid) else {
        return false;
    };
    target.claims(&environ_markers(&bytes, session_id, target.selects_tab()))
}

/// What a tab reap anchors its descendant walk on.
///
/// Not a `bool` because the two callers differ on a safety property, not
/// on an optimization: see [`Supervisor::reap_tab_tree`]'s `anchor` docs
/// for the sibling-tab reap that asking about an already-destroyed pane
/// produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabReapAnchor {
    /// Look the pane up and use its process as the closure's root when it
    /// is alive — what a reap performed while the window still exists
    /// does.
    PaneIfLive,
    /// Do not look the pane up at all: the caller knows it is gone.
    MarkerOnly,
}

/// One process-table walk's findings: every readable process's (ppid,
/// start-time), plus which of those carry `session_id`'s environment
/// marker. The marker scan is driven BY the walk's own results — a pid
/// the walk could not read is never asked about its environment — which
/// is why the two halves cannot disagree about which processes exist, and
/// costs nothing: [`enumerate_tree`] drops a marked pid with no
/// start-time anyway, since there would be no identity to validate later.
/// Neither half is, importantly, taken at one consistent instant in time;
/// see [`snapshot_proc`]'s own docs on that.
struct ProcSnapshot {
    /// pid → (ppid, starttime). Absent means this process's row could not
    /// be read because it exited mid-scan — the ordinary, tolerated case
    /// ([`procs::snapshot`] simply omits it). A partial map built
    /// this way only ever UNDER-collects candidates for the PPID closure
    /// below; it can never invent an ancestor relationship that does not
    /// exist. That under-collection is a SEPARATE concern from pid
    /// reuse, which this map does nothing to close on its own: a pid
    /// found here can still have been reused by the time it is acted on
    /// later (after a sleep, a signal round, another walk).
    /// [`signal_validated`] is what actually closes that gap, by
    /// re-checking start-time at the moment of signaling regardless of
    /// how fresh or stale the value recorded here is.
    ///
    /// Accepted residual, and NOT one the cgroup hardening addresses
    /// (a cgroup kill names a unit, never a pid, so it neither creates
    /// nor fixes this): this map is
    /// still just ONE sequential pass, not an atomic system-wide
    /// snapshot, so in principle a pid could exit and be recycled to an
    /// unrelated process WHILE this same walk is still in progress — an
    /// edge this one snapshot cannot itself detect or rule out. Two
    /// things bound how much that can matter rather than eliminating it
    /// outright: `signal_validated` re-reads and re-checks identity at
    /// the actual moment of signaling (so a within-this-walk recycling
    /// only risks a wrong PPID edge being followed, never an unvalidated
    /// signal reaching the wrong process), and `kill_process_tree`
    /// re-runs the whole closure on every later round rather than
    /// trusting this one walk's shape indefinitely. The remainder — a
    /// closure edge briefly followed on the strength of a since-recycled
    /// pid, within one walk, before the next walk or signal re-validates
    /// it — is accepted as the honest cost of a single-pass enumeration.
    stats: procs::ProcessTable,
    /// pids this sweep's markers CLAIMED in this same walk, per
    /// [`SweepTarget::claims`] — positive selection throughout, so a
    /// process is here because something said it belongs to this sweep,
    /// never merely because nothing excluded it.
    marked: HashSet<u32>,
}

/// Walk the process table once, in full, for `session_id`, and decide
/// which of the processes found carry this sweep's markers.
///
/// Fails closed, and that propagates straight out of [`procs::snapshot`]:
/// a problem that stops the walk from happening at all is an `Err`, never
/// an empty and falsely-clean snapshot — silently reporting "found
/// nothing" when the walk never actually ran is exactly the failure mode
/// this module exists to avoid. A problem confined to ONE process is
/// collected into the returned error list rather than aborting the whole
/// walk, so one unreadable row does not blind this scan to every other
/// process in the tree.
///
/// The walk itself is a plain sequential scan, not an atomic system-wide
/// snapshot: a fork happening WHILE it runs may or may not be visible to
/// it, so one walk can under-report a tree that is still actively forking
/// — a sequential-scan race, not (as an earlier version of this comment
/// mis-described) some impossible ordering between a parent and a child
/// that does not exist yet. [`kill_process_tree`] is what actually
/// compensates for this: it re-walks between each signal phase rather than
/// trusting one walk to see a tree that can still be growing.
fn snapshot_proc(
    session_id: &str,
    target: &SweepTarget,
) -> Result<(ProcSnapshot, Vec<String>), String> {
    let (stats, soft_errors) = procs::snapshot()?;
    // Only pids the walk actually read are asked about their environment.
    // That read is the expensive half — one syscall and a kilobyte or so
    // per process, host-wide, on every round of every sweep — and a marked
    // pid the walk has no start-time for is discarded by `enumerate_tree`
    // anyway, for want of an identity to re-validate before signaling.
    let marked = stats
        .keys()
        .copied()
        .filter(|&pid| environ_marker_verdict(pid, session_id, target))
        .collect();
    Ok((ProcSnapshot { stats, marked }, soft_errors))
}

/// One enumeration for [`kill_process_tree`]: the transitive PPID closure
/// expanded from EVERY root — the pane's process (`root_pid`, only ever
/// meaningful on the very first call; see below), every process carrying
/// `session_id`'s environment marker, and every `(pid, starttime)`
/// identity in `seeds` (everything a PREVIOUS round of this same kill
/// already found).
///
/// Marker pids are folded into the closure's STARTING set, before the
/// closure expands — not appended as leaves afterward — because a
/// reparented daemon can itself have gone on to spawn further children
/// after reparenting; those children are only reachable if the daemon
/// itself is a root the closure walks from, not a leaf the walk never
/// continues past.
///
/// `root_pid` carries no prior identity to validate on its first use —
/// this is the moment its (pid, starttime) pair gets established, by
/// trusting whatever the snapshot reports for it right now (it was just
/// queried fresh from tmux). Every LATER round passes `root_pid: None`
/// and relies on `seeds` instead, because by then the root's identity, if
/// it was found, is already sitting in `seeds` (folded in by
/// `kill_process_tree` reusing its own previous result) — from that point
/// on it is validated exactly like any other seed: a seed pid whose
/// CURRENT starttime does not match the recorded one is dropped outright
/// (that process is gone, or a different one now wears its number), and
/// re-enters the result only if independently rediscovered via the PPID
/// closure or the marker scan this same round.
///
/// Returns pid → starttime for everything found in THIS one walk, plus
/// any soft per-pid errors `snapshot_proc` collected — the values a
/// caller re-validates via [`signal_validated`] before ever signaling, so
/// nothing here is trusted past the moment it was read.
///
/// `target` decides which markers admit a process as a ROOT (PLAN_M4.md
/// item 2's marker split — see [`SweepTarget`] for the three sets and why
/// selection is positive rather than subtractive). It changes nothing
/// about the closure itself: a process reached only by walking down from
/// a root really is a descendant of something this sweep claims, whatever
/// markers it happens to wear.
fn enumerate_tree(
    root_pid: Option<u32>,
    session_id: &str,
    seeds: &HashMap<u32, u64>,
    target: &SweepTarget,
) -> Result<(HashMap<u32, u64>, Vec<String>), String> {
    let (snapshot, soft_errors) = snapshot_proc(session_id, target)?;
    let mut found: HashMap<u32, u64> = HashMap::new();

    // Marker pids first: roots the closure expands FROM.
    for &pid in &snapshot.marked {
        if let Some(&(_, starttime)) = snapshot.stats.get(&pid) {
            found.insert(pid, starttime);
        }
    }
    // The pane root, establishing its identity fresh on round one only.
    // Always admitted: it is the terminal this sweep was called FOR, named
    // by the caller rather than discovered by a marker.
    if let Some(pid) = root_pid
        && let Some(&(_, starttime)) = snapshot.stats.get(&pid)
    {
        found.insert(pid, starttime);
    }
    // Every previously-found identity, re-validated against this walk.
    for (&pid, &starttime) in seeds {
        if snapshot.stats.get(&pid).map(|&(_, st)| st) == Some(starttime) {
            found.insert(pid, starttime);
        }
    }

    loop {
        let mut grew = false;
        for (&pid, &(ppid, starttime)) in &snapshot.stats {
            if found.contains_key(&ppid) && found.insert(pid, starttime).is_none() {
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    Ok((found, soft_errors))
}

/// Re-read `pid`'s start-time and signal it only if that read still
/// matches `expected_starttime` — the barrier that makes a pid recorded
/// at some earlier moment (before a sleep, a signal round, or a
/// re-enumeration) safe to act on now. If the original process already
/// exited and the kernel handed its number to something unrelated, the
/// start-time will have moved on (or the pid will be gone outright), and
/// this refuses to touch whatever now holds that number.
///
/// Returns `Ok(())` for every outcome that is NOT a genuine failure worth
/// reporting: the pid being gone entirely, its start-time no longer
/// matching (pid reused), and `ESRCH` from the signal call itself (a
/// benign race against the process's own concurrent exit — a DIFFERENT
/// check from the "gone" [`procs::read_process`] reports, since `ESRCH`
/// here is `kill`'s own "no such process" errno for a pid that was still
/// listed a moment ago). Any other read failure, or a signal errno other
/// than `ESRCH` (`EPERM`, chiefly), comes back as `Err`: both mean a
/// process this sweep was supposed to reach could not be confirmed
/// reachable, which the caller must learn about rather than silently
/// treat as handled.
fn signal_validated(pid: u32, expected_starttime: u64, signal: i32) -> Result<(), String> {
    match procs::read_process(pid) {
        Ok(Some((_, starttime, _state))) if starttime == expected_starttime => {
            // SAFETY: `libc::kill` validates `pid` itself; passing a pid
            // this process does not own simply yields EPERM, handled
            // below like any other non-ESRCH errno. No memory safety
            // concern either way.
            let ret = unsafe { libc::kill(pid as libc::pid_t, signal) };
            if ret != 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() != Some(libc::ESRCH) {
                    return Err(format!("signaling pid {pid} with signal {signal}: {err}"));
                }
            }
            Ok(())
        }
        // Gone, or a different process now wears this number: neither is
        // ours to signal, and neither is an error.
        Ok(_) => Ok(()),
        Err(e) => Err(format!("re-validating pid {pid} before signaling: {e}")),
    }
}

/// Validate-and-signal every `(pid, starttime)` pair, continuing through
/// the whole set even when some fail — a single unsignalable process
/// (`EPERM`, say) must not stop the rest of the sweep from at least being
/// attempted — and returning every failure message collected along the
/// way for the caller to aggregate.
fn signal_all(pids: &HashMap<u32, u64>, signal: i32) -> Vec<String> {
    let mut errors = Vec::new();
    for (&pid, &starttime) in pids {
        if let Err(e) = signal_validated(pid, starttime, signal) {
            errors.push(e);
        }
    }
    errors
}

/// Enumerate one round for [`kill_process_tree`], folding `fallback` (the
/// previous round's result) in as this round's seeds — see
/// [`enumerate_tree`]'s docs for what that buys. `root_pid` should be
/// `Some` only on the very first call; every later round passes `None`
/// and lets `fallback` carry the root's identity forward instead, exactly
/// like every other seed.
///
/// A hard enumeration failure (an unreadable process table, or the
/// blocking task itself panicking) is recorded into `errors` rather than
/// aborting the sweep, and `fallback` is returned unchanged so the caller
/// still has SOMETHING to signal. Reusing stale starttimes here is safe:
/// whoever actually signals re-validates them fresh via
/// [`signal_validated`] regardless of how old the value passed in is.
async fn enumerate_or_reuse(
    root_pid: Option<u32>,
    session_id: &str,
    fallback: &HashMap<u32, u64>,
    target: &SweepTarget,
    errors: &mut Vec<String>,
) -> HashMap<u32, u64> {
    let session_id = session_id.to_string();
    let seeds = fallback.clone();
    let target = target.clone();
    match tokio::task::spawn_blocking(move || {
        enumerate_tree(root_pid, &session_id, &seeds, &target)
    })
    .await
    {
        Ok(Ok((found, soft_errors))) => {
            errors.extend(soft_errors);
            found
        }
        Ok(Err(e)) => {
            errors.push(format!("enumerating process tree: {e}"));
            fallback.clone()
        }
        Err(e) => {
            errors.push(format!("process enumeration task panicked: {e}"));
            fallback.clone()
        }
    }
}

/// Poll every identity in `found` until each has been CONFIRMED gone — a
/// starttime mismatch, an outright-gone process row, or a zombie still
/// awaiting an ancestor's reap (nothing this sweep does can force a reap,
/// and a zombie cannot run anything regardless, so waiting for one to be
/// reaped before declaring success would fail a sweep for a reason no
/// amount of signaling could ever fix) — or `timeout` elapses, whichever
/// comes first.
///
/// This exists because `SIGKILL` succeeding only proves the signal was
/// DELIVERED, not that the kernel has finished tearing the process down
/// by the time [`kill_process_tree`] returns; a caller trusting `Ok(())`
/// to mean "the tree is gone" deserves that to be actually true, not just
/// "the last signal in the sequence did not error".
///
/// The error direction matters as much as the confirmation logic: only
/// the three cases above count as confirmed-absent. A read failure
/// that is NOT one of those (a permission problem this process should not
/// see for a pid it is supposed to own, a malformed row) is a genuine
/// confirmation failure and is reported as an error — never silently
/// folded into "gone", which would let a sweep claim success over a pid
/// nobody actually confirmed dead. Likewise a panic in the polling task
/// itself aborts this function with a reported error rather than treating
/// the unexamined remainder as gone by default (the bug a bare
/// `.unwrap_or_default()` on the task join would otherwise hide).
async fn confirm_gone(found: &HashMap<u32, u64>, timeout: Duration) -> Vec<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut remaining = found.clone();
    let mut errors = Vec::new();
    loop {
        let still_alive = remaining.clone();
        let poll = tokio::task::spawn_blocking(move || {
            let mut alive = HashMap::new();
            let mut poll_errors = Vec::new();
            for (pid, starttime) in still_alive {
                match procs::read_process(pid) {
                    // Identity still matches and the process has not yet
                    // become a zombie: genuinely still alive.
                    Ok(Some((_, st, state)))
                        if st == starttime && state != ProcessState::Zombie =>
                    {
                        alive.insert(pid, starttime);
                    }
                    // Gone outright, a different process now wears this
                    // pid, or a zombie: all three are confirmed absence.
                    Ok(_) => {}
                    // A real read/parse problem: not confirmed either
                    // way, and must not be silently counted as gone.
                    Err(e) => poll_errors.push(format!("confirming pid {pid} is gone: {e}")),
                }
            }
            (alive, poll_errors)
        })
        .await;
        let (alive, poll_errors) = match poll {
            Ok(result) => result,
            Err(e) => {
                // The polling task itself panicked: nothing this round
                // was actually confirmed, which is a hard failure to
                // report, not license to assume success for whatever was
                // still pending.
                errors.push(format!("kill-confirmation polling task panicked: {e}"));
                errors.extend(remaining.keys().map(|&pid| {
                    format!("pid {pid} could not be confirmed gone: polling task panicked")
                }));
                return errors;
            }
        };
        errors.extend(poll_errors);
        remaining = alive;
        if remaining.is_empty() || tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(KILL_CONFIRM_STEP).await;
    }
    errors.extend(
        remaining
            .keys()
            .map(|&pid| format!("pid {pid} still alive {timeout:?} after SIGKILL")),
    );
    errors
}

/// Kill one terminal's entire process tree (SPEC.md: stop/delete reap the
/// agent and every descendant — MCP servers, dev servers, anything it
/// started; close does the same for one tab's shell), per the sequence
/// lore/2026-07-27-m2-process-tree-stop.md settled on after simpler cuts
/// proved insufficient:
///
/// 1. Enumerate (PPID closure from `root_pid` if any, unioned with the
///    environment-marker scan for `session_id`) and SIGTERM the result.
/// 2. After a grace period, re-enumerate (seeded with everything round 1
///    found, so a reparented survivor is not lost) and SIGSTOP it —
///    freezing every survivor before the fixpoint below closes the
///    fork-during-teardown race: a `SIGTERM` handler that forks a child
///    and then exits would otherwise let that child slip past a kill that
///    only ever looks at what existed at signal time.
/// 3. Re-enumerate to a bounded fixpoint (stopped processes cannot fork,
///    so this converges): any pid not seen in the previous round is new —
///    a fork that landed in the gap before this round's SIGSTOP — and
///    gets SIGSTOPped too, so nothing sneaks past quiescence. Exhausting
///    [`MAX_QUIESCE_PASSES`] while a pass STILL found something new is
///    itself a reported failure — a sweep that never converged cannot
///    honestly claim to have frozen the whole tree.
/// 4. SIGKILL everyone found, stopped or not (`SIGKILL` terminates a
///    stopped process unconditionally, so no `SIGCONT` step is needed
///    first), then poll until every one of them has actually disappeared
///    (see [`confirm_gone`]).
///
/// `target` says WHICH of the session's processes this sweep claims —
/// the agent's with tabs subtracted, the whole session, or one tab (see
/// [`SweepTarget`], and PLAN_M4.md item 2 for why the split exists at
/// all). It changes only which processes the marker scan admits as roots
/// and which the closure may expand through; the five-phase escalation
/// below is identical for all three.
///
/// `root` is the pane process's `(pid, starttime)` as its caller validated
/// it, and `None` for a dead or absent pane, or a terminal-less
/// (restart-gap) entry: there is no live pid worth trusting in any of
/// those cases (a dead pane's remembered pid may already be recycled),
/// but SPEC.md still assigns reaping any leftover descendants of a PAST
/// run to the session's next stop or delete — and the environment-marker
/// scan is the only mechanism that can still find such a survivor once
/// there is no live pane process to walk ancestry from at all. So this is
/// called on every stop and delete, not only when the pane looks alive;
/// `None` simply means the PPID closure has nothing to seed itself with
/// beyond the marker scan's own findings.
///
/// Every signal is starttime-validated (`signal_validated`) — a pid
/// carried across the grace period, the quiesce fixpoint, or simply
/// reused between rounds is never signaled unless a fresh read of the
/// process table still agrees it is the same process this sweep found.
///
/// Errors accumulate rather than short-circuit: enumeration failures,
/// non-`ESRCH` signal errors, non-convergence, and post-SIGKILL survivors
/// are all collected while the sweep otherwise runs to completion
/// (signaling everyone it can), and only then does this return the
/// aggregated failure — a caller must not conclude "half the tree died so
/// the rest can wait" from an early return. `Ok(())` is the caller's only
/// license to treat the sweep as complete.
///
/// Residual, and the reason [`reap_process_tree`] exists on top of this:
/// a descendant that `exec`'d with a scrubbed environment after
/// reparenting to init is invisible to both the PPID closure (wrong
/// parent) and the marker scan (marker gone), and survives this sweep. On
/// a host with a systemd user manager the launch's cgroup catches it
/// before this ever runs; on a host without one, that gap is still open
/// and is exactly what M2 shipped with. Nothing in this function changed
/// with the cgroup work — it remains the whole mechanism wherever no
/// manager exists, and the backstop everywhere else, so this is still the
/// function to reason about when asking what stop guarantees at minimum.
async fn kill_process_tree(
    root: Option<(u32, u64)>,
    session_id: &str,
    target: &SweepTarget,
) -> anyhow::Result<()> {
    let mut errors: Vec<String> = Vec::new();

    // The root enters as a SEED — an identity, not a bare number — so it is
    // validated exactly like every other carried-forward pid. The identity
    // was captured by the caller before anything was killed
    // (`reap_process_tree`), which matters now that a cgroup kill and its
    // grace period can run first: a pane pid read before that window and
    // trusted after it could name a completely unrelated process by the
    // time this walk starts.
    let seed: HashMap<u32, u64> = root.into_iter().collect();
    let mut found = enumerate_or_reuse(None, session_id, &seed, target, &mut errors).await;
    errors.extend(signal_all(&found, libc::SIGTERM));

    tokio::time::sleep(KILL_GRACE).await;

    found = enumerate_or_reuse(None, session_id, &found, target, &mut errors).await;
    errors.extend(signal_all(&found, libc::SIGSTOP));

    let mut converged = false;
    for _ in 0..MAX_QUIESCE_PASSES {
        let next = enumerate_or_reuse(None, session_id, &found, target, &mut errors).await;
        // Identity, not just pid: a pid present in BOTH `found` and `next`
        // but with a DIFFERENT starttime is not the same process anymore
        // — the old one died and the kernel already recycled its number —
        // so it counts as newly found here and gets SIGSTOPped like any
        // other survivor, rather than being silently treated as already
        // handled because its number happened to repeat.
        let newly_found: HashMap<u32, u64> = next
            .iter()
            .filter(|&(&pid, &starttime)| found.get(&pid) != Some(&starttime))
            .map(|(&pid, &starttime)| (pid, starttime))
            .collect();
        found = next;
        if newly_found.is_empty() {
            converged = true;
            break;
        }
        errors.extend(signal_all(&newly_found, libc::SIGSTOP));
    }
    if !converged {
        errors.push(format!(
            "quiesce did not converge within {MAX_QUIESCE_PASSES} passes; the process tree may \
             not be fully frozen"
        ));
    }

    errors.extend(signal_all(&found, libc::SIGKILL));
    errors.extend(confirm_gone(&found, KILL_CONFIRM_TIMEOUT).await);

    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("process-tree kill hit {}", summarize_errors(&errors))
    }
}

/// The scope unit one launch runs in, or `None` when it selected the
/// portable sweep alone (or when its id cannot name a unit at all).
///
/// The ONE place a unit name is produced for an existing launch, so the
/// derivation cannot drift between the code that records a selection and
/// the code that acts on it. Derived rather than read back from the store
/// on purpose: see `store::StoredSession::launch_scoped`.
pub(crate) fn launch_scope_unit(id: &str, generation: i64, scoped: bool) -> Option<String> {
    scoped
        .then(|| crate::scope::unit_name(id, generation))
        .flatten()
}

/// Reap one launch's whole process tree: its cgroup scope first, then —
/// always, unconditionally — the portable sweep (PLAN_M3.md item 10).
///
/// This is the ONE entry point every stop, delete, and restart-reap goes
/// through, which is what makes "the sweep still runs" a property of the
/// code rather than a rule call sites are asked to remember.
///
/// # Why both, in this order
///
/// SPEC_impl.md's belt-and-suspenders rule. The scope kill is strictly
/// MORE complete than the sweep — cgroup membership is inherited across
/// fork and exec and cannot be shed, so it catches the double-forked,
/// environment-scrubbed descendant lore/2026-07-27-m2-process-tree-stop.md
/// names as the sweep's one blind spot — but it is also strictly NARROWER:
/// it can only reach what this launch's `systemd-run` actually placed in
/// the cgroup. A process that carries the session marker but was never in
/// the scope (a survivor of an earlier, unscoped launch of the same
/// session; anything the user started with the marker in its environment)
/// is invisible to it and visible to the sweep. Neither subsumes the
/// other, so both run, and the scope runs first because a cgroup kill is
/// one round trip that empties most of the tree before the sweep pays for
/// enumerating it.
///
/// # Why the scope may never fail a stop
///
/// The sweep's verdict is the whole answer. `Ok(())` from this function
/// means the sweep confirmed nothing is left running, and that is exactly
/// what it meant in M2 — item 10's "absence of a manager never degrades
/// stop below M2's guarantees" read in the other direction: PRESENCE of a
/// broken manager must not degrade it either. A scope kill that failed
/// (unit already collected, manager not answering) is therefore logged and
/// carried into the error only when the sweep ALSO failed, where it is
/// diagnostic context for a stop that is genuinely unconfirmed.
///
/// `units` are the scopes to kill before the sweep — normally the one
/// RECORDED for the launch being torn down, and for a DELETE additionally
/// every open tab's own scope (PLAN_M4.md item 2), since delete is the one
/// teardown that claims tabs too and a tab's cgroup is the only mechanism
/// that reaches an environment-scrubbing double-fork under it. Existence
/// is re-checked before each use because the names are re-derived rather
/// than stored, and a unit may long since have been garbage-collected
/// (item 10's reload/restart interplay). An empty slice means this
/// teardown has no cgroup to lean on at all, which is the ordinary state
/// on a host with no user manager. A NON-empty slice on such a host is
/// also ordinary — delete derives tab unit names unconditionally — and is
/// skipped wholesale rather than asked about; see the availability check
/// below for why that skip is safe.
pub(crate) async fn reap_process_tree(
    scopes: &crate::scope::ScopeManager,
    units: &[String],
    root_pid: Option<u32>,
    session_id: &str,
    target: &SweepTarget,
) -> anyhow::Result<()> {
    // Captured BEFORE anything is killed, and this ordering is the whole
    // point of doing it here rather than inside the sweep: `kill_scope`
    // below sends SIGTERM and then sleeps out a grace period, during which
    // the pane's process can die and the kernel can hand its number to
    // something unrelated. A bare pid read before that window and trusted
    // after it is exactly how a sweep signals a stranger.
    let root = root_pid.and_then(|pid| match procs::read_process(pid) {
        Ok(Some((_, starttime, _))) => Some((pid, starttime)),
        // Already gone, or unreadable: either way there is no identity to
        // carry, and the marker scan is what finds this session's
        // processes from here on.
        Ok(None) => None,
        Err(e) => {
            debug!(
                session = %session_id, pid, error = %e,
                "could not read the pane process's start time before reaping; the sweep will \
                 rely on the marker scan alone for it"
            );
            None
        }
    });

    if units.is_empty() {
        debug!(
            session = %session_id,
            "no cgroup scope recorded for this teardown; the process-tree sweep is the \
             whole mechanism"
        );
    }
    // A non-empty unit list on a host with NO user manager is ordinary,
    // not contradictory: delete derives every open tab's unit name without
    // asking whether that tab was ever scoped (see `teardown_session`),
    // precisely so a unit that MIGHT exist is never skipped. Where the
    // availability probe says no manager exists, none of those names can
    // possibly have a unit behind them — there is nothing to ask, and
    // asking anyway converts each name into a string of "no systemd user
    // manager" errors plus a full `SCOPE_CONFIRM_TIMEOUT` poll, turning
    // every macOS (and manager-less Linux) delete slow and noisy for
    // nothing. Skipping is safe for the same reason the probe's timeout
    // is: the sweep below runs regardless and its verdict is the whole
    // answer. A manager that probed AVAILABLE but fails mid-teardown is
    // deliberately NOT skipped — those failures stay visible as
    // diagnostic context for a stop the sweep could not confirm.
    let units: &[String] = if units.is_empty() || scopes.available().await {
        units
    } else {
        debug!(
            session = %session_id,
            "this host has no systemd user manager, so the recorded scope names cannot \
             have units behind them; the process-tree sweep is the whole mechanism"
        );
        &[]
    };
    // Every unit is killed even when an earlier one failed, for
    // `kill_scope`'s own accumulate-never-short-circuit reason one level
    // up: a delete that stopped at the first unresponsive tab scope would
    // skip the agent's, which is the one most likely to have worked.
    let mut scope_errors: Vec<String> = Vec::new();
    for unit in units {
        if let Err(e) = kill_scope(scopes, unit, session_id).await {
            scope_errors.push(format!("{e:#}"));
        }
    }
    let scope_error = (!scope_errors.is_empty()).then(|| scope_errors.join("; "));

    match (
        kill_process_tree(root, session_id, target).await,
        scope_error,
    ) {
        (Ok(()), Some(e)) => {
            // The sweep proved the tree is gone, so this stop is complete
            // by M2's own standard; the scope's trouble is worth knowing
            // about but is not the user's problem.
            warn!(
                session = %session_id, error = %e,
                "a cgroup scope could not be fully torn down, but the process-tree \
                 sweep confirmed nothing is left running"
            );
            Ok(())
        }
        (Ok(()), None) => Ok(()),
        (Err(sweep), Some(e)) => Err(sweep.context(format!(
            "a cgroup scope also could not be fully torn down ({e})"
        ))),
        (Err(sweep), None) => Err(sweep),
    }
}

/// How long [`kill_scope`] waits for a killed unit to actually go away
/// before reporting the cgroup unconfirmed.
///
/// `systemctl kill` returns once the signals are DELIVERED, which is not
/// the same as the cgroup being empty — the same distinction
/// [`confirm_gone`] exists for on the sweep's side, and for the same
/// reason: an uncollected unit means processes this stop was supposed to
/// end may still be running. Generous enough for an ordinary teardown,
/// bounded because the caller (a user waiting on a stop, or a restart
/// about to relaunch) cannot wait forever on a manager that has stopped
/// collecting units.
const SCOPE_CONFIRM_TIMEOUT: Duration = Duration::from_secs(2);

/// Poll interval within [`SCOPE_CONFIRM_TIMEOUT`].
const SCOPE_CONFIRM_POLL: Duration = Duration::from_millis(50);

/// SIGTERM the whole scope, wait out the same grace the sweep gives,
/// SIGKILL it, and confirm the unit actually went away — the existing
/// escalation, mapped onto cgroup operations.
///
/// The mapping is deliberately partial, and honestly so. `kill_process_tree`
/// has five phases (TERM, grace, SIGSTOP-quiesce to a fixpoint, KILL,
/// confirm); this has four, because quiescing has nothing to do here:
/// it exists to stop a dying process from forking a child the NEXT
/// enumeration would miss, and a cgroup needs no enumeration — a child
/// forked during teardown lands in the same cgroup its parent is in and
/// dies to the same `systemctl kill`. Every other phase carries its
/// meaning across: a well-behaved agent still gets [`KILL_GRACE`] to exit
/// on SIGTERM before anything unkillable happens to it, and the unit's
/// disappearance is confirmed rather than assumed, because `systemctl
/// kill` returning only proves delivery.
///
/// # Errors accumulate; nothing short-circuits
///
/// Every step runs even when an earlier one failed, and the failures are
/// aggregated. This is not defensive style — a transient failure at the
/// existence check or the SIGTERM used to `?` straight out of this
/// function, skipping the SIGKILL that is the step most likely to have
/// worked, and leaving a cgroup full of processes the caller was told
/// nothing about. The one exception is a unit the manager reports as
/// ALREADY GONE, which is a definitive answer rather than a failure and
/// ends the escalation with nothing to do.
async fn kill_scope(
    scopes: &crate::scope::ScopeManager,
    unit: &str,
    session_id: &str,
) -> anyhow::Result<()> {
    let mut errors: Vec<String> = Vec::new();
    match scopes.exists(unit).await {
        Ok(false) => {
            // Ordinary, not exceptional: `--collect` disposes of a scope
            // the moment its last process exits, so every stop of an
            // already-exited agent takes this path.
            debug!(
                session = %session_id, unit,
                "the launch's cgroup scope is already gone; the process-tree sweep is the \
                 whole mechanism for this stop"
            );
            return Ok(());
        }
        Ok(true) => {}
        // "Cannot tell" is not "gone": the escalation proceeds, because
        // refusing to signal a unit that may well be full of processes is
        // the strictly worse failure.
        Err(e) => errors.push(format!("checking whether scope {unit} still exists: {e:#}")),
    }
    info!(
        session = %session_id, unit,
        "killing the launch's cgroup scope before the backstop process-tree sweep"
    );
    if let Err(e) = scopes.kill(unit, "SIGTERM").await {
        errors.push(format!("{e:#}"));
    }
    tokio::time::sleep(KILL_GRACE).await;
    // Re-checked rather than killed unconditionally, unlike the sweep's own
    // SIGKILL (which signals pids, where an already-dead one is a harmless
    // ESRCH). Here the polite case is the COMMON case: an agent that exits
    // on SIGTERM empties its cgroup, `--collect` retires the unit
    // immediately, and a SIGKILL aimed at a retired unit is a hard
    // `systemctl` error — which would make every clean stop report a scope
    // failure. A check that itself fails falls through to the kill, since
    // the honest response to "cannot tell" is to try.
    if scopes.exists(unit).await.unwrap_or(true)
        && let Err(e) = scopes.kill(unit, "SIGKILL").await
    {
        errors.push(format!("{e:#}"));
    }
    if let Err(e) = confirm_scope_gone(scopes, unit).await {
        errors.push(format!("{e:#}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(
            "tearing down cgroup scope {unit} hit {}",
            summarize_errors(&errors)
        )
    }
}

/// Poll until `unit` is gone, or [`SCOPE_CONFIRM_TIMEOUT`] elapses.
///
/// A unit that outlives its SIGKILL is a cgroup that still holds
/// processes — systemd retires a `--collect` scope as soon as its last
/// member exits — so "still there" is the one answer that must not be
/// read as success. A manager that cannot be ASKED is reported for the
/// same reason: an unconfirmed teardown is unconfirmed either way.
async fn confirm_scope_gone(scopes: &crate::scope::ScopeManager, unit: &str) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + SCOPE_CONFIRM_TIMEOUT;
    loop {
        // Carried out of the match rather than accumulated across rounds, so
        // the message on timeout describes THIS round's reason — a unit that
        // is still loaded, or a manager that stopped answering — and never a
        // stale one from a condition that has since changed.
        let why_not_gone = match scopes.exists(unit).await {
            Ok(false) => return Ok(()),
            Ok(true) => None,
            Err(e) => Some(e),
        };
        if tokio::time::Instant::now() >= deadline {
            return Err(match why_not_gone {
                Some(e) => e.context(format!(
                    "could not confirm scope {unit} was collected within \
                     {SCOPE_CONFIRM_TIMEOUT:?}"
                )),
                None => anyhow::anyhow!(
                    "scope {unit} was still loaded {SCOPE_CONFIRM_TIMEOUT:?} after its SIGKILL, \
                     so its cgroup may still hold processes"
                ),
            });
        }
        tokio::time::sleep(SCOPE_CONFIRM_POLL).await;
    }
}

/// Bounds how many individual error strings [`summarize_errors`] includes
/// verbatim, so a pathological process table (a fork storm producing thousands
/// of unsignalable pids, say) cannot build an unbounded — and, chained
/// through `ControlMsg::Error`, potentially oversized — reply out of this
/// module's own error aggregation. The total count is always reported
/// even when the detail list is truncated, so a caller still learns HOW
/// BAD the sweep was, just not every last message.
const MAX_REPORTED_ERRORS: usize = 20;

/// Render an aggregated error list as `"N error(s): msg; msg; ..."`,
/// truncating the detail strings at [`MAX_REPORTED_ERRORS`] while always
/// stating the true total.
fn summarize_errors(errors: &[String]) -> String {
    let total = errors.len();
    let shown = errors
        .iter()
        .take(MAX_REPORTED_ERRORS)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    if total > MAX_REPORTED_ERRORS {
        format!(
            "{total} error(s), first {MAX_REPORTED_ERRORS} shown: {shown} \
             ({} more omitted)",
            total - MAX_REPORTED_ERRORS
        )
    } else {
        format!("{total} error(s): {shown}")
    }
}

/// Why a stop of a LIVE agent did not complete cleanly, split by what the
/// caller may still conclude about the agent — see [`stop_live_agent`].
///
/// The three are genuinely different answers, not shades of one failure:
/// after `Unrecorded` nothing was killed at all, after `Sweep` the agent
/// may still be running, and after `UnrecordedOutcome` it is provably
/// stopped and only the bookkeeping is behind. A restart may proceed past
/// none of the first two and (with a warning) past the third.
pub(crate) enum StopFailure {
    /// The durable stop intent could not be written, so the sweep never
    /// ran. This is the window the intent exists to close (a crash mid-kill
    /// silently converting "the user stopped this" into "the agent finished
    /// on its own"), which is why it refuses rather than killing anyway.
    Unrecorded(anyhow::Error),
    /// The kill sweep itself could not be confirmed complete — not "nothing
    /// was found to kill". A caller must be able to tell those apart
    /// (`ControlMsg::StopSession`'s docs).
    Sweep(anyhow::Error),
    /// The tree IS stopped; recording that outcome failed, so the session
    /// may list as a plain exit rather than an annotated one.
    UnrecordedOutcome(anyhow::Error),
}

impl StopFailure {
    /// The user-facing wording for this failure, worded so the reader can
    /// tell what actually happened to their agent. Kept here rather than at
    /// the call sites so `StopSession` and restart report the same failure
    /// the same way.
    pub(crate) fn message(&self) -> String {
        match self {
            StopFailure::Unrecorded(e) => {
                format!("recording the stop failed, so nothing was killed: {e:#}")
            }
            StopFailure::Sweep(e) => format!("{e:#}"),
            StopFailure::UnrecordedOutcome(e) => format!(
                "the session was stopped, but recording that outcome failed, so it may list \
                 as a plain exit: {e:#}"
            ),
        }
    }
}

/// Stop an agent this caller has just observed ALIVE: the durable intent,
/// the process-tree sweep, and the outcome — PLAN_M3.md item 4's stop
/// lifecycle, shared by `StopSession` and by item 9's restart.
///
/// The ORDER is the contract, and it is why this is one function rather
/// than three calls each caller makes for itself:
///
/// - The intent commits BEFORE a single signal is sent. `kill_process_tree`
///   runs for seconds — SIGTERM, a grace period, re-enumeration, SIGKILL —
///   and a crash anywhere in there used to leave a session the next startup
///   read as a plain exit, silently converting "the user stopped this" into
///   "the agent finished on its own". With the intent recorded first,
///   reload reconciles it: a dead pane means the stop landed (annotated
///   exit), a live one means it never did (intent cleared), and a reboot
///   straddling it interrupts like any other live session.
/// - The outcome commits BEFORE the caller does anything else with its
///   success — the kill has already happened, so any crash from here on
///   must find a record that says so.
///
/// The exit code is re-queried rather than assumed. A killed process
/// usually leaves tmux nothing to reduce to a plain code, but where it
/// does, that code is worth keeping — and `pane_states`, not
/// `pane_process`, is what carries `#{pane_dead_status}` at all. A failed
/// query costs the code, not the annotation, and a later list can still
/// enrich the record (the store's transitions are monotonic).
///
/// `root_pid` is the pid the caller's own liveness check found, passed in
/// rather than re-derived here so the pid signaled is the one the aliveness
/// decision was actually made about.
pub(crate) async fn stop_live_agent(
    sup: &Supervisor,
    session_id: &str,
    entry: &SessionEntry,
    root_pid: Option<u32>,
) -> Result<(), StopFailure> {
    sup.record(session_id, entry, Transition::StopRequested)
        .await
        .map_err(StopFailure::Unrecorded)?;
    // `AgentOnly`, shared verbatim by `StopSession` and the restart's
    // pre-relaunch reap — the two operations SPEC.md says leave a
    // session's terminal tabs running (PLAN_M4.md item 2's marker split).
    reap_process_tree(
        &sup.seams.scopes,
        entry.scope.as_slice(),
        root_pid,
        session_id,
        &SweepTarget::AgentOnly,
    )
    .await
    .map_err(StopFailure::Sweep)?;
    let exit_code = dead_pane_exit_code(sup, entry.terminal.as_ref(), session_id).await;
    sup.record(session_id, entry, Transition::StopCompleted { exit_code })
        .await
        .map_err(StopFailure::UnrecordedOutcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// The environment-marker scanner's whole job: match COMPLETE, exact
    /// NUL-delimited entries and nothing looser.
    ///
    /// A substring match would misfire on a session whose id is a prefix
    /// of another's, or on an unrelated variable that happens to embed
    /// the marker text — either would make `kill_process_tree` reap or
    /// spare the wrong session's processes. Constructed byte buffers, not
    /// a real process, since `environ_markers` is factored exactly so
    /// this needs neither.
    #[test]
    fn environ_markers_match_exact_entries_only() {
        let session = "abc-123";
        let read = |environ: &[u8]| environ_markers(environ, session, None);

        let mut buf = b"PATH=/bin\0".to_vec();
        buf.extend_from_slice(b"FARHELM_SESSION_ID=abc-123\0");
        buf.extend_from_slice(b"HOME=/root\0");
        assert!(read(&buf).session);

        // A value merely PREFIXED by the id ("abc-123" is a proper prefix
        // of "abc-1234", a DIFFERENT session) must not match.
        assert!(!read(b"FARHELM_SESSION_ID=abc-1234\0").session);
        // A different variable that merely embeds the marker text must
        // not match — only a complete NUL-delimited entry counts.
        assert!(!read(b"OTHER_VAR=FARHELM_SESSION_ID=abc-123\0").session);
        // A variable merely PREFIXED with a marker's NAME is not that
        // marker, which is why the "any" forms require the `=` too.
        assert!(!read(b"FARHELM_SESSION_ID_ALT=abc-123\0").session);
        assert!(!read(b"FARHELM_AGENT_ID_ALT=abc-123\0").any_agent);
        assert!(!read(b"FARHELM_TAB_ID_ALT=abc-123\0").any_tab);
        // No markers at all.
        assert_eq!(read(b"PATH=/bin\0HOME=/root\0"), EnvironMarkers::default());
    }

    /// Marker parsing returns only match flags and retains no environment
    /// bytes, including the spawn credential beside the session id.
    ///
    /// This is deliberately a parser-boundary claim. It does not claim to
    /// capture every future tracing call in the surrounding process scan; it
    /// proves that no credential bytes survive in the value that scan code
    /// can inspect or format after parsing.
    #[test]
    fn environ_marker_results_retain_no_spawn_credential_bytes() {
        let secret = "credential-that-must-not-reach-logs";
        let environ = format!(
            "{}=abc-123\0{}={secret}\0",
            crate::launch::SESSION_ID_ENV_VAR,
            crate::launch::SESSION_TOKEN_ENV_VAR,
        );
        let markers = environ_markers(environ.as_bytes(), "abc-123", None);
        assert!(markers.session);
        assert!(
            !format!("{markers:?}").contains(secret),
            "the parser result contains match flags, never environment bytes"
        );
    }

    /// The two "any" flags answer a different question from their exact
    /// siblings — not "is this ours" but "has something already claimed
    /// this process for a kind of terminal" — and that difference is what
    /// keeps the legacy bucket from swallowing another session's agent.
    ///
    /// The tab flag additionally demands a COMPLETE minted-shaped value:
    /// an empty or malformed `FARHELM_TAB_ID` is not a claim, or anything
    /// that exported the name could shield itself from a stop.
    #[test]
    fn the_any_marker_flags_require_a_real_claim() {
        let session = "abc-123";
        let tab = "9c3d5a71-0000-4000-8000-0000000000ff";

        let other_agent = environ_markers(b"FARHELM_AGENT_ID=other-session\0", session, None);
        assert!(
            other_agent.any_agent,
            "another session's agent marker is still a claim"
        );
        assert!(!other_agent.agent, "but it is not THIS session's");

        assert!(!environ_markers(b"FARHELM_AGENT_ID=\0", session, None).any_agent);
        assert!(!environ_markers(b"FARHELM_TAB_ID=\0", session, None).any_tab);
        assert!(
            !environ_markers(b"FARHELM_TAB_ID=not-a-uuid\0", session, None).any_tab,
            "a malformed tab value is not a claim either"
        );

        let real_tab = format!("FARHELM_TAB_ID={tab}\0");
        assert!(environ_markers(real_tab.as_bytes(), session, None).any_tab);
        // The EXACT tab flag only lights for the tab actually being swept.
        assert!(environ_markers(real_tab.as_bytes(), session, Some(tab)).tab);
        assert!(!environ_markers(real_tab.as_bytes(), session, Some("other-tab")).tab);
    }

    /// The three sweeps' selection rules, pinned as a table so the split
    /// reads as one decision rather than three scattered matches.
    ///
    /// Every row starts from the SESSION marker, which every variant
    /// requires: that is what keeps one session's sweep from ever reaching
    /// another session's processes however their other markers read.
    #[test]
    fn each_sweep_target_claims_exactly_what_its_operation_promises() {
        let markers = |agent: bool, any_agent: bool, any_tab: bool, tab: bool| EnvironMarkers {
            session: true,
            agent,
            any_agent,
            any_tab,
            tab,
        };
        let agent_launch = markers(true, true, false, false);
        let tab_launch = markers(false, false, true, true);
        let other_tab = markers(false, false, true, false);
        // A daemon left by a session launched before either kind marker
        // existed: session-marked and nothing else.
        let legacy = markers(false, false, false, false);
        // An INNER agent launched by a supervisor running inside somebody
        // else's tab, if the launch scrub ever regressed: it carries its
        // own agent marker AND an ambient outer tab marker.
        let inner_agent_with_ambient_tab = markers(true, true, true, false);

        let stop = SweepTarget::AgentOnly;
        assert!(stop.claims(&agent_launch));
        assert!(
            stop.claims(&legacy),
            "a pre-marker daemon must stay reachable, or stop silently weakens on upgrade"
        );
        assert!(
            !stop.claims(&tab_launch) && !stop.claims(&other_tab),
            "SPEC.md: stopping the agent leaves the session's tabs running"
        );
        assert!(
            stop.claims(&inner_agent_with_ambient_tab),
            "a positive agent marker outranks an ambient tab marker — the dogfooding hole \
             exclusion-only selection left open"
        );

        let delete = SweepTarget::WholeSession;
        assert!(delete.claims(&agent_launch) && delete.claims(&tab_launch));
        assert!(delete.claims(&legacy));

        let close = SweepTarget::Tab("9c3d5a71-0000-4000-8000-0000000000ff".to_string());
        assert_eq!(
            close.selects_tab(),
            Some("9c3d5a71-0000-4000-8000-0000000000ff")
        );
        assert!(close.claims(&tab_launch));
        assert!(
            !close.claims(&other_tab) && !close.claims(&agent_launch) && !close.claims(&legacy),
            "closing one tab reaches that tab and nothing else"
        );

        // And nothing at all without the session marker, whatever else is
        // set — the cross-session boundary.
        let foreign = EnvironMarkers {
            session: false,
            agent: true,
            any_agent: true,
            any_tab: true,
            tab: true,
        };
        for target in [stop, delete, close] {
            assert!(
                !target.claims(&foreign),
                "{target:?} claimed a process carrying no session marker"
            );
        }
    }

    /// Spawn a process carrying `FARHELM_SESSION_ID=<id>`, returning its
    /// pid — the only thing the marker sweep can find in a unit test.
    ///
    /// The child is the test binary in sleeper mode rather than a `sh`
    /// child: on macOS 26+ the kernel withholds a PLATFORM binary's
    /// environment from `KERN_PROCARGS2`, so a shell child is invisible to
    /// the very scan these tests exist to exercise — see
    /// [`crate::procs::sleeper`] for the full story, and
    /// `environ_marker_verdict`'s docs for what that means in production.
    fn spawn_marked_process(session_id: &str) -> std::process::Child {
        crate::procs::sleeper::spawn(&[(crate::launch::SESSION_ID_ENV_VAR, session_id)])
    }

    /// Whether `pid` is gone in the sense a kill test means it: no row in
    /// the process table, or a zombie still waiting to be reaped.
    ///
    /// The zombie case is not pedantry here — it is the ONLY outcome these
    /// tests produce. The marked process is a direct child of the test
    /// process, which does not `wait()` on it until the assertions are
    /// done, so a successfully SIGKILLed victim keeps its row until then;
    /// a bare "does the pid exist" check would call every successful sweep
    /// a failure.
    ///
    /// Deliberately routed through [`procs::read_process`] rather than
    /// reading `/proc` itself, which is what makes every kill test in this
    /// module a macOS regression suite for free: these tests spawn real
    /// processes and go through the real sweep, so the only thing that had
    /// to become portable for them to pin the Mac path too was this
    /// helper. A read ERROR (as opposed to "gone") reports "still there",
    /// so an unreadable process table fails the assertion loudly rather
    /// than passing it by accident.
    fn marked_process_gone(pid: u32) -> bool {
        match procs::read_process(pid) {
            Ok(None) => true,
            Ok(Some((_, _, state))) => state == ProcessState::Zombie,
            Err(_) => false,
        }
    }

    /// The ORDER of stop's two mechanisms, which nothing about the end
    /// state can show: both the cgroup kill and the marker sweep leave the
    /// same corpse, so "scope, then sweep" and "sweep, then scope" are
    /// indistinguishable afterwards. This pins it by watching a process only
    /// the SWEEP can reach and asking, at every scope operation, whether it
    /// is still alive — the answer is yes for all of them if and only if the
    /// sweep has not started yet.
    ///
    /// Worth pinning because the order is a real decision and a silent one:
    /// SPEC_impl.md's belt-and-suspenders rule says the sweep is the
    /// BACKSTOP, which is a claim about sequence. A refactor that ran the
    /// sweep first would still pass every other test in this file.
    ///
    /// The escalation itself is pinned in the same pass, including the
    /// post-SIGKILL confirmation: `systemctl kill` returning proves delivery,
    /// not that the cgroup emptied, so a stop that skipped the confirmation
    /// could report a tree reaped while processes it was meant to end were
    /// still running.
    ///
    /// Runs everywhere, systemd or not: the manager is a fake, and the only
    /// real process involved is one this test spawns itself.
    #[tokio::test]
    async fn the_backstop_sweep_runs_after_the_scope_kill_never_before() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let mut child = spawn_marked_process(&session_id);
        let decoy = child.id();

        // Each recorded op carries whether the sweep's victim was still
        // alive when it happened.
        let observed: Arc<std::sync::Mutex<Vec<(crate::scope::ScopeOp, bool)>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = {
            let observed = Arc::clone(&observed);
            Arc::new(move |op: &crate::scope::ScopeOp| {
                let alive = !marked_process_gone(decoy);
                observed.lock().unwrap().push((op.clone(), alive));
            }) as crate::scope::ScopeOpSink
        };
        // The unit survives the first two existence checks (the pre-TERM one
        // and the pre-KILL one) and is gone by the third, which is the
        // confirmation — the shape a real teardown produces.
        let scopes = crate::scope::ScopeManager::fake_vanishing(2, sink);

        let unit = crate::scope::unit_name(&session_id, 0).expect("a UUID id must name a unit");
        reap_process_tree(
            &scopes,
            std::slice::from_ref(&unit),
            None,
            &session_id,
            &SweepTarget::AgentOnly,
        )
        .await
        .expect("the sweep must confirm the marked process is gone");

        let observed = observed.lock().expect("op sink mutex poisoned");
        assert_eq!(
            observed
                .iter()
                .map(|(op, _)| op.clone())
                .collect::<Vec<_>>(),
            vec![
                // The availability probe comes first: reap consults it to
                // decide whether the recorded names can have units behind
                // them at all before asking the manager anything.
                crate::scope::ScopeOp::Probe,
                crate::scope::ScopeOp::Exists(unit.clone()),
                crate::scope::ScopeOp::Kill {
                    unit: unit.clone(),
                    signal: "SIGTERM".to_string(),
                },
                crate::scope::ScopeOp::Exists(unit.clone()),
                crate::scope::ScopeOp::Kill {
                    unit: unit.clone(),
                    signal: "SIGKILL".to_string(),
                },
                crate::scope::ScopeOp::Exists(unit.clone()),
            ],
            "the scope escalation must check existence, TERM, re-check, KILL, then confirm"
        );
        assert!(
            observed.iter().all(|(_, alive)| *alive),
            "the sweep must not have run yet at any point during the scope kill: {observed:?}"
        );
        assert!(
            marked_process_gone(decoy),
            "the backstop sweep must still have run afterwards and reaped the marked process"
        );
        let _ = child.wait();
    }

    /// A cgroup that never empties must not be reported as reaped — and must
    /// still not fail a stop the sweep confirmed.
    ///
    /// Both halves matter and they pull in opposite directions. Skipping the
    /// confirmation would let `systemctl kill`'s "signals delivered" pass for
    /// "processes gone"; treating the unconfirmed unit as fatal would make a
    /// stubborn cgroup fail a stop that M2 (which has no cgroups at all)
    /// would have called complete. The scope's trouble is diagnostic, the
    /// sweep's verdict is the answer.
    #[tokio::test]
    async fn a_scope_that_never_goes_away_is_reported_but_does_not_fail_the_stop() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let mut child = spawn_marked_process(&session_id);
        let decoy = child.id();

        // Never vanishes: every existence check answers "still loaded", so
        // the confirmation runs out its bound.
        let scopes = crate::scope::ScopeManager::fake(true, Arc::new(|_| {}));
        let unit = crate::scope::unit_name(&session_id, 0).expect("a UUID id must name a unit");
        reap_process_tree(
            &scopes,
            std::slice::from_ref(&unit),
            None,
            &session_id,
            &SweepTarget::AgentOnly,
        )
        .await
        .expect("an unconfirmed scope must not fail a stop the sweep confirmed");
        assert!(
            marked_process_gone(decoy),
            "the sweep must still have reaped the marked process"
        );
        let _ = child.wait();
    }

    /// A manager that is present but cannot kill must not weaken stop:
    /// PLAN_M3.md item 10's "absence of a manager never degrades stop below
    /// M2's guarantees", read in the direction it is easier to get wrong.
    ///
    /// The failure mode this excludes is a stop that reports failure — and,
    /// through `StopFailure::Sweep`, refuses a restart — because a cgroup
    /// operation errored while the sweep it exists to reinforce succeeded
    /// completely. The sweep's verdict is the whole answer.
    #[tokio::test]
    async fn a_broken_user_manager_never_fails_a_stop_the_sweep_confirmed() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let mut child = spawn_marked_process(&session_id);
        let decoy = child.id();

        let scopes = crate::scope::ScopeManager::fake_failing_kills(Arc::new(|_| {}));
        let unit = crate::scope::unit_name(&session_id, 0).expect("a UUID id must name a unit");
        reap_process_tree(
            &scopes,
            std::slice::from_ref(&unit),
            None,
            &session_id,
            &SweepTarget::AgentOnly,
        )
        .await
        .expect("a failing scope kill must not fail a stop the sweep confirmed");
        assert!(
            marked_process_gone(decoy),
            "the sweep must still have reaped the marked process"
        );
        let _ = child.wait();
    }

    /// A host with NO user manager must not be asked about derived scope
    /// names at all: no per-unit errors, no `SCOPE_CONFIRM_TIMEOUT` polls —
    /// and the sweep still reaps.
    ///
    /// This pins the scope half of a real macOS delete failure (2026-08):
    /// delete derives every open tab's unit name unconditionally, so on a
    /// manager-less host `reap_process_tree` received real unit names and
    /// `kill_scope` turned each into four "no systemd user manager" errors
    /// plus a two-second confirm poll. The sweep's own (separate) macOS
    /// failure then promoted that noise into a user-visible delete error.
    /// Skipping units when the availability probe already answered "no
    /// manager" is safe precisely because the sweep below remains the whole
    /// mechanism — which this test also re-asserts by watching the marked
    /// process die.
    #[tokio::test]
    async fn an_absent_user_manager_is_never_asked_about_derived_scope_units() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let mut child = spawn_marked_process(&session_id);
        let decoy = child.id();

        let observed: Arc<std::sync::Mutex<Vec<crate::scope::ScopeOp>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = {
            let observed = Arc::clone(&observed);
            Arc::new(move |op: &crate::scope::ScopeOp| {
                observed.lock().unwrap().push(op.clone());
            }) as crate::scope::ScopeOpSink
        };
        let scopes = crate::scope::ScopeManager::fake(false, sink);
        let unit = crate::scope::unit_name(&session_id, 0).expect("a UUID id must name a unit");
        reap_process_tree(
            &scopes,
            std::slice::from_ref(&unit),
            None,
            &session_id,
            &SweepTarget::AgentOnly,
        )
        .await
        .expect("scope names on a manager-less host must not fail a stop the sweep confirmed");
        assert!(
            marked_process_gone(decoy),
            "the sweep must still have reaped the marked process"
        );
        let observed = observed.lock().expect("op sink mutex poisoned");
        assert!(
            observed
                .iter()
                .all(|op| matches!(op, crate::scope::ScopeOp::Probe)),
            "an absent manager must only ever see the availability probe, got: {observed:?}"
        );
        let _ = child.wait();
    }

    /// A scope kill must never leave the sweep seeding itself from a pid the
    /// kill's own grace window let the kernel recycle.
    ///
    /// The window is real: `kill_scope` sends SIGTERM and sleeps out
    /// [`KILL_GRACE`], and the pane's process is precisely the one most
    /// likely to die inside it. Passing a bare pid through that window and
    /// then walking the PPID closure from it would mean sweeping — and
    /// signaling — a completely unrelated process tree that happened to
    /// inherit the number.
    ///
    /// Reproduced without racing the kernel for a recycled pid: the pane pid
    /// handed in is one that is ALREADY gone, so any implementation that
    /// trusts the number would seed from whatever now holds it, while one
    /// that captured `(pid, starttime)` first correctly finds no identity to
    /// carry at all.
    #[tokio::test]
    async fn a_dead_pane_pid_is_never_seeded_into_the_sweep() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let mut gone = spawn_marked_process(&session_id);
        let gone_pid = gone.id();
        let _ = gone.kill();
        let _ = gone.wait();

        let scopes = crate::scope::ScopeManager::disabled();
        reap_process_tree(
            &scopes,
            &[],
            Some(gone_pid),
            &session_id,
            &SweepTarget::AgentOnly,
        )
        .await
        .expect("a dead pane pid must not fail the sweep");
    }

    /// `signal_validated`'s entire reason to exist: a pid whose CURRENT
    /// start-time does not match what was recorded earlier must be left
    /// alone, even under a signal as unblockable as `SIGKILL`.
    /// Uses a REAL child process (there is no way to fabricate a process
    /// table row) and a starttime that cannot possibly be correct, then
    /// confirms the child is still alive before cleaning it up for real.
    #[test]
    fn signal_validated_skips_a_pid_whose_starttime_does_not_match() {
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .expect("spawn a real child to validate against");
        let pid = child.id();
        let (_, real_starttime, _state) = procs::read_process(pid)
            .expect("reading a just-spawned child must not error")
            .expect("a just-spawned child must have a readable process row");

        // Any starttime other than the real one proves the point; adding
        // a large offset makes collision with the real value effectively
        // impossible without needing to reason about clock/jiffy units.
        let bogus_starttime = real_starttime.wrapping_add(999_999_999);
        signal_validated(pid, bogus_starttime, libc::SIGKILL)
            .expect("a starttime mismatch is a skip, not an error");

        std::thread::sleep(Duration::from_millis(200));
        assert!(
            matches!(child.try_wait(), Ok(None)),
            "child must survive a signal validated against the wrong starttime"
        );

        // Clean up for real, with the correct identity this time —
        // otherwise this test leaks a `sleep 5` process.
        signal_validated(pid, real_starttime, libc::SIGKILL)
            .expect("signaling with the correct starttime must succeed");
        let _ = child.wait();
    }
}
