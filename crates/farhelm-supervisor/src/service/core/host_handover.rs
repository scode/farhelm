//! Keeping host terminals across a supervisor restart (the plan's PR 17):
//! the stopping supervisor saves hosts with enough output to repay an encode
//! ([`Supervisor::checkpoint_hosts_for_exit`]), the starting one finds the
//! host behind every host row the store holds and adopts it again
//! ([`attribute_host_rows`], the inventory's top risk 2), and a running one
//! keeps trying a host it could not use ([`Supervisor::start_readoption`]).
//! Between a stop and the next start, a host keeps running on its own, its
//! program never noticing.
//!
//! ## Stopping
//!
//! An orderly stop checkpoints host terminals with enough new output under
//! one deadline, using at most half the machine's cores (at least one).
//! Hosts with the largest output tails get a slot first, so checkpoints
//! that finish save the most replay. It then closes each runtime's
//! connection. A checkpoint is what keeps the next
//! supervisor's recovery short: the host drops the output before an
//! accepted snapshot, and the next runtime restores the snapshot and replays
//! only what follows. A checkpoint the deadline cuts short, one that never
//! got a core, or one the terminal cannot take (a sequence left open, a
//! runtime still replaying), leaves the host its previous snapshot and all
//! the output after it, which the next supervisor replays: a slower recovery
//! of the same terminal.
//!
//! The deadline bounds the process's exit too, not only the wait. An encode
//! runs on tokio's blocking pool, which nothing interrupts from outside, and
//! the runtime `farhelm supervisor run` owns waits for its blocking jobs as
//! it drops, holding the state directory's claim until then
//! (`core::on_owned_runtime`). So closing a runtime cancels its encodes
//! (`HostTerminal::abort`), the encoder gives up between rows
//! (`farhelm_terminal_emulator::Cancel`, about a millisecond), and the stop
//! waits for every encode to have ended before it returns. Capping the
//! encodes is what lets some finish: started all at once,
//! many full terminals on few cores would share the cores until the
//! deadline and none would finish. Leaving half the cores free also leaves
//! room for the update or desktop app that is replacing this supervisor.
//! These slots cover the stop's own checkpoints, not automatic encodes
//! already running or started by runtimes still waiting for a slot.
//!
//! A host with less than the policy's stop threshold of processed output
//! since its latest known snapshot (normally 1 MiB) is closed immediately,
//! without taking a slot. Recovery replays that short tail cheaply. The
//! runtime checks again when it takes the request up: an automatic encode
//! may have shortened the tail while the stop waited for a slot. A deliberate
//! skip is a successful stop. The cost avoided is an encode of the whole
//! terminal, not of the new output (SPEC_impl.md, "Recovery and checkpoints",
//! gives the measured cost).
//!
//! Closing each connection right after its checkpoint limits the output the
//! stopping supervisor counts as delivered without acting on it to what the
//! host had already written to the connection: that output is replayed by
//! the next supervisor without side effects, so a query in it goes
//! unanswered (the plan's D-7, the delivery ambiguity accepted at a stop as
//! at a crash). Output written after the close is unseen, and its queries
//! are answered.
//!
//! The runtimes are gathered from the session map, plus the host tabs of
//! every session a Restart has taken off it for its window
//! (`Supervisor::restarting_tabs`): those tabs keep running across the
//! Restart, and the map alone would miss them. A Restart that puts a
//! session back after the stop began stops its tab runtimes and its new
//! agent runtime instead of publishing them connected
//! (`Supervisor::fence_republished_tabs`, `Supervisor::publishable`).
//!
//! ## Finding the hosts again
//!
//! A host outlives the supervisor that started it, and the supervisor keeps no
//! list of hosts: a restarted supervisor finds them by scanning its socket
//! directory (`crate::hosts::registry`) and matches them to rows here, in
//! [`attribute_host_rows`], the one place a supervisor learns which host
//! belongs to which session. Host rows never go through tmux's pane lookup:
//! their `terminal_address` column holds a socket name (the plan's D-14), which
//! no tmux lookup may read as a pane, and "no such pane" would record a running
//! agent as exited.
//!
//! What identifies a row's host, in order:
//!
//! - **The tag.** A live host whose tag names the row's session and launch
//!   generation is the row's, whatever its socket is called. This is how a
//!   launch that never reached its confirmation (an empty `terminal_address`)
//!   is found again, as tmux's agent-window marker finds an unconfirmed pane.
//! - **The socket the row names**: its `terminal_address` once the launch was
//!   confirmed, else the socket the launch record of the row's own generation
//!   names (`<state>/launch/<id>.<gen>.host`, `host_launch`'s "The launch
//!   record"), never an earlier generation's record. What is there when no live
//!   host carries the row's tag decides between "unavailable" and "absent". One
//!   host there is the row's although its tag names an earlier generation: an
//!   agent host of the same session at the socket the row's `terminal_address`
//!   names. Only a confirmation writes `terminal_address`, and a Restart
//!   empties it as it opens a generation, so a `terminal_address` naming such a
//!   host is one a Restart that failed definitively put back with the previous
//!   run's record: that host is still the session's terminal (`host_restart`).
//! - **A Restart that never started its replacement.** A Restart empties
//!   `terminal_address` as it claims the new generation, before its replacement
//!   host has a launch record. A supervisor that dies in between leaves a row
//!   whose own generation has no host, no `terminal_address` and no record, and
//!   the previous host, which still holds the session's history. Only then
//!   (never when the row's generation has a record, readable or not, or a host)
//!   the host of the newest earlier generation that has a launch record is the
//!   row's terminal, when the scan read it at the record's socket. A record
//!   whose host the scan found gone is passed over for the next older one; any
//!   other record whose host was not read there (and a launch directory that
//!   cannot be listed in full) leaves the row with none, and the next start
//!   tries again ([`newest_recorded_earlier`]). Reconciliation records the
//!   socket of a host it takes in `terminal_address` as for any found host (the
//!   row then lists the previous run's exit, or running if its program still
//!   runs), and the next Restart seeds from it and ends it.
//!
//! Each row ends in one of three classes ([`RowHost`]), and the reload treats
//! them as it treats a found, an unattributed and an absent pane, except that
//! an unavailable host records nothing durable at all, not even a launch
//! sentinel (the plan's D-16, as `status::TerminalEvidence::Unattributed` says for
//! hosts):
//!
//! - **Adopted**: a runtime now drives the host, recovered from its stored
//!   snapshot and the journal after it (`Adoption::Recovered`).
//! - **Unavailable**: a host of the row's launch may run and cannot be used:
//!   it does not answer, speaks another protocol version, stays busy,
//!   carries a tag this build cannot read, the socket directory could not be
//!   scanned at all, or the launch record that names the socket cannot be
//!   read. It is left as it is, never treated as gone, and the session holds
//!   a handle that answers as unavailable (`HostTerminal::unavailable`), so
//!   it lists as unknown, the verbs refuse, and no observation records
//!   anything for it. A session never holds no terminal for this: no
//!   terminal reads as gone.
//! - **Absent**: no host of the row's launch runs. Only provable from the
//!   socket the row names (nothing serves it, or the file is missing: a
//!   socket name is drawn fresh for each launch, so another host cannot be
//!   there) or from there never having been a host start that got its spec
//!   (no launch record for an unconfirmed launch).
//!
//! The scan takes over every host it reaches, so it runs only in a
//! supervisor that holds its state directory's lock, and it skips the hosts
//! this supervisor already holds a terminal for: `serve`'s second reload
//! keeps the constructor's runtimes and placeholders, and skips the sockets
//! the constructor found gone (socket names are never reused, so a gone one
//! stays gone). Hosts the scan finds that no row claims (a tab's, an
//! orphan's) are left as they were: their
//! connections close with the registry, except an earlier generation's
//! agent host (below). A session's tab hosts are adopted beside this, from
//! its tab records (`host_tabs`, "After a restart"); the hosts no session or
//! tab accounts for are ended by `serve`'s startup reclaim once both have run
//! (`host_reclaim`).
//!
//! The orderly stop's checkpoints and closes cover the sessions' tab hosts
//! too: they outlive the supervisor the same way, and a runtime left open
//! would keep its encodes running past the stop.
//!
//! ## Superseded agent hosts
//!
//! A Restart of a host agent starts a new host and ends the old one once the
//! new launch is confirmed (`host_restart`). A supervisor that dies in between
//! leaves the old host running with nothing in memory to end it, and it is no
//! orphan: its session exists. So the reload ends it, with the plan's D-25 rule
//! ([`superseded`]): a live agent host that no row claimed, whose tag names a
//! session whose row is a host row at a later launch generation, and whose
//! row's later launch is confirmed (its `terminal_address` names a socket, and
//! not this host's). The decision is taken after the reload's own
//! reconciliation has committed, on the row as it then stands, so a restart
//! interrupted before its confirmation converges in one start: the reload finds
//! the new host by its tag and confirms it, and then the old host is
//! superseded. Until a row's later launch is confirmed, its earlier host is
//! kept (a definitive failure may still make it the session's terminal, and an
//! unconfirmed launch may have started nothing).
//!
//! It is ended over the scan's own connection (`Shutdown`, then the
//! verified signal to the connection's peer), never by connecting to its
//! socket again, and its launch record goes once it is confirmed ended. The
//! current generation's host is never superseded (it is claimed, or kept
//! as unavailable), nor is a host this build cannot read: only live hosts,
//! whose tags were read, are looked at. Nor is a tab's host: its tag names
//! a tab, not a launch generation, and the agent's Restart leaves the
//! session's tabs as they are. Ending the host hangs up its program, whose
//! own Restart stopped it before the new launch began; a descendant that
//! survives a hangup is left to the session's next cleanup (its scope, or
//! the marker sweep where containment is uncertain).
//!
//! ## Adopting again
//!
//! An agent's or tab's host that is unavailable for a reason that can pass is retried
//! while the supervisor runs, whether startup could not adopt it or its
//! runtime became unavailable later. The task started with the supervisor
//! ([`Supervisor::start_readoption`]) retries one at a time, after 1 s and
//! then doubling to a minute. The attempt count survives a successful
//! adoption for that session and launch, or that tab, so repeated failures never return
//! to the fastest retries. Each episode in which a runtime gave up after
//! repeated reconnects (`Unstable`) waits at least the whole reconnect
//! window before its first try: re-adoption must not immediately renew a
//! fight with whoever keeps taking the host over. The next due time is set
//! before every attempt, and healthy launches' kept counts schedule no wake.
//! An attempt
//!
//! 1. takes the session's lifecycle claim without waiting (a busy session,
//!    a Stop, Restart or Delete in progress, is retried later), and holds it
//!    to the end, so nothing else acts on the session's host meanwhile;
//! 2. checks that the session's entry still holds that very handle for
//!    the same launch generation, or its tab cell still holds it for the
//!    same tab id, and that no orderly stop has begun;
//! 3. looks at the one socket the handle names, and nothing else: a scan
//!    restricted to that name, which a launch drew for itself, so no host
//!    another runtime of this supervisor holds or is starting can be taken
//!    over; a host whose tag names the session's launch (or, as at startup, an
//!    earlier launch of its agent: the socket is the one the row's
//!    `terminal_address` names) is adopted as recovered, and anything else is
//!    classified as at startup;
//! 4. under [`Supervisor::host_adoption`] and the session map, checks the
//!    entry and the stop again and swaps the outcome in (a runtime, a new
//!    placeholder, or no terminal for a host that has ended), keeping the
//!    entry's run and session cells; a runtime it made that is not
//!    installed is stopped, leaving its host as it was.
//!    A tab keeps its id, known creation stamp and scope, or learns those
//!    from its tag when startup left them unknown. A tab's host found gone
//!    is kept as a finished handle for the reap to close and remove its
//!    record, retaining the old adopted process for that close's check.
//!    A tab whose close already failed belongs to the reap and is not retried.
//!
//! The ticker forwards passing unavailability changes to this task's wake,
//! so a runtime that becomes unavailable is scheduled without waiting for a
//! periodic tick. A reason that will not pass on its own (a protocol,
//! snapshot format or tag of a newer release, a launch record that cannot be
//! read) is never retried. A failed unconditional Delete may have aborted a
//! runtime whose last liveness was unavailable; it can be adopted again by
//! the same rule, and the Delete's retry ends it through the new runtime.
//!
//! An orderly stop sets [`Supervisor::stopping`] under the session map as
//! it gathers the host terminals to close, so no retry installs a runtime
//! after that, and the task returns.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinSet;
use tracing::{debug, info, warn};

