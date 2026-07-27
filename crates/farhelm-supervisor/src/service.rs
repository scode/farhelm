//! The supervisor service: farhelm-proto over a unix socket.
//!
//! This is the only doorway to sessions — the helm (local or via the ssh
//! stdio proxy) and every future caller speak the same protocol to the
//! same handlers, which is what keeps "CLI flags bypass the creation UI,
//! never the creation API" true. The supervisor listens on no network
//! port (SPEC.md): the unix socket plus ssh exec is the entire reachable
//! surface.
//!
//! M1 scope: sessions live in memory and tmux is the truth. SQLite
//! arrives with multi-session management in M2 (PLAN_M1.md).

use crate::launch::{LaunchSpec, resolve_shell, window_command};
use crate::tmux::TmuxDriver;
use anyhow::Context;
use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
use farhelm_proto::{ControlMsg, Frame, SessionInfo};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{info, warn};

/// Data-frame chunk size for replay. Well under MAX_FRAME_LEN; small
/// enough that the first screenful renders while the rest streams.
const REPLAY_CHUNK: usize = 32 * 1024;

/// A session as the supervisor tracks it: the wire-visible metadata plus
/// the two tmux handles that address its terminal.
///
/// Both handles are needed and neither substitutes for the other: session
/// name is the target for anything window-scoped (`resize-window`, the
/// control-mode attach), pane id (`%N`) for anything pane-scoped
/// (`paste-buffer`, `capture-pane`, format queries). Entries are immutable
/// once created — shared as `Arc` and never mutated in place — so nothing
/// has to hold the session map while talking to tmux.
struct SessionEntry {
    info: SessionInfo,
    tmux_name: String,
    pane: String,
}

/// The one live attachment a session may have (SPEC.md: at most one,
/// last attach wins). `notify` reaches the owning connection's writer so
/// a takeover can tell the old client it was detached.
struct ActiveAttach {
    channel: u32,
    notify: mpsc::UnboundedSender<Frame>,
    /// The forwarder task. A `JoinHandle` rather than an `AbortHandle`
    /// because a takeover must be able to *wait* for the old forwarder to
    /// finish: `abort()` only schedules cancellation, so the old
    /// control-mode client's process would otherwise still be alive when
    /// the new one starts.
    forwarder: tokio::task::JoinHandle<()>,
}

/// One host's session authority, shared by every connection.
///
/// Sessions are keyed by session id, attachments by session id too —
/// separate maps because their lifetimes differ: a session outlives any
/// number of attach/detach cycles, and SPEC.md caps the attachments at one
/// per session while placing no such cap on sessions.
///
/// Lock discipline: the two mutexes are never held at once, and no tmux
/// call happens while `sessions` is held. `attachments` is deliberately
/// the exception — the whole attach takeover runs under it, because that
/// is the only way "at most one attachment, last attach wins" survives two
/// concurrent attaches (see the `Attach` handler), and the input and
/// `Resize` arms hold it across their tmux calls for the same reason: an
/// ownership check that releases the lock before acting goes stale the
/// moment a takeover interleaves.
///
/// Known coarseness, acceptable in M1 and revisit at M2: the map-wide
/// mutex serializes input for EVERY session behind any in-flight attach
/// (which holds the lock across takeover, resize, and the bounded
/// control-mode cutover). With many sessions per supervisor, this wants
/// per-session attachment slots so sessions stall independently.
pub struct Supervisor {
    state_dir: PathBuf,
    tmux: TmuxDriver,
    sessions: Mutex<HashMap<String, Arc<SessionEntry>>>,
    attachments: Mutex<HashMap<String, ActiveAttach>>,
    /// This binary's own path: the launch shim is a subcommand of it.
    farhelm_exe: PathBuf,
}

impl Supervisor {
    /// Production constructor: the launch shim is this very process's
    /// binary. Test harnesses must NOT use this — their `current_exe` is
    /// the libtest runner, not farhelm — and use `new_with_exe` instead.
    pub async fn new(state_dir: &Path) -> anyhow::Result<Arc<Supervisor>> {
        let exe = std::env::current_exe().context("resolving own binary path")?;
        Self::new_with_exe(state_dir, exe).await
    }

