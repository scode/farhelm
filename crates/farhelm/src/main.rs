//! The `farhelm` multi-call binary.
//!
//! One artifact carries every role — helm serving and token management,
//! `supervisor run`, and the hidden `internal` namespace — because
//! provisioning copies exactly one binary to a host and the launch shim must
//! exist inside every session without separate installation (SPEC_impl.md,
//! "CLI").
//! `farhelm spawn` keeps stdout machine-readable: its only successful output
//! is the child id and every diagnostic goes to stderr.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod fake_agent;

#[derive(Parser)]
#[command(
    name = "farhelm",
    version,
    about = "Supervise coding agents in real terminals"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a session on the supervisor that launched this one.
    Spawn {
        /// Child working directory. Relative paths resolve against this
        /// process's real current directory before crossing the wire.
        #[arg(long)]
        cwd: PathBuf,
        /// Optional display title; omitted derives from the directory.
        #[arg(long)]
        title: Option<String>,
        /// Agent profile name; omitted derives the host's last-used profile.
        #[arg(long)]
        agent: Option<String>,
        /// Organizational parent id. Never defaults to this session.
        #[arg(long)]
        parent: Option<String>,
        /// Retry key, valid only for the child session's lifetime.
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    /// Run the helm: the single control-plane process serving the UI.
    Helm {
        #[command(subcommand)]
        command: HelmCmd,
    },
    /// Run the per-host supervisor.
    Supervisor {
        #[command(subcommand)]
        command: SupervisorCmd,
    },
    /// Internal commands: machinery, not user surface. Hidden from help;
    /// scripts and other farhelm processes are the only callers.
    #[command(hide = true)]
    Internal {
        #[command(subcommand)]
        command: InternalCmd,
    },
}

#[derive(Subcommand)]
enum HelmCmd {
    /// Serve the web UI and API on loopback, connected to the registered fleet.
    Run(farhelm_helm::HelmArgs),
    /// View or rotate the browser bootstrap token on the helm's machine.
    Token {
        #[command(subcommand)]
        command: TokenCmd,
    },
}

