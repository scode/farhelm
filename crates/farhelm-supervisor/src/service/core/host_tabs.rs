//! Terminal tabs that run in terminal hosts (the plan's PR 18): opening one,
//! the tab strip and lookup, closing one, the automatic reap of one whose
//! shell exited, and finding them again after a supervisor restart.
//!
//! Under the supervisor's switch (`SupervisorSeams::new_terminals` set to
//! `Host`, as `farhelm supervisor run` does by default, or by a test
//! harness) every new tab is a
//! host of its own, whatever its session's agent runs in: a tmux agent's
//! session then holds its old tmux tabs and new host tabs side by side (a
//! mixed session), and a host agent's session holds only host tabs. A tab never changes kind while it lives.
//!
//! ## What a host tab is
//!
//! A host whose child is today's tab chain (the forwarder, then
//! `farhelm internal tab-shell`, which enters the session's directory and
//! execs the user's login shell), inside the tab's own scope when there is a
//! user manager, exactly as a tmux tab window runs it; its environment is
//! a host child's (`host_launch::host_child_env`: the supervisor's
//! environment with Farhelm's terminal variables), with the tab's markers
//! and credential, which reach the child only. The host itself
//! carries none, so no marker sweep ever ends it.
//!
//! Its client tag names the session, the tab's id, whether its child runs in
//! a scope of its own, and its creation time, which orders the session's
//! host tabs (a new tab is stamped past the session's newest, so a clock
//! stepped backwards cannot reorder them). A tab has no stored row: the tag is what a tab host is
//! recognized by, and the session's runtimes for its tab hosts live in its
//! session cells ([`HostTab`], `SessionCells::host_tabs`), where the tab
//! strip, lookup, the reap and Delete read them. Beside the tag, every tab
//! host start writes a launch record (`<state>/launch/<id>.tab-<tab id>.host`,
//! the agent launch's record under a tab's name, `host_launch`'s "The launch
//! record"), before its first stage is given the spec, under the supervisor's
//! `host_scan` fence as an agent start holds it; the record is removed only
//! once its host is confirmed ended. It is not a registry of tabs: it is how
//! Delete finds a tab host no runtime holds, and how a restarted supervisor
//! knows which socket to ask, cross-checked against the host's tag.
//!
//! ## Opening
//!
//! [`Supervisor::open_host_tab`], under the session's lifecycle claim on a
//! supervisor-owned task (`Supervisor::open_tab` decides the gate and the
//! kind): the host starts at the size of the agent's terminal (its tmux
//! window, or its host's emulator), so the first attach repaints nothing;
//! the open then waits up to `TAB_LAUNCH_SETTLE` for the shell to die, as a
//! tmux tab's does, but from the runtime's view of the child rather than a
//! poll of tmux, and a shell dead by then is a refused open whose detail is
//! the emulator's screen once the host's exit record has been processed (its
//! last words); `host_scan` is let go as soon as the shell is seen not
//! alive. Any failure after the host started ends it (its `Shutdown`, else
//! the verified signal, `Supervisor::end_launch_host`), sweeps the tab's
//! scope and marker (with a root only when a runtime pinned the child, so
//! a failure before adoption sweeps without one), and removes its record,
//! so a failed open leaves nothing; what cannot be confirmed cleaned up is
//! in the refusal.
//!
//! ## Closing and reaping
//!
//! [`Supervisor::close_host_tab_claimed`] is the tmux close's counterpart: a
//! tab in a host is never the agent (the cell holds only hosts tagged as
//! tabs, and the agent's own runtime is checked against it), an automatic
//! close goes ahead only while the runtime says the child exited, the
//! scope comes from the tag, the sweep is rooted at the child only while it
//! still runs (`Terminal::live_root`), and then the host is ended and its
//! record removed. A runtime that judged its host gone (from the socket
//! alone) has the process it adopted confirmed gone before the tab is
//! forgotten. A close that cannot confirm the end keeps the tab and its
//! record and marks it ([`HostTab::end_failed`]), and the reap closes it
//! again, since its stopped runtime will announce nothing: a second after
//! the failure, then doubling to at most a minute between tries. The
//! automatic reap is driven by the host's exit event:
//! the runtime announces it on the supervisor's liveness notifier, which
//! wakes the ticker's reap pass, so an exited tab disappears without a poll
//! (`ticker::reap_dead_host_tabs`).
//!
//! ## After a restart
//!
//! [`adopt_tab_hosts`] runs in every reload beside the agent hosts'
//! attribution: for every session the store holds, each of its tab records
//! names a socket, and a live host there whose tag names that session and
//! tab is adopted as recovered. What cannot be used is kept as a placeholder
//! that lists the tab and refuses its verbs (the agent hosts' rules,
//! `host_handover`), and what is gone is dropped, but only once its record
//! is settled by the rules a retried create uses: a start its supervisor
//! did not finish may still bring its host up, during the scan or after it,
//! and that host is adopted rather than left running unseen. The settles
//! run all at once, so a reload pays for the slowest (about twenty seconds
//! for a first stage that never finishes), not for their sum. Tab records of sessions the
//! store no longer holds are left for the startup reclaim of orphaned hosts
//! (`host_reclaim`), which `serve` runs once both reloads are done, and which
//! leaves their hosts running (the records keep those sessions known) and
//! removes a record once its host is gone; so is the record of a tab dropped
//! here as gone, which that reclaim removes, with any socket file its killed
//! host left, once its host is still gone.
//! While the supervisor runs, a placeholder or a runtime that became
//! unavailable is tried again when its reason may pass (`host_handover`,
//! "Adopting again"). The retry looks only at its socket, keeps the tab's
//! id, and preserves a known creation stamp and scope across outcomes. A
//! startup placeholder learns those from a readable tag. A host found gone
//! stays as a finished tab for the ordinary reap, which confirms its adopted
//! process ended and removes the record; a tab whose close already failed
//! belongs to that reap alone and is never re-adopted.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use farhelm_proto::{ErrorKind, TabInfo};
use terminal_host::protocol::Size;
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

use super::host_handover::{GoneHosts, absolute_hosts_dir, unavailability};
use super::host_launch::{
    AtRecordedSocket, HostLeft, LaunchHostRecord, host_child_env, read_launch_record,
    settle_launch_record_with,
};
use super::{
    KeyedGuard, RequestError, SessionEntry, Supervisor, TabOpenStage, TabReapAnchor, error_kind,
    truncate_for_error,
};
use crate::hosts::{
    AdoptError, Adoption, ChildSpec, DEFAULT_JOURNAL_CAP, HostEvidence, HostRegistry, HostRole,
    HostTag, HostTerminal, HostTerminalOptions, LiveHost, Liveness, NamedHost, ScanOptions,
    SpawnError, Unavailability,
};
use crate::service::terminals::{
    TAB_LAUNCH_SETTLE, TAB_LAUNCH_SETTLE_STEP, Terminal, TmuxTerminal,
};
use crate::terminal_process::{TabScope, TerminalProbe};

// ---- The session's host tabs -----------------------------------------------------

/// One of a session's terminal tabs that runs in a terminal host, with what
/// its client tag says about it.
#[derive(Debug, Clone)]
pub(crate) struct HostTab {
    /// The tab's id (`TabInfo::id`), as its tag carries it.
    pub(crate) tab_id: String,
    /// The tag's creation stamp, which orders the session's host tabs.
    /// `u64::MAX` for a startup placeholder whose tag could not be read,
    /// which then lists last. Once learned, kept across re-adoptions even
    /// when a later retry cannot read the host.
    pub(crate) created_ns: u64,
    /// Whether the tab's program runs in the tab's own systemd scope, which
    /// closing it must then stop. Learned from the tag and kept across
    /// re-adoptions; unknown (`false`) for an unread startup placeholder.
    pub(crate) scoped: bool,
    /// The runtime that drives the tab's host, or a placeholder for a host a
    /// restarted supervisor could not use (`HostTerminal::unavailable`),
    /// or a gone-reading handle waiting for the reap to close the tab.
    pub(crate) host: HostTerminal,
    /// Whether a close of this tab could not confirm its host ended. Such a
    /// close has stopped the runtime (`Supervisor::end_launch_host` aborts
    /// it so that it cannot take the host back), whose status then stays
    /// what it last was, possibly "running" for good: nothing would ever
    /// announce the host's end. So the reap closes such a tab again
    /// ([`Self::reap_due`]), which ends the host by the process its runtime
    /// adopted, until a close confirms the end; but no sooner than
    /// [`Self::retry_at`], since an end that keeps failing (a host that
    /// survives SIGKILL, a process table that cannot be read) would
    /// otherwise hold the session's lifecycle claim and a reap permit on
    /// every pass.
    pub(crate) end_failed: bool,
    /// How many closes of this tab could not confirm its host ended.
    pub(crate) failed_ends: u32,
    /// When the reap may close this tab again after a failed end: one
    /// second after the first failure, doubling with each further one, at
    /// most a minute apart ([`retry_delay`]), as re-adoption backs off.
    /// `None` until a close fails.
    pub(crate) retry_at: Option<tokio::time::Instant>,
}

