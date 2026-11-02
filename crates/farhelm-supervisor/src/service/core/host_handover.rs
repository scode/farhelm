//! Keeping host terminals across a supervisor restart (the plan's PR 17):
//! the stopping supervisor leaves each host a fresh snapshot
//! ([`Supervisor::checkpoint_hosts_for_exit`]), the starting one finds the
//! host behind every host row the store holds and adopts it again
//! ([`attribute_host_rows`], the inventory's top risk 2), and a running one
//! keeps trying a host it could not use ([`Supervisor::start_readoption`]).
//! Between a stop and the next start, a host keeps running on its own, its
//! program never noticing.
//!
//! ## Stopping
//!
//! An orderly stop asks every host terminal's runtime for a checkpoint under
//! one deadline, at most as many at once as the machine has cores, and then
//! closes each runtime's connection. A checkpoint is what keeps the next
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
//! encodes at the core count is what lets some finish: started all at once,
//! many full terminals on few cores would share the cores until the
//! deadline and none would finish.
//!
//! Every host with output since its last snapshot gets a checkpoint, however
//! little that output is (a host with none answers at once, with nothing
//! encoded). The cost is the encode, which is of the whole terminal, not of
//! the new output: up to about two seconds of a core for a full scrollback
//! in true color, more for one colored like an image.
//!
//! Closing each connection right after its checkpoint limits the output the
//! stopping supervisor counts as delivered without acting on it to what the
//! host had already written to the connection: that output is replayed by
//! the next supervisor without side effects, so a query in it goes
//! unanswered (the plan's D-7, the delivery ambiguity accepted at a stop as
//! at a crash). Output written after the close is unseen, and its queries
//! are answered.
//!
//! ## Finding the hosts again
//!
//! A host outlives the supervisor that started it, and the supervisor keeps
//! no list of hosts: a restarted supervisor finds them by scanning its socket
//! directory (`crate::hosts::registry`) and matches them to rows here, in
//! [`attribute_host_rows`], the one place a supervisor learns which host
//! belongs to which session. Host rows never go through tmux's pane lookup:
//! their `pane` column holds a socket name (the plan's D-14), which no tmux
//! lookup may read as a pane, and "no such pane" would record a running
//! agent as exited.
//!
//! What identifies a row's host, in order:
//!
//! - **The tag.** A live host whose tag names the row's session and launch
//!   generation is the row's, whatever its socket is called. This is how a
//!   launch that never reached its confirmation (an empty `pane`) is found
//!   again, as tmux's agent-window marker finds an unconfirmed pane.
//! - **The socket the row names**: its `pane` once the launch was confirmed,
//!   else the socket the launch record names (`<state>/launch/<id>.<gen>.host`,
//!   `host_launch`'s "The launch record"). What is there when no live host
//!   carries the row's tag decides between "unavailable" and "absent".
//!
//! Each row ends in one of three classes ([`RowHost`]), and the reload treats
//! them as it treats a found, an unattributed and an absent pane, except that
//! an unavailable host records nothing durable at all, not even a launch
//! sentinel (the plan's D-16, as `status::PaneEvidence::Unattributed` says for
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
//! orphan's, an older generation's) are left as they were: their
//! connections close with the registry. Reclaiming orphans and adopting tabs
//! are later changes'.
//!
//! ## Adopting again
//!
//! A host startup could not adopt for a reason that can pass (it did not
//! answer in time on a loaded machine, was briefly stopped, stayed busy, or
//! the socket directory could not be scanned) is retried while the
//! supervisor runs, by a task started with it ([`Supervisor::start_readoption`]):
//! one placeholder at a time, after 1 s and then doubling to a minute, the
//! next due time set before each attempt and never reset, so a host that
//! keeps failing is asked less and less often and the task never spins. An
//! attempt
//!
//! 1. takes the session's lifecycle claim without waiting (a busy session,
//!    a Stop, Restart or Delete in progress, is retried later), and holds it
//!    to the end, so nothing else acts on the session's host meanwhile;
//! 2. checks that the session's entry still holds that very placeholder for
//!    the same launch generation, and that no orderly stop has begun;
//! 3. looks at the one socket the placeholder names, and nothing else: a
//!    scan restricted to that name, which a launch drew for itself, so no
//!    host another runtime of this supervisor holds or is starting can be
//!    taken over; a host whose tag names the session's launch is adopted as
//!    recovered, and anything else is classified as at startup;
//! 4. under [`Supervisor::host_adoption`] and the session map, checks the
//!    entry and the stop again and swaps the outcome in (a runtime, a new
//!    placeholder, or no terminal for a host that has ended), keeping the
//!    entry's run and session cells; a runtime it made that is not
//!    installed is stopped, leaving its host as it was.
//!
//! Only placeholders are retried. A runtime that becomes unavailable after
//! it ran stays unavailable until the supervisor's next start, whose reload
//! adopts the host afresh; so does a placeholder whose reason will not pass
//! on its own (a protocol, snapshot format or tag of a newer release, a
//! launch record that cannot be read).
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
use super::host_launch::read_launch_record;
use crate::hosts::{
    AdoptError, Adoption, CheckpointOutcome, HostRegistry, HostTerminal, HostTerminalOptions,
    Liveness, NamedHost, NotTaken, ScanOptions, Unavailability, WhyUnavailable,
};
use crate::launch::LaunchArtifacts;
use crate::service::terminals::Terminal;
use crate::store::{LastOutcome, StoredSession, TerminalKind, Transition};

