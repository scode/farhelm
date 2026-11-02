//! A session's terminal hosts as Delete sees them: which hosts are the
//! session's, the roots their programs give the process sweep, what
//! Delete's guard must count, and ending every one of them once the sweep
//! is done.
//!
//! A host outlives its program (it keeps the final screen until it is shut
//! down, the plan's D-17) and carries no session marker, so neither the
//! marker sweep nor the end of the session's tmux session ends it. Delete is
//! the one verb that ends a session for good, so it ends the hosts on
//! purpose (the inventory's top risk 7).
//!
//! # Which hosts
//!
//! Delete acts only on what the session itself records, never on a scan of
//! the socket directory (a host found there could be anyone's, and telling
//! whose takes machinery a record makes unnecessary):
//!
//! - **The agent's runtime**, when the session's agent terminal is a host
//!   (`Terminal::Host`). The caller roots the sweep at its child as pinned
//!   at adoption (`hosts::HostStatus::pinned_child_root`) and hands the
//!   runtime to [`SessionHosts::end`].
//! - **The tab runtimes**: the session's tabs in terminal hosts
//!   (`SessionCells::host_tabs`), handed to [`Supervisor::session_hosts`]
//!   beside the agent's terminal and held in the result: their children are
//!   sweep roots as pinned at adoption, their tags' scopes are named, the
//!   guard counts the ones whose shell runs as open tabs (and one that cannot
//!   be read as "cannot tell"), and [`SessionHosts::end`] ends them by the
//!   agent runtime's path. A tab's startup placeholder holds no runtime and
//!   is settled from its record like the rest. A startup placeholder
//!   (`hosts::HostTerminal::is_placeholder`, for a host a restarted
//!   supervisor could not adopt) is no runtime: it holds no connection and
//!   adopted no process, so its socket is settled from the session's launch
//!   record like the rest. A placeholder no record names has nothing but
//!   its host's tag to tie the socket to the session: a host there whose
//!   tag names the session is taken as a recorded one of the session
//!   would be, a socket nothing serves is gone, and anything else (another
//!   tag, an unreadable tag, a busy, wedged or silent process, a path that
//!   is not a host socket) refuses and keeps the session without
//!   signalling anything, since a process that merely names the socket on
//!   its command line is not shown to be the session's.
//! - **The session's launch records** (`<state>/launch/<id>.<generation>.host`,
//!   `service::core::host_launch`, and a tab host's
//!   `<state>/launch/<id>.tab-<tab id>.host`, `service::core::host_tabs`),
//!   written before every host start the supervisor makes for the session
//!   and kept until that host is confirmed ended (the startup sweep of the
//!   launch directory keeps them too). Each one no runtime holds (an agent
//!   host of a session with no terminal right now, an earlier generation's
//!   host left by a crash, a start a crashed supervisor left half done, a tab
//!   host a restarted supervisor could not adopt) is settled the way a
//!   retried create settles its own ([`Supervisor::settle_launch_record`]):
//!   its recorded first stage is fenced first, so that nothing can start a
//!   host there once Delete has looked, and then its socket is looked at.
//!
//! What a record's socket holds decides what Delete does with it:
//!
//! - **nothing** (and no process names it): nothing to end; a socket file a
//!   killed host left is removed, judged gone again first;
//! - **a host whose tag names this session**: the guard counts it (its
//!   program running is a live agent, or an open tab),
//!   its child is pinned over the connection as a sweep root, and it is
//!   ended over that same connection;
//! - **a host whose tag names anything else**: Delete refuses and keeps the
//!   session, touching nothing (this session's record names a socket now
//!   serving someone else's host, which only a person can sort out);
//! - **something that does not answer usefully** (stopped, wedged, busy,
//!   another protocol, a tag this build cannot read) but whose process is
//!   known (the connection's peer credentials, or the process naming the
//!   socket): a guarded Delete cannot tell what runs there and refuses; an
//!   unconditional one, after the sweep, ends that process with the
//!   verified signal, without connecting again. It has no sweep root: its
//!   program is reached by the marker sweep, the scope kill and its hangup.
//!   A host that answers with a valid `Info` but a tag this build cannot
//!   read is treated the same way rather than shut down over the
//!   connection: nothing but the socket's name would tie it to the record,
//!   and that is exactly what a process renamed over the path shares;
//! - **anything else** (no process known, a first stage that could not be
//!   fenced, a record that cannot be read or decoded, such as a newer
//!   release's format): Delete refuses and keeps the session.
//!
//! # Ending them
//!
//! [`SessionHosts::end`] ends every host of the session at once: the
//! agent's runtime and the tab runtimes the way a failed create ends its own
//! host
//! (`Supervisor::end_launch_host`: its `Shutdown`, else the verified signal
//! to the process the runtime adopted, never whatever serves the socket
//! now, so a process renamed over the socket is left alone while the host
//! it replaced is still ended), the others as above, and then the socket
//! file a signal leaves is removed. Settling the records (every record at
//! once) gets [`DELETE_HOST_END_BUDGET`], and ending the hosts gets another
//! such budget of its own, counted from when the ending starts, so that the
//! sweep and the rest of the teardown between the two count against
//! neither; a record not settled, or a host not confirmed ended, within its
//! budget or at all, keeps the session (and its launch records) for a
//! retry, so Delete never reports a session gone while one of its hosts may
//! still run.
//!
//! What keeps a session undeletable until a person deals with it: a record
//! that cannot be read or decoded; a record whose socket serves another
//! terminal's host; a recorded host whose process is not known, whose
//! command line the verified signal rejects (it does not end with
//! `--socket` and this socket's path), or that survives SIGKILL; a
//! recorded host whose child runs but cannot be read to pin it as a root;
//! and a startup placeholder no record names whose socket is served by
//! anything but a host tagged as this session's.
//!
//! # What keeps the connections Delete holds
//!
//! Between settling a record and [`SessionHosts::end`], Delete holds the
//! connection to each answering host of the session, which owns that host
//! until it is dropped. Nothing else connects to a session's recorded hosts
//! meanwhile: the session's lifecycle claim excludes its own create, Restart
//! and tab operations, and re-adoption of a startup placeholder, which takes
//! the same claim (`try_claim`) before it probes the placeholder's socket;
//! and the only other code that settles a record (a retried create) settles
//! its own session's.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

