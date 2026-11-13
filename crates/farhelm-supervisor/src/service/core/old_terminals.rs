//! Ending the old terminals: the private tmux server earlier releases ran
//! every session in, and what still ran in it when this release started.
//!
//! Releases before terminal hosts ran each session's agent and shell tabs
//! in one tmux server per supervisor, on `<state dir>/tmux.sock`. This
//! release ends that server at startup and treats whatever ran in it as
//! ended (SPEC.md, "Terminal experience"; SPEC_impl.md, "Ending the
//! old private tmux server"). It is the only tmux-related behavior
//! the running supervisor keeps, and a later release removes it once no
//! host can still have such a server (TODO.md's phase 3 entry).
//!
//! ## What it reads and what it signals
//!
//! No tmux program runs, and none needs to be installed: the server is
//! reached with a plain connect to its socket.
//!
//! - **Nothing answers**: the socket file is missing, or (on Linux) the
//!   connection is refused, which is a socket no process listens on. The
//!   leftover `tmux.sock`, `tmux.conf` and `.tmux.conf.tmp-*` files are
//!   removed. Any other failure to connect leaves the files in place and is
//!   logged: a wedged server with a full backlog answers `EAGAIN` on Linux,
//!   and macOS refuses a connection to a live listener the same way it
//!   refuses one to a dead socket, so neither proves the server is gone,
//!   and a later start can still reach it. That residual is accepted.
//! - **Something answers**: its process is taken from the connection's peer
//!   credentials ([`crate::hosts::process::peer_process`]) and refused
//!   unless it belongs to this user. Its subtree is snapshotted, and each
//!   pane process (a direct child of the server) is attributed to a session
//!   and to its agent or one of its tabs by the `FARHELM_*` markers its
//!   environment carries. Then the server is ended with SIGTERM and, after
//!   a bound, SIGKILL ([`crate::hosts::process::terminate_process_within`]),
//!   each signal sent only while the process is still the one pinned and its
//!   command line still names the socket with `-S`, and the socket and config
//!   files are removed once it is gone.
//!
//! ## Cleaning up what ran there
//!
//! Ending the server hangs up its panes, which ends a well-behaved agent
//! and shell; detached daemons outlive it, as they outlive an old pane
//! that exits. They are swept by the session's stored identity, not by
//! anything the server said, so the cleanup works when no server answered:
//!
//! - every old-kind row the reload converts to interrupted (an agent that
//!   was launching, running or being stopped) is swept as a Stop of an
//!   agent that already exited is ([`SweepTarget::AgentOnly`] through the
//!   row's recorded launch scope, which also covers the session-only marker
//!   shape agents of the oldest releases carry), with the processes the
//!   snapshot found under that session's agent pane as extra roots;
//! - old tabs are swept one by one ([`SweepTarget::Tab`]) wherever their ids
//!   can be found: from a tab-marked pane in the snapshot, and from the tab
//!   scope units the user manager lists for a session whose agent was
//!   converted or whose old tab the snapshot found, skipping any tab that
//!   has a terminal host's launch record, which is a new tab of the same
//!   session.
//!
//! A session is never swept as a whole: a session whose agent or tabs
//! already run in terminal hosts carries the same session marker there.
//! Daemons of old tabs whose ids cannot be found are left for that
//! session's Delete, which sweeps the whole session; that residual is
//! accepted too. Once the server is gone (before this start, or unreadable),
//! an old tab's id is found only through the scope units of a session whose
//! agent this start converted; so a scoped tab of any other session (one
//! whose agent had already exited, errored or been interrupted, or runs in a
//! terminal host) is found only by its Delete, and so is an unscoped tab, or
//! one whose shell's environment macOS hides.
//!
//! ## Failure is never fatal
//!
//! The maintainer chose "start anyway and treat old sessions as ended":
//! every failure here is logged and startup goes on, with no retry state,
//! refusal or lock-out, accepting that a Restart could then start a second
//! agent beside one that still runs. The step's own work, ending the server
//! and the sweeps together, has one budget ([`OLD_TERMINALS_DEADLINE`]) that
//! keeps a restarted supervisor well inside the time the helm waits for it
//! to come back during Update; the rest of the reload between the two (host
//! discovery) is not charged to it. What a sweep that ran out of time did
//! not reach waits for the session's Restart or Delete: the conversion
//! commits regardless, so a later start does not sweep that row again.
//!
//! A crash between the conversion's commit and the writes after it (the
//! launch artifacts' removal and the notice, [`record_ended`]) leaves the
//! row interrupted without its notice, and its launch files for the
//! session's Delete; a later start does not convert it again. Startup-only
//! and short, so accepted, like the reboot conversion's same window.

