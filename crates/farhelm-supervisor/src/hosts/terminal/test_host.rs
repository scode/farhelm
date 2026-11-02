//! A stand-in terminal host for the supervisor's own tests of its terminal
//! switch points and grid streams: a listener in the test process that
//! speaks just enough of the protocol for a [`HostTerminal`] to adopt it,
//! applies resizes as a host does (a resize record in the stream), accepts
//! every checkpoint, and that a test drives through a [`TestHost`] (output,
//! the child's exit, becoming unreadable, holding and releasing input,
//! dropping the connection, a restored snapshot).
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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use terminal_host::protocol::{
    ChildExit, ClientMessage, HostMessage, Info, MAX_FRAME, Position, Size, decode_client,
    encode_host, split_frame,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, watch};

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
    /// Send the journaled output (see [`TestHostSpec::journaled`]) up to
    /// this many bytes, from where the last such command stopped.
    Replay(usize),
    /// Stop acknowledging input: keep what arrives, unwritten, as a host
    /// does while its program does not read the terminal; say so once in
    /// force. The first `pass` inputs to arrive are still written, as a
    /// program that stops reading partway through.
    HoldInput {
        pass: usize,
        held: tokio::sync::oneshot::Sender<()>,
    },
    /// Write and acknowledge the input held, and every later input.
    ReleaseInput,
    /// End the connection, as a host whose client was cut off: input held
    /// and not acknowledged is lost with it, as a host drops what a
    /// displaced client sent and it had not acted on.
    DropConnection,
    /// Send a snapshot record of this blob at a position just past the
    /// output sent so far, which the runtime restores (a rebuild).
    Restore(Vec<u8>),
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
    /// How many bytes `received` holds, to wait on.
    received_len: watch::Receiver<usize>,
    /// How many inputs are held unacknowledged (`Command::HoldInput`).
    held: watch::Receiver<usize>,
    commands: mpsc::UnboundedSender<Command>,
    /// Whether the host answers every connection with a protocol version no
    /// supervisor speaks; shared with its task.
    unreadable: Arc<AtomicBool>,
    /// How many connections the host has accepted.
    accepted: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
    /// The socket's own directory, for a host started outside any state
    /// directory; `None` for one placed where the test asked.
    _dir: Option<tempfile::TempDir>,
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

    /// Wait, at most `within`, until the host has acknowledged exactly
    /// `expected` as written; says whether it did.
    pub(crate) async fn received_becomes(&self, expected: &[u8], within: Duration) -> bool {
        let mut len = self.received_len.clone();
        let waited = tokio::time::timeout(within, async {
            while self.received() != expected {
                if len.changed().await.is_err() {
                    return;
                }
            }
        })
        .await;
        waited.is_ok() && self.received() == expected
    }

    /// Have the child print `data`, in records no longer than a host's
    /// (`MAX_OUTPUT_CHUNK`).
    pub(crate) fn output(&self, data: &[u8]) {
        for chunk in data.chunks(terminal_host::protocol::MAX_OUTPUT_CHUNK) {
            let _ = self.commands.send(Command::Output(chunk.to_vec()));
        }
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
        self.unreadable.store(true, Ordering::SeqCst);
        let _ = self.commands.send(Command::BecomeUnreadable);
    }

    /// How many connections the host has accepted so far: what shows that
    /// a caller did, or did not, connect to it.
    pub(crate) fn connections(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }

    /// Stream the output the host journaled before its adoption
    /// ([`TestHostSpec::journaled`]) up to byte `upto`: the replay the
    /// runtime is catching up with, delivered when the test says.
    pub(crate) fn replay_journaled(&self, upto: usize) {
        let _ = self.commands.send(Command::Replay(upto));
    }

    /// Stop acknowledging input, as a host whose program does not read its
    /// terminal: the runtime's input window fills and callers' sends wait.
    /// Returns once the host holds input (it must be serving a connection).
    pub(crate) async fn hold_input(&self) {
        self.hold_input_after(0).await;
    }

    /// [`Self::hold_input`], once the next `pass` inputs have been written
    /// and acknowledged: for a test that needs the start of something
    /// written and the rest held.
    pub(crate) async fn hold_input_after(&self, pass: usize) {
        let (held, holding) = tokio::sync::oneshot::channel();
        let _ = self.commands.send(Command::HoldInput { pass, held });
        let _ = tokio::time::timeout(ADOPT_WAIT, holding).await;
    }

    /// Wait, at most `within`, until the host holds at least `count` inputs
    /// unacknowledged; says whether it did.
    pub(crate) async fn holds(&self, count: usize, within: Duration) -> bool {
        let mut held = self.held.clone();
        tokio::time::timeout(within, held.wait_for(|held| *held >= count))
            .await
            .is_ok_and(|held| held.is_ok())
    }

    /// Write and acknowledge the input held, and every later input.
    pub(crate) fn release_input(&self) {
        let _ = self.commands.send(Command::ReleaseInput);
    }

    /// End the runtime's connection; it reconnects, and input held and not
    /// acknowledged is lost (its fate unknown to the runtime).
    pub(crate) fn drop_connection(&self) {
        let _ = self.commands.send(Command::DropConnection);
    }

    /// Have the runtime restore the snapshot `blob`, as after a stream
    /// that resumed past a gap: a rebuild of its emulator.
    pub(crate) fn restore(&self, blob: Vec<u8>) {
        let _ = self.commands.send(Command::Restore(blob));
    }
}