use super::Supervisor;
use super::host_launch::{read_launch_record, remove_ended_socket_file};
use crate::hosts::terminal::RECONNECT_WINDOW;
use crate::hosts::{
    AdoptError, Adoption, CheckpointOutcome, HostRegistry, HostRole, HostTag, HostTerminal,
    HostTerminalOptions, LiveHost, Liveness, NamedHost, NotTaken, ScanOptions, Unavailability,
    WhyUnavailable,
};
use crate::launch::LaunchArtifacts;
use crate::service::terminals::Terminal;
use crate::store::{LastOutcome, SessionStore, StoredSession, TerminalKind, Transition};

// ---- Stopping ------------------------------------------------------------------------

/// How long past its deadline an orderly stop waits for the encodes it
/// cancelled to end. A cancelled encode gives up within about a millisecond
/// (the encoder looks at its cancel between rows); this only bounds a
/// pathological one, which the process's exit then waits for anyway.
const ENCODE_END_GRACE: Duration = Duration::from_secs(1);

/// Leave half the machine available to the operation replacing the
/// supervisor, without preventing progress on a one-core machine.
fn exit_checkpoint_slots(cores: usize) -> usize {
    (cores / 2).max(1)
}

impl Supervisor {
    /// Save substantial host-terminal tails and close every runtime within
    /// `budget`, because this supervisor is exiting (module docs, "Stopping").
    ///
    /// Each runtime's checkpoint is asked for once it gets one of at most
    /// half as many slots as the machine has cores (at least one), largest
    /// output tail first, within the deadline; each runtime is
    /// then stopped ([`HostTerminal::abort`]), which cancels any encode it
    /// still runs, and this waits until every such encode has ended, so
    /// none outlives the stop on the blocking pool. The hosts keep running.
    /// Short tails take no slot and their runtimes close immediately. A
    /// request can also skip after a newer automatic checkpoint shortens
    /// its tail. Returns whether every wanted checkpoint completed: an
    /// accepted or current snapshot, or a deliberate skip, is success.
    /// Runtimes that had already ended (an unavailable host, a gone one)
    /// hold no connection and run no encode, and are left out.
    ///
    /// After this, the session map still holds the stopped runtimes, which
    /// keep answering from their last status (an exit, a child that runs)
    /// and refuse every request: this is for a supervisor that is exiting.
    pub(crate) async fn checkpoint_hosts_for_exit(&self, budget: Duration) -> bool {
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        self.checkpoint_hosts_for_exit_with_slots(budget, exit_checkpoint_slots(cores))
            .await
    }

    /// Run the same stop with an explicit slot count, so tests can hold a
    /// checkpoint in the only slot and observe which host is admitted next.
    /// Production derives the count from the machine in the wrapper above.
    pub(super) async fn checkpoint_hosts_for_exit_with_slots(
        &self,
        budget: Duration,
        slot_count: usize,
    ) -> bool {
        let deadline = tokio::time::Instant::now() + budget;
        // The map's lock is held only for map operations, but the deadline
        // bounds even a wait for it. `stopping` is set under the same lock:
        // a re-adoption installs a runtime only under it, after checking the
        // flag, so none appears in the map after this gathers it.
        let gathered = tokio::time::timeout_at(deadline, async {
            let sessions = self.sessions.lock().await;
            self.stopping
                .store(true, std::sync::atomic::Ordering::SeqCst);
            gather_host_terminals(&sessions, &self.restarting_tabs)
        })
        .await;
        self.stopping
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.readoption_wake.notify_one();
        self.stop_began.notify_waiters();
        // Without the map nothing here can be reached: other holders of a
        // runtime's handle exist (a pending hold, an attachment, a create in
        // flight), but nothing else lists them. The stop's final close tries
        // the map again
        // (`Self::close_host_terminals`), and every runtime cancels its
        // encodes when its task is dropped, which the process's tokio
        // runtime does to every task before it waits for the blocking pool:
        // the exit stays bounded by one encode's look at its cancel.
        let Ok(all) = gathered else {
            warn!("could not read the sessions to checkpoint their terminal hosts before exit");
            return false;
        };
        // Only a runtime still driving its host is asked for a checkpoint;
        // every one is stopped below, since one that has just ended may
        // still be settling an encode of its own.
        let mut hosts: Vec<(u64, HostTerminal)> =
            all.iter()
                .filter(|host| still_driven(host))
                .filter_map(|host| {
                    match host.exit_checkpoint_tail() {
                        Some(tail) => Some((tail, host.clone())),
                        None => {
                            // A short tail is cheap to replay. Close now,
                            // before waiting for other hosts' slots, so this
                            // runtime neither reads more delivered output nor
                            // starts an automatic encode during the wait.
                            host.abort();
                            debug!(socket = %host.socket().display(), "skipped a small terminal tail at exit");
                            None
                        }
                    }
                })
                .collect();
        // Keep one observation per host: automatic checkpoints and output
        // can move these positions while the ordered queue is draining.
        hosts.sort_by_key(|(tail, _)| std::cmp::Reverse(*tail));
        if hosts.is_empty() {
            close_all(&all, deadline + ENCODE_END_GRACE).await;
            return true;
        }
        info!(
            count = hosts.len(),
            "stopping: checkpointing terminal hosts before exit"
        );
        let slots = Arc::new(tokio::sync::Semaphore::new(slot_count.max(1)));
        let mut checkpoints = JoinSet::new();
        for (_, host) in hosts {
            // Admission happens in this one ordered loop. Acquiring inside
            // each spawned task would let the executor's polling order
            // decide who got a core first, undoing the sort.
            let slot = tokio::time::timeout_at(deadline, Arc::clone(&slots).acquire_owned()).await;
            checkpoints.spawn(async move {
                let outcome = match slot {
                    Ok(Ok(_slot)) => host.checkpoint_now(deadline).await,
                    // No core came free in time: the host keeps what it has.
                    Ok(Err(_)) | Err(_) => Ok(CheckpointOutcome::Kept(NotTaken::TimedOut)),
                };
                host.abort();
                (host, outcome)
            });
        }
        let mut all_current = true;
        while let Some(joined) = checkpoints.join_next().await {
            let (host, outcome) = match joined {
                Ok(done) => done,
                // A checkpoint task is this function's own and never
                // aborted; a panic in one is logged, and its runtime is
                // stopped below with every other.
                Err(error) => {
                    warn!(%error, "a terminal host's checkpoint task failed at exit");
                    all_current = false;
                    continue;
                }
            };
            let socket = host.socket().display().to_string();
            match outcome {
                Ok(
                    CheckpointOutcome::Accepted(position) | CheckpointOutcome::Current(position),
                ) => {
                    debug!(
                        socket,
                        ?position,
                        "checkpointed a terminal host before exit"
                    );
                }
                Ok(CheckpointOutcome::Kept(why)) => {
                    all_current = false;
                    info!(
                        socket,
                        ?why,
                        "left a terminal host its previous snapshot at exit; the next supervisor \
                         replays the output after it"
                    );
                }
                Ok(CheckpointOutcome::Skipped { tail }) => {
                    debug!(socket, tail, "skipped a small terminal tail at exit");
                }
                Err(liveness) => {
                    all_current = false;
                    debug!(
                        socket,
                        ?liveness,
                        "a terminal host's runtime ended before its checkpoint at exit"
                    );
                }
            }
        }
        // Each task stopped its own runtime; this also covers one whose task
        // failed before it could, and every runtime that had already ended.
        close_all(&all, deadline + ENCODE_END_GRACE).await;
        all_current
    }