    /// Constructor with an explicit farhelm binary path for the launch
    /// shim. Used by tests (pointing at the built `farhelm` artifact)
    /// and by any embedder whose own executable is not farhelm.
    pub async fn new_with_exe(
        state_dir: &Path,
        farhelm_exe: PathBuf,
    ) -> anyhow::Result<Arc<Supervisor>> {
        // 0700 on both: the socket and the launch specs (which hold full
        // agent command lines) live here. See ensure_private_dir.
        crate::ensure_private_dir(state_dir).await?;
        crate::ensure_private_dir(&state_dir.join("launch")).await?;
        let tmux = TmuxDriver::new(state_dir);
        tmux.ensure_server().await?;
        Ok(Arc::new(Supervisor {
            state_dir: state_dir.to_path_buf(),
            tmux,
            sessions: Mutex::new(HashMap::new()),
            attachments: Mutex::new(HashMap::new()),
            farhelm_exe,
        }))
    }

    /// The supervisor's unix socket path within a state dir. Shared with
    /// `farhelm internal stdio`, which is just a dumb pipe to this.
    pub fn socket_path(state_dir: &Path) -> PathBuf {
        state_dir.join("supervisor.sock")
    }

    /// Accept connections forever on this supervisor's own state dir.
    /// The directory is not a parameter: a caller passing a different one
    /// would bind the socket in one place while launch specs land in
    /// another.
    pub async fn serve(self: &Arc<Self>) -> anyhow::Result<()> {
        let path = Self::socket_path(&self.state_dir);
        // Exclusivity is an OS lock, not an inspect-the-socket dance: a
        // probe-then-remove-then-bind sequence is a TOCTOU where two
        // racing supervisors can each pass the probe, and the slower one
        // then unlinks the faster one's freshly bound socket — leaving
        // two processes driving the same tmux server with disjoint
        // session maps. The flock is atomic, and holding it for the
        // process lifetime is what makes the socket removal and the
        // launch-dir sweep below single-owner. The file stays locked
        // until this function returns, which for a healthy supervisor is
        // never.
        let lock_path = self.state_dir.join("supervisor.lock");
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .context("opening supervisor lock file")?;
        if let Err(e) = lock.try_lock() {
            anyhow::bail!(
                "a supervisor is already running against {} \n\
                 (SPEC.md allows at most one supervisor per user per host; lock: {e})",
                self.state_dir.display()
            );
        }
        // Holding the lock proves any existing socket file is a leftover
        // from a dead supervisor (the lock dies with its process), so
        // removing it is safe.
        if path.exists() {
            let _ = tokio::fs::remove_file(&path).await;
        }
        let listener = UnixListener::bind(&path).context("binding supervisor socket")?;
        // Belt to the state dir's braces: connecting to this socket means
        // running commands as this user, so do not inherit the umask.
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await?;
        }
        // Sweep launch specs orphaned by a previous run — a login shell
        // that died before reaching the shim leaves one behind, and it
        // holds the agent's command line; nothing later would remove it.
        // Deliberately AFTER the bind above: the bind is what proves this
        // process is the state dir's one supervisor. Sweeping in the
        // constructor let a second `supervisor run` destroy the live
        // supervisor's in-flight specs and only then bail on the
        // exclusivity check.
        // Best-effort, but never silent: this sweep is credential
        // hygiene, so a failure that leaves specs behind must at least
        // say so in the log.
        let launch_dir = self.state_dir.join("launch");
        match tokio::fs::read_dir(&launch_dir).await {
            Err(e) => warn!(error = %e, "could not sweep launch dir; orphaned specs may remain"),
            Ok(mut entries) => loop {
                match entries.next_entry().await {
                    Ok(None) => break,
                    Ok(Some(entry)) => {
                        if let Err(e) = tokio::fs::remove_file(entry.path()).await {
                            warn!(spec = %entry.path().display(), error = %e,
                                "could not remove orphaned launch spec");
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "launch-dir sweep aborted early; orphaned specs may remain");
                        break;
                    }
                }
            },
        }
        info!(socket = %path.display(), "supervisor listening");
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let sup = Arc::clone(self);
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(sup, stream).await {
                            warn!(error = %e, "connection ended with error");
                        }
                    });
                }
                // A transient accept failure (EMFILE under fd pressure,
                // say) must not kill the process whose entire purpose is
                // outliving everything else.
                Err(e) => warn!(error = %e, "accept failed; continuing"),
            }
        }
    }

    /// The one true creation path: validate preconditions, hand the agent
    /// argv to the launch shim, and start a tmux session running it.
    ///
    /// Failure semantics follow SPEC.md's split. Everything checkable up
    /// front — the directory exists, the invocation parses into an argv —
    /// fails here, leaving no session behind at all. Whether the agent
    /// itself then execs successfully is *not* decided here. The shim
    /// writes evidence of `exec` failure for a later milestone's status
    /// classifier, but M1 does not consume that evidence; the terminal
    /// remains available either way so the launch diagnostic is visible.
    ///
    /// `title` defaults to the working directory's basename, which is what
    /// SPEC.md's "auto-generated when omitted" means in M1.
    async fn create_session(
        &self,
        cwd: &str,
        invocation: &str,
        title: Option<String>,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<SessionInfo> {
        let cwd_path = PathBuf::from(cwd);
        // Preserve the distinction between a bad caller precondition and
        // a host I/O failure. Calling both "does not exist" sends users
        // looking for a typo when the real problem is permission, a
        // symlink loop, or a failing filesystem.
        match tokio::fs::metadata(&cwd_path).await {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => anyhow::bail!("working directory is not a directory: {cwd}"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                anyhow::bail!("working directory does not exist: {cwd}");
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("reading working directory metadata for {cwd}"));
            }
        }
        // The invocation itself stays out of the error: it may carry
        // credentials (`--api-key ...`), and this message travels into
        // the HTTP error body and the helm's stderr/journal. shell-words'
        // own error names the syntax problem.
        let argv = shell_words::split(invocation).context("parsing agent invocation")?;
        if argv.is_empty() {
            anyhow::bail!("agent invocation is empty");
        }

        let id = uuid::Uuid::new_v4().to_string();
        let short = &id[..8];
        let tmux_name = format!("fh-{short}");
        let title = title.unwrap_or_else(|| {
            cwd_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "session".to_string())
        });

        let spec_path = self.state_dir.join("launch").join(format!("{id}.json"));
        let spec = LaunchSpec {
            argv,
            status_file: self.state_dir.join("launch").join(format!("{id}.status")),
        };
        // 0600 from the first byte: the spec holds the full agent command
        // line, which users do put credentials into (`--api-key ...`).
        // Mode is set at open, not chmod-after-write — a write-then-chmod
        // leaves a window where the default umask exposes the contents.
        // A failed write cleans up too: a partial spec (disk full after
        // create) would otherwise strand a credential prefix on disk
        // until the next supervisor restart's sweep.
        crate::write_private_file(&spec_path, &serde_json::to_vec(&spec)?)
            .await
            .context("writing launch spec")?;

        let shell = resolve_shell().await;
        let cmd = window_command(&shell, &self.farhelm_exe, &spec_path);
        let pane = match self
            .tmux
            .create_session(&tmux_name, &cwd_path, cols, rows, &cmd)
            .await
        {
            Ok(pane) => pane,
            Err(e) => {
                // The shim unlinks the spec once it has read it, so a
                // launch that never happens would strand a file holding
                // the agent's full command line — credentials included —
                // with nothing left to clean it up.
                return match tokio::fs::remove_file(&spec_path).await {
                    Ok(()) => Err(e),
                    Err(cleanup) => Err(e.context(format!(
                        "could not remove launch spec {} after tmux creation failed: {cleanup}",
                        spec_path.display()
                    ))),
                };
            }
        };

        let info = SessionInfo {
            id: id.clone(),
            title,
            cwd: cwd.to_string(),
            invocation: invocation.to_string(),
        };
        info!(session = %id, tmux = %tmux_name, %pane, "session created");
        self.sessions.lock().await.insert(
            id,
            Arc::new(SessionEntry {
                info: info.clone(),
                tmux_name,
                pane,
            }),
        );
        Ok(info)
    }
}

