//! Starting a terminal host and waiting until it is ready.
//!
//! A host starts in two stages (`crates/terminal-host/SPEC.md`,
//! "Start and readiness"): the first stage reads the child spec from stdin,
//! forks the host proper, and exits 0 once the socket is bound and the child
//! runs, or 1 with one line on stderr when anything failed, leaving nothing
//! behind. So a spawn is: pick a socket name, run the first stage, write the
//! spec, and wait for its exit status. The supervisor never has a long-lived
//! child to reap; the host proper is re-parented away from it as soon as the
//! first stage exits.
//!
//! Three things here are deliberate, and each closes a way a host could hurt
//! the sessions it serves:
//!
//! - **The program is the supervisor's own executable**, never the Mac
//!   forwarder: see [`own_host_program`]. The host's command line is then
//!   private to one build, and only its wire protocol is a cross-version
//!   surface.
//! - **The host's own environment is empty.** The child's environment
//!   travels in the spec, on stdin, and nothing of the supervisor's reaches
//!   the host process. That keeps the session markers (`FARHELM_SESSION_ID`
//!   and the like, which Stop's process sweep searches for) off the host, so
//!   a sweep never kills it, and it keeps credentials out of an environment
//!   that any same-user process can read for the weeks a host lives.
//! - **Nothing secret is on the command line**: only the socket path, which
//!   is all the host's command line may carry.
//!
//! A start whose first stage did not exit normally (the deadline passed, a
//! signal killed it, or waiting for it failed) has no known outcome: the
//! host may have reported ready just before. Such a start is never reported
//! as a clean refusal. The spawn ends whatever answers at the socket itself,
//! and says either that nothing is left ([`SpawnError::Abandoned`]) or where
//! something may be ([`SpawnError::MayRemain`]). A tab has no stored record,
//! so a host that outlived a failed open would otherwise pass, once found by
//! its tag, for a tab the user was told never opened.

use std::ffi::OsString;
use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use terminal_host::protocol::{
    ClientMessage, PROTOCOL_VERSION, Size, Spawn, encode_client, encode_spawn,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use super::dir::{HostsDirError, ensure_hosts_dir};
use super::names::HostName;
use super::tag::{HostTag, TagError};

/// How long a spawn waits for the first stage to exit. A first stage only
/// reads the spec, forks, and waits for the host to bind its socket and start
/// the child, so this is far beyond any healthy start; it exists so that a
/// wedged start cannot hold a session operation forever.
const SPAWN_DEADLINE: Duration = Duration::from_secs(20);

/// How long a spawn whose start did not finish spends ending whatever
/// answers at the socket. A host ends as soon as its `Goodbye` is written
/// (its own bound on that is one second); this is room for a loaded machine.
const CLEANUP_DEADLINE: Duration = Duration::from_secs(5);

/// Pause between looks at the socket while ending an unfinished start.
const CLEANUP_POLL: Duration = Duration::from_millis(50);

/// How long a failed first stage's stderr is read after its exit. Its one
/// line is written before it exits, so this only bounds a pipe that another
/// process inherited and keeps open.
const STDERR_DRAIN: Duration = Duration::from_secs(1);

/// The most of the first stage's stderr a spawn keeps. A failing first stage
/// prints one line; this only bounds one that misbehaves.
const STDERR_CAP: usize = 4 * 1024;

/// How many fresh names a spawn draws before giving up on finding a free
/// one. With 40 random bits per name, a second draw is already a curiosity.
const NAME_ATTEMPTS: usize = 8;

/// Everything about the program a host runs. It reaches the host on stdin
/// (the host crate's `Spawn`, minus the tag, which [`spawn_host`] adds).
///
/// The host checks the spec and refuses one that breaks its rules (an empty
/// argv, a relative working directory, a NUL byte, a cap out of range) with
/// its own one-line reason, which [`SpawnError::Refused`] carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildSpec {
    /// The program and its arguments; `argv[0]` is the program.
    pub argv: Vec<OsString>,
    /// The working directory, absolute.
    pub cwd: PathBuf,
    /// The child's complete environment. The host adds nothing to it.
    pub env: Vec<(OsString, OsString)>,
    /// The terminal's initial size.
    pub size: Size,
    /// The host's journal budget, in bytes (see
    /// `crates/terminal-host/SPEC.md`, "Journal").
    pub journal_cap: u64,
    /// A snapshot of the client's own making, for a host that continues a
    /// terminal another host ran before.
    pub initial_snapshot: Option<Vec<u8>>,
}

/// A host that reported ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnedHost {
    /// The socket's name in the socket directory.
    pub name: HostName,
    /// The socket's full path, always absolute.
    pub socket: PathBuf,
}