    /// Stop every host terminal now in the session map (and every host tab
    /// of a session a Restart has off it) and wait, until
    /// `deadline`, for the encodes they started to end: what an orderly
    /// stop does last ([`super::Supervisor::stop_in_order`]), after its
    /// pending-hold wait. It matters only after the stop's gather timed out
    /// on the map's lock: otherwise the gather already closed everything
    /// published before [`Self::stopping`] was set (under that same lock),
    /// and nothing is published after. Bounded: the map's lock is waited
    /// for until `deadline` as well.
    pub(crate) async fn close_host_terminals(&self, deadline: tokio::time::Instant) {
        let gathered = tokio::time::timeout_at(deadline, async {
            let sessions = self.sessions.lock().await;
            gather_host_terminals(&sessions, &self.restarting_tabs)
        })
        .await;
        match gathered {
            Ok(all) => close_all(&all, deadline).await,
            Err(_) => warn!(
                "could not read the sessions to close their terminal hosts before exit; each \
                 runtime's encodes are cancelled as the process's tasks are dropped"
            ),
        }
    }

    /// The terminal a create may publish now, under [`Self::sessions`],
    /// which the caller holds: `terminal` itself, unless it is a host
    /// runtime and an orderly stop has begun gathering the host terminals it
    /// closes ([`Self::stopping`]). Such a runtime would be closed by
    /// nothing before the process exits, so it is stopped instead and a
    /// placeholder takes its place, which records nothing; the create's
    /// durable evidence (its row, its launch record) stays for the next
    /// supervisor, whose reload adopts the host by its tag.
    pub(crate) fn publishable(&self, terminal: Option<Terminal>) -> Option<Terminal> {
        match terminal {
            Some(Terminal::Host(host)) if self.hosts_stopping() && !host.is_placeholder() => {
                host.abort();
                info!(socket = %host.socket().display(),
                    "the supervisor is stopping; leaving a new terminal host to the next one");
                Some(Terminal::Host(HostTerminal::unavailable(
                    host.socket(),
                    Unavailability::Unidentified(
                        "the supervisor was stopping when it was published".into(),
                    ),
                    None,
                )))
            }
            other => other,
        }
    }

    /// Stop the runtimes of the host tabs in `tabs`, a session's tab cell
    /// that a Restart puts back on the session map (or drops with a session
    /// deleted meanwhile), if an orderly stop has begun gathering the host
    /// terminals it closes ([`Self::stopping`]); otherwise leave them
    /// running, as a Restart does. Called under [`Self::sessions`], which
    /// the caller holds. The stop gathered these runtimes from
    /// [`Self::restarting_tabs`] and stops them itself; this is the
    /// publication's own fence, as [`Self::publishable`] is for the agent's
    /// runtime, so no tab runtime is put back connected after the stop.
    /// Unlike the agent's, a tab's runtime stays in its cell, stopped: it
    /// keeps answering from its last status, as every runtime in the map
    /// does once the stop has closed it.
    ///
    /// Before a stop, republication wakes re-adoption: a pass while Restart
    /// had the entry off-map may have parked with no active retry. The
    /// caller holds the session map through publication, so the woken task
    /// cannot inspect it before the tabs are visible again.
    pub(crate) fn fence_republished_tabs(&self, tabs: &super::HostTabs) {
        if !self.hosts_stopping() {
            self.readoption_wake.notify_one();
            return;
        }
        for tab in super::host_tabs_of(tabs) {
            tab.host.abort();
        }
    }
}

impl Supervisor {
    /// [`Self::checkpoint_hosts_for_exit`] with a budget of the test's
    /// choosing, for the in-process harnesses: a stop's host side alone,
    /// shorter than [`super::HOST_CHECKPOINT_BUDGET`]. Compiled only for
    /// tests and the `test-seams` feature.
    #[cfg(any(test, feature = "test-seams"))]
    pub async fn checkpoint_hosts_for_test(&self, budget: Duration) -> bool {
        self.checkpoint_hosts_for_exit(budget).await
    }
}

/// Every host terminal runtime the session map holds, each once: the
/// sessions' agent terminals that are hosts, and their tabs in hosts
/// (`host_tabs`), whose hosts outlive the supervisor the same way and need
/// the same checkpoint and the same close; and the tabs of every session a
/// Restart has off the map (`restarting`, [`Supervisor::restarting_tabs`]),
/// which keep running across it. A restarting session's agent runtime is
/// not among them: the Restart itself settles it (a seed refused, a
/// publication fenced, [`Supervisor::publishable`]). The caller holds
/// [`Supervisor::sessions`], under which `restarting` is written.
fn gather_host_terminals(
    sessions: &HashMap<String, Arc<super::SessionEntry>>,
    restarting: &std::sync::Mutex<HashMap<String, super::HostTabs>>,
) -> Vec<HostTerminal> {
    let mut all: Vec<HostTerminal> = Vec::new();
    for entry in sessions.values() {
        match &entry.terminal {
            Some(Terminal::Host(host)) if !all.contains(host) => all.push(host.clone()),
            Some(Terminal::Host(_) | Terminal::Tmux(_)) | None => {}
        }
    }
    for host in super::host_tabs::tab_runtimes(sessions.values()) {
        if !all.contains(&host) {
            all.push(host);
        }
    }
    let restarting = restarting.lock().unwrap_or_else(|e| e.into_inner());
    for tab in restarting.values().flat_map(super::host_tabs_of) {
        if !all.contains(&tab.host) {
            all.push(tab.host);
        }
    }
    all
}

/// Stop every runtime of `hosts` (which cancels their encodes) and wait,
/// until `deadline`, for each encode to have given up.
async fn close_all(hosts: &[HostTerminal], deadline: tokio::time::Instant) {
    for host in hosts {
        host.abort();
    }
    let ended = tokio::time::timeout_at(deadline, async {
        for host in hosts {
            host.encodes_ended().await;
        }
    })
    .await;
    if ended.is_err() {
        warn!("a terminal checkpoint's encode did not stop when cancelled; the exit waits for it");
    }
}

// ---- Finding the hosts again ---------------------------------------------------------

/// The socket names a supervisor's startup found gone, shared by its
/// reloads and its spare-claim check: a socket nothing serves stays that
/// way, since socket names are drawn fresh for each host and never reused,
/// so a later look at it would only pay the refusal window again (three
/// seconds per stale socket) to learn the same.
pub(super) type GoneHosts = Arc<std::sync::Mutex<HashSet<OsString>>>;

/// What reload found behind one host row ([`attribute_host_rows`]).
pub(super) enum RowHost {
    /// A runtime drives the row's host: adopted now, or kept from an
    /// earlier reload of the same supervisor.
    Adopted(HostTerminal),
    /// A host of the row's launch may run and cannot be used, and this is a
    /// handle that answers as unavailable: for the socket the row's host is
    /// at, or, when not even that is known (a launch record that cannot be
    /// read), for the socket directory itself.
    Unavailable(HostTerminal),
    /// No host of the row's launch runs.
    Absent,
}

/// What [`attribute_host_rows`] works with besides the rows.
pub(super) struct HostDiscovery {
    /// Whether this supervisor holds the state directory's lock. Without
    /// it nothing is scanned, since a scan takes over every host it reaches
    /// and these hosts belong to the supervisor that holds the lock: every
    /// host row is then unavailable to this one.
    pub(super) owns_state_dir: bool,
    /// What each adopted runtime is made with: the supervisor's liveness
    /// notifier, and any test adjustment.
    pub(super) options: HostTerminalOptions,
    /// The terminals this supervisor already holds for host rows, by
    /// session, with the launch generation each is for. One for a row of
    /// that generation is kept as it is (a runtime still driving its host,
    /// or an unavailable one, which re-adoption retries), and its socket is
    /// never scanned.
    pub(super) adopted: HashMap<String, (i64, HostTerminal)>,
    /// The supervisor's `host_scan` lock, taken exclusively around the scan
    /// and the adoptions, so that no host spawn of this supervisor runs
    /// meanwhile (a spawning host's socket answers before it is on any
    /// map, and the scan would take it over). `None` before the supervisor
    /// exists, when nothing can spawn.
    pub(super) no_spawns: Option<Arc<tokio::sync::RwLock<()>>>,
    /// Sockets found gone earlier, never scanned again; those found gone
    /// now are added.
    pub(super) gone: GoneHosts,
    /// The tabs in terminal hosts this supervisor already holds, by session
    /// (`SessionCells::host_tabs`): kept by the reload as agent terminals in
    /// `adopted` are, and never scanned. [`attribute_host_rows`] only skips
    /// their sockets (its scan reaches every socket not skipped, and taking
    /// a tab host over would cut its runtime off); the reload hands the
    /// same tabs to `host_tabs::adopt_tab_hosts`, which keeps them and finds
    /// a session's other tab hosts from its tab records.
    pub(super) tabs: HashMap<String, Vec<super::HostTab>>,
}