use anyhow::Context as _;
use tracing::debug;

use super::core::{AtRecordedSocket, HostTab, Supervisor, read_launch_record};
use super::terminals::Terminal;
use crate::hosts::stop::{HostOutcome, StopLimits, shut_down, signal};
use crate::hosts::terminal::{Child, pin_child};
use crate::hosts::{
    ConnectError, HostConnection, HostName, HostProcess, HostRole, HostTag, HostTerminal, Liveness,
};
use terminal_host::protocol::Info;

/// How long Delete spends settling a session's launch records, all of them
/// together, and then, separately, ending its terminal hosts, all of them
/// together, before it reports the ones not yet settled or confirmed ended
/// and keeps the session. Each budget counts only its own phase: the process
/// sweep, scope, upload and container work between them count against
/// neither, so a slow sweep never cuts the ending short.
///
/// The slowest single end the steps can describe: its runtime's `Shutdown`
/// within the 15 seconds a failed create gives it, then the verified signal
/// to its process (its command line read within one process-table deadline,
/// then SIGTERM's and SIGKILL's graces, 10 seconds), and the removal of the
/// socket file a signal leaves (the refusal window and one process-table
/// walk). Settling one record takes at most its first stage's wait (5
/// seconds) and the verified signal, the stray wait (5 seconds) and a
/// connect. Records and hosts are each handled at once, so each budget
/// bounds the slowest of its phase, not their sum.
pub(crate) const DELETE_HOST_END_BUDGET: Duration = Duration::from_secs(60);

/// One recorded host of the session that no runtime holds, as settling its
/// record found it.
#[derive(Debug)]
enum RecordedHost {
    /// Nothing serves the socket; a file a killed host left may remain.
    Gone { socket: PathBuf },
    /// A host whose tag names this session, with the connection that owns
    /// it (boxed, being much the largest variant) and its `Info` from that
    /// connection.
    Ours {
        socket: PathBuf,
        connection: Box<HostConnection>,
        tag: HostTag,
        info: Info,
    },
    /// Something that did not answer usefully, and the process serving the
    /// socket, which the verified signal may end.
    Unreadable {
        socket: PathBuf,
        peer: HostProcess,
        why: String,
    },
}