impl HostTab {
    /// The reply's description of this tab: [`Terminal::tab_info`] for its
    /// host, so the tab reports itself as a terminal host's (the client's
    /// migration band and terminal kinds read that) without spelling the kind
    /// out here.
    pub(crate) fn tab_info(&self) -> TabInfo {
        Terminal::Host(self.host.clone()).tab_info(self.tab_id.clone())
    }

    /// Whether the tab is over: its shell exited (its runtime processed the
    /// host's exit record, so the final screen is in), or its host ended
    /// under it (killed, say), which leaves nothing to show. The state in
    /// which the tab strip and lookup hide it, as a dead tmux window is
    /// hidden, and the reap closes it, which for a host that is already gone
    /// only removes what is left (its record, its socket file). A
    /// placeholder never says so: its host may run.
    pub(crate) fn finished(&self) -> bool {
        match self.host.evidence() {
            HostEvidence::Owned { dead, .. } => dead,
            HostEvidence::Absent => true,
            HostEvidence::Unattributed => false,
        }
    }

    /// Whether the reap is to close this tab now: it is
    /// [finished](Self::finished) and no close of it has failed, or an
    /// earlier close could not confirm its host ended
    /// ([`Self::end_failed`]) and its retry time has come.
    pub(crate) fn reap_due(&self) -> bool {
        if self.end_failed {
            return self
                .retry_at
                .is_none_or(|at| at <= tokio::time::Instant::now());
        }
        self.finished()
    }

    /// A startup tab whose host cannot be used: its creation stamp and
    /// scope are not known, so it lists last. Re-adoption can later learn
    /// them from the tag; a failed retry of a known tab keeps its old
    /// metadata instead of reverting to these defaults. Its close refuses
    /// while unavailable, and Delete settles it from its record.
    fn placeholder(
        tab_id: &str,
        socket: &Path,
        reason: Unavailability,
        peer: Option<crate::hosts::HostProcess>,
    ) -> HostTab {
        HostTab {
            tab_id: tab_id.to_string(),
            created_ns: u64::MAX,
            scoped: false,
            host: HostTerminal::unavailable(socket, reason, peer),
            end_failed: false,
            failed_ends: 0,
            retry_at: None,
        }
    }

    /// Carry known tab facts across a retry without reclassifying a
    /// successfully adopted runtime. Only a failed placeholder inherits
    /// the old pinned process; a new runtime owns its newly adopted peer,
    /// even if its task already made it unavailable before installation.
    pub(super) fn after_retry(mut self, old: &HostTab) -> HostTab {
        // Liveness can advance as soon as adoption spawns the runtime.
        // Construction provenance, rather than current liveness alone,
        // decides whether this is an adoption failure.
        if self.host.is_placeholder()
            && let Liveness::Unavailable { reason, peer } = self.host.liveness()
        {
            self.host = old.host.unavailable_again(reason, peer);
        }
        // A startup placeholder learns these facts from its first usable
        // tag. Later outcomes must neither reorder a known tab nor lose
        // the scope that its eventual close must stop.
        if old.created_ns != u64::MAX {
            self.created_ns = old.created_ns;
            self.scoped = old.scoped;
        }
        self
    }

    /// The scope answer `Supervisor::reap_tab_tree` takes, from the tag.
    fn tab_scope(&self) -> TabScope {
        if self.scoped {
            TabScope::Scoped
        } else {
            TabScope::Unscoped
        }
    }
}

/// A session's host tabs (`SessionCells::host_tabs`): a leaf mutex, held
/// across no await.
pub(crate) type HostTabs = Arc<std::sync::Mutex<Vec<HostTab>>>;

/// A [`HostTabs`] cell holding `tabs`.
pub(crate) fn host_tab_cell(tabs: Vec<HostTab>) -> HostTabs {
    Arc::new(std::sync::Mutex::new(tabs))
}

/// A copy of the cell's tabs, in creation order (the tag's stamp, then the
/// tab id for equal stamps, so repeated reads agree).
pub(crate) fn host_tabs_of(cell: &HostTabs) -> Vec<HostTab> {
    let mut tabs = cell.lock().unwrap_or_else(|e| e.into_inner()).clone();
    tabs.sort_by(|a, b| (a.created_ns, a.tab_id.as_str()).cmp(&(b.created_ns, b.tab_id.as_str())));
    tabs
}

/// The host tabs of a session as its tab strip lists them, in creation
/// order, after its tmux tabs: finished ones ([`HostTab::finished`]) left
/// out unless `include_exited` (the teardown's spelling, as for tmux tabs).
/// A placeholder is listed: its host may run, and its tab is the session's
/// until a close or Delete says otherwise.
pub(crate) fn listed_host_tabs(cell: &HostTabs, include_exited: bool) -> Vec<TabInfo> {
    host_tabs_of(cell)
        .into_iter()
        .filter(|tab| include_exited || !tab.finished())
        .map(|tab| tab.tab_info())
        .collect()
}

/// The host tab called `tab_id`, if the session has one.
pub(crate) fn find_host_tab(cell: &HostTabs, tab_id: &str) -> Option<HostTab> {
    cell.lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .cloned()
}

/// Remove the tab whose runtime is `host` from the cell, if it is still
/// there.
fn forget_host_tab(cell: &HostTabs, host: &HostTerminal) {
    cell.lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|tab| tab.host != *host);
}

/// The first wait before the reap closes a tab again after a failed end.
const RETRY_FIRST: std::time::Duration = std::time::Duration::from_secs(1);

/// The longest wait between two such closes.
const RETRY_MAX: std::time::Duration = std::time::Duration::from_secs(60);

/// How long the reap waits after failed end number `failures` (1 for the
/// first) before closing the tab again: 1 s doubling, at most a minute.
fn retry_delay(failures: u32) -> std::time::Duration {
    RETRY_FIRST
        .saturating_mul(1 << failures.saturating_sub(1).min(6))
        .min(RETRY_MAX)
}

/// Mark the tab whose runtime is `host` as one whose close could not
/// confirm its host ended ([`HostTab::end_failed`]), and set when the reap
/// may try again ([`HostTab::retry_at`]).
fn mark_end_failed(cell: &HostTabs, host: &HostTerminal) {
    let now = tokio::time::Instant::now();
    for tab in cell
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter_mut()
        .filter(|tab| tab.host == *host)
    {
        tab.end_failed = true;
        tab.failed_ends = tab.failed_ends.saturating_add(1);
        tab.retry_at = Some(now + retry_delay(tab.failed_ends));
    }
}

/// The ids of the session's host tabs the reap is to close
/// ([`HostTab::reap_due`]).
pub(crate) fn reap_due_host_tabs(cell: &HostTabs) -> Vec<String> {
    host_tabs_of(cell)
        .into_iter()
        .filter(HostTab::reap_due)
        .map(|tab| tab.tab_id)
        .collect()
}

/// The creation stamp for a new tab of the session whose host tabs `cell`
/// holds: now, but never at or below the newest stamp already there, so
/// that a wall clock stepped backwards between two opens cannot list the
/// later tab first (the stamp is the order, and it is kept in the tag for
/// good). Placeholders' unknown stamps (`u64::MAX`) are not counted.
fn next_creation_stamp(cell: &HostTabs) -> u64 {
    let newest = cell
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .map(|tab| tab.created_ns)
        .filter(|stamp| *stamp != u64::MAX)
        .max();
    let now = crate::hosts::tag::creation_stamp();
    match newest {
        Some(newest) => now.max(newest.saturating_add(1)),
        None => now,
    }
}

/// What `Supervisor::open_tab` opens: a window in the agent's tmux session,
/// or a terminal host of the tab's own (module docs).
pub(crate) enum NewTab {
    Window(TmuxTerminal),
    Host,
}

// ---- Opening ---------------------------------------------------------------------

/// The size a new tab starts at when the agent's terminal gives none: the
/// size every terminal Farhelm makes has before a viewer resizes it.
const DEFAULT_TAB_SIZE: Size = Size { cols: 80, rows: 24 };

/// Why [`Supervisor::start_tab_host`] did not start the tab.
enum TabStartFailure {
    /// The start failed; `left` says what it may have left at its socket,
    /// for [`Supervisor::unwind_unstarted_tab`].
    Unwind {
        error: anyhow::Error,
        left: HostLeft,
    },
    /// The start was interrupted on purpose (`HostStartInterrupt`), as a
    /// supervisor crash would interrupt it: the open returns at once with
    /// no cleanup, leaving the record and the first stage.
    #[cfg(any(test, feature = "test-seams"))]
    Interrupted(anyhow::Error),
}

/// A tab host started and adopted, before it is published.
struct StartedTab {
    host: HostTerminal,
    /// `host_scan` held shared since just before the first stage started,
    /// until the tab is published in its session's cell (or given up, when
    /// it must be released first: ending the host takes the lock
    /// exclusively).
    spawning: tokio::sync::OwnedRwLockReadGuard<()>,
}