/// Which host, if any, is each host row's agent terminal, adopting every
/// usable one: [`RowHost`] for every row of `rows` whose terminal kind is
/// [`TerminalKind::Host`], keyed by session id, and nothing for tmux rows.
/// The hosts are those in `state_dir`'s socket directory.
///
/// This is where a supervisor learns a host's session, and the only place:
/// the scan here reads every host's tag (at startup, of every host in the
/// directory not skipped), and each adoption pairs a host with its row's
/// session (agent role, that generation), for reloads and re-adoption alike.
/// A caller that keeps track of which hosts belong to which session records
/// it from the returned map (module docs for how a host is matched).
///
/// Never fails: a scan that cannot run leaves every host row it would have
/// decided unavailable, which records nothing. Adoptions run at once, each
/// within the client's deadlines and the restore of its snapshot (on the
/// blocking pool); none waits for its terminal to catch up with the
/// journal, which the runtime then replays on its own.
///
/// Also returns the live agent hosts of the rows' earlier launch
/// generations that no row claimed, with the scan's connections to them,
/// for [`end_superseded_hosts`] to judge once the reload has committed
/// (module docs, "Superseded agent hosts"); dropping them leaves those
/// hosts as they were.
pub(super) async fn attribute_host_rows(
    state_dir: &Path,
    discovery: HostDiscovery,
    rows: &[StoredSession],
) -> (HashMap<String, RowHost>, EarlierAgentHosts) {
    let HostDiscovery {
        owns_state_dir,
        options,
        adopted,
        no_spawns,
        gone,
        tabs,
    } = discovery;
    let mut found = HashMap::new();
    let none_earlier = EarlierAgentHosts(Vec::new());
    let host_rows: Vec<&StoredSession> = rows
        .iter()
        .filter(|row| match row.terminal_kind {
            TerminalKind::Host => true,
            TerminalKind::Tmux => false,
        })
        .collect();
    if host_rows.is_empty() {
        return (found, none_earlier);
    }
    let hosts_dir = absolute_hosts_dir(state_dir);

    // Terminals this supervisor already holds keep their hosts: scanning
    // one a runtime drives would take the host over from it, and it would
    // take the host back. Gone sockets stay gone.
    let mut skip: HashSet<OsString> = gone.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let tab_hosts = tabs.values().flatten().map(|tab| &tab.host);
    for host in adopted.values().map(|(_, host)| host).chain(tab_hosts) {
        if let Some(name) = host.socket().file_name() {
            skip.insert(name.to_os_string());
        }
    }
    let mut pending = Vec::new();
    for row in host_rows {
        match adopted.get(&row.id) {
            Some((generation, host)) if *generation == row.generation => {
                let kept = if still_driven(host) {
                    RowHost::Adopted(host.clone())
                } else {
                    RowHost::Unavailable(host.clone())
                };
                found.insert(row.id.clone(), kept);
            }
            Some(_) | None => pending.push(row),
        }
    }
    if pending.is_empty() {
        return (found, none_earlier);
    }

    let mut named = HashMap::new();
    for row in &pending {
        named.insert(row.id.clone(), named_socket(state_dir, row).await);
    }
    if !owns_state_dir {
        for row in pending {
            let why = "this supervisor does not hold its state directory, so it leaves the \
                       terminal hosts there to the one that does";
            let host = unavailable(
                &hosts_dir,
                &named[&row.id],
                Unavailability::Unidentified(why.into()),
            );
            found.insert(row.id.clone(), host);
        }
        return (found, none_earlier);
    }
    let scan = ScanOptions {
        deadlines: options.deadlines,
        skip,
        only: None,
    };
    // Held through the adoptions too, which return before this does: no
    // spawn of this supervisor starts a host here while the scan could take
    // it over.
    let _no_spawns = match &no_spawns {
        Some(lock) => Some(lock.clone().write_owned().await),
        None => None,
    };
    // The scan: every host here not skipped (at startup) or named (when
    // adopting again) answers its `Info`, and its tag is read, whether or
    // not a row claims it.
    let mut registry = match HostRegistry::scan(state_dir, &scan).await {
        Ok(registry) => registry,
        Err(error) => {
            warn!(error = %error,
                "could not look for the sessions' terminal hosts; leaving every host session \
                 unknown and its host as it is");
            // The host at a socket the row names was never asked, so this can
            // pass, and re-adoption tries that socket again.
            let why = format!("the terminal host directory could not be scanned: {error}");
            for row in pending {
                let host = unavailable(
                    &hosts_dir,
                    &named[&row.id],
                    Unavailability::NotScanned(why.clone()),
                );
                found.insert(row.id.clone(), host);
            }
            return (found, none_earlier);
        }
    };
    {
        let mut gone = gone.lock().unwrap_or_else(|e| e.into_inner());
        for host in registry.gone() {
            gone.insert(host.name.clone());
        }
    }

    let mut adopting = JoinSet::new();
    for row in pending {
        // The row's host: the one its tag names for the row's generation, else
        // an earlier generation's at the socket the row's `terminal_address`
        // names (module docs).
        let claimed = match registry.agent(&row.id, row.generation) {
            Ok(Some(host)) => Ok(Some(host.name.clone())),
            Ok(None) => match earlier_at_address(&registry, row) {
                Some(name) => Ok(Some(name)),
                // Nothing of the row's own generation ever started (no host
                // carries its tag, its `terminal_address` is empty and it has
                // no launch record): a Restart claimed the generation and died
                // before its replacement got a record (module docs).
                None if matches!(named[&row.id], NamedSocket::NoHostStarted) => {
                    Ok(newest_recorded_earlier(&registry, state_dir, row).await)
                }
                None => Ok(None),
            },
            Err(duplicate) => Err(duplicate),
        };
        let host = match claimed {
            Ok(Some(name)) => {
                let host = registry
                    .take(&name)
                    .expect("a host the lookup just found is still in the registry");
                let options = options.clone();
                let id = row.id.clone();
                adopting.spawn(async move {
                    let socket = host.socket.clone();
                    let adopted =
                        HostTerminal::adopt_found(host, Adoption::Recovered, options).await;
                    (id, socket, adopted)
                });
                continue;
            }
            Err(duplicate) => {
                warn!(session = %row.id, error = %duplicate,
                    "several terminal hosts claim this session's agent; leaving them all as they are");
                let socket = duplicate
                    .names
                    .first()
                    .map_or_else(|| hosts_dir.clone(), |name| hosts_dir.join(name));
                RowHost::Unavailable(HostTerminal::unavailable(
                    &socket,
                    Unavailability::Unidentified(duplicate.to_string()),
                    None,
                ))
            }
            Ok(None) => by_named_socket(&registry, &hosts_dir, &named[&row.id], &gone),
        };
        found.insert(row.id.clone(), host);
    }
    while let Some(joined) = adopting.join_next().await {
        // The tasks are this function's own and never aborted; a panic in
        // one is a bug, surfaced where reload was called.
        let (id, socket, adopted) = match joined {
            Ok(adopted) => adopted,
            Err(error) => std::panic::resume_unwind(error.into_panic()),
        };
        let host = match adopted {
            Ok(host) => {
                info!(session = %id, socket = %socket.display(),
                    "found the session's terminal host again and adopted it");
                RowHost::Adopted(host)
            }
            Err(AdoptError::Gone(why)) => {
                info!(session = %id, socket = %socket.display(), %why,
                    "the session's terminal host ended before it could be adopted");
                RowHost::Absent
            }
            Err(AdoptError::Unavailable { reason, peer }) => {
                warn!(session = %id, socket = %socket.display(), %reason,
                    "could not adopt the session's terminal host; leaving it as it is");
                RowHost::Unavailable(HostTerminal::unavailable(&socket, reason, peer))
            }
        };
        found.insert(id, host);
    }
    // What is left live of the rows' earlier agent generations, taken out
    // with its connection so that it outlives the registry.
    let generations: HashMap<&str, i64> = rows
        .iter()
        .map(|row| (row.id.as_str(), row.generation))
        .collect();
    let earlier_names: Vec<OsString> = registry
        .live()
        .iter()
        .filter(|host| {
            match (
                &host.tag.role,
                generations.get(host.tag.session_id.as_str()),
            ) {
                (HostRole::Agent { generation }, Some(current)) => generation < current,
                (HostRole::Agent { .. } | HostRole::Tab { .. }, _) => false,
            }
        })
        .map(|host| host.name.clone())
        .collect();
    let earlier = earlier_names
        .iter()
        .filter_map(|name| registry.take(name))
        .collect();
    (found, EarlierAgentHosts(earlier))
}

/// The live agent hosts of sessions' earlier launch generations that no row
/// claimed at a reload, with the scan's connections to them
/// ([`attribute_host_rows`]): the candidates of the superseded-host rule
/// ([`end_superseded_hosts`]). Dropping it closes the connections, which
/// leaves those hosts as they were.
pub(super) struct EarlierAgentHosts(Vec<LiveHost>);

/// The name of a live host at the socket `row`'s `terminal_address` names whose
/// tag names `row`'s session's agent at an earlier launch generation, if there
/// is one: the row's own host after a Restart that failed definitively (module
/// docs, "What identifies a row's host").
fn earlier_at_address(registry: &HostRegistry, row: &StoredSession) -> Option<OsString> {
    if row.terminal_address.is_empty() {
        return None;
    }
    match registry.by_name(std::ffi::OsStr::new(&row.terminal_address)) {
        Some(NamedHost::Live(host)) if earlier_agent_of(&host.tag, &row.id, row.generation) => {
            Some(host.name.clone())
        }
        Some(NamedHost::Live(_) | NamedHost::Unavailable(_) | NamedHost::Gone(_)) | None => None,
    }
}