/// What a stand-in host is started with ([`start_test_host`]).
#[derive(Default)]
pub(crate) struct TestHostSpec {
    /// Where its socket is bound; a fresh private directory's `h` when
    /// `None`. A test of discovery binds it in a state directory's socket
    /// directory, so that a supervisor's scan finds it.
    pub(crate) socket: Option<PathBuf>,
    /// The client tag its `Info` returns.
    pub(crate) tag: Vec<u8>,
    /// Output journaled before any supervisor read it, sent only on
    /// [`TestHost::replay_journaled`].
    pub(crate) journaled: Vec<u8>,
    /// The child's exit status, as reaped before any supervisor connected:
    /// its `Info` says so from the start, while the exit record follows the
    /// journal only on [`TestHost::exit`]. A host whose child exited while
    /// no supervisor was attached, with its replay not yet done.
    pub(crate) exited: Option<i32>,
    /// The options a test's own adoption of it uses.
    pub(crate) options: Option<HostTerminalOptions>,
}

/// The options a test adopts a stand-in host with: `notifier`, and client
/// deadlines short enough that a reconnect to an unreadable host is judged
/// in about a second.
pub(crate) fn test_host_options(notifier: Option<LivenessNotifier>) -> HostTerminalOptions {
    HostTerminalOptions {
        notifier,
        deadlines: ClientDeadlines {
            connect: Duration::from_secs(1),
            refusal_window: Duration::from_secs(1),
            retry_interval: Duration::from_millis(50),
            reply: Duration::from_secs(5),
            shutdown: Duration::from_secs(5),
        },
        ..HostTerminalOptions::default()
    }
}

/// Start a stand-in host as `spec` says, without adopting it: for a test
/// that has the supervisor find it.
pub(crate) fn start_test_host(spec: TestHostSpec) -> TestHost {
    let (socket, dir) = match spec.socket {
        Some(socket) => (socket, None),
        None => {
            let dir = tempfile::tempdir().expect("a directory for the test host's socket");
            (dir.path().join("h"), Some(dir))
        }
    };
    let mut host = start_with(&socket, spec.tag, false, spec.journaled, spec.exited);
    host._dir = dir;
    host
}

