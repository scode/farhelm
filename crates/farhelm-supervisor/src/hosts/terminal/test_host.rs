//! A stand-in terminal host for the supervisor's own tests of its terminal
//! switch points: a listener in the test process that speaks just enough of
//! the protocol for a [`HostTerminal`] to adopt it, and that a test drives
//! through a [`TestHost`] (output, the child's exit, becoming unreadable).
//!
//! The runtime's own tests have a stand-in of their own
//! (`hosts::terminal::tests`), which plays each record and acknowledgement
//! by hand because what they test is the runtime's reaction to exact frame
//! orders. The service's tests need the opposite: a host that behaves the
//! way a healthy one does on its own, so that a test can be about what the
//! supervisor does with a host terminal rather than about the protocol.
//!
//! The host's child is a real process the [`TestHost`] owns (a `sleep`),
//! which its `Info` names, so the runtime pins a live process that is the
//! test's own: a verb that kills from the terminal's root (Stop, Delete)
//! reaches that `sleep` and nothing else, and a test can end it to withdraw
//! the root without the host saying so ([`TestHost::kill_child`]). The host
//! itself, and so its peer process, is the test runner, which nothing here
//! signals. The full lifecycle against real hosts is in the e2e suite
//! (`crates/farhelm/tests/e2e/host_terminals.rs`).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use terminal_host::protocol::{
    ChildExit, ClientMessage, HostMessage, Info, MAX_FRAME, Position, Size, decode_client,
    encode_host, split_frame,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

use super::{Adoption, HostTerminal, HostTerminalOptions, LivenessNotifier};
use crate::hosts::client::ClientDeadlines;

/// How long adopting a test host may take before the test fails.
const ADOPT_WAIT: Duration = Duration::from_secs(10);

/// What a test asks its host to do.
enum Command {
    /// Send output, as the child printing it.
    Output(Vec<u8>),
    /// Send the child's exit record with this status.
    Exit(i32),
    /// End the connection and answer every later one with a protocol
    /// version no supervisor speaks, so that the runtime finds its host
    /// unreadable when it reconnects.
    BecomeUnreadable,
}

/// A running stand-in host. Dropping it ends the host's task and removes
/// its socket's directory.
pub(crate) struct TestHost {
    socket: PathBuf,
    /// The child the host's `Info` names, until the test kills it.
    child: Mutex<Option<std::process::Child>>,
    child_pid: u32,
    /// Every input byte the host acknowledged as written, in order.
    received: Arc<Mutex<Vec<u8>>>,
    commands: mpsc::UnboundedSender<Command>,
    task: tokio::task::JoinHandle<()>,
    _dir: tempfile::TempDir,
}

impl Drop for TestHost {
    fn drop(&mut self) {
        self.task.abort();
        self.kill_child();
    }
}

impl TestHost {
    /// The host's socket.
    pub(crate) fn socket(&self) -> &Path {
        &self.socket
    }

    /// The pid of the child the host names.
    pub(crate) fn child_pid(&self) -> u32 {
        self.child_pid
    }

    /// End the child and reap it, without the host reporting anything: the
    /// runtime's root is withdrawn (its pinned process is gone) while no
    /// exit record has arrived, the window in which a real host has reaped
    /// its child and is still draining its output.
    pub(crate) fn kill_child(&self) {
        let child = self.child.lock().expect("the child").take();
        if let Some(mut child) = child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Every input byte the host has acknowledged as written to its child,
    /// in order.
    pub(crate) fn received(&self) -> Vec<u8> {
        self.received.lock().expect("the input record").clone()
    }

    /// Have the child print `data`.
    pub(crate) fn output(&self, data: &[u8]) {
        let _ = self.commands.send(Command::Output(data.to_vec()));
    }

    /// Have the child exit with `code`, which the host reports after the
    /// output sent before.
    pub(crate) fn exit(&self, code: i32) {
        let _ = self.commands.send(Command::Exit(code));
    }

    /// Make the host unreadable to its runtime: the connection ends, and
    /// the reconnect finds a protocol version it does not speak, which
    /// makes the terminal unavailable (`Liveness::Unavailable`) while the
    /// host, as far as anyone can tell, may still run.
    pub(crate) fn become_unreadable(&self) {
        let _ = self.commands.send(Command::BecomeUnreadable);
    }
}

/// Start a stand-in host and adopt it as freshly spawned, with `notifier`
/// for its liveness changes. Fails the test when the adoption does not
/// succeed in time.
pub(crate) async fn adopt_test_host(
    notifier: Option<LivenessNotifier>,
) -> (HostTerminal, TestHost) {
    let dir = tempfile::tempdir().expect("a directory for the test host's socket");
    let socket = dir.path().join("h");
    let listener = UnixListener::bind(&socket).expect("bind the test host's socket");
    // Bounded, so that a test process killed before its drop leaves the
    // child behind for a minute at most; every test using a test host
    // finishes well within that.
    let child = std::process::Command::new("sleep")
        .arg("60")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("start the test host's child");
    let child_pid = child.id();
    let (commands, receiver) = mpsc::unbounded_channel();
    let received = Arc::new(Mutex::new(Vec::new()));
    let task = tokio::spawn(serve(listener, receiver, Arc::clone(&received), child_pid));
    let options = HostTerminalOptions {
        notifier,
        // A reconnect to an unreadable host is judged in about a second.
        deadlines: ClientDeadlines {
            connect: Duration::from_secs(1),
            refusal_window: Duration::from_secs(1),
            retry_interval: Duration::from_millis(50),
            reply: Duration::from_secs(5),
            shutdown: Duration::from_secs(5),
        },
        ..HostTerminalOptions::default()
    };
    let adopted = tokio::time::timeout(
        ADOPT_WAIT,
        HostTerminal::adopt(&socket, Adoption::Fresh, options),
    )
    .await;
    let mut child = child;
    let terminal = match adopted {
        Ok(Ok(terminal)) => terminal,
        failed => {
            // The child is the test's to end, whatever went wrong.
            let _ = child.kill();
            let _ = child.wait();
            panic!("the test host was not adopted: {failed:?}");
        }
    };
    let host = TestHost {
        socket,
        child: Mutex::new(Some(child)),
        child_pid,
        received,
        commands,
        task,
        _dir: dir,
    };
    (terminal, host)
}

/// The host's side of its connections: one at a time, each served until it
/// ends, with the journal (output offset, exit) kept across them.
async fn serve(
    listener: UnixListener,
    mut commands: mpsc::UnboundedReceiver<Command>,
    received: Arc<Mutex<Vec<u8>>>,
    child_pid: u32,
) {
    let mut journal = Journal {
        child_pid,
        ..Journal::default()
    };
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        if journal.unreadable {
            refuse(stream).await;
            continue;
        }
        if serve_one(stream, &mut journal, &mut commands, &received)
            .await
            .is_err()
        {
            return;
        }
    }
}