#[derive(Subcommand)]
enum TokenCmd {
    /// Print the current token, minting it on first need.
    Show {
        /// State directory holding helm.db.
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// Replace the token and invalidate every browser device session.
    Rotate {
        /// State directory holding helm.db.
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum SupervisorCmd {
    /// Run the supervisor in the foreground (SPEC.md's no-fuss try-it
    /// path; systemd wraps this same invocation later).
    Run {
        /// State directory (default: ~/.local/state/farhelm).
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Exit when the spawning desktop app closes its inherited pipe.
        ///
        /// Hidden because ordinary foreground and systemd supervisors own
        /// their own lifetime; only the bundled desktop launcher holds the
        /// corresponding pipe open.
        #[arg(long, hide = true)]
        exit_on_stdin_close: bool,
    },
}

#[derive(Subcommand)]
enum InternalCmd {
    /// Proxy stdio to the local supervisor's unix socket. This is the
    /// remote end of the helm's ssh transport: `ssh host farhelm
    /// internal stdio` yields a byte pipe to the supervisor.
    Stdio {
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// The launch shim: exec a LaunchSpec's argv, recording exec failure
    /// to the spec's status file. See farhelm_supervisor::launch for why
    /// this exists (zsh terminates on failed exec; a shell-side sentinel
    /// can never fire there).
    Launch { spec: PathBuf },
    /// A scripted TUI standing in for real agents in tests: prompts,
    /// echoes, colors, terminal modes, and raw-byte output — deterministic
    /// and free of vendor auth (PLAN_M1.md's test harness).
    FakeAgent {
        /// Behavior script.
        #[arg(long, value_enum, default_value_t = fake_agent::Script::Basic)]
        script: fake_agent::Script,
        /// Root the record-writing scripts hang their `.claude`/`.codex`
        /// trees off, mirroring the supervisor's own injectable agent home
        /// (PLAN_M3.md item 8). A flag rather than `$HOME` because the
        /// tests that use it must not mutate the test process's
        /// environment, and because concurrent harnesses each need their
        /// own tree. Ignored by every other script.
        #[arg(long)]
        record_home: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Spawn {
            cwd,
            title,
            agent,
            parent,
            idempotency_key,
        } => {
            let child = runtime()?.block_on(spawn_session(SpawnArgs {
                cwd,
                title,
                agent,
                parent,
                idempotency_key,
            }))?;
            println!("{child}");
            Ok(())
        }
        Cmd::Helm {
            command: HelmCmd::Run(args),
        } => {
            init_tracing();
            runtime()?.block_on(farhelm_helm::run(args))
        }
        Cmd::Helm {
            command: HelmCmd::Token { command },
        } => {
            init_tracing();
            let runtime = runtime()?;
            let token = match command {
                TokenCmd::Show { state_dir } => {
                    runtime.block_on(farhelm_helm::show_token(state_dir))?
                }
                TokenCmd::Rotate { state_dir } => {
                    runtime.block_on(farhelm_helm::rotate_token(state_dir))?
                }
            };
            println!("{token}");
            Ok(())
        }
        Cmd::Supervisor {
            command:
                SupervisorCmd::Run {
                    state_dir,
                    exit_on_stdin_close,
                },
        } => {
            init_tracing();
            let dir = match state_dir {
                Some(dir) => dir,
                None => farhelm_supervisor::default_state_dir()?,
            };
            runtime()?.block_on(run_supervisor(&dir, exit_on_stdin_close))
        }
        Cmd::Internal { command } => match command {
            InternalCmd::Stdio { state_dir } => {
                // No tracing init — belt and braces, not necessity:
                // tracing goes to stderr by construction (init_tracing),
                // but this process's stdout IS the protocol channel, so
                // the proxy stays as close to a bare pipe as possible
                // rather than trusting every future logging tweak to
                // keep stdout clean. Its one diagnostic is the explicit
                // eprintln on the error path.
                let dir = match state_dir {
                    Some(dir) => dir,
                    None => farhelm_supervisor::default_state_dir()?,
                };
                runtime()?.block_on(stdio_proxy(&dir))
            }
            InternalCmd::Launch { spec } => {
                // On success exec never returns; reaching here is failure.
                Err(farhelm_supervisor::launch::exec_launch_spec(&spec))
            }
            InternalCmd::FakeAgent {
                script,
                record_home,
            } => fake_agent::run(script, record_home),
        },
    }
}

/// Run a supervisor with the optional desktop-app lifetime tether.
///
/// Child destructors are not reliable when a GUI framework terminates its
/// process directly. A pipe is an operating-system lifetime primitive: every
/// exit path closes the desktop parent's write end, so EOF releases the child
/// even when Rust cleanup never runs. The blocking read lives on a detached OS
/// thread, not Tokio's blocking pool: if the supervisor itself fails first,
/// runtime shutdown cannot wait forever for an uncancellable stdin read.
async fn run_supervisor(
    state_dir: &std::path::Path,
    exit_on_stdin_close: bool,
) -> anyhow::Result<()> {
    use anyhow::Context as _;

    if !exit_on_stdin_close {
        return farhelm_supervisor::service::run(state_dir).await;
    }
    let (stdin_closed_tx, stdin_closed) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("farhelm-supervisor-stdin-tether".to_string())
        .spawn(move || {
            let result = std::io::copy(&mut std::io::stdin(), &mut std::io::sink()).map(|_| ());
            let _ = stdin_closed_tx.send(result);
        })
        .context("starting desktop supervisor stdin watcher")?;
    tokio::select! {
        result = farhelm_supervisor::service::run(state_dir) => result,
        result = stdin_closed => {
            result.context("desktop supervisor stdin watcher stopped without reporting EOF")??;
            Ok(())
        }
    }
}

/// Parsed spawn inputs after clap has enforced the one required flag.
struct SpawnArgs {
    cwd: PathBuf,
    title: Option<String>,
    agent: Option<String>,
    parent: Option<String>,
    idempotency_key: Option<String>,
}

/// Validate the injected spawn contract before the first socket operation.
///
/// A session id with no token is the recognizable upgrade edge. Every
/// missing value names the exact variable; no default supervisor is dialed.
fn spawn_environment() -> anyhow::Result<(String, String, PathBuf)> {
    use anyhow::Context;

    let session_id = std::env::var_os(farhelm_supervisor::launch::SESSION_ID_ENV_VAR);
    let session_token = std::env::var_os(farhelm_supervisor::launch::SESSION_TOKEN_ENV_VAR);
    let supervisor_sock = std::env::var_os(farhelm_supervisor::launch::SUPERVISOR_SOCK_ENV_VAR);
    if session_id.is_some() && session_token.is_none() {
        anyhow::bail!(
            "this session predates spawn support and must be restarted before running \
             farhelm spawn"
        );
    }
    let socket = supervisor_sock
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} is required; farhelm spawn will not guess which supervisor to dial",
                farhelm_supervisor::launch::SUPERVISOR_SOCK_ENV_VAR
            )
        })?
        .into_string()
        .map_err(|_| anyhow::anyhow!("FARHELM_SUPERVISOR_SOCK is not valid UTF-8"))?;
    let session_id = session_id
        .context("FARHELM_SESSION_ID is required inside a Farhelm session")?
        .into_string()
        .map_err(|_| anyhow::anyhow!("FARHELM_SESSION_ID is not valid UTF-8"))?;
    let token = session_token
        .context("FARHELM_SESSION_TOKEN is required inside a Farhelm session")?
        .into_string()
        .map_err(|_| anyhow::anyhow!("FARHELM_SESSION_TOKEN is not valid UTF-8"))?;
    Ok((session_id, token, PathBuf::from(socket)))
}