/// Start a stand-in host as `spec` says and adopt it as `adoption` says.
/// Fails the test when the adoption does not succeed in time.
pub(crate) async fn adopt_test_host_with(
    mut spec: TestHostSpec,
    adoption: Adoption,
) -> (HostTerminal, TestHost) {
    let options = spec
        .options
        .take()
        .unwrap_or_else(|| test_host_options(None));
    let host = start_test_host(spec);
    let adopted = tokio::time::timeout(
        ADOPT_WAIT,
        HostTerminal::adopt(&host.socket, adoption, options),
    )
    .await;
    match adopted {
        // The child is the test's to end, whatever went wrong: dropping the
        // host does that.
        Ok(Ok(terminal)) => (terminal, host),
        failed => panic!("the test host was not adopted: {failed:?}"),
    }
}

/// Start a stand-in host that journaled `journaled` before any supervisor
/// read it, and adopt it as one that ran before (`Adoption::Recovered`), as
/// a restarted supervisor does. The host's `Info` reports the journal's end
/// from the start, but its stream sends the journaled bytes only when the
/// test calls [`TestHost::replay_journaled`], so the runtime stays short of
/// caught up (`HostStatus::caught_up`) for as long as the test wants: the
/// window after a restart in which the emulator holds a past screen.
pub(crate) async fn adopt_test_host_recovering(
    notifier: Option<LivenessNotifier>,
    journaled: Vec<u8>,
) -> (HostTerminal, TestHost) {
    adopt_test_host_with(
        TestHostSpec {
            journaled,
            options: Some(test_host_options(notifier)),
            ..TestHostSpec::default()
        },
        Adoption::Recovered,
    )
    .await
}

/// Start a stand-in host and adopt it as freshly spawned, with `notifier`
/// for its liveness changes. Fails the test when the adoption does not
/// succeed in time.
pub(crate) async fn adopt_test_host(
    notifier: Option<LivenessNotifier>,
) -> (HostTerminal, TestHost) {
    let dir = tempfile::tempdir().expect("a directory for the test host's socket");
    let socket = dir.path().join("h");
    let (terminal, mut host) = adopt_test_host_at(&socket, Vec::new(), notifier).await;
    host._dir = Some(dir);
    (terminal, host)
}

/// [`adopt_test_host`] with the stand-in serving `socket`, a path the test
/// chose (inside a state directory's socket directory, where a launch
/// record can name it), and carrying `tag` in its `Info`.
pub(crate) async fn adopt_test_host_at(
    socket: &Path,
    tag: Vec<u8>,
    notifier: Option<LivenessNotifier>,
) -> (HostTerminal, TestHost) {
    let host = start(socket, tag, false);
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
        HostTerminal::adopt(socket, Adoption::Fresh, options),
    )
    .await;
    match adopted {
        // The child is the test's to end, whatever went wrong: dropping the
        // host does that.
        Ok(Ok(terminal)) => (terminal, host),
        failed => panic!("the test host was not adopted: {failed:?}"),
    }
}

/// Start a stand-in host serving `socket` (a path the test chose, such as
/// one inside a state directory's socket directory, where a launch record
/// can name it) and carrying `tag` in its `Info`, adopted by nothing. With
/// `unreadable` it answers every connection from the start with a protocol
/// version no supervisor speaks, as a host of a newer release, or one this
/// build cannot read, would.
pub(crate) fn start_test_host_at(socket: &Path, tag: Vec<u8>, unreadable: bool) -> TestHost {
    start(socket, tag, unreadable)
}

/// Bind `socket`, start the host's child and serve the socket.
fn start(socket: &Path, tag: Vec<u8>, unreadable: bool) -> TestHost {
    start_with(socket, tag, unreadable, Vec::new(), None)
}

