//! A test stand-in for the `ssh` process the helm launches, with the stdio
//! behavior real OpenSSH gives that process.
//!
//! NOTE: This is not an ssh implementation. The "remote" command runs as a
//! local process; only the lifetime and contents of the helm's three pipes
//! and the local process's exit status are modeled.
//!
//! Provisioning tests used to stand in for `ssh` with a bare `sh -c <remote
//! command>`. That has the pipe behavior of an unshared connection: kill the
//! local process and every pipe closes at once. The helm almost never runs
//! that way. Every provisioning command carries `ControlMaster=auto` and
//! `ControlPersist` (see `crate::ssh::ssh_base_args`), so the `ssh` process the
//! helm holds is a client of a background master, and it is the master that
//! holds the session's stdio. Two bugs shipped because no test saw the
//! difference: remote uninstall was refused because the probe's stderr
//! outlived its killed ssh process and the helm discarded what it had read
//! (#3005), and a stalled upload hung the install for the same reason
//! (#3006).
//!
//! ## What real ssh does
//!
//! Measured on 2026-11-05 against a local sshd with OpenSSH 9.6p1 (Ubuntu).
//! The basic kill case (stdin and stdout closing at once, stderr held while
//! the remote runs) was also measured with OpenSSH 10.3p1 (macOS 26), which
//! agreed.
//!
//! Shared connection, while the local ssh process lives:
//!
//! - stdin is forwarded until the helm closes it, which the remote reads as
//!   EOF. If the remote closes its stdin first, ssh keeps taking the helm's
//!   writes (about 0.5 MB) and then blocks them; they fail only once ssh
//!   exits;
//! - stdout and stderr reach EOF only once the remote command has exited and
//!   nothing still holds its output, even when the remote closed one of them
//!   early (a helper it left in the background keeps them open);
//! - the local process exits at that same moment, with the remote command's
//!   exit status, or 255 when a signal ended it.
//!
//! Shared connection, after the helm kills the local ssh process:
//!
//! - stdin is closed at once: the helm's next write fails with EPIPE, and the
//!   remote command reads EOF;
//! - stdout is closed at once at both ends: the helm reads EOF, even if the
//!   remote had already closed its end, and the remote's next write to it
//!   fails (by default, SIGPIPE kills it);
//! - stderr stays open until the remote command itself exits, however long
//!   that is, and at once if it already has, even while a helper it left
//!   behind still holds it; it carries nothing the remote writes after the
//!   kill;
//! - the remote command keeps running.
//!
//! An unshared connection (`ControlMaster=no`, the fallback for a control
//! socket path that cannot be bound) closes all three pipes when the local
//! process dies. The stand-in keeps the older approximation there, killing the
//! remote command with it, which no current test depends on distinguishing.
//!
//! ## Where the stand-in differs
//!
//! - Unread helm input waits in one pipe (64 KiB on Linux), not ssh's larger
//!   buffers, so a helm write blocks sooner than it would over real ssh.
//! - Whatever is in the remote's stderr pipe when the master notices the
//!   client's death (a few milliseconds after it) is still delivered, as far
//!   as the helm's pipe has room. That covers everything written before the
//!   kill. A real master delivers what had reached it, which depends on the
//!   network; the stand-in picks the deterministic answer so a test does not
//!   race the kill.
//! - When the session ends, whatever is left of the remote's process group is
//!   killed, so a test does not leak it. On a real host a helper the remote
//!   left behind would keep running; a test that relies on that, such as one
//!   starting a background service through a remote command, will see it
//!   die.
//!
//! ## Short control directories
//!
//! The helm shares a connection only when the control socket path fits (see
//! `crate::ssh`), and on macOS the default temporary directory is too long
//! for that. A test whose control directory comes from `tempfile::tempdir()`
//! then gets the unshared approximation there. Tests that depend on sharing
//! use [`control_root`] and assert the sharing they rely on.

use std::{
    io,
    os::fd::{AsRawFd, OwnedFd, RawFd},
    process::Stdio,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::unix::pipe,
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
    sync::{oneshot, watch},
};

use super::backend::isolate_process_group;

