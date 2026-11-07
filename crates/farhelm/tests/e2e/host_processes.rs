//! Terminal hosts under a real supervisor process: `farhelm supervisor run`,
//! whose hidden `--new-terminals` flag defaults to terminal hosts, so that
//! every shipped launch (none passes the flag) creates its new terminals in
//! them.
//!
//! Every other test of host terminals runs an in-process supervisor with the
//! switch set in its seams (`host_creates`, `host_restarts`,
//! `host_terminals`). What those cannot show is the shipped binary itself:
//! that the bare command creates its agents and tabs in hosts while the
//! process fixture for tests written against tmux
//! (`harness::supervisor_process_tmux`, which passes `--new-terminals tmux`)
//! still creates tmux terminals, that the flag reaches a create, that the
//! process's own SIGTERM handling
//! leaves its hosts running and recoverable, that a process killed outright
//! leaves them recoverable too, that a new process's startup finds them, that
//! the socket paths it binds stay within the bound every state directory
//! already lives under, and that a helm relays a host terminal's grid stream
//! and its input over the browser's WebSocket. So these tests start the
//! products as processes, as `prompt_running_agent` and
//! `agent_listing_real_stack` do, and look only through what a viewer sees:
//! the session's grid stream (screen rows) and the input it sends, plus the
//! host's process found by its command line and the session's durable row.
//!
//! Each test's state directory is a `farhelm_teststate` one, whose drop ends
//! any host left in it; the supervisor process is killed before that drop, so
//! nothing starts a host after the drop has looked.

use crate::agent_listing_real_stack::await_local_host;
use crate::harness::*;
use crate::host_terminals::host_process;
use farhelm_proto::grid::{GridInput, GridOutput, GridStreamCheck, Row, ScreenRow};
use farhelm_supervisor::store::TerminalKind;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// How long a test waits for a screen to show something or a process to
/// change: a create through a login shell, a supervisor's startup and an
/// orderly stop's checkpoints all fit well inside it on a loaded machine.
const WAIT: Duration = Duration::from_secs(60);

/// How much of the end of a failed test's supervisor log goes to stderr.
const LOG_TAIL_BYTES: u64 = 16 * 1024;

/// The longest unix socket path macOS accepts: `sun_path` is 104 bytes there,
/// and the path needs its terminating NUL. Linux allows 108, so a path this
/// long binds on both.
const MACOS_SOCKET_PATH_MAX: usize = 103;

// ---- A supervisor process with terminal hosts on ------------------------------

/// The `--new-terminals host` a stack's supervisor is started with unless a
/// test asks for the bare command ([`HostStack::bare`]).
///
/// Named explicitly although it is the default: these tests are about host
/// terminals, and what they check must not change if the default ever
/// does. Only the test of the default itself runs without it.
const EXPLICIT_HOSTS: &[&str] = &["--new-terminals", "host"];

/// The `--new-terminals tmux` a test sets on its stack to stand for a
/// supervisor from before terminal hosts were the default: what the
/// sessions a user already had were created by.
const EXPLICIT_TMUX: &[&str] = &["--new-terminals", "tmux"];

/// One test's real supervisor process, started with `--new-terminals host`
/// (or with no flag at all, for the test of the default), and what it needs
/// around it.
///
/// Field order is drop order: the supervisor process first (its `Child` is
/// killed on drop), then its tmux server, then the directory holding both
/// their sockets and every host's, whose drop ends the hosts the test left.
/// A test that fails before [`HostStack::finish`] still tears down in that
/// order, after its drop has printed the end of the supervisor's log.
struct HostStack {
    supervisor: Option<tokio::process::Child>,
    tmux: Option<TmuxServerGuard>,
    /// The supervisor's state directory: `root` itself, or a directory
    /// inside it when a test needs a path of a chosen length.
    state: PathBuf,
    /// What [`HostStack::start`] passes to `supervisor run` about the kind
    /// of new terminals: [`EXPLICIT_HOSTS`], nothing, or (for a test that
    /// needs an old session first) [`EXPLICIT_TMUX`]. Read at each start, so
    /// a test can change it between restarts.
    kind_args: &'static [&'static str],
    /// Further arguments for `supervisor run`, read at each start like
    /// `kind_args`: a `--tmux` naming the program a test wants it to run.
    extra_args: Vec<std::ffi::OsString>,
    root: farhelm_teststate::TestDir,
    work: farhelm_teststate::TestDir,
    _slot: tokio::sync::SemaphorePermit<'static>,
}

impl HostStack {
    /// A stack whose state directory is a fresh test directory.
    async fn new() -> Self {
        let root = farhelm_teststate::tempdir().expect("state directory");
        let state = root.path().to_path_buf();
        Self::on(root, state).await
    }

    /// A stack like [`HostStack::new`]'s whose supervisor is started as
    /// every shipped launch starts it: a bare `farhelm supervisor run
    /// --state-dir`, with no `--new-terminals`.
    async fn bare() -> Self {
        let mut stack = Self::new().await;
        stack.kind_args = &[];
        stack
    }

    /// A stack whose state directory is `state`, inside `root`, which holds
    /// it and is what cleans it up.
    async fn on(root: farhelm_teststate::TestDir, state: PathBuf) -> Self {
        let slot = SLOTS.acquire().await.expect("semaphore is never closed");
        Self {
            supervisor: None,
            tmux: None,
            state,
            kind_args: EXPLICIT_HOSTS,
            extra_args: Vec::new(),
            root,
            work: farhelm_teststate::tempdir().expect("workdir"),
            _slot: slot,
        }
    }

