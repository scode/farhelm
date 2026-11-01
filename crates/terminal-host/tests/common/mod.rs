//! A minimal client for the integration tests: start a real host (the
//! crate's own binary) on a real pty, connect to its socket, and speak the
//! protocol through the crate's codec.
//!
//! Every wait is bounded, as a whole and not only per message (a host that
//! keeps sending something other than what a test waits for fails the test
//! instead of holding it), and so is the output a helper keeps while it
//! waits. Every host a test starts is ended when its handle drops,
//! confirmed by its pid, so a failing test leaves no process behind (or
//! says on stderr which one it could not end).

#![allow(dead_code)]

use std::io::{Read as _, Write as _};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use terminal_host::protocol::{
    ChildExit, ClientMessage, HostMessage, Info, MAX_FRAME, Position, Size, Spawn, decode_host,
    encode_client, encode_spawn, split_frame,
};

/// How long any single wait in these tests may take before it fails.
pub const WAIT: Duration = Duration::from_secs(10);

/// The most output a helper keeps while it waits: far more than any test's
/// program prints, and little enough that a host streaming without end
/// fails the wait rather than growing the test without bound.
pub const MAX_RETAINED: usize = 16 << 20;

/// A child spec running `argv` in `/` with a small environment.
pub fn spec(argv: &[&str]) -> Spawn {
    Spawn {
        argv: argv.iter().map(|arg| arg.as_bytes().to_vec()).collect(),
        cwd: b"/".to_vec(),
        env: vec![
            b"PATH=/usr/bin:/bin".to_vec(),
            b"TERM=xterm-256color".to_vec(),
            b"LC_ALL=C".to_vec(),
        ],
        size: Size { cols: 80, rows: 24 },
        journal_cap: 1024 * 1024,
        tag: b"test tag".to_vec(),
        initial_snapshot: None,
    }
}

/// A running host started by a test, ended when its handle drops.
///
/// Dropping asks the host to `Shutdown` (taking over from any client) and
/// waits for its pid to be gone; a host that does not end that way, because
/// its loop is broken or it no longer serves, is killed with SIGKILL and
/// waited for again. Only then is the socket directory removed. One that
/// cannot be confirmed gone keeps its directory, and its pid and socket are
/// printed on stderr, so it can still be found.
pub struct Host {
    /// `None` only while dropping.
    dir: Option<tempfile::TempDir>,
    pub socket: PathBuf,
    /// The host process, from the listening socket's credentials: known
    /// from the start without the host's loop having to answer anything.
    pub pid: u32,
}

/// How starting a host went.
pub struct Started {
    pub status: i32,
    pub stderr: String,
}

/// Run the host binary with `spec` on stdin, returning how its first stage
/// exited. The socket lives in a fresh private directory; `{dir}` in `args`
/// names it.
pub fn start_raw(spec_bytes: &[u8], args: &[&str]) -> (tempfile::TempDir, Started) {
    start_raw_with(spec_bytes, args, |_| {})
}

/// [`start_raw`], with `configure` applied to the starter's command first:
/// how a test gives the host a starter in some unusual state (a `pre_exec`
/// that blocks signals, say) without changing its own process.
pub fn start_raw_with(
    spec_bytes: &[u8],
    args: &[&str],
    configure: impl FnOnce(&mut Command),
) -> (tempfile::TempDir, Started) {
    let dir = tempfile::tempdir().expect("fixture: socket directory");
    let mut command = Command::new(env!("CARGO_BIN_EXE_terminal-host"));
    for arg in args {
        command.arg(arg.replace("{dir}", &dir.path().to_string_lossy()));
    }
    configure(&mut command);
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("fixture: spawning the host");
    // A host refusing its command line exits without reading the spec, so a
    // broken pipe here is an outcome the caller asserts on, not a failure.
    match child.stdin.take().unwrap().write_all(spec_bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {}
        Err(error) => panic!("fixture: writing the child spec: {error}"),
    }
    let output = child
        .wait_with_output()
        .expect("fixture: waiting for the first stage");
    (
        dir,
        Started {
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        },
    )
}

/// Start a host for `spec` and require that it started.
pub fn start(spec: &Spawn) -> Host {
    start_with(spec, |_| {})
}