use super::super::notifications::{NotificationKind, OLD_TERMINAL_ENDED_TEXT};
use super::{ScopeKillFailure, ScopeUnits, SweepTarget, launch_scope_unit, reap_process_tree};
use crate::hosts::process::{
    TerminateError, Termination, names_tmux_server_socket, peer_process, terminate_process_within,
};
use crate::store::SessionStore;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::Instant;
use tracing::{info, warn};

/// The budget for the startup step's own work: ending the private server
/// and sweeping what ran in it, not the reload's other work between them.
///
/// Well under the 30 seconds the helm allows a supervisor it has just
/// restarted, during Update, to come back
/// (`farhelm-helm`'s provisioning `ATTACH_TIMEOUT`), which also has to
/// cover the rest of startup. Ending the server takes at most its two signal
/// graces; the rest is for the sweeps, each of which gives SIGTERM up to
/// five seconds before it escalates.
pub(crate) const OLD_TERMINALS_DEADLINE: Duration = Duration::from_secs(15);

/// How long the server gets to exit after SIGTERM before SIGKILL. tmux
/// handles SIGTERM by exiting at once; this is room for a loaded machine.
const SERVER_TERM_GRACE: Duration = Duration::from_secs(2);

/// How long the server gets to go after SIGKILL.
const SERVER_KILL_GRACE: Duration = Duration::from_secs(2);

/// The private server's files in the state directory.
const SOCKET: &str = "tmux.sock";
const CONFIG: &str = "tmux.conf";

/// The prefix of the config's staged writes (`crate::files`), which a crash
/// mid-write left beside it.
const CONFIG_TEMP_PREFIX: &str = ".tmux.conf.tmp-";

/// Which terminal of a session an old pane ran, as its markers say.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PaneOwner {
    /// The session's agent: its agent marker, or (the oldest launches) the
    /// session marker alone.
    Agent(String),
    /// One of the session's tabs, by its tab id.
    Tab { session: String, tab: String },
}

/// What one start learned about the old terminals, carried from ending the
/// server (before the reload) to the reload's sweep of the rows it
/// converts.
pub(crate) struct OldTerminals {
    /// How much of [`OLD_TERMINALS_DEADLINE`] ending the server spent; the
    /// sweep gets the rest.
    spent: Duration,
    /// The processes of the server's subtree, each with its start
    /// identity, grouped by the pane they descend from. Extra roots for the
    /// sweeps, and the only source of old tab ids while the server ran.
    panes: HashMap<PaneOwner, Vec<(u32, u64)>>,
}

impl OldTerminals {
    /// No server was looked for (a supervisor without the state
    /// directory's claim, or `serve`'s second reload): the reload still
    /// converts and sweeps old rows by their stored identity, within the
    /// whole budget.
    pub(crate) fn none() -> Self {
        Self {
            spent: Duration::ZERO,
            panes: HashMap::new(),
        }
    }

    /// Find the private server on `state_dir`'s socket and end it, as the
    /// module docs describe, recording the time it took. Never fails: what
    /// goes wrong is logged.
    pub(crate) async fn end_private_server(state_dir: &Path) -> Self {
        let started = Instant::now();
        let panes = end_server(state_dir, started + OLD_TERMINALS_DEADLINE).await;
        Self {
            spent: started.elapsed(),
            panes,
        }
    }
}

