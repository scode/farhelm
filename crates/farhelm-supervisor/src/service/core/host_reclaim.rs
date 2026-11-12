//! The startup reclaim of orphaned terminal hosts (the plan's PR 20): what a
//! starting supervisor ends, and what it removes, among the hosts and host
//! files that no session or tab its state directory knows of accounts for.
//!
//! A host outlives the supervisor that started it and carries no session
//! marker, so a host whose session is gone keeps running, with its program,
//! until something ends it on purpose. Delete ends every host the session
//! records (`service::session_hosts`) and only then removes the session's
//! launch records and its row, keeping all of them while a host may still
//! run; every host start writes its record first; and a tab's close ends its
//! host before it removes the record. So a live host whose session nothing
//! knows is a leftover made by hand (or by a build whose Delete did not end
//! hosts yet), and the far likelier case behind a host the database does not
//! know is a database that went backwards: removed, replaced, or restored
//! from an older copy, which the desktop app's identity-mismatch message
//! tells people to do. The rules below are built so that the second case
//! never costs a running terminal. SPEC_impl.md's "Cleanup on quit" rules
//! out start-up sweeps without the maintainer's approval; this one has it
//! (the goal's "Approve startup reclaim", quoted there).
//!
//! ## What is known, and what is an orphan
//!
//! A session is known when the store has a row or a create reservation for
//! it ([`crate::store::SessionStore::known_session_ids`]), or when the launch
//! directory holds a host launch record named for it
//! (`<id>.<generation>.host` or `<id>.tab-<tab id>.host`, counted by name, so
//! a record that cannot be read counts too). A tab is known when a runtime
//! of its session holds it or a tab record is named for it. Records survive
//! a lost database (they live beside it, not in it), so a host started under
//! a database that is now missing, replaced or older stays known by its
//! record, at every later start, until its host ends. That reading of
//! "known", broader than the store alone and within the approval's words,
//! is a decision logged for this change (2026-11-02).
//!
//! The records are evidence of knowing only when they were all seen: a
//! launch directory that is missing holds none, but one that cannot be
//! listed, or whose listing stops partway, reclaims nothing that start,
//! since a record not read would leave its session's or tab's live hosts
//! looking like orphans.
//!
//! A live host of a session only its records know is left at every start,
//! and the warning repeats each time: the session has no row, so nothing in
//! the product lists it or can delete it, and its host runs until it is
//! stopped by hand. No command stops one host: ending its process (SIGTERM
//! ends a host at once, closing its terminal; the process is the one whose
//! command line ends with `--socket` and the socket the warning names) does,
//! after which the next start clears its record and socket file.
//!
//! An orphan is a host this build can read whose tag names a session that
//! is not known, or a tab of a known session that is not known. Nothing
//! else is ever ended here:
//!
//! - **An agent host of a known session, and a known tab's host of a known
//!   session**, are never ended, whatever runs in them and whether or not a
//!   runtime holds them. One whose session the store does not know but a
//!   record does is left with a loud warning naming the session and the
//!   socket: its database was most likely restored, replaced or removed. A
//!   superseded agent host of an existing session (an earlier generation's,
//!   which a Restart that the supervisor's end cut short left running) is
//!   the reload's to end, by the rule of `host_handover`'s "Superseded agent
//!   hosts", which has run by the time the reclaim does; one that rule keeps
//!   (its session's later launch is not confirmed yet) is kept here too.
//!   Likewise a host the reload took as a session's terminal through the
//!   restart claim-window fallback is on the session map, and so held.
//! - **A host that cannot be read** is left exactly as it is, never
//!   signalled and its socket and record kept: it does not answer, stays
//!   busy, speaks another protocol version, does not report its `Info`,
//!   carries a tag this build cannot read, or holds a snapshot this build
//!   cannot restore (a newer supervisor may own it and come back after a
//!   downgrade, the goal's "Downgrades and unreadable hosts"). The snapshot
//!   is learned only from the host's stream, so before ending a host the
//!   reclaim adopts it over the connection that read its tag, exactly as a
//!   reload adopts a recovered host (`HostTerminal::adopt_found`, whose
//!   restore is the only snapshot decoder): a host whose adoption fails is
//!   left. The adoption answers the queries its program asked while no
//!   supervisor was attached, as a reload's does, just before the host is
//!   ended.
//! - **Whatever a terminal on the session map holds** is never contacted:
//!   every agent terminal and every tab, runtimes and startup placeholders
//!   alike. Contacting a host takes it over from its runtime, and a
//!   placeholder's socket is re-adoption's to retry.
//! - **Anything at all, when the store has no session and no reservation**
//!   (at every start while that holds, as after a database went missing),
//!   when the launch directory cannot be listed whole (above), or when the
//!   socket directory is in a shape the
//!   supervisor refuses (a symlink, another owner, a looser mode,
//!   `hosts::dir`: a host reached through it may be another installation's).
//!
//! ## How it finds them
//!
//! - **Launch records** of a session the store does not know (one known only
//!   by its records). Each is settled by the rules a retried create uses
//!   ([`Supervisor::settle_launch_record`]): its recorded first stage is
//!   waited for and then fenced with the verified signal, so a start a
//!   crashed supervisor left half done cannot bind its socket after the
//!   reclaim looked, and a socket nothing serves is gone only once no live
//!   process names it. When nothing serves the socket then, its file and the
//!   record are removed; a host that answers is left, with the warning above.
//! - **Tab records of a known session that the reload did not keep**: the
//!   reload adopts every tab record's host it can, keeps a placeholder for
//!   one it cannot use, and drops a tab whose host it found gone, leaving
//!   the record and any socket file a killed host left. Settled the same
//!   way, such a record goes with its socket file once its host is still
//!   gone, as a dead tab's would at its reap; anything else at its socket
//!   is left for the next start's reload.
//! - **A scan of the socket directory**, skipping every socket a terminal
//!   on the map holds and every socket a record above names: a live orphan
//!   is ended, and every socket nothing serves is removed through the
//!   registry's re-verifying removal
//!   (`hosts::registry::GoneHost::remove_socket`). A record that cannot be
//!   read names no socket the scan could skip, but its file name makes its
//!   session known, so its host is left as well.
//!
//! Only regular files are read as records, so a FIFO named like one cannot
//! hold the start.
//!
//! ## Ending an orphan
//!
//! Only the host is ended: once the adoption above has shown it readable,
//! through that runtime (`Supervisor::end_launch_host`: `Shutdown`, else
//! the verified signal to the process the adoption pinned), and its socket
//! file is removed. Ending the host closes its pty, so its program gets the
//! terminal's hangup; descendants that ignore the hangup are not looked for,
//! as `farhelm internal stop-terminal-hosts` does not look for them either.
//!
//! ## When it runs
//!
//! Once per start, in `serve` (a supervisor that only constructs never
//! reclaims), once it holds the right to serve under the state directory's
//! lock and the second reload has reconciled every row (adopting agent
//! hosts and tab hosts, settling reservations), and before the socket is
//! bound: no create, tab open, close or Delete can run beside it, and a
//! client that sees the socket sees a state directory already reclaimed.
//! The scan and the removal of gone sockets hold `host_scan` exclusively,
//! as the reload's scan and every socket removal do; the records and the
//! endings take it only for their own removals.
//!
//! Everything runs within the budget (`SupervisorTimeouts::orphan_reclaim`,
//! [`ORPHAN_RECLAIM_BUDGET`] in production), discovery included: discovery
//! is a task of its own, and one that does not finish in time (a store or a
//! launch directory that stalls) ends the reclaim before anything is ended
//! or removed. After discovery every record, the scan and every ending run
//! at once, each bounded by the client's and the verified signal's own
//! waits; one still running at the budget is dropped where it is, logged,
//! and left for the next start. A task that panics, discovery's included,
//! is logged and its work left, so the reclaim never keeps the supervisor
//! from starting. Nothing is retried while the supervisor runs.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};

