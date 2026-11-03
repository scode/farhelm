//! Ending every terminal host of a state directory, for
//! `farhelm internal stop-terminal-hosts --state-dir <dir>`.
//!
//! A host outlives the supervisor that started it, so removing an install
//! (or cleaning up after one) has to end its hosts on purpose: nothing else
//! will. This is that step, kept apart from the supervisor's own use of
//! hosts so that it can run with no supervisor at all, typically after the
//! supervisor has stopped. A newer helm runs it on a remote host whose
//! installed `farhelm` may be older than the helm, so the command line and
//! its exit status are a surface every later release keeps (SPEC_impl.md,
//! "Terminal host", the plan's D-20); the per-host lines it writes to stderr
//! are for people and logs, not for parsing.
//!
//! The run has three phases, all within one deadline:
//!
//! 1. **Finding the hosts** ([`discover`]), as blocking work the run stops
//!    waiting for at the deadline (a stalled filesystem or process table
//!    must not hold the caller): the sockets directly inside
//!    `<state dir>/hosts`, listed through one handle opened without
//!    following a symlink, which is also the handle a leftover socket file
//!    is later removed through. A `hosts` that is a symlink or not a
//!    directory, or that cannot be opened, is refused, and nothing it leads
//!    to is contacted. Hosts are found by place, as tmux's sessions are by
//!    its socket: a host whose socket file was deleted inside an intact
//!    directory is not found (in production the remote uninstall's stop of
//!    the supervisor's unit ends such hosts with the unit's control group).
//! 2. **Stopping them**, all at once:
//!    - **Through the protocol first.** Connect with take-over (the client's
//!      own rules, [`super::client`]), send `Shutdown`, and wait for the
//!      host's process to go. A host that ends this way closes its pty, so
//!      the program in it gets the terminal's hangup, and removes its own
//!      socket file.
//!    - **The verified signal otherwise.** A host that does not answer
//!      (stopped, wedged), stays busy, speaks a protocol version this build
//!      does not, or took `Shutdown` and did not end, is ended with
//!      [`super::process::terminate_verified`], which signals only the pid
//!      the connection (or the process table) named, and only while that
//!      process still has the host's start identity and a command line
//!      naming its socket exactly as this run spells it. A host started
//!      under another spelling of the state directory (through a symlink)
//!      stops through the protocol all the same, but one that cannot take
//!      `Shutdown` stays unconfirmed rather than be signalled on a guess.
//!    - **Gone is gone.** A socket nothing serves (the client's
//!      [`super::client::Gone`]: no file, or refusals for the whole refusal
//!      window with no process serving it) counts as stopped, and a socket
//!      file left behind by a host that is now gone (killed with SIGKILL,
//!      say) is removed.
//! 3. **Reporting** ([`write_lines_within`]): one line per host on stderr,
//!    written on a thread of its own and waited for only briefly, so that a
//!    stderr nobody reads (a full pipe) cannot hold the exit status back.
//!
//! "Stopped" means confirmed: the host's process was seen gone (or, when the
//! platform named no process, its socket file was), or nothing served the
//! socket. A `Goodbye` or a closed connection is not enough. Anything less
//! is unconfirmed, and one unconfirmed host, or a search for hosts that was
//! not complete, makes the whole run fail, so a caller knows to retry or to
//! say so. A state directory with no `hosts` directory has no hosts and
//! succeeds.
//!
//! Deliberately out of scope: the programs inside hosts get their hangup
//! and nothing more (descendants that ignore it are the business of the
//! session's scope or the supervisor's unit, as for tmux), nothing outside
//! the one `hosts` directory is looked at, and no state lock is taken: a
//! supervisor still running on the state directory loses its terminals'
//! hosts, which is what a caller of this command asked for.

use std::collections::{BTreeSet, HashMap};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::AsFd as _;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rustix::fs::{AtFlags, FileType, Mode, OFlags};
use tokio::task::JoinSet;
use tokio::time::Instant;

use super::client::{ClientDeadlines, ConnectError, Gone, HostConnection};
use super::dir::hosts_dir;
use super::process::{HostProcess, Termination, terminate_verified};

// ---- Limits --------------------------------------------------------------------

/// How long the whole command takes at most, from the call to the exit
/// status: [`STOP_BUDGET`] for finding and stopping the hosts, then at most
/// [`REPORT_WAIT`] for the report on stderr.
pub const COMMAND_BUDGET: Duration = Duration::from_secs(30);

/// How long the command waits for its report to be written to stderr
/// before it exits anyway (a stderr whose reader stopped reading).
pub const REPORT_WAIT: Duration = Duration::from_secs(1);

/// How long finding and stopping the hosts takes at most, from the call.
///
/// Hosts are handled at once, so this bounds the slowest host, not their
/// sum, plus finding them. The slowest host the client and the signal can
/// describe is one that answers only at the end of the connect deadline
/// (5 s), takes `Shutdown` within its deadline (5 s), then lingers past
/// [`END_WAIT`] (5 s), and is signalled through SIGTERM's and SIGKILL's
/// graces (5 s each): 25 s. A host that never answers costs the connect
/// deadline and the graces; a socket that only refuses at most the connect
/// deadline and the refusal window (3 s); and on macOS, a stopped host whose
/// backlog is full refuses like that, is then found by its process, and is
/// signalled: the connect deadline, the refusal window and both graces,
/// 18 s. A signal that finds its process already gone looks at the socket
/// once more instead of waiting out the graces, which costs at most another
/// connect deadline and refusal window (8 s) in their place: 23 s after a
/// `Shutdown` that did not end the host. That leaves a few seconds for
/// finding the hosts on a loaded machine.
pub const STOP_BUDGET: Duration = Duration::from_secs(29);

