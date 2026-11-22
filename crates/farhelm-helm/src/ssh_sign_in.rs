//! Establish a shared SSH master before starting a noninteractive client. Only this attempt can ask questions.

use std::collections::HashMap;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use anyhow::Context as _;
use farhelm_proto::ui_channel::{SshConnectionPurpose, SshQuestion, SshQuestionTarget};
use tokio::io::AsyncReadExt as _;
use tokio::net::{UnixListener, UnixStream};
use tokio::process::{Child, Command};
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{Instant, sleep_until, timeout_at};
use uuid::Uuid;

use crate::askpass::{self, Answer, Questions};
use crate::ssh::{self, SharedConnection, SshAccess};
use crate::transport::{SshExit, SshObservation, SshSnapshot};

const QUESTION_LIFETIME: Duration = Duration::from_secs(120);
/// A stopped client can leave a proxy holding stderr. Settle an immediate diagnostic without waiting on that proxy.
const DIAGNOSTIC_SETTLE: Duration = Duration::from_millis(100);

/// Failure after the user has been asked ends the retry ladder. A new sign-in starts on explicit Retry or app startup.
#[derive(Debug, thiserror::Error)]
#[error("SSH sign-in needed")]
pub(crate) struct SignInNeeded;

/// A failure before the first question may carry bounded stderr through the ordinary connection diagnosis.
#[derive(Debug, thiserror::Error)]
#[error("SSH could not establish the shared connection")]
pub(crate) struct BeforeQuestion {
    pub(crate) snapshot: SshSnapshot,
    /// A timed-out child was stopped by the helm. Its exit cannot diagnose a remote refusal.
    pub(crate) timed_out: bool,
}

/// The identity of one proposed interactive SSH process. Setup planning allocates it before returning pending,
/// so the questions and the eventual result carry the same correlation even when no question was needed.
#[derive(Clone, Debug)]
pub(crate) struct Attempt {
    pub(crate) id: String,
    target: SshQuestionTarget,
    connection: SharedConnection,
    startup: bool,
}

impl Attempt {
    /// Only the sessions actor knows whether it is making an initial startup dial.
    pub(crate) fn for_sessions(host: i64, startup: bool) -> Self {
        Self {
            startup,
            ..Self::for_host(host, SharedConnection::Supervisor)
        }
    }

    /// A draft has no registry row to retain its sign-in after the dialog closes.
    pub(crate) fn is_draft(&self) -> bool {
        matches!(self.target, SshQuestionTarget::Draft { .. })
    }

    pub(crate) fn for_host(host: i64, connection: SharedConnection) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            target: SshQuestionTarget::Host { host },
            connection,
            startup: false,
        }
    }

    /// A new destination has no registry id. Its pending reply identifies the checking attempt in the Add dialog.
    pub(crate) fn for_draft() -> Self {
        let id = Uuid::new_v4().to_string();
        Self {
            target: SshQuestionTarget::Draft { draft: id.clone() },
            connection: SharedConnection::Provisioning,
            id,
            startup: false,
        }
    }
}

/// Use SSH's expanded control path as the gate key. Distinct aliases can name the same socket. Only the ordinary
/// SSH host signs in; a container using the same place and Tensorlake connections keep their usual batch clients.
pub(crate) struct SignIn {
    state_dir: PathBuf,
    cli: PathBuf,
    pub(crate) questions: Arc<Questions>,
    gates: Mutex<HashMap<PathBuf, Weak<AsyncMutex<()>>>>,
}

impl SignIn {
    pub(crate) fn new(state_dir: PathBuf, cli: PathBuf, questions: Arc<Questions>) -> Self {
        Self {
            state_dir,
            cli,
            questions,
            gates: Mutex::new(HashMap::new()),
        }
    }

    fn gate(&self, path: PathBuf) -> Arc<AsyncMutex<()>> {
        let mut gates = self.gates.lock().expect("SSH sign-in gate mutex");
        gates.retain(|_, value| value.strong_count() != 0);
        let slot = gates.entry(path).or_default();
        if let Some(existing) = slot.upgrade() {
            return existing;
        }
        let gate = Arc::new(AsyncMutex::new(()));
        *slot = Arc::downgrade(&gate);
        gate
    }