/// The name of the host of `row`'s newest earlier launch generation whose
/// host may still hold history, when the scan read that host at the
/// record's socket, tagged for that very launch: the row's terminal when
/// nothing of the row's own generation ever started (module docs, "What
/// identifies a row's host").
///
/// Chosen by the launch records, newest generation first, not by the hosts
/// the scan could read. A record whose host the scan found gone (its socket
/// file left by a host that ended) is passed over for the next older one:
/// that host has no history to keep and nothing to supersede. Any other
/// record that does not name a readable host of its launch ends the search
/// with nothing adopted: a host the scan could not read (busy, stopped), a
/// socket it did not see (whose first stage may still bind it), a host with
/// another tag, or a record that cannot be read. Adopting an older host
/// there would leave a newer one that a later start, reading it, would end
/// as superseded with the newer history; the next start tries again. `None`
/// too when the records cannot be listed in full, since a listing cut short
/// may have missed the newest record.
async fn newest_recorded_earlier(
    registry: &HostRegistry,
    state_dir: &Path,
    row: &StoredSession,
) -> Option<OsString> {
    let launch_dir = state_dir.join("launch");
    let mut entries = tokio::fs::read_dir(&launch_dir).await.ok()?;
    let mut listing = Vec::new();
    loop {
        match entries.next_entry().await {
            Ok(Some(entry)) => listing.push(Ok(entry.file_name())),
            Ok(None) => break,
            Err(error) => {
                listing.push(Err(error));
                break;
            }
        }
    }
    for generation in earlier_recorded_generations(listing, row)? {
        let record = LaunchArtifacts::for_launch(state_dir, &row.id, generation);
        let recorded = read_launch_record(record.host()).await.ok()??;
        let name = OsString::from(recorded.socket.to_string());
        match registry.by_name(&name) {
            Some(NamedHost::Live(host))
                if host.tag.session_id == row.id
                    && host.tag.role == (HostRole::Agent { generation }) =>
            {
                return Some(name);
            }
            Some(NamedHost::Gone(_)) => {}
            Some(NamedHost::Live(_) | NamedHost::Unavailable(_)) | None => return None,
        }
    }
    None
}

/// The earlier launch generations of `row`'s session that have a launch
/// host record, newest first, from the launch directory's `listing` as it
/// was read: `None` when the listing holds an error, since a listing cut
/// short may have missed the newest record, and the fallback must then not
/// choose by what it did see ([`newest_recorded_earlier`]).
fn earlier_recorded_generations(
    listing: impl IntoIterator<Item = std::io::Result<OsString>>,
    row: &StoredSession,
) -> Option<Vec<i64>> {
    let mut generations = Vec::new();
    for name in listing {
        let name = name.ok()?;
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.ends_with(".host") {
            continue;
        }
        // A tab's host record (`LaunchFileOf::Tab`) names a tab host, never
        // the agent's terminal, so only the agent's launch records count.
        if let Some((id, crate::launch::LaunchFileOf::Launch(generation))) =
            crate::launch::parse_launch_file_name(name)
            && id == row.id
            && generation < row.generation
        {
            generations.push(generation);
        }
    }
    generations.sort_unstable_by(|a, b| b.cmp(a));
    Some(generations)
}

/// Whether `tag` names session `session_id`'s agent at a launch generation
/// before `generation`.
fn earlier_agent_of(tag: &HostTag, session_id: &str, generation: i64) -> bool {
    tag.session_id == session_id
        && match tag.role {
            HostRole::Agent { generation: tagged } => tagged < generation,
            HostRole::Tab { .. } => false,
        }
}

/// Whether the live agent host tagged `tag`, at the socket called `name`, is
/// superseded (the plan's D-25, module docs): `row` is its session's row as it
/// stands after the reload, and it must be a host row at a later launch
/// generation than the tag's whose launch is confirmed, which is what a
/// non-empty `terminal_address` naming another socket says (only a confirmation
/// writes `terminal_address`; a Restart empties it as it opens a generation,
/// and one that failed definitively puts back the `terminal_address` of the
/// host that stays the session's terminal). A session with no row is an orphan,
/// which is not this rule's to end; a tab host is not an agent's.
pub(super) fn superseded(
    tag: &HostTag,
    name: &std::ffi::OsStr,
    row: Option<&StoredSession>,
) -> bool {
    let Some(row) = row else {
        return false;
    };
    row.terminal_kind == TerminalKind::Host
        && earlier_agent_of(tag, &row.id, row.generation)
        && row.outcome != LastOutcome::Launching
        && !row.terminal_address.is_empty()
        && std::ffi::OsStr::new(&row.terminal_address) != name
}

/// End the superseded hosts among `earlier` (module docs, "Superseded agent
/// hosts"), deciding each against its session's row as `store` holds it
/// now, after the reload's reconciliation: over the scan's connection
/// (`Shutdown`, then the verified signal to the connection's peer when the
/// host does not end), then the socket file a signal leaves is removed and
/// the host's launch record goes. Every host not superseded is left as it
/// was. The hosts are ended at once, all within one stop budget
/// (`hosts::stop::STOP_BUDGET`, the most one host's end can take), so that a
/// start with several superseded hosts is delayed by the slowest, not their
/// sum; one not confirmed ended within it, or at all, keeps its record,
/// which the session's Delete settles, and the next start tries again.
///
/// `no_spawns` is the supervisor's `host_scan` lock when a supervisor
/// exists (`serve`'s second reload), taken exclusively around each socket
/// removal as that removal requires; the constructor's reload has none,
/// since nothing can spawn a host before the supervisor exists.
pub(super) async fn end_superseded_hosts(
    state_dir: &Path,
    store: &SessionStore,
    earlier: EarlierAgentHosts,
    no_spawns: Option<Arc<tokio::sync::RwLock<()>>>,
) {
    let mut ending = JoinSet::new();
    for host in earlier.0 {
        let row = match store.session(&host.tag.session_id).await {
            Ok(row) => row,
            Err(error) => {
                warn!(session = %host.tag.session_id, error = %format!("{error:#}"),
                    "could not read the session of an earlier launch's terminal host; leaving it");
                continue;
            }
        };
        if !superseded(&host.tag, &host.name, row.as_ref()) {
            continue;
        }
        let HostRole::Agent { generation } = host.tag.role else {
            continue;
        };
        let session_id = host.tag.session_id.clone();
        ending.spawn(async move {
            let socket = host.socket.clone();
            let outcome = crate::hosts::stop::shut_down(
                host.connection,
                &socket,
                crate::hosts::StopLimits::default(),
            )
            .await;
            (session_id, generation, socket, outcome)
        });
    }
    let deadline = tokio::time::Instant::now() + crate::hosts::stop::STOP_BUDGET;
    loop {
        let joined = match tokio::time::timeout_at(deadline, ending.join_next()).await {
            Ok(Some(joined)) => joined,
            Ok(None) => break,
            Err(_) => {
                warn!(
                    left = ending.len(),
                    "terminal hosts that restarts had replaced were not all ended within the \
                     stop budget; the session's Delete or the next start ends them"
                );
                // Dropping the set aborts the ends still running, closing
                // their connections, which leaves those hosts as they are.
                break;
            }
        };
        // The tasks are this function's own and never aborted before here;
        // a panic in one is a bug, surfaced where reload was called.
        let (session_id, generation, socket, outcome) = match joined {
            Ok(ended) => ended,
            Err(error) => std::panic::resume_unwind(error.into_panic()),
        };
        match outcome {
            crate::hosts::HostOutcome::Stopped(how) => {
                info!(session = %session_id, socket = %socket.display(), generation, how = %how,
                    "ended a terminal host its session's restart had replaced");
                {
                    let _no_spawns = match &no_spawns {
                        Some(lock) => Some(lock.write().await),
                        None => None,
                    };
                    remove_ended_socket_file(&socket).await;
                }
                let record = LaunchArtifacts::for_launch(state_dir, &session_id, generation);
                match read_launch_record(record.host()).await {
                    Ok(Some(recorded))
                        if Some(recorded.socket.to_string().as_str())
                            == socket.file_name().and_then(|name| name.to_str()) =>
                    {
                        if let Err(error) = tokio::fs::remove_file(record.host()).await
                            && error.kind() != std::io::ErrorKind::NotFound
                        {
                            warn!(path = %record.host().display(), %error,
                                "could not remove an ended terminal host's launch record");
                        }
                    }
                    Ok(_) => {}
                    Err(error) => warn!(error = %format!("{error:#}"),
                        "could not read an ended terminal host's launch record"),
                }
            }
            crate::hosts::HostOutcome::Unconfirmed(why) => {
                warn!(session = %session_id, socket = %socket.display(), %why,
                    "a terminal host its session's restart had replaced may still run; the \
                     session's Delete or the next start ends it");
            }
        }
    }
}