/// [`OldTerminals::end_private_server`]'s work, within `deadline`: the
/// server's subtree as sweep roots, empty unless the process answering was
/// verified as the server and signalled (finding no server, or leaving an
/// unverified process alone, yields none).
async fn end_server(state_dir: &Path, deadline: Instant) -> HashMap<PaneOwner, Vec<(u32, u64)>> {
    let none = HashMap::new;
    let socket = state_dir.join(SOCKET);
    let stream = match tokio::net::UnixStream::connect(&socket).await {
        Ok(stream) => stream,
        Err(error) if no_server(&error) => {
            remove_server_files(state_dir).await;
            return none();
        }
        Err(error) => {
            warn!(socket = %socket.display(), %error,
                    "could not connect to the private tmux socket an earlier release used; leaving \
                     it and its files for a later start");
            return none();
        }
    };
    let owned = stream
        .peer_cred()
        // SAFETY: geteuid has no preconditions and cannot fail.
        .is_ok_and(|cred| cred.uid() == unsafe { libc::geteuid() });
    let Some(server) = peer_process(&stream).filter(|_| owned) else {
        warn!(socket = %socket.display(),
                "something answers on the private tmux socket an earlier release used, but it is \
                 not a process of this user that could be identified; leaving it alone");
        return none();
    };
    drop(stream);
    let pid = server.pid;
    let snapshot = tokio::time::timeout_at(
        deadline,
        tokio::task::spawn_blocking(move || snapshot_panes(pid)),
    );
    let panes = match snapshot
        .await
        .unwrap_or_else(|_| Ok(Err("it ran out of time".into())))
    {
        Ok(Ok(panes)) => panes,
        Ok(Err(error)) => {
            warn!(%error, "could not read the processes of the private tmux server; ending it \
                    all the same, and sweeping its sessions by their records alone");
            HashMap::new()
        }
        Err(join) => {
            warn!(error = %join, "reading the private tmux server's processes failed");
            HashMap::new()
        }
    };
    let ended = tokio::time::timeout_at(
        deadline,
        terminate_process_within(
            &server,
            &socket,
            names_tmux_server_socket,
            SERVER_TERM_GRACE,
            SERVER_KILL_GRACE,
        ),
    )
    .await;
    match ended {
        // The process the socket's peer credentials named was gone before
        // anything signalled it, so nothing shows the snapshot, taken by a
        // bare pid, was its subtree: another process may have taken the pid
        // since. None of it is swept; the converted rows are still swept by
        // their records.
        Ok(Ok(Termination::AlreadyGone)) => {
            info!(socket = %socket.display(),
                    "the private tmux server an earlier release ran sessions in ended before it \
                     was signalled; sweeping its sessions by their records alone");
            remove_server_files(state_dir).await;
            none()
        }
        Ok(Ok(Termination::Terminated | Termination::Killed)) => {
            info!(socket = %socket.display(), panes = panes.len(),
                    "ended the private tmux server an earlier release ran sessions in");
            remove_server_files(state_dir).await;
            panes
        }
        // Verified as the server and signalled, so its subtree is its own.
        Ok(Ok(Termination::StillRunning)) => {
            warn!(socket = %socket.display(), pid,
                    "the private tmux server outlived SIGKILL; leaving its files for a later start");
            panes
        }
        // Not shown to be the server: nothing of its subtree is swept.
        Ok(Err(TerminateError::NotThisHost { .. })) => {
            warn!(socket = %socket.display(), pid,
                    "the process answering on the private tmux socket does not name it on its \
                     command line, so it was not signalled and its processes are not swept; \
                     leaving it and its files alone");
            none()
        }
        Ok(Err(error)) => {
            warn!(socket = %socket.display(), %error,
                    "did not end the private tmux server, and sweep none of its processes; leaving \
                     its files for a later start");
            none()
        }
        Err(_) => {
            warn!(socket = %socket.display(),
                    "ending the private tmux server ran out of time; sweeping none of its \
                     processes and leaving its files for a later start");
            none()
        }
    }
}