    /// Start `farhelm supervisor run` (with `--new-terminals host` unless
    /// this is a [`HostStack::bare`] stack) on this stack's state directory,
    /// wait until it accepts connections, and return a client of it.
    ///
    /// Its stderr is appended to `supervisor.log` in the test directory, one
    /// file across restarts. The directory goes when the test ends, failed
    /// or not, so a failing test's drop prints the log's end to the test's
    /// stderr, where the runner keeps it with the failure. The child never
    /// inherits the test's stderr itself: the hosts it starts could then
    /// hold it open after the test, which nextest reports as a leak.
    async fn start(&mut self) -> Arc<SupervisorClient> {
        assert!(self.supervisor.is_none(), "one supervisor at a time");
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.path().join("supervisor.log"))
            .expect("open the supervisor's log");
        let mut command = tokio::process::Command::new(farhelm_bin());
        command
            .args(["supervisor", "run"])
            .args(self.kind_args)
            .args(&self.extra_args)
            .arg("--state-dir")
            .arg(&self.state);
        if self.tmux.is_none() {
            self.tmux = Some(tmux_guard_for_supervisor_child(
                self.state.join("tmux.sock"),
                command.as_std(),
            ));
        }
        let child = command
            .stdout(std::process::Stdio::null())
            .stderr(log)
            .kill_on_drop(true)
            .spawn()
            .expect("spawn the supervisor");
        self.supervisor = Some(child);
        wait_for_supervisor_ready(&self.state).await;
        self.client().await
    }

    /// A new client connection to the running supervisor.
    async fn client(&self) -> Arc<SupervisorClient> {
        let stream = farhelm_supervisor::service::connect(&self.state)
            .await
            .expect("connect to the supervisor");
        let (read, write) = tokio::io::split(stream);
        tokio::time::timeout(WAIT, SupervisorClient::start(read, write))
            .await
            .expect("the handshake finished in time")
            .expect("handshake")
    }

    /// Send `signal` to the supervisor process and wait for it to exit.
    async fn stop(&mut self, signal: libc::c_int) -> std::process::ExitStatus {
        let mut child = self.supervisor.take().expect("a supervisor runs");
        let pid = child.id().expect("the supervisor's pid") as libc::pid_t;
        // SAFETY: `pid` is this test's own child, which nothing has waited
        // on yet, so it is still this process and has not been reused.
        assert_eq!(
            unsafe { libc::kill(pid, signal) },
            0,
            "signal the supervisor"
        );
        tokio::time::timeout(WAIT, child.wait())
            .await
            .expect("the supervisor exited in time")
            .expect("wait for the supervisor")
    }

    /// Create a session running the fake agent's `basic` script, whose
    /// prompt echoes each line it is given as `echo:<line>`.
    async fn create(&self, client: &SupervisorClient) -> SessionInfo {
        client
            .create_session(
                &self.work.path().to_string_lossy(),
                &fixture_cmd("fake-agent --script basic"),
                None,
                80,
                24,
            )
            .await
            .expect("create")
    }

    /// The session's durable row, read from the store the supervisor
    /// writes.
    async fn row(&self, session_id: &str) -> StoredSession {
        SessionStore::open(&self.state.join("supervisor.db"), false)
            .await
            .expect("open the store")
            .session(session_id)
            .await
            .expect("read the row")
            .expect("the row exists")
    }

    /// The socket of the host a session's row names, after checking that
    /// the row is a confirmed host row: the switch reached the create.
    async fn host_socket(&self, session_id: &str) -> PathBuf {
        let row = self.row(session_id).await;
        assert_eq!(
            row.terminal_kind,
            TerminalKind::Host,
            "a create by a supervisor started with {:?} starts its agent in a host",
            self.kind_args
        );
        assert!(
            !row.terminal_address.is_empty(),
            "the launch is confirmed: {row:?}"
        );
        self.state.join("hosts").join(&row.terminal_address)
    }

    /// Kill the supervisor and wait for it, leaving the hosts for the state
    /// directory's drop to end, with no runtime holding their connections.
    async fn finish(mut self) {
        if self.supervisor.is_some() {
            self.stop(libc::SIGKILL).await;
        }
    }

    /// Whether this test's fixture still stands, for a wait that timed out:
    /// is the supervisor process still running (or how it ended), and does
    /// a host process still serve `socket`. A screen that never showed a
    /// line reads differently when either is gone.
    fn premise(&mut self, socket: &Path) -> String {
        let supervisor = match self.supervisor.as_mut().map(|child| child.try_wait()) {
            None => "not started".to_string(),
            Some(Ok(None)) => "running".to_string(),
            Some(Ok(Some(status))) => format!("exited ({status})"),
            Some(Err(e)) => format!("unknown ({e})"),
        };
        let host = match host_process(socket) {
            Some(pid) => format!("pid {pid} serves {}", socket.display()),
            None => format!("no process serves {}", socket.display()),
        };
        format!("supervisor: {supervisor}; host: {host}")
    }
}

impl Drop for HostStack {
    /// On a failing test, print the end of the supervisor's log before the
    /// test directory holding it is removed (see [`HostStack::start`]).
    fn drop(&mut self) {
        if !std::thread::panicking() {
            return;
        }
        let path = self.root.path().join("supervisor.log");
        let tail = std::fs::File::open(&path).and_then(|mut file| {
            use std::io::{Read, Seek, SeekFrom};
            let len = file.metadata()?.len();
            file.seek(SeekFrom::Start(len.saturating_sub(LOG_TAIL_BYTES)))?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(bytes)
        });
        match tail {
            Ok(bytes) => eprintln!(
                "---- the last {} bytes of {} ----\n{}\n---- end of the supervisor's log ----",
                bytes.len(),
                path.display(),
                String::from_utf8_lossy(&bytes)
            ),
            Err(e) => eprintln!("the supervisor's log {} is unreadable: {e}", path.display()),
        }
    }
}

// ---- What a viewer sees ---------------------------------------------------------------

/// The text of a grid stream's screen, kept from the messages as a viewer
/// applies them, and checked against the format's rules as they arrive.
///
/// Only text and placement: runs land at their columns, styles and history
/// are ignored. That is all these tests ask of a screen: whether it shows
/// a line, before and after a restart.
struct GridScreen {
    rows: Vec<String>,
    /// Rows the next `frame` commits.
    pending: Vec<ScreenRow>,
    check: GridStreamCheck,
}