/// [`start`], with output `journaled` before any reader (streamed only on
/// [`TestHost::replay_journaled`]) and the child already reaped with
/// `exited`, as [`TestHostSpec`] describes.
fn start_with(
    socket: &Path,
    tag: Vec<u8>,
    unreadable: bool,
    journaled: Vec<u8>,
    exited: Option<i32>,
) -> TestHost {
    let listener = UnixListener::bind(socket).expect("bind the test host's socket");
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
    let (received_len_tx, received_len) = watch::channel(0);
    let (held_tx, held) = watch::channel(0);
    let unreadable = Arc::new(AtomicBool::new(unreadable));
    let accepted = Arc::new(AtomicUsize::new(0));
    let journal = Journal {
        child_pid,
        tag,
        unreadable: Arc::clone(&unreadable),
        end: journaled.len() as u64,
        journaled,
        exit: exited.map(ChildExit::Code),
        ..Journal::default()
    };
    let task = tokio::spawn(serve(
        listener,
        receiver,
        Arc::clone(&received),
        Records {
            received_len: received_len_tx,
            held: held_tx,
        },
        journal,
        Arc::clone(&accepted),
    ));
    TestHost {
        socket: socket.to_path_buf(),
        child: Mutex::new(Some(child)),
        child_pid,
        received,
        received_len,
        held,
        commands,
        unreadable,
        accepted,
        task,
        _dir: None,
    }
}

/// The host's side of its connections: one at a time, each served until it
/// ends, with the journal (output offset, exit) kept across them.
async fn serve(
    listener: UnixListener,
    mut commands: mpsc::UnboundedReceiver<Command>,
    received: Arc<Mutex<Vec<u8>>>,
    records: Records,
    mut journal: Journal,
    accepted: Arc<AtomicUsize>,
) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        accepted.fetch_add(1, Ordering::SeqCst);
        if journal.unreadable.load(Ordering::SeqCst) {
            refuse(stream).await;
            continue;
        }
        if serve_one(stream, &mut journal, &mut commands, &received, &records)
            .await
            .is_err()
        {
            return;
        }
    }
}

/// The size a test host starts at.
pub(crate) const TEST_HOST_SIZE: Size = Size { cols: 80, rows: 24 };

/// What the host has journaled, as its `Info` reports it.
struct Journal {
    /// The child's pid, as `Info` reports it.
    child_pid: u32,
    end: u64,
    exit: Option<ChildExit>,
    /// Shared with the [`TestHost`], so the test can make the host
    /// unreadable while no connection is being served.
    unreadable: Arc<AtomicBool>,
    /// The client tag `Info` reports.
    tag: Vec<u8>,
    /// Input bytes acknowledged as written on the current connection:
    /// acknowledgements count a connection's input, from 0 on each.
    written: u64,
    /// Output journaled before the adoption, at offset 0, which the stream
    /// sends only on [`Command::Replay`].
    journaled: Vec<u8>,
    /// How much of `journaled` the stream has sent.
    replayed: usize,
    /// The size in force: the last resize's, or the initial one.
    size: Size,
    /// Resizes applied so far.
    resizes: u64,
    /// While input is held (`Command::HoldInput`), what arrived, unwritten.
    held: Option<Vec<Vec<u8>>>,
    /// While input is held, how many more inputs are written all the same.
    pass: usize,
}

impl Default for Journal {
    fn default() -> Self {
        Journal {
            child_pid: 0,
            end: 0,
            exit: None,
            unreadable: Arc::default(),
            tag: Vec::new(),
            written: 0,
            journaled: Vec::new(),
            replayed: 0,
            size: TEST_HOST_SIZE,
            resizes: 0,
            held: None,
            pass: 0,
        }
    }
}