/// Create one child under the environment's session authority.
///
/// This is the centralized scripting contract for `farhelm spawn`: validate
/// all three injected environment values before dialing, preserve the cwd's
/// lexical spelling (resolving only a relative input against this process's
/// cwd), authenticate the connection, and return the child id for the sole
/// stdout line. A `SessionCreated` reply means creation succeeded regardless
/// of the status snapshot it carries; every refusal and protocol mismatch is
/// an error and therefore produces no id.
async fn spawn_session(args: SpawnArgs) -> anyhow::Result<String> {
    use anyhow::Context;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake_with_session_auth, parse_control};
    use farhelm_proto::{ControlMsg, SessionAuth};

    let (session_id, token, socket) = spawn_environment()?;
    let cwd = if args.cwd.is_absolute() {
        args.cwd
    } else {
        std::env::current_dir()
            .context("reading farhelm spawn's current directory")?
            .join(args.cwd)
    };
    let cwd = cwd
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("spawn working directory is not valid UTF-8"))?
        .to_string();

    let stream = tokio::net::UnixStream::connect(&socket)
        .await
        .with_context(|| format!("connecting to supervisor socket {}", socket.display()))?;
    let (read, write) = tokio::io::split(stream);
    let mut reader = FrameReader::new(read);
    let mut writer = FrameWriter::new(write);
    handshake_with_session_auth(&mut reader, &mut writer, SessionAuth { session_id, token })
        .await
        .context("performing the authenticated supervisor handshake")?;

    const REQUEST_ID: u64 = 1;
    writer
        .write_control(&ControlMsg::CreateSession {
            req_id: REQUEST_ID,
            parent: args.parent,
            cwd,
            invocation: None,
            profile_id: None,
            profile_name: args.agent,
            title: args.title,
            cols: 80,
            rows: 24,
            intent_key: args.idempotency_key,
            agent_kind: None,
            resume_template: None,
        })
        .await
        .context("sending the spawn request")?;

    let frame = reader
        .read_frame()
        .await
        .context("reading the spawn reply")?
        .ok_or_else(|| anyhow::anyhow!("the supervisor closed before answering spawn"))?;
    let message = parse_control(&frame).context("decoding the spawn reply")?;
    match message {
        ControlMsg::SessionCreated {
            req_id: REQUEST_ID,
            session,
        } => Ok(session.id),
        ControlMsg::Error {
            req_id: 0 | REQUEST_ID,
            message,
            ..
        } => anyhow::bail!(message),
        unexpected => {
            anyhow::bail!("the supervisor sent an unexpected spawn reply: {unexpected:?}")
        }
    }
}

/// A multi-threaded tokio runtime, built per subcommand rather than by a
/// `#[tokio::main]` on `main`: `internal launch` execs and must never pay
/// for (or be complicated by) a runtime it will replace, and `main` stays
/// synchronous so that path is obvious.
fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Runtime::new()?)
}