/// What the host has journaled, as its `Info` reports it.
#[derive(Default)]
struct Journal {
    /// The child's pid, as `Info` reports it.
    child_pid: u32,
    end: u64,
    exit: Option<ChildExit>,
    unreadable: bool,
    /// Input bytes acknowledged as written, in all.
    written: u64,
}

impl Journal {
    fn info(&self) -> Info {
        let size = Size { cols: 80, rows: 24 };
        Info {
            host_pid: std::process::id(),
            child_pid: self.child_pid,
            child_exit: self.exit,
            size,
            initial_size: size,
            snapshot: Position::default(),
            tail_start: 0,
            end: self.end,
            resizes: 0,
            gap: false,
            delivered_end: 0,
            tag: Vec::new(),
        }
    }
}

/// Serve one connection until it ends or a test makes the host unreadable.
/// `Err` when the test dropped its [`TestHost`].
async fn serve_one(
    mut stream: UnixStream,
    journal: &mut Journal,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    received: &Mutex<Vec<u8>>,
) -> Result<(), ()> {
    let mut buf = Vec::new();
    loop {
        while let Some(frame) = split_frame(&buf, MAX_FRAME).expect("a well-formed frame") {
            let consumed = frame.consumed;
            let message = decode_client(frame, 1).expect("a well-formed message");
            buf.drain(..consumed);
            let reply = match message {
                ClientMessage::Hello { .. } => Some(HostMessage::Welcome { version: 1 }),
                ClientMessage::Info => Some(HostMessage::InfoReply(journal.info())),
                ClientMessage::Input(data) => {
                    journal.written += data.len() as u64;
                    received
                        .lock()
                        .expect("the input record")
                        .extend_from_slice(&data);
                    Some(HostMessage::InputAck {
                        written: journal.written,
                        discarded: 0,
                    })
                }
                ClientMessage::Shutdown => {
                    let _ = stream.write_all(&encode_host(&HostMessage::Goodbye)).await;
                    return Ok(());
                }
                // The stream starts empty, so `Read` has nothing to send;
                // resizes and snapshots are not what these tests are about.
                _ => None,
            };
            if let Some(reply) = reply
                && stream.write_all(&encode_host(&reply)).await.is_err()
            {
                return Ok(());
            }
        }
        tokio::select! {
            read = stream.read_buf(&mut buf) => match read {
                Ok(0) | Err(_) => return Ok(()),
                Ok(_) => {}
            },
            command = commands.recv() => {
                let record = match command.ok_or(())? {
                    Command::Output(data) => {
                        let offset = journal.end;
                        journal.end += data.len() as u64;
                        HostMessage::Output { offset, data }
                    }
                    Command::Exit(code) => {
                        journal.exit = Some(ChildExit::Code(code));
                        HostMessage::Exited {
                            offset: journal.end,
                            exit: ChildExit::Code(code),
                        }
                    }
                    Command::BecomeUnreadable => {
                        journal.unreadable = true;
                        return Ok(());
                    }
                };
                if stream.write_all(&encode_host(&record)).await.is_err() {
                    return Ok(());
                }
            }
        }
    }
}

/// Answer a connection's `Hello` with a protocol version no supervisor
/// speaks.
async fn refuse(mut stream: UnixStream) {
    let mut buf = Vec::new();
    loop {
        if let Ok(Some(frame)) = split_frame(&buf, MAX_FRAME) {
            if decode_client(frame, 1).is_ok() {
                let _ = stream
                    .write_all(&encode_host(&HostMessage::Welcome { version: 99 }))
                    .await;
            }
            return;
        }
        match stream.read_buf(&mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
    }
}