// ---- Stopping ------------------------------------------------------------------------

/// How long past its deadline an orderly stop waits for the encodes it
/// cancelled to end. A cancelled encode gives up within about a millisecond
/// (the encoder looks at its cancel between rows); this only bounds a
/// pathological one, which the process's exit then waits for anyway.
const ENCODE_END_GRACE: Duration = Duration::from_secs(1);

impl Supervisor {
    /// Checkpoint every host terminal this supervisor drives, because the
    /// process is about to exit, and close each one's connection, all by
    /// `budget` (module docs, "Stopping").
    ///
    /// Each runtime's checkpoint is asked for once it gets one of as many
    /// slots as the machine has cores, within the deadline; each runtime is
    /// then stopped ([`HostTerminal::abort`]), which cancels any encode it
    /// still runs, and this waits until every such encode has ended, so
    /// none outlives the stop on the blocking pool. The hosts keep running.
    /// Returns whether every runtime it asked left its host a snapshot of
    /// where its terminal is ([`CheckpointOutcome::Accepted`] or `Current`).
    /// Runtimes that had already ended (an unavailable host, a gone one)
    /// hold no connection and run no encode, and are left out.
    ///
    /// After this, the session map still holds the stopped runtimes, which
    /// keep answering from their last status (an exit, a child that runs)
    /// and refuse every request: this is for a supervisor that is exiting.
    pub(crate) async fn checkpoint_hosts_for_exit(&self, budget: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + budget;
        // The map's lock is held only for map operations, but the deadline
        // bounds even a wait for it. `stopping` is set under the same lock:
        // a re-adoption installs a runtime only under it, after checking the
        // flag, so none appears in the map after this gathers it.
        let gathered = tokio::time::timeout_at(deadline, async {
            let sessions = self.sessions.lock().await;
            self.stopping
                .store(true, std::sync::atomic::Ordering::SeqCst);
            let mut all: Vec<HostTerminal> = Vec::new();
            for entry in sessions.values() {
                match &entry.terminal {
                    Some(Terminal::Host(host)) if !all.contains(host) => all.push(host.clone()),
                    Some(Terminal::Host(_) | Terminal::Tmux(_)) | None => {}
                }
            }
            all
        })
        .await;
        self.stopping
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.readoption_wake.notify_one();
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
        let hosts: Vec<HostTerminal> = all
            .iter()
            .filter(|host| still_driven(host))
            .cloned()
            .collect();
        if hosts.is_empty() {
            close_all(&all, deadline + ENCODE_END_GRACE).await;
            return true;
        }
        info!(
            count = hosts.len(),
            "stopping: checkpointing terminal hosts before exit"
        );
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        let slots = Arc::new(tokio::sync::Semaphore::new(cores));
        let mut checkpoints = JoinSet::new();
        for host in hosts.iter().cloned() {
            let slots = Arc::clone(&slots);
            checkpoints.spawn(async move {
                let outcome = match tokio::time::timeout_at(deadline, slots.acquire_owned()).await {
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

    /// Stop every host terminal now in the session map and wait, until
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
            let mut all: Vec<HostTerminal> = Vec::new();
            for entry in sessions.values() {
                match &entry.terminal {
                    Some(Terminal::Host(host)) if !all.contains(host) => all.push(host.clone()),
                    Some(Terminal::Host(_) | Terminal::Tmux(_)) | None => {}
                }
            }
            all
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
pub(super) async fn attribute_host_rows(
    state_dir: &Path,
    discovery: HostDiscovery,
    rows: &[StoredSession],
) -> HashMap<String, RowHost> {
    let HostDiscovery {
        owns_state_dir,
        options,
        adopted,
        no_spawns,
        gone,
    } = discovery;
    let mut found = HashMap::new();
    let host_rows: Vec<&StoredSession> = rows
        .iter()
        .filter(|row| match row.terminal_kind {
            TerminalKind::Host => true,
            TerminalKind::Tmux => false,
        })
        .collect();
    if host_rows.is_empty() {
        return found;
    }
    let hosts_dir = absolute_hosts_dir(state_dir);

    // Terminals this supervisor already holds keep their hosts: scanning
    // one a runtime drives would take the host over from it, and it would
    // take the host back. Gone sockets stay gone.
    let mut skip: HashSet<OsString> = gone.lock().unwrap_or_else(|e| e.into_inner()).clone();
    for (_, host) in adopted.values() {
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
        return found;
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
        return found;
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
            return found;
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
        let host = match registry.agent(&row.id, row.generation) {
            Ok(Some(host)) => {
                let name = host.name.clone();
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
    found
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
/// host left stays on disk until the startup reclaim of a later change
/// removes it, and must not keep the claim alive meanwhile). Otherwise a
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
fn absolute_hosts_dir(state_dir: &Path) -> std::path::PathBuf {
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
    /// A socket name: the row's `pane`, or its launch record's.
    Name(String),
    /// The launch was never confirmed and has no launch record, so no host
    /// start ever got as far as being given its spec: there is no host.
    NoHostStarted,
    /// The launch record could not be read, so the socket is not known.
    Unknown(String),
}

/// What `row` names as its host's socket (module docs).
async fn named_socket(state_dir: &Path, row: &StoredSession) -> NamedSocket {
    if !row.pane.is_empty() {
        return NamedSocket::Name(row.pane.clone());
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
fn unavailability(why: &WhyUnavailable) -> Unavailability {
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

/// The first retry of a placeholder, after the reload that made it.
const READOPT_FIRST: Duration = Duration::from_secs(1);

/// The longest wait between two retries of one placeholder.
const READOPT_MAX: Duration = Duration::from_secs(60);

/// When each startup placeholder is retried next, by session and launch
/// generation: the re-adoption task's own state. A retry's attempts are
/// never reset, so a host that keeps failing is asked less and less often.
#[derive(Default)]
pub(super) struct ReadoptSchedule {
    due: HashMap<(String, i64), Retry>,
}

/// One placeholder's retries so far, and when the next is due.
struct Retry {
    attempts: u32,
    due: tokio::time::Instant,
}

impl ReadoptSchedule {
    /// How many retries of `id`'s placeholder for `generation` were made,
    /// or attempted while the session was busy: for tests of the backoff.
    #[cfg(test)]
    pub(super) fn attempts(&self, id: &str, generation: i64) -> Option<u32> {
        self.due
            .get(&(id.to_string(), generation))
            .map(|retry| retry.attempts)
    }
}

/// How long to wait after retry number `attempts` (1 for the first) before
/// the next: 1 s doubling, at most a minute.
fn readopt_delay(attempts: u32) -> Duration {
    READOPT_FIRST
        .saturating_mul(1 << attempts.min(6))
        .min(READOPT_MAX)
}

/// A placeholder re-adoption retries: one startup made, for a reason that
/// may pass.
fn retried_placeholder(host: &HostTerminal) -> bool {
    host.is_placeholder()
        && match host.liveness() {
            Liveness::Unavailable { reason, .. } => reason.may_pass(),
            Liveness::Running { .. } | Liveness::Exited { .. } | Liveness::Gone => false,
        }
}

impl Supervisor {
    /// Start the task that retries startup placeholders (module docs,
    /// "Adopting again"), for a supervisor that holds its state directory.
    /// It holds the supervisor weakly, ends with it, and returns once the
    /// supervisor is stopping.
    ///
    /// It wakes when a retry is due, and when [`Self::readoption_wake`] is
    /// notified (`serve`'s reload left placeholders; a stop began). With
    /// nothing scheduled it waits for a wake: placeholders are made only by
    /// the reloads, and a runtime that becomes unavailable later is not
    /// retried.
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
            }
        });
    }

    /// Whether an orderly stop has begun (`Self::checkpoint_hosts_for_exit`).
    fn hosts_stopping(&self) -> bool {
        self.stopping.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// One pass of re-adoption: schedule each startup placeholder not yet
    /// scheduled (first retry [`READOPT_FIRST`] from now), retry the ones
    /// that are due one at a time, and say when the next is due (`None`
    /// with nothing scheduled, or once stopping).
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
        let now = tokio::time::Instant::now();
        let placeholders: Vec<(String, i64, HostTerminal)> = self
            .sessions
            .lock()
            .await
            .iter()
            .filter_map(|(id, entry)| match &entry.terminal {
                Some(Terminal::Host(host)) if retried_placeholder(host) => {
                    Some((id.clone(), entry.generation, host.clone()))
                }
                Some(Terminal::Host(_) | Terminal::Tmux(_)) | None => None,
            })
            .collect();
        let current: HashSet<(String, i64)> = placeholders
            .iter()
            .map(|(id, generation, _)| (id.clone(), *generation))
            .collect();
        schedule.due.retain(|key, _| current.contains(key));
        for (id, generation, old) in placeholders {
            let retry = schedule
                .due
                .entry((id.clone(), generation))
                .or_insert_with(|| Retry {
                    attempts: 0,
                    due: now + READOPT_FIRST,
                });
            if retry.due > now {
                continue;
            }
            retry.attempts += 1;
            retry.due = tokio::time::Instant::now() + readopt_delay(retry.attempts);
            self.readopt_one(&id, generation, &old).await;
            if self.hosts_stopping() {
                return None;
            }
        }
        schedule.due.values().map(|retry| retry.due).min()
    }

    /// Retry one placeholder (module docs, "Adopting again"): under the
    /// session's lifecycle claim, taken without waiting (a busy session is
    /// retried after the backoff), and only while the entry still holds
    /// `old` for `generation`. Says whether the outcome was installed.
    async fn readopt_one(&self, id: &str, generation: i64, old: &HostTerminal) -> bool {
        let Some(_claim) = self.lifecycle_locks.try_claim(id) else {
            debug!(session = %id, "a session's lifecycle is busy; retrying its terminal host later");
            return false;
        };
        if !self.holds_placeholder(id, generation, old).await {
            return false;
        }
        let outcome = self.probe_placeholder(id, generation, old).await;
        self.install_readopted(id, generation, old, outcome).await
    }

    /// Whether session `id`'s entry still holds `old` for `generation`, and
    /// no stop has begun.
    async fn holds_placeholder(&self, id: &str, generation: i64, old: &HostTerminal) -> bool {
        let sessions = self.sessions.lock().await;
        !self.hosts_stopping()
            && sessions.get(id).is_some_and(|entry| {
                entry.generation == generation
                    && matches!(&entry.terminal, Some(Terminal::Host(host)) if host == old)
            })
    }

    /// What is at the one socket placeholder `old` names now, for session
    /// `id`'s launch `generation`: its host adopted as recovered when its
    /// tag names that launch, as at startup; a fresh placeholder when it
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
        match registry.agent(id, generation) {
            Ok(Some(_)) => {
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
            Ok(None) => by_named_socket(
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
                    RowHost::Adopted(host) | RowHost::Unavailable(host) => {
                        Some(Terminal::Host(host.clone()))
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
                    let pane = Terminal::Host(host.clone()).recorded_pane().to_string();
                    if let Err(error) = self
                        .record(id, &replacement, Transition::ConfirmRunning { pane })
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