/// [`start`], with `configure` applied to the starter's command (see
/// [`start_raw_with`]).
pub fn start_with(spec: &Spawn, configure: impl FnOnce(&mut Command)) -> Host {
    let (dir, started) = start_raw_with(&encode_spawn(spec), &["--socket", "{dir}/h"], configure);
    assert_eq!(started.status, 0, "the host started: {}", started.stderr);
    let socket = dir.path().join("h");
    Host::adopt(dir, socket)
}

/// The pid of the process listening on the other end of `stream`. A
/// connection's peer credentials are the listener's, taken when it called
/// `listen`, and the kernel queues a connection whether or not the host
/// ever accepts it, so this works for a host whose loop is broken. (The
/// host then sees a connection that closes before its `Hello`, which it
/// drops.)
/// `None` when the credentials cannot be read or name no plausible process
/// (a pid of 0 or 1 would make a signal hit this test's own process group or
/// init).
fn peer_pid(stream: &UnixStream) -> Option<u32> {
    use std::os::fd::AsRawFd as _;
    let fd = stream.as_raw_fd();
    #[cfg(target_os = "linux")]
    let pid = {
        // SAFETY: ucred is plain data; SO_PEERCRED fills one of this size.
        let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut cred as *mut libc::ucred).cast(),
                &mut len,
            )
        };
        if result != 0 {
            return None;
        }
        cred.pid
    };
    #[cfg(target_os = "macos")]
    let pid = {
        let mut pid: libc::pid_t = 0;
        let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
        // SAFETY: LOCAL_PEERPID fills one pid_t.
        let result = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_LOCAL,
                libc::LOCAL_PEERPID,
                (&mut pid as *mut libc::pid_t).cast(),
                &mut len,
            )
        };
        if result != 0 {
            return None;
        }
        pid
    };
    (pid > 1).then_some(pid as u32)
}

impl Host {
    /// Take charge of the host listening on `socket` in `dir`, which a test
    /// started its own way.
    pub fn adopt(dir: tempfile::TempDir, socket: PathBuf) -> Host {
        let stream = UnixStream::connect(&socket).expect("fixture: connecting to the host");
        let pid = peer_pid(&stream).expect("fixture: the host's pid from its socket");
        Host {
            dir: Some(dir),
            socket,
            pid,
        }
    }

    /// The directory holding the socket, which the test may use for files
    /// of its own.
    pub fn dir(&self) -> &Path {
        self.dir.as_ref().expect("not dropped").path()
    }

    /// Connect and complete the hello.
    pub fn connect(&self, take_over: bool) -> Client {
        let mut client = Client::raw(&self.socket);
        client.send(&ClientMessage::Hello {
            version: 1,
            take_over,
        });
        let welcome = client.recv();
        assert_eq!(welcome, HostMessage::Welcome { version: 1 });
        client
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        // A host still alive still listens, whatever its loop does; one
        // that is gone (a test ended it) leaves no listener. Reaching the
        // pid through the socket is also what makes it this fixture's host
        // and not some process that reused the pid since.
        let listening = UnixStream::connect(&self.socket)
            .ok()
            .filter(|stream| peer_pid(stream) == Some(self.pid));
        let mut ended = true;
        if let Some(mut stream) = listening {
            let mut bytes = encode_client(&ClientMessage::Hello {
                version: 1,
                take_over: true,
            });
            bytes.extend(encode_client(&ClientMessage::Shutdown));
            let _ = stream.write_all(&bytes);
            // A pid that never stopped running since the check above is
            // still that host: SIGKILL cannot hit anyone else.
            if !gone_within(self.pid, WAIT) {
                // SAFETY: signalling the host this fixture started (its pid
                // was checked to be above 1 when it was learned).
                unsafe { libc::kill(self.pid as libc::pid_t, libc::SIGKILL) };
                ended = gone_within(self.pid, WAIT);
            }
        }
        let dir = self.dir.take().expect("dropped once");
        if !ended {
            eprintln!(
                "fixture: the host (pid {}) on {} did not end; its directory is kept",
                self.pid,
                self.socket.display()
            );
            let _ = dir.keep();
        }
    }
}

