//! The `farhelm` multi-call binary.
//!
//! One artifact carries every role — `helm run`, `supervisor run`, and
//! the hidden `internal` namespace — because provisioning copies exactly
//! one binary to a host and the launch shim must exist inside every
//! session without separate installation (SPEC_impl.md, "CLI").
//! `farhelm spawn` is a later milestone; the grammar here is the subset
//! of SPEC_impl.md's CLI section that exists so far.

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
    /// Serve the web UI and API on loopback, connected to one supervisor.
    Run(farhelm_helm::HelmArgs),
}

#[derive(Subcommand)]
enum SupervisorCmd {
    /// Run the supervisor in the foreground (SPEC.md's no-fuss try-it
    /// path; systemd wraps this same invocation later).
    Run {
        /// State directory (default: ~/.local/state/farhelm).
        #[arg(long)]
        state_dir: Option<PathBuf>,
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
        Cmd::Helm {
            command: HelmCmd::Run(args),
        } => {
            init_tracing();
            runtime()?.block_on(farhelm_helm::run(args))
        }
        Cmd::Supervisor {
            command: SupervisorCmd::Run { state_dir },
        } => {
            init_tracing();
            let dir = match state_dir {
                Some(dir) => dir,
                None => farhelm_supervisor::default_state_dir()?,
            };
            runtime()?.block_on(farhelm_supervisor::service::run(&dir))
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
async fn stdio_proxy(state_dir: &std::path::Path) -> anyhow::Result<()> {
    use anyhow::Context;
    use tokio::io::AsyncWriteExt;

    let stream = farhelm_supervisor::service::connect(state_dir).await?;
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