use super::Supervisor;
use super::host_launch::{AtRecordedSocket, LaunchHostRecord, read_launch_record};
use super::host_tabs::host_tabs_of;
use crate::hosts::{
    AdoptError, Adoption, GoneHost, HostRegistry, HostRole, HostTag, HostTerminal, LiveHost,
    ScanOptions, SocketRemoval,
};
use crate::launch::LaunchFileOf;

/// How long the startup reclaim takes at most, from its start, before what
/// is still running is dropped and left for the next start.
///
/// Discovery (the store's ids, the launch directory, the records) normally
/// takes milliseconds and is cut off by this bound like the rest. After it,
/// everything runs at once, so this bounds the slowest item, not their sum,
/// and the slowest can need more than one start: a record's first stage
/// that never finishes costs its wait (5 s) and the verified signal (two
/// process-table reads of up to ten seconds each around SIGTERM's and
/// SIGKILL's five-second graces, up to 30 s), then a forked second stage's
/// wait (5 s) and a connect with its refusal window (up to 8 s); an orphan
/// the scan finds is adopted (a connect, two `Info`s and a snapshot restore)
/// and ended (its runtime's `Shutdown` within 15 s, else the verified
/// signal, up to 30 s), and a socket removal costs a refusal window and a
/// walk. Work cut by the budget is left as it is and finished by a later
/// start. The ordinary start reclaims nothing and costs a listing of the
/// launch directory and a scan that contacts no socket.
pub const ORPHAN_RECLAIM_BUDGET: Duration = Duration::from_secs(60);