/// Whether `pid` is gone (or a zombie) within `wait`.
fn gone_within(pid: u32, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    while running(pid) {
        if Instant::now() >= deadline {
            return false;
        }
        // sleep-ok: polling interval for the end of a process the fixture owns.
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

/// One protocol connection.
pub struct Client {
    pub stream: UnixStream,
    buf: Vec<u8>,
}

impl Client {
    pub fn raw(socket: &Path) -> Self {
        let stream = UnixStream::connect(socket).expect("fixture: connecting to the host");
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        Self {
            stream,
            buf: Vec::new(),
        }
    }

    pub fn send(&mut self, message: &ClientMessage) {
        self.stream
            .write_all(&encode_client(message))
            .expect("sending to the host");
    }

    /// The next message, or `None` if the host closed the connection.
    pub fn try_recv(&mut self, wait: Duration) -> Option<HostMessage> {
        let deadline = Instant::now() + wait;
        loop {
            if let Some(frame) = split_frame(&self.buf, MAX_FRAME).expect("a well-formed frame") {
                let message = decode_host(frame, 1).expect("a well-formed message");
                let consumed = frame.consumed;
                self.buf.drain(..consumed);
                return Some(message);
            }
            assert!(
                Instant::now() < deadline,
                "no message from the host within {wait:?}"
            );
            let mut chunk = [0u8; 64 * 1024];
            match self.stream.read(&mut chunk) {
                Ok(0) => return None,
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => return None,
                Err(error) => panic!("reading from the host: {error}"),
            }
        }
    }

    pub fn recv(&mut self) -> HostMessage {
        self.try_recv(WAIT).expect("the host closed the connection")
    }

    /// Whether the host closes the connection (end of stream) within the wait.
    pub fn closed(&mut self) -> bool {
        let deadline = Instant::now() + WAIT;
        while Instant::now() < deadline {
            match self.try_recv(deadline - Instant::now()) {
                None => return true,
                Some(_) => continue,
            }
        }
        false
    }

    pub fn info(&mut self) -> Info {
        self.send(&ClientMessage::Info);
        match self.until(|message| matches!(message, HostMessage::InfoReply(_))) {
            HostMessage::InfoReply(info) => info,
            _ => unreachable!("until returns a match"),
        }
    }

    /// Start a read from nothing.
    pub fn read_all(&mut self) {
        self.send(&ClientMessage::Read(Position::default()));
    }

    /// Collect output until `pattern` appears in it, returning everything
    /// received meanwhile (output bytes, and the other stream messages).
    /// Bounded by [`WAIT`] and [`MAX_RETAINED`] in all.
    pub fn output_until(&mut self, pattern: &str) -> (String, Vec<HostMessage>) {
        let deadline = Instant::now() + WAIT;
        let mut text = Vec::new();
        let mut others = Vec::new();
        while !String::from_utf8_lossy(&text).contains(pattern) {
            assert!(
                Instant::now() < deadline
                    && text.len() <= MAX_RETAINED
                    && others.len() <= MAX_RETAINED / 1024,
                "{pattern:?} never appeared; {} bytes of output and {} other messages, ending {:?}",
                text.len(),
                others.len(),
                String::from_utf8_lossy(&text[text.len().saturating_sub(4096)..])
            );
            match self.try_recv(deadline - Instant::now()) {
                Some(HostMessage::Output { data, .. }) => text.extend(data),
                Some(other) => others.push(other),
                None => panic!(
                    "closed before {pattern:?}; output: {:?}",
                    String::from_utf8_lossy(&text)
                ),
            }
        }
        (String::from_utf8_lossy(&text).into_owned(), others)
    }

    /// Wait for a message matching `predicate`, skipping others, within
    /// [`WAIT`] for the whole wait.
    pub fn until(&mut self, mut predicate: impl FnMut(&HostMessage) -> bool) -> HostMessage {
        let deadline = Instant::now() + WAIT;
        let mut skipped = 0usize;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "no matching message within {WAIT:?}; {skipped} others came"
            );
            let message = self.try_recv(left).expect("the host closed the connection");
            if predicate(&message) {
                return message;
            }
            skipped += 1;
        }
    }

    /// Everything the child printed from the start until its exit record,
    /// and the exit: start a read from nothing and collect its output,
    /// however the reads split it, within [`WAIT`] and [`MAX_RETAINED`].
    pub fn output_to_exit(&mut self) -> (String, ChildExit) {
        self.read_all();
        let deadline = Instant::now() + WAIT;
        let mut text = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero() && text.len() <= MAX_RETAINED,
                "no exit record within {WAIT:?}; {} bytes of output, ending {:?}",
                text.len(),
                String::from_utf8_lossy(&text[text.len().saturating_sub(4096)..])
            );
            match self.try_recv(left).expect("the host closed the connection") {
                HostMessage::Output { data, .. } => text.extend(data),
                HostMessage::Exited { exit, .. } => {
                    return (String::from_utf8_lossy(&text).into_owned(), exit);
                }
                _ => {}
            }
        }
    }
}