const _: () =
    assert!(STOP_BUDGET.as_millis() + REPORT_WAIT.as_millis() <= COMMAND_BUDGET.as_millis());

/// How long a host that answered `Shutdown` gets for its process (or its
/// socket file) to go before it is signalled. A host exits as soon as its
/// `Goodbye` is written, so this is room for a loaded machine.
pub const END_WAIT: Duration = Duration::from_secs(5);

/// How often a wait for a host to end looks again.
const END_POLL: Duration = Duration::from_millis(50);

/// The time limits one run works within. [`Default`] gives the constants
/// above; tests shorten them to reach each outcome in moments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopLimits {
    /// See [`STOP_BUDGET`].
    pub budget: Duration,
    /// The client's connect, refusal and shutdown deadlines.
    pub client: ClientDeadlines,
    /// See [`END_WAIT`].
    pub end_wait: Duration,
}

impl Default for StopLimits {
    fn default() -> Self {
        Self {
            budget: STOP_BUDGET,
            client: ClientDeadlines::default(),
            end_wait: END_WAIT,
        }
    }
}

// ---- Outcomes ------------------------------------------------------------------

/// How one host was confirmed stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// It took `Shutdown` and ended.
    ShutDown,
    /// The protocol could not end it; the verified signal
    /// ([`super::process::terminate_verified`]'s checks) did, or found
    /// the process already gone. Never [`Termination::StillRunning`].
    Signalled(Termination),
    /// Nothing served the socket.
    Gone(Gone),
}

impl fmt::Display for Stopped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShutDown => f.write_str("stopped through the protocol"),
            Self::Signalled(Termination::AlreadyGone) => f.write_str("ended while being stopped"),
            Self::Signalled(Termination::Terminated) => f.write_str("ended by SIGTERM"),
            Self::Signalled(Termination::Killed) => f.write_str("ended by SIGKILL"),
            Self::Signalled(Termination::StillRunning) => f.write_str("survived SIGKILL"),
            Self::Gone(gone) => write!(f, "already gone ({gone})"),
        }
    }
}

/// What became of one host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostOutcome {
    Stopped(Stopped),
    /// Not confirmed stopped: the host may still run. The reason is for the
    /// diagnostic line.
    Unconfirmed(String),
}

/// One host's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostReport {
    /// The host's socket, spelled from the given state directory.
    pub socket: PathBuf,
    pub outcome: HostOutcome,
    /// Why a socket file left by a stopped host could not be removed. Only a
    /// diagnostic: the file holds no process, and the host still counts as
    /// stopped.
    pub leftover: Option<String>,
}

/// What a whole run found and did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StopReport {
    /// Every host found, in path order.
    pub hosts: Vec<HostReport>,
    /// What kept the search for hosts from being complete, if anything did.
    /// A host the search missed may still run, so any problem here makes
    /// the run unconfirmed.
    pub scan_problems: Vec<String>,
}

impl StopReport {
    /// Whether every host was confirmed stopped and the search was
    /// complete: exit status 0.
    pub fn all_stopped(&self) -> bool {
        self.scan_problems.is_empty()
            && self
                .hosts
                .iter()
                .all(|host| matches!(host.outcome, HostOutcome::Stopped(_)))
    }

    /// One line per search problem and per host, for stderr.
    pub fn lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self.scan_problems.clone();
        for host in &self.hosts {
            let socket = host.socket.display();
            lines.push(match &host.outcome {
                HostOutcome::Stopped(how) => format!("terminal host {socket}: {how}"),
                HostOutcome::Unconfirmed(why) => {
                    format!("terminal host {socket}: could not confirm it stopped: {why}")
                }
            });
            if let Some(leftover) = &host.leftover {
                lines.push(format!("terminal host {socket}: {leftover}"));
            }
        }
        lines
    }
}

// ---- The run -------------------------------------------------------------------

/// Find and end every terminal host of `state_dir` within [`STOP_BUDGET`].
pub async fn stop_terminal_hosts(state_dir: &Path) -> StopReport {
    stop_terminal_hosts_within(state_dir, StopLimits::default()).await
}

/// [`stop_terminal_hosts`] with its limits injected, so that tests reach
/// the budget and each outcome in moments.
pub async fn stop_terminal_hosts_within(state_dir: &Path, limits: StopLimits) -> StopReport {
    let state_dir = state_dir.to_path_buf();
    run(limits, move || discover(&state_dir)).await
}

/// The hosts a search found, and what kept it from being complete.
#[derive(Default)]
struct Discovery {
    /// Hosts with a socket in the socket directory, in path order.
    sockets: Vec<Target>,
    problems: Vec<String>,
}

/// One host socket as the search found it.
#[derive(Clone)]
struct Target {
    /// The socket's path, spelled from the given state directory: what the
    /// client connects to, what the verified signal compares a command line
    /// with, and what the report names.
    path: PathBuf,
    /// The socket's file name.
    name: OsString,
    /// The socket directory as the search opened it (without following a
    /// symlink), through which a leftover socket file is removed.
    dir: Arc<File>,
}

/// One run, with the search for hosts injected so that a stalled search
/// can be tested.
///
/// The search runs as blocking work that is given up on at the deadline
/// (its thread is left to finish on its own; the command's runtime does
/// not wait for it). Hosts still being handled at the deadline are given up
/// on too (their tasks are aborted, possibly between a SIGTERM and its
/// confirmation) and reported unconfirmed.
async fn run<F>(limits: StopLimits, discover: F) -> StopReport
where
    F: FnOnce() -> Discovery + Send + 'static,
{
    let deadline = Instant::now() + limits.budget;
    let found = match tokio::time::timeout_at(deadline, tokio::task::spawn_blocking(discover)).await
    {
        Ok(Ok(found)) => found,
        Ok(Err(error)) => Discovery {
            problems: vec![format!("finding the terminal hosts failed: {error}")],
            ..Discovery::default()
        },
        Err(_) => Discovery {
            problems: vec![
                "finding the terminal hosts did not finish within the time budget".into(),
            ],
            ..Discovery::default()
        },
    };
    let mut tasks = JoinSet::new();
    let mut pending = HashMap::new();
    for target in found.sockets {
        let socket = target.path.clone();
        let task = tasks.spawn(stop_host(target, limits));
        pending.insert(task.id(), socket);
    }
    StopReport {
        hosts: join_within(tasks, pending, deadline).await,
        scan_problems: found.problems,
    }
}