impl Supervisor {
    /// Open a terminal tab of `entry`'s session in a terminal host of its
    /// own, and publish it in the session's cell (module docs, "Opening").
    ///
    /// Called by `Supervisor::open_tab` on its supervisor-owned task, under
    /// the session's lifecycle claim, once the gate passed and the working
    /// directory was checked: `agent` is the session's agent terminal, which
    /// gives the tab's size, `cwd` the directory the tab enters, and
    /// `container` the session's container for a containerized session.
    pub(super) async fn open_host_tab(
        &self,
        entry: &SessionEntry,
        session_token: &str,
        agent: &Terminal,
        cwd: &str,
        container: Option<&crate::container::ContainerRuntime>,
    ) -> Result<TabInfo, RequestError> {
        let session_id = entry.info.id.as_str();
        let tab_id = uuid::Uuid::new_v4().to_string();
        let (scope_prefix, _unit) = self.tab_scope_prefix(session_id, &tab_id).await;
        let scoped = !scope_prefix.is_empty();
        let env = self.tab_environment(session_id, session_token, &tab_id);
        let (argv, shell) = self.tab_command(cwd, container, scope_prefix).await?;
        let size = self
            .agent_terminal_size(session_id, agent)
            .await
            .unwrap_or(DEFAULT_TAB_SIZE);
        let record = crate::launch::tab_host_record_path(&self.state_dir, session_id, &tab_id);
        let tag = HostTag {
            session_id: session_id.to_string(),
            created_ns: next_creation_stamp(&entry.session.host_tabs),
            role: HostRole::Tab {
                tab_id: tab_id.clone(),
                scoped,
            },
        };
        let scope = if scoped {
            TabScope::Scoped
        } else {
            TabScope::Unscoped
        };
        let StartedTab { host, spawning } = match self
            .start_tab_host(&record, argv, &env, cwd, size, &tag, &shell)
            .await
        {
            Ok(started) => started,
            Err(TabStartFailure::Unwind { error, left }) => {
                return Err(self
                    .unwind_unstarted_tab(session_id, &tab_id, scope, &record, left, error)
                    .await);
            }
            #[cfg(any(test, feature = "test-seams"))]
            Err(TabStartFailure::Interrupted(error)) => {
                return Err(RequestError::new(ErrorKind::Internal, format!("{error:#}")));
            }
        };
        // Held until the tab is published, or given up as soon as its shell
        // is found not alive: the refusal below waits up to two more seconds
        // for the host's exit record, which no other session's host spawn or
        // socket removal should wait behind.
        let mut spawning = Some(spawning);
        let tab = HostTab {
            tab_id: tab_id.clone(),
            created_ns: tag.created_ns,
            scoped,
            host,
            end_failed: false,
            failed_ends: 0,
            retry_at: None,
        };

        // The dead-at-reply refusal (`ControlMsg::OpenTab`'s contract), as
        // for a tmux tab: see `open_tab_window` for why the open waits at
        // all, and why the gate exists. The wait reads the runtime's pinned
        // child (`Self::settled_host_tab`), which says dead as soon as the
        // child stops running, before the host has even reported it.
        if let Some(gate) = self.seams.faults.tab_settle_gate() {
            gate().await;
        }
        let settled = self.settled_host_tab(&tab.host).await;
        if !matches!(settled, Ok(true)) {
            drop(spawning.take());
        }
        let refusal = match settled {
            Ok(true) => None,
            Ok(false) => {
                // The screen is final only once the host's exit record, sent
                // after the child's drained output, has been processed.
                let detail = if Terminal::Host(tab.host.clone())
                    .holds_final_state(super::super::handlers::HOST_EXIT_RECORD_WAIT)
                    .await
                {
                    let words = tab.host.screen_tail(MAX_LAST_WORDS);
                    if words.trim().is_empty() {
                        " and printed nothing".to_string()
                    } else {
                        format!(": {}", words.trim())
                    }
                } else {
                    ", and its output could not be read (the terminal host did not report its \
                     exit in time)"
                        .to_string()
                };
                Some(format!(
                    "the terminal tab's shell ({shell}) was already dead when the tab \
                     opened{detail}"
                ))
            }
            Err(error) => Some(format!(
                "could not confirm the new terminal tab's shell is running: {error:#}"
            )),
        };
        // The seam stands in for any failure after the shell started and
        // before the tab is published, which no other input reaches.
        let refusal = refusal.or_else(|| match self.seams.faults.tab_open_fault() {
            Some(fault) => fault(TabOpenStage::BeforePublishing)
                .err()
                .map(|error| format!("{error:#}")),
            None => None,
        });
        if let Some(because) = refusal {
            drop(spawning.take());
            return Err(self
                .discard_failed_host_tab(session_id, &tab, &record, because)
                .await);
        }

        // Published under the session map, as a create publishes its host:
        // an orderly stop gathers every host runtime there under the same
        // lock, and one published after it began would be closed by
        // nothing, so it is stopped and a placeholder takes its place,
        // whose record the next supervisor adopts the host by.
        {
            let _sessions = self.sessions.lock().await;
            let published = match self.publishable(Some(Terminal::Host(tab.host.clone()))) {
                Some(Terminal::Host(host)) => HostTab {
                    host,
                    ..tab.clone()
                },
                Some(Terminal::Tmux(_)) | None => tab.clone(),
            };
            entry
                .session
                .host_tabs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(published);
        }
        drop(spawning);
        info!(
            session = %session_id, tab = %tab_id, socket = %tab.host.socket().display(),
            scoped, cols = size.cols, rows = size.rows,
            "terminal tab opened in a terminal host"
        );
        Ok(tab.tab_info())
    }

    /// The size of the agent's terminal, for a new tab to start at: its
    /// tmux window's, or its host's emulator's. `None` when it cannot be
    /// read, which costs only a resize at the tab's first attach.
    async fn agent_terminal_size(&self, session_id: &str, agent: &Terminal) -> Option<Size> {
        match agent {
            Terminal::Tmux(tmux) => {
                match self.tmux.window_size(&tmux.tmux_name, &tmux.pane).await {
                    Ok(size) => size.map(|(cols, rows)| Size { cols, rows }),
                    Err(error) => {
                        debug!(session = %session_id, error = %format!("{error:#}"),
                        "could not read the agent window's size; the first attach will resize \
                         the tab");
                        None
                    }
                }
            }
            Terminal::Host(host) => match host.liveness() {
                Liveness::Running { .. } | Liveness::Exited { .. } => Some(host.size()),
                Liveness::Gone | Liveness::Unavailable { .. } => None,
            },
        }
    }