impl OldTerminals {
    /// Sweep what the old terminals left: each converted row's agent, and
    /// every old tab whose id can be found (the module docs), within the
    /// step's deadline. `converted` holds `(session, generation, scoped)`
    /// of each old-kind row this reload is converting to interrupted.
    ///
    /// Runs before the conversion commits, so that a crash part way leaves
    /// the rows live for the next start to convert and sweep again.
    pub(crate) async fn sweep(
        &self,
        scopes: &crate::scope::ScopeManager,
        state_dir: &Path,
        converted: &[(String, i64, bool)],
    ) {
        if converted.is_empty() && self.panes.is_empty() {
            return;
        }
        let work = async {
            for (id, generation, scoped) in converted {
                let roots = self
                    .panes
                    .get(&PaneOwner::Agent(id.clone()))
                    .cloned()
                    .unwrap_or_default();
                if let Err(error) = reap_process_tree(
                    scopes,
                    ScopeUnits::recorded(launch_scope_unit(id, *generation, *scoped)),
                    roots,
                    id,
                    &SweepTarget::AgentOnly,
                    ScopeKillFailure::Warn,
                )
                .await
                {
                    warn!(session = %id, error = %format!("{error:#}"),
                        "could not confirm that the agent of an ended old terminal is gone; \
                         whatever is left is the session's Delete's to end");
                }
            }
            let sessions: HashSet<&str> = converted
                .iter()
                .map(|(id, _, _)| id.as_str())
                .chain(self.panes.keys().filter_map(|owner| match owner {
                    PaneOwner::Tab { session, .. } => Some(session.as_str()),
                    PaneOwner::Agent(_) => None,
                }))
                .collect();
            for (session, tab) in self.old_tabs(scopes, state_dir, &sessions).await {
                let owner = PaneOwner::Tab {
                    session: session.clone(),
                    tab: tab.clone(),
                };
                let roots = self.panes.get(&owner).cloned().unwrap_or_default();
                if let Err(error) = reap_process_tree(
                    scopes,
                    ScopeUnits::possible(crate::scope::tab_unit_name(&session, &tab)),
                    roots,
                    &session,
                    &SweepTarget::Tab(tab.clone()),
                    ScopeKillFailure::Warn,
                )
                .await
                {
                    warn!(%session, %tab, error = %format!("{error:#}"),
                        "could not confirm that an ended old tab is gone; whatever is left is the \
                         session's Delete's to end");
                }
            }
        };
        let deadline = Instant::now() + OLD_TERMINALS_DEADLINE.saturating_sub(self.spent);
        if tokio::time::timeout_at(deadline, work).await.is_err() {
            warn!(
                "sweeping what the old terminals left ran out of time; whatever it did not \
                reach is left for each session's Delete"
            );
        }
    }

    /// Every old tab whose id can be found, as `(session, tab)`: the
    /// tab-marked panes of the snapshot, and the tab scope units of
    /// `sessions` that no terminal host's launch record claims.
    async fn old_tabs(
        &self,
        scopes: &crate::scope::ScopeManager,
        state_dir: &Path,
        sessions: &HashSet<&str>,
    ) -> Vec<(String, String)> {
        let mut tabs: Vec<(String, String)> = self
            .panes
            .keys()
            .filter_map(|owner| match owner {
                PaneOwner::Tab { session, tab } => Some((session.clone(), tab.clone())),
                PaneOwner::Agent(_) => None,
            })
            .collect();
        if scopes.available().await {
            for session in sessions {
                let Some(glob) = crate::scope::tab_unit_glob(session) else {
                    continue;
                };
                let units = match scopes.units_matching(&glob).await {
                    Ok(units) => units,
                    Err(error) => {
                        warn!(%session, error = %format!("{error:#}"),
                            "could not list an old session's tab scopes");
                        continue;
                    }
                };
                let prefix = glob.strip_suffix("*.scope").unwrap_or(&glob);
                for unit in units {
                    let Some(tab) = unit
                        .strip_prefix(prefix)
                        .and_then(|rest| rest.strip_suffix(".scope"))
                        .filter(|tab| crate::scope::is_uuid_shaped(tab))
                    else {
                        continue;
                    };
                    let host_record = crate::launch::tab_host_record_path(state_dir, session, tab);
                    if tokio::fs::symlink_metadata(&host_record).await.is_err() {
                        tabs.push((session.to_string(), tab.to_string()));
                    }
                }
            }
        }
        tabs.sort();
        tabs.dedup();
        tabs
    }
}