/// Why the reclaim settles a launch record (module docs, "How it finds
/// them"). Neither kind ever ends a host: a record's host that answers is
/// left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordKind {
    /// A record of a session the store does not know, known only by its
    /// records: cleared when its host is gone, and its host, when one
    /// answers, left with a warning about the database.
    Unbacked,
    /// A tab record of a session the store knows whose tab the reload did
    /// not keep, having found its host gone: cleared when its host is
    /// still gone.
    DroppedTab,
}

/// One launch record the reclaim settles.
struct ReclaimRecord {
    /// The record's own path, removed once what it names is cleared.
    path: PathBuf,
    /// The session the record's name names.
    session_id: String,
    kind: RecordKind,
    record: LaunchHostRecord,
    /// The socket it names, spelled as the spawn spelled it.
    socket: PathBuf,
}

/// What the terminals on the session map hold, as the reclaim must leave it.
struct Held {
    /// The socket file names of every terminal on the map: agent terminals
    /// and tabs, runtimes and placeholders alike. Never contacted.
    sockets: HashSet<OsString>,
    /// The tab ids each session on the map holds in its cell, by session:
    /// a known session's tab record whose tab is not here was dropped by
    /// the reload.
    tabs: HashMap<String, HashSet<String>>,
}

/// What discovery found, for the destructive phase to act on.
struct Discovery {
    judge: Judge,
    held_sockets: HashSet<OsString>,
    records: Vec<ReclaimRecord>,
}

/// What decides whether a host is an orphan (module docs, "What is known,
/// and what is an orphan"), shared by the record tasks, the scan and the
/// endings. Built from discovery's reads alone: judging touches no file.
struct Judge {
    /// Every session id the store knows: rows and reservations.
    stored: HashSet<String>,
    /// Every session id a host launch record in the launch directory is
    /// named for, readable or not.
    recorded: HashSet<String>,
    /// Every (session, tab) a tab record in the launch directory is named
    /// for, readable or not.
    recorded_tabs: HashSet<(String, String)>,
    /// The tab ids each session on the map holds.
    held_tabs: HashMap<String, HashSet<String>>,
}

impl Judge {
    /// Whether the session `session_id` is known: by the store or by a
    /// launch record.
    fn knows(&self, session_id: &str) -> bool {
        self.stored.contains(session_id) || self.recorded.contains(session_id)
    }

    /// Whether the host `tag` describes is an orphan: its session is not
    /// known, or it is a tab of a known session that no runtime holds and no
    /// tab record is named for.
    fn is_orphan(&self, tag: &HostTag) -> bool {
        if !self.knows(&tag.session_id) {
            return true;
        }
        match &tag.role {
            HostRole::Agent { .. } => false,
            HostRole::Tab { tab_id, .. } => {
                let held = self
                    .held_tabs
                    .get(&tag.session_id)
                    .is_some_and(|tabs| tabs.contains(tab_id));
                let recorded = self
                    .recorded_tabs
                    .contains(&(tag.session_id.clone(), tab_id.clone()));
                !held && !recorded
            }
        }
    }