/// Serve one protocol connection. Generic over the byte stream so tests
/// can drive it over an in-process duplex pipe with the same code path
/// production uses over the unix socket.
pub async fn handle_connection<S>(sup: Arc<Supervisor>, stream: S) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (r, w) = tokio::io::split(stream);
    let mut reader = FrameReader::new(r);
    let mut writer = FrameWriter::new(w);
    handshake(&mut reader, &mut writer, "supervisor").await?;

    // Single writer task; everything that wants to send (request
    // handlers, the output forwarder, takeover notifications) goes
    // through this queue so frames never interleave mid-write.
    let (tx, mut rx) = mpsc::unbounded_channel::<Frame>();
    let (writer_failed_tx, mut writer_failed_rx) = oneshot::channel();
    let writer_task = tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            if let Err(e) = writer.write_frame(&frame).await {
                warn!(error = %e, "frame write to client failed");
                let _ = writer_failed_tx.send(e.to_string());
                break;
            }
        }
    });

    // Which session each of this connection's data channels types into.
    // Connection-local by necessity: channel ids are unique only within a
    // connection, since every client numbers its channels from 1.
    let mut input_routes: HashMap<u32, Arc<SessionEntry>> = HashMap::new();

    let result: anyhow::Result<()> = async {
        loop {
            // Either half failing ends the connection. Waiting only for
            // read EOF leaks attachments when a half-broken peer keeps
            // writing after it has stopped reading our replies.
            let frame = tokio::select! {
                frame = reader.read_frame() => frame?,
                error = &mut writer_failed_rx => {
                    let detail = error.unwrap_or_else(|_| "writer task exited".to_string());
                    return Err(anyhow::anyhow!("frame write to client failed: {detail}"));
                }
            };
            let Some(frame) = frame else {
                break;
            };
            match frame.kind {
                farhelm_proto::FrameKind::Data => {
                    // Route input only if this channel is still the
                    // session's live attachment: a client kicked by a
                    // takeover must not keep typing into a pane it no
                    // longer owns, and the supervisor enforces that
                    // rather than trusting clients to stop.
                    if let Some(entry) = input_routes.get(&frame.channel).cloned() {
                        // The check and the paste run under ONE lock hold,
                        // like the Resize arm: releasing between them is a
                        // TOCTOU where a takeover completes in the gap and
                        // the kicked client's already-validated keystrokes
                        // land in the winner's pane — and keystrokes into
                        // an agent terminal are command execution. Safe
                        // against deadlock because forwarders never take
                        // this lock, and the Attach handler already holds
                        // it across its own tmux calls.
                        //
                        // Both halves of the check matter: channel ids are
                        // unique only within a connection (every client
                        // numbers from 1), so comparing the channel alone
                        // would let a kicked client on another connection
                        // pass whenever the numbers collide.
                        // `same_channel` identifies the owning connection.
                        let mut attachments = sup.attachments.lock().await;
                        let live = attachments.get(&entry.info.id).is_some_and(|a| {
                            a.channel == frame.channel && a.notify.same_channel(&tx)
                        });
                        if live {
                            // A failed paste is this session's problem,
                            // not the shared connection's. It is still
                            // fatal to this attachment: accepting later
                            // chunks after silently losing one can turn
                            // a command into a different command.
                            if let Err(e) = sup.tmux.send_input(&entry.pane, &frame.body).await {
                                warn!(session = %entry.info.id, error = %e, "input dropped");
                                if let Some(old) = attachments.remove(&entry.info.id) {
                                    old.forwarder.abort();
                                    let _ = old.forwarder.await;
                                    let _ =
                                        old.notify.send(Frame::control(&ControlMsg::Detached {
                                            channel: old.channel,
                                            reason: format!("terminal input failed: {e:#}"),
                                        }));
                                }
                                input_routes.remove(&frame.channel);
                            }
                        } else {
                            drop(attachments);
                            // This channel lost its attachment; stop
                            // holding the session entry alive for it.
                            input_routes.remove(&frame.channel);
                        }
                    }
                }
                farhelm_proto::FrameKind::Control => {
                    let msg = parse_control(&frame)?;
                    handle_control(&sup, msg, &tx, &mut input_routes).await;
                }
            }
        }
        Ok(())
    }
    .await;

    // Connection gone: tear down any attachments it owned so the next
    // attach doesn't fight a dead forwarder. Abort AND await, exactly
    // like the takeover path: abort only schedules cancellation, and the
    // old control-mode client's process is not gone until the cancelled
    // task has been polled to completion. Removing the entry without
    // waiting would let a new connection's attach — which finds no
    // incumbent to kick — open its control client while the old one is
    // still dying, the documented frozen-replay hazard. Awaiting under
    // the lock is safe (forwarders never take it) and is what serializes
    // that new attach behind this teardown.
    let mut attachments = sup.attachments.lock().await;
    let mine: Vec<ActiveAttach> = attachments
        .extract_if(|_, attachment| attachment.notify.same_channel(&tx))
        .map(|(_, attachment)| attachment)
        .collect();
    for attachment in mine {
        attachment.forwarder.abort();
        let _ = attachment.forwarder.await;
    }
    drop(attachments);
    drop(tx);
    let _ = writer_task.await;
    result
}