impl GridScreen {
    fn new() -> Self {
        Self {
            rows: Vec::new(),
            pending: Vec::new(),
            check: GridStreamCheck::new(),
        }
    }

    /// Apply one message of the stream, refusing one the format forbids.
    fn apply(&mut self, message: &GridOutput) {
        self.check
            .accept(message)
            .unwrap_or_else(|e| panic!("the grid stream broke a rule ({e}): {message:?}"));
        match message {
            GridOutput::Hello { rows, .. } => self.rows = vec![String::new(); usize::from(*rows)],
            GridOutput::Rows { rows, .. } => self.pending.extend(rows.iter().cloned()),
            GridOutput::Frame { rows, shift, .. } => {
                let shift = usize::from(*shift).min(self.rows.len());
                self.rows.drain(..shift);
                self.rows.resize(usize::from(*rows), String::new());
                for ScreenRow { y, row } in std::mem::take(&mut self.pending) {
                    if let Some(slot) = self.rows.get_mut(usize::from(y)) {
                        *slot = row_text(&row);
                    }
                }
            }
            GridOutput::History { .. }
            | GridOutput::HistoryReset
            | GridOutput::State(_)
            | GridOutput::Clipboard { .. } => {}
        }
    }

    fn text(&self) -> String {
        self.rows.join("\n")
    }
}

/// A row's text with each run at its column, blanks between them as spaces.
fn row_text(row: &Row) -> String {
    let mut text = String::new();
    let mut column = 0u32;
    for run in &row.runs {
        while column < u32::from(run.col) {
            text.push(' ');
            column += 1;
        }
        text.push_str(&run.text);
        column += u32::from(run.width);
    }
    text
}

/// The two messages a browser sends to type `line` and press Enter: the
/// characters as `text`, and Enter as the `key` the grid client sends.
fn typed_line(line: &str) -> [Vec<u8>; 2] {
    let enter = GridInput::Key {
        key: "Enter".into(),
        code: "Enter".into(),
        kc: 13,
        mods: 0,
        mac: false,
    };
    [
        GridInput::Text { data: line.into() }
            .encode()
            .expect("a text message"),
        enter.encode().expect("a key message"),
    ]
}

/// A grid attachment through the supervisor's own client, as the helm
/// makes one for a browser that can draw grids.
struct GridViewer {
    channel: u32,
    stream: TermStream,
    screen: GridScreen,
}

impl GridViewer {
    /// Attach to `session_id`'s agent terminal as a viewer that can draw
    /// grids, and require a grid stream.
    async fn attach(client: &SupervisorClient, session_id: &str) -> Self {
        let (channel, stream) = client
            .attach_terminal(
                session_id,
                80,
                24,
                TerminalSelector::default(),
                "",
                ViewerStreams::BytesOrGrid,
            )
            .await
            .expect("a grid attach");
        assert_eq!(
            stream.stream_kind(),
            farhelm_proto::StreamKind::Grid,
            "a host terminal streams as a grid"
        );
        Self {
            channel,
            stream,
            screen: GridScreen::new(),
        }
    }

    /// Apply the stream until the screen shows every one of `needles`. On a
    /// timeout the failure names the screen and `premise()`, the fixture's
    /// state (see [`HostStack::premise`]).
    async fn wait_for(&mut self, needles: &[&str], premise: impl FnOnce() -> String) {
        let shown = |screen: &GridScreen| needles.iter().all(|n| screen.text().contains(n));
        let waited = tokio::time::timeout(WAIT, async {
            while !shown(&self.screen) {
                match self.stream.recv().await {
                    Some(TermEvent::Data(bytes)) => self
                        .screen
                        .apply(&GridOutput::decode(&bytes).expect("a grid message")),
                    Some(TermEvent::ReplayComplete) => {}
                    Some(TermEvent::Detached(detach)) => panic!("detached: {detach:?}"),
                    None => panic!("the attachment ended"),
                }
            }
        })
        .await;
        assert!(
            waited.is_ok(),
            "the screen never showed {needles:?} ({}):\n{}",
            premise(),
            self.screen.text()
        );
    }

    /// Type `line` and Enter into the terminal.
    async fn type_line(&self, client: &SupervisorClient, line: &str) {
        for message in typed_line(line) {
            client
                .send_grid_input(self.channel, message)
                .await
                .expect("the attachment takes input");
        }
    }
}

// ---- Which kind a real supervisor process creates ------------------------------------

/// A bare `farhelm supervisor run` creates a new session's agent and a new
/// tab in terminal hosts.
///
/// Why: this is the switch-on. No shipped launch passes `--new-terminals`
/// (`farhelm helm setup`'s units, the desktop app and a hand-started
/// supervisor all run the bare command), so the flag's default is what every
/// user's new terminals run in; every other test here names the kind
/// explicitly and would keep passing if the default fell back to tmux.
/// Spec: with no `--new-terminals`, a created session is a confirmed host
/// row whose socket a host process serves; its agent terminal attaches as a
/// grid stream that shows the agent's ready line; and a tab opened in it
/// attaches as a grid stream too (a tmux window would answer a viewer that
/// can draw grids with bytes).
#[farhelm_testtrace::test]
async fn a_bare_supervisor_run_creates_agents_and_tabs_in_hosts() {
    let mut stack = HostStack::bare().await;
    let client = stack.start().await;
    let session = stack.create(&client).await;
    let socket = stack.host_socket(&session.id).await;
    assert!(
        host_process(&socket).is_some(),
        "the host's process serves {}",
        socket.display()
    );
    let mut viewer = GridViewer::attach(&client, &session.id).await;
    viewer
        .wait_for(&["FAKE-AGENT READY"], || stack.premise(&socket))
        .await;

    let tab = client.open_tab(&session.id).await.expect("open a tab");
    let (tab_channel, tab_stream) = client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "",
            ViewerStreams::BytesOrGrid,
        )
        .await
        .expect("attach the tab");
    assert_eq!(
        tab_stream.stream_kind(),
        farhelm_proto::StreamKind::Grid,
        "a new tab runs in a terminal host, which streams as a grid"
    );
    client.detach(tab_channel).await;
    drop(tab_stream);
    drop(viewer);
    drop(client);
    stack.finish().await;
}