    /// Start a tab host running `argv` in `cwd` at `size`, tagged `tag`, and
    /// adopt it as freshly spawned, reporting to the liveness notifier: the
    /// tab counterpart of `Supervisor::spawn_agent_in_host`'s start, in the
    /// same order and for the same reasons (that function's docs and
    /// `host_launch`'s "The launch record"). The socket name is drawn first;
    /// `host_scan` is taken shared just before the first stage starts; the
    /// launch record at `record` names the socket and the first stage,
    /// pinned to its start identity, before that first stage is given its
    /// spec, and a start whose first stage cannot be pinned or whose record
    /// cannot be written is ended there, with nothing started.
    ///
    /// The child's environment is an agent host child's
    /// (`host_child_env`), with `env` (the tab's markers, credential and the
    /// launch environment) as its launch environment. `shell` is the login
    /// shell the helper runs, for `SHELL`.
    ///
    /// On failure the guard is released and what the start may have left is
    /// returned, for [`Self::unwind_unstarted_tab`]. The open's fault seam
    /// can fail it after the host reported ready and before its runtime
    /// adopts it (`TabOpenStage::BeforeAdoption`), a failure whose host may
    /// remain; the create's interrupt seam (`HostStartInterrupt`) can stop
    /// it as a crash would, with the record written and the first stage
    /// given its spec.
    #[allow(clippy::too_many_arguments)]
    async fn start_tab_host(
        &self,
        record: &Path,
        argv: Vec<String>,
        env: &[(String, String)],
        cwd: &str,
        size: Size,
        tag: &HostTag,
        shell: &str,
    ) -> Result<StartedTab, TabStartFailure> {
        let shell_variable = self.host_shell_variable(shell).await;
        // Only host terminals receive the managed value. Shared tab env also
        // feeds tmux -e, whose arguments must never contain this credential.
        let mut child_env = host_child_env(std::env::vars_os(), env, &shell_variable, cwd);
        crate::github_token::inject(
            &mut child_env,
            &crate::github_token::host_file(&self.state_dir),
        )
        .map_err(|error| TabStartFailure::Unwind {
            error,
            left: HostLeft::Nothing,
        })?;
        let child = ChildSpec {
            argv: argv.into_iter().map(OsString::from).collect(),
            cwd: PathBuf::from(cwd),
            env: child_env,
            size: Size {
                cols: size.cols.clamp(1, 10_000),
                rows: size.rows.clamp(1, 10_000),
            },
            journal_cap: DEFAULT_JOURNAL_CAP,
            initial_snapshot: None,
        };
        let nothing = |error: anyhow::Error| TabStartFailure::Unwind {
            error,
            left: HostLeft::Nothing,
        };
        let name = crate::hosts::free_host_name(&self.state_dir).map_err(|e| nothing(e.into()))?;
        let spawning = self.host_scan.clone().read_owned().await;
        let starting =
            crate::hosts::start_host(&self.host_program(), &self.state_dir, name, child, tag)
                .map_err(|e| nothing(e.into()))?;
        let Some(starter) = starting.starter() else {
            return Err(nothing(anyhow::anyhow!(
                "its first stage's process could not be read"
            )));
        };
        let recorded = super::host_launch::LaunchHostRecord {
            socket: starting.name().clone(),
            starter,
        };
        crate::write_private_file(record, recorded.encode().as_bytes())
            .await
            .map_err(|e| {
                nothing(anyhow::Error::new(e).context("recording the tab's terminal host"))
            })?;
        #[cfg(any(test, feature = "test-seams"))]
        {
            if let Some(interrupt) = self.seams.faults.host_start_interrupt()
                && let Err(error) = interrupt()
            {
                if let Err(delivery) = starting.deliver_and_abandon_for_test().await {
                    warn!(session = %tag.session_id, error = %delivery,
                        "the interrupted tab start's spec was not delivered");
                }
                return Err(TabStartFailure::Interrupted(
                    error.context(super::SimulatedCrash),
                ));
            }
        }
        let spawned = match starting.finish().await {
            Ok(spawned) => spawned,
            Err(error) => {
                let left = match &error {
                    SpawnError::MayRemain { socket, .. } => HostLeft::MayRemain {
                        socket: socket.clone(),
                    },
                    SpawnError::StateDir(_)
                    | SpawnError::HostsDir(_)
                    | SpawnError::Tag(_)
                    | SpawnError::Name(_)
                    | SpawnError::Start { .. }
                    | SpawnError::Refused { .. }
                    | SpawnError::Abandoned { .. } => HostLeft::Nothing,
                };
                return Err(TabStartFailure::Unwind {
                    error: anyhow::Error::new(error),
                    left,
                });
            }
        };
        if let Some(fault) = self.seams.faults.tab_open_fault()
            && let Err(error) = fault(TabOpenStage::BeforeAdoption)
        {
            return Err(TabStartFailure::Unwind {
                error,
                left: HostLeft::MayRemain {
                    socket: spawned.socket,
                },
            });
        }
        let mut options = HostTerminalOptions {
            notifier: Some(self.host_liveness.clone()),
            ..Default::default()
        };
        if let Some(adjust) = self.seams.faults.host_terminal_options() {
            adjust(&mut options);
        }
        match HostTerminal::adopt(&spawned.socket, Adoption::Fresh, options).await {
            Ok(host) => Ok(StartedTab { host, spawning }),
            Err(error) => {
                let left = match &error {
                    AdoptError::Gone(_) => HostLeft::Nothing,
                    AdoptError::Unavailable { .. } => HostLeft::MayRemain {
                        socket: spawned.socket.clone(),
                    },
                };
                Err(TabStartFailure::Unwind {
                    error: anyhow::Error::new(error)
                        .context("connecting to the tab's new terminal host"),
                    left,
                })
            }
        }
    }

    /// Watch a new tab host's child until it is either seen dead or has
    /// survived [`TAB_LAUNCH_SETTLE`]: `Ok(true)` alive, `Ok(false)` dead.
    ///
    /// Reads the runtime's probe, which says dead as soon as the pinned
    /// child is no longer running (before the host's exit record arrives),
    /// so the settle ends on the death itself. A host that ended or cannot
    /// be read is an error, which refuses the open like a failed tmux query.
    async fn settled_host_tab(&self, host: &HostTerminal) -> anyhow::Result<bool> {
        let deadline = tokio::time::Instant::now() + TAB_LAUNCH_SETTLE;
        loop {
            match host.probe()? {
                TerminalProbe::Owned(process) if process.dead => return Ok(false),
                TerminalProbe::Owned(_) => {}
                TerminalProbe::Gone => anyhow::bail!("the tab's terminal host ended"),
                TerminalProbe::ForeignOwner { owner } => {
                    anyhow::bail!(Terminal::Host(host.clone()).unknown_owner_refusal(&owner))
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(true);
            }
            tokio::time::sleep(TAB_LAUNCH_SETTLE_STEP).await;
        }
    }

    /// Undo a tab host start that failed before its runtime existed, and
    /// turn `error` into the open's refusal: end whatever the start may have
    /// left at its socket (only a host tagged as this session's is shut
    /// down, `Supervisor::end_launch_host`), then reap what the tab's shell
    /// may have started, in the tab's scope (`scope`, the selection the
    /// open made) and by the tab's marker, whatever the start left: even a
    /// start that left no host may have run its child for a moment
    /// (`HostLeft::Nothing`), and ending a host only hangs its child up,
    /// which a shell's background job can ignore. The sweep has no root:
    /// with no runtime there is no child pinned to its start identity, and a
    /// root is never taken from an unverified pid. The launch record goes
    /// once nothing can run at its socket; a host that cannot be confirmed
    /// ended keeps its record, which Delete finds it by. Whatever could not
    /// be confirmed cleaned up is in the refusal.
    async fn unwind_unstarted_tab(
        &self,
        session_id: &str,
        tab_id: &str,
        scope: TabScope,
        record: &Path,
        left: HostLeft,
        error: anyhow::Error,
    ) -> RequestError {
        let ended = match left {
            HostLeft::Nothing => Ok(()),
            HostLeft::MayRemain { socket } => self.end_launch_host(None, &socket, session_id).await,
        };
        let swept = self
            .reap_tab_processes(session_id, tab_id, scope, None)
            .await;
        let mut left_behind: Vec<String> = Vec::new();
        match ended {
            Ok(()) => remove_tab_record(record).await,
            Err(why) => left_behind.push(format!(
                "its terminal host, which keeps its record so that deleting the session ends it \
                 ({why})"
            )),
        }
        if let Err(why) = swept {
            left_behind.push(format!("its processes ({why:#})"));
        }
        if left_behind.is_empty() {
            return RequestError::new(
                error_kind(&error),
                format!(
                    "could not open a terminal tab for session {}: {error:#}",
                    truncate_for_error(session_id)
                ),
            );
        }
        warn!(session = %session_id, tab = %tab_id,
            "a failed terminal-tab open could not be fully unwound");
        RequestError::new(
            ErrorKind::Internal,
            format!(
                "could not open a terminal tab for session {}: {error:#}, and it could not be \
                 cleaned up: {} of terminal tab {} may still exist",
                truncate_for_error(session_id),
                left_behind.join(" and "),
                truncate_for_error(tab_id)
            ),
        )
    }

    /// Tear down a tab host an open decided not to keep, and turn `because`
    /// into the error that open reports: the tmux discard's counterpart
    /// (`Supervisor::discard_failed_tab_window`), with the same aggregation
    /// of what could not be cleaned up. The tab's process tree is reaped
    /// first (rooted at the child while it runs), then its host is ended,
    /// its record is removed once that end is confirmed, and what outlived
    /// the hangup is swept by the tab's marker and scope. The caller must
    /// not hold `host_scan`.
    async fn discard_failed_host_tab(
        &self,
        session_id: &str,
        tab: &HostTab,
        record: &Path,
        because: String,
    ) -> RequestError {
        info!(session = %session_id, tab = %tab.tab_id, socket = %tab.host.socket().display(),
            reason = "failed_open", "discarding a terminal tab that did not finish opening");
        let mut left_behind: Vec<String> = Vec::new();
        if let Err(error) = self
            .reap_tab_tree(
                session_id,
                &Terminal::Host(tab.host.clone()),
                &tab.tab_id,
                tab.tab_scope(),
                TabReapAnchor::PaneIfLive,
            )
            .await
        {
            left_behind.push(format!("its processes ({error:#})"));
        }
        match self
            .end_launch_host(Some(&tab.host), tab.host.socket(), session_id)
            .await
        {
            Ok(()) => remove_tab_record(record).await,
            Err(why) => left_behind.push(format!("its terminal host ({why})")),
        }
        // What outlived the host's hangup, by the tab's marker and scope, as
        // a close's second pass does.
        if let Err(error) = self
            .reap_tab_processes(session_id, &tab.tab_id, tab.tab_scope(), None)
            .await
        {
            left_behind.push(format!("its processes ({error:#})"));
        }
        if left_behind.is_empty() {
            return RequestError::new(
                ErrorKind::Internal,
                format!("{because}, so the tab was removed again"),
            );
        }
        warn!(session = %session_id, tab = %tab.tab_id,
            "a failed terminal-tab open could not be fully unwound");
        RequestError::new(
            ErrorKind::Internal,
            format!(
                "{because}, and it could not be cleaned up: {} of terminal tab {} may still \
                 exist (terminal host {})",
                left_behind.join(" and "),
                truncate_for_error(&tab.tab_id),
                tab.host.socket().display()
            ),
        )
    }
}

/// The longest refusal detail quoted from a dead tab's screen, as for a
/// tmux tab's last words: far below a replay's cap, since it ends up inside
/// a protocol error message.
const MAX_LAST_WORDS: usize = 4 * 1024;

/// Remove a tab host's launch record, once its host is confirmed ended (or
/// never started). Best effort: a record left behind names a socket nothing
/// serves, which Delete settles as gone, so a failure is only logged.
async fn remove_tab_record(record: &Path) {
    match tokio::fs::remove_file(record).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => warn!(record = %record.display(), %error,
            "could not remove an ended terminal tab host's launch record"),
    }
}

