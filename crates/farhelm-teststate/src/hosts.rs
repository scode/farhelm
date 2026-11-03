//! Ending the terminal hosts a test run leaves in its state directories.
//!
//! A terminal host is daemonized (its own session, re-parented away from
//! whoever started it) and has no idle timeout, so a host started by a test
//! outlives the test, the test process and its state directory unless
//! something ends it. Removing the directory is worse than useless: the host
//! keeps running with its socket gone, and nothing can reach it to end it
//! any more. So every cleanup of an owned state directory ends the hosts in
//! it first, and keeps the directory when it cannot confirm that every one of
//! them ended, so that a later sweep can retry through the socket.
//!
//! The client here is deliberately tiny and blocking (it runs from `Drop`
//! and from the sweep, where there is no async runtime): connect, `Hello`
//! with take-over, `Shutdown`, then wait for the end of the stream and for
//! the host's socket file to go (a host removes it as it ends), all within
//! [`HOST_STOP_DEADLINE`]. It speaks through the host crate's own codec, so
//! its bytes are the protocol's. A socket that only refuses connections is
//! judged stale (its host killed) after [`STALE_REFUSAL_WINDOW`] of nothing
//! but refusals, never sooner: on macOS a live host whose listen backlog is
//! full refuses too. And a socket is never judged gone, stale or missing,
//! while a live process of this user still serves it by its command line
//! (`... --socket <that path>`, the host's own): a stopped or wedged host
//! whose backlog filled refuses for as long as it likes, and one whose
//! socket file went missing cannot be reached at all.
//!
//! Anything short of that keeps the directory: a scan that could not read
//! part of the tree, a host that did not answer, a socket that stayed.
//!
//! Hosts are found by place, not by process: any socket directly inside a
//! directory named [`HOSTS_DIR_NAME`] within the walked tree. Nothing outside
//! a directory the run owns is ever contacted, and no process is ever
//! signalled; other agents run tests with hosts of their own on the same
//! machine.

use std::fs;
use std::io::{self, Read as _, Write as _};
use std::os::unix::fs::FileTypeExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use terminal_host::protocol::{ClientMessage, PROTOCOL_VERSION, encode_client};

/// The name of a supervisor's host socket directory inside its state
/// directory. The supervisor's own constant (`farhelm_supervisor::hosts`) is
/// pinned equal to this by a test there; this crate cannot depend on the
/// supervisor.
pub const HOSTS_DIR_NAME: &str = "hosts";

/// How long one host gets to end before its cleanup counts as unconfirmed.
/// A healthy host ends as soon as its `Goodbye` is written (the host's own
/// bound on that is one second), so this is room for a loaded machine, not
/// for a host that is merely slow: a stopped or wedged host never answers.
pub const HOST_STOP_DEADLINE: Duration = Duration::from_secs(5);

/// How deep below a run's directory host sockets are looked for. Depth 3
/// covers everything the harnesses build: a Rust test's state directory is
/// the `d` inside its container, so its sockets are at `d/hosts/` and a
/// nested remote state's at `d/remote/hosts/`; the stack script's are at
/// `hosts/` and `remote/hosts/`.
pub const HOST_SOCKET_WALK_DEPTH: usize = 3;

/// How long a socket must refuse every connection, without one accepted or
/// any other answer in between, before it counts as stale (left by a killed
/// host). On Linux one refusal would do, since a refusal means nothing
/// listens; on macOS a live host whose listen backlog is full refuses as
/// well, so the goal's "gone only after repeated attempts over a few
/// seconds" applies. It must stay below [`HOST_STOP_DEADLINE`].
///
/// Refusals alone cannot tell a dead host from a stopped one whose backlog
/// is full: every connection that times out against a host that does not
/// accept (this cleanup's own included) stays queued until the host accepts
/// it, so a host stopped long enough fills its backlog. So a socket that
/// refused for this long still counts as gone only when no process of this
/// user serves it ([`serving_process`]).
pub const STALE_REFUSAL_WINDOW: Duration = Duration::from_secs(3);