/// The real-process fixture for tests written against tmux still creates
/// tmux terminals, now that the bare command creates hosts.
///
/// Why: `harness::supervisor_process_tmux` passes `--new-terminals tmux` so
/// that the tests built on it (byte-stream attaches, reads of the private
/// tmux server) keep testing old terminals for as long as old terminals exist.
/// Without that pin they would run against host terminals and either fail
/// for reasons unrelated to their subject or quietly stop covering tmux.
/// Spec: a session created through that fixture's supervisor is a tmux row,
/// its agent's pane in the private tmux server shows the agent ready, and a
/// viewer that can draw grids is still answered with a byte stream.
#[farhelm_testtrace::test]
async fn the_tmux_process_fixture_still_creates_tmux_terminals() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let supervisor = supervisor_process_tmux().await;
    let state = supervisor.state.path();
    wait_for_supervisor_ready(state).await;
    let stream = farhelm_supervisor::service::connect(state)
        .await
        .expect("connect to the supervisor");
    let (read, write) = tokio::io::split(stream);
    let client = tokio::time::timeout(WAIT, SupervisorClient::start(read, write))
        .await
        .expect("the handshake finished in time")
        .expect("handshake");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = client
        .create_session(
            &work.path().to_string_lossy(),
            &fixture_cmd("fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let row = SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the store")
        .session(&session.id)
        .await
        .expect("read the row")
        .expect("the row exists");
    assert_eq!(
        row.terminal_kind,
        TerminalKind::Tmux,
        "the tmux fixture's supervisor creates tmux terminals: {row:?}"
    );
    wait_for_tmux_agent_ready(&state.join("tmux.sock"), &session.id).await;
    let (channel, stream) = client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::default(),
            "",
            ViewerStreams::BytesOrGrid,
        )
        .await
        .expect("attach");
    assert_eq!(
        stream.stream_kind(),
        farhelm_proto::StreamKind::Bytes,
        "a tmux terminal answers even a grid-capable viewer with bytes"
    );
    client.detach(channel).await;
}

/// A session a supervisor created in tmux is still served from tmux by the
/// bare `farhelm supervisor run` started on its state directory afterwards,
/// and is reported as an old terminal on a supervisor that creates new ones,
/// while a session created now runs in a host.
///
/// Why: this is every existing user's first start after the update that
/// turns terminal hosts on. Their running sessions must keep working where
/// they are, and the listing must carry exactly the two facts the migration
/// band and the session details are drawn from (the session's agent terminal
/// is old, and its supervisor creates new ones); the browser specs check the
/// band against seeded rows and the real-host half only for sessions that
/// are already new, since a browser stack cannot make an old session on a
/// host that creates new ones. Spec: a session created by a supervisor
/// started with `--new-terminals tmux` is a tmux row whose agent is ready in
/// the private tmux server; after an orderly stop and a bare start on the
/// same state directory, the listing reports that session with
/// `agent_terminal: tmux` and `supervisor_creates_host_terminals: true`, its
/// agent is still ready in tmux, and a session created afterwards is a
/// confirmed host row listed with `agent_terminal: host`. The old session's
/// terminal is attached through the bare supervisor by a viewer that can draw
/// grids and still answers with a byte stream: served from tmux.
#[farhelm_testtrace::test]
async fn an_old_session_after_the_switch_to_hosts_is_reported_as_one_to_move() {
    let mut stack = HostStack::new().await;
    stack.kind_args = EXPLICIT_TMUX;
    let client = stack.start().await;
    let old = stack.create(&client).await;
    assert_eq!(
        stack.row(&old.id).await.terminal_kind,
        TerminalKind::Tmux,
        "premise: the first supervisor creates tmux terminals"
    );
    let tmux_socket = stack.state.join("tmux.sock");
    wait_for_tmux_agent_ready(&tmux_socket, &old.id).await;
    drop(client);
    let status = stack.stop(libc::SIGTERM).await;
    assert!(
        status.success(),
        "an orderly stop exits cleanly: {status:?}"
    );

    stack.kind_args = &[];
    let client = stack.start().await;
    let listed = |listing: &farhelm_helm::SessionListing, id: &str| {
        listing
            .sessions
            .iter()
            .find(|session| session.id == id)
            .cloned()
            .unwrap_or_else(|| panic!("session {id} is listed"))
    };
    let listing = client.list_sessions().await.expect("list sessions");
    let old_row = listed(&listing, &old.id);
    assert_eq!(
        old_row.agent_terminal,
        Some(farhelm_proto::TerminalKind::Tmux),
        "the old session's agent is still in tmux: {old_row:?}"
    );
    assert!(
        old_row.supervisor_creates_host_terminals,
        "a bare supervisor reports that it creates new terminals: {old_row:?}"
    );
    wait_for_tmux_agent_ready(&tmux_socket, &old.id).await;
    let (channel, stream) = client
        .attach_terminal(
            &old.id,
            80,
            24,
            TerminalSelector::default(),
            "",
            ViewerStreams::BytesOrGrid,
        )
        .await
        .expect("attach the old session through the bare supervisor");
    assert_eq!(
        stream.stream_kind(),
        farhelm_proto::StreamKind::Bytes,
        "the bare supervisor serves the old session from tmux, as a byte stream"
    );
    client.detach(channel).await;
    drop(stream);

    let new = stack.create(&client).await;
    stack.host_socket(&new.id).await;
    let listing = client.list_sessions().await.expect("list sessions");
    assert_eq!(
        listed(&listing, &new.id).agent_terminal,
        Some(farhelm_proto::TerminalKind::Host),
        "a session created now runs in a host"
    );
    drop(client);
    stack.finish().await;
}

// ---- The private tmux server only while it holds old terminals -------------------------