    /// Whether the setup caller can use its ordinary batch path without starting an interactive SSH process.
    /// An unshareable socket keeps the batch path and its existing diagnostic.
    pub(crate) async fn setup_ready(
        &self,
        program: &Path,
        destination: &str,
        deadline: Instant,
    ) -> anyhow::Result<bool> {
        if ssh::control_socket(&self.state_dir, SharedConnection::Provisioning).is_none() {
            return Ok(true);
        }
        let args = ssh::ssh_base_args(
            destination,
            &self.state_dir,
            SharedConnection::Provisioning,
            &SshAccess::UserConfig,
        )?;
        control_is_live(program, &args, deadline).await
    }

    /// Reuse a live shared connection, even one Farhelm did not start. If sharing is unavailable the ordinary client
    /// still gets its existing batch invocation and diagnostic. Every socket probe is local and bounded.
    pub(crate) async fn establish(
        &self,
        program: &Path,
        destination: &str,
        attempt: &Attempt,
        connect_timeout: Duration,
        on_question: &mut (dyn FnMut(bool) + Send),
    ) -> anyhow::Result<()> {
        let connection = attempt.connection;
        let Some(_) = ssh::control_socket(&self.state_dir, connection) else {
            return Ok(());
        };
        let args = ssh::ssh_base_args(
            destination,
            &self.state_dir,
            connection,
            &SshAccess::UserConfig,
        )?;
        let deadline = Instant::now() + connect_timeout;
        if control_is_live(program, &args, deadline).await? {
            return Ok(());
        }
        let path = control_path(program, &args, deadline).await?;
        let gate = self.gate(path);
        let _turn = timeout_at(deadline, gate.lock())
            .await
            .context("waiting for an SSH sign-in turn")?;
        if control_is_live(program, &args, deadline).await? {
            return Ok(());
        }

        // The directory is private before ssh sees it. Its name is short enough for the same state directories
        // where ControlMaster can bind its much longer socket name and temporary suffix.
        let private = tempfile::Builder::new()
            .prefix("ask-")
            .tempdir_in(&self.state_dir)?;
        let socket = private.path().join("sock");
        let helper = private.path().join(askpass::ASKPASS_PROGRAM_NAME);
        let cli = std::path::absolute(&self.cli)?;
        symlink(&cli, &helper).context("locating the SSH sign-in helper")?;
        symlink(
            &cli,
            private.path().join(askpass::ASKPASS_JUMP_PROGRAM_NAME),
        )
        .context("locating the SSH jump helper")?;
        let listener = UnixListener::bind(&socket).context("opening the SSH sign-in socket")?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;

        let interactive = ssh::ssh_sign_in_args(
            destination,
            &self.state_dir,
            connection,
            &SshAccess::UserConfig,
        )?;
        // OpenSSH generates ProxyJump using argv[0]. A unique name resolved by this client's PATH avoids inserting
        // a user path into a shell command. The directory has no `ssh` entry to shadow the original program.
        let path = std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into());
        let search = std::env::join_paths(
            std::iter::once(private.path().to_path_buf()).chain(std::env::split_paths(&path)),
        )
        .context("setting the SSH jump helper's path")?;
        let mut command = Command::new(program);
        command
            .arg0(askpass::ASKPASS_JUMP_PROGRAM_NAME)
            .args(interactive)
            .arg("true")
            .env("SSH_ASKPASS", &helper)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env_remove("SSH_ASKPASS_PROMPT")
            .env(askpass::ASKPASS_SOCKET_ENV, &socket)
            .env(askpass::ASKPASS_SSH_PROGRAM_ENV, program)
            .env("PATH", search)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // SAFETY: setsid is async-signal-safe. It drops the controlling terminal and puts ssh, the helper and any
        // ProxyJump processes in an owned process group before they can access an interactive input source.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        let mut process = InteractiveChild::new(command.spawn().context("starting SSH sign-in")?);
        let result = process
            .run(listener, &self.questions, attempt, deadline, on_question)
            .await;
        if process.has_asked() {
            on_question(false);
        }
        let status = match result {
            Ok(status) => status,
            Err(error) => {
                let _ = process.cancel().await;
                return Err(if process.has_asked() {
                    SignInNeeded.into()
                } else {
                    error
                });
            }
        };
        // A host may restrict the commands it will run. The socket's answer proves authentication even if the
        // throwaway command was refused. Explicit cancellation never reaches this check.
        match control_is_live(program, &args, Instant::now() + connect_timeout).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(process.without_master(status).await),
            _ if process.has_asked() => Err(SignInNeeded.into()),
            Err(error) => Err(error),
        }
    }
}

