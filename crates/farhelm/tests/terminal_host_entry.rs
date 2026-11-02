//! `farhelm internal terminal-host` with the real `farhelm` binary: the
//! entry point that ships the terminal host inside Farhelm.
//!
//! The host's own behavior is the `terminal-host` crate's to test; what is
//! Farhelm's here is that the entry point holds no version of Farhelm. A
//! host lives for weeks, and a version lock it kept would hold that version
//! against pruning and uninstall's exclusive lock for as long (SPEC_impl.md,
//! "Terminal host").

use std::io::{Read as _, Write as _};
use std::os::fd::AsRawFd as _;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use terminal_host::protocol::{ClientMessage, Size, Spawn, encode_client, encode_spawn};

/// The pid of the process listening on the other end of `stream`: a
/// connection's peer credentials are the listener's.
fn peer_pid(stream: &UnixStream) -> libc::pid_t {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: ucred is plain data; SO_PEERCRED fills one of this size.
        let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut cred as *mut libc::ucred).cast(),
                &mut len,
            )
        };
        assert_eq!(result, 0, "SO_PEERCRED");
        cred.pid
    }
    #[cfg(target_os = "macos")]
    {
        let mut pid: libc::pid_t = 0;
        let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
        // SAFETY: LOCAL_PEERPID fills one pid_t.
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_LOCAL,
                libc::LOCAL_PEERPID,
                (&mut pid as *mut libc::pid_t).cast(),
                &mut len,
            )
        };
        assert_eq!(result, 0, "LOCAL_PEERPID");
        pid
    }
}

/// The host this test started, ended when the guard drops, so that a
/// failing assertion cannot leave it running: `Shutdown` through its
/// socket, then SIGKILL if it still runs after a bounded wait.
struct Running {
    socket: PathBuf,
    pid: libc::pid_t,
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Ok(mut stream) = UnixStream::connect(&self.socket) {
            let mut bytes = encode_client(&ClientMessage::Hello {
                version: 1,
                take_over: true,
            });
            bytes.extend(encode_client(&ClientMessage::Shutdown));
            let _ = stream.write_all(&bytes);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut buf = [0u8; 256];
            while Instant::now() < deadline && !matches!(stream.read(&mut buf), Ok(0) | Err(_)) {}
        }
        // The socket is gone once the host ended; a host still listening
        // after the Shutdown is killed. (A pid of 0 or 1 would hit this
        // test's own process group or init.)
        if self.socket.exists() && self.pid > 1 {
            // SAFETY: signalling the host this test started.
            unsafe { libc::kill(self.pid, libc::SIGKILL) };
        }
    }
}

/// A host started through `farhelm internal terminal-host` while it inherits
/// a shared lock on a version file (as descriptor 8, announced in
/// `FARHELM_VERSION_LOCK_FD`, the way the forwarder hands its lock to the
/// program it starts) keeps neither the lock nor the variable.
///
/// Why: the forwarder passes its lock to whatever `farhelm` command it
/// starts, and every other command keeps it for its lifetime on purpose; a
/// host must not, or a version would stay held for as long as any terminal
/// lives. Spec: once the host has started and the test dropped its own copy
/// of the lock, an exclusive lock on the version file succeeds while the host
/// runs, and (on Linux) no descriptor of the host refers to that file. (The
/// variable itself is not checked: `/proc/<pid>/environ` shows the
/// environment as it was at exec, not after `farhelm` removed the variable,
/// and its value names a descriptor, nothing secret.)
#[test]
fn a_terminal_host_holds_no_version_of_farhelm() {
    let dir = tempfile::tempdir().unwrap();
    let version = dir.path().join("farhelm-version");
    std::fs::write(&version, b"stand-in program").unwrap();
    let lock = std::fs::File::open(&version).unwrap();
    lock.lock_shared().unwrap();
    let lock_fd = lock.as_raw_fd();
    let socket = dir.path().join("h");
    let mut command = Command::new(env!("CARGO_BIN_EXE_farhelm"));
    command
        .args(["internal", "terminal-host", "--socket"])
        .arg(&socket)
        .env("FARHELM_VERSION_LOCK_FD", "8")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: dup2 is async-signal-safe; it gives the child the lock as fd 8.
    // A failed handoff fails the spawn: a host that never got the lock would
    // pass the check below without testing anything.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(lock_fd, 8) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    let spawn = Spawn {
        argv: vec![b"/bin/sleep".to_vec(), b"60".to_vec()],
        cwd: b"/".to_vec(),
        env: vec![b"PATH=/usr/bin:/bin".to_vec()],
        size: Size { cols: 80, rows: 24 },
        journal_cap: 1024 * 1024,
        tag: vec![],
        initial_snapshot: None,
    };
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&encode_spawn(&spawn))
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "the host started: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let host = Running {
        pid: peer_pid(&UnixStream::connect(&socket).unwrap()),
        socket,
    };
    drop(lock);
    let probe = std::fs::File::open(&version).unwrap();
    let result = probe.try_lock();
    #[cfg(target_os = "linux")]
    let host_files: Vec<std::path::PathBuf> = std::fs::read_dir(format!("/proc/{}/fd", host.pid))
        .unwrap()
        .filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
        .collect();
    drop(host);
    assert!(
        result.is_ok(),
        "the host still held the version lock: {result:?}"
    );
    #[cfg(target_os = "linux")]
    assert!(
        !host_files
            .iter()
            .any(|target| target.ends_with("farhelm-version")),
        "the host kept a descriptor on the version file: {host_files:?}"
    );
}