/// A stand-in tmux at `path` that appends each invocation's arguments to
/// `log`, one line each, and fails. The observable for "the supervisor ran
/// tmux", the way the desktop smoke's sentinel is: no reply a client gets
/// can show that a command was not issued.
fn write_recording_tmux(path: &Path, log: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\necho 'recording tmux: refused' >&2\nexit 1\n",
        shell_words::quote(&log.to_string_lossy())
    );
    std::fs::write(path, script).expect("write the recording tmux");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("make the recording tmux executable");
}

/// Whether a tmux server answers on `socket`, asked with tmux directly.
async fn tmux_server_runs(socket: &Path) -> bool {
    tmux_query(socket, &["list-sessions"])
        .await
        .status
        .success()
}

/// A supervisor whose sessions are all new terminals runs no tmux at all,
/// through everything a user does with a session.
///
/// Why: new terminals do not use tmux, and a machine without old sessions
/// must run no tmux server (old servers are meant to die out as machines
/// reboot). Every per-session path used to ask tmux for every session (the
/// listing, a session's details, a tab lookup, the ticker's passes and its
/// pane-death client, Delete, shutdown), so this drives each of them. The
/// recording tmux the supervisor is given through `--tmux` is the
/// observable; it fails every command, so a supervisor that needed it would
/// also show up as a failure. Spec: with `--tmux` naming a recording
/// program, a bare supervisor starts, creates a session (a host row) whose
/// agent draws its ready line, opens, attaches and closes a tab, lists,
/// renames and deletes the session, keeps running across several ticker
/// intervals, and stops on SIGTERM cleanly, and the program was never run
/// and no tmux socket exists in the state directory.
#[farhelm_testtrace::test]
async fn a_supervisor_with_only_new_terminals_runs_no_tmux() {
    let tools = farhelm_teststate::tempdir().expect("tools directory");
    let recorder = tools.path().join("tmux");
    let log = tools.path().join("tmux-invocations.log");
    write_recording_tmux(&recorder, &log);
    // Premise: the recorder records. Without this, a recorder whose append
    // failed would leave the log empty and pass the test vacuously.
    let ran = std::process::Command::new(&recorder)
        .arg("premise-probe")
        .status()
        .expect("run the recording tmux");
    assert!(
        !ran.success(),
        "premise: the recorder fails like a refused tmux"
    );
    assert_eq!(
        std::fs::read_to_string(&log).expect("premise: the recorder wrote its log"),
        "premise-probe\n"
    );
    std::fs::remove_file(&log).expect("reset the log");
    let mut stack = HostStack::bare().await;
    stack.extra_args = vec!["--tmux".into(), recorder.clone().into_os_string()];

    let client = stack.start().await;
    // Premise: the supervisor was handed the recorder (it can run no other
    // tmux), seen on its command line.
    let pid = stack
        .supervisor
        .as_ref()
        .and_then(|child| child.id())
        .expect("the supervisor's pid");
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).expect("read its command line");
    assert!(
        cmdline
            .split(|byte| *byte == 0)
            .any(|arg| arg == recorder.as_os_str().as_encoded_bytes()),
        "premise: the supervisor runs with --tmux naming the recorder"
    );
    let session = stack.create(&client).await;
    let socket = stack.host_socket(&session.id).await;
    let mut viewer = GridViewer::attach(&client, &session.id).await;
    viewer
        .wait_for(&["FAKE-AGENT READY"], || stack.premise(&socket))
        .await;
    let tab = client.open_tab(&session.id).await.expect("open a tab");
    let (tab_channel, tab_stream) = client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "",
            ViewerStreams::BytesOrGrid,
        )
        .await
        .expect("attach the tab");
    client.detach(tab_channel).await;
    drop(tab_stream);
    let listing = client.list_sessions().await.expect("list sessions");
    assert!(
        listing
            .sessions
            .iter()
            .any(|listed| listed.id == session.id),
        "the session is listed"
    );
    client
        .rename_session(&session.id, "renamed", None)
        .await
        .expect("rename");
    // The ticker's passes (sampling, the reap) run on their own every
    // couple of seconds and report nothing a client can wait on.
    // sleep-ok: an observation window of several ticker intervals, for passes with no completion signal.
    tokio::time::sleep(Duration::from_secs(7)).await;
    client
        .close_tab(&session.id, &tab.id)
        .await
        .expect("close the tab");
    drop(viewer);
    client.delete_session(&session.id).await.expect("delete");
    drop(client);
    let status = stack.stop(libc::SIGTERM).await;
    assert!(
        status.success(),
        "an orderly stop exits cleanly: {status:?}"
    );

    let invocations = std::fs::read_to_string(&log).unwrap_or_default();
    assert_eq!(
        invocations, "",
        "a supervisor with only new terminals must not run tmux"
    );
    assert!(
        !stack.state.join("tmux.sock").exists(),
        "no tmux server was started on the private socket"
    );
    stack.finish().await;
}

/// The private tmux server that holds a host's old terminals is adopted,
/// keeps serving them, and ends with the last one.
///
/// Why: old sessions must keep working after the update that stops
/// starting tmux, and the server must not outlive them: an earlier build
/// kept it running with nothing in it forever. Spec: a session a supervisor
/// started with `--new-terminals tmux` created is, after an orderly stop
/// and a bare start, still ready in the private server and attaches as a
/// byte stream; a session created then is a host row; deleting the old
/// session ends the private server.
#[farhelm_testtrace::test]
async fn the_private_tmux_server_ends_with_the_last_old_terminal() {
    let mut stack = HostStack::new().await;
    stack.kind_args = EXPLICIT_TMUX;
    let client = stack.start().await;
    let old = stack.create(&client).await;
    let tmux_socket = stack.state.join("tmux.sock");
    wait_for_tmux_agent_ready(&tmux_socket, &old.id).await;
    drop(client);
    let status = stack.stop(libc::SIGTERM).await;
    assert!(
        status.success(),
        "an orderly stop exits cleanly: {status:?}"
    );

    stack.kind_args = &[];
    let client = stack.start().await;
    wait_for_tmux_agent_ready(&tmux_socket, &old.id).await;
    let (channel, stream) = client
        .attach_terminal(
            &old.id,
            80,
            24,
            TerminalSelector::default(),
            "",
            ViewerStreams::BytesOrGrid,
        )
        .await
        .expect("attach the old session through the bare supervisor");
    assert_eq!(
        stream.stream_kind(),
        farhelm_proto::StreamKind::Bytes,
        "the adopted server still serves the old session"
    );
    client.detach(channel).await;
    drop(stream);
    let new = stack.create(&client).await;
    stack.host_socket(&new.id).await;

    client
        .delete_session(&old.id)
        .await
        .expect("delete the old session");
    let deadline = tokio::time::Instant::now() + WAIT;
    while tmux_server_runs(&tmux_socket).await {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the private tmux server must end with the last old terminal"
        );
        // sleep-ok: polls a server's exit, which tmux reports to no one.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    drop(client);
    stack.finish().await;
}