/// Bound purely local SSH inspection even if an operator's config invokes a slow Match command.
fn inspection(program: &Path, args: &[String], inserted: &[&str]) -> Command {
    let (options, destination) = args.split_at(args.len() - 2);
    let mut command = Command::new(program);
    command
        .args(options)
        .args(inserted)
        .args(destination)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    command
}

async fn control_is_live(
    program: &Path,
    args: &[String],
    deadline: Instant,
) -> anyhow::Result<bool> {
    let status = timeout_at(
        deadline,
        inspection(program, args, &["-O", "check"])
            .stdout(Stdio::null())
            .status(),
    )
    .await
    .context("checking the shared SSH socket timed out")??;
    Ok(status.success())
}

async fn control_path(
    program: &Path,
    args: &[String],
    deadline: Instant,
) -> anyhow::Result<PathBuf> {
    let mut child = inspection(program, args, &["-G"])
        .stdout(Stdio::piped())
        .spawn()?;
    // A local configuration may contain arbitrary names, but an oversized dump cannot hold memory or a turn open.
    let mut output = Vec::new();
    let mut stdout = child
        .stdout
        .take()
        .expect("ssh -G stdout is piped")
        .take(128 * 1024);
    let status = timeout_at(deadline, async {
        stdout.read_to_end(&mut output).await?;
        child.wait().await
    })
    .await
    .context("reading the shared SSH socket path timed out")??;
    anyhow::ensure!(
        status.success(),
        "could not read the shared SSH socket path"
    );
    let words = std::str::from_utf8(&output).context("SSH configuration is not UTF-8")?;
    let path = words
        .lines()
        .find_map(|line| line.strip_prefix("controlpath "))
        .context("SSH did not report a shared socket")?;
    Ok(PathBuf::from(path))
}

/// The socket remains open until cancellation has signalled the process group. Giving a still-running ssh an EOF
/// first could turn a cancelled prompt into an empty authenticator response.
struct InteractiveChild {
    child: Child,
    helper: Option<UnixStream>,
    stderr: SshObservation,
    drain: Option<tokio::task::JoinHandle<()>>,
    question_seen: Arc<std::sync::atomic::AtomicBool>,
}