/// Whether a host of `row`'s own launch may still run, for a session whose
/// entry holds no terminal: the spare-checkout claims at startup protect
/// such a session's claim (the inventory's top risk 3), since its agent,
/// still preparing the checkout, may be the host's child.
///
/// Decided without connecting, so that no host is taken over, and in
/// agreement with the reload that just ran: a launch that was never given a
/// host (unconfirmed, with no launch record) has none, and neither has one
/// whose socket the reload found gone (`gone`; a stale socket file a killed
/// host left may still be on disk, where the startup reclaim, which runs
/// before this check, could not remove it or ran out of time, and must not
/// keep the claim alive). Otherwise a
/// socket file still at the path the row names may be the host's, and a
/// launch record that cannot be read may name one. Errs towards "may",
/// which keeps a claim rather than reclaiming a checkout an agent may still
/// be filling.
pub(super) async fn host_may_remain(
    state_dir: &Path,
    row: &StoredSession,
    gone: &GoneHosts,
) -> bool {
    match named_socket(state_dir, row).await {
        NamedSocket::Name(name) => {
            if gone
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains(std::ffi::OsStr::new(&name))
            {
                return false;
            }
            let socket = absolute_hosts_dir(state_dir).join(name);
            !matches!(std::fs::symlink_metadata(&socket),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound)
        }
        NamedSocket::NoHostStarted => false,
        NamedSocket::Unknown(_) => true,
    }
}

/// `state_dir`'s socket directory, absolute, as a spawn names its socket:
/// what a handle reports and the process-table checks compare is that
/// spelling.
pub(super) fn absolute_hosts_dir(state_dir: &Path) -> std::path::PathBuf {
    match std::path::absolute(state_dir) {
        Ok(state_dir) => crate::hosts::hosts_dir(&state_dir),
        Err(_) => crate::hosts::hosts_dir(state_dir),
    }
}

/// Whether `host`'s runtime still holds its host's connection: not ended
/// gone or unavailable.
fn still_driven(host: &HostTerminal) -> bool {
    match host.liveness() {
        Liveness::Running { .. } | Liveness::Exited { .. } => true,
        Liveness::Gone | Liveness::Unavailable { .. } => false,
    }
}

/// The socket a row names for its host, before any host is asked.
enum NamedSocket {
    /// A socket name: the row's `terminal_address`, or its launch record's.
    Name(String),
    /// The launch was never confirmed and has no launch record, so no host
    /// start ever got as far as being given its spec: there is no host.
    NoHostStarted,
    /// The launch record could not be read, so the socket is not known.
    Unknown(String),
}

/// What `row` names as its host's socket (module docs).
async fn named_socket(state_dir: &Path, row: &StoredSession) -> NamedSocket {
    if !row.terminal_address.is_empty() {
        return NamedSocket::Name(row.terminal_address.clone());
    }
    let record = LaunchArtifacts::for_launch(state_dir, &row.id, row.generation);
    match read_launch_record(record.host()).await {
        Ok(Some(record)) => NamedSocket::Name(record.socket.to_string()),
        Ok(None) => NamedSocket::NoHostStarted,
        Err(error) => NamedSocket::Unknown(format!("{error:#}")),
    }
}

/// The row's host when no live host carries its tag: decided by what is at
/// the socket the row names.
fn by_named_socket(
    registry: &HostRegistry,
    hosts_dir: &Path,
    named: &NamedSocket,
    gone: &GoneHosts,
) -> RowHost {
    let name = match named {
        NamedSocket::Name(name) => name,
        NamedSocket::NoHostStarted => return RowHost::Absent,
        NamedSocket::Unknown(why) => {
            warn!(why = %why,
                "the launch record of a session's unconfirmed terminal host start cannot be \
                 read; leaving the session unknown");
            return unavailable(hosts_dir, named, Unavailability::Unidentified(why.clone()));
        }
    };
    if gone
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(std::ffi::OsStr::new(name))
    {
        return RowHost::Absent;
    }
    let socket = hosts_dir.join(name);
    match registry.by_name(std::ffi::OsStr::new(name)) {
        // A host this build reads, at the row's socket, whose tag names
        // another terminal: not the row's, and not to be guessed at.
        Some(NamedHost::Live(host)) => RowHost::Unavailable(HostTerminal::unavailable(
            &socket,
            Unavailability::Unidentified(format!(
                "the host at its socket is tagged for another terminal ({:?})",
                host.tag
            )),
            host.peer(),
        )),
        Some(NamedHost::Unavailable(host)) => RowHost::Unavailable(HostTerminal::unavailable(
            &socket,
            unavailability(&host.why),
            host.peer,
        )),
        Some(NamedHost::Gone(_)) => RowHost::Absent,
        // Not in the scan at all: absent only when the file is missing.
        // Anything else at the path (an entry the scan could not read, a
        // listing that stopped partway) may still be the row's host, which
        // was never asked: the socket is known, so re-adoption tries it.
        None => match std::fs::symlink_metadata(&socket) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => RowHost::Absent,
            Ok(_) | Err(_) => RowHost::Unavailable(HostTerminal::unavailable(
                &socket,
                Unavailability::NotScanned("the scan did not reach its socket".into()),
                None,
            )),
        },
    }
}

/// A row's host as unavailable for `reason`: at the socket the row names,
/// or at the socket directory itself when the socket is not known (then
/// always [`Unavailability::Unidentified`], since there is no socket to try
/// again); absent when no host start ever got its spec.
fn unavailable(hosts_dir: &Path, named: &NamedSocket, reason: Unavailability) -> RowHost {
    let (socket, reason) = match named {
        NamedSocket::Name(name) => (hosts_dir.join(name), reason),
        NamedSocket::NoHostStarted => return RowHost::Absent,
        NamedSocket::Unknown(why) => (
            hosts_dir.to_path_buf(),
            Unavailability::Unidentified(why.clone()),
        ),
    };
    RowHost::Unavailable(HostTerminal::unavailable(&socket, reason, None))
}

/// Why discovery found a host unavailable, as a runtime's reason.
pub(super) fn unavailability(why: &WhyUnavailable) -> Unavailability {
    match why {
        WhyUnavailable::Connect(reason) => Unavailability::Connect(reason.clone()),
        WhyUnavailable::Busy => Unavailability::Busy,
        WhyUnavailable::Info(error) => Unavailability::Request(error.clone()),
        WhyUnavailable::Tag { tag, .. } => {
            Unavailability::Unidentified(format!("its tag cannot be read: {tag}"))
        }
    }
}

// ---- Adopting again ------------------------------------------------------------------

/// The first retry of an unavailable host, unless a longer backoff is owed.
const READOPT_FIRST: Duration = Duration::from_secs(1);

/// The longest wait between two retries of one unavailable host.
const READOPT_MAX: Duration = Duration::from_secs(60);

/// The terminal whose retries share one backoff, even across healthy runs.
/// An agent's new generation starts its own count; a tab belongs to the
/// session across those generations, and keeps its count until it closes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ReadoptKey {
    Agent { id: String, generation: i64 },
    Tab { id: String, tab_id: String },
}

/// Retry history kept for each session and launch while it still exists.
/// Healthy launches keep their attempt count, but only an active episode
/// has a due time the task waits for. This keeps a past due time from making
/// the task spin after a successful adoption.
#[derive(Default)]
pub(super) struct ReadoptSchedule {
    due: HashMap<ReadoptKey, Retry>,
}

/// One launch's retry count, plus the current unavailability episode.
struct Retry {
    attempts: u32,
    due: tokio::time::Instant,
    active: bool,
}

impl ReadoptSchedule {
    /// How many retries of `id`'s host for `generation` were made,
    /// or attempted while the session was busy: for tests of the backoff.
    #[cfg(test)]
    pub(super) fn attempts(&self, id: &str, generation: i64) -> Option<u32> {
        self.due
            .get(&ReadoptKey::Agent {
                id: id.to_string(),
                generation,
            })
            .map(|retry| retry.attempts)
    }

    /// Make an already scheduled launch due without waiting out its real
    /// backoff. Tests first assert the due time, then use this to exercise
    /// the retry itself without sleeping through the reconnect window.
    #[cfg(test)]
    pub(super) fn make_due(&mut self, id: &str, generation: i64) {
        self.due
            .get_mut(&ReadoptKey::Agent {
                id: id.to_string(),
                generation,
            })
            .expect("the launch was scheduled")
            .due = tokio::time::Instant::now();
    }

    /// Retries made for a tab, including ones deferred by a busy session.
    /// The key has no agent generation, so Restart cannot reset its backoff.
    #[cfg(test)]
    pub(super) fn tab_attempts(&self, id: &str, tab_id: &str) -> Option<u32> {
        self.due
            .get(&ReadoptKey::Tab {
                id: id.to_string(),
                tab_id: tab_id.to_string(),
            })
            .map(|retry| retry.attempts)
    }

    /// Make a scheduled tab due after a test asserted its real deadline,
    /// without waiting through an unstable runtime's reconnect window.
    #[cfg(test)]
    pub(super) fn make_tab_due(&mut self, id: &str, tab_id: &str) {
        self.due
            .get_mut(&ReadoptKey::Tab {
                id: id.to_string(),
                tab_id: tab_id.to_string(),
            })
            .expect("the tab was scheduled")
            .due = tokio::time::Instant::now();
    }
}

/// How long to wait after retry number `attempts` (1 for the first) before
/// the next: 1 s doubling, at most a minute.
fn readopt_delay(attempts: u32) -> Duration {
    READOPT_FIRST
        .saturating_mul(1 << attempts.min(6))
        .min(READOPT_MAX)
}