impl RecordedHost {
    /// The socket this host was found at.
    fn socket(&self) -> &std::path::Path {
        match self {
            RecordedHost::Gone { socket }
            | RecordedHost::Ours { socket, .. }
            | RecordedHost::Unreadable { socket, .. } => socket,
        }
    }
}

/// The terminal hosts of one session that no runtime in this supervisor
/// holds, as settling the session's launch records found them (module
/// docs). The agent's runtime, when it has one, is not in here: the caller
/// passes it to [`Self::end`].
///
/// Holding one holds the connection to each answering host of the
/// session, which owns that host until this is dropped (which leaves the
/// hosts as they were) or [`Self::end`] consumes it.
#[derive(Debug)]
pub(crate) struct SessionHosts {
    session_id: String,
    hosts: Vec<RecordedHost>,
    /// The session's tab runtimes (not startup placeholders), whose records
    /// were left to them, as the agent's runtime's is.
    tabs: Vec<HostTab>,
    /// The budget [`Self::end`] gets for ending the hosts, counted from
    /// when it starts: the same length as settling's own, which it does not
    /// share, so that the sweep and the rest of the teardown between the
    /// two are never charged to either.
    budget: Duration,
}

impl Supervisor {
    /// Settle every launch record of `session_id` whose socket no runtime
    /// here holds (module docs, "Which hosts"): neither `agent`'s, when the
    /// session's agent terminal is a host runtime, nor one of `tabs`, the
    /// session's tabs in terminal hosts, whose runtimes the result holds.
    ///
    /// Called under the session's lifecycle claim and directory admission,
    /// so nothing starts a host for this session meanwhile. A session with
    /// no launch records costs one directory listing. An error is a refusal
    /// that changed nothing beyond fencing recorded first stages: a record
    /// that cannot be read or decoded, a first stage that could not be
    /// fenced, a socket serving another terminal's host, or one that does
    /// not answer and whose process is not known.
    pub(crate) async fn session_hosts(
        &self,
        session_id: &str,
        agent: Option<&Terminal>,
        tabs: Vec<HostTab>,
        budget: Duration,
    ) -> anyhow::Result<SessionHosts> {
        let deadline = tokio::time::Instant::now() + budget;
        // A startup placeholder (`HostTerminal::is_placeholder`) holds no
        // connection and adopted no process: its socket is settled from the
        // session's record like any other, so that a recorded first stage is
        // fenced and only a host the record names is ended. One that no
        // record names is settled by its host's tag alone, below.
        let (held, placeholder) = match agent {
            Some(Terminal::Host(host)) if host.is_placeholder() => (None, Some(host)),
            Some(Terminal::Host(host)) => (host.socket().file_name().map(ToOwned::to_owned), None),
            Some(Terminal::Tmux(_)) | None => (None, None),
        };
        // A tab's runtime holds its host as the agent's does; a tab's
        // placeholder, like the agent's, is settled from its record.
        let tabs: Vec<HostTab> = tabs
            .into_iter()
            .filter(|tab| !tab.host.is_placeholder())
            .collect();
        let held_tabs: Vec<&std::ffi::OsStr> = tabs
            .iter()
            .filter_map(|tab| tab.host.socket().file_name())
            .collect();
        let mut records = Vec::new();
        for path in self.launch_records_of(session_id).await? {
            let record = read_launch_record(&path)
                .await?
                .with_context(|| format!("the launch host record {} went away", path.display()))?;
            let socket = self.recorded_socket(&record)?;
            let name = socket.file_name();
            if name != held.as_deref() && !name.is_some_and(|name| held_tabs.contains(&name)) {
                records.push((path, record, socket));
            }
        }
        let unrecorded = match placeholder {
            Some(placeholder) => {
                let socket = self.placeholder_socket(placeholder)?;
                (!records.iter().any(|(_, _, recorded)| *recorded == socket)).then_some(socket)
            }
            None => None,
        };
        // Every record at once, within settling's own budget: each can
        // wait for its first stage and for a stray second stage.
        type Settling<'a> = Pin<Box<dyn Future<Output = anyhow::Result<RecordedHost>> + Send + 'a>>;
        let mut settlings: Vec<Settling<'_>> = records
            .iter()
            .map(|(path, record, socket)| {
                Box::pin(async move {
                    let settled =
                        self.settle_launch_record(record, socket)
                            .await
                            .map_err(|why| {
                                anyhow::anyhow!(
                                    "a start of this session's terminal host recorded in {} \
                                 (process {}) could not be confirmed ended: {why}",
                                    path.display(),
                                    record.starter.pid
                                )
                            })?;
                    classify(session_id, socket.clone(), settled)
                }) as Settling<'_>
            })
            .collect();
        if let Some(socket) = &unrecorded {
            settlings.push(Box::pin(settle_unrecorded_placeholder(
                session_id,
                socket.clone(),
            )));
        }
        let mut settled = join_until(settlings, deadline).await.into_iter();
        let mut hosts = Vec::new();
        for ((path, _, _), settled) in records.iter().zip(settled.by_ref()) {
            match settled {
                Some(host) => hosts.push(host?),
                None => anyhow::bail!(
                    "the terminal host recorded in {} could not be settled within {budget:?}",
                    path.display()
                ),
            }
        }
        if let Some(socket) = &unrecorded {
            match settled.next().flatten() {
                Some(host) => hosts.push(host?),
                None => anyhow::bail!(
                    "this session's terminal host at {} could not be settled within {budget:?}",
                    socket.display()
                ),
            }
        }
        Ok(SessionHosts {
            session_id: session_id.to_owned(),
            hosts,
            tabs,
            budget,
        })
    }