    /// The warning for a live host of `session_id` at `socket` that the
    /// store does not know but a launch record does, when that is the case:
    /// it says what happened and how to stop the host by hand, since no
    /// product verb reaches a session that has no row (module docs).
    fn unbacked(&self, session_id: &str, socket: &Path) -> Option<String> {
        (!self.stored.contains(session_id) && self.recorded.contains(session_id)).then(|| {
            format!(
                "session {session_id} is not in the session database, but its launch record and \
                 a live terminal host of it at {} remain; supervisor.db was likely restored, \
                 replaced or removed, so the host is left running (nothing in Farhelm lists or \
                 deletes it; to stop it, end the process serving that socket with SIGTERM)",
                socket.display()
            )
        })
    }
}

/// What became of one thing the reclaim looked at.
#[derive(Debug)]
enum Outcome {
    /// An orphaned host was confirmed ended, and its socket file removed.
    Ended,
    /// Nothing ran there; what was left (a socket file, a record) is gone.
    Cleared,
    /// Left as it was, with why: a host that cannot be read or is not an
    /// orphan, or an end or removal that could not be confirmed.
    Left(String),
}

/// What one of the reclaim's tasks finished with.
enum Done {
    /// One record, gone socket or orphan, by what it is (a path) and how it
    /// ended up.
    Item(String, Outcome),
    /// The scan, with the live orphans it found (each still on the
    /// connection that read its tag), and what it did with the sockets
    /// nothing served.
    Scanned {
        orphans: Vec<LiveHost>,
        removals: Vec<(String, Outcome)>,
    },
}

impl Supervisor {
    /// End the orphaned hosts of this supervisor's state directory and
    /// remove what is left of hosts that are gone, all within `budget`
    /// (module docs). Returns nothing and keeps nothing from starting:
    /// everything that cannot be settled, in time or at all, and the work of
    /// any task that panicked, is left and logged.
    pub(super) async fn reclaim_orphaned_hosts(self: &Arc<Self>, budget: Duration) {
        let deadline = tokio::time::Instant::now() + budget;
        // A task of its own, so that a panic in it is a log line and not a
        // supervisor that never binds.
        let mut discovering = tokio::spawn({
            let sup = Arc::clone(self);
            async move { sup.discover_orphans().await }
        });
        let discovery = match tokio::time::timeout_at(deadline, &mut discovering).await {
            Ok(Ok(Some(discovery))) => discovery,
            // Logged where it was decided.
            Ok(Ok(None)) => return,
            Ok(Err(join)) => {
                error!(error = %join,
                    "the startup reclaim's discovery failed; no terminal host was ended and nothing \
                     was removed");
                return;
            }
            Err(_) => {
                discovering.abort();
                warn!(
                    ?budget,
                    "the startup reclaim's discovery did not finish within its budget; no \
                     terminal host was ended and nothing was removed, the next start tries again"
                );
                return;
            }
        };
        let Discovery {
            judge,
            held_sockets,
            records,
        } = discovery;
        let judge = Arc::new(judge);
        // The scan skips what the records name: those are settled by the
        // stricter rules above, which fence a start before believing its
        // socket.
        let mut skip = held_sockets;
        skip.extend(
            records
                .iter()
                .filter_map(|record| record.socket.file_name().map(OsString::from)),
        );

        let mut tasks: JoinSet<Done> = JoinSet::new();
        // What each task is, for the ones the budget cuts off or that panic.
        let mut labels: HashMap<tokio::task::Id, String> = HashMap::new();
        for record in records {
            let label = record.path.display().to_string();
            let sup = Arc::clone(self);
            let judge = Arc::clone(&judge);
            let task = tasks.spawn(async move {
                let path = record.path.display().to_string();
                Done::Item(path, sup.reclaim_record(&judge, record).await)
            });
            labels.insert(task.id(), label);
        }
        {
            let sup = Arc::clone(self);
            let judge = Arc::clone(&judge);
            let task = tasks.spawn(async move { sup.scan_for_orphans(&judge, skip).await });
            labels.insert(
                task.id(),
                "the scan of the terminal host directory".to_string(),
            );
        }

        let (mut ended, mut cleared, mut left) = (0usize, 0usize, 0usize);
        let mut tally = |what: &str, outcome: Outcome| match outcome {
            Outcome::Ended => {
                ended += 1;
                info!(what, "ended an orphaned terminal host at startup");
            }
            Outcome::Cleared => {
                cleared += 1;
                debug!(what, "removed what an ended terminal host left");
            }
            Outcome::Left(why) => {
                left += 1;
                warn!(what, %why, "left a terminal host as it is at startup");
            }
        };
        loop {
            let joined = match tokio::time::timeout_at(deadline, tasks.join_next_with_id()).await {
                Ok(Some(joined)) => joined,
                Ok(None) => break,
                Err(_) => {
                    for what in labels.values() {
                        tally(
                            what,
                            Outcome::Left(format!(
                                "not settled within the startup reclaim's {budget:?}; the next \
                                 start tries again"
                            )),
                        );
                    }
                    // Cancelled where each one is: a connection closes and
                    // leaves its host as it was, and a process-table read
                    // that cannot be cancelled keeps its thread until it
                    // returns, as everywhere else.
                    tasks.shutdown().await;
                    break;
                }
            };
            let (id, done) = match joined {
                Ok(done) => done,
                // A bug in an optional cleanup must not keep the supervisor
                // from starting: the task's item is left for the next start.
                Err(join) => {
                    let what = labels
                        .remove(&join.id())
                        .unwrap_or_else(|| "a startup reclaim task".to_string());
                    error!(what, error = %join, "a task of the startup reclaim failed");
                    tally(
                        &what,
                        Outcome::Left("its task failed; the next start tries again".into()),
                    );
                    continue;
                }
            };
            labels.remove(&id);
            match done {
                Done::Item(what, outcome) => tally(&what, outcome),
                Done::Scanned { orphans, removals } => {
                    for (what, outcome) in removals {
                        tally(&what, outcome);
                    }
                    for orphan in orphans {
                        let what = orphan.socket.display().to_string();
                        let sup = Arc::clone(self);
                        let label = what.clone();
                        let task = tasks.spawn(async move {
                            let outcome = sup.end_orphan(orphan).await;
                            Done::Item(what, outcome)
                        });
                        labels.insert(task.id(), label);
                    }
                }
            }
        }
        if ended + cleared + left > 0 {
            info!(
                ended,
                cleared, left, "finished the startup reclaim of orphaned terminal hosts"
            );
        }
    }