/// Old terminals whose tmux cannot be used are reported not alive, are
/// neither deleted nor restarted, and the supervisor still serves new ones.
///
/// Why: a supervisor's new terminals need no tmux, so a missing tmux must
/// not keep it from starting, and an old terminal nothing can reach is not
/// running as far as anyone can tell. But its agent may well still run in
/// that server: a Delete that removed the row would leave it running with
/// nothing that knows to end it (and the server alive for it), and a
/// Restart would start a second agent beside it. Spec: with an old
/// session's server still running, a bare supervisor started with `--tmux`
/// naming a program that does not exist starts, lists the old session as
/// exited with no terminal, refuses its Delete with a reason naming the
/// unusable server (the row stays) and its Restart, creates a new session
/// as a host row, and leaves the server running with the old agent ready.
#[farhelm_testtrace::test]
async fn old_terminals_without_a_usable_tmux_are_reported_not_alive() {
    let mut stack = HostStack::new().await;
    stack.kind_args = EXPLICIT_TMUX;
    let client = stack.start().await;
    let old = stack.create(&client).await;
    let tmux_socket = stack.state.join("tmux.sock");
    wait_for_tmux_agent_ready(&tmux_socket, &old.id).await;
    drop(client);
    let status = stack.stop(libc::SIGTERM).await;
    assert!(
        status.success(),
        "an orderly stop exits cleanly: {status:?}"
    );

    stack.kind_args = &[];
    stack.extra_args = vec![
        "--tmux".into(),
        stack.root.path().join("no-such-tmux").into_os_string(),
    ];
    let client = stack.start().await;
    let listing = client.list_sessions().await.expect("list sessions");
    let listed = listing
        .sessions
        .iter()
        .find(|session| session.id == old.id)
        .expect("the old session is still listed");
    assert_eq!(
        listed.status,
        SessionStatus::Exited { exit_code: None },
        "an old terminal no tmux can reach is not alive: {listed:?}"
    );
    assert_eq!(listed.agent_terminal, None, "{listed:?}");
    let deleted = client
        .delete_session(&old.id)
        .await
        .expect_err("Delete of an agent that may still run is refused");
    assert!(
        format!("{deleted:#}").contains("cannot use it"),
        "{deleted:#}"
    );
    stack.row(&old.id).await;
    // Restart is refused too, but this fixture's agent reports no
    // conversation, so its refusal comes from an earlier gate than the
    // unusable server's; what this pins is only that no second agent
    // starts. The unusable server's own refusal is the driver's
    // (`tmux::tests::a_running_server_without_a_usable_tmux_is_left_alone_and_absent`).
    client
        .restart_session(&old.id, true)
        .await
        .expect_err("Restart beside an agent that may still run is refused");
    let new = stack.create(&client).await;
    stack.host_socket(&new.id).await;
    assert!(
        tmux_server_runs(&tmux_socket).await,
        "the server holding the old session is left running"
    );
    wait_for_tmux_agent_ready(&tmux_socket, &old.id).await;
    drop(client);
    stack.finish().await;
}

// ---- A supervisor restart and a supervisor kill ----------------------------------------

/// Run a host agent through a restart of its supervisor process that ends
/// with `signal`, and require that the agent, its host and its screen came
/// through and that it still takes input.
async fn a_host_agent_survives(signal: libc::c_int) {
    let mut stack = HostStack::new().await;
    let client = stack.start().await;
    let session = stack.create(&client).await;
    let socket = stack.host_socket(&session.id).await;
    let host = host_process(&socket).expect("the host's process serves its socket");
    let mut viewer = GridViewer::attach(&client, &session.id).await;
    viewer
        .wait_for(&["FAKE-AGENT READY"], || stack.premise(&socket))
        .await;
    viewer.type_line(&client, "before-restart").await;
    viewer
        .wait_for(&["echo:before-restart"], || stack.premise(&socket))
        .await;
    drop(viewer);
    drop(client);

    let status = stack.stop(signal).await;
    if signal == libc::SIGTERM {
        assert!(
            status.success(),
            "an orderly stop exits cleanly: {status:?}"
        );
    }
    assert_eq!(
        host_process(&socket),
        Some(host),
        "the host outlives its supervisor"
    );

    let client = stack.start().await;
    let mut viewer = GridViewer::attach(&client, &session.id).await;
    viewer
        .wait_for(&["FAKE-AGENT READY", "echo:before-restart"], || {
            stack.premise(&socket)
        })
        .await;
    viewer.type_line(&client, "after-restart").await;
    viewer
        .wait_for(&["echo:after-restart"], || stack.premise(&socket))
        .await;
    assert_eq!(
        host_process(&socket),
        Some(host),
        "the same host serves the session"
    );
    let row = stack.row(&session.id).await;
    assert_eq!(row.outcome, LastOutcome::Running);
    assert_eq!(row.terminal_kind, TerminalKind::Host);
    drop(viewer);
    drop(client);
    stack.finish().await;
}