/// Logging to stderr, always. stdout belongs to the protocol under
/// `internal stdio` and to machine-readable output elsewhere, so a stray
/// log line on it would corrupt frames rather than merely look untidy.
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
}

/// Pump bytes both ways between our stdio and the supervisor socket.
/// Deliberately dumb: framing, hello, and versioning belong to the two
/// endpoints, not the pipe between them.
///
/// Stdin EOF half-closes the socket's write side rather than tearing the
/// proxy down, so frames already in flight from the supervisor still
/// reach the helm. The two pumps are selected only until stdin finishes:
/// clean EOF keeps the downstream pump alive, while an upstream error
/// terminates the otherwise read-only proxy.
///
/// Failure to make the initial local socket connection exits 75 only for
/// not-found/refused. Provisioning treats that status as positive absence;
/// every other failure remains an ordinary error and must never offer setup.
async fn stdio_proxy(state_dir: &std::path::Path) -> anyhow::Result<()> {
    use anyhow::Context;
    use tokio::io::AsyncWriteExt;

    let stream = farhelm_supervisor::service::connect(state_dir)
        .await
        .unwrap_or_else(|error| {
            // Provisioning may offer setup only after the command itself ran
            // and positively established that no supervisor answered. A
            // dedicated exit status carries exactly that evidence through
            // ssh; authentication failures and missing remote commands use
            // different statuses and therefore remain probe errors.
            let absent = error.chain().any(|cause| {
                cause.downcast_ref::<std::io::Error>().is_some_and(|error| {
                    matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                    )
                })
            });
            eprintln!("farhelm internal stdio: {error:#}");
            std::process::exit(if absent { 75 } else { 1 });
        });
    let (mut sock_r, mut sock_w) = tokio::io::split(stream);
    let mut upstream = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        tokio::io::copy(&mut stdin, &mut sock_w)
            .await
            .context("copying stdin to supervisor")?;
        sock_w
            .shutdown()
            .await
            .context("half-closing supervisor socket")
    });
    let mut stdout = tokio::io::stdout();
    let downstream = async {
        tokio::io::copy(&mut sock_r, &mut stdout).await?;
        stdout.flush().await
    };
    tokio::pin!(downstream);

    // A clean stdin EOF only half-closes the upstream and keeps waiting
    // for in-flight replies. An upstream failure is different: no more
    // request bytes can reach the supervisor, so reporting success—or
    // leaving a read-only proxy parked on downstream output—would hide
    // the broken transport.
    let result = tokio::select! {
        downstream_result = &mut downstream => downstream_result.map_err(anyhow::Error::from),
        upstream_result = &mut upstream => match upstream_result {
            Ok(Ok(())) => downstream.await.map_err(anyhow::Error::from),
            Ok(Err(error)) => Err(error),
            Err(error) => Err(anyhow::Error::new(error).context("stdin proxy task failed")),
        },
    };

    // Exit the PROCESS, never return — on success AND on error. The
    // upstream half may still be parked in tokio's Stdin, which is an
    // uncancelable blocking read on the blocking pool: aborting does not
    // unblock it, and dropping the runtime on return WAITS for it, so
    // the proxy would linger with its stdout open. Over ssh that
    // lingering process keeps the channel alive, the helm never sees
    // EOF, and a supervisor crash turns into a silently frozen terminal
    // instead of a prompt `Detached`. A `?` here would take exactly that
    // path on a socket error (an ECONNRESET from a crashed supervisor is
    // the realistic case), which is why the error is reported by hand.
    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("farhelm internal stdio: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both token verbs accept the state directory after the verb, matching
    /// the command shape the e2e harness and user-facing plan document.
    #[test]
    fn token_cli_parses_show_and_rotate_with_an_explicit_state_dir() {
        for verb in ["show", "rotate"] {
            let cli = Cli::try_parse_from([
                "farhelm",
                "helm",
                "token",
                verb,
                "--state-dir",
                "/tmp/farhelm-test-state",
            ])
            .unwrap();
            assert!(matches!(
                cli.command,
                Cmd::Helm {
                    command: HelmCmd::Token { .. }
                }
            ));
        }
    }
}