// ---- Closing ---------------------------------------------------------------------

impl Supervisor {
    /// Close the host tab `tab_id` of `entry`'s session, whose runtime
    /// `resolve_terminal_for_close` found as `host`, under the lifecycle
    /// claim `lifecycle`, which moves onto the supervisor-owned task the
    /// close runs on (`Supervisor::close_tab_claimed`, whose `only_if_dead`
    /// marks the ticker's reap).
    ///
    /// The tmux close's steps, each from the host's side (module docs,
    /// "Closing and reaping"):
    ///
    /// - **Never the agent.** The cell holds only hosts whose tag names a
    ///   tab of this session (the open wrote it; a reload checked it), and
    ///   one that is the agent's own runtime as well is refused.
    /// - **A host that cannot be read** is refused: whether its shell runs
    ///   cannot be told (the plan's D-16), and only Delete ends such a host.
    /// - **Only if dead**, for the reap: the runtime must have processed the
    ///   host's exit record (or seen its host end), checked again here under
    ///   the claim, unless an earlier close could not confirm the host ended.
    /// - **The reap** roots at the child only while it runs, with the
    ///   tab's scope as its tag records it, then the host is ended (its
    ///   `Shutdown`, else the verified signal to the process its runtime
    ///   adopted), then a marker-only sweep catches what outlived the
    ///   hangup, as the tmux close's second pass after `kill-window` does.
    /// - **The record** goes once the host is confirmed ended, and so does
    ///   the tab from the session's cell; a host judged gone from its socket
    ///   alone is confirmed by its adopted process first. A host not
    ///   confirmed ended keeps both, and the tab is marked
    ///   ([`HostTab::end_failed`]) so that the reap closes it again.
    /// - **The attachment** is detached unconditionally, as for a tmux tab,
    ///   but before the host is ended rather than after (see the body).
    pub(super) async fn close_host_tab_claimed(
        self: &Arc<Self>,
        lifecycle: KeyedGuard,
        entry: &Arc<SessionEntry>,
        tab_id: &str,
        host: HostTerminal,
        only_if_dead: bool,
    ) -> Result<(), RequestError> {
        let not_found = |why: &str| RequestError::new(ErrorKind::NotFound, why.to_string());
        let Some(tab) =
            find_host_tab(&entry.session.host_tabs, tab_id).filter(|tab| tab.host == host)
        else {
            return Err(not_found("the terminal tab was closed meanwhile"));
        };
        match &entry.terminal {
            Some(Terminal::Host(agent)) if *agent == tab.host => {
                return Err(not_found(&format!(
                    "terminal tab {} is the session's agent terminal",
                    truncate_for_error(tab_id)
                )));
            }
            Some(Terminal::Host(_) | Terminal::Tmux(_)) | None => {}
        }
        // A runtime stopped by an earlier close that could not confirm the
        // end can read as anything; that close is what is being finished.
        if let Liveness::Unavailable { reason, .. } = tab.host.liveness()
            && !tab.end_failed
        {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                format!(
                    "terminal tab {}'s terminal host cannot be read ({reason}), so it is left as \
                     it is; deleting the session ends it",
                    truncate_for_error(tab_id)
                ),
            ));
        }
        let sup = Arc::clone(self);
        let entry = Arc::clone(entry);
        let tab_id = tab_id.to_string();
        let task = tokio::spawn(async move {
            let _lifecycle = lifecycle;
            let closed = sup.close_host_tab(&entry, &tab, only_if_dead).await;
            if closed.is_ok() {
                sup.hint_sessions_changed();
            }
            closed
        });
        match task.await {
            Ok(result) => result,
            Err(join) => Err(RequestError::new(
                ErrorKind::Internal,
                format!(
                    "the terminal-tab close task failed for {}: {join}",
                    truncate_for_error(&tab_id)
                ),
            )),
        }
    }

    /// [`Self::close_host_tab_claimed`]'s side-effecting half, on the
    /// supervisor-owned task.
    async fn close_host_tab(
        &self,
        entry: &SessionEntry,
        tab: &HostTab,
        only_if_dead: bool,
    ) -> Result<(), RequestError> {
        let session_id = entry.info.id.as_str();
        let tab_id = tab.tab_id.as_str();
        if let Some(gate) = self.seams.faults.tab_close_gate() {
            gate().await;
        }
        if only_if_dead {
            if !tab.reap_due() {
                return Err(RequestError::new(
                    ErrorKind::NotFound,
                    "the terminal tab is no longer confirmed dead; automatic cleanup was skipped",
                ));
            }
            info!(session = %session_id, tab = %tab_id, socket = %tab.host.socket().display(),
                exit = ?tab.host.exit_code(),
                "automatically closing a terminal tab whose shell exited");
        }
        info!(session = %session_id, tab = %tab_id, socket = %tab.host.socket().display(),
            reason = if only_if_dead { "shell_exited" } else { "user_close" },
            "closing terminal tab");
        let terminal = Terminal::Host(tab.host.clone());
        // A close finishing an earlier one that could not confirm the end
        // takes no root: that close reaped the tree from the live child
        // already, and the runtime it stopped says nothing current about it.
        let anchor = if tab.end_failed {
            TabReapAnchor::MarkerOnly
        } else {
            TabReapAnchor::PaneIfLive
        };
        self.reap_tab_tree(session_id, &terminal, tab_id, tab.tab_scope(), anchor)
            .await
            .map_err(|e| {
                RequestError::new(
                    ErrorKind::Internal,
                    format!("closing terminal tab {}: {e:#}", truncate_for_error(tab_id)),
                )
            })?;
        // The attachment goes before the host, unlike a tmux tab's after its
        // window: a grid stream watches its host, and would end on its own
        // with "terminal ended" the moment the host does, racing the
        // truthful "tab closed". The tab is being closed whatever the end
        // reports, so its viewer is told so first.
        let detach = self.detach_closed_tab(session_id, tab_id).await;
        let ended = self.end_tab_host(session_id, tab).await;
        // The host's end hangs its pty up; this pass reaches what outlived
        // the hangup by the tab's marker and scope alone, never by a root:
        // the child is gone, or its pid may be anyone's by now.
        let survivors = self
            .reap_tab_tree(
                session_id,
                &terminal,
                tab_id,
                tab.tab_scope(),
                TabReapAnchor::MarkerOnly,
            )
            .await;
        match ended {
            Ok(()) => {
                remove_tab_record(&crate::launch::tab_host_record_path(
                    &self.state_dir,
                    session_id,
                    tab_id,
                ))
                .await;
                forget_host_tab(&entry.session.host_tabs, &tab.host);
                super::tab_close_cleanup_result(tab_id, survivors, detach)?;
                info!(session = %session_id, tab = %tab_id, "terminal tab closed");
                Ok(())
            }
            Err(why) => {
                // Its runtime is stopped by now and will announce nothing,
                // so the tab is marked for the reap to close again.
                mark_end_failed(&entry.session.host_tabs, &tab.host);
                Err(RequestError::new(
                    ErrorKind::Internal,
                    format!(
                        "terminal tab {}'s processes were reaped but its terminal host could not \
                         be confirmed ended: {why}",
                        truncate_for_error(tab_id)
                    ),
                ))
            }
        }
    }

    /// End a tab's host for its close and confirm it ended.
    ///
    /// `Supervisor::end_launch_host`, except for a runtime that has already
    /// judged its host gone: a runtime judges that from its socket (missing,
    /// or refusing for the refusal window), and the socket file can go while
    /// the host process still runs. A tab would then be forgotten with its
    /// record removed and nothing left that finds its host, so for such a
    /// runtime the process it adopted is ended (or seen gone) by the
    /// verified signal first, which checks that the process is still that
    /// host before it signals anything.
    async fn end_tab_host(&self, session_id: &str, tab: &HostTab) -> Result<(), String> {
        if let Some(fault) = self.seams.faults.tab_host_end_fault() {
            fault()?;
        }
        if matches!(tab.host.liveness(), Liveness::Gone)
            && let Some(adopted) = tab.host.adopted_process()
        {
            return self.end_adopted_process(adopted, tab.host.socket()).await;
        }
        self.end_launch_host(Some(&tab.host), tab.host.socket(), session_id)
            .await
    }
}

// ---- Adopting again --------------------------------------------------------------