    /// The socket a startup placeholder stands for, spelled the way a launch
    /// record's socket is ([`Supervisor::named_socket`]), so that the record
    /// naming it, if any, compares equal. An error when the placeholder's
    /// path does not end in a host socket name (a startup that could not
    /// tell which socket the row's host is at records the socket directory
    /// itself): then nothing says which host is the session's, and Delete
    /// refuses rather than guess.
    fn placeholder_socket(&self, placeholder: &HostTerminal) -> anyhow::Result<PathBuf> {
        let name = placeholder
            .socket()
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(HostName::parse)
            .with_context(|| {
                format!(
                    "this session's terminal host could not be identified at startup ({}), so \
                     which host to end is not known",
                    placeholder.socket().display()
                )
            })?;
        self.named_socket(&name)
    }

    /// The paths of `session_id`'s launch records, in the launch directory:
    /// every agent launch's (every generation's) and every host tab's
    /// (`launch::LaunchFileOf::Tab`, `<id>.tab-<tab id>.host`). Delete
    /// settles both kinds, and a restart that ended a host reads them all to
    /// remove the ones naming it. A caller acting on the agent's launches
    /// alone must skip the tab records, as a Restart's
    /// `settle_outstanding_launches` does: a tab record names a tab host,
    /// whose shell is not the agent's.
    pub(crate) async fn launch_records_of(&self, session_id: &str) -> anyhow::Result<Vec<PathBuf>> {
        let launch_dir = self.state_dir.join("launch");
        let mut entries = match tokio::fs::read_dir(&launch_dir).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(anyhow::Error::new(error).context(format!(
                    "listing {} for launch records",
                    launch_dir.display()
                )));
            }
        };
        let mut records = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .with_context(|| format!("listing {} for launch records", launch_dir.display()))?
        {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".host")
                && crate::launch::parse_launch_file_name(&name)
                    .is_some_and(|(id, _)| id == session_id)
            {
                records.push(entry.path());
            }
        }
        records.sort();
        Ok(records)
    }
}