/// Pause between connection attempts, and between looks at an ending
/// host's socket file.
const RETRY_INTERVAL: Duration = Duration::from_millis(100);

/// How one host's cleanup ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostStop {
    /// It took the `Shutdown`: it closed the connection, and its socket
    /// file went, which a host removes as it ends.
    Ended,
    /// Nothing was listening: the socket was already gone, or was a stale
    /// file (left by a host that was killed), which this removed.
    Gone,
    /// Neither could be confirmed by the deadline. The reason is for the
    /// diagnostic line.
    Unconfirmed(String),
}

/// End every host whose socket lies directly inside a [`HOSTS_DIR_NAME`]
/// directory within `depth` levels below `dir`, and say whether every one of
/// them was confirmed ended (or already gone).
///
/// Each host gets [`HOST_STOP_DEADLINE`]. Once `budget` has passed, the
/// remaining hosts are not contacted and count as unconfirmed: the budget
/// bounds a directory full of wedged hosts as a whole. Unconfirmed hosts are
/// named on stderr.
///
/// A scan that could not read part of the tree (anything but a directory
/// that vanished meanwhile) is not confirmed either, though the hosts it did
/// find are still ended: a host in the part it missed would outlive the
/// directory's removal.
pub fn stop_hosts_within(dir: &Path, depth: usize, budget: Instant) -> bool {
    let mut sockets = Vec::new();
    let mut confirmed = collect_host_sockets(dir, depth, &mut sockets);
    for socket in sockets {
        if Instant::now() >= budget {
            crate::diag(format!(
                "terminal host cleanup budget exhausted; not contacting {}",
                socket.display()
            ));
            confirmed = false;
            continue;
        }
        match stop_host(&socket, Instant::now() + HOST_STOP_DEADLINE) {
            HostStop::Ended | HostStop::Gone => {}
            HostStop::Unconfirmed(why) => {
                crate::diag(format!(
                    "could not confirm the terminal host at {} ended: {why}",
                    socket.display()
                ));
                confirmed = false;
            }
        }
    }
    confirmed
}

/// Ask the host listening on `socket` to shut down, and wait, until
/// `deadline`, for it to close the connection and stop listening.
///
/// `Hello` takes over from any client the host has, so a host whose
/// supervisor is still connected (or died connected) ends too. "Ended" needs
/// two observations: the end of this connection's stream, and then the
/// socket file gone. The end of a stream alone could be a host refusing the
/// connection, or something that is not a host; a refused reconnect could be
/// a full backlog. Only a host that ends removes its own socket file.
pub fn stop_host(socket: &Path, deadline: Instant) -> HostStop {
    let mut stream = match connect(socket, deadline) {
        Connected::Stream(stream) => stream,
        Connected::Gone => return HostStop::Gone,
        Connected::Unconfirmed(why) => return HostStop::Unconfirmed(why),
    };
    if let Err(why) = shut_down(&mut stream, deadline) {
        return HostStop::Unconfirmed(why);
    }
    drop(stream);
    // Looked at, not connected to: a connection here would queue on a host
    // that is not accepting, and one more refusal proves nothing.
    loop {
        if matches!(
            fs::symlink_metadata(socket),
            Err(error) if error.kind() == io::ErrorKind::NotFound
        ) {
            return HostStop::Ended;
        }
        if Instant::now() >= deadline {
            return HostStop::Unconfirmed(
                "it closed the connection but its socket file is still there".into(),
            );
        }
        // sleep-ok: polling interval while an ending host removes its socket; bounded by the deadline above.
        std::thread::sleep(RETRY_INTERVAL);
    }
}

/// What connecting to a host socket found.
enum Connected {
    Stream(std::os::unix::net::UnixStream),
    Gone,
    Unconfirmed(String),
}