/// Whether the helm's `command` rides a shared ssh connection.
///
/// Read from the argv the helm built, so a test never has to say which kind
/// it wants: a provisioning command over ssh asks for a shared connection
/// unless its control socket path did not fit. This does not consult the
/// user's ssh config or OpenSSH's option precedence, which the helm's own
/// overrides make irrelevant for the commands tests launch.
pub(super) fn shares_connection(command: &Command) -> bool {
    let command = command.as_std();
    command.get_program() == "ssh" && command.get_args().any(|arg| arg == "ControlMaster=auto")
}

/// A fresh, private control directory short enough for the helm to share
/// connections from on every platform (see the module docs). It is created
/// under `/tmp` with a random name, so concurrent tests and agents never
/// share one.
pub(super) fn control_root() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("fh-ssh-")
        .tempdir_in("/tmp")
        .expect("create a short control directory")
}

/// Start `remote` as the command sshd would run for the helm's `command`, and
/// return the child the helm should hold in place of its `ssh` process.
///
/// `remote` is the test's local stand-in for the remote command. Its stdio is
/// overwritten here; anything else on it (arguments, environment, working
/// directory) is the caller's. The returned child always has all three pipes,
/// whatever the helm asked for: a helm command that wanted stdin from
/// `/dev/null` sees the same EOF once the child is waited on or dropped.
///
/// For a shared connection the returned child is a small client process that
/// exits with the remote command's status, and a task on the current tokio
/// runtime plays the master (see the module docs). The remote command leads a
/// process group of its own, so the helm's group kill reaches only the client,
/// as it reaches only the local ssh process in production. When the master
/// task ends, normally or because the runtime drops it, it kills whatever is
/// left of that group, so a test does not leak the remote or its helpers.
pub(super) fn spawn_remote(command: &Command, mut remote: Command) -> io::Result<Child> {
    remote
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    isolate_process_group(&mut remote);
    if shares_connection(command) {
        spawn_shared(remote)
    } else {
        remote.spawn()
    }
}