/// What Delete does with one settled record (module docs): `Err` for every
/// case that refuses.
fn classify(
    session_id: &str,
    socket: PathBuf,
    settled: AtRecordedSocket,
) -> anyhow::Result<RecordedHost> {
    let unreadable = |socket: PathBuf, peer: Option<HostProcess>, why: String| match peer {
        Some(peer) => Ok(RecordedHost::Unreadable { socket, peer, why }),
        None => Err(anyhow::anyhow!(
            "this session's terminal host at {} cannot be read ({why}) and its process is not \
             known, so it cannot be told ended",
            socket.display()
        )),
    };
    match settled {
        AtRecordedSocket::Gone => Ok(RecordedHost::Gone { socket }),
        AtRecordedSocket::Answered { connection, info } => match HostTag::decode(&info.tag) {
            Ok(tag) if tag.session_id == session_id => Ok(RecordedHost::Ours {
                socket,
                connection,
                tag,
                info,
            }),
            Ok(tag) => Err(anyhow::anyhow!(
                "this session's launch record names the terminal host socket {}, which now \
                 serves a host for another terminal ({tag:?}); nothing was touched",
                socket.display()
            )),
            Err(unreadable_tag) => {
                let peer = connection.peer();
                unreadable(socket, peer, unreadable_tag.to_string())
            }
        },
        AtRecordedSocket::NoAnswer(error) => {
            let peer = match &error {
                ConnectError::Busy { peer } | ConnectError::Unavailable { peer, .. } => *peer,
                ConnectError::Gone(_) => None,
            };
            unreadable(socket, peer, error.to_string())
        }
        AtRecordedSocket::NoInfo { error, peer } => unreadable(socket, peer, error.to_string()),
    }
}

/// Settle the socket of the session's startup placeholder when no launch
/// record of the session names it (module docs, "Which hosts"). With no
/// record there is no first stage to fence and no record tying the socket
/// to the session, so the only evidence that the host there is the
/// session's is the host's own tag: a host that answers with a tag naming
/// `session_id` is [`RecordedHost::Ours`] (counted by the guard, rooted and
/// ended over this connection like a recorded one), and a socket nothing
/// serves is [`RecordedHost::Gone`].
///
/// Everything else is an error, which refuses the Delete and keeps the
/// session, and nothing is ever signalled from here: a host that is busy,
/// wedged, speaks another protocol, does not report its `Info` or carries a
/// tag this build cannot read is only a process whose command line names
/// the socket, which a process renamed over the path shares, so it is never
/// [`RecordedHost::Unreadable`] (whose end is the verified signal).
async fn settle_unrecorded_placeholder(
    session_id: &str,
    socket: PathBuf,
) -> anyhow::Result<RecordedHost> {
    let unproven = |socket: &std::path::Path, why: String| {
        anyhow::anyhow!(
            "this session's terminal host at {} has no launch record, and {why}, so nothing \
             shows it is this session's; nothing was signalled",
            socket.display()
        )
    };
    let mut connection =
        match HostConnection::connect_with(&socket, StopLimits::default().client).await {
            Ok(connection) => connection,
            Err(ConnectError::Gone(_)) => return Ok(RecordedHost::Gone { socket }),
            Err(error) => return Err(unproven(&socket, format!("it does not answer ({error})"))),
        };
    let info = match connection.info().await {
        Ok(info) => info,
        Err(error) => {
            return Err(unproven(
                &socket,
                format!("it did not report itself ({error})"),
            ));
        }
    };
    match HostTag::decode(&info.tag) {
        Ok(tag) if tag.session_id == session_id => Ok(RecordedHost::Ours {
            socket,
            connection: Box::new(connection),
            tag,
            info,
        }),
        Ok(tag) => Err(anyhow::anyhow!(
            "this session's terminal host socket {} now serves a host for another terminal \
             ({tag:?}); nothing was touched",
            socket.display()
        )),
        Err(unreadable) => Err(unproven(
            &socket,
            format!("its tag cannot be read ({unreadable})"),
        )),
    }
}