/// A host agent comes through an orderly restart of a real supervisor
/// process with its screen, and takes input from the next one.
///
/// Why: this is the restart every update and every `systemctl restart` is.
/// The in-process tests drive the orderly stop as a function; here it is the
/// process's own SIGTERM handling, its exit, and a new process's startup
/// that must find the host by its tag. Spec: a supervisor started with
/// `--new-terminals host` creates the session in a host (a host row with
/// the socket's name); after SIGTERM it exits successfully and the host's
/// process still serves the socket; the next supervisor's grid stream of the
/// session shows the agent's ready line and the line echoed before the
/// restart, a line typed now is echoed, the same host process serves the
/// socket, and the row is still a running host row.
#[farhelm_testtrace::test]
async fn a_host_agent_survives_an_orderly_supervisor_restart() {
    a_host_agent_survives(libc::SIGTERM).await;
}

/// A host agent comes through a supervisor process killed outright, with its
/// screen, and takes input from the next one.
///
/// Why: a crash, an OOM kill or a `kill -9` leaves no orderly stop and no
/// checkpoint; the host keeps its last snapshot and the output since, and
/// the next process must rebuild the terminal from them. Spec: as for the
/// orderly restart, with SIGKILL in place of SIGTERM and no claim about the
/// exit status.
#[farhelm_testtrace::test]
async fn a_host_agent_survives_a_killed_supervisor() {
    a_host_agent_survives(libc::SIGKILL).await;
}

// ---- The socket path bound -------------------------------------------------------------

/// A state directory whose supervisor socket path is as long as macOS
/// allows also holds its host sockets, and the host terminal works.
///
/// Why: a host socket lives in `<state>/hosts/` under a short name, so that
/// every state directory that works today works for hosts; a longer name or
/// a deeper directory would make a long home directory on a Mac fail every
/// create in a host while the supervisor itself starts fine. Spec: with
/// `<state>/supervisor.sock` exactly 103 bytes long, a supervisor started
/// with `--new-terminals host` creates the session in a host whose socket
/// path is shorter than the supervisor's, its grid stream shows the agent's
/// ready line, and a line typed into it is echoed.
#[farhelm_testtrace::test]
async fn a_state_directory_at_the_socket_path_bound_holds_its_hosts() {
    let root = farhelm_teststate::tempdir().expect("test directory");
    let suffix = "/supervisor.sock".len();
    let base = root.path().as_os_str().len() + 1;
    assert!(
        base + suffix < MACOS_SOCKET_PATH_MAX,
        "premise: the test directory leaves room for a name: {}",
        root.path().display()
    );
    let state = root
        .path()
        .join("s".repeat(MACOS_SOCKET_PATH_MAX - base - suffix));
    // Private, as the supervisor makes its own default state directory.
    std::os::unix::fs::DirBuilderExt::mode(&mut std::fs::DirBuilder::new(), 0o700)
        .create(&state)
        .expect("create the long state directory");
    let supervisor_socket = state.join("supervisor.sock");
    assert_eq!(
        supervisor_socket.as_os_str().len(),
        MACOS_SOCKET_PATH_MAX,
        "premise: the supervisor's socket path is at the bound"
    );

    let mut stack = HostStack::on(root, state).await;
    let client = stack.start().await;
    let session = stack.create(&client).await;
    let socket = stack.host_socket(&session.id).await;
    assert!(
        socket.as_os_str().len() < supervisor_socket.as_os_str().len(),
        "a host socket path ({}) is shorter than the supervisor's ({})",
        socket.display(),
        supervisor_socket.display()
    );
    assert!(
        host_process(&socket).is_some(),
        "the host's process serves {}",
        socket.display()
    );
    let mut viewer = GridViewer::attach(&client, &session.id).await;
    viewer
        .wait_for(&["FAKE-AGENT READY"], || stack.premise(&socket))
        .await;
    viewer.type_line(&client, "at-the-bound").await;
    viewer
        .wait_for(&["echo:at-the-bound"], || stack.premise(&socket))
        .await;
    drop(viewer);
    drop(client);
    stack.finish().await;
}

// ---- Through the helm's WebSocket ----------------------------------------------------------

/// The browser's end of the helm's terminal WebSocket, hand-rolled down to
/// the frames for want of a WebSocket client among this workspace's
/// dependencies: the upgrade, whole server messages (reassembled from
/// fragments), and masked client binary frames.
struct BrowserSocket {
    stream: tokio::net::TcpStream,
}

/// The largest server message this client accepts: a grid message is at
/// most 1 MiB, and anything bigger here is a broken stream, not a terminal.
const MAX_SERVER_MESSAGE: usize = 2 * 1024 * 1024;

/// One server message: its opcode (1 text, 2 binary) and payload. A close
/// frame is `None`.
type ServerMessage = Option<(u8, Vec<u8>)>;