/// The helm-facing half of a shared session: the client process and the
/// pipes the helm reads and writes. The other ends belong to the master task.
fn spawn_shared(mut remote: Command) -> io::Result<Child> {
    let session = remote.spawn()?;
    let group = RemoteGroup(session.id().map(|pid| pid as libc::pid_t));
    let (helm_stdin_reader, helm_stdin) = io::pipe()?;
    let (helm_stdout, helm_stdout_writer) = io::pipe()?;
    let (helm_stderr, helm_stderr_writer) = io::pipe()?;
    // The client learns the exit status through `status` and announces its
    // own death by closing `control`, the way a mux client's control socket
    // closes when it dies. Only the client may hold `control`'s write end, so
    // the `Command` that passed it is dropped before anything waits on it.
    let (status_reader, status) = io::pipe()?;
    let (control, control_writer) = io::pipe()?;
    let mut client = {
        let mut client = Command::new("sh");
        client
            .args(["-c", r#"read status; exit "${status:-255}""#])
            .stdin(Stdio::from(status_reader))
            .stdout(Stdio::from(control_writer))
            .stderr(Stdio::null())
            .kill_on_drop(true);
        isolate_process_group(&mut client);
        client.spawn()?
    };
    client.stdin = Some(ChildStdin::from_std(std::process::ChildStdin::from(
        OwnedFd::from(helm_stdin),
    ))?);
    client.stdout = Some(ChildStdout::from_std(std::process::ChildStdout::from(
        OwnedFd::from(helm_stdout),
    ))?);
    client.stderr = Some(ChildStderr::from_std(std::process::ChildStderr::from(
        OwnedFd::from(helm_stderr),
    ))?);
    let master = Master {
        session,
        group,
        helm_stdin: pipe::Receiver::from_owned_fd(helm_stdin_reader.into())?,
        helm_stdout: pipe::Sender::from_owned_fd(helm_stdout_writer.into())?,
        helm_stderr: pipe::Sender::from_owned_fd(helm_stderr_writer.into())?,
        control: pipe::Receiver::from_owned_fd(control.into())?,
        status,
    };
    tokio::spawn(master.run());
    Ok(client)
}

/// The master's side of one session: the remote command and the far ends of
/// the helm's pipes, plus the client's status and control pipes. A task owns
/// it for the session's life.
struct Master {
    session: Child,
    group: RemoteGroup,
    helm_stdin: pipe::Receiver,
    helm_stdout: pipe::Sender,
    helm_stderr: pipe::Sender,
    control: pipe::Receiver,
    status: io::PipeWriter,
}

impl Master {
    async fn run(mut self) {
        // Dropped last, whether `run` finishes or its task is dropped.
        let _group = self.group;
        let session_stdin = self.session.stdin.take().expect("piped session stdin");
        let session_stdout = self.session.stdout.take().expect("piped session stdout");
        let session_stderr = self.session.stderr.take().expect("piped session stderr");
        let (gone_tx, gone) = watch::channel(false);
        let mut control = self.control;
        tokio::spawn(async move {
            // Any outcome means the client no longer holds the write end:
            // it exited, normally or not.
            let _ = control.read(&mut [0; 1]).await;
            let _ = gone_tx.send(true);
        });
        let input = tokio::spawn(forward_input(self.helm_stdin, session_stdin, gone.clone()));
        let (exited_tx, exited) = watch::channel(false);
        let (over_tx, over) = watch::channel(false);
        let signals = Signals { gone, exited, over };
        // Each forwarder drops its sender once it is done with the remote's
        // stream, at EOF or when it closes the stream early.
        let (stdout_done_tx, stdout_done) = oneshot::channel::<()>();
        let (stderr_done_tx, stderr_done) = oneshot::channel::<()>();
        let session = &mut self.session;
        let (exit, (), ()) = tokio::join!(
            async {
                let exit = session.wait().await;
                let _ = exited_tx.send(true);
                let _ = stdout_done.await;
                let _ = stderr_done.await;
                // The remote has exited and its output reached EOF: a live
                // client's session is over.
                let _ = over_tx.send(true);
                exit
            },
            forward_output(
                session_stdout,
                self.helm_stdout,
                signals.clone(),
                AfterDeath::Close,
                stdout_done_tx,
            ),
            // This is the pipe both #3005 and #3006 waited on.
            forward_output(
                session_stderr,
                self.helm_stderr,
                signals,
                AfterDeath::Discard,
                stderr_done_tx,
            ),
        );
        // The forwarders have closed the helm's output ends. Close its stdin
        // too, and only then let a live client exit, so the helm never sees
        // the exit before the pipes close.
        input.abort();
        let _ = input.await;
        let code = exit.ok().and_then(|status| status.code()).unwrap_or(255);
        // A client that is already gone cannot read this; that is fine.
        let _ = io::Write::write_all(&mut self.status, format!("{code}\n").as_bytes());
    }
}

/// The remote command's process group, killed when the master is done with
/// it. By then the remote itself has normally exited and been reaped; this
/// ends any helper it left behind, which in production would live on the
/// remote host and here would outlive the test.
///
/// A pid is not reused while a process group with that id still has
/// members, so the kill can only miss, never hit another test's group,
/// unless the group emptied and the id was reused within the moment between
/// the session's end and this drop.
struct RemoteGroup(Option<libc::pid_t>);

impl Drop for RemoteGroup {
    fn drop(&mut self) {
        if let Some(group) = self.0 {
            // SAFETY: killpg takes plain integers; a missing group is ESRCH.
            unsafe {
                libc::killpg(group, libc::SIGKILL);
            }
        }
    }
}

/// Copy the helm's stdin to the remote command until the helm closes it or
/// the client dies, then drop both ends: the remote reads EOF, and a helm
/// still writing gets EPIPE.
///
/// When the remote closes its stdin first, the helm's end stays open and
/// unread until the client dies or the master aborts this task at the end of
/// the session, so the helm's writes block and then fail, as over real ssh.
async fn forward_input(
    mut helm: pipe::Receiver,
    mut session: tokio::process::ChildStdin,
    mut gone: watch::Receiver<bool>,
) {
    let mut buffer = vec![0; 16 * 1024];
    loop {
        let count = tokio::select! {
            () = raised(&mut gone) => return,
            read = helm.read(&mut buffer) => match read {
                Ok(0) | Err(_) => return,
                Ok(count) => count,
            },
        };
        tokio::select! {
            () = raised(&mut gone) => return,
            written = session.write_all(&buffer[..count]) => if written.is_err() { break },
        }
    }
    raised(&mut gone).await;
}

/// The session events a forwarder reacts to. Each is a flag that only ever
/// turns on.
#[derive(Clone)]
struct Signals {
    /// The client has died.
    gone: watch::Receiver<bool>,
    /// The remote command itself has exited; helpers it left may live on.
    exited: watch::Receiver<bool>,
    /// The remote command has exited and both its output streams have
    /// reached EOF: for a live client, the session is over.
    over: watch::Receiver<bool>,
}

/// Resolves once `flag` has turned on.
async fn raised(flag: &mut watch::Receiver<bool>) {
    // An error means the sender is gone, which only happens after it
    // raised the flag or while the runtime shuts down.
    let _ = flag.wait_for(|raised| *raised).await;
}

/// What the master does with one of the remote's output streams once the
/// client has died (see the module docs).
#[derive(Clone, Copy)]
enum AfterDeath {
    /// stdout: close both ends at once, whatever the remote is doing.
    Close,
    /// stderr: deliver what is already in the pipe, then read and discard the
    /// rest until the remote's end reaches EOF, and close the helm's end as
    /// soon as the remote command exits (at once, if it already has).
    Discard,
}

/// Copy one of the remote command's output streams to the helm, and close
/// the helm's end when the session ends for it: once the session is over
/// while the client lives, or as `after_death` says once it has died.
/// `done` is dropped as soon as this is finished with the remote's stream.
///
/// A helm that closes its read end early stops nothing: the remote's output
/// is then drained and discarded, so the remote never blocks on a full pipe.
/// That is not measured against real ssh; no test depends on it.
async fn forward_output<R>(
    mut session: R,
    helm: pipe::Sender,
    mut signals: Signals,
    after_death: AfterDeath,
    done: oneshot::Sender<()>,
) where
    R: AsyncRead + AsRawFd + Unpin,
{
    let mut helm = Some(helm);
    let mut done = Some(done);
    // Whether the remote's end of the stream is still open.
    let mut open = true;
    let mut buffer = vec![0; 16 * 1024];
    // The part of `buffer` read from the remote but not yet delivered when
    // the client died.
    let mut unsent = 0..0;
    'live: loop {
        let count = tokio::select! {
            () = raised(&mut signals.gone) => break,
            () = raised(&mut signals.over) => return,
            read = session.read(&mut buffer), if open => match read {
                Ok(0) | Err(_) => {
                    open = false;
                    drop(done.take());
                    continue;
                }
                Ok(count) => count,
            },
        };
        let Some(writer) = helm.as_mut() else {
            continue;
        };
        let mut sent = 0;
        while sent < count {
            // A helm that stops reading fills the pipe and parks this write;
            // the client's death must still take effect, as in the master.
            tokio::select! {
                () = raised(&mut signals.gone) => {
                    unsent = sent..count;
                    break 'live;
                }
                written = writer.write(&buffer[sent..count]) => match written {
                    Ok(written @ 1..) => sent += written,
                    _ => {
                        helm = None;
                        break;
                    }
                },
            }
        }
    }
    // The client has died. Returning drops `session`, which closes the
    // remote's end of the stream if it is still open.
    if let AfterDeath::Close = after_death {
        return;
    }
    if let Some(writer) = helm.as_ref() {
        write_without_waiting(writer.as_raw_fd(), &buffer[unsent]);
        if open {
            deliver_already_written(session.as_raw_fd(), writer.as_raw_fd());
        }
    }
    loop {
        tokio::select! {
            () = raised(&mut signals.exited), if helm.is_some() => helm = None,
            read = session.read(&mut buffer), if open => if !matches!(read, Ok(1..)) {
                open = false;
                drop(done.take());
            },
            else => return,
        }
    }
}

/// Move whatever is readable on `from` right now to `to`, without waiting on
/// either: bytes that do not fit in `to` are dropped. Both are pipes tokio
/// has already made non-blocking.
///
/// This uses the raw descriptors because tokio's own non-blocking reads go by
/// readiness it has cached, which can lag bytes already in the pipe, and the
/// point here is that every byte written before the client's death counts.
fn deliver_already_written(from: RawFd, to: RawFd) {
    let mut buffer = [0u8; 16 * 1024];
    loop {
        // SAFETY: `buffer` is valid for its length, and `from` is a live
        // descriptor the caller owns for the duration of the call.
        let count = unsafe { libc::read(from, buffer.as_mut_ptr().cast(), buffer.len()) };
        let Ok(count @ 1..) = usize::try_from(count) else {
            return;
        };
        if !write_without_waiting(to, &buffer[..count]) {
            return;
        }
    }
}

/// Write all of `bytes` to the non-blocking pipe `to` unless it is full or
/// closed, and say whether all of it went.
fn write_without_waiting(to: RawFd, mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        // SAFETY: `bytes` is valid for its length, and `to` is a live
        // descriptor the caller owns for the duration of the call.
        let wrote = unsafe { libc::write(to, bytes.as_ptr().cast(), bytes.len()) };
        let Ok(wrote @ 1..) = usize::try_from(wrote) else {
            return false;
        };
        bytes = &bytes[wrote..];
    }
    true
}