impl Supervisor {
    /// Start the task that retries unavailable agent and tab hosts (module docs,
    /// "Adopting again"), for a supervisor that holds its state directory.
    /// It holds the supervisor weakly, ends with it, and returns once the
    /// supervisor is stopping.
    ///
    /// It wakes when a retry is due, and when [`Self::readoption_wake`] is
    /// notified (`serve`'s reload left placeholders, the ticker saw a
    /// passing unavailability change, or a stop began). With no active
    /// episode it waits for a wake, even while it keeps healthy terminals'
    /// retry histories.
    pub(super) fn start_readoption(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let wake = Arc::clone(&self.readoption_wake);
        tokio::spawn(async move {
            let mut schedule = ReadoptSchedule::default();
            // The constructor's reload may have left placeholders.
            let mut next = Some(tokio::time::Instant::now());
            loop {
                match next {
                    Some(due) => tokio::select! {
                        () = wake.notified() => {}
                        () = tokio::time::sleep_until(due) => {}
                    },
                    None => wake.notified().await,
                }
                let Some(sup) = weak.upgrade() else {
                    break;
                };
                if sup.hosts_stopping() {
                    break;
                }
                next = sup.readopt_pass(&mut schedule).await;
                #[cfg(test)]
                if let Some(passes) = &sup.seams.readoption_passes {
                    passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }
        });
    }

    /// Whether an orderly stop has begun (`Self::checkpoint_hosts_for_exit`).
    pub(super) fn hosts_stopping(&self) -> bool {
        self.stopping.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Schedule and retry unavailable agent and tab hosts, keeping each
    /// terminal's backoff across successful adoptions. Every new episode starts from
    /// that kept backoff, or at least [`RECONNECT_WINDOW`] when its runtime
    /// gave up as unstable. Healthy launches contribute no next due time.
    ///
    /// Each retry's next due time is set before the retry is made, so no
    /// outcome, a busy session or a failure included, can leave one due at
    /// once again: the loop never spins.
    pub(super) async fn readopt_pass(
        &self,
        schedule: &mut ReadoptSchedule,
    ) -> Option<tokio::time::Instant> {
        if self.hosts_stopping() {
            return None;
        }
        let sessions = self.sessions.lock().await;
        let mut current = HashSet::new();
        let mut unavailable = Vec::new();
        for (id, entry) in sessions.iter() {
            let agent = ReadoptKey::Agent {
                id: id.clone(),
                generation: entry.generation,
            };
            current.insert(agent.clone());
            if let Some(Terminal::Host(host)) = &entry.terminal
                && host.liveness().may_readopt()
            {
                unavailable.push((agent, host.clone()));
            }
            for tab in super::host_tabs::host_tabs_of(&entry.session.host_tabs) {
                let key = ReadoptKey::Tab {
                    id: id.clone(),
                    tab_id: tab.tab_id.clone(),
                };
                current.insert(key.clone());
                if !tab.end_failed && tab.host.liveness().may_readopt() {
                    unavailable.push((key, tab.host));
                }
            }
        }
        // A Restart temporarily takes its entry off the map, but its tab
        // cell remains the session's. Keep those retry counts while it is
        // off-map, without probing anything until the entry returns.
        for (id, cell) in self
            .restarting_tabs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            for tab in super::host_tabs::host_tabs_of(cell) {
                current.insert(ReadoptKey::Tab {
                    id: id.clone(),
                    tab_id: tab.tab_id,
                });
            }
        }
        drop(sessions);
        // Start an episode's wait after observing its unavailable handle.
        // Time spent waiting for the session map must not consume any of
        // an unstable runtime's reconnect window.
        let now = tokio::time::Instant::now();
        let active: HashSet<ReadoptKey> = unavailable.iter().map(|(key, _)| key.clone()).collect();
        schedule.due.retain(|key, retry| {
            if !active.contains(key) {
                retry.active = false;
            }
            current.contains(key)
        });
        for (key, old) in unavailable {
            let retry = schedule.due.entry(key.clone()).or_insert_with(|| Retry {
                attempts: 0,
                due: now + READOPT_FIRST,
                active: false,
            });
            if !retry.active {
                let delay = if matches!(
                    old.liveness(),
                    Liveness::Unavailable {
                        reason: Unavailability::Unstable,
                        ..
                    }
                ) {
                    readopt_delay(retry.attempts).max(RECONNECT_WINDOW)
                } else {
                    readopt_delay(retry.attempts)
                };
                retry.due = now + delay;
                retry.active = true;
            }
            if retry.due > now {
                continue;
            }
            retry.attempts = retry.attempts.saturating_add(1);
            retry.due = tokio::time::Instant::now() + readopt_delay(retry.attempts);
            // Clear the episode at the successful swap, even if the new
            // runtime fails before the next pass sees it healthy. In
            // particular, each new unstable runtime must wait its window.
            let recovered = match key {
                ReadoptKey::Agent { id, generation } => {
                    self.readopt_one(&id, generation, &old).await
                }
                ReadoptKey::Tab { id, tab_id } => self.readopt_tab_one(&id, &tab_id, &old).await,
            };
            if recovered {
                retry.active = false;
            }
            if self.hosts_stopping() {
                return None;
            }
        }
        schedule
            .due
            .values()
            .filter(|retry| retry.active)
            .map(|retry| retry.due)
            .min()
    }

    /// Retry one unavailable agent host (module docs, "Adopting again"): under the
    /// session's lifecycle claim, taken without waiting (a busy session is
    /// retried after the backoff), and only while the entry still holds
    /// `old` for `generation`. Says whether an adopted runtime was
    /// installed, ending this retry episode; an installed placeholder or
    /// absent outcome does not count as a recovery.
    pub(super) async fn readopt_one(&self, id: &str, generation: i64, old: &HostTerminal) -> bool {
        let Some(_claim) = self.lifecycle_locks.try_claim(id) else {
            debug!(session = %id, "a session's lifecycle is busy; retrying its terminal host later");
            return false;
        };
        if !self.holds_retried(id, generation, old).await {
            return false;
        }
        let outcome = self.probe_placeholder(id, generation, old).await;
        let recovered = matches!(&outcome, RowHost::Adopted(_));
        self.install_readopted(id, generation, old, outcome).await && recovered
    }

    /// Whether session `id`'s entry still holds `old` for `generation`, and
    /// no stop has begun.
    async fn holds_retried(&self, id: &str, generation: i64, old: &HostTerminal) -> bool {
        let sessions = self.sessions.lock().await;
        !self.hosts_stopping()
            && sessions.get(id).is_some_and(|entry| {
                entry.generation == generation
                    && matches!(&entry.terminal, Some(Terminal::Host(host)) if host == old)
            })
    }

    /// What is at the one socket unavailable handle `old` names now, for session
    /// `id`'s launch `generation`: its host adopted as recovered when its
    /// tag names that launch or an earlier one of the session's agent, as
    /// at startup; a fresh placeholder when it
    /// still cannot be used; absent when nothing is there. Contacts only
    /// that socket, which a launch drew for itself, so no other runtime's
    /// host is ever taken over.
    pub(super) async fn probe_placeholder(
        &self,
        id: &str,
        generation: i64,
        old: &HostTerminal,
    ) -> RowHost {
        let socket = old.socket();
        let Some(name) = socket.file_name().map(OsString::from) else {
            return RowHost::Unavailable(old.clone());
        };
        let options = Self::host_reload_options(&self.seams, &self.host_liveness);
        let scan = ScanOptions {
            deadlines: options.deadlines,
            skip: HashSet::new(),
            only: Some(HashSet::from([name.clone()])),
        };
        let mut registry = match HostRegistry::scan(&self.state_dir, &scan).await {
            Ok(registry) => registry,
            Err(error) => {
                return RowHost::Unavailable(HostTerminal::unavailable(
                    socket,
                    Unavailability::NotScanned(error.to_string()),
                    None,
                ));
            }
        };
        if registry.gone().iter().any(|host| host.name == name) {
            self.gone_hosts
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(name.clone());
        }
        let hosts_dir = absolute_hosts_dir(&self.state_dir);
        // An unavailable handle's socket is the one its row names (its
        // `terminal_address`, or its generation's launch record), so a host
        // there tagged for the session's agent at an earlier generation is the
        // row's own, as at startup (module docs, "What identifies a row's
        // host").
        let earlier_here = matches!(registry.by_name(&name),
            Some(NamedHost::Live(host)) if earlier_agent_of(&host.tag, id, generation));
        let ours = match registry.agent(id, generation) {
            Ok(found) => Ok(found.is_some() || earlier_here),
            Err(duplicate) => Err(duplicate),
        };
        match ours {
            Ok(true) => {
                let host = registry
                    .take(&name)
                    .expect("the one host scanned is the one found");
                match HostTerminal::adopt_found(host, Adoption::Recovered, options).await {
                    Ok(host) => RowHost::Adopted(host),
                    Err(AdoptError::Gone(_)) => RowHost::Absent,
                    Err(AdoptError::Unavailable { reason, peer }) => {
                        RowHost::Unavailable(HostTerminal::unavailable(socket, reason, peer))
                    }
                }
            }
            // One socket cannot hold two hosts.
            Err(duplicate) => RowHost::Unavailable(HostTerminal::unavailable(
                socket,
                Unavailability::Unidentified(duplicate.to_string()),
                None,
            )),
            Ok(false) => by_named_socket(
                &registry,
                &hosts_dir,
                &NamedSocket::Name(name.to_string_lossy().into_owned()),
                &self.gone_hosts,
            ),
        }
    }

    /// Put a retry's outcome in session `id`'s entry, under
    /// [`Self::host_adoption`] and the session map, only if the entry still
    /// holds `old` for `generation` and no stop has begun; otherwise a
    /// runtime the retry made is stopped, leaving its host as it was. Says
    /// whether it installed the outcome. The entry keeps its run and session
    /// cells. An adopted host whose child the host reports running also
    /// confirms a launch the row has not confirmed (or a stop whose kill
    /// never landed), as a reload would; one whose child the host reaped
    /// leaves that to the observer, once the exit record is replayed.
    pub(super) async fn install_readopted(
        &self,
        id: &str,
        generation: i64,
        old: &HostTerminal,
        outcome: RowHost,
    ) -> bool {
        let installed = {
            let _adoption = self.host_adoption.lock().await;
            let mut sessions = self.sessions.lock().await;
            let current = sessions.get(id).cloned().filter(|current| {
                !self.hosts_stopping()
                    && current.generation == generation
                    && matches!(&current.terminal, Some(Terminal::Host(host)) if host == old)
            });
            current.map(|current| {
                let terminal = match &outcome {
                    RowHost::Adopted(host) => Some(Terminal::Host(host.clone())),
                    RowHost::Unavailable(host) => {
                        // A failed reconnect disproves neither pinned process.
                        let host = match host.liveness() {
                            Liveness::Unavailable { reason, peer } => {
                                old.unavailable_again(reason, peer)
                            }
                            _ => host.clone(),
                        };
                        Some(Terminal::Host(host))
                    }
                    RowHost::Absent => None,
                };
                let replacement = Arc::new(super::SessionEntry {
                    info: current.info.clone(),
                    terminal,
                    run: current.run.clone(),
                    session: current.session.clone(),
                    snapshot: current.snapshot.clone(),
                    canonical_cwd: current.canonical_cwd.clone(),
                    generation: current.generation,
                    scope: current.scope.clone(),
                });
                sessions.insert(id.to_string(), Arc::clone(&replacement));
                replacement
            })
        };
        let Some(replacement) = installed else {
            if let RowHost::Adopted(host) = &outcome {
                host.abort();
            }
            return false;
        };
        match &outcome {
            RowHost::Adopted(host) => {
                info!(session = %id, socket = %host.socket().display(),
                    "adopted a session's terminal host again");
                let recorded = replacement
                    .run
                    .outcome
                    .lock()
                    .expect("outcome mutex poisoned")
                    .clone();
                let child_runs =
                    matches!(host.status().liveness, Liveness::Running { child: Some(_) });
                if child_runs
                    && matches!(
                        recorded,
                        LastOutcome::Launching | LastOutcome::StopRequested
                    )
                {
                    let address = Terminal::Host(host.clone()).recorded_address().to_string();
                    if let Err(error) = self
                        .record(id, &replacement, Transition::ConfirmRunning { address })
                        .await
                    {
                        warn!(session = %id, error = %format!("{error:#}"),
                            "could not confirm a session whose terminal host was adopted again");
                    }
                }
            }
            RowHost::Absent => {
                info!(session = %id,
                    "a session's unavailable terminal host turned out to have ended");
            }
            RowHost::Unavailable(_) => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    //! The superseded-host rule as a pure function of a host's tag and its
    //! session's row, and the claim-window fallback's reading of the launch
    //! directory. That a reload ends a superseded host over the scan's
    //! connection, after confirming the restart's new host, and leaves the
    //! current host and an unreadable one alone, is shown against real hosts
    //! in `crates/farhelm/tests/e2e/host_relaunches.rs`.

    use super::*;
    use crate::service::core::tests::host_terminals::host_row;
    use std::ffi::OsStr;

    /// Shutdown leaves half the reported cores available while keeping one
    /// checkpoint slot even on a one-core machine. Odd core counts round
    /// down so the stop cannot consume more than the agreed half.
    #[farhelm_testtrace::test]
    fn exit_checkpoints_use_half_the_cores_with_at_least_one_slot() {
        for (cores, slots) in [(1, 1), (2, 1), (3, 1), (4, 2), (7, 3), (8, 4)] {
            assert_eq!(exit_checkpoint_slots(cores), slots, "{cores} cores");
        }
    }

    /// The tag of `session`'s agent at launch `generation`.
    fn agent_tag(session: &str, generation: i64) -> HostTag {
        HostTag {
            session_id: session.to_string(),
            created_ns: 1,
            role: HostRole::Agent { generation },
        }
    }

    /// A host row for `s1` at `generation`, naming socket `socket`, with
    /// `outcome`.
    fn row(generation: i64, socket: &str, outcome: LastOutcome) -> StoredSession {
        let mut row = host_row("s1", Path::new("/state/hosts/placeholder"));
        row.generation = generation;
        row.terminal_address = socket.to_string();
        row.outcome = outcome;
        row
    }

    /// An earlier generation's agent host is superseded once its session's
    /// later launch is confirmed in another host, and only then.
    ///
    /// Why: the rule ends hosts at startup, so every case where it must not is
    /// a running program or a session's only terminal lost. Spec: the
    /// generation-0 agent host `OLDHOST1` of `s1` is superseded by a running
    /// (or exited) host row at generation 1 naming `NEWHOST1`; it is not when
    /// the row is still launching with no `terminal_address` (the restart is
    /// unconfirmed), when the row's `terminal_address` names `OLDHOST1` itself
    /// (a restart that failed definitively put it back), when the row is at
    /// generation 0 (it is the current host), when the row is a tmux row, when
    /// the tag names another session or a tab, or when no row exists (an
    /// orphan, not this rule's).
    #[test]
    fn an_earlier_agent_host_is_superseded_only_by_a_confirmed_later_launch() {
        let old = OsStr::new("OLDHOST1");
        let tag = agent_tag("s1", 0);
        let exited = LastOutcome::Exited {
            exit_code: Some(0),
            annotation: None,
        };
        assert!(superseded(
            &tag,
            old,
            Some(&row(1, "NEWHOST1", LastOutcome::Running))
        ));
        assert!(superseded(&tag, old, Some(&row(1, "NEWHOST1", exited))));

        assert!(!superseded(
            &tag,
            old,
            Some(&row(1, "", LastOutcome::Launching))
        ));
        assert!(!superseded(
            &tag,
            old,
            Some(&row(1, "OLDHOST1", LastOutcome::Running))
        ));
        assert!(!superseded(
            &tag,
            old,
            Some(&row(0, "NEWHOST1", LastOutcome::Running))
        ));
        let mut tmux = row(1, "%3", LastOutcome::Running);
        tmux.terminal_kind = TerminalKind::Tmux;
        assert!(!superseded(&tag, old, Some(&tmux)));
        assert!(!superseded(
            &agent_tag("s2", 0),
            old,
            Some(&row(1, "NEWHOST1", LastOutcome::Running))
        ));
        let tab = HostTag {
            role: HostRole::Tab {
                tab_id: "t1".to_string(),
                scoped: false,
            },
            ..agent_tag("s1", 0)
        };
        assert!(!superseded(
            &tab,
            old,
            Some(&row(1, "NEWHOST1", LastOutcome::Running))
        ));
        assert!(!superseded(&tag, old, None));
    }

    /// Only an agent host of the same session at an earlier generation counts
    /// as the row's own at the socket its `terminal_address` names.
    ///
    /// Why: the row's `terminal_address` is what lets a reload keep a session's
    /// terminal after a restart that failed definitively; a host of another
    /// session, a tab, or a later generation there is not the row's and must
    /// stay unavailable rather than be adopted. Spec: for `s1` at generation 2,
    /// `s1`'s agent at generations 0 and 1 counts, at 2 and 3 it does not, nor
    /// does `s2`'s agent at 0 or `s1`'s tab.
    #[test]
    fn an_earlier_agent_of_the_same_session_is_the_rows_own_at_its_address() {
        assert!(earlier_agent_of(&agent_tag("s1", 0), "s1", 2));
        assert!(earlier_agent_of(&agent_tag("s1", 1), "s1", 2));
        assert!(!earlier_agent_of(&agent_tag("s1", 2), "s1", 2));
        assert!(!earlier_agent_of(&agent_tag("s1", 3), "s1", 2));
        assert!(!earlier_agent_of(&agent_tag("s2", 0), "s1", 2));
        let tab = HostTag {
            role: HostRole::Tab {
                tab_id: "t1".to_string(),
                scoped: true,
            },
            ..agent_tag("s1", 0)
        };
        assert!(!earlier_agent_of(&tab, "s1", 2));
    }

    /// The claim-window fallback chooses among the session's earlier launch
    /// records, newest first, only from a launch directory listed in full.
    ///
    /// Why: the sol review's third-round finding. A listing that fails
    /// partway may have yielded an older record and missed a newer one, and
    /// the fallback would then adopt the older host, after which a later
    /// start would end the newer one as superseded with the latest history.
    /// Spec: for `s1`'s row at generation 3, a listing of `s1`'s host
    /// records for generations 0, 2 and 1, its generation-3 record, another
    /// session's record, a spec file, a name that is no launch file and one
    /// of `s1`'s tab records (a tab host is never the agent's terminal)
    /// yields generations 2, 1, 0; the same listing with an error after the
    /// generation-0 record yields nothing.
    #[test]
    fn the_fallback_chooses_only_from_a_whole_listing_of_earlier_records() {
        let row = row(3, "", LastOutcome::Launching);
        let names = |names: &[&str]| -> Vec<std::io::Result<OsString>> {
            names.iter().map(|name| Ok(OsString::from(name))).collect()
        };
        let mut listing = names(&[
            "s1.0.host",
            "s1.2.host",
            "s2.1.host",
            "s1.2.json",
            "s1.3.host",
            "README",
            "s1.tab-7d2e9f40-1c3b-4a5d-8e6f-0a1b2c3d4e5f.host",
            "s1.1.host",
        ]);
        assert_eq!(
            earlier_recorded_generations(listing, &row),
            Some(vec![2, 1, 0])
        );

        listing = names(&["s1.0.host"]);
        listing.push(Err(std::io::Error::other("the listing failed")));
        assert_eq!(earlier_recorded_generations(listing, &row), None);
    }
}