/// Connect without ever blocking in `connect` (a full backlog must not hang
/// a cleanup), retrying until `deadline`. A missing file is gone at once;
/// refusals are believed only once they have lasted
/// [`STALE_REFUSAL_WINDOW`] without a break, and then the stale file is
/// removed. Either way, only once no process serves the socket.
fn connect(socket: &Path, deadline: Instant) -> Connected {
    let mut refusing_since: Option<Instant> = None;
    loop {
        let last = match crate::connect_without_waiting(socket) {
            Ok(stream) => return Connected::Stream(stream),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return confirm_gone(socket, false);
            }
            Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                let since = *refusing_since.get_or_insert_with(Instant::now);
                if since.elapsed() >= STALE_REFUSAL_WINDOW {
                    return confirm_gone(socket, true);
                }
                error
            }
            // A full backlog (EAGAIN on Linux), or anything unexpected: not
            // evidence either way, so retry until the deadline.
            Err(error) => {
                refusing_since = None;
                error
            }
        };
        if Instant::now() >= deadline {
            return Connected::Unconfirmed(format!("connecting: {last}"));
        }
        // sleep-ok: interval between connection attempts to a host socket; bounded by the deadline above.
        std::thread::sleep(RETRY_INTERVAL);
    }
}

/// Judge a socket that takes no connection gone, removing it first when it
/// is a stale file, unless a process still serves it.
fn confirm_gone(socket: &Path, stale: bool) -> Connected {
    match serving_process(socket) {
        Ok(None) => {
            if stale {
                remove_stale_socket(socket);
            }
            Connected::Gone
        }
        Ok(Some(pid)) => Connected::Unconfirmed(format!(
            "it takes no connection, but process {pid} still serves it"
        )),
        Err(why) => Connected::Unconfirmed(format!(
            "it takes no connection, and whether a process serves it is unknown: {why}"
        )),
    }
}

/// A live process of this user whose command line ends with
/// `--socket <socket>`, as a host's does, if there is one.
///
/// Linux reads `/proc/<pid>/cmdline` of this user's processes (a zombie's
/// is empty, so it never matches). macOS has no `/proc`, so it reads `ps`,
/// whose command column joins the arguments with spaces; that is exact
/// enough here, since test socket paths contain no spaces and a match only
/// ever keeps a directory. Any failure to look is an error: the caller then
/// keeps the directory.
fn serving_process(socket: &Path) -> Result<Option<u32>, String> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt as _;
        use std::os::unix::fs::MetadataExt as _;
        // SAFETY: geteuid has no preconditions.
        let me = unsafe { libc::geteuid() };
        let entries = fs::read_dir("/proc").map_err(|error| format!("listing /proc: {error}"))?;
        let wanted = socket.as_os_str().as_bytes();
        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u32>().ok())
            else {
                continue;
            };
            if !entry.metadata().is_ok_and(|meta| meta.uid() == me) {
                continue;
            }
            let Ok(cmdline) = fs::read(entry.path().join("cmdline")) else {
                continue;
            };
            let args: Vec<&[u8]> = cmdline
                .strip_suffix(b"\0")
                .unwrap_or(&cmdline)
                .split(|&b| b == 0)
                .collect();
            if let [.., flag, path] = args.as_slice()
                && *flag == b"--socket"
                && *path == wanted
            {
                return Ok(Some(pid));
            }
        }
        Ok(None)
    }
    #[cfg(not(target_os = "linux"))]
    {
        use crate::process::{CommandRunLimits, run_bounded};
        // SAFETY: geteuid has no preconditions.
        let me = unsafe { libc::geteuid() }.to_string();
        let limits = CommandRunLimits::new(
            Duration::from_secs(5),
            Duration::from_millis(500),
            8 * 1024 * 1024,
            64 * 1024,
            64 * 1024,
        )
        .map_err(|error| format!("{error:?}"))?;
        let outcome = run_bounded(
            std::process::Command::new("ps").args(["-A", "-ww", "-o", "pid=,uid=,command="]),
            &limits,
        )
        .map_err(|error| format!("running ps: {error:?}"))?;
        if !outcome.status.is_some_and(|status| status.success()) || !outcome.stdout.complete {
            return Err("ps did not list the processes".into());
        }
        let suffix = format!(" --socket {}", socket.display());
        for line in String::from_utf8_lossy(&outcome.stdout.prefix).lines() {
            let mut fields = line.split_whitespace();
            let (Some(pid), Some(uid)) = (fields.next(), fields.next()) else {
                continue;
            };
            let command = fields.collect::<Vec<_>>().join(" ");
            // A zombie's command is shown as `<defunct>`, so it never
            // matches.
            if uid == me
                && command.ends_with(&suffix)
                && let Ok(pid) = pid.parse()
            {
                return Ok(Some(pid));
            }
        }
        Ok(None)
    }
}