/// Collect every host task's report until `deadline`, reporting a task
/// that failed (panicked) as such and every task still running then as cut
/// off by the budget; `pending` names each task's host.
async fn join_within(
    mut tasks: JoinSet<HostReport>,
    mut pending: HashMap<tokio::task::Id, PathBuf>,
    deadline: Instant,
) -> Vec<HostReport> {
    let unconfirmed = |socket: PathBuf, why: String| HostReport {
        socket,
        outcome: HostOutcome::Unconfirmed(why),
        leftover: None,
    };
    let mut hosts = Vec::new();
    loop {
        match tokio::time::timeout_at(deadline, tasks.join_next_with_id()).await {
            Ok(None) | Err(_) => break,
            Ok(Some(Ok((id, report)))) => {
                pending.remove(&id);
                hosts.push(report);
            }
            Ok(Some(Err(error))) => {
                if let Some(host) = pending.remove(&error.id()) {
                    hosts.push(unconfirmed(host, format!("its stop failed: {error}")));
                }
            }
        }
    }
    tasks.abort_all();
    hosts.extend(pending.into_values().map(|host| {
        unconfirmed(
            host,
            "its stop did not finish within the time budget".into(),
        )
    }));
    hosts.sort_by(|a, b| a.socket.cmp(&b.socket));
    hosts
}

// ---- Finding the hosts -----------------------------------------------------------

/// Search `state_dir` for hosts: the sockets in its socket directory.
/// Blocking.
fn discover(state_dir: &Path) -> Discovery {
    let dir = hosts_dir(state_dir);
    let mut found = Discovery::default();
    if let Listing::Listed { handle, names } = list_sockets(&dir, &mut found.problems) {
        let handle = Arc::new(handle);
        found.sockets = names
            .into_iter()
            .map(|name| Target {
                path: dir.join(&name),
                name,
                dir: handle.clone(),
            })
            .collect();
    }
    found
}

/// What listing a socket directory found.
enum Listing {
    /// There is no socket directory (nor, perhaps, a state directory).
    Absent,
    /// Something is there that cannot be used as the socket directory (a
    /// symlink, a file) or could not be opened; a problem says which.
    Refused,
    /// The directory, opened without following a symlink, and the names of
    /// the sockets listed through it, in order (all of them, unless a
    /// problem says otherwise).
    Listed {
        handle: File,
        names: BTreeSet<OsString>,
    },
}

/// List the sockets directly inside `dir`.
///
/// `dir` is opened once, refusing a symlink (no supervisor puts hosts
/// behind one, `super::dir` refuses it, and following one could reach
/// another state directory's hosts), and everything is read through that
/// handle, so a symlink swapped in meanwhile is never followed. Entries
/// that are not sockets (files, directories) are no hosts and are skipped;
/// so is one that vanished while being read.
fn list_sockets(dir: &Path, problems: &mut Vec<String>) -> Listing {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let handle = match rustix::fs::open(dir, flags, Mode::empty()) {
        Ok(handle) => File::from(handle),
        Err(rustix::io::Errno::NOENT) => return Listing::Absent,
        Err(rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR) => {
            problems.push(format!(
                "{} is not a directory; nothing it leads to was looked at",
                dir.display()
            ));
            return Listing::Refused;
        }
        Err(error) => {
            problems.push(format!(
                "cannot open {}, so no terminal host of it was looked at: {}",
                dir.display(),
                io::Error::from(error)
            ));
            return Listing::Refused;
        }
    };
    let mut names = BTreeSet::new();
    let entries = match rustix::fs::Dir::read_from(&handle) {
        Ok(entries) => entries,
        Err(error) => {
            problems.push(format!(
                "cannot list {}: {}",
                dir.display(),
                io::Error::from(error)
            ));
            return Listing::Listed { handle, names };
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                problems.push(format!(
                    "error listing {}: {}",
                    dir.display(),
                    io::Error::from(error)
                ));
                continue;
            }
        };
        let name = entry.file_name();
        if name == c"." || name == c".." {
            continue;
        }
        let os_name = OsStr::from_bytes(name.to_bytes());
        match rustix::fs::statat(handle.as_fd(), name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::Socket => {
                names.insert(os_name.to_owned());
            }
            Ok(_) | Err(rustix::io::Errno::NOENT) => {}
            Err(error) => problems.push(format!(
                "cannot stat {}: {}",
                dir.join(os_name).display(),
                io::Error::from(error)
            )),
        }
    }
    Listing::Listed { handle, names }
}

// ---- One host ------------------------------------------------------------------

/// Stop the host at `target`'s socket, and remove its socket file if one is
/// left once it is confirmed stopped.
async fn stop_host(target: Target, limits: StopLimits) -> HostReport {
    let outcome = match HostConnection::connect_with(&target.path, limits.client).await {
        Ok(connection) => shut_down(connection, &target, limits).await,
        Err(ConnectError::Gone(gone)) => HostOutcome::Stopped(Stopped::Gone(gone)),
        Err(error @ (ConnectError::Busy { peer } | ConnectError::Unavailable { peer, .. })) => {
            match peer {
                Some(peer) => signal(&peer, &target, &error.to_string(), limits.client).await,
                None => HostOutcome::Unconfirmed(format!("{error}, and its process is unknown")),
            }
        }
    };
    finish(target, outcome)
}