/// Whether a failed connect proves no server listens: the socket file is
/// missing, or (Linux only) the kernel refused the connection, which it does
/// only for a socket no process listens on (see the module docs for macOS).
fn no_server(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound
        || (cfg!(target_os = "linux") && error.kind() == std::io::ErrorKind::ConnectionRefused)
}

/// Remove the private server's socket, config and the config's staged
/// writes from `state_dir`. Best effort: a file that cannot be removed is
/// logged and tried again at the next start.
async fn remove_server_files(state_dir: &Path) {
    let mut paths: Vec<PathBuf> = vec![state_dir.join(SOCKET), state_dir.join(CONFIG)];
    match tokio::fs::read_dir(state_dir).await {
        Ok(mut entries) => {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(CONFIG_TEMP_PREFIX))
                {
                    paths.push(entry.path());
                }
            }
        }
        Err(error) => warn!(%error, "could not list the state directory for old tmux files"),
    }
    for path in paths {
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {
                info!(path = %path.display(), "removed a file of the old private tmux server")
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => warn!(path = %path.display(), %error,
                "could not remove a file of the old private tmux server"),
        }
    }
}

/// The private server's subtree, grouped by the pane each process descends
/// from: `server`'s direct children are its panes, and each is attributed by
/// its own markers ([`pane_owner`]). A pane whose environment cannot be read
/// or carries no session marker is left out, so nothing found here is ever
/// swept on a guess.
fn snapshot_panes(server: u32) -> Result<HashMap<PaneOwner, Vec<(u32, u64)>>, String> {
    let (table, _) = crate::procs::snapshot()?;
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (&pid, &(ppid, _)) in &table {
        children.entry(ppid).or_default().push(pid);
    }
    let mut panes: HashMap<PaneOwner, Vec<(u32, u64)>> = HashMap::new();
    for &pane in children.get(&server).into_iter().flatten() {
        let Some(owner) = crate::procs::read_environ(pane).and_then(|env| pane_owner(&env)) else {
            continue;
        };
        let mut subtree = vec![pane];
        let mut next = 0;
        while let Some(&pid) = subtree.get(next) {
            subtree.extend(children.get(&pid).into_iter().flatten());
            next += 1;
        }
        panes
            .entry(owner)
            .or_default()
            .extend(subtree.into_iter().map(|pid| (pid, table[&pid].1)));
    }
    Ok(panes)
}

/// Which session's terminal a pane ran, from its environment: a tab when
/// it carries a tab marker of the shape the supervisor mints, the agent
/// when its agent marker names its own session or (the oldest launches) it
/// has none. A pane without a session marker, or one whose agent marker
/// names another session (an agent started inside some other session's
/// terminal), is no one's here.
fn pane_owner(environ: &[u8]) -> Option<PaneOwner> {
    let value = |name: &str| {
        environ.split(|&b| b == 0).find_map(|entry| {
            entry
                .strip_prefix(name.as_bytes())
                .and_then(|rest| rest.strip_prefix(b"="))
                .and_then(|value| std::str::from_utf8(value).ok())
        })
    };
    let session = value(crate::launch::SESSION_ID_ENV_VAR)
        .filter(|id| crate::scope::is_uuid_shaped(id))?
        .to_string();
    if let Some(tab) =
        value(crate::launch::TAB_ID_ENV_VAR).filter(|id| crate::scope::is_uuid_shaped(id))
    {
        return Some(PaneOwner::Tab {
            session,
            tab: tab.to_string(),
        });
    }
    match value(crate::launch::AGENT_ID_ENV_VAR) {
        None | Some("") => Some(PaneOwner::Agent(session)),
        Some(agent) if agent == session => Some(PaneOwner::Agent(session)),
        Some(_) => None,
    }
}