    /// The read-only phase (module docs): `None`, logged, when nothing may
    /// be reclaimed this start (a socket directory in a refused shape, a
    /// store that cannot be read or knows nothing). The caller runs it as a
    /// task bounded by the reclaim's deadline.
    async fn discover_orphans(&self) -> Option<Discovery> {
        if let Some(gate) = self.seams.faults.reclaim_discovery_gate() {
            gate().await;
        }
        let state_dir = match std::path::absolute(&self.state_dir) {
            Ok(state_dir) => state_dir,
            Err(error) => {
                warn!(%error, "could not resolve the state directory; reclaiming no terminal host");
                return None;
            }
        };
        // The record paths below are joined onto this directory, so it is
        // checked as a scan checks it, before anything is settled.
        if let Err(error) = crate::hosts::existing_hosts_dir(&state_dir) {
            warn!(%error,
                "the terminal host directory is in a shape this supervisor refuses; reclaiming \
                 no terminal host, and leaving everything there as it is");
            return None;
        }
        let stored = match self.store.known_session_ids().await {
            Ok(stored) => stored,
            Err(error) => {
                warn!(error = %format!("{error:#}"),
                    "could not read which sessions exist; reclaiming no terminal host this start");
                return None;
            }
        };
        if stored.is_empty() {
            info!(
                "the session database knows no session; reclaiming no terminal host, and leaving \
                 every one in the terminal host directory as it is"
            );
            return None;
        }
        let held = self.held_terminals().await;
        let listed = self.reclaimable_records(&stored, &held).await?;
        Some(Discovery {
            judge: Judge {
                stored,
                recorded: listed.recorded,
                recorded_tabs: listed.recorded_tabs,
                held_tabs: held.tabs,
            },
            held_sockets: held.sockets,
            records: listed.records,
        })
    }