/// Why a host did not start. Every variant but [`Self::MayRemain`] leaves no
/// host running and no socket behind.
#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    /// The state directory's absolute path could not be made.
    #[error("resolving the state directory: {0}")]
    StateDir(io::Error),
    /// The socket directory is missing a usable shape.
    #[error(transparent)]
    HostsDir(#[from] HostsDirError),
    /// The tag cannot be written.
    #[error(transparent)]
    Tag(#[from] TagError),
    /// No socket name could be drawn.
    #[error("choosing a terminal host socket name: {0}")]
    Name(io::Error),
    /// The host program could not be run at all.
    #[error("running the terminal host program {program}: {source}")]
    Start { program: PathBuf, source: io::Error },
    /// The host refused or failed to start, and said so: it exited with
    /// `status` (not 0), and `message` is the first line of its stderr
    /// (empty when it printed none). A host that exits this way leaves
    /// nothing behind
    /// (`crates/terminal-host/SPEC.md`, "Start and readiness").
    #[error("the terminal host did not start (exit status {status}): {message}")]
    Refused { status: i32, message: String },
    /// The start did not finish (`reason`: the deadline, a signal, a failed
    /// wait), and the spawn confirmed that nothing of it is left: the socket
    /// is gone, and a second stage that never reached its socket ends itself
    /// once its readiness report finds the first stage gone.
    #[error("the terminal host did not start: {reason}")]
    Abandoned { reason: String },
    /// The start did not finish (`reason`), and something still answers, or
    /// may, at `socket` that the spawn could not end in time. Whoever sees
    /// this owns that socket: a host there carries this spawn's tag, so
    /// anything that finds hosts by their tags must not take it for a
    /// terminal that opened.
    #[error("the terminal host did not start ({reason}), and something may be left at {}", socket.display())]
    MayRemain { reason: String, socket: PathBuf },
}

/// The program a production supervisor starts hosts from, given its own
/// program as resolved at startup (the path it also runs its launch shim
/// from).
///
/// On Linux it is `/proc/self/exe`: the kernel resolves that to the file the
/// supervisor is running, even after an update has renamed a new `farhelm`
/// over the old path, so a host never starts from a newer build than its
/// supervisor during an update's rename window. Elsewhere (the Mac) it is
/// `own_program`: in the app's layout that is the supervisor's own versioned
/// program (`…/Contents/Versions/<v>/farhelm`), never the forwarder, and an
/// update never removes a version a running supervisor holds.
///
/// Tests never use this: their own executable is the test runner, which has
/// no `internal terminal-host`. They pass the built `farhelm` to
/// [`spawn_host`] directly.
pub fn own_host_program(own_program: &Path) -> PathBuf {
    if cfg!(target_os = "linux") {
        PathBuf::from("/proc/self/exe")
    } else {
        own_program.to_path_buf()
    }
}

/// The time limits of one spawn, injectable so tests can reach the
/// unfinished-start paths in moments.
#[derive(Debug, Clone, Copy)]
struct Deadlines {
    /// For the first stage to exit.
    start: Duration,
    /// For ending whatever answers after an unfinished start.
    cleanup: Duration,
}

impl Default for Deadlines {
    fn default() -> Self {
        Self {
            start: SPAWN_DEADLINE,
            cleanup: CLEANUP_DEADLINE,
        }
    }
}

/// Start a host for `child`, tagged `tag`, with its socket in `state_dir`'s
/// socket directory, and return once it is ready: its first stage exited 0,
/// so its socket is bound and its child has started.
///
/// `program` is a `farhelm` (in production, [`own_host_program`]); it runs as
/// `<program> internal terminal-host --socket <path>`, with an empty
/// environment. The path is absolute even for a relative `state_dir` (the
/// host refuses a relative one); a relative state directory can therefore
/// give a socket path longer than the supervisor's own relative one.
///
/// Readiness is the first stage's exit status alone. Its stderr is read only
/// for a failure's message, never waited on to its end: another process
/// that inherited the pipe can keep it open.
///
/// Dropping the returned future before it finishes kills the first stage
/// without the cleanup [`SpawnError::Abandoned`] and
/// [`SpawnError::MayRemain`] describe; a host may then be left at the
/// socket.
pub async fn spawn_host(
    program: &Path,
    state_dir: &Path,
    child: ChildSpec,
    tag: &HostTag,
) -> Result<SpawnedHost, SpawnError> {
    spawn_host_within(program, state_dir, child, tag, Deadlines::default()).await
}

/// [`spawn_host`] with its deadlines injected.
async fn spawn_host_within(
    program: &Path,
    state_dir: &Path,
    child: ChildSpec,
    tag: &HostTag,
    deadlines: Deadlines,
) -> Result<SpawnedHost, SpawnError> {
    let tag = tag.encode()?;
    let state_dir = std::path::absolute(state_dir).map_err(SpawnError::StateDir)?;
    let hosts = ensure_hosts_dir(&state_dir)?;
    let (name, socket) = free_name(&hosts)?;
    let spec = encode_child(child, tag);

    let mut first = tokio::process::Command::new(program)
        .args(["internal", "terminal-host", "--socket"])
        .arg(&socket)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        // A spawn abandoned midway kills the first stage rather than leave
        // it waiting on a pipe nobody writes.
        .kill_on_drop(true)
        .spawn()
        .map_err(|source| SpawnError::Start {
            program: program.to_path_buf(),
            source,
        })?;
    let mut stdin = first.stdin.take().expect("stdin is piped");
    let mut stderr = first.stderr.take().expect("stderr is piped");
    let mut message = Vec::new();
    let mut stderr_open = true;
    // The spec is written, stderr read and the exit awaited all at once, and
    // only the exit decides: a first stage that refuses early stops reading
    // its stdin, and one that fails writes its line before it exits.
    let waited = tokio::time::timeout(deadlines.start, async {
        let write = async move {
            // A write error means the first stage stopped reading (it
            // refused, or died); its exit says why. Dropping stdin here is
            // the end of file the first stage waits for.
            let _ = stdin.write_all(&spec).await;
        };
        tokio::pin!(write);
        let mut written = false;
        let mut chunk = [0u8; 1024];
        loop {
            tokio::select! {
                () = &mut write, if !written => written = true,
                read = stderr.read(&mut chunk), if stderr_open => match read {
                    Ok(n) if n > 0 => keep_stderr(&mut message, &chunk[..n], &mut stderr_open),
                    _ => stderr_open = false,
                },
                status = first.wait() => break status,
            }
        }
    })
    .await;

    let reason = match waited {
        Ok(Ok(status)) if status.success() => return Ok(SpawnedHost { name, socket }),
        Ok(Ok(status)) => match status.code() {
            Some(code) => {
                if stderr_open {
                    drain_stderr(&mut stderr, &mut message).await;
                }
                let message = String::from_utf8_lossy(&message);
                return Err(SpawnError::Refused {
                    status: code,
                    message: message.lines().next().unwrap_or("").trim().to_string(),
                });
            }
            None => format!(
                "its first stage was killed by signal {}",
                status.signal().unwrap_or(0)
            ),
        },
        Ok(Err(error)) => format!("waiting for its first stage failed: {error}"),
        Err(_) => format!("it did not report readiness within {:?}", deadlines.start),
    };
    // The first stage must be gone before the socket is judged: only then
    // does a second stage that has not reported yet find its report unread
    // and end itself. SIGKILL, then reap, bounded: a killed process exits at
    // once, and if even that wait fails, kill_on_drop still has it.
    let _ = first.start_kill();
    if !matches!(
        tokio::time::timeout(deadlines.cleanup, first.wait()).await,
        Ok(Ok(_))
    ) {
        return Err(SpawnError::MayRemain {
            reason: format!("{reason}; its first stage could not be reaped"),
            socket,
        });
    }
    if end_unreported_host(&socket, deadlines.cleanup).await {
        Err(SpawnError::Abandoned { reason })
    } else {
        Err(SpawnError::MayRemain { reason, socket })
    }
}

/// The child spec's frame, tag included.
fn encode_child(child: ChildSpec, tag: Vec<u8>) -> Vec<u8> {
    encode_spawn(&Spawn {
        argv: child
            .argv
            .iter()
            .map(|arg| arg.as_bytes().to_vec())
            .collect(),
        cwd: child.cwd.as_os_str().as_bytes().to_vec(),
        env: child
            .env
            .iter()
            .map(|(name, value)| {
                let mut entry = name.as_bytes().to_vec();
                entry.push(b'=');
                entry.extend_from_slice(value.as_bytes());
                entry
            })
            .collect(),
        size: child.size,
        journal_cap: child.journal_cap,
        tag,
        initial_snapshot: child.initial_snapshot,
    })
}

/// Keep `chunk` of the first stage's stderr, up to [`STDERR_CAP`] in all;
/// past the cap, stop reading it.
fn keep_stderr(message: &mut Vec<u8>, chunk: &[u8], open: &mut bool) {
    let room = STDERR_CAP.saturating_sub(message.len());
    message.extend_from_slice(&chunk[..chunk.len().min(room)]);
    if message.len() >= STDERR_CAP {
        *open = false;
    }
}

/// Read what is left of a failed first stage's stderr, within
/// [`STDERR_DRAIN`] and [`STDERR_CAP`].
async fn drain_stderr(stderr: &mut tokio::process::ChildStderr, message: &mut Vec<u8>) {
    let _ = tokio::time::timeout(STDERR_DRAIN, async {
        let mut open = true;
        let mut chunk = [0u8; 1024];
        while open {
            match stderr.read(&mut chunk).await {
                Ok(n) if n > 0 => keep_stderr(message, &chunk[..n], &mut open),
                _ => break,
            }
        }
    })
    .await;
}

/// End whatever answers at `socket` after a start that did not finish, and
/// say whether the socket is gone within `within`.
///
/// Called once the first stage is reaped. Then a second stage that has not
/// reported readiness yet ends itself when it tries (the host crate's start
/// contract), whether or not it has bound the socket yet; one that did
/// report runs as a host, which takes over this connection's `Shutdown` and
/// removes its socket as it ends. So a socket that is gone means nothing is
/// left, and one that stays means something may be.
async fn end_unreported_host(socket: &Path, within: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + within;
    let mut asked = false;
    loop {
        if matches!(
            std::fs::symlink_metadata(socket),
            Err(error) if error.kind() == io::ErrorKind::NotFound
        ) {
            return true;
        }
        if !asked
            && let Ok(Ok(mut stream)) =
                tokio::time::timeout_at(deadline, tokio::net::UnixStream::connect(socket)).await
        {
            asked = true;
            let mut request = encode_client(&ClientMessage::Hello {
                version: PROTOCOL_VERSION,
                take_over: true,
            });
            request.extend(encode_client(&ClientMessage::Shutdown));
            // A failed write is a connection already closed; what the socket
            // does next decides either way. The reply is read only to see the
            // connection end.
            let _ = tokio::time::timeout_at(deadline, async {
                if stream.write_all(&request).await.is_ok() {
                    let mut buf = [0u8; 4096];
                    while matches!(stream.read(&mut buf).await, Ok(n) if n > 0) {}
                }
            })
            .await;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(CLEANUP_POLL).await;
    }
}

/// A name whose socket path is free in the socket directory `hosts`.
///
/// The host refuses a socket path that exists (it never replaces a file), so
/// a taken name would only fail the spawn; checking first turns that rare
/// collision into a second draw. The check cannot race another spawn into a
/// silent overwrite: the host's own refusal still stands behind it.
fn free_name(hosts: &Path) -> Result<(HostName, PathBuf), SpawnError> {
    for _ in 0..NAME_ATTEMPTS {
        let name = HostName::random().map_err(SpawnError::Name)?;
        let socket = name.socket_in(hosts);
        match std::fs::symlink_metadata(&socket) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((name, socket)),
            Err(error) => return Err(SpawnError::Name(error)),
            Ok(_) => continue,
        }
    }
    Err(SpawnError::Name(io::Error::other(format!(
        "{NAME_ATTEMPTS} random names were all taken in {}",
        hosts.display()
    ))))
}

#[cfg(test)]
mod tests {
    //! The spawn's half of the start protocol, against stand-in first stages
    //! (small shell scripts) rather than real hosts: the command line, the
    //! empty environment, the spec on stdin, and how each way a first stage
    //! can end is reported, including the cleanup after a start that did not
    //! finish. Real hosts are started only by the tests beside the real
    //! `farhelm` binary (`crates/farhelm/tests/terminal_host_spawn.rs`),
    //! because a host forks and must not be forked out of a multi-threaded
    //! test process.

    use super::*;
    use crate::hosts::tag::HostRole;
    use std::io::{Read as _, Write as _};
    use std::os::unix::net::UnixListener;
    use terminal_host::protocol::{MAX_FRAME, decode_client, split_frame};

    /// A stand-in first stage at `dir/name` running `body` under `/bin/sh`.
    /// Commands in it use absolute paths: the spawn gives it no `PATH`.
    ///
    /// The script is run once before it is returned, as `tmux.rs`'s
    /// `probe_fixture` does: writing an executable and spawning it at once
    /// races `ETXTBSY` against any other thread's fork, which may still hold
    /// the descriptor that wrote it. The warm-up run exits before the body.
    fn starter(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::OpenOptionsExt as _;
        let path = dir.join(name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o755)
            .open(&path)
            .expect("fixture script");
        write!(
            file,
            "#!/bin/sh\ncase \"$1\" in --warmup) exit 0;; esac\n{body}\n"
        )
        .expect("write fixture");
        drop(file);
        for _ in 0..200 {
            match std::process::Command::new(&path).arg("--warmup").output() {
                Ok(_) => return path,
                Err(error) if error.raw_os_error() == Some(libc::ETXTBSY) => {
                    // sleep-ok: retry only ETXTBSY while an inherited writer closes; successful exec is the fixture boundary.
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("fixture {name} is not runnable: {error}"),
            }
        }
        panic!("fixture {name} stayed busy");
    }

    fn child() -> ChildSpec {
        ChildSpec {
            argv: vec!["/bin/sleep".into(), "60".into()],
            cwd: "/".into(),
            env: vec![
                ("PATH".into(), "/usr/bin:/bin".into()),
                ("FARHELM_SESSION_TOKEN".into(), "not-for-the-host".into()),
            ],
            size: Size { cols: 80, rows: 24 },
            journal_cap: 1024 * 1024,
            initial_snapshot: Some(b"snapshot".to_vec()),
        }
    }

    fn tag() -> HostTag {
        HostTag {
            session_id: "0f8e2b9c-4c1d-4e7a-9a53-7d2f61c0b8e4".into(),
            created_ns: 42,
            role: HostRole::Agent { generation: 3 },
        }
    }

    /// Deadlines long enough for a loaded machine to run the stand-ins,
    /// short enough that the unfinished-start tests take seconds.
    fn short(start_secs: u64, cleanup_secs: u64) -> Deadlines {
        Deadlines {
            start: Duration::from_secs(start_secs),
            cleanup: Duration::from_secs(cleanup_secs),
        }
    }

    /// A stand-in first stage that records its socket path in `record`,
    /// waits until something is bound there, and then never exits: a start
    /// that does not finish, with a socket for the cleanup to find.
    fn binds_then_hangs(dir: &Path, record: &Path) -> PathBuf {
        starter(
            dir,
            "farhelm",
            &format!(
                "/bin/cat > /dev/null\nprintf '%s' \"$4\" > {}\n\
                 while [ ! -S \"$4\" ]; do /bin/sleep 0.05; done\nexec /bin/sleep 600",
                record.display()
            ),
        )
    }

    /// What a stand-in listener at a recorded socket path does.
    enum Listener {
        /// Answers as a host does on `Shutdown`: reads `Hello` and
        /// `Shutdown`, removes its socket file, closes.
        Host,
        /// Accepts nothing and never answers, until the test lets it go.
        Silent(std::sync::mpsc::Receiver<()>),
    }

    /// Wait (bounded) for `record` to name a socket path, bind a stand-in
    /// listener there on a thread, and hand back the thread: it returns the
    /// path and what it received.
    fn listen_at_recorded(
        record: PathBuf,
        behavior: Listener,
    ) -> std::thread::JoinHandle<(PathBuf, Vec<ClientMessage>)> {
        std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let socket = loop {
                if let Ok(text) = std::fs::read_to_string(&record)
                    && !text.is_empty()
                {
                    break PathBuf::from(text);
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "the stand-in never recorded its socket"
                );
                // sleep-ok: polling interval for a file the test's own stand-in writes.
                std::thread::sleep(Duration::from_millis(10));
            };
            let listener = UnixListener::bind(&socket).expect("binding the stand-in");
            let mut received = Vec::new();
            match behavior {
                Listener::Silent(release) => {
                    let _ = release.recv_timeout(Duration::from_secs(30));
                }
                Listener::Host => {
                    let (mut conn, _) = listener.accept().expect("a connection");
                    conn.set_read_timeout(Some(Duration::from_secs(10)))
                        .unwrap();
                    let mut buf = Vec::new();
                    while received.len() < 2 {
                        let mut chunk = [0u8; 256];
                        let n = conn.read(&mut chunk).expect("reading the request");
                        assert!(n > 0, "the spawn closed before Shutdown");
                        buf.extend_from_slice(&chunk[..n]);
                        while let Some(frame) = split_frame(&buf, MAX_FRAME).unwrap() {
                            received.push(decode_client(frame, 1).unwrap());
                            let consumed = frame.consumed;
                            buf.drain(..consumed);
                        }
                    }
                    std::fs::remove_file(&socket).unwrap();
                }
            }
            (socket, received)
        })
    }

    /// The first stage runs as `<program> internal terminal-host --socket
    /// <path>` with an empty environment, and receives exactly the encoded
    /// child spec, tag included, on stdin; its exit 0 is readiness.
    ///
    /// Why: the command line is the host's contract (only the socket on it),
    /// an inherited environment would put the supervisor's markers and
    /// credentials on a process that lives for weeks, and the spec on stdin
    /// is the only way the child's environment may travel. Spec: a stand-in
    /// that records its arguments, its environment and its stdin, then exits
    /// 0, sees those four arguments with an absolute socket path inside
    /// `<state>/hosts/`, no environment variable beyond what `/bin/sh` sets
    /// for itself, and stdin equal to `encode_spawn` of the child spec with
    /// the encoded tag; the spawn returns that socket path.
    #[farhelm_testtrace::test]
    async fn the_first_stage_gets_the_socket_alone_an_empty_environment_and_the_spec() {
        let state = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let o = out.path().display();
        let program = starter(
            out.path(),
            "farhelm",
            &format!(
                "for a in \"$@\"; do printf '%s\\n' \"$a\"; done > {o}/args\n\
                 /usr/bin/env > {o}/env\n/bin/cat > {o}/stdin\nexit 0"
            ),
        );
        let spawned = spawn_host(&program, state.path(), child(), &tag())
            .await
            .expect("a first stage that exits 0 is a ready host");
        assert_eq!(
            spawned.socket,
            state.path().join("hosts").join(spawned.name.as_str())
        );
        let args = std::fs::read_to_string(out.path().join("args")).unwrap();
        assert_eq!(
            args,
            format!(
                "internal\nterminal-host\n--socket\n{}\n",
                spawned.socket.display()
            )
        );
        let env = std::fs::read_to_string(out.path().join("env")).unwrap();
        let names: Vec<&str> = env
            .lines()
            .filter_map(|line| line.split_once('=').map(|(name, _)| name))
            .collect();
        // What a shell exports on its own (and macOS's text encoding hint,
        // which the system may add to any process), and nothing else.
        let own = ["PWD", "OLDPWD", "SHLVL", "_", "__CF_USER_TEXT_ENCODING"];
        assert!(
            names.iter().all(|name| own.contains(name)),
            "the first stage inherited an environment: {env}"
        );
        let stdin = std::fs::read(out.path().join("stdin")).unwrap();
        let spec = child();
        let expected = encode_spawn(&Spawn {
            argv: vec![b"/bin/sleep".to_vec(), b"60".to_vec()],
            cwd: b"/".to_vec(),
            env: vec![
                b"PATH=/usr/bin:/bin".to_vec(),
                b"FARHELM_SESSION_TOKEN=not-for-the-host".to_vec(),
            ],
            size: spec.size,
            journal_cap: spec.journal_cap,
            tag: tag().encode().unwrap(),
            initial_snapshot: spec.initial_snapshot,
        });
        assert_eq!(stdin, expected);
    }

    /// A relative state directory still gives the host an absolute socket
    /// path.
    ///
    /// Why: `farhelm supervisor run --state-dir` accepts a relative path,
    /// and the host refuses a relative `--socket` with a usage error, so
    /// every spawn would fail. Spec: with the state directory named by a
    /// path relative to this process's working directory, the stand-in's
    /// fourth argument is absolute and equals the returned socket, which is
    /// the absolute form of that state directory joined with `hosts/<name>`.
    #[farhelm_testtrace::test]
    async fn a_relative_state_directory_gives_an_absolute_socket() {
        let state = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let cwd = std::env::current_dir().unwrap();
        // Up from the working directory to `/`, then down to the state
        // directory: relative, without changing this process's directory.
        let mut relative = PathBuf::new();
        for _ in cwd.components().skip(1) {
            relative.push("..");
        }
        relative.push(state.path().strip_prefix("/").unwrap());
        assert!(relative.is_relative(), "premise: {}", relative.display());
        let program = starter(
            out.path(),
            "farhelm",
            &format!(
                "printf '%s' \"$4\" > {}\n/bin/cat > /dev/null\nexit 0",
                out.path().join("socket").display()
            ),
        );
        let spawned = spawn_host(&program, &relative, child(), &tag())
            .await
            .expect("a first stage that exits 0 is a ready host");
        let given = PathBuf::from(std::fs::read_to_string(out.path().join("socket")).unwrap());
        assert!(given.is_absolute(), "{}", given.display());
        assert_eq!(given, spawned.socket);
        assert_eq!(
            spawned.socket,
            std::path::absolute(&relative)
                .unwrap()
                .join("hosts")
                .join(spawned.name.as_str())
        );
    }

    /// A first stage that fails is reported with its stderr line and exit
    /// status, and leaves the socket directory as it was.
    ///
    /// Why: the host's one stderr line is the only account of why a
    /// terminal did not start (a missing program, a refused spec), and the
    /// caller shows it to the user. Spec: a stand-in that reads its stdin,
    /// prints `terminal-host: no such program` and a second line, and exits
    /// 1 gives `Refused` with status 1 and exactly the first line; the
    /// socket directory stays empty.
    #[farhelm_testtrace::test]
    async fn a_refusal_carries_the_first_line_of_stderr() {
        let state = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let program = starter(
            bin.path(),
            "farhelm",
            "/bin/cat > /dev/null\necho 'terminal-host: no such program' >&2\necho second >&2\nexit 1",
        );
        match spawn_host(&program, state.path(), child(), &tag()).await {
            Err(SpawnError::Refused { status, message }) => {
                assert_eq!(status, 1);
                assert_eq!(message, "terminal-host: no such program");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert_eq!(
            std::fs::read_dir(state.path().join("hosts"))
                .unwrap()
                .count(),
            0
        );
    }

    /// A first stage that exits without reading its stdin is a refusal, not
    /// a write error.
    ///
    /// Why: the host exits on a usage error before it reads anything, so the
    /// spec write meets a closed pipe, and that must not hide the host's own
    /// reason. Spec: a stand-in that prints a line and exits 2 at once gives
    /// `Refused` with status 2 and that line.
    #[farhelm_testtrace::test]
    async fn a_first_stage_that_never_reads_its_stdin_is_a_refusal() {
        let state = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let program = starter(bin.path(), "farhelm", "echo usage >&2\nexit 2");
        let mut big = child();
        // Far more than a pipe buffers, so the write cannot complete.
        big.initial_snapshot = Some(vec![0; 4 * 1024 * 1024]);
        match spawn_host(&program, state.path(), big, &tag()).await {
            Err(SpawnError::Refused { status, message }) => {
                assert_eq!((status, message.as_str()), (2, "usage"));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// Readiness is the first stage's exit, not the end of its stderr.
    ///
    /// Why: another process that inherited the stderr pipe (a concurrent
    /// spawn's child, on a platform that cannot create pipes close-on-exec
    /// in one call) can keep it open long after the first stage exits; a
    /// spawn that waited for its end would report a running host as timed
    /// out. Spec: a stand-in that starts a background `sleep` holding its
    /// stderr and then exits 0 gives a ready host within the 10 s start
    /// deadline, well before that `sleep` ends.
    #[farhelm_testtrace::test]
    async fn readiness_does_not_wait_for_stderr_to_close() {
        let state = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let pid_file = bin.path().join("holder");
        let program = starter(
            bin.path(),
            "farhelm",
            &format!(
                "/bin/cat > /dev/null\n/bin/sleep 30 > /dev/null &\necho $! > {}\nexit 0",
                pid_file.display()
            ),
        );
        let started = std::time::Instant::now();
        let result = spawn_host_within(&program, state.path(), child(), &tag(), short(10, 1)).await;
        let holder: Option<libc::pid_t> = std::fs::read_to_string(&pid_file)
            .ok()
            .and_then(|text| text.trim().parse().ok());
        if let Some(pid) = holder.filter(|&pid| pid > 1) {
            // SAFETY: ending the stand-in's own background sleep.
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        assert!(result.is_ok(), "{result:?}");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{:?}",
            started.elapsed()
        );
    }

    /// A host program that cannot be run is reported as such.
    ///
    /// Why: a supervisor whose own program vanished (an uninstall under it)
    /// must say what it tried to run. Spec: a program path that does not
    /// exist gives `Start` naming that path.
    #[farhelm_testtrace::test]
    async fn a_missing_program_is_a_start_error() {
        let state = tempfile::tempdir().unwrap();
        let program = state.path().join("no-such-farhelm");
        match spawn_host(&program, state.path(), child(), &tag()).await {
            Err(SpawnError::Start { program: named, .. }) => assert_eq!(named, program),
            other => panic!("expected a start error, got {other:?}"),
        }
    }

    /// A first stage that never reports is killed at the deadline and
    /// reaped, and with no socket bound the start is abandoned with nothing
    /// left.
    ///
    /// Why: a wedged start must not hold a session operation forever, nor
    /// leave a stray first stage behind. Spec: a stand-in that writes its
    /// pid, reads its stdin and then waits forever (as `sleep`, exec'd so
    /// the pid is the first stage's) gives `Abandoned` within the injected
    /// 2 s start deadline plus a margin, and that pid no longer exists
    /// afterwards (killed and reaped, not a zombie).
    #[farhelm_testtrace::test]
    async fn a_first_stage_that_never_reports_is_killed_at_the_deadline() {
        let state = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let pid_file = bin.path().join("pid");
        let program = starter(
            bin.path(),
            "farhelm",
            &format!(
                "echo $$ > {}\n/bin/cat > /dev/null\nexec /bin/sleep 600",
                pid_file.display()
            ),
        );
        let started = std::time::Instant::now();
        let result = spawn_host_within(&program, state.path(), child(), &tag(), short(2, 2)).await;
        assert!(
            matches!(&result, Err(SpawnError::Abandoned { reason }) if reason.contains("readiness")),
            "{result:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the deadline did not bound the wait: {:?}",
            started.elapsed()
        );
        let pid: libc::pid_t = std::fs::read_to_string(&pid_file)
            .expect("premise: the stand-in started")
            .trim()
            .parse()
            .unwrap();
        // SAFETY: signal 0 only asks whether the pid exists; a reaped child's
        // pid does not (a zombie's would).
        assert_ne!(
            unsafe { libc::kill(pid, 0) },
            0,
            "the first stage {pid} survived or was not reaped"
        );
    }

    /// A first stage killed by a signal is an unfinished start, never a
    /// clean refusal.
    ///
    /// Why: a first stage can die after its host reported ready, and a
    /// refusal promises the caller that nothing is left. Spec: a stand-in
    /// that reads its stdin and kills itself with SIGKILL gives `Abandoned`
    /// (no socket was bound) whose reason names signal 9.
    #[farhelm_testtrace::test]
    async fn a_first_stage_killed_by_a_signal_is_an_unfinished_start() {
        let state = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let program = starter(bin.path(), "farhelm", "/bin/cat > /dev/null\nkill -9 $$");
        let result = spawn_host_within(&program, state.path(), child(), &tag(), short(10, 2)).await;
        assert!(
            matches!(&result, Err(SpawnError::Abandoned { reason }) if reason.contains("signal 9")),
            "{result:?}"
        );
    }

    /// After a start that did not finish, the spawn ends whatever answers at
    /// the socket, and reports nothing left once it is gone.
    ///
    /// Why: a host that reported ready just before its first stage was
    /// killed runs on, tagged as this spawn asked; left running, it would
    /// later pass for a terminal the user was told never opened. Spec: with
    /// a stand-in first stage that never exits and a stand-in host bound at
    /// its socket, the spawn (2 s start deadline) sends `Hello` with
    /// take-over and `Shutdown`, and once the stand-in removes its socket
    /// the result is `Abandoned`.
    #[farhelm_testtrace::test]
    async fn an_unfinished_start_ends_a_host_that_answers() {
        let state = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let record = bin.path().join("socket");
        let program = binds_then_hangs(bin.path(), &record);
        let host = listen_at_recorded(record, Listener::Host);
        let result = spawn_host_within(&program, state.path(), child(), &tag(), short(2, 5)).await;
        let (_, received) = host.join().expect("the stand-in host");
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
        assert!(
            matches!(result, Err(SpawnError::Abandoned { .. })),
            "{result:?}"
        );
    }

    /// After a start that did not finish, a socket that stays is reported
    /// with its path.
    ///
    /// Why: when the spawn cannot confirm that nothing is left, its caller
    /// must know where something may be, so that it is never taken for a
    /// terminal that opened. Spec: with a stand-in first stage that never
    /// exits and a listener at its socket that never answers, the spawn (2 s
    /// start, 1 s cleanup) gives `MayRemain` naming that socket.
    #[farhelm_testtrace::test]
    async fn an_unfinished_start_whose_socket_stays_may_remain() {
        let state = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let record = bin.path().join("socket");
        let program = binds_then_hangs(bin.path(), &record);
        let (release, wait) = std::sync::mpsc::channel();
        let silent = listen_at_recorded(record, Listener::Silent(wait));
        let result = spawn_host_within(&program, state.path(), child(), &tag(), short(2, 1)).await;
        let _ = release.send(());
        let (socket, _) = silent.join().expect("the stand-in listener");
        match result {
            Err(SpawnError::MayRemain { socket: named, .. }) => assert_eq!(named, socket),
            other => panic!("expected MayRemain, got {other:?}"),
        }
    }

    /// A production supervisor starts hosts from the file it is running on
    /// Linux, and from its own program elsewhere.
    ///
    /// Why: during an update's rename window, the path a Linux supervisor was
    /// started from names the new build, which must not run a host that the
    /// old supervisor then talks to; on the Mac the forwarder must never be
    /// the host's program. Spec: on Linux the program is `/proc/self/exe`
    /// whatever own program is given; elsewhere it is the given path.
    #[test]
    fn the_host_program_is_the_running_executable() {
        let own = Path::new("/Applications/Farhelm.app/Contents/Versions/9.9.9/farhelm");
        if cfg!(target_os = "linux") {
            assert_eq!(own_host_program(own), Path::new("/proc/self/exe"));
        } else {
            assert_eq!(own_host_program(own), own);
        }
    }
}