impl BrowserSocket {
    /// Open `path` on the helm with `secret` as the browser's credential,
    /// which a browser can only carry in `Sec-WebSocket-Protocol`.
    async fn open(helm: &HelmProcess, secret: &str, path: &str) -> Self {
        let addr = helm.addr();
        let mut stream = tokio::net::TcpStream::connect(addr)
            .await
            .expect("connect to the helm");
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: Upgrade\r\n\
             Upgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Key: AAAAAAAAAAAAAAAAAAAAAA==\r\n\
             Sec-WebSocket-Protocol: farhelm, farhelm-device-{secret}\r\n\r\n"
        );
        stream
            .write_all(request.as_bytes())
            .await
            .expect("send the upgrade request");
        // Exactly to the end of the response headers, leaving the first
        // frame's bytes in the socket.
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            let n = stream
                .read(&mut byte)
                .await
                .expect("read the upgrade response");
            assert!(n > 0, "the helm closed during the WebSocket upgrade");
            head.push(byte[0]);
        }
        let response = String::from_utf8_lossy(&head).into_owned();
        assert!(
            response.starts_with("HTTP/1.1 101"),
            "the upgrade for {path} was refused: {response}"
        );
        Self { stream }
    }

    /// The next whole message from the helm, `None` once it closes.
    async fn next(&mut self) -> ServerMessage {
        let mut opcode = None;
        let mut payload = Vec::new();
        loop {
            let mut head = [0u8; 2];
            if self.stream.read_exact(&mut head).await.is_err() {
                return None;
            }
            let fin = head[0] & 0x80 != 0;
            let frame_opcode = head[0] & 0x0f;
            let len = match head[1] & 0x7f {
                126 => usize::from(self.stream.read_u16().await.expect("a length")),
                127 => usize::try_from(self.stream.read_u64().await.expect("a length"))
                    .expect("a length that fits"),
                len => usize::from(len),
            };
            assert_eq!(head[1] & 0x80, 0, "a server frame is never masked");
            assert!(
                payload.len() + len <= MAX_SERVER_MESSAGE,
                "a server message over {MAX_SERVER_MESSAGE} bytes"
            );
            let mut part = vec![0u8; len];
            self.stream
                .read_exact(&mut part)
                .await
                .expect("read a frame's payload");
            match frame_opcode {
                8 => return None,
                // Control frames may come between a message's fragments and
                // are not part of it; the helm sends no ping these tests
                // must answer.
                9 | 10 => continue,
                0 => payload.extend_from_slice(&part),
                started => {
                    opcode = Some(started);
                    payload = part;
                }
            }
            if fin {
                return Some((opcode.expect("a message starts with its opcode"), payload));
            }
        }
    }

    /// Send `payload` as one masked binary message.
    async fn send_binary(&mut self, payload: &[u8]) {
        const MASK: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];
        let mut frame = vec![0x82];
        match payload.len() {
            len if len < 126 => frame.push(0x80 | len as u8),
            len if len <= usize::from(u16::MAX) => {
                frame.push(0x80 | 126);
                frame.extend_from_slice(&(len as u16).to_be_bytes());
            }
            len => {
                frame.push(0x80 | 127);
                frame.extend_from_slice(&(len as u64).to_be_bytes());
            }
        }
        frame.extend_from_slice(&MASK);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ MASK[i % 4]));
        self.stream
            .write_all(&frame)
            .await
            .expect("send a frame to the helm");
    }

    /// Apply the grid stream to `screen` until it shows `needle`, requiring
    /// that the stream notice said `grid` before any data. On a timeout the
    /// failure names the screen and `premise()`, as [`GridViewer::wait_for`]
    /// does.
    async fn wait_for(
        &mut self,
        screen: &mut GridScreen,
        notice: &mut bool,
        needle: &str,
        premise: impl FnOnce() -> String,
    ) {
        let waited = tokio::time::timeout(WAIT, async {
            while !screen.text().contains(needle) {
                match self.next().await {
                    Some((1, text)) => {
                        let message: serde_json::Value =
                            serde_json::from_slice(&text).expect("a JSON text frame");
                        match message["type"].as_str() {
                            Some("stream") => {
                                assert_eq!(message["kind"], "grid", "{message}");
                                *notice = true;
                            }
                            Some("detached") => panic!("detached: {message}"),
                            _ => {}
                        }
                    }
                    Some((2, data)) => {
                        assert!(*notice, "the stream notice comes before any data");
                        screen.apply(&GridOutput::decode(&data).expect("a grid message"));
                    }
                    Some((opcode, _)) => panic!("an unexpected message opcode {opcode}"),
                    None => panic!("the helm closed the terminal socket"),
                }
            }
        })
        .await;
        assert!(
            waited.is_ok(),
            "the screen never showed {needle:?} ({}):\n{}",
            premise(),
            screen.text()
        );
    }
}

/// A host terminal reaches a browser through the helm's terminal WebSocket
/// as a grid stream, and the browser's typing reaches the program.
///
/// Why: this is the whole path a user's browser takes to a terminal in a
/// host, with nothing in-process: the helm passes `grid=1` on as the
/// attach's grid capability, relays the supervisor's grid messages unread,
/// and carries each grid input message up as one frame. The helm's own tests
/// run against stand-in supervisors and the supervisor's against its own
/// client; only here do the real processes meet. Spec: with a supervisor
/// started with `--new-terminals host` and a helm on its state directory, a
/// session created through the helm's API is a host row; its terminal
/// socket with `?grid=1` says `{"type":"stream","kind":"grid"}` before any
/// data, its binary frames are grid messages whose screen shows the agent's
/// ready line, and a line sent as `text` and `key` messages is echoed by the
/// agent on that screen.
#[farhelm_testtrace::test]
async fn a_host_terminal_reaches_a_browser_through_the_helm_as_a_grid() {
    let mut stack = HostStack::new().await;
    stack.start().await;
    let helm = helm_process(&stack.state, None).await;
    let secret = device_secret(&stack.state, &helm.base).await;
    let http = client_with_secret(&secret);
    await_local_host(&http, &helm.base).await;
    let (status, body) = post(
        &http,
        &format!("{}/api/sessions", helm.base),
        serde_json::json!({
            "cwd": stack.work.path(),
            "command": {"command": fixture_cmd("fake-agent --script basic"), "yolo": false},
            "title": "grid through the helm",
        }),
    )
    .await;
    assert!(status.is_success(), "create through the helm: {body}");
    let session: SessionInfo = serde_json::from_str(&body).expect("the created session");
    let bare = session
        .id
        .parse::<farhelm_proto::SessionRef>()
        .expect("the helm returns a full session reference")
        .bare_id()
        .to_string();
    let host_socket = stack.host_socket(&bare).await;

    let mut socket = BrowserSocket::open(
        &helm,
        &secret,
        &format!("/api/sessions/{}/term?grid=1", session.id),
    )
    .await;
    let mut screen = GridScreen::new();
    let mut notice = false;
    socket
        .wait_for(&mut screen, &mut notice, "FAKE-AGENT READY", || {
            stack.premise(&host_socket)
        })
        .await;
    for message in typed_line("through-the-helm") {
        socket.send_binary(&message).await;
    }
    socket
        .wait_for(&mut screen, &mut notice, "echo:through-the-helm", || {
            stack.premise(&host_socket)
        })
        .await;
    drop(socket);
    helm.stop_and_reap().await;
    stack.finish().await;
}