impl Supervisor {
    /// Retry one unavailable tab under its session's lifecycle claim, taken
    /// without waiting. The tab must still hold `old` in the same cell when
    /// the result is installed; an agent Restart may change the entry and
    /// generation while keeping that cell, because the tab belongs to the
    /// session. A successful runtime adoption ends this retry episode.
    pub(super) async fn readopt_tab_one(&self, id: &str, tab_id: &str, old: &HostTerminal) -> bool {
        let Some(_claim) = self.lifecycle_locks.try_claim(id) else {
            debug!(session = %id, tab = %tab_id,
                "a session's lifecycle is busy; retrying its tab host later");
            return false;
        };
        let current = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(id)
                .filter(|_| !self.hosts_stopping())
                .and_then(|entry| {
                    find_host_tab(&entry.session.host_tabs, tab_id)
                        .filter(|tab| tab.host == *old && !tab.end_failed)
                        .map(|tab| (Arc::clone(&entry.session.host_tabs), tab))
                })
        };
        let Some((cell, tab)) = current else {
            return false;
        };
        let replacement = self.probe_readopted_tab(id, &tab).await;
        let recovered = replacement.host != *old && !replacement.host.is_placeholder();
        self.install_readopted_tab(id, &cell, old, replacement)
            .await
            && recovered
    }

    /// What is at this tab's one socket now, without touching any other
    /// host. A readable matching tag is adopted as recovered; a failure
    /// leaves a placeholder with its new reason. A gone host leaves a
    /// finished handle for the reap rather than discarding the tab and
    /// leaking its record or any process its old runtime adopted.
    ///
    /// The caller holds the session's lifecycle claim throughout the probe
    /// and install. That socket was drawn for this tab's start, so a scan
    /// restricted to it cannot take over another runtime here or race a
    /// new start at the same socket.
    pub(super) async fn probe_readopted_tab(&self, id: &str, old: &HostTab) -> HostTab {
        let socket = old.host.socket();
        let Some(name) = socket.file_name().map(OsString::from) else {
            return old.clone();
        };
        let options = Self::host_reload_options(&self.seams, &self.host_liveness);
        let scan = ScanOptions {
            deadlines: options.deadlines,
            skip: HashSet::new(),
            only: Some(HashSet::from([name.clone()])),
        };
        let placeholder = |reason, peer| HostTab::placeholder(&old.tab_id, socket, reason, peer);
        let gone = || HostTab {
            host: HostTerminal::gone(socket, old.host.adopted_process()),
            ..old.clone()
        };
        let replacement = match HostRegistry::scan(&self.state_dir, &scan).await {
            Err(error) => placeholder(Unavailability::NotScanned(error.to_string()), None),
            Ok(mut registry) => {
                if registry.gone().iter().any(|host| host.name == name) {
                    self.gone_hosts
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(name.clone());
                }
                match registry.by_name(&name) {
                    Some(NamedHost::Live(host)) if names_tab(&host.tag, id, &old.tab_id) => {
                        let host = registry
                            .take(&name)
                            .expect("the matching tab host was scanned");
                        adopt_tab(id, &old.tab_id, socket, host, options)
                            .await
                            .unwrap_or_else(gone)
                    }
                    Some(NamedHost::Live(host)) => placeholder(
                        Unavailability::Unidentified(format!(
                            "the host at its socket is tagged for another terminal ({:?})",
                            host.tag
                        )),
                        host.peer(),
                    ),
                    Some(NamedHost::Unavailable(host)) => {
                        placeholder(unavailability(&host.why), host.peer)
                    }
                    Some(NamedHost::Gone(_)) => gone(),
                    None => match tokio::fs::symlink_metadata(socket).await {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => gone(),
                        Ok(_) | Err(_) => placeholder(
                            Unavailability::NotScanned("the scan did not reach its socket".into()),
                            None,
                        ),
                    },
                }
            }
        };
        replacement.after_retry(old)
    }

    /// Install a tab retry only while its exact cell, tab id and old handle
    /// remain current and no orderly stop has begun. A close that already
    /// failed owns the tab through the reap instead. The session map is
    /// held for this check and swap, matching the orderly stop's fence;
    /// host_adoption also prevents a reload from replacing this map.
    /// A runtime made for a result that loses the fence is aborted, leaving
    /// its host running for whoever now owns the tab.
    pub(super) async fn install_readopted_tab(
        &self,
        id: &str,
        cell: &HostTabs,
        old: &HostTerminal,
        replacement: HostTab,
    ) -> bool {
        let installed = {
            let _adoption = self.host_adoption.lock().await;
            let sessions = self.sessions.lock().await;
            let current = sessions.get(id).filter(|entry| {
                !self.hosts_stopping() && Arc::ptr_eq(&entry.session.host_tabs, cell)
            });
            current.is_some_and(|entry| {
                let mut tabs = entry
                    .session
                    .host_tabs
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let Some(tab) = tabs.iter_mut().find(|tab| {
                    tab.tab_id == replacement.tab_id && tab.host == *old && !tab.end_failed
                }) else {
                    return false;
                };
                *tab = replacement.clone();
                true
            })
        };
        if installed {
            self.hint_sessions_changed();
        } else {
            replacement.host.abort();
        }
        installed
    }
}

// ---- After a restart -------------------------------------------------------------

/// What [`adopt_tab_hosts`] works with: `HostDiscovery`'s settings, and the
/// tab runtimes this supervisor already holds.
pub(super) struct TabDiscovery {
    /// Whether this supervisor holds the state directory's lock; without it
    /// nothing is scanned or settled, as for agent hosts.
    pub(super) owns_state_dir: bool,
    /// What each adopted runtime is made with.
    pub(super) options: HostTerminalOptions,
    /// The host tabs this supervisor already holds, by session: `serve`'s
    /// second reload keeps the constructor's, and never contacts their
    /// sockets.
    pub(super) held: HashMap<String, Vec<HostTab>>,
    /// The `host_scan` lock, taken exclusively around the scan, the settling
    /// and the adoptions; `None` before the supervisor exists.
    pub(super) no_spawns: Option<Arc<tokio::sync::RwLock<()>>>,
    /// Sockets found gone earlier, never scanned again.
    pub(super) gone: GoneHosts,
    /// The supervisor's test hooks, for settling an interrupted start's
    /// record (`host_launch::settle_launch_record_with`).
    pub(super) faults: super::FaultHooks,
}

/// One tab record of a session the store holds.
struct TabRecord {
    session_id: String,
    tab_id: String,
    /// What the record says, or why it could not be read.
    read: Result<LaunchHostRecord, String>,
    /// The record's own path, which names the tab when its socket is not
    /// known.
    path: PathBuf,
}

/// A tab record whose socket is known, waiting for the scan's answer.
struct Pending {
    session_id: String,
    tab_id: String,
    record: LaunchHostRecord,
    name: OsString,
    socket: PathBuf,
}