impl InteractiveChild {
    /// Once any question was shown, another automatic attempt could prompt again after the user chose to stop.
    fn has_asked(&self) -> bool {
        self.question_seen
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Drain stderr without tracing it. Once a question is visible, its remote peer could echo whatever the user
    /// sends. Only output from a command that asked nothing can contribute to a connection diagnosis.
    fn new(mut child: Child) -> Self {
        use std::sync::atomic::{AtomicBool, Ordering};

        let stderr = SshObservation::new();
        let question_seen = Arc::new(AtomicBool::new(false));
        let drain = child.stderr.take().map(|mut pipe| {
            let observed = stderr.clone();
            let question_seen = Arc::clone(&question_seen);
            tokio::spawn(async move {
                let mut bytes = [0u8; 4096];
                while let Ok(count @ 1..) = pipe.read(&mut bytes).await {
                    if !question_seen.load(Ordering::Acquire) {
                        let _ = observed.observe_chunk(&bytes[..count]);
                    }
                }
            })
        });
        Self {
            child,
            helper: None,
            stderr,
            drain,
            question_seen,
        }
    }

    fn stop(&mut self) {
        if let Some(pid) = self.child.id() {
            // SAFETY: the child is still owned and unreaped, and setsid made its PID the group ID.
            unsafe {
                libc::killpg(pid as libc::pid_t, libc::SIGKILL);
            }
        }
    }

    /// Reap the SSH client while the helper socket is still held. A live client can substitute an empty answer if
    /// its helper sees EOF before the group is signalled.
    async fn cancel(&mut self) -> std::io::Result<ExitStatus> {
        self.stop();
        self.child.wait().await
    }

    async fn run(
        &mut self,
        listener: UnixListener,
        questions: &Questions,
        attempt: &Attempt,
        mut deadline: Instant,
        on_question: &mut (dyn FnMut(bool) + Send),
    ) -> anyhow::Result<ExitStatus> {
        loop {
            let (stream, _) = tokio::select! {
                biased;
                status = self.child.wait() => return status.context("waiting for the SSH sign-in client"),
                _ = sleep_until(deadline) => return Err(self.expired()),
                accepted = listener.accept() => accepted.context("accepting the SSH sign-in helper")?,
            };
            let stream = self.helper.insert(stream);
            let request = tokio::select! {
                biased;
                status = self.child.wait() => return status.context("waiting for the SSH sign-in client"),
                _ = sleep_until(deadline) => return Err(self.expired()),
                request = askpass::read_question(stream) => request,
            };
            let (prompt, notice) = match request {
                Ok(request) => request,
                Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // A quick security-key touch can end the notice helper before it has written its whole frame.
                    self.helper.take();
                    continue;
                }
                Err(error) => return Err(error).context("reading the SSH question"),
            };
            deadline = Instant::now() + QUESTION_LIFETIME;
            self.question_seen
                .store(true, std::sync::atomic::Ordering::Release);
            on_question(true);
            let mut question = questions.open(
                SshQuestion {
                    id: Uuid::new_v4().to_string(),
                    attempt: attempt.id.clone(),
                    target: attempt.target.clone(),
                    purpose: match attempt.connection {
                        SharedConnection::Supervisor => SshConnectionPurpose::Sessions,
                        SharedConnection::Provisioning => SshConnectionPurpose::Setup,
                    },
                    startup: attempt.startup,
                    prompt,
                    notice,
                    expires_at: crate::manager::unix_millis()
                        + QUESTION_LIFETIME.as_millis() as i64,
                },
                deadline,
            );
            tokio::select! {
                biased;
                status = self.child.wait() => return status.context("waiting for the SSH sign-in client"),
                _ = sleep_until(deadline) => return Err(SignInNeeded.into()),
                ended = stream.read_u8() => {
                    if !notice || !ended.is_err_and(|error| error.kind() == std::io::ErrorKind::UnexpectedEof) {
                        return Err(SignInNeeded.into());
                    }
                }
                answer = &mut question.reply => match answer {
                    Ok(Answer::Send(answer)) => {
                        if askpass::send_answer(stream, answer).await.is_err() {
                            return Err(SignInNeeded.into());
                        }
                    }
                    Ok(Answer::Cancel) | Err(_) => return Err(SignInNeeded.into()),
                },
            }
            drop(question);
            self.helper.take();
            on_question(false);
        }
    }

    async fn without_master(&mut self, status: ExitStatus) -> anyhow::Error {
        if self.has_asked() {
            SignInNeeded.into()
        } else if status.success() {
            anyhow::anyhow!("SSH signed in but did not leave a shared connection")
        } else if status.code() != Some(255) {
            anyhow::anyhow!("the SSH sign-in command exited with {status}")
        } else {
            if let Some(drain) = &mut self.drain {
                let _ = tokio::time::timeout(DIAGNOSTIC_SETTLE, drain).await;
            }
            let mut stderr = self.stderr.snapshot();
            stderr.exit = Some(SshExit::Status(status));
            BeforeQuestion {
                snapshot: stderr,
                timed_out: false,
            }
            .into()
        }
    }

    /// Before a question opens, carry only real diagnostic evidence into the ordinary connection failure.
    fn expired(&self) -> anyhow::Error {
        if self.has_asked() {
            SignInNeeded.into()
        } else {
            BeforeQuestion {
                snapshot: self.stderr.snapshot(),
                timed_out: true,
            }
            .into()
        }
    }
}

impl Drop for InteractiveChild {
    fn drop(&mut self) {
        self.stop();
        if let Some(drain) = self.drain.take() {
            drain.abort();
        }
    }
}

#[cfg(test)]
mod tests;