impl SessionHosts {
    /// The sweep's roots: the program of every answering host of the
    /// session that still runs it, pinned to its start identity over the
    /// connection (two `Info`s around reading the process,
    /// `hosts::terminal::pin_child`), so the sweep reaches the program's
    /// whole tree even where no process in it carries a marker. A host
    /// whose child has been reaped gives no root, never its old pid; an
    /// unreadable host gives none (module docs).
    ///
    /// A tab runtime's child is a root as its runtime pinned it at
    /// adoption, checked against the process table whatever the host now
    /// says, as the agent runtime's is (the teardown's reasoning there).
    ///
    /// An error when a host stops answering, or says its child runs while
    /// the child cannot be read: a child that may still run without a root
    /// is unexamined, and the caller refuses rather than sweep without it.
    pub(crate) async fn sweep_roots(&mut self) -> anyhow::Result<Vec<(u32, u64)>> {
        let mut roots = Vec::new();
        for tab in &self.tabs {
            match tab.host.status().pinned_child_root() {
                Ok(Some(root)) => roots.push((root.pid, root.start)),
                Ok(None) => {}
                Err(why) => anyhow::bail!(
                    "could not read the child of terminal tab {}'s host before deleting ({why}); \
                     refusing to report its process tree cleaned up",
                    tab.tab_id
                ),
            }
        }
        for host in &mut self.hosts {
            let RecordedHost::Ours {
                socket, connection, ..
            } = host
            else {
                continue;
            };
            let (child, info) = pin_child(connection).await.with_context(|| {
                format!(
                    "reading the terminal host at {} before deleting",
                    socket.display()
                )
            })?;
            match child {
                Child::Running(root) => roots.push((root.pid, root.start)),
                Child::Reaped => {}
                Child::Unpinned => anyhow::bail!(
                    "the terminal host at {} says its child {} runs, but the child could not \
                     be read; refusing to sweep without it",
                    socket.display(),
                    info.child_pid
                ),
            }
        }
        Ok(roots)
    }

    /// The scope units of the session's tab hosts whose tags say their
    /// program runs in a scope of its own: named this way since a tab in a
    /// host has no tmux window to read its id from. A tab runtime's tag was
    /// read at its open or adoption ([`HostTab::scoped`]).
    pub(crate) fn scoped_tab_units(&self, session_id: &str) -> Vec<String> {
        let held = self
            .tabs
            .iter()
            .filter(|tab| tab.scoped)
            .filter_map(|tab| crate::scope::tab_unit_name(session_id, &tab.tab_id));
        self.hosts
            .iter()
            .filter_map(|host| match host {
                RecordedHost::Ours {
                    tag:
                        HostTag {
                            role:
                                HostRole::Tab {
                                    tab_id,
                                    scoped: true,
                                },
                            ..
                        },
                    ..
                } => crate::scope::tab_unit_name(session_id, tab_id),
                RecordedHost::Ours { .. }
                | RecordedHost::Gone { .. }
                | RecordedHost::Unreadable { .. } => None,
            })
            .chain(held)
            .collect()
    }

    /// Why Delete's guard cannot let anything through, if it cannot: a
    /// recorded host of the session that does not answer usefully, or a tab
    /// runtime that can no longer read its host, so that whether its program
    /// runs cannot be told.
    pub(crate) fn untold(&self) -> Option<String> {
        self.hosts
            .iter()
            .find_map(|host| match host {
                RecordedHost::Unreadable { socket, why, .. } => Some(format!(
                    "this session's terminal host at {} cannot be read ({why}), so whether its \
                     program is still running cannot be told",
                    socket.display()
                )),
                RecordedHost::Gone { .. } | RecordedHost::Ours { .. } => None,
            })
            .or_else(|| {
                self.tabs.iter().find_map(|tab| match tab.host.liveness() {
                    Liveness::Unavailable { reason, .. } => Some(format!(
                        "terminal tab {}'s host cannot be read ({reason}), so whether its shell \
                         is still running cannot be told",
                        tab.tab_id
                    )),
                    Liveness::Running { .. } | Liveness::Exited { .. } | Liveness::Gone => None,
                })
            })
    }

    /// Why the session's agent counts as alive for Delete's guard, if it
    /// does: an agent host of the session, of any launch generation, whose
    /// program still runs as its `Info` said. Only hosts no runtime holds
    /// are here; the agent terminal's own runtime answers through its probe.
    pub(crate) fn agent_alive(&self) -> Option<String> {
        self.hosts
            .iter()
            .any(|host| {
                matches!(
                    host,
                    RecordedHost::Ours {
                        tag: HostTag {
                            role: HostRole::Agent { .. },
                            ..
                        },
                        info: Info {
                            child_exit: None,
                            ..
                        },
                        ..
                    }
                )
            })
            .then(|| "this session's agent is still running in a terminal host".to_string())
    }