/// The notices and warnings of this start's ending of the old terminals,
/// written straight to the store before any session is published (the
/// reload loads every session's notifications after it): one
/// [`NotificationKind::OldTerminalEnded`] notice for each row in `ended`
/// (`(session, generation)` of the rows the reload converted), and every
/// unresolved old-terminal warning resolved, since nothing runs in an old
/// terminal any more.
///
/// Runs at every start, so that a warning an earlier start could not
/// resolve is resolved by a later one. Never fails startup: a write that
/// fails is logged.
pub(crate) async fn record_ended(store: &SessionStore, ended: &[(String, i64)]) {
    for (id, generation) in ended {
        if let Err(error) = store
            .record_session_notification(
                id,
                *generation,
                NotificationKind::OldTerminalEnded.as_str(),
                OLD_TERMINAL_ENDED_TEXT,
                crate::store::now_unix(),
                false,
            )
            .await
        {
            warn!(session = %id, error = %format!("{error:#}"),
                "could not record that a session's old terminal was ended");
        }
    }
    if let Err(error) = store
        .resolve_notifications_of_kind(NotificationKind::OldTerminal.as_str())
        .await
    {
        warn!(error = %format!("{error:#}"), "could not resolve the old-terminal warnings");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION: &str = "11111111-2222-4333-8444-555555555555";
    const OTHER: &str = "66666666-7777-4888-9999-aaaaaaaaaaaa";
    const TAB: &str = "bbbbbbbb-cccc-4ddd-8eee-ffffffffffff";

    fn environ(entries: &[&str]) -> Vec<u8> {
        entries
            .iter()
            .flat_map(|e| [e.as_bytes(), b"\0"].concat())
            .collect()
    }

    /// A pane is attributed by its own markers, and only to a session the
    /// supervisor could have minted.
    ///
    /// Why: these processes become sweep roots whose whole subtree is
    /// signalled, so a pane attributed to the wrong session or terminal
    /// would end another session's processes (a host tab beside an old
    /// agent carries the same session marker). Spec: a tab marker makes the
    /// pane that tab's; an agent marker naming its own session, or none (the
    /// oldest launches), makes it the agent's; an agent marker naming
    /// another session, a malformed tab or session id (read as no tab, and no
    /// session), or no session marker make it no one's.
    #[test]
    fn a_pane_is_attributed_by_its_markers() {
        let session = format!("FARHELM_SESSION_ID={SESSION}");
        let agent = PaneOwner::Agent(SESSION.into());
        assert_eq!(
            pane_owner(&environ(&[&session, &format!("FARHELM_TAB_ID={TAB}")])),
            Some(PaneOwner::Tab {
                session: SESSION.into(),
                tab: TAB.into()
            })
        );
        assert_eq!(
            pane_owner(&environ(&[
                &session,
                &format!("FARHELM_AGENT_ID={SESSION}")
            ])),
            Some(agent.clone())
        );
        assert_eq!(
            pane_owner(&environ(&["HOME=/x", &session])),
            Some(agent.clone())
        );
        assert_eq!(
            pane_owner(&environ(&[&session, "FARHELM_TAB_ID=not-a-tab"])),
            Some(agent)
        );
        assert_eq!(
            pane_owner(&environ(&[&session, &format!("FARHELM_AGENT_ID={OTHER}")])),
            None
        );
        assert_eq!(pane_owner(&environ(&["FARHELM_SESSION_ID=fh-x"])), None);
        assert_eq!(pane_owner(&environ(&["HOME=/x"])), None);
    }
}