/// The host tabs of every session in `sessions` (session ids the store
/// holds), by session: the ones this supervisor already holds, kept, and
/// for every other tab record of those sessions, what its socket holds
/// (module docs, "After a restart"):
///
/// - a live host whose tag names that session and tab is adopted as
///   recovered;
/// - nothing (the scan found the socket gone, did not reach it, or its file
///   is missing) is a finished tab only once the record is settled, by the
///   rules a retried create uses (`host_launch::settle_launch_record_with`:
///   the first stage waited for and then fenced, the socket looked at again
///   while a forked second stage may still bind it): a start interrupted by
///   its supervisor's end leaves a first stage that has its spec and may
///   still bind the socket and start the shell, possibly during the scan
///   itself, and a host that comes up there is adopted like any other;
/// - anything that may run and cannot be used, a tag naming anything else,
///   a start that cannot be settled, or a record that cannot be read, is a
///   placeholder, which lists the tab and refuses its close (its session's
///   Delete settles it from the record).
///
/// Never fails. Every record whose socket the scan found nothing at is
/// settled, all at once ([`settle_tab`] says what that costs).
pub(super) async fn adopt_tab_hosts(
    state_dir: &Path,
    discovery: TabDiscovery,
    sessions: &HashSet<String>,
) -> HashMap<String, Vec<HostTab>> {
    let TabDiscovery {
        owns_state_dir,
        options,
        held,
        no_spawns,
        gone,
        faults,
    } = discovery;
    let mut found: HashMap<String, Vec<HostTab>> = HashMap::new();
    let mut held_names: HashSet<OsString> = HashSet::new();
    let mut held_ids: HashSet<(String, String)> = HashSet::new();
    for (session_id, tabs) in held {
        for tab in tabs {
            if !sessions.contains(&session_id) {
                tab.host.abort();
                continue;
            }
            if let Some(name) = tab.host.socket().file_name() {
                held_names.insert(name.to_os_string());
            }
            held_ids.insert((session_id.clone(), tab.tab_id.clone()));
            found.entry(session_id.clone()).or_default().push(tab);
        }
    }
    let hosts_dir = absolute_hosts_dir(state_dir);
    let mut pending = Vec::new();
    for TabRecord {
        session_id,
        tab_id,
        read,
        path,
    } in tab_records(state_dir, sessions).await
    {
        if held_ids.contains(&(session_id.clone(), tab_id.clone())) {
            continue;
        }
        let tab = match read {
            Err(why) => {
                warn!(session = %session_id, tab = %tab_id, %why,
                    "a terminal tab's launch record cannot be read; leaving the tab unknown");
                HostTab::placeholder(
                    &tab_id,
                    &path,
                    Unavailability::Unidentified(format!(
                        "its launch record cannot be read, so its socket is not known: {why}"
                    )),
                    None,
                )
            }
            Ok(record) if !owns_state_dir => HostTab::placeholder(
                &tab_id,
                &record.socket.socket_in(&hosts_dir),
                Unavailability::Unidentified(
                    "this supervisor does not hold its state directory, so it leaves the \
                     terminal hosts there to the one that does"
                        .into(),
                ),
                None,
            ),
            Ok(record) => {
                let name = OsString::from(record.socket.to_string());
                if held_names.contains(&name) {
                    continue;
                }
                let socket = hosts_dir.join(&name);
                pending.push(Pending {
                    session_id,
                    tab_id,
                    record,
                    name,
                    socket,
                });
                continue;
            }
        };
        found.entry(session_id).or_default().push(tab);
    }
    if pending.is_empty() {
        return found;
    }
    let earlier_gone = gone.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let scan = ScanOptions {
        deadlines: options.deadlines,
        skip: held_names.iter().chain(&earlier_gone).cloned().collect(),
        only: Some(pending.iter().map(|record| record.name.clone()).collect()),
    };
    let _no_spawns = match &no_spawns {
        Some(lock) => Some(lock.clone().write_owned().await),
        None => None,
    };
    let mut registry = match HostRegistry::scan(state_dir, &scan).await {
        Ok(registry) => registry,
        Err(error) => {
            warn!(error = %error,
                "could not look for the sessions' terminal tab hosts; leaving those tabs unknown");
            let why = format!("the terminal host directory could not be scanned: {error}");
            for record in pending {
                let tab = HostTab::placeholder(
                    &record.tab_id,
                    &record.socket,
                    Unavailability::NotScanned(why.clone()),
                    None,
                );
                found.entry(record.session_id).or_default().push(tab);
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
    // One task per record from here, all at once: an adoption, or a settle
    // followed by an adoption, each bounded by its own waits, so several
    // interrupted starts cost the slowest one rather than their sum.
    let mut resolving: JoinSet<(Pending, Option<HostTab>)> = JoinSet::new();
    for record in pending {
        match registry.by_name(&record.name) {
            Some(NamedHost::Live(host))
                if names_tab(&host.tag, &record.session_id, &record.tab_id) =>
            {
                let Some(host) = registry.take(&record.name) else {
                    continue;
                };
                let options = options.clone();
                resolving.spawn(async move {
                    let tab = adopt_tab(
                        &record.session_id,
                        &record.tab_id,
                        &record.socket,
                        host,
                        options,
                    )
                    .await;
                    (record, tab)
                });
            }
            Some(NamedHost::Live(host)) => {
                let tab = HostTab::placeholder(
                    &record.tab_id,
                    &record.socket,
                    Unavailability::Unidentified(format!(
                        "the host at its socket is tagged for another terminal ({:?})",
                        host.tag
                    )),
                    host.peer(),
                );
                found.entry(record.session_id).or_default().push(tab);
            }
            Some(NamedHost::Unavailable(host)) => {
                let tab = HostTab::placeholder(
                    &record.tab_id,
                    &record.socket,
                    unavailability(&host.why),
                    host.peer,
                );
                found.entry(record.session_id).or_default().push(tab);
            }
            // Nothing there at the scan (or not looked at): settled, so that
            // a start the scan raced (a first stage that bound the socket
            // during or after it, and may have ended since) is found by its
            // own record rather than read as gone or as unreachable.
            Some(NamedHost::Gone(_)) | None => {
                let options = options.clone();
                let faults = faults.clone();
                resolving.spawn(async move {
                    let tab = settle_tab(&record, options, &faults).await;
                    (record, tab)
                });
            }
        }
    }
    while let Some(joined) = resolving.join_next().await {
        // The tasks are this function's own and never aborted; a panic in
        // one is a bug, surfaced where reload was called.
        let (record, tab) = match joined {
            Ok(resolved) => resolved,
            Err(error) => std::panic::resume_unwind(error.into_panic()),
        };
        match tab {
            Some(tab) => found.entry(record.session_id).or_default().push(tab),
            None => info!(session = %record.session_id, tab = %record.tab_id,
                "a terminal tab's host ended while no supervisor was running"),
        }
    }
    found
}

/// Whether `tag` names this session and tab, shared by startup discovery
/// and a retry at the tab's recorded socket.
fn names_tab(tag: &HostTag, session_id: &str, wanted_tab: &str) -> bool {
    tag.session_id == session_id
        && matches!(&tag.role, HostRole::Tab { tab_id, .. } if tab_id.as_str() == wanted_tab)
}

/// Adopt the live host the scan or a settle found for this tab at `socket`, as
/// recovered: its runtime, a placeholder when it cannot be used, or `None`
/// when it ended first. Identity was checked by [`names_tab`] before this
/// call, so the tab's creation stamp and scope come from its tag.
async fn adopt_tab(
    session_id: &str,
    tab_id: &str,
    socket: &Path,
    host: LiveHost,
    options: HostTerminalOptions,
) -> Option<HostTab> {
    let (created_ns, scoped) = match &host.tag.role {
        HostRole::Tab { scoped, .. } => (host.tag.created_ns, *scoped),
        HostRole::Agent { .. } => (u64::MAX, false),
    };
    let host = match HostTerminal::adopt_found(host, Adoption::Recovered, options).await {
        Ok(host) => {
            info!(session = %session_id, tab = %tab_id,
                socket = %socket.display(),
                "found a terminal tab's host again and adopted it");
            host
        }
        Err(AdoptError::Gone(why)) => {
            info!(session = %session_id, tab = %tab_id, %why,
                "a terminal tab's host ended before it could be adopted");
            return None;
        }
        Err(AdoptError::Unavailable { reason, peer }) => {
            warn!(session = %session_id, tab = %tab_id, %reason,
                "could not adopt a terminal tab's host; leaving it as it is");
            HostTerminal::unavailable(socket, reason, peer)
        }
    };
    Some(HostTab {
        tab_id: tab_id.to_string(),
        created_ns,
        scoped,
        host,
        end_failed: false,
        failed_ends: 0,
        retry_at: None,
    })
}

/// What `record`'s socket holds when the scan found nothing there: settled
/// by the rules a retried create uses (`settle_launch_record_with`), then
/// adopted ([`adopt_tab`]) when a host tagged as this tab answers, `None`
/// when nothing is there and nothing can start it, and a placeholder for
/// everything else.
///
/// Cheap in the ordinary case: a first stage long gone is checked once, a
/// missing socket file is gone at once, and a socket file a killed host
/// left costs the client's refusal window (three seconds). The worst case,
/// a first stage that does not finish, costs the settle's waits: five
/// seconds for the first stage, then the verified signal's graces (up to
/// ten), then up to five for a forked second stage. Every record is settled
/// at once, so a reload pays the slowest record's, not their sum.
async fn settle_tab(
    record: &Pending,
    options: HostTerminalOptions,
    faults: &super::FaultHooks,
) -> Option<HostTab> {
    let placeholder = |reason: Unavailability, peer| {
        Some(HostTab::placeholder(
            &record.tab_id,
            &record.socket,
            reason,
            peer,
        ))
    };
    match settle_launch_record_with(&record.record, &record.socket, faults).await {
        Ok(AtRecordedSocket::Gone) => None,
        Ok(AtRecordedSocket::Answered { connection, info }) => match HostTag::decode(&info.tag) {
            Ok(tag) if names_tab(&tag, &record.session_id, &record.tab_id) => {
                let host = LiveHost {
                    name: record.name.clone(),
                    socket: record.socket.clone(),
                    tag,
                    info,
                    connection: *connection,
                };
                adopt_tab(
                    &record.session_id,
                    &record.tab_id,
                    &record.socket,
                    host,
                    options,
                )
                .await
            }
            Ok(tag) => placeholder(
                Unavailability::Unidentified(format!(
                    "the host at its socket is tagged for another terminal ({tag:?})"
                )),
                connection.peer(),
            ),
            Err(unreadable) => placeholder(
                Unavailability::Unidentified(format!("its tag cannot be read: {unreadable}")),
                connection.peer(),
            ),
        },
        Ok(AtRecordedSocket::NoAnswer(error)) => {
            let peer = match &error {
                crate::hosts::ConnectError::Busy { peer }
                | crate::hosts::ConnectError::Unavailable { peer, .. } => *peer,
                crate::hosts::ConnectError::Gone(_) => None,
            };
            placeholder(
                Unavailability::Unidentified(format!(
                    "its socket does not answer usefully: {error}"
                )),
                peer,
            )
        }
        Ok(AtRecordedSocket::NoInfo { error, peer }) => {
            placeholder(Unavailability::Request(error), peer)
        }
        Err(why) => {
            warn!(session = %record.session_id, tab = %record.tab_id, %why,
                "an interrupted terminal tab start could not be settled; leaving the tab \
                 unknown");
            placeholder(
                Unavailability::Unidentified(format!(
                    "its interrupted start (process {}) could not be confirmed ended: {why}",
                    record.record.starter.pid
                )),
                None,
            )
        }
    }
}

/// Every tab record in `state_dir`'s launch directory whose session is in
/// `sessions`, read. A launch directory that cannot be listed gives none,
/// logged: the tabs then come back at the next start.
async fn tab_records(state_dir: &Path, sessions: &HashSet<String>) -> Vec<TabRecord> {
    let launch_dir = state_dir.join("launch");
    let mut entries = match tokio::fs::read_dir(&launch_dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            warn!(dir = %launch_dir.display(), %error,
                "could not list the launch directory for terminal tab hosts");
            return Vec::new();
        }
    };
    let mut records = Vec::new();
    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(error) => {
                warn!(dir = %launch_dir.display(), %error,
                    "listing the launch directory for terminal tab hosts stopped early");
                break;
            }
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some((session_id, crate::launch::LaunchFileOf::Tab(tab_id))) =
            crate::launch::parse_launch_file_name(&name)
        else {
            continue;
        };
        if !sessions.contains(session_id) {
            continue;
        }
        let path = entry.path();
        let read = match read_launch_record(&path).await {
            Ok(Some(record)) => Ok(record),
            Ok(None) => continue,
            Err(error) => Err(format!("{error:#}")),
        };
        records.push(TabRecord {
            session_id: session_id.to_string(),
            tab_id: tab_id.to_string(),
            read,
            path,
        });
    }
    records.sort_by(|a, b| (&a.session_id, &a.tab_id).cmp(&(&b.session_id, &b.tab_id)));
    records
}

/// Every host tab runtime in `sessions`' cells, placeholders included: what
/// the orderly stop (`host_handover`) gathers beside the agents' runtimes,
/// to checkpoint the ones still driving their hosts and close them all.
pub(super) fn tab_runtimes<'a>(
    sessions: impl IntoIterator<Item = &'a Arc<SessionEntry>>,
) -> Vec<HostTerminal> {
    let mut all = Vec::new();
    for entry in sessions {
        for tab in host_tabs_of(&entry.session.host_tabs) {
            if !all.contains(&tab.host) {
                all.push(tab.host);
            }
        }
    }
    all
}