    /// How many of the session's tab hosts still run their program, as
    /// their `Info` said, or as a tab runtime's processed exit record has
    /// not yet said otherwise: open tabs, for Delete's guard.
    pub(crate) fn live_tabs(&self) -> usize {
        let held = self
            .tabs
            .iter()
            .filter(|tab| {
                matches!(
                    tab.host.evidence(),
                    crate::hosts::HostEvidence::Owned { dead: false, .. }
                )
            })
            .count();
        held + self
            .hosts
            .iter()
            .filter(|host| {
                matches!(
                    host,
                    RecordedHost::Ours {
                        tag: HostTag {
                            role: HostRole::Tab { .. },
                            ..
                        },
                        info: Info {
                            child_exit: None,
                            ..
                        },
                        ..
                    }
                )
            })
            .count()
    }

    /// End every host here, the tab runtimes' included, and `agent` (the
    /// agent terminal's runtime, when the session has one), at once, and
    /// confirm each ended, all within the budget counted from this call
    /// (module docs, "Ending them"). A tab runtime is ended the way the
    /// agent's is.
    ///
    /// `Err` lists every host that could not be confirmed ended, or was not
    /// within the budget, which may still run; the caller keeps the session
    /// for a retry. The agent's runtime may have been stopped by then
    /// (`HostTerminal::abort`, so that it does not take its host back
    /// mid-end), in which case the kept session's agent reads as
    /// unreachable until the retry, which ends the process the runtime
    /// adopted (`HostTerminal::adopted_process`, which an abort does not
    /// change) with the verified signal.
    pub(crate) async fn end(
        self,
        sup: &Supervisor,
        agent: Option<HostTerminal>,
    ) -> Result<(), String> {
        let deadline = tokio::time::Instant::now() + self.budget;
        let session_id = self.session_id.as_str();
        // Held here by tests, standing in for an end that does not finish.
        let gate = || async move {
            if let Some(gate) = sup.seams.faults.delete_host_end_gate() {
                gate().await;
            }
        };
        type Ending<'a> = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;
        let mut sockets: Vec<PathBuf> = Vec::new();
        let mut endings: Vec<Ending<'_>> = Vec::new();
        // A placeholder holds no host: `Supervisor::session_hosts` settled
        // its socket into `self.hosts` (from the record naming it, or by its
        // host's tag when none does) or refused, and it is ended from there
        // below. One whose socket is not there was never settled, and is
        // never ended (or signalled) on the strength of the placeholder
        // alone: refuse instead.
        let agent = match agent {
            Some(agent) if agent.is_placeholder() => {
                let settled = self
                    .hosts
                    .iter()
                    .any(|host| host.socket().file_name() == agent.socket().file_name());
                if !settled {
                    return Err(format!(
                        "the terminal host at {} was never settled for this Delete, so it may \
                         still run",
                        agent.socket().display()
                    ));
                }
                None
            }
            agent => agent,
        };
        if let Some(agent) = agent {
            let socket = agent.socket().to_path_buf();
            sockets.push(socket.clone());
            endings.push(Box::pin(async move {
                gate().await;
                sup.end_launch_host(Some(&agent), &socket, session_id).await
            }));
        }
        for tab in self.tabs {
            let socket = tab.host.socket().to_path_buf();
            sockets.push(socket.clone());
            endings.push(Box::pin(async move {
                gate().await;
                sup.end_launch_host(Some(&tab.host), &socket, session_id)
                    .await
            }));
        }
        let limits = StopLimits::default();
        for host in self.hosts {
            match host {
                RecordedHost::Gone { socket } => {
                    sockets.push(socket.clone());
                    endings.push(Box::pin(async move {
                        sup.remove_ended_socket(&socket).await;
                        Ok(())
                    }));
                }
                RecordedHost::Ours {
                    socket, connection, ..
                } => {
                    sockets.push(socket.clone());
                    endings.push(Box::pin(async move {
                        gate().await;
                        confirmed(sup, &socket, shut_down(*connection, &socket, limits).await).await
                    }));
                }
                RecordedHost::Unreadable { socket, peer, why } => {
                    sockets.push(socket.clone());
                    endings.push(Box::pin(async move {
                        gate().await;
                        let outcome = signal(&peer, &socket, &why, limits.client).await;
                        confirmed(sup, &socket, outcome).await
                    }));
                }
            }
        }
        let outcomes = join_until(endings, deadline).await;
        let unconfirmed: Vec<String> = sockets
            .iter()
            .zip(outcomes)
            .filter_map(|(socket, outcome)| match outcome {
                Some(Ok(())) => None,
                Some(Err(why)) => Some(format!(
                    "the terminal host at {} may still run: {why}",
                    socket.display()
                )),
                None => Some(format!(
                    "the terminal host at {} was not confirmed ended within Delete's budget for \
                     its terminal hosts",
                    socket.display()
                )),
            })
            .collect();
        if unconfirmed.is_empty() {
            Ok(())
        } else {
            Err(unconfirmed.join("; "))
        }
    }
}