/// The report for one host, removing the socket file a stopped host left.
fn finish(target: Target, outcome: HostOutcome) -> HostReport {
    let leftover = match outcome {
        HostOutcome::Stopped(_) => remove_left_socket(&target),
        HostOutcome::Unconfirmed(_) => None,
    };
    HostReport {
        socket: target.path,
        outcome,
        leftover,
    }
}

/// `Shutdown` over a connection, falling back to the verified signal when
/// it fails or the host does not end within the limits' end wait.
async fn shut_down(connection: HostConnection, target: &Target, limits: StopLimits) -> HostOutcome {
    let peer = connection.peer();
    let shutdown = connection.shutdown().await;
    let why = match shutdown {
        // A `Goodbye`, or a connection that ended once `Shutdown` was
        // written whole: neither is a confirmation (see `ShutdownOutcome`);
        // the process (or the socket file) going is.
        Ok(_) => {
            if ended(peer, &target.path, limits.end_wait).await {
                return HostOutcome::Stopped(Stopped::ShutDown);
            }
            "it took Shutdown but did not end".to_string()
        }
        Err(error) => format!("Shutdown failed: {error}"),
    };
    match peer {
        Some(peer) => signal(&peer, target, &why, limits.client).await,
        None => HostOutcome::Unconfirmed(format!("{why}, and its process is unknown")),
    }
}

/// Whether the host ends within `wait`: its process, when the connection
/// named one, else its socket file, which a host removes just before it
/// exits. A process table that cannot be read says nothing, so the wait
/// goes on (and the verified signal after it reports what it could not
/// read).
async fn ended(peer: Option<HostProcess>, socket: &Path, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        let gone = match peer {
            Some(peer) => peer.is_running() == Ok(false),
            None => matches!(
                std::fs::symlink_metadata(socket),
                Err(error) if error.kind() == io::ErrorKind::NotFound
            ),
        };
        if gone {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(END_POLL).await;
    }
}

/// End `peer` with the verified signal, which checks that its command line
/// names `target`'s socket path exactly; `why` says why the protocol did
/// not.
///
/// `AlreadyGone` (the pinned process no longer runs) confirms nothing by
/// itself. `peer` may not be the host at all: when no connection named
/// one, it comes from the client's search of the process table, which
/// matches any process naming an absolute socket path with this socket's
/// file name, in whatever directory. Such a process exiting says nothing
/// about the host. So the socket is looked at again, by the client's own
/// rules, and only a socket judged gone (no file, or refusals with no
/// process serving it) confirms the stop.
async fn signal(
    peer: &HostProcess,
    target: &Target,
    why: &str,
    client: ClientDeadlines,
) -> HostOutcome {
    match terminate_verified(peer, &target.path).await {
        Ok(Termination::StillRunning) => HostOutcome::Unconfirmed(format!(
            "{why}; process {} survived SIGTERM and SIGKILL",
            peer.pid
        )),
        Ok(Termination::AlreadyGone) => {
            match HostConnection::connect_with(&target.path, client).await {
                Err(ConnectError::Gone(_)) => {
                    HostOutcome::Stopped(Stopped::Signalled(Termination::AlreadyGone))
                }
                Ok(_) => HostOutcome::Unconfirmed(format!(
                    "{why}; process {} ended by itself, but something still answers at the socket",
                    peer.pid
                )),
                Err(error) => HostOutcome::Unconfirmed(format!(
                    "{why}; process {} ended by itself, but the socket is not gone: {error}",
                    peer.pid
                )),
            }
        }
        Ok(termination) => HostOutcome::Stopped(Stopped::Signalled(termination)),
        Err(error) => HostOutcome::Unconfirmed(format!("{why}; {error}")),
    }
}

/// Remove the socket file of a host confirmed stopped, if one is left (a
/// host killed by SIGKILL, or judged gone after refusals, leaves it) and it
/// is still a socket, through the socket directory's handle. `Some` says
/// why it could not be.
///
/// Only called once the process that served the socket is gone, or nothing
/// served it: a host refuses to bind a path that exists, so no live host
/// can be serving a file found there now, short of a new host drawing the
/// same random name in between.
fn remove_left_socket(target: &Target) -> Option<String> {
    let dir = &target.dir;
    let name = target.name.as_os_str();
    match rustix::fs::statat(dir.as_fd(), name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::Socket => {}
        _ => return None,
    }
    match rustix::fs::unlinkat(dir.as_fd(), name, AtFlags::empty()) {
        Ok(()) | Err(rustix::io::Errno::NOENT) => None,
        Err(error) => Some(format!(
            "cannot remove its leftover socket file: {}",
            io::Error::from(error)
        )),
    }
}

// ---- Reporting -----------------------------------------------------------------

/// Write `lines` to `out`, one per line, waiting at most `wait` for the
/// write to finish; whether it did.
///
/// The write runs on a thread of its own, which is left behind if it does
/// not finish: a stderr pipe whose reader stopped reading blocks a write
/// for as long as it likes, and the command must still exit within its
/// budget (the caller exits the process right after, which ends the
/// thread). Write errors, a closed stderr among them, are ignored rather
/// than turned into a panic: by now the hosts are handled, and the exit
/// status says how.
pub fn write_lines_within<W>(mut out: W, lines: Vec<String>, wait: Duration) -> bool
where
    W: io::Write + Send + 'static,
{
    if lines.is_empty() {
        return true;
    }
    let mut text = lines.join("\n");
    text.push('\n');
    let (done, finished) = std::sync::mpsc::channel();
    let writer = std::thread::Builder::new()
        .name("stop-report".into())
        .spawn(move || {
            let _ = out.write_all(text.as_bytes());
            let _ = out.flush();
            let _ = done.send(());
        });
    writer.is_ok() && finished.recv_timeout(wait).is_ok()
}