#[cfg(test)]
mod tests {
    //! These pin the stand-in to the measurements in the module docs. A
    //! change here is a change to what every provisioning test believes
    //! about ssh, so it needs new measurements, not just new expectations.

    use super::*;
    use std::{path::Path, time::Duration};
    use tokio::io::AsyncReadExt;

    /// A helm-shaped ssh command: what `shares_connection` reads is the
    /// program and the `ControlMaster` option, as production builds them.
    fn ssh(control_master: &str) -> Command {
        let mut command = Command::new("ssh");
        command.args([
            "-o",
            &format!("ControlMaster={control_master}"),
            "--",
            "host.example",
            "ignored",
        ]);
        command
    }

    fn remote(script: &str) -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", script]);
        command
    }

    fn quote(path: &Path) -> String {
        shell_words::quote(path.to_str().unwrap()).into_owned()
    }

    /// Read a stream to EOF, failing the test past `bound`.
    async fn read_to_eof(stream: &mut (impl AsyncRead + Unpin), bound: Duration) -> String {
        let mut text = String::new();
        tokio::time::timeout(bound, stream.read_to_string(&mut text))
            .await
            .expect("the stream reaches EOF within its bound")
            .expect("read the stream");
        text
    }

    /// Wait for a marker file the remote creates once it reaches a point the
    /// test needs.
    async fn wait_for_marker(path: &Path) {
        assert!(
            super::super::tests::wait_for_fixture_file(path, Duration::from_secs(10)).await,
            "the remote creates {}",
            path.display()
        );
    }

    /// Assert that `stream` neither yields bytes nor reaches EOF within a
    /// short observation window.
    async fn assert_still_open(stream: &mut (impl AsyncRead + Unpin), what: &str) {
        let mut byte = [0; 1];
        let read = tokio::time::timeout(Duration::from_millis(300), stream.read(&mut byte)).await;
        assert!(read.is_err(), "{what} stays open and silent: {read:?}");
    }

    /// Why: every provisioning stand-in decides shared or unshared from this,
    /// so a production change to the option spelling must not silently turn
    /// every test back into the unshared approximation.
    ///
    /// Spec: an `ssh` command with `ControlMaster=auto` is shared; one with
    /// `ControlMaster=no` and any non-ssh program are not; and the argv
    /// production builds for a provisioning command from a [`control_root`]
    /// is shared.
    #[farhelm_testtrace::test]
    fn sharing_follows_the_production_argv() {
        assert!(shares_connection(&ssh("auto")));
        assert!(!shares_connection(&ssh("no")));
        assert!(!shares_connection(&Command::new("sh")));
        let root = control_root();
        let backend = super::super::backend::SystemBackend::new(root.path().to_path_buf());
        let production = backend
            .ssh_command(
                "host.example",
                crate::ssh::SshAccessKind::UserConfig,
                "true".to_string(),
            )
            .expect("build a provisioning ssh command");
        assert!(
            shares_connection(&production),
            "{:?}",
            production.as_std().get_args().collect::<Vec<_>>()
        );
    }

    /// Why: tests read the remote command's status through the stand-in's
    /// exit code, and production classifies failures by it (255 is a
    /// connection failure there).
    ///
    /// Spec: over a shared connection, the client exits with the remote's
    /// status once the remote has exited and its output reached EOF, and
    /// with 255 when a signal ended the remote.
    #[farhelm_testtrace::test]
    async fn a_live_client_exits_with_the_remote_status() {
        let mut child =
            spawn_remote(&ssh("auto"), remote("printf out; printf err >&2; exit 3")).unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        assert_eq!(
            read_to_eof(&mut stdout, Duration::from_secs(10)).await,
            "out"
        );
        assert_eq!(
            read_to_eof(&mut stderr, Duration::from_secs(10)).await,
            "err"
        );
        assert_eq!(child.wait().await.unwrap().code(), Some(3));

        let mut child = spawn_remote(&ssh("auto"), remote("kill -9 $$")).unwrap();
        assert_eq!(child.wait().await.unwrap().code(), Some(255));
    }

    /// Why: over real ssh, a remote that closes a pipe early does not close
    /// the helm's end; the helm sees every EOF, and its writes fail, only
    /// when the remote exits. A stand-in that closed them early would hide
    /// helm code that waits on the wrong one.
    ///
    /// Spec: with a live client, a remote that closes its stdin, stdout and
    /// stderr and keeps running leaves both output pipes open and silent and
    /// the client running; the helm's input is not refused while the remote
    /// runs, even after the master has found the remote's stdin closed; once
    /// the remote exits, both outputs reach EOF, the client exits with the
    /// remote's status, and a helm write fails.
    #[farhelm_testtrace::test]
    async fn a_live_client_holds_every_pipe_until_the_remote_exits() {
        let root = tempfile::tempdir().unwrap();
        let closed = root.path().join("closed");
        let release = root.path().join("release");
        let script = format!(
            "exec <&- >&- 2>&-; : > {closed}; \
             while [ ! -e {release} ]; do sleep 0.01; done; exit 4",
            closed = quote(&closed),
            release = quote(&release),
        );
        let mut child = spawn_remote(&ssh("auto"), remote(&script)).unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        wait_for_marker(&closed).await;

        stdin
            .write_all(b"accepted while the remote runs\n")
            .await
            .expect("the helm's input is not refused while the remote runs");
        assert_still_open(&mut stdout, "stdout").await;
        assert_still_open(&mut stderr, "stderr").await;
        // By now the master has tried to pass the first write on and found
        // the remote's stdin closed; a later write must still not fail.
        stdin
            .write_all(b"still accepted\n")
            .await
            .expect("the helm's input is not refused after the remote closed its stdin");
        assert!(child.try_wait().unwrap().is_none(), "the client is alive");

        std::fs::write(&release, b"").unwrap();
        assert_eq!(read_to_eof(&mut stdout, Duration::from_secs(10)).await, "");
        assert_eq!(read_to_eof(&mut stderr, Duration::from_secs(10)).await, "");
        assert_eq!(child.wait().await.unwrap().code(), Some(4));
        assert!(
            stdin.write_all(b"late\n").await.is_err(),
            "input fails once the session is over"
        );
    }

    /// Why: a helper the remote leaves in the background holds its output,
    /// and real ssh does not exit until that output closes. That is the
    /// held-pipe case the helm's bounded joins exist for.
    ///
    /// Spec: with a live client, when the remote exits but a background
    /// helper still holds stdout, the client keeps running; once the helper
    /// writes and exits, stdout carries both writes, and the client exits
    /// with the remote's own status.
    #[farhelm_testtrace::test]
    async fn a_live_client_waits_for_a_helper_holding_output() {
        let root = tempfile::tempdir().unwrap();
        let helper = root.path().join("helper");
        let release = root.path().join("release");
        let script = format!(
            "(while kill -0 $$ 2>/dev/null; do sleep 0.01; done; : > {helper}; \
              while [ ! -e {release} ]; do sleep 0.01; done; printf helper) & \
             printf 'main '; exit 2",
            helper = quote(&helper),
            release = quote(&release),
        );
        let mut child = spawn_remote(&ssh("auto"), remote(&script)).unwrap();
        let mut stdout = child.stdout.take().unwrap();
        // The helper marks only once the remote's own shell is gone (and
        // reaped, which the master does at once).
        wait_for_marker(&helper).await;
        assert!(child.try_wait().unwrap().is_none(), "the client waits");

        std::fs::write(&release, b"").unwrap();
        assert_eq!(
            read_to_eof(&mut stdout, Duration::from_secs(10)).await,
            "main helper"
        );
        assert_eq!(child.wait().await.unwrap().code(), Some(2));
    }

    /// Why: this is the behavior behind #3005 and #3006, and the reason this
    /// module exists. A test helm that kills its ssh process must see the
    /// stderr pipe a real shared connection leaves open, and only what was
    /// written on it before the kill.
    ///
    /// Spec: after the client is killed with the remote still running and
    /// the helm still holding stdin, stdout reaches EOF promptly, the remote
    /// reads EOF on its stdin and keeps running, and a helm write fails.
    /// stderr stays open until the remote exits and carries nothing the
    /// remote writes after the kill, and the remote's own later write to
    /// stdout fails.
    #[farhelm_testtrace::test]
    async fn killing_the_client_leaves_stderr_with_the_remote() {
        let root = tempfile::tempdir().unwrap();
        let eof = root.path().join("eof");
        let release = root.path().join("release");
        let stdout_write = root.path().join("stdout-write");
        // The remote ignores SIGPIPE so that a failed stdout write is
        // recorded instead of ending it.
        let script = format!(
            "trap '' PIPE; printf 'before\\n' >&2; cat >/dev/null; : > {eof}; \
             while [ ! -e {release} ]; do sleep 0.01; done; printf 'after\\n' >&2; \
             if printf 'late\\n'; then r=delivered; else r=failed; fi; \
             echo $r > {stdout_write}.tmp; mv {stdout_write}.tmp {stdout_write}",
            eof = quote(&eof),
            release = quote(&release),
            stdout_write = quote(&stdout_write),
        );
        let mut child = spawn_remote(&ssh("auto"), remote(&script)).unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let mut line = [0; 7];
        stderr.read_exact(&mut line).await.unwrap();
        assert_eq!(&line, b"before\n", "premise: the remote is running");

        super::super::backend::terminate_child(&mut child).await;

        assert_eq!(read_to_eof(&mut stdout, Duration::from_secs(10)).await, "");
        wait_for_marker(&eof).await;
        // The master closes its read end as soon as it notices the client
        // died; give the write a bound rather than assuming it already has.
        tokio::time::timeout(Duration::from_secs(10), async {
            while stdin.write_all(b"late\n").await.is_ok() {
                // sleep-ok: retry interval while the master notices the death.
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("a write after the client died fails");
        assert_still_open(&mut stderr, "stderr while the remote runs").await;

        std::fs::write(&release, b"").unwrap();
        assert_eq!(read_to_eof(&mut stderr, Duration::from_secs(10)).await, "");
        wait_for_marker(&stdout_write).await;
        assert_eq!(
            std::fs::read_to_string(&stdout_write).unwrap(),
            "failed\n",
            "the remote's stdout is closed after the kill"
        );
    }

    /// Why: after a kill, real ssh closes stdout at once even if the remote
    /// closed its own end earlier. A stand-in that kept the helm's end until
    /// the session ended would make a helm read wait where ssh would not.
    ///
    /// Spec: when the client is killed while the remote, having closed its
    /// stdout, still runs, the helm's stdout reaches EOF at once.
    #[farhelm_testtrace::test]
    async fn killing_the_client_closes_stdout_the_remote_already_closed() {
        let root = tempfile::tempdir().unwrap();
        let closed = root.path().join("closed");
        let release = root.path().join("release");
        let script = format!(
            "exec >&-; : > {closed}; while [ ! -e {release} ]; do sleep 0.01; done",
            closed = quote(&closed),
            release = quote(&release),
        );
        let mut child = spawn_remote(&ssh("auto"), remote(&script)).unwrap();
        let mut stdout = child.stdout.take().unwrap();
        wait_for_marker(&closed).await;
        assert_still_open(&mut stdout, "premise: stdout while the client lives").await;

        super::super::backend::terminate_child(&mut child).await;

        // The remote runs until released, so only the kill can end stdout.
        assert_eq!(read_to_eof(&mut stdout, Duration::from_secs(10)).await, "");
        std::fs::write(&release, b"").unwrap();
    }

    /// Why: after a kill, real ssh closes stderr when the remote command
    /// itself exits, not when the last process holding it does. A stand-in
    /// that waited for helpers would hold stderr longer than ssh does.
    ///
    /// Spec: when the client is killed after the remote has exited, while a
    /// background helper still holds the remote's stdout and stderr, both
    /// reach EOF at once.
    #[farhelm_testtrace::test]
    async fn killing_the_client_after_the_remote_exited_ignores_its_helper() {
        let root = tempfile::tempdir().unwrap();
        let gone = root.path().join("gone");
        let release = root.path().join("release");
        let script = format!(
            "(while kill -0 $$ 2>/dev/null; do sleep 0.01; done; : > {gone}; \
              while [ ! -e {release} ]; do sleep 0.01; done) & exit 0",
            gone = quote(&gone),
            release = quote(&release),
        );
        let mut child = spawn_remote(&ssh("auto"), remote(&script)).unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        // The helper marks only once the remote's own shell is gone.
        wait_for_marker(&gone).await;
        assert!(
            child.try_wait().unwrap().is_none(),
            "premise: the helper's pipes keep the client waiting"
        );

        super::super::backend::terminate_child(&mut child).await;

        // The helper runs until released, so only the kill can end these.
        assert_eq!(read_to_eof(&mut stdout, Duration::from_secs(10)).await, "");
        assert_eq!(read_to_eof(&mut stderr, Duration::from_secs(10)).await, "");
        std::fs::write(&release, b"").unwrap();
    }

    /// Why: the other half of the rule above. While the remote command
    /// still runs, stderr must outlive the kill (the pipe #3005 and #3006
    /// waited on), but only until the remote exits.
    ///
    /// Spec: when the client is killed while the remote runs and a helper
    /// also holds stderr, stderr stays open; once the remote exits, it
    /// reaches EOF, though the helper still runs.
    #[farhelm_testtrace::test]
    async fn killing_the_client_ends_stderr_with_the_remote_not_its_helper() {
        let root = tempfile::tempdir().unwrap();
        let helper = root.path().join("helper");
        let exit = root.path().join("exit");
        let release = root.path().join("release");
        let script = format!(
            "(: > {helper}; while [ ! -e {release} ]; do sleep 0.01; done) & \
             while [ ! -e {exit} ]; do sleep 0.01; done",
            helper = quote(&helper),
            exit = quote(&exit),
            release = quote(&release),
        );
        let mut child = spawn_remote(&ssh("auto"), remote(&script)).unwrap();
        let mut stderr = child.stderr.take().unwrap();
        wait_for_marker(&helper).await;

        super::super::backend::terminate_child(&mut child).await;

        assert_still_open(&mut stderr, "stderr while the remote runs").await;
        std::fs::write(&exit, b"").unwrap();
        // The helper runs until released, so the remote's exit ends stderr.
        assert_eq!(read_to_eof(&mut stderr, Duration::from_secs(10)).await, "");
        std::fs::write(&release, b"").unwrap();
    }

    /// Why: the unshared fallback keeps the older stand-in's behavior, and
    /// tests of that path must not get the shared one by accident.
    ///
    /// Spec: with `ControlMaster=no`, the returned child is the remote
    /// command itself, and killing it closes stderr at once.
    #[farhelm_testtrace::test]
    async fn an_unshared_command_is_the_remote_itself() {
        let mut child =
            spawn_remote(&ssh("no"), remote("printf 'up\\n' >&2; exec sleep 60")).unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let mut line = [0; 3];
        stderr.read_exact(&mut line).await.unwrap();
        super::super::backend::terminate_child(&mut child).await;
        assert_eq!(read_to_eof(&mut stderr, Duration::from_secs(10)).await, "");
    }
}
