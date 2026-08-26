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
mod hook;
mod setup;

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
        /// process's real current directory before crossing the wire;
        /// `~` and `~/path` are forwarded as written and expand on the
        /// supervisor (`~user` forms are refused there).
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
    /// Install (or remove) the systemd user units that run this helm and
    /// its supervisor on this machine.
    Setup(setup::SetupOptions),
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
        /// Drive this tmux binary instead of the one on PATH.
        ///
        /// Overrides FARHELM_TMUX; with neither, plain `tmux` off PATH is
        /// used. The binary is version-checked like any other and refused
        /// by name if it is older than Farhelm's floor. This is a "you own
        /// the substrate" knob, not a supported configuration: Farhelm
        /// drives tmux harder than interactive use does, versions below
        /// the floor have crashed under it, and versions above the tested
        /// one are unaudited.
        ///
        /// Only tmux's stable releases and single-letter patch releases
        /// are recognized (3.7, 3.7c, 3.10). Its development and
        /// release-candidate spellings — next-3.8, 3.8-rc, 3.8-rc2 — are
        /// refused whatever they are pointed at, because Farhelm has no
        /// defined ordering for those stages against a stable release.
        #[arg(long = "tmux", value_name = "PATH")]
        tmux: Option<PathBuf>,
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
    /// The agent's `SessionStart` hook: read the vendor's JSON payload
    /// from stdin and report the conversation id it names to the
    /// supervisor that launched this session.
    ///
    /// Farhelm injects `<farhelm_exe> internal hook` into the agent's own
    /// launch, so this runs as a child of the agent, inside the user's
    /// terminal, with the session credential in its environment. It takes
    /// no flags: everything it needs arrives on stdin or in that
    /// environment, and a flag would be one more thing the injected
    /// command line could get wrong. See `hook.rs` for the silence and
    /// budget contract this arm exists to honour.
    Hook,
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
        /// Whatever the supervisor appends for the real vendor after the
        /// fixture's own flags — the per-launch hook flags, or anything a
        /// test's resume template places there. Every script tolerates the
        /// tail; only the record-writing scripts (`claude-record`,
        /// `codex-record`) print the process argv in their `FAKE-AGENT
        /// ARGV:` marker, which is where a test asserts on injection
        /// without the fixture understanding the injected flags.
        ///
        /// The conversation-capture and restart integration fixtures
        /// (`conversation_identity_capture.rs`, `restart_with_resume.rs`)
        /// ARE this binary, symlinked as `claude` or `codex` and invoked as
        /// `internal fake-agent`. A supervisor that appends real-vendor
        /// flags to that argv the way it does for the genuine CLI —
        /// `--settings <json>` for Claude-kind sessions, or
        /// `--dangerously-bypass-hook-trust -c ... -c ...` for Codex-kind
        /// ones — would otherwise hand clap flags it has no arm for, and
        /// clap rejects the whole argv with a usage error, killing the
        /// fixture before a single line of script output is produced.
        /// `trailing_var_arg` plus `allow_hyphen_values` make capture begin
        /// at the first token clap does not recognize (`--script` and
        /// `--record-home` still bind normally ahead of it), the same
        /// tolerance a real vendor CLI has for flags appended after its own.
        /// Order is preserved so the marker can be asserted on verbatim.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra: Vec<String>,
    },
    /// Reap dead test-harness state dirs under /tmp (see farhelm_teststate
    /// for the naming scheme and flock liveness protocol). Called by
    /// e2e/start-stack.sh at stack startup; the Rust integration tests
    /// reach the same sweep in-process. Best-effort and always exits 0 —
    /// a broken sweep must not block testing.
    SweepTestState,
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
            command: HelmCmd::Setup(options),
        } => run_helm_setup(options),
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
                    tmux,
                    exit_on_stdin_close,
                },
        } => {
            init_tracing();
            let dir = match state_dir {
                Some(dir) => dir,
                None => farhelm_supervisor::default_state_dir()?,
            };
            // Resolved here and nowhere else: this is the supervisor's one
            // startup, so a single resolution is what makes "every launch
            // path honors the override" true rather than aspirational.
            let tmux = farhelm_supervisor::tmux::resolve_tmux_program_from_env(tmux.as_deref());
            // The same rule, for the same reason, one line down: this is
            // the ONLY read of `FARHELM_AGENT_HOOKS` in the codebase (plan
            // D5). Everything downstream consults the parsed seam value, so
            // no launch path can observe a variable that changed under a
            // running supervisor, and no test ever has to mutate its own
            // process's environment to exercise the opt-out. An unset
            // variable is not an error — it is the default, "hook every
            // integrated kind" — so `NotPresent` takes the default in
            // silence.
            //
            // A value that is not UTF-8 is a different thing and gets a
            // line: nobody types one on purpose, so it is a mistake worth
            // naming, and `parse_agent_hooks` (a `&str` parser) cannot be
            // shown it to complain on its own. The fallback direction is
            // the same one an unrecognized token takes, and for the same
            // reason: this variable is an opt-OUT, and a value nobody can
            // read must not silently become "opt out of everything".
            // `init_tracing()` has already run in this arm, so the warning
            // actually reaches stderr.
            let agent_hooks = match std::env::var("FARHELM_AGENT_HOOKS") {
                Ok(value) => farhelm_supervisor::agent_kind::parse_agent_hooks(&value),
                Err(std::env::VarError::NotPresent) => {
                    farhelm_supervisor::agent_kind::AgentHooks::default()
                }
                Err(std::env::VarError::NotUnicode(_)) => {
                    tracing::warn!(
                        "FARHELM_AGENT_HOOKS is not valid UTF-8 and cannot be parsed; falling \
                         back to the default (every kind hooked) rather than guessing what was \
                         meant"
                    );
                    farhelm_supervisor::agent_kind::AgentHooks::default()
                }
            };
            let startup = farhelm_supervisor::service::SupervisorStartup {
                tmux_program: tmux,
                agent_hooks,
            };
            runtime()?.block_on(run_supervisor(&dir, startup, exit_on_stdin_close))
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
            InternalCmd::Hook => {
                // No tracing init, and this one is necessity rather than
                // belt and braces: init_tracing logs to stderr at `info`,
                // and this process's stderr is the AGENT's terminal. A
                // single log line here is a line the user sees in the
                // middle of their session — or, on a non-zero exit, one
                // the vendor surfaces as a hook error.
                //
                // The panic hook goes in before anything can panic: the
                // default one prints to that same stderr. hook::run_with
                // catches the unwind itself; this only silences it.
                std::panic::set_hook(Box::new(|_| {}));

                // The environment read lives here, not in hook.rs: the
                // tests over there must not depend on the environment of
                // the process running them (a test suite run from inside a
                // farhelm session already carries all three variables and
                // would otherwise dial a live supervisor).
                //
                // Read variable by variable rather than through
                // `spawn_environment`, which answers a different question
                // ("may `farhelm spawn` dial?") and answers it
                // all-or-nothing. The two outputs below do not need the
                // same inputs: reporting needs all three values, while the
                // LOG PATH needs only the session id and the socket's
                // directory. Deriving them together is what used to make a
                // half-configured environment — an id and a socket with the
                // token missing — leave no trace at all, which is precisely
                // the situation whose only evidence would have been this
                // file. Now such a run still writes its `no-credential`
                // line.
                //
                // A missing or non-UTF-8 value is treated as absent
                // throughout: the supervisor's own ids and paths are UTF-8
                // strings, so a value that is not one cannot be ours. No
                // default supervisor is ever guessed — the socket comes
                // from FARHELM_SUPERVISOR_SOCK or the run reports nothing.
                let session_id = std::env::var(farhelm_supervisor::launch::SESSION_ID_ENV_VAR).ok();
                let token = std::env::var(farhelm_supervisor::launch::SESSION_TOKEN_ENV_VAR).ok();
                let socket = std::env::var(farhelm_supervisor::launch::SUPERVISOR_SOCK_ENV_VAR)
                    .ok()
                    .map(PathBuf::from);

                // <state_dir>/hook-log/<session>.log, where the state dir
                // is the socket's own directory — the derivation the
                // supervisor mirrors in `hook_log_path`; change one and you
                // must change the other.
                let hook_log = match (&session_id, &socket) {
                    (Some(id), Some(socket)) => socket
                        .parent()
                        .map(|dir| dir.join("hook-log").join(format!("{id}.log"))),
                    _ => None,
                };
                let credential = match (session_id, token, socket) {
                    (Some(session_id), Some(token), Some(socket)) => Some(hook::HookCredential {
                        session_id,
                        token,
                        socket,
                    }),
                    // Anything short of all three is "no credential":
                    // there is no supervisor to report to, whether the
                    // environment is absent entirely or a session
                    // predating spawn support left the token out.
                    _ => None,
                };

                // Well under the timeout the injected hook config gives
                // the vendor, so the vendor never gets to time us out.
                const BUDGET: std::time::Duration = std::time::Duration::from_secs(2);
                hook::run_with(credential, std::io::stdin(), BUDGET, hook_log);

                // Exit rather than return, for the STATUS above all: 0 is
                // decided right here, unconditionally, rather than by
                // whatever `main`'s shared return path grows later — a
                // non-zero exit is what makes the vendor show the user an
                // error about a hook that is, by contract, allowed to fail
                // silently.
                //
                // It also draws the line under the budget, though not for
                // the reason it might look like: no runtime is waiting,
                // because `run_with` builds and drops its own inside that
                // call. What is still out there is the detached
                // payload-reader thread, possibly blocked forever on a pipe
                // the vendor holds open. Abandoning it is the design (see
                // `hook::read_payload`), and terminating here is what makes
                // the abandonment immediate instead of merely eventual.
                std::process::exit(0);
            }
            InternalCmd::FakeAgent {
                script,
                record_home,
                // The fake agent does not need to understand the injected
                // tail — it only has to survive parsing it. The record
                // scripts read the same strings straight from
                // `std::env::args()` for their `FAKE-AGENT ARGV:` marker,
                // so nothing is passed through here.
                extra: _,
            } => fake_agent::run(script, record_home),
            InternalCmd::SweepTestState => {
                let outcome = farhelm_teststate::sweep(
                    std::path::Path::new(farhelm_teststate::TMP_ROOT),
                    &farhelm_teststate::SweepPolicy::default(),
                );
                farhelm_teststate::report(&outcome);
                Ok(())
            }
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
///
/// `startup` arrives ALREADY resolved — the tmux program (`--tmux` over
/// `FARHELM_TMUX` over `PATH`) and the `FARHELM_AGENT_HOOKS` opt-out — and
/// is passed straight through. Resolving either here instead would put the
/// decision on both sides of the tether branch, which is one place too many
/// for values that must be identical on every launch path.
async fn run_supervisor(
    state_dir: &std::path::Path,
    startup: farhelm_supervisor::service::SupervisorStartup,
    exit_on_stdin_close: bool,
) -> anyhow::Result<()> {
    use anyhow::Context as _;

    if !exit_on_stdin_close {
        return farhelm_supervisor::service::run(state_dir, startup).await;
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
        result = farhelm_supervisor::service::run(state_dir, startup) => result,
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
/// lexical spelling (an ordinary relative input resolves against this
/// process's cwd; a `~`-prefixed input is forwarded verbatim for the
/// supervisor's own expansion — see the branch below for why absolutizing
/// it would be wrong), authenticate the connection, and return the child id
/// for the sole stdout line. A `SessionCreated` reply means creation
/// succeeded regardless of the status snapshot it carries; every refusal
/// and protocol mismatch is an error and therefore produces no id.
async fn spawn_session(args: SpawnArgs) -> anyhow::Result<String> {
    use anyhow::Context;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake_with_session_auth, parse_control};
    use farhelm_proto::{ControlMsg, SessionAuth};

    let (session_id, token, socket) = spawn_environment()?;
    // `~`-prefixed paths are forwarded verbatim: the SUPERVISOR owns that
    // contract (SPEC.md — `~` expands against its own home, `~user` is its
    // refusal to give), and a spawn always targets the same host it runs
    // on, so nothing is gained by resolving locally. Absolutizing them
    // here would instead manufacture `<cwd>/~...` — a path that at best
    // fails as nonexistent and at worst names a real directory literally
    // called `~user`, silently dodging the supervisor's refusal. Ordinary
    // relative paths keep resolving against this process's cwd, which is
    // the spelling a shell user means.
    let cwd = if args.cwd.is_absolute() || args.cwd.to_str().is_some_and(|c| c.starts_with('~')) {
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

/// Capture the whole environment `farhelm helm setup` is allowed to
/// depend on, once, and hand it to the command.
///
/// This is the ONLY place those variables are read. Everything the setup
/// path decides — the unit directory, the default state directory, which
/// tmux to pin, whether the binary looks like a build artifact — follows
/// from this one capture, which is what makes the command testable
/// without a test ever mutating its own process environment.
///
/// No tokio runtime: setup is synchronous, spawns `systemctl` with
/// `std::process::Command`, and has nothing to await.
#[cfg(target_os = "linux")]
fn run_helm_setup(options: setup::SetupOptions) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .context("HOME is not set, so farhelm helm setup cannot tell where your units belong")?;
    let context = setup::SetupContext {
        exe: std::env::current_exe().context("locating the running farhelm binary")?,
        home: PathBuf::from(home),
        cwd: std::env::current_dir()
            .context("locating the directory farhelm helm setup was run from")?,
        path: std::env::var_os("PATH").unwrap_or_default(),
        xdg_config_home: std::env::var_os("XDG_CONFIG_HOME"),
        xdg_state_home: std::env::var_os("XDG_STATE_HOME"),
        tmux_env: std::env::var_os(farhelm_supervisor::tmux::TMUX_PROGRAM_ENV),
        temp_dir: std::env::temp_dir(),
    };
    let mut units = setup::SystemctlUnitManager;
    let mut out = std::io::stdout().lock();
    setup::run_setup(&context, &options, &mut units, &mut out)
}

/// What a non-Linux `farhelm helm setup` says before exiting 2.
///
/// A constant so the text is testable on every platform: the arm that
/// prints it calls `process::exit`, so only a child process can observe
/// the real thing, and the wording is the whole behaviour there.
pub const NON_LINUX_SETUP_MESSAGE: &str = "farhelm helm setup manages systemd user units and only runs on Linux; on macOS run \
     farhelm-desktop, which starts its own helm and supervisor";

/// systemd user units are a Linux mechanism, and the Mac has its own
/// answer (the desktop app owns a helm and a supervisor of its own), so
/// there is nothing here to degrade gracefully into.
#[cfg(not(target_os = "linux"))]
fn run_helm_setup(_options: setup::SetupOptions) -> anyhow::Result<()> {
    eprintln!("{NON_LINUX_SETUP_MESSAGE}");
    std::process::exit(2);
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

    /// On a Mac, this message IS `farhelm helm setup` — the arm prints it
    /// and exits 2, and nothing else happens. Only a child process can
    /// observe that exit, so the wording is pinned here instead, on every
    /// platform, where a rewrite that dropped the pointer to the desktop
    /// app would fail the build rather than ship silently.
    #[test]
    fn the_non_linux_setup_message_names_the_platform_and_the_alternative() {
        assert_eq!(
            NON_LINUX_SETUP_MESSAGE,
            "farhelm helm setup manages systemd user units and only runs on Linux; on macOS run \
             farhelm-desktop, which starts its own helm and supervisor"
        );
    }

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

    /// The sweep verb parses under the hidden internal namespace — the
    /// exact invocation e2e/start-stack.sh performs, pinned here so a
    /// rename breaks a test instead of silently breaking the script's
    /// startup sweep.
    #[test]
    fn internal_sweep_test_state_parses() {
        let cli = Cli::try_parse_from(["farhelm", "internal", "sweep-test-state"]).unwrap();
        assert!(matches!(
            cli.command,
            Cmd::Internal {
                command: InternalCmd::SweepTestState
            }
        ));
    }

    /// A supervisor that injects real-vendor hook flags past `--script`/
    /// `--record-home` must not turn the claude/codex-kind test fixtures
    /// into a clap usage error. This exercises a Claude-shaped tail
    /// (`--settings <json>`) immediately followed by a Codex-shaped one
    /// (`--dangerously-bypass-hook-trust -c a=b`) plus one more unknown
    /// flag pair (`--resume <id>`, purely a synthetic example of a tail
    /// the fixture has never heard of), and checks `extra` captures every
    /// one of those tokens untouched and in argv order — the exact
    /// property the `FAKE-AGENT ARGV:` marker relies on to let a test
    /// assert injection happened without the fixture parsing the injected
    /// flags itself.
    #[test]
    fn internal_fake_agent_parses_with_injected_vendor_tail() {
        let cli = Cli::try_parse_from([
            "farhelm",
            "internal",
            "fake-agent",
            "--script",
            "basic",
            "--settings",
            r#"{"x":1}"#,
            "--dangerously-bypass-hook-trust",
            "-c",
            "a=b",
            "--resume",
            "conv-1",
        ])
        .unwrap();
        let Cmd::Internal {
            command: InternalCmd::FakeAgent { script, extra, .. },
        } = cli.command
        else {
            panic!("expected InternalCmd::FakeAgent");
        };
        assert!(matches!(script, fake_agent::Script::Basic));
        assert_eq!(
            extra,
            vec![
                "--settings",
                r#"{"x":1}"#,
                "--dangerously-bypass-hook-trust",
                "-c",
                "a=b",
                "--resume",
                "conv-1",
            ]
        );
    }

    /// Companion to the tail-injection test above: `trailing_var_arg`
    /// only starts collecting once clap can no longer match a known flag,
    /// so this pins that `--script` and `--record-home` still bind to
    /// their own fields — rather than being swallowed into `extra` — when
    /// an injected tail follows them on the same command line.
    #[test]
    fn internal_fake_agent_named_flags_still_parse_before_the_tail() {
        let cli = Cli::try_parse_from([
            "farhelm",
            "internal",
            "fake-agent",
            "--script",
            "claude-record",
            "--record-home",
            "/tmp/fake-agent-home",
            "--settings",
            "{}",
        ])
        .unwrap();
        let Cmd::Internal {
            command:
                InternalCmd::FakeAgent {
                    script,
                    record_home,
                    extra,
                },
        } = cli.command
        else {
            panic!("expected InternalCmd::FakeAgent");
        };
        assert!(matches!(script, fake_agent::Script::ClaudeRecord));
        assert_eq!(
            record_home,
            Some(std::path::PathBuf::from("/tmp/fake-agent-home"))
        );
        assert_eq!(extra, vec!["--settings", "{}"]);
    }
}