/// Send `Hello` (with take-over) and `Shutdown`, then read and discard until
/// the host closes the connection. `Err` says why that was not seen by the
/// deadline.
fn shut_down(stream: &mut std::os::unix::net::UnixStream, deadline: Instant) -> Result<(), String> {
    let remaining = || deadline.saturating_duration_since(Instant::now());
    stream
        .set_nonblocking(false)
        .map_err(|error| format!("making the connection blocking: {error}"))?;
    // A zero timeout means "none" to the socket API, so a deadline already
    // passed still gets a moment rather than an unbounded wait.
    let floor = |left: Duration| left.max(Duration::from_millis(1));
    set_timeout(stream, Direction::Write, floor(remaining()))?;
    let mut request = encode_client(&ClientMessage::Hello {
        version: PROTOCOL_VERSION,
        take_over: true,
    });
    request.extend(encode_client(&ClientMessage::Shutdown));
    match stream.write_all(&request) {
        Ok(()) => {}
        // The connection is already closed: the host is ending (or refused
        // it). What follows decides which.
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
            ) =>
        {
            return Ok(());
        }
        Err(error) => return Err(format!("sending Shutdown: {error}")),
    }
    let mut buf = [0u8; 4096];
    loop {
        let left = remaining();
        if left.is_zero() {
            return Err("no end of stream after Shutdown".into());
        }
        set_timeout(stream, Direction::Read, floor(left))?;
        match stream.read(&mut buf) {
            // Everything before the end (`Welcome`, `Goodbye`) is discarded:
            // only the close itself tells that the host acted.
            Ok(0) => return Ok(()),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => return Ok(()),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(format!("reading the reply: {error}")),
        }
    }
}

/// Which of a connection's two socket timeouts [`set_timeout`] sets.
#[derive(Clone, Copy)]
enum Direction {
    Read,
    Write,
}

/// Set one socket timeout on `stream`, treating macOS's refusal on a closed
/// connection as success.
///
/// macOS rejects socket options with `EINVAL` once the peer has closed the
/// connection, and a closed connection is exactly what [`shut_down`] waits
/// for: a host closes right after its `Goodbye`, so the next timeout set
/// fails the moment the host has done what was asked. Leaving the timeout
/// as it was is safe then, because the read or write that follows returns at
/// once with the end of stream or a broken pipe. Linux sets options on a
/// closed connection without complaint, so this only matters on macOS.
fn set_timeout(
    stream: &std::os::unix::net::UnixStream,
    direction: Direction,
    timeout: Duration,
) -> Result<(), String> {
    let result = match direction {
        Direction::Read => stream.set_read_timeout(Some(timeout)),
        Direction::Write => stream.set_write_timeout(Some(timeout)),
    };
    match result {
        Ok(()) => Ok(()),
        Err(error) if cfg!(target_os = "macos") && error.raw_os_error() == Some(libc::EINVAL) => {
            Ok(())
        }
        Err(error) => Err(format!("setting a socket timeout: {error}")),
    }
}

/// Remove a socket file nothing listens on any more, if it is still a
/// socket. A failure is only a diagnostic: the file holds no process, and the
/// directory's removal takes it anyway.
fn remove_stale_socket(socket: &Path) {
    let is_socket = fs::symlink_metadata(socket).is_ok_and(|meta| meta.file_type().is_socket());
    if !is_socket {
        return;
    }
    if let Err(error) = fs::remove_file(socket)
        && error.kind() != io::ErrorKind::NotFound
    {
        crate::diag(format!(
            "cannot remove stale host socket {}: {error}",
            socket.display()
        ));
    }
}