/// A host's end as confirmed or not, removing the socket file a confirmed
/// end by signal leaves.
async fn confirmed(
    sup: &Supervisor,
    socket: &std::path::Path,
    outcome: HostOutcome,
) -> Result<(), String> {
    match outcome {
        HostOutcome::Stopped(how) => {
            debug!(socket = %socket.display(), how = %how, "ended a deleted session's terminal host");
            sup.remove_ended_socket(socket).await;
            Ok(())
        }
        HostOutcome::Unconfirmed(why) => Err(why),
    }
}

/// Drive every future in `futures` at once, on the caller's task, until all
/// are done or `deadline` passes, and return each one's output in order:
/// `None` for one still running at the deadline, which is dropped there.
/// On the caller's task rather than spawned, so the futures may borrow (the
/// supervisor, here), which a spawned task cannot.
async fn join_until<'a, T>(
    futures: Vec<Pin<Box<dyn Future<Output = T> + Send + 'a>>>,
    deadline: tokio::time::Instant,
) -> Vec<Option<T>> {
    let mut pending: Vec<Option<_>> = futures.into_iter().map(Some).collect();
    let mut outputs: Vec<Option<T>> = pending.iter().map(|_| None).collect();
    let all = std::future::poll_fn(|cx| {
        let mut done = true;
        for (slot, output) in pending.iter_mut().zip(outputs.iter_mut()) {
            if let Some(future) = slot {
                match future.as_mut().poll(cx) {
                    Poll::Ready(value) => {
                        *output = Some(value);
                        *slot = None;
                    }
                    Poll::Pending => done = false,
                }
            }
        }
        if done { Poll::Ready(()) } else { Poll::Pending }
    });
    let _ = tokio::time::timeout_at(deadline, all).await;
    drop(pending);
    outputs
}

#[cfg(test)]
mod tests {
    //! The bounded join. Everything that needs hosts is in
    //! `service::core::tests::host_terminals` (stand-in hosts) and
    //! `crates/farhelm/tests/e2e/host_terminals.rs` and `host_creates.rs`
    //! (real ones).

    use super::*;

    /// The bounded join reports what finished and gives up on the rest at
    /// its deadline.
    ///
    /// Why: Delete's host ending must return within its budget even when
    /// one host's end never finishes, and must still report the ones that
    /// did. Spec: of a future that finishes at once and one that never does,
    /// joined until a deadline 50 ms away, the first's output is `Some` and
    /// the second's `None`, and the join returns.
    #[farhelm_testtrace::test]
    async fn the_bounded_join_gives_up_at_its_deadline() {
        let futures: Vec<Pin<Box<dyn Future<Output = u8> + Send>>> =
            vec![Box::pin(async { 1 }), Box::pin(std::future::pending())];
        let deadline = tokio::time::Instant::now() + Duration::from_millis(50);
        let outputs = tokio::time::timeout(Duration::from_secs(10), join_until(futures, deadline))
            .await
            .expect("the join returns at its deadline");
        assert_eq!(outputs, vec![Some(1), None]);
    }
}