/// Whether process `pid` exists and is not a zombie: whether it still
/// runs. (`kill(pid, 0)` alone succeeds for a zombie too.)
pub fn running(pid: u32) -> bool {
    exists(pid) && !zombie(pid)
}

/// Whether a process `pid` exists at all, zombies included.
pub fn exists(pid: u32) -> bool {
    // SAFETY: kill with signal 0 only checks for the process.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// Whether `pid` is a zombie: exited, and not reaped by its parent yet.
/// A pid that no longer exists is not one.
pub fn zombie(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        // The state is the field after the command name, which is in
        // parentheses and may itself contain spaces or parentheses.
        std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
            stat.rsplit_once(") ")
                .is_some_and(|(_, rest)| rest.starts_with('Z'))
        })
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("/bin/ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .is_ok_and(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .starts_with('Z')
            })
    }
}

/// When process `pid` started, in a form that tells two processes that had
/// the same pid apart; `None` when no process has that pid. A zombie still
/// has its start time.
pub fn start_time(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        // Field 22 of the stat line, counted after the command name (which
        // is in parentheses and may itself contain spaces or parentheses):
        // the state is field 3, so the start time is the 20th after it.
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let (_, rest) = stat.rsplit_once(") ")?;
        rest.split_whitespace().nth(19).map(str::to_owned)
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/bin/ps")
            .args(["-o", "lstart=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (output.status.success() && !text.is_empty()).then_some(text)
    }
}

/// A process the test started some other way (a descendant of a host's
/// child, say), killed with SIGKILL when the guard drops, so that a failing
/// assertion cannot leave it running.
pub struct KillOnDrop(pub u32);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if self.0 > 1 {
            // SAFETY: signalling a process the test started.
            unsafe { libc::kill(self.0 as libc::pid_t, libc::SIGKILL) };
        }
    }
}

/// A process that records its own pid in a file, killed with SIGKILL when
/// the guard drops, whether or not it ever got that far.
///
/// Armed with the file's path before the process exists, so that a test
/// failing while it waits on the process (an event that never comes, output
/// without the pid it expected) still ends it; a guard made from a pid the
/// test parsed only covers what happens after the parse. Drop it before the
/// directory holding the file.
pub struct KillByPidFile(pub PathBuf);

impl Drop for KillByPidFile {
    fn drop(&mut self) {
        let pid = std::fs::read_to_string(&self.0)
            .ok()
            .and_then(|text| text.trim().parse::<u32>().ok());
        if let Some(pid) = pid {
            drop(KillOnDrop(pid));
        }
    }
}

/// Make a FIFO at `path`, for a child's script to block on.
pub fn make_fifo(path: &Path) {
    use std::os::unix::ffi::OsStrExt as _;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: mkfifo with a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0, "mkfifo");
}

/// Write a line to the FIFO at `path` once a reader has it open, bounded:
/// opening it for writing without blocking fails with ENXIO until then.
pub fn release(path: &Path) {
    use std::os::unix::fs::OpenOptionsExt as _;
    eventually(
        "a reader opens the FIFO",
        || match std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
        {
            Ok(mut file) => {
                file.write_all(b"go\n").expect("writing to the FIFO");
                true
            }
            Err(error) if error.raw_os_error() == Some(libc::ENXIO) => false,
            Err(error) => panic!("opening the FIFO: {error}"),
        },
    );
}

/// Wait until `condition` holds, bounded.
pub fn eventually(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "{what} did not happen within {WAIT:?}"
        );
        // sleep-ok: polling interval for an observable owned by the test.
        std::thread::sleep(Duration::from_millis(20));
    }
}