impl Journal {
    fn info(&self) -> Info {
        Info {
            host_pid: std::process::id(),
            child_pid: self.child_pid,
            child_exit: self.exit,
            size: self.size,
            initial_size: TEST_HOST_SIZE,
            snapshot: Position::default(),
            tail_start: 0,
            end: self.end,
            resizes: self.resizes,
            gap: false,
            delivered_end: 0,
            tag: self.tag.clone(),
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
    records: &Records,
) -> Result<(), ()> {
    journal.written = 0;
    let mut buf = Vec::new();
    loop {
        while let Some(frame) = split_frame(&buf, MAX_FRAME).expect("a well-formed frame") {
            let consumed = frame.consumed;
            let message = decode_client(frame, 1).expect("a well-formed message");
            buf.drain(..consumed);
            let reply = match message {
                ClientMessage::Hello { .. } => Some(HostMessage::Welcome { version: 1 }),
                ClientMessage::Info => Some(HostMessage::InfoReply(journal.info())),
                ClientMessage::Input(data) => match &mut journal.held {
                    Some(held) if journal.pass == 0 => {
                        held.push(data);
                        records.held.send_replace(held.len());
                        None
                    }
                    held => {
                        if held.is_some() {
                            journal.pass -= 1;
                        }
                        Some(write_input(journal, received, &records.received_len, &data))
                    }
                },
                ClientMessage::Shutdown => {
                    let _ = stream.write_all(&encode_host(&HostMessage::Goodbye)).await;
                    return Ok(());
                }
                // Applied at once, as a resize record at the current end of
                // the output, which is where a host puts it.
                ClientMessage::Resize(size) => {
                    journal.size = size;
                    journal.resizes += 1;
                    Some(HostMessage::Resized {
                        offset: journal.end,
                        index: journal.resizes - 1,
                        size,
                    })
                }
                // Every checkpoint is accepted (and its blob dropped: the
                // stand-in never resumes a stream from one), so that a test
                // writing megabytes is not cut off waiting for the answer.
                ClientMessage::Snapshot { position, .. } => {
                    Some(HostMessage::SnapshotAccepted(position))
                }
                // `Read` sends nothing itself: what was journaled before
                // the adoption is streamed on the test's cue
                // (`Command::Replay`).
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
                        journal.unreadable.store(true, Ordering::SeqCst);
                        return Ok(());
                    }
                    Command::Replay(upto) => {
                        let upto = upto.min(journal.journaled.len()).max(journal.replayed);
                        let offset = journal.replayed as u64;
                        let data = journal.journaled[journal.replayed..upto].to_vec();
                        journal.replayed = upto;
                        HostMessage::Output { offset, data }
                    }
                    Command::HoldInput { pass, held } => {
                        journal.held.get_or_insert_with(Vec::new);
                        journal.pass = pass;
                        let _ = held.send(());
                        continue;
                    }
                    Command::ReleaseInput => {
                        journal.pass = 0;
                        let mut ack = None;
                        for data in journal.held.take().unwrap_or_default() {
                            ack = Some(write_input(
                                journal,
                                received,
                                &records.received_len,
                                &data,
                            ));
                        }
                        records.held.send_replace(0);
                        match ack {
                            Some(ack) => ack,
                            None => continue,
                        }
                    }
                    Command::DropConnection => {
                        // What was held goes with the connection; holding
                        // goes on.
                        if let Some(held) = &mut journal.held {
                            held.clear();
                        }
                        records.held.send_replace(0);
                        return Ok(());
                    }
                    Command::Restore(blob) => {
                        journal.end += 1;
                        HostMessage::SnapshotRecord {
                            position: Position {
                                offset: journal.end,
                                resizes: journal.resizes,
                            },
                            blob,
                        }
                    }
                };
                if stream.write_all(&encode_host(&record)).await.is_err() {
                    return Ok(());
                }
            }
        }
    }
}

/// What the host reports to its [`TestHost`] as input arrives.
struct Records {
    /// How many bytes it has written.
    received_len: watch::Sender<usize>,
    /// How many inputs it holds unacknowledged.
    held: watch::Sender<usize>,
}

/// Write `data` to the child, as far as the record goes, and the
/// acknowledgement that says so.
fn write_input(
    journal: &mut Journal,
    received: &Mutex<Vec<u8>>,
    received_len: &watch::Sender<usize>,
    data: &[u8],
) -> HostMessage {
    journal.written += data.len() as u64;
    let len = {
        let mut received = received.lock().expect("the input record");
        received.extend_from_slice(data);
        received.len()
    };
    received_len.send_replace(len);
    HostMessage::InputAck {
        written: journal.written,
        discarded: 0,
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