    /// The sockets and tabs the terminals on the session map hold (module
    /// docs, "What is known, and what is an orphan"), with the host tabs of
    /// every session a Restart has taken off the map for its window
    /// ([`Supervisor::restarting_tabs`]). Before the bind no Restart can run,
    /// so that map is empty at the reclaim; it is read anyway, so that a tab
    /// a Restart holds can never look like an unknown one.
    async fn held_terminals(&self) -> Held {
        let sessions = self.sessions.lock().await;
        let mut held = Held {
            sockets: HashSet::new(),
            tabs: HashMap::new(),
        };
        // `sessions` before `restarting_tabs`, the struct's lock order.
        let restarting: Vec<(String, super::HostTabs)> = self
            .restarting_tabs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(id, cell)| (id.clone(), Arc::clone(cell)))
            .collect();
        for (id, cell) in restarting {
            let tabs = held.tabs.entry(id).or_default();
            for tab in host_tabs_of(&cell) {
                if let Some(name) = tab.host.socket().file_name() {
                    held.sockets.insert(name.to_os_string());
                }
                tabs.insert(tab.tab_id);
            }
        }
        for (id, entry) in sessions.iter() {
            if let Some(name) = entry
                .terminal
                .as_ref()
                .and_then(|host| host.socket().file_name())
            {
                held.sockets.insert(name.to_os_string());
            }
            let tabs = held.tabs.entry(id.clone()).or_default();
            for tab in host_tabs_of(&entry.session.host_tabs) {
                if let Some(name) = tab.host.socket().file_name() {
                    held.sockets.insert(name.to_os_string());
                }
                tabs.insert(tab.tab_id);
            }
        }
        held
    }

    /// One listing of the launch directory (module docs, "How it finds
    /// them"): the sessions and tabs its host records are named for, every
    /// one counted whatever its file holds, and the records the reclaim
    /// settles, read. A missing launch directory holds no records; one that
    /// cannot be listed, or whose listing stops partway, is `None`, logged,
    /// and nothing is reclaimed this start (module docs, "What is known, and
    /// what is an orphan"). A record to settle that is not a regular file,
    /// cannot be read, or names a socket a terminal on the map holds is left
    /// where it is, logged.
    async fn reclaimable_records(&self, stored: &HashSet<String>, held: &Held) -> Option<Listed> {
        let mut listed = Listed::default();
        let launch_dir = self.state_dir.join("launch");
        let incomplete = |error: &dyn std::fmt::Display| {
            warn!(dir = %launch_dir.display(), %error,
                "the launch directory could not be listed whole; reclaiming no terminal host this \
                 start, since a record not read could leave a known host looking like an orphan");
        };
        let fault = self.seams.faults.reclaim_listing_fault();
        let injected = || std::io::Error::other("an injected listing failure");
        let opened = match fault {
            Some(fault) if fault(None) => Err(injected()),
            Some(_) | None => tokio::fs::read_dir(&launch_dir).await,
        };
        let mut entries = match opened {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Some(listed),
            Err(error) => {
                incomplete(&error);
                return None;
            }
        };
        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(error) => {
                    incomplete(&error);
                    return None;
                }
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(fault) = fault
                && fault(Some(&name))
            {
                incomplete(&injected());
                return None;
            }
            if !name.ends_with(".host") {
                continue;
            }
            let Some((session_id, of)) = crate::launch::parse_launch_file_name(&name) else {
                continue;
            };
            // Known by its name, whatever the file turns out to hold.
            listed.recorded.insert(session_id.to_string());
            if let LaunchFileOf::Tab(tab_id) = of {
                listed
                    .recorded_tabs
                    .insert((session_id.to_string(), tab_id.to_string()));
            }
            let kind = if !stored.contains(session_id) {
                RecordKind::Unbacked
            } else {
                match (of, held.tabs.get(session_id)) {
                    (LaunchFileOf::Tab(tab_id), Some(tabs)) if !tabs.contains(tab_id) => {
                        RecordKind::DroppedTab
                    }
                    // An agent launch's record stays with its session
                    // (Delete settles it), and a known session the map
                    // does not hold (a reservation without a row) has no
                    // reload's verdict on its tabs.
                    (LaunchFileOf::Tab(_), Some(_) | None) | (LaunchFileOf::Launch(_), _) => {
                        continue;
                    }
                }
            };
            let path = entry.path();
            // A FIFO or a device named like a record would hold the read
            // open; only what a record's own write makes is read.
            match entry.file_type().await {
                Ok(kind) if kind.is_file() => {}
                Ok(_) | Err(_) => {
                    warn!(record = %path.display(),
                        "a launch record of a terminal host is not a regular file; leaving it");
                    continue;
                }
            }
            let record = match read_launch_record(&path).await {
                Ok(Some(record)) => record,
                Ok(None) => continue,
                Err(error) => {
                    warn!(record = %path.display(), error = %format!("{error:#}"),
                        "a launch record of a terminal host cannot be read; leaving it, and its \
                         session's hosts with it");
                    continue;
                }
            };
            let socket = match self.recorded_socket(&record) {
                Ok(socket) => socket,
                Err(error) => {
                    warn!(record = %path.display(), error = %format!("{error:#}"),
                        "could not tell where a launch record's terminal host is; leaving it");
                    continue;
                }
            };
            if socket
                .file_name()
                .is_some_and(|name| held.sockets.contains(name))
            {
                warn!(record = %path.display(), socket = %socket.display(),
                    "a launch record names the socket of a terminal another session holds; \
                     leaving both as they are");
                continue;
            }
            listed.records.push(ReclaimRecord {
                path,
                session_id: session_id.to_string(),
                kind,
                record,
                socket,
            });
        }
        listed.records.sort_by(|a, b| a.path.cmp(&b.path));
        Some(listed)
    }

    /// Settle one record and act on what its socket holds (module docs, "How
    /// it finds them"): nothing there clears the socket file and the record,
    /// and anything there is left, a live host of a session only its records
    /// know with the warning about the database.
    async fn reclaim_record(&self, judge: &Judge, item: ReclaimRecord) -> Outcome {
        let settled = match self.settle_launch_record(&item.record, &item.socket).await {
            Ok(settled) => settled,
            Err(why) => {
                return Outcome::Left(format!(
                    "its recorded start (process {}) could not be confirmed ended: {why}",
                    item.record.starter.pid
                ));
            }
        };
        match (item.kind, settled) {
            (_, AtRecordedSocket::Gone) => {
                self.remove_ended_socket(&item.socket).await;
                match remove_record(&item.path).await {
                    Ok(()) => Outcome::Cleared,
                    Err(why) => Outcome::Left(why),
                }
            }
            (RecordKind::Unbacked, AtRecordedSocket::Answered { .. }) => Outcome::Left(
                judge
                    .unbacked(&item.session_id, &item.socket)
                    .unwrap_or_else(|| "a live host answers at its socket".to_string()),
            ),
            // The reload found this tab's host gone, and something answers
            // at its socket now: the next start's reload adopts it, or keeps
            // a placeholder for it, by its tag.
            (RecordKind::DroppedTab, AtRecordedSocket::Answered { .. }) => Outcome::Left(format!(
                "a host answers at {} again; the next start's reload looks at it",
                item.socket.display()
            )),
            (_, AtRecordedSocket::NoAnswer(error)) => Outcome::Left(format!(
                "its socket {} does not answer usefully ({error})",
                item.socket.display()
            )),
            (_, AtRecordedSocket::NoInfo { error, .. }) => Outcome::Left(format!(
                "the host at {} did not report itself ({error})",
                item.socket.display()
            )),
        }
    }

    /// The scan for orphans no record names (module docs, "How it finds
    /// them"), under `host_scan` held exclusively: every socket in the
    /// socket directory not in `skip` is contacted, the live orphans come
    /// back with the connections that read their tags, and every socket
    /// nothing serves is removed, judged gone again first, before the lock
    /// is let go. Everything else the scan reached is left as it was: its
    /// connection closes with the registry.
    async fn scan_for_orphans(&self, judge: &Judge, skip: HashSet<OsString>) -> Done {
        let options = Self::host_reload_options(&self.seams, &self.host_liveness);
        let scan = ScanOptions {
            deadlines: options.deadlines,
            skip,
            only: None,
        };
        let _no_spawns = self.host_scan.write().await;
        let mut registry = match HostRegistry::scan(&self.state_dir, &scan).await {
            Ok(registry) => registry,
            Err(error) => {
                return Done::Scanned {
                    orphans: Vec::new(),
                    removals: vec![(
                        "the scan of the terminal host directory".to_string(),
                        Outcome::Left(format!("it could not run: {error}")),
                    )],
                };
            }
        };
        {
            let mut gone = self.gone_hosts.lock().unwrap_or_else(|e| e.into_inner());
            for host in registry.gone() {
                gone.insert(host.name.clone());
            }
        }
        let mut removing: JoinSet<(String, Outcome)> = JoinSet::new();
        let mut removing_what: HashMap<tokio::task::Id, String> = HashMap::new();
        for host in registry.gone().iter().cloned() {
            let what = host.socket.display().to_string();
            let label = what.clone();
            let task = removing.spawn(async move { (what, removed(host).await) });
            removing_what.insert(task.id(), label);
        }
        let mut removals = Vec::new();
        while let Some(joined) = removing.join_next_with_id().await {
            match joined {
                Ok((_, removal)) => removals.push(removal),
                Err(join) => {
                    let what = removing_what
                        .remove(&join.id())
                        .unwrap_or_else(|| "a stale terminal host socket".to_string());
                    error!(what, error = %join, "removing a stale terminal host socket failed");
                    removals.push((
                        what,
                        Outcome::Left("its removal failed; the next start tries again".into()),
                    ));
                }
            }
        }
        // Never reclaimed, whatever their tags would say (module docs).
        for host in registry.unavailable() {
            debug!(socket = %host.socket.display(), why = %host.why,
                "left a terminal host that cannot be read as it is");
        }
        for host in registry.live() {
            if !judge.is_orphan(&host.tag)
                && let Some(warning) = judge.unbacked(&host.tag.session_id, &host.socket)
            {
                removals.push((host.socket.display().to_string(), Outcome::Left(warning)));
            }
        }
        let names: Vec<OsString> = registry
            .live()
            .iter()
            .filter(|host| judge.is_orphan(&host.tag))
            .map(|host| host.name.clone())
            .collect();
        let orphans = names
            .iter()
            .filter_map(|name| registry.take(name))
            .collect();
        Done::Scanned { orphans, removals }
    }

    /// End an orphaned host found over `host.connection`, the one that read
    /// its tag (module docs, "Ending an orphan"): adopt it, which leaves it
    /// as it is when its snapshot or child cannot be read, then end it
    /// through that runtime and remove the socket file it leaves. When the
    /// adoption pinned the host's process (on every platform that names a
    /// connection's peer), the end never reconnects to the socket: the
    /// runtime's `Shutdown`, else the verified signal to that process.
    async fn end_orphan(&self, host: LiveHost) -> Outcome {
        let tag = host.tag.clone();
        let socket = host.socket.clone();
        debug!(socket = %socket.display(), ?tag,
            "a terminal host is known to no session or tab here; ending it");
        let mut options = Self::host_reload_options(&self.seams, &self.host_liveness);
        // Its exit concerns no session this supervisor serves.
        options.notifier = None;
        let runtime = match HostTerminal::adopt_found(host, Adoption::Recovered, options).await {
            Ok(runtime) => runtime,
            Err(AdoptError::Gone(_)) => {
                self.remove_ended_socket(&socket).await;
                return Outcome::Cleared;
            }
            Err(AdoptError::Unavailable { reason, .. }) => {
                return Outcome::Left(format!(
                    "it cannot be read ({reason}), so it is not taken for an orphan"
                ));
            }
        };
        match self
            .end_launch_host(Some(&runtime), &socket, &tag.session_id)
            .await
        {
            Ok(()) => Outcome::Ended,
            Err(why) => Outcome::Left(format!(
                "an orphaned terminal host could not be confirmed ended: {why}"
            )),
        }
    }
}

/// What one listing of the launch directory found
/// ([`Supervisor::reclaimable_records`]).
#[derive(Default)]
struct Listed {
    /// Every session a host record is named for.
    recorded: HashSet<String>,
    /// Every (session, tab) a tab record is named for.
    recorded_tabs: HashSet<(String, String)>,
    /// The records to settle.
    records: Vec<ReclaimRecord>,
}

/// Remove the socket file of `host`, which the scan found nothing serving,
/// if it is still gone (`GoneHost::remove_socket`, whose precondition the
/// caller's exclusive `host_scan` meets).
async fn removed(host: GoneHost) -> Outcome {
    match host.remove_socket().await {
        SocketRemoval::Removed | SocketRemoval::AlreadyGone => Outcome::Cleared,
        other => Outcome::Left(format!(
            "its socket file was not removed, since it is not settled as gone: {other:?}"
        )),
    }
}

/// Remove a launch record once the host it names is confirmed gone; `Err`
/// with why it is still there.
async fn remove_record(path: &Path) -> Result<(), String> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "its host is gone, but its launch record {} could not be removed: {error}",
            path.display()
        )),
    }
}