/// A host starts from a version that an update or uninstall holds
/// exclusively, without waiting for it.
///
/// Why: every other `farhelm` command run from the Mac app's version layout
/// first takes a shared lock on its own version, and waits, then refuses,
/// while an update or uninstall holds it exclusively. A terminal host must
/// skip that: starting a terminal during an update would stall or fail, and
/// a host that held its version would block pruning and uninstall for as
/// long as it lives. The test above cannot tell whether the entry point
/// skips the lock: outside the layout no lock is ever taken, and the host's
/// second stage closes every inherited descriptor anyway. So this one runs
/// the program from inside a version layout. Spec: with a copy (or hard
/// link) of `farhelm` at `Farhelm.app/Contents/Versions/9.9.9/farhelm`
/// that the test holds exclusively, an ordinary command run from it
/// (`internal stdio`, on a private state directory) refuses because the
/// version is being updated, which shows the fixture is the layout and the
/// lock is in force; `farhelm internal terminal-host` run from it starts
/// its host (exit status 0), and the host is listening.
#[test]
fn a_terminal_host_starts_while_its_version_is_held_exclusively() {
    let dir = tempfile::tempdir().unwrap();
    let version_dir = dir.path().join("Farhelm.app/Contents/Versions/9.9.9");
    std::fs::create_dir_all(&version_dir).unwrap();
    let program = version_dir.join("farhelm");
    // A hard link where the build directory allows one, so that no copy of a
    // large debug binary is made; otherwise a copy.
    if std::fs::hard_link(env!("CARGO_BIN_EXE_farhelm"), &program).is_err() {
        std::fs::copy(env!("CARGO_BIN_EXE_farhelm"), &program).unwrap();
    }
    let held = std::fs::File::open(&program).unwrap();
    held.lock().unwrap();
    let ordinary = Command::new(&program)
        .args(["internal", "stdio", "--state-dir"])
        .arg(dir.path().join("state"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let refusal = String::from_utf8_lossy(&ordinary.stderr);
    assert!(
        !ordinary.status.success() && refusal.contains("being updated or uninstalled"),
        "premise: an ordinary command from this version refuses while it is held: {refusal}"
    );
    let socket = dir.path().join("h");
    let mut child = Command::new(&program)
        .args(["internal", "terminal-host", "--socket"])
        .arg(&socket)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let spawn = Spawn {
        argv: vec![b"/bin/sleep".to_vec(), b"60".to_vec()],
        cwd: b"/".to_vec(),
        env: vec![b"PATH=/usr/bin:/bin".to_vec()],
        size: Size { cols: 80, rows: 24 },
        journal_cap: 1024 * 1024,
        tag: vec![],
        initial_snapshot: None,
    };
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&encode_spawn(&spawn))
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "the host did not start while its version was held: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _host = Running {
        pid: peer_pid(&UnixStream::connect(&socket).unwrap()),
        socket,
    };
}