#[cfg(test)]
mod tests {
    //! The listing, the budget, the confirmation rule and the report against
    //! stand-ins in this test process. Real hosts (live, killed, stopped,
    //! through a symlinked state directory, behind a symlinked `hosts`)
    //! are stopped by the real command in
    //! `crates/farhelm/tests/terminal_host_stop.rs`, since a host forks and
    //! is never started inside a test process.

    use super::*;
    use std::io::{Read as _, Write as _};
    use std::os::unix::fs::PermissionsExt as _;
    use std::os::unix::net::UnixListener;
    use terminal_host::protocol::{
        ClientMessage, HostMessage, MAX_FRAME, decode_client, encode_host, split_frame,
    };

    /// Limits short enough that a silent or refusing stand-in is judged in a
    /// second or two, and a stand-in that does not end after `Shutdown` in a
    /// moment.
    fn short() -> StopLimits {
        StopLimits {
            client: ClientDeadlines {
                connect: Duration::from_millis(500),
                refusal_window: Duration::from_millis(500),
                ..ClientDeadlines::default()
            },
            end_wait: Duration::from_millis(200),
            ..StopLimits::default()
        }
    }

    /// A state directory with a private socket directory, as a supervisor
    /// makes it.
    fn state_with_hosts_dir() -> tempfile::TempDir {
        let state = tempfile::tempdir().unwrap();
        crate::hosts::ensure_hosts_dir(state.path()).unwrap();
        state
    }