/// Collect every socket directly inside a directory named
/// [`HOSTS_DIR_NAME`] under `dir`, recursing into at most `depth` nested
/// directory levels (a socket directory at the recursion floor is still
/// read) and never following symlinks: inside a directory about to be
/// deleted, a symlink is the one thing that could point the cleanup at
/// another run's hosts.
///
/// Returns whether the scan was complete: false after any failure to read a
/// directory, an entry or its type, other than something that vanished
/// meanwhile (which holds no host).
fn collect_host_sockets(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) -> bool {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return true,
        Err(error) => {
            crate::diag(format!("cannot walk {}: {error}", dir.display()));
            return false;
        }
    };
    let mut complete = true;
    let in_hosts_dir = dir.file_name() == Some(HOSTS_DIR_NAME.as_ref());
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                crate::diag(format!("error listing {}: {error}", dir.display()));
                complete = false;
                continue;
            }
        };
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                crate::diag(format!("cannot stat {}: {error}", path.display()));
                complete = false;
                continue;
            }
        };
        if meta.is_dir() {
            if depth > 0 {
                complete &= collect_host_sockets(&path, depth - 1, out);
            }
        } else if in_hosts_dir && meta.file_type().is_socket() {
            out.push(path);
        }
    }
    complete
}

#[cfg(test)]
mod tests {
    //! The client against stand-in listeners, which pin each outcome
    //! without a real host. Real hosts, started by the real `farhelm`, are
    //! ended by this code in `crates/farhelm/tests/terminal_host_spawn.rs`.

    use super::*;
    use std::os::unix::net::UnixListener;
    use terminal_host::protocol::{
        HostMessage, MAX_FRAME, decode_client, encode_host, split_frame,
    };