/// Dispatch one control message from a connected client.
///
/// Failures belonging to one request—bad cwd, a tmux hiccup, an unknown
/// session—become `ControlMsg::Error` replies here. They must not escape
/// into the connection loop: one connection carries every session the
/// helm is driving, so request-local failure cannot be allowed to detach
/// unrelated terminals.
///
/// `tx` doubles as this connection's identity: `same_channel` against it
/// is how the handlers tell "the connection that owns this attachment"
/// from any other, which channel ids alone cannot do.
async fn handle_control(
    sup: &Supervisor,
    msg: ControlMsg,
    tx: &mpsc::UnboundedSender<Frame>,
    input_routes: &mut HashMap<u32, Arc<SessionEntry>>,
) {
    let send = |m: &ControlMsg| {
        let _ = tx.send(Frame::control(m));
    };
    match msg {
        ControlMsg::CreateSession {
            req_id,
            cwd,
            invocation,
            title,
            cols,
            rows,
        } => match sup
            .create_session(&cwd, &invocation, title, cols, rows)
            .await
        {
            Ok(session) => send(&ControlMsg::SessionCreated { req_id, session }),
            Err(e) => send(&ControlMsg::Error {
                req_id,
                message: format!("{e:#}"),
            }),
        },
        ControlMsg::ListSessions { req_id } => {
            let sessions = sup
                .sessions
                .lock()
                .await
                .values()
                .map(|s| s.info.clone())
                .collect();
            send(&ControlMsg::SessionList { req_id, sessions });
        }
        ControlMsg::Attach {
            req_id,
            session_id,
            channel,
            cols,
            rows,
        } => {
            if channel == 0 || input_routes.contains_key(&channel) {
                let message = if channel == 0 {
                    "attachment channel 0 is reserved".to_string()
                } else {
                    format!("attachment channel {channel} is already in use")
                };
                send(&ControlMsg::Error { req_id, message });
                return;
            }
            let entry = sup.sessions.lock().await.get(&session_id).cloned();
            let Some(entry) = entry else {
                send(&ControlMsg::Error {
                    req_id,
                    message: format!("no such session: {session_id}"),
                });
                return;
            };

            // The whole takeover — kick the old attachment, set up tmux,
            // install the new one — runs under one lock. Without it, two
            // concurrent attaches can both pass the kick step and both
            // install forwarders, leaving two live attachments for a
            // session SPEC.md says has at most one. It also makes the
            // winner the *last* attach rather than whichever client's
            // tmux calls happened to finish last.
            let mut attachments = sup.attachments.lock().await;

            if let Some(old) = attachments.remove(&session_id) {
                old.forwarder.abort();
                // Awaiting the abort is what actually makes the old
                // control-mode client gone: dropping its OutputStream
                // (and so killing the process) happens when the task is
                // polled after cancellation, not when abort() returns.
                // The forwarder never takes this lock, so awaiting it
                // here cannot deadlock.
                let _ = old.forwarder.await;
                let _ = old.notify.send(Frame::control(&ControlMsg::Detached {
                    channel: old.channel,
                    reason: "another client attached".to_string(),
                }));
            }

            // Size the window now, not during prep: resizing is a
            // mutation the incumbent would have seen, and a later prep
            // failure would have left its terminal reflowed to a size
            // nobody is using.
            if let Err(e) = sup.tmux.resize_window(&entry.tmux_name, cols, rows).await {
                warn!(session = %session_id, error = %e, "resize during attach failed");
            }

            // The incumbent must be fully gone before the replacement
            // control client starts. Overlap reproducibly froze the new
            // stream after replay, even though two steady-state control
            // clients both receive output in isolation. The replacement
            // attaches with output disabled, captures replay, and enables
            // live output through that SAME client; its final command
            // block is the exact replay/live boundary.
            //
            // This setup happens after takeover on purpose. There is no
            // safe way to preserve the incumbent while also avoiding
            // control-client overlap, so failure here leaves the session
            // detached and reports only this attach request as failed.
            let (modes, prefill, mut stream) = match sup
                .tmux
                .open_replay_stream(&entry.tmux_name, &entry.pane)
                .await
            {
                Ok(parts) => parts,
                Err(e) => {
                    drop(attachments);
                    send(&ControlMsg::Error {
                        req_id,
                        message: format!("{e:#}"),
                    });
                    return;
                }
            };

            send(&ControlMsg::Attached { req_id, channel });
            // Order is load-bearing: the alternate-screen switch must
            // precede the content (it clears the buffer it switches to),
            // and cursor placement must follow it (writing content moves
            // the cursor). See PaneModes.
            let _ = tx.send(Frame::data(
                channel,
                modes.pre_content_sequences().into_bytes(),
            ));
            for chunk in prefill.chunks(REPLAY_CHUNK) {
                let _ = tx.send(Frame::data(channel, chunk.to_vec()));
            }
            let _ = tx.send(Frame::data(
                channel,
                modes.post_content_sequences().into_bytes(),
            ));

            let fwd_tx = tx.clone();
            let task = tokio::spawn(async move {
                loop {
                    match stream.next_output().await {
                        Ok(Some(bytes)) => {
                            // Chunked like the replay above, and for a
                            // harsher reason: one bounded `%output`
                            // notification may still be larger than a
                            // protocol frame, and the encoder now rejects
                            // that rather than sending something the far
                            // side cannot decode. This is the last
                            // chunking boundary; input and replay already
                            // do the same.
                            let client_gone = bytes
                                .chunks(REPLAY_CHUNK)
                                .any(|c| fwd_tx.send(Frame::data(channel, c.to_vec())).is_err());
                            if client_gone {
                                break;
                            }
                        }
                        Ok(None) => {
                            let _ = fwd_tx.send(Frame::control(&ControlMsg::Detached {
                                channel,
                                reason: "session terminal ended".to_string(),
                            }));
                            break;
                        }
                        // Must notify too: swallowing this leaves the
                        // client with a terminal that silently stops
                        // updating while still accepting input, and no
                        // log line anywhere explaining why.
                        Err(e) => {
                            warn!(channel, error = %e, "output stream failed");
                            let _ = fwd_tx.send(Frame::control(&ControlMsg::Detached {
                                channel,
                                reason: format!("output stream failed: {e:#}"),
                            }));
                            break;
                        }
                    }
                }
                stream.shutdown().await;
            });

            attachments.insert(
                session_id.clone(),
                ActiveAttach {
                    channel,
                    notify: tx.clone(),
                    forwarder: task,
                },
            );
            drop(attachments);
            input_routes.insert(channel, entry);
        }
        ControlMsg::Detach { channel } => {
            input_routes.remove(&channel);
            let mut attachments = sup.attachments.lock().await;
            let mine = attachments.iter().find_map(|(id, a)| {
                (a.channel == channel && a.notify.same_channel(tx)).then(|| id.clone())
            });
            if let Some(id) = mine
                && let Some(a) = attachments.remove(&id)
            {
                // Abort AND await, mirroring the takeover path: detach
                // followed by an immediate reattach (a browser reload is
                // exactly this) finds no incumbent to kick, so the only
                // thing keeping the old control-mode client from
                // overlapping the new one — the documented frozen-replay
                // hazard — is waiting for it here, before the lock is
                // released. Awaiting cannot deadlock: forwarders never
                // take this lock.
                a.forwarder.abort();
                let _ = a.forwarder.await;
            }
        }
        ControlMsg::Resize {
            session_id,
            channel,
            cols,
            rows,
        } => {
            // Same trust boundary as input, and the same two-part check
            // as the Data arm: `same_channel` identifies the owning
            // connection, and the channel id tells apart clients
            // multiplexed over ONE connection — every browser tab rides
            // the helm's single supervisor connection, so a
            // connection-level check alone would let a tab that just
            // lost a takeover reflow the winner's terminal.
            //
            // The session entry is fetched first so the two supervisor
            // locks are never held at once (see the struct docs); the
            // resize itself then runs UNDER the attachments lock, like
            // the Attach handler's tmux calls. Checking ownership and
            // then resizing after releasing the lock is a TOCTOU: a
            // takeover can interleave in that gap, and the kicked
            // client's already-authorized resize would land after the
            // winner's attach-time resize, reflowing the winner's
            // terminal with nothing to correct it.
            let entry = sup.sessions.lock().await.get(&session_id).cloned();
            if let Some(entry) = entry {
                let attachments = sup.attachments.lock().await;
                let owns = attachments
                    .get(&session_id)
                    .is_some_and(|a| a.channel == channel && a.notify.same_channel(tx));
                if owns {
                    // Fire-and-forget: a resize has no req_id to answer,
                    // and a tmux failure here must not take the
                    // connection (and every other session on it) down.
                    if let Err(e) = sup.tmux.resize_window(&entry.tmux_name, cols, rows).await {
                        warn!(session = %session_id, error = %e, "resize failed");
                    }
                }
            }
        }
        ControlMsg::Hello { .. } => {
            // A second hello is a protocol violation; ignore rather than
            // kill the connection over it.
        }
        // Response/event messages arriving at the supervisor are peer
        // bugs; log and continue.
        other => warn!(?other, "unexpected control message at supervisor"),
    }
}

/// `farhelm supervisor run` in one call: build a supervisor on `state_dir`
/// and serve its socket until the process dies. Returns only on a fatal
/// error — a successful supervisor never returns.
pub async fn run(state_dir: &Path) -> anyhow::Result<()> {
    let sup = Supervisor::new(state_dir).await?;
    sup.serve().await
}

/// Connect to a running supervisor's socket (used by `internal stdio`).
/// Fails if no supervisor is listening; this deliberately does not start
/// one, because discovery-first (SPEC.md) means an absent supervisor is
/// the caller's decision to make, not a side effect of dialing.
pub async fn connect(state_dir: &Path) -> anyhow::Result<UnixStream> {
    let path = Supervisor::socket_path(state_dir);
    UnixStream::connect(&path)
        .await
        .with_context(|| format!("connecting to supervisor socket {}", path.display()))
}