#[cfg(test)]
mod tests {
    //! The tab strip's order and filter over a session's host tabs, and what
    //! the reap picks, with stand-in hosts in the test process
    //! (`hosts::terminal::test_host`) and placeholders. Re-adoption's
    //! schedule, cell fences and record cleanup are pinned against those
    //! stand-ins in `core::tests::host_terminals`; actual host opening,
    //! closing and process cleanup run in
    //! `crates/farhelm/tests/e2e/host_tabs.rs`.

    use super::*;
    use crate::hosts::terminal::test_host::adopt_test_host;

    /// A placeholder tab `tab_id` stamped `created_ns`.
    fn placeholder(tab_id: &str, created_ns: u64) -> HostTab {
        HostTab {
            created_ns,
            ..HostTab::placeholder(
                tab_id,
                Path::new("/nonexistent/hosts/abcdefgh"),
                Unavailability::Busy,
                None,
            )
        }
    }

    /// The tab `tab_id` stamped `created_ns` whose runtime is `host`.
    fn driven(tab_id: &str, created_ns: u64, host: &HostTerminal) -> HostTab {
        HostTab {
            tab_id: tab_id.to_string(),
            created_ns,
            scoped: false,
            host: host.clone(),
            end_failed: false,
            failed_ends: 0,
            retry_at: None,
        }
    }

    /// Host tabs list in the order they were opened, whatever order the
    /// session's cell holds them in; an exited tab is left out unless asked
    /// for and is what the reap closes; a tab whose host could not be read
    /// is listed (last when its stamp is unknown) and never reaped; a tab
    /// whose earlier close failed is reaped again, once its retry time has
    /// come, while still listed.
    ///
    /// Why: SPEC.md promises a tab strip in creation order, and host tabs
    /// have no window ids to order them by; their tags' creation stamps are
    /// the order (the cell's own order changes as tabs are adopted after a
    /// restart, all at once). An exited tab must vanish from the strip as a
    /// dead tmux window does and be reaped, a placeholder's host may still
    /// run so hiding or reaping it would lose a running tab, and a tab whose
    /// close could not confirm its host ended would otherwise sit with a
    /// stopped runtime forever. Spec: a cell holding a running stand-in
    /// stamped 30, an exited one stamped 10, placeholders stamped 20, 10
    /// (id "a") and unknown, lists "a", "d", "c", "unknown" without exited
    /// tabs and "a", "b", "d", "c", "unknown" with them; the reap picks only
    /// the exited "b"; a lookup finds a tab by id and nothing for an unknown
    /// id; once "c" is marked as a failed close, the reap still picks only
    /// "b" until "c"'s retry time, and then "b" and "c", while the strip
    /// still lists "c".
    #[farhelm_testtrace::test]
    async fn host_tabs_list_in_creation_order_and_the_reap_picks_finished_ones() {
        let (running, _running_host) = adopt_test_host(None).await;
        let (exited, exited_host) = adopt_test_host(None).await;
        exited_host.exit(0);
        let mut status = exited.subscribe();
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while !matches!(exited.evidence(), HostEvidence::Owned { dead: true, .. }) {
                status.changed().await.expect("the runtime runs");
            }
        })
        .await
        .expect("premise: the stand-in's exit is processed");
        let cell = host_tab_cell(vec![
            driven("c", 30, &running),
            driven("b", 10, &exited),
            placeholder("unknown", u64::MAX),
            placeholder("d", 20),
            placeholder("a", 10),
        ]);
        let ids = |tabs: Vec<TabInfo>| tabs.into_iter().map(|tab| tab.id).collect::<Vec<_>>();
        assert_eq!(
            ids(listed_host_tabs(&cell, false)),
            vec!["a", "d", "c", "unknown"]
        );
        assert_eq!(
            ids(listed_host_tabs(&cell, true)),
            vec!["a", "b", "d", "c", "unknown"]
        );
        assert_eq!(reap_due_host_tabs(&cell), vec!["b"]);
        assert_eq!(
            find_host_tab(&cell, "d").map(|tab| tab.created_ns),
            Some(20)
        );
        assert!(find_host_tab(&cell, "z").is_none());

        mark_end_failed(&cell, &running);
        assert_eq!(
            reap_due_host_tabs(&cell),
            vec!["b"],
            "a failed close waits for its retry time"
        );
        for tab in cell.lock().unwrap().iter_mut() {
            if tab.tab_id == "c" {
                tab.retry_at = Some(tokio::time::Instant::now());
            }
        }
        assert_eq!(reap_due_host_tabs(&cell), vec!["b", "c"]);
        assert_eq!(
            ids(listed_host_tabs(&cell, false)),
            vec!["a", "d", "c", "unknown"]
        );
    }

    /// A failed end's retry waits 1 s, doubling, at most a minute.
    ///
    /// Why: an end that keeps failing would otherwise be retried on every
    /// reap pass, holding its session's lifecycle claim and a reap permit
    /// almost continuously. Spec: after failure 1 the wait is 1 s, after 2
    /// it is 2 s, after 3 it is 4 s, and from failure 7 on it is 60 s.
    #[test]
    fn a_failed_ends_retry_backs_off_to_a_minute() {
        let secs = |failures| retry_delay(failures).as_secs();
        assert_eq!([1, 2, 3, 6, 7, 40].map(secs), [1, 2, 4, 32, 60, 60]);
    }

    /// A new tab's stamp is past every stamp already in the session, even
    /// when the clock reads earlier.
    ///
    /// Why: the stamp is the tab strip's order, kept in the tag for the
    /// host's life; a clock stepped backwards between two opens would
    /// otherwise list the later tab first, for good. Spec: with a tab
    /// stamped far in the future, the next stamp is one past it; an unknown
    /// stamp is not counted; with no tabs, the stamp is now.
    #[farhelm_testtrace::test]
    async fn a_new_tab_is_stamped_after_the_sessions_newest() {
        let future = crate::hosts::tag::creation_stamp() + 3_600_000_000_000;
        let cell = host_tab_cell(vec![placeholder("x", future), placeholder("y", u64::MAX)]);
        assert_eq!(next_creation_stamp(&cell), future + 1);
        let before = crate::hosts::tag::creation_stamp();
        let stamp = next_creation_stamp(&host_tab_cell(Vec::new()));
        assert!(stamp >= before && stamp <= crate::hosts::tag::creation_stamp());
    }
}