    /// A listener at `socket` that behaves as a host does on `Shutdown`: it
    /// answers `Welcome` and `Goodbye`, removes its socket file, and closes.
    /// It checks what it received is exactly `Hello` (take-over) then
    /// `Shutdown`, and reports that through the returned handle.
    fn fake_host(socket: &Path) -> std::thread::JoinHandle<Vec<ClientMessage>> {
        let listener = UnixListener::bind(socket).unwrap();
        let socket = socket.to_path_buf();
        std::thread::spawn(move || {
            // Bounded, so a client that never connects fails the test
            // rather than hanging it.
            listener.set_nonblocking(true).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut conn = loop {
                match listener.accept() {
                    Ok((conn, _)) => break conn,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "nothing connected");
                        // sleep-ok: polling interval of a stand-in listener waiting for the client under test.
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("accepting: {error}"),
                }
            };
            conn.set_nonblocking(false).unwrap();
            conn.set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut buf = Vec::new();
            let mut received = Vec::new();
            while received.len() < 2 {
                let mut chunk = [0u8; 256];
                let n = conn.read(&mut chunk).unwrap();
                assert!(n > 0, "the client closed before Shutdown");
                buf.extend_from_slice(&chunk[..n]);
                while let Some(frame) = split_frame(&buf, MAX_FRAME).unwrap() {
                    received.push(decode_client(frame, 1).unwrap());
                    let consumed = frame.consumed;
                    buf.drain(..consumed);
                }
            }
            conn.write_all(&encode_host(&HostMessage::Welcome { version: 1 }))
                .unwrap();
            conn.write_all(&encode_host(&HostMessage::Goodbye)).unwrap();
            fs::remove_file(&socket).unwrap();
            drop(listener);
            received
        })
    }

    /// The client sends exactly `Hello` with take-over and `Shutdown`, and a
    /// host that answers, closes and removes its socket counts as ended.
    ///
    /// Why: take-over is what ends a host whose dead supervisor still holds
    /// the connection, and "ended" is what lets the sweep delete the
    /// directory. Spec: the stand-in receives `Hello { version: 1,
    /// take_over: true }` then `Shutdown`, the result is `Ended`, and the
    /// socket is gone.
    #[test]
    fn a_host_that_shuts_down_is_ended() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        let host = fake_host(&socket);
        let result = stop_host(&socket, Instant::now() + Duration::from_secs(10));
        let received = host.join().unwrap();
        assert_eq!(
            received,
            vec![
                ClientMessage::Hello {
                    version: 1,
                    take_over: true
                },
                ClientMessage::Shutdown
            ]
        );
        assert_eq!(result, HostStop::Ended);
        assert!(!socket.exists());
    }

    /// A socket nothing listens on is gone once it has refused for the
    /// whole stale window, and its file is removed; a missing socket is gone
    /// at once.
    ///
    /// Why: a host killed with SIGKILL leaves its socket file behind, and a
    /// run whose hosts are all dead must still be reclaimed; but a refusal
    /// is believed only over time, because on macOS a live host with a full
    /// backlog refuses too. Spec: a socket file whose listener was closed
    /// gives `Gone` no sooner than [`STALE_REFUSAL_WINDOW`] and no longer
    /// exists; a path with nothing at it gives `Gone`.
    #[test]
    fn a_stale_socket_is_gone_and_removed_after_the_window() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        drop(UnixListener::bind(&socket).unwrap());
        assert!(socket.exists(), "premise: the stale file remains");
        let started = Instant::now();
        let deadline = started + Duration::from_secs(10);
        assert_eq!(stop_host(&socket, deadline), HostStop::Gone);
        assert!(started.elapsed() >= STALE_REFUSAL_WINDOW);
        assert!(!socket.exists());
        assert_eq!(stop_host(&socket, deadline), HostStop::Gone);
    }

    /// Refusals for less than the stale window are not evidence: the socket
    /// is kept and the host unconfirmed.
    ///
    /// Why: a stopped or busy macOS host whose backlog is full refuses every
    /// connection while it lives, and unlinking its socket after a few quick
    /// refusals would strand it for good. Spec: a refusing socket with a
    /// deadline of 1 s (shorter than the window) gives `Unconfirmed`, and the
    /// socket file still exists.
    #[test]
    fn brief_refusals_keep_the_socket() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        drop(UnixListener::bind(&socket).unwrap());
        let result = stop_host(&socket, Instant::now() + Duration::from_secs(1));
        assert!(matches!(result, HostStop::Unconfirmed(_)), "{result:?}");
        assert!(socket.exists(), "the socket was removed on brief refusals");
    }

    /// A listener that accepts but never answers is unconfirmed, by the
    /// deadline, and keeps its socket.
    ///
    /// Why: a stopped or wedged host is still running; removing its socket
    /// would leave it unreachable for good, and a cleanup that waited for it
    /// would hang. Spec: against a bound listener that never accepts, with a
    /// 300 ms deadline, the result is `Unconfirmed` within a few seconds and
    /// the socket file still exists.
    #[test]
    fn a_listener_that_never_answers_is_unconfirmed() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        let _listener = UnixListener::bind(&socket).unwrap();
        let started = Instant::now();
        let result = stop_host(&socket, Instant::now() + Duration::from_millis(300));
        assert!(matches!(result, HostStop::Unconfirmed(_)), "{result:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(socket.exists());
    }

    /// A listener that closes the connection but keeps its socket file is
    /// unconfirmed.
    ///
    /// Why: an end of stream alone is also what a host refusing a
    /// connection, or something that is not a host, produces; only the
    /// socket file going, which a host does as it ends, shows a host ended.
    /// Spec: a stand-in that accepts and closes every connection while
    /// staying bound gives `Unconfirmed` by a 500 ms deadline, and its socket
    /// remains.
    #[test]
    fn a_listener_that_stays_after_closing_is_unconfirmed() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let closer = {
            let stop = stop.clone();
            std::thread::spawn(move || {
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((conn, _)) => drop(conn),
                        // sleep-ok: polling interval of a stand-in listener the test stops below.
                        Err(_) => std::thread::sleep(Duration::from_millis(5)),
                    }
                }
            })
        };
        let result = stop_host(&socket, Instant::now() + Duration::from_millis(500));
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        closer.join().unwrap();
        assert!(matches!(result, HostStop::Unconfirmed(_)), "{result:?}");
        assert!(socket.exists());
    }

    /// The walk finds sockets directly inside every `hosts` directory within
    /// its depth, and nothing else.
    ///
    /// Why: a socket the walk misses is a host that outlives its directory's
    /// removal, unreachable for good; one it wrongly includes is a stranger's
    /// socket sent a `Shutdown`. Spec: sockets in `hosts/`, `d/hosts/` and
    /// `d/remote/hosts/` (the deepest the harnesses use, at the depth bound)
    /// are found; a socket in `a/b/c/hosts/` (one level past the bound), a
    /// socket outside any `hosts` directory, a regular file inside one, a
    /// socket nested below a `hosts` directory, and a socket reached only
    /// through a symlink are not.
    #[test]
    fn the_walk_finds_sockets_in_hosts_directories_within_its_depth() {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        let mut listeners = Vec::new();
        let mut bind = |path: PathBuf| {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            listeners.push(UnixListener::bind(&path).unwrap());
            path
        };
        let mut expected = vec![
            bind(r.join("hosts/aaaaaaaa")),
            bind(r.join("d/hosts/bbbbbbbb")),
            bind(r.join("d/remote/hosts/cccccccc")),
        ];
        bind(r.join("a/b/c/hosts/dddddddd"));
        bind(r.join("d/tmux.sock"));
        bind(r.join("d/hosts/nested/eeeeeeee"));
        fs::write(r.join("d/hosts/not-a-socket"), b"").unwrap();
        let outside = tempfile::tempdir().unwrap();
        bind(outside.path().join("hosts/ffffffff"));
        std::os::unix::fs::symlink(outside.path(), r.join("escape")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("hosts"), r.join("d/remote/elsewhere"))
            .unwrap();

        let mut found = Vec::new();
        assert!(collect_host_sockets(r, HOST_SOCKET_WALK_DEPTH, &mut found));
        found.sort();
        expected.sort();
        assert_eq!(found, expected);
    }

    /// A scan that cannot read part of the tree is not confirmed, though the
    /// hosts it found are still ended.
    ///
    /// Why: a host in a directory the scan could not read would never be
    /// contacted, and removing the run would strand it; the directory must
    /// stay for a later sweep. Spec: with an unreadable directory (mode 000)
    /// beside a `hosts` directory holding one stand-in host, the walk reports
    /// itself incomplete, `stop_hosts_within` reports not confirmed, and the
    /// stand-in host was still asked to shut down (it got `Shutdown`).
    #[test]
    fn an_incomplete_scan_is_unconfirmed() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(HOSTS_DIR_NAME)).unwrap();
        let host = fake_host(&root.path().join("hosts/aaaaaaaa"));
        let locked = root.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let unreadable = fs::read_dir(&locked).is_err();
        let mut found = Vec::new();
        let complete = collect_host_sockets(root.path(), HOST_SOCKET_WALK_DEPTH, &mut found);
        let confirmed = stop_hosts_within(
            root.path(),
            HOST_SOCKET_WALK_DEPTH,
            Instant::now() + Duration::from_secs(30),
        );
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(unreadable, "premise: the directory cannot be read");
        assert!(!complete, "the walk reported itself complete");
        assert!(!confirmed, "an incomplete scan was confirmed");
        assert!(host.join().unwrap().contains(&ClientMessage::Shutdown));
    }

    /// Hosts past the cleanup budget are not contacted and make the cleanup
    /// unconfirmed.
    ///
    /// Why: the budget is what bounds a directory full of wedged hosts; a
    /// host skipped for it is still running, so its directory must stay.
    /// Spec: with a budget already spent, a directory holding one stand-in
    /// listener reports not confirmed, and the listener was never connected
    /// to (its accept queue is empty).
    #[test]
    fn hosts_past_the_budget_are_unconfirmed_and_untouched() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(HOSTS_DIR_NAME)).unwrap();
        let listener = UnixListener::bind(root.path().join("hosts/aaaaaaaa")).unwrap();
        listener.set_nonblocking(true).unwrap();
        assert!(!stop_hosts_within(
            root.path(),
            HOST_SOCKET_WALK_DEPTH,
            Instant::now()
        ));
        assert_eq!(
            listener.accept().map_err(|error| error.kind()).err(),
            Some(io::ErrorKind::WouldBlock),
            "a skipped host must not be contacted"
        );
    }
}