    /// A stand-in that speaks the protocol from this test process: on its
    /// one connection it reads `Hello`, answers `Welcome`, reads `Shutdown`,
    /// then answers `Goodbye` if `goodbye` is set, and closes the
    /// connection. It keeps listening (its socket file stays) until the
    /// returned listener is dropped; the handle returns it, so the test
    /// decides when.
    fn protocol_stand_in(
        listener: UnixListener,
        goodbye: bool,
    ) -> std::thread::JoinHandle<UnixListener> {
        std::thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut conn = loop {
                match listener.accept() {
                    Ok((conn, _)) => break conn,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(std::time::Instant::now() < deadline, "nothing connected");
                        // sleep-ok: polling interval of a stand-in listener waiting for the client under test.
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("accepting: {error}"),
                }
            };
            conn.set_nonblocking(false).unwrap();
            conn.set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut conn_w = conn.try_clone().unwrap();
            let mut buf = Vec::new();
            let mut next = || loop {
                if let Some(frame) = split_frame(&buf, MAX_FRAME).unwrap() {
                    let message = decode_client(frame, 1).unwrap();
                    let consumed = frame.consumed;
                    buf.drain(..consumed);
                    return message;
                }
                let mut chunk = [0u8; 256];
                let n = conn.read(&mut chunk).unwrap();
                assert!(n > 0, "the client closed early");
                buf.extend_from_slice(&chunk[..n]);
            };
            assert!(matches!(next(), ClientMessage::Hello { .. }));
            conn_w
                .write_all(&encode_host(&HostMessage::Welcome { version: 1 }))
                .unwrap();
            assert_eq!(next(), ClientMessage::Shutdown);
            if goodbye {
                conn_w
                    .write_all(&encode_host(&HostMessage::Goodbye))
                    .unwrap();
            }
            // Both handles close the connection only together.
            drop(conn_w);
            drop(conn);
            listener
        })
    }

    /// A state directory with no socket directory, and one that does not
    /// exist at all, have no hosts and stop successfully.
    ///
    /// Why: the command runs on installs being removed, most of which never
    /// started a host, and a failure there would block or alarm every such
    /// removal. Spec: both give a report with no hosts and no search
    /// problems, which `all_stopped`.
    #[farhelm_testtrace::test]
    async fn a_missing_directory_has_nothing_to_stop() {
        let state = tempfile::tempdir().unwrap();
        for dir in [state.path().to_path_buf(), state.path().join("absent")] {
            let report = stop_terminal_hosts_within(&dir, short()).await;
            assert_eq!(report, StopReport::default(), "{}", dir.display());
            assert!(report.all_stopped());
        }
    }

    /// Only sockets directly inside the socket directory are hosts.
    ///
    /// Why: anything else there is not a host, and contacting it, or
    /// failing over it, would make the command's result depend on stray
    /// files. Spec: with a regular file, a subdirectory holding a stale
    /// socket and one stale socket at the top, the report names that one
    /// socket alone, gone after its refusals, and removes its file; the
    /// nested socket is left as it was.
    #[farhelm_testtrace::test]
    async fn only_sockets_directly_inside_are_hosts() {
        let state = state_with_hosts_dir();
        let hosts = hosts_dir(state.path());
        std::fs::write(hosts.join("not-a-socket"), b"").unwrap();
        std::fs::create_dir(hosts.join("nested")).unwrap();
        drop(UnixListener::bind(hosts.join("nested/aaaaaaaa")).unwrap());
        let stale = hosts.join("bbbbbbbb");
        drop(UnixListener::bind(&stale).unwrap());

        let report = stop_terminal_hosts_within(state.path(), short()).await;
        assert_eq!(
            report.hosts,
            vec![HostReport {
                socket: stale.clone(),
                outcome: HostOutcome::Stopped(Stopped::Gone(Gone::Refused)),
                leftover: None,
            }]
        );
        assert!(report.all_stopped(), "{report:?}");
        assert!(!stale.exists(), "the stale socket file was left");
        assert!(hosts.join("nested/aaaaaaaa").exists());
    }

    /// A socket directory that is a symlink is not followed, and the run
    /// fails.
    ///
    /// Why: no supervisor puts hosts behind a symlink, and following one
    /// could send `Shutdown` to another state directory's hosts. Failing,
    /// rather than reporting nothing to stop, tells the caller that this
    /// state directory is not as Farhelm leaves it. Spec: with `hosts` a
    /// symlink to a directory holding a listening socket, the report has no
    /// hosts and names `hosts` as not a directory, is not `all_stopped`,
    /// and the listener was never connected to.
    #[farhelm_testtrace::test]
    async fn a_symlinked_socket_directory_is_not_followed() {
        let state = tempfile::tempdir().unwrap();
        let elsewhere = state.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).unwrap();
        let listener = UnixListener::bind(elsewhere.join("aaaaaaaa")).unwrap();
        listener.set_nonblocking(true).unwrap();
        std::os::unix::fs::symlink(&elsewhere, hosts_dir(state.path())).unwrap();

        let report = stop_terminal_hosts_within(state.path(), short()).await;
        assert!(report.hosts.is_empty(), "{report:?}");
        assert!(
            report
                .scan_problems
                .iter()
                .any(|p| p.contains("is not a directory")),
            "{report:?}"
        );
        assert!(!report.all_stopped());
        assert_eq!(
            listener.accept().map_err(|error| error.kind()).err(),
            Some(io::ErrorKind::WouldBlock),
            "a socket behind the symlink was contacted"
        );
    }

    /// A socket directory that cannot be listed fails the run.
    ///
    /// Why: a host in a directory the search could not read may still be
    /// running, and reporting success would let an uninstall remove the
    /// state directory under it. Spec: with the socket directory at mode
    /// 000 (and the premise checked that it really cannot be read, which
    /// fails under root), the report is not `all_stopped` and names the
    /// directory.
    #[farhelm_testtrace::test]
    async fn an_unreadable_socket_directory_fails() {
        let state = state_with_hosts_dir();
        let hosts = hosts_dir(state.path());
        std::fs::set_permissions(&hosts, std::fs::Permissions::from_mode(0o000)).unwrap();
        let unreadable = std::fs::read_dir(&hosts).is_err();
        let report = stop_terminal_hosts_within(state.path(), short()).await;
        std::fs::set_permissions(&hosts, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(unreadable, "premise: the directory cannot be read");
        assert!(!report.all_stopped());
        assert!(
            report
                .scan_problems
                .iter()
                .any(|p| p.contains(hosts.to_str().unwrap())),
            "{report:?}"
        );
    }

    /// Something that answers at a host socket but is not a host is never
    /// signalled, and stays unconfirmed.
    ///
    /// Why: the verified signal is the one destructive step, and the pid a
    /// connection names may belong to anything that listens there. Here it
    /// is this very test process, whose command line names no socket, so a
    /// signal would end the test. Spec: a listener in this process that
    /// never accepts gives an unconfirmed host whose reason says the
    /// process is not that socket's host, the run is not `all_stopped`, and
    /// the socket file is kept.
    #[farhelm_testtrace::test]
    async fn a_listener_that_is_not_a_host_is_not_signalled() {
        let state = state_with_hosts_dir();
        let socket = hosts_dir(state.path()).join("aaaaaaaa");
        let _listener = UnixListener::bind(&socket).unwrap();

        let report = stop_terminal_hosts_within(state.path(), short()).await;
        let [host] = report.hosts.as_slice() else {
            panic!("expected one host: {report:?}");
        };
        match &host.outcome {
            HostOutcome::Unconfirmed(why) => {
                assert!(why.contains("is not the terminal host"), "{why}");
            }
            other => panic!("expected an unconfirmed host, got {other:?}"),
        }
        assert!(!report.all_stopped());
        assert!(
            socket.exists(),
            "the socket of an unconfirmed host was removed"
        );
    }

    /// A peer that acknowledges `Shutdown`, with `Goodbye` or by closing
    /// the connection, and does not end is not stopped.
    ///
    /// Why: success must mean the host is gone, and a `Goodbye` or a closed
    /// connection only says the host meant to end (or that something else
    /// answered). Claiming a stop on either would let an uninstall remove a
    /// state directory under a running host. Spec: for a stand-in in this
    /// test process that answers `Welcome`, reads `Shutdown`, then sends
    /// `Goodbye` and closes, and for one that closes without `Goodbye`,
    /// each staying alive with its socket present: the host is unconfirmed,
    /// its reason says it took `Shutdown` but did not end and that the
    /// process is not that socket's host (so nothing was signalled, which
    /// this test still running confirms), the run is not `all_stopped`, and
    /// the socket file is kept.
    #[farhelm_testtrace::test]
    async fn an_acknowledged_shutdown_without_an_end_is_unconfirmed() {
        for goodbye in [true, false] {
            let state = state_with_hosts_dir();
            let socket = hosts_dir(state.path()).join("aaaaaaaa");
            let stand_in = protocol_stand_in(UnixListener::bind(&socket).unwrap(), goodbye);

            let report = stop_terminal_hosts_within(state.path(), short()).await;
            let _listener = stand_in
                .join()
                .expect("the stand-in saw Hello and Shutdown");
            let [host] = report.hosts.as_slice() else {
                panic!("goodbye {goodbye}: expected one host: {report:?}");
            };
            match &host.outcome {
                HostOutcome::Unconfirmed(why) => {
                    assert!(why.contains("took Shutdown but did not end"), "{why}");
                    assert!(why.contains("is not the terminal host"), "{why}");
                }
                other => panic!("goodbye {goodbye}: expected unconfirmed, got {other:?}"),
            }
            assert!(!report.all_stopped());
            assert!(
                socket.exists(),
                "goodbye {goodbye}: the socket file was removed"
            );
        }
    }

    /// A search for hosts that stalls is given up on at the budget, and the
    /// run fails.
    ///
    /// Why: the bound is the caller's (an uninstall over ssh) only
    /// protection against a stalled filesystem or process table; the
    /// search is blocking work and must not keep the run past its deadline,
    /// and a search that did not finish may have missed a host. Spec: with
    /// a search that blocks until the test ends and a 300 ms budget, the run
    /// returns within a few seconds with no hosts and a problem saying the
    /// search did not finish, and is not `all_stopped`.
    #[farhelm_testtrace::test]
    async fn a_stalled_search_is_given_up_on_at_the_budget() {
        let (release, stalled) = std::sync::mpsc::channel::<()>();
        let limits = StopLimits {
            budget: Duration::from_millis(300),
            ..short()
        };
        let started = std::time::Instant::now();
        let report = run(limits, move || {
            let _ = stalled.recv();
            Discovery::default()
        })
        .await;
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
        assert!(report.hosts.is_empty(), "{report:?}");
        assert!(
            report
                .scan_problems
                .iter()
                .any(|p| p.contains("did not finish")),
            "{report:?}"
        );
        assert!(!report.all_stopped());
        drop(release);
    }

    /// Hosts still being handled when the budget runs out are reported
    /// unconfirmed, and the run returns at the budget.
    ///
    /// Why: the budget is what bounds a caller facing a directory of hosts
    /// that never answer; a host given up on may still run, so the run must
    /// fail. Spec: with the default client deadlines (a 5 s connect) and a
    /// 1.5 s budget, a listener that never accepts is unconfirmed for the
    /// budget, and the run returns in well under the connect deadline.
    #[farhelm_testtrace::test]
    async fn the_budget_bounds_the_run() {
        let state = state_with_hosts_dir();
        let socket = hosts_dir(state.path()).join("aaaaaaaa");
        let _listener = UnixListener::bind(&socket).unwrap();

        let started = std::time::Instant::now();
        let limits = StopLimits {
            budget: Duration::from_millis(1500),
            ..StopLimits::default()
        };
        let report = stop_terminal_hosts_within(state.path(), limits).await;
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(
            report.hosts,
            vec![HostReport {
                socket,
                outcome: HostOutcome::Unconfirmed(
                    "its stop did not finish within the time budget".into()
                ),
                leftover: None,
            }],
            "{report:?}"
        );
        assert!(!report.all_stopped());
    }

    /// A host task that fails is reported as failed, not as cut off by the
    /// budget.
    ///
    /// Why: the stderr line is all a remote uninstall gets to explain a
    /// failure, and "out of time" would send whoever reads it after the
    /// wrong cause. Spec: a task that panics, collected with a generous
    /// deadline, gives an unconfirmed report for its host whose reason says
    /// its stop failed and names the panic.
    #[farhelm_testtrace::test]
    async fn a_failed_host_task_is_reported_as_failed() {
        let mut tasks = JoinSet::new();
        let task = tasks.spawn(async { panic!("deliberate test panic") });
        let socket = PathBuf::from("/nonexistent/hosts/aaaaaaaa");
        let pending = HashMap::from([(task.id(), socket.clone())]);
        let hosts = join_within(tasks, pending, Instant::now() + Duration::from_secs(10)).await;
        let [host] = hosts.as_slice() else {
            panic!("expected one report: {hosts:?}");
        };
        assert_eq!(host.socket, socket);
        match &host.outcome {
            HostOutcome::Unconfirmed(why) => {
                assert!(why.starts_with("its stop failed"), "{why}");
                assert!(why.contains("panic"), "{why}");
            }
            other => panic!("expected unconfirmed, got {other:?}"),
        }
    }

    /// The report is written when its reader reads, and given up on, within
    /// the wait, when it does not.
    ///
    /// Why: the command's bound covers its exit status reaching the caller,
    /// and a stderr pipe nobody drains would otherwise block the exit
    /// forever after every host was handled. Spec: into a buffer, two lines
    /// are written newline-terminated and the call says so; into a writer
    /// that blocks until the test ends, the call returns false within a
    /// second or two of a 200 ms wait.
    #[test]
    fn the_report_is_given_up_on_when_nobody_reads_it() {
        #[derive(Clone, Default)]
        struct Shared(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl io::Write for Shared {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        /// A writer whose every write waits until the test drops its
        /// sender, as one into a full pipe waits for its reader.
        struct Stuck(std::sync::mpsc::Receiver<()>);
        impl io::Write for Stuck {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                let _ = self.0.recv();
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let buffer = Shared::default();
        let lines = vec!["one".to_string(), "two".to_string()];
        assert!(write_lines_within(
            buffer.clone(),
            lines,
            Duration::from_secs(10)
        ));
        assert_eq!(buffer.0.lock().unwrap().as_slice(), b"one\ntwo\n");

        let (release, stuck) = std::sync::mpsc::channel();
        let started = std::time::Instant::now();
        let written = write_lines_within(
            Stuck(stuck),
            vec!["line".to_string()],
            Duration::from_millis(200),
        );
        assert!(!written);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        drop(release);
    }

    /// The budget covers the slowest host the client and the verified
    /// signal can describe, and the command's bound covers the budget and
    /// the report.
    ///
    /// Why: a budget shorter than one host's worst case would report a host
    /// unconfirmed that would have been stopped, failing uninstalls for
    /// nothing. Spec: each of these fits within [`STOP_BUDGET`]: the
    /// connect deadline plus SIGTERM's and SIGKILL's graces (a host that
    /// never answers); the connect deadline plus the refusal window (a
    /// socket that only refuses); the connect deadline, the refusal window
    /// and both graces (on macOS, a stopped host with a full backlog,
    /// refused until found by its process); the connect deadline, the
    /// shutdown deadline and [`END_WAIT`] plus both graces (a host that
    /// answers late, takes `Shutdown` and does not end); and each of the
    /// last two with a second connect deadline and refusal window in place
    /// of the graces (a signal that finds its process already gone and
    /// looks at the socket again). And [`STOP_BUDGET`] plus [`REPORT_WAIT`]
    /// is at most [`COMMAND_BUDGET`].
    #[test]
    fn the_budget_covers_one_host_worst_case() {
        use crate::hosts::process::{KILL_GRACE, TERMINATE_GRACE};
        let deadlines = ClientDeadlines::default();
        let signal = TERMINATE_GRACE + KILL_GRACE;
        assert!(deadlines.connect + signal < STOP_BUDGET);
        assert!(deadlines.connect + deadlines.refusal_window < STOP_BUDGET);
        assert!(deadlines.connect + deadlines.refusal_window + signal < STOP_BUDGET);
        assert!(deadlines.connect + deadlines.shutdown + END_WAIT + signal < STOP_BUDGET);
        let recheck = deadlines.connect + deadlines.refusal_window;
        assert!(deadlines.connect + deadlines.refusal_window + recheck < STOP_BUDGET);
        assert!(deadlines.connect + deadlines.shutdown + END_WAIT + recheck < STOP_BUDGET);
        assert!(STOP_BUDGET + REPORT_WAIT <= COMMAND_BUDGET);
    }

    /// A process that names this socket's file name in another directory
    /// and is the signal's candidate, already ended, made to stand in for
    /// the real thing: a short-lived shell whose command line ends with
    /// `--socket <elsewhere>/<name>`, pinned while it ran, then reaped.
    fn ended_candidate(elsewhere: &Path, name: &OsStr) -> HostProcess {
        let mut child = std::process::Command::new("/bin/sh")
            // `read` fails at the end of input, which is how this ends.
            .args(["-c", "read line; exit 0"])
            .arg("--socket")
            .arg(elsewhere.join(name))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("fixture: starting the candidate");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let candidate = loop {
            let argv = crate::procs::read_process_argv(child.id()).unwrap_or_default();
            if argv.last().map(Vec::as_slice) == Some(elsewhere.join(name).as_os_str().as_bytes())
                && let Some(candidate) = HostProcess::current(child.id())
            {
                break candidate;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "fixture: the candidate never ran its own command line"
            );
            // sleep-ok: polling interval for the candidate's exec.
            std::thread::sleep(Duration::from_millis(10));
        };
        drop(child.stdin.take());
        assert!(
            child.wait().unwrap().success(),
            "fixture: the candidate did not end cleanly"
        );
        assert_eq!(
            candidate.is_running(),
            Ok(false),
            "premise: the candidate is gone"
        );
        candidate
    }

    /// The target the search would build for `name` in `hosts`.
    fn target_in(hosts: &Path, name: &str) -> Target {
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let handle = File::from(rustix::fs::open(hosts, flags, Mode::empty()).unwrap());
        Target {
            path: hosts.join(name),
            name: OsString::from(name),
            dir: Arc::new(handle),
        }
    }

    /// A signal candidate that turns out already gone confirms the stop only
    /// once the socket is gone too.
    ///
    /// Why: when no connection names a peer, the candidate comes from the
    /// client's process-table search, which matches any process naming an
    /// absolute socket path with this file name in any directory. One such
    /// unrelated process ending (before the signal's checks reach it) says
    /// nothing about the host, and counting it as the host's end would
    /// remove a live host's socket and let the run exit 0. Spec: with a
    /// candidate whose command line named another directory's socket of
    /// the same name and that has ended (premise checked): against a socket
    /// a listener in this process still holds (and never accepts), the
    /// outcome is unconfirmed, says the process ended by itself, the run's
    /// report is not `all_stopped`, and the socket file is kept; against a
    /// socket whose listener is gone (a stale file), the outcome is stopped
    /// (`AlreadyGone`) and the stale file is removed.
    #[farhelm_testtrace::test]
    async fn an_ended_candidate_confirms_nothing_while_the_socket_is_served() {
        let state = state_with_hosts_dir();
        let hosts = hosts_dir(state.path());
        let elsewhere = tempfile::tempdir().unwrap();
        let name = crate::hosts::HostName::random().unwrap().to_string();
        let candidate = ended_candidate(elsewhere.path(), OsStr::new(&name));

        let served = target_in(&hosts, &name);
        let _listener = UnixListener::bind(&served.path).unwrap();
        let outcome = signal(&candidate, &served, "stand-in", short().client).await;
        match &outcome {
            HostOutcome::Unconfirmed(why) => assert!(why.contains("ended by itself"), "{why}"),
            other => panic!("expected unconfirmed, got {other:?}"),
        }
        let report = StopReport {
            hosts: vec![finish(served.clone(), outcome)],
            scan_problems: Vec::new(),
        };
        assert!(!report.all_stopped());
        assert!(served.path.exists(), "the served socket file was removed");
        drop(_listener);
        std::fs::remove_file(&served.path).unwrap();

        let stale_name = crate::hosts::HostName::random().unwrap().to_string();
        let candidate = ended_candidate(elsewhere.path(), OsStr::new(&stale_name));
        let stale = target_in(&hosts, &stale_name);
        drop(UnixListener::bind(&stale.path).unwrap());
        let outcome = signal(&candidate, &stale, "stand-in", short().client).await;
        assert_eq!(
            outcome,
            HostOutcome::Stopped(Stopped::Signalled(Termination::AlreadyGone))
        );
        let report = finish(stale.clone(), outcome);
        assert_eq!(report.leftover, None);
        assert!(!stale.path.exists(), "the stale socket file was left");
    }
}
