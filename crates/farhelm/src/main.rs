//! The `farhelm` multi-call binary.
//!
//! One artifact carries every role — helm serving and token management,
//! `supervisor run`, and the hidden `internal` namespace — because
//! provisioning copies exactly one binary to a host and the launch shim must
//! exist inside every session without separate installation (SPEC_impl.md,
//! "CLI").
//! The two in-session commands keep stdout machine-readable, with every
//! diagnostic on stderr: `farhelm spawn`'s only successful output is the
//! child id, and `farhelm agent`'s is the listing it was asked for
//! (`hosts`/`sessions`), the one-line confirmation of a lifecycle action
//! (`rename`/`stop`/`restart`), or — for the two creating verbs
//! (`create`/`clone`) — the NEW SESSION'S ID and nothing else, with the
//! human-readable confirmation on stderr beside it. That last shape is
//! `spawn`'s contract deliberately: the created id is the one agent output
//! meant to be captured as a SINGLE VALUE, since it is what a caller goes
//! on to pass as `--session`. The listings are machine-readable too — a
//! fixture and a README example both parse the hosts table — but they are
//! parsed as a table, and a table that grew a column would still be one.
//! An id that grew a confirmation line beside it would not. They share the
//! injected-environment contract and the one-request round trip
//! ([`agent_client`]) and nothing else — a spawn is answered by the
//! supervisor on the other end of the socket, while an agent request is
//! relayed by it to the helm.
//!
//! `farhelm spawn` and `farhelm agent create` overlap and both stay: a
//! spawn creates on the host it runs on, answered by that supervisor
//! alone, and works with no helm attached; `agent create` goes through the
//! helm and can therefore name any host in the fleet. The first is the
//! scripting primitive, the second is the fleet-aware one.
//!
//! `farhelm agent instructions` is the exception on both counts, and
//! deliberately so: its stdout is prose for a language model rather than a
//! table, and it needs no session at all. It is the command an agent runs
//! first, on the strength of one line the identity hook printed at it, so
//! it must work in a session whose relay is broken — see
//! [`agent_instructions`].

use agent_client::{SessionEnv, SpawnArgs, agent_request, spawn_session};
use clap::{Parser, Subcommand, ValueEnum};
use farhelm_proto::AgentReply;
use render::{host_cell, quoted, render_agent_reply, safe_cell, truncation_notice};
use std::io::Write;
use std::path::PathBuf;

mod agent_client;
mod agent_instructions;
mod goose_hook;
mod hook;
mod render;
mod setup;
mod uninstall;

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
    /// Remove this standalone installation, retaining user data.
    Uninstall(uninstall::Options),
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
        /// Exact agent profile name, resolved by the attached helm.
        #[arg(long, conflicts_with_all = ["profile_id", "inherit_agent"], required_unless_present_any = ["profile_id", "inherit_agent"])]
        agent: Option<String>,
        /// Exact profile id from `farhelm agent profiles`.
        #[arg(long, conflicts_with_all = ["agent", "inherit_agent"], required_unless_present_any = ["agent", "inherit_agent"])]
        profile_id: Option<String>,
        /// Deliberately copy this session's stored agent bundle.
        #[arg(long, conflicts_with_all = ["agent", "profile_id"], required_unless_present_any = ["agent", "profile_id"])]
        inherit_agent: bool,
        /// Organizational parent id. Never defaults to this session.
        #[arg(long)]
        parent: Option<String>,
        /// Retry key, valid only for the child session's lifetime.
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    /// Ask the helm about the fleet, or act on it, from inside a Farhelm
    /// session — `hosts`/`sessions` are read-only questions,
    /// `rename`/`stop`/`restart` are fleet-wide lifecycle actions, and
    /// `create`/`clone` put a new session on any host in the fleet.
    ///
    /// `disable_help_subcommand` because [`AgentCmd::Help`] is farhelm's
    /// own: `farhelm agent help` prints the agent-facing instructions, not
    /// clap's usage screen. `--help` is untouched and still prints the
    /// usage screen, which is the surface a human wants.
    #[command(disable_help_subcommand = true)]
    Agent {
        #[command(subcommand)]
        command: AgentCmd,
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

/// The verbs `farhelm agent` currently carries.
///
/// `Instructions` and `Help` render locally; the other verbs use the
/// attached helm's relay. See [`AgentCmd::verb`] for that boundary.
///
/// The creating verbs print DIFFERENTLY from every other verb here, and
/// the difference is a contract rather than a style: their stdout is the
/// new session's id and nothing else, exactly as `farhelm spawn`'s is, so
/// a caller can capture it. The human-readable confirmation goes to
/// stderr. That is the opposite of the lifecycle verbs, whose confirmation
/// IS their stdout — those act on a session the caller already named, so
/// there is no new identifier to hand back.
///
/// Every value-taking option on the CREATING verbs carries
/// `allow_hyphen_values`, and that is a rule about where the values are
/// judged rather than a per-flag convenience. A host name, a directory, a
/// profile name, an invocation, a title and an idempotency key are all
/// plain strings that something DOWNSTREAM decides the legality of — the
/// helm's registry and profile catalog, the target supervisor's filesystem,
/// and the relay's own byte caps. Every one of them may legally begin with `-`, and
/// without the allowance clap refuses such a value here as an unrecognized
/// option, which turns "the target will tell you why that name is wrong"
/// into "this CLI would not even carry your name". The cost is the usual
/// one and is accepted knowingly: a caller who forgets a value gets the
/// NEXT flag consumed as that value, and a refusal from the far end rather
/// than a usage error here.
///
/// Each variant's `///` doc comment is AGENT-FACING PROSE, not only help
/// text: [`agent_instructions`] walks this enum's clap definition and
/// prints each verb's `about` as its one-line meaning. Write them as
/// sentences that stand alone, because that is where they are read.
#[derive(Subcommand)]
enum AgentCmd {
    /// List the hosts the helm knows, marking this session's own.
    Hosts {
        /// Print the versioned machine-readable discovery envelope.
        #[arg(long)]
        json: bool,
    },
    /// List the sessions the helm knows, marking this one.
    Sessions {
        /// Print the versioned machine-readable discovery envelope.
        #[arg(long)]
        json: bool,
    },
    /// List the helm-wide profile catalog without private launch data.
    Profiles {
        /// Print the versioned machine-readable discovery envelope.
        #[arg(long)]
        json: bool,
    },
    /// Rename an explicitly named session if its title is unchanged.
    Rename {
        /// The new title, forwarded to the helm verbatim.
        ///
        /// TWO refusals stand between it and the target, both enforced by
        /// the supervisor rather than here: SPEC.md's control-character
        /// rule, and the 64 KiB field cap (`CREATE_FIELD_CAP`) the relay's
        /// own `validate_agent_verb` applies at the first hop. Anything
        /// else — leading hyphens, punctuation, the empty string — is a
        /// legal title.
        ///
        /// `allow_hyphen_values` exists because of the first of those:
        /// since a leading hyphen is legal in a title, clap would otherwise
        /// misparse one as an unrecognized flag before it ever reached the
        /// wire.
        #[arg(allow_hyphen_values = true)]
        title: String,
        /// Exact session id from `farhelm agent sessions`.
        #[arg(long = "session", allow_hyphen_values = true)]
        session: String,
        /// Exact title observed in the discovery result, including empty.
        #[arg(long = "expected-title", allow_hyphen_values = true)]
        expected_title: String,
    },
    /// Stop an explicitly named session's agent process tree.
    Stop {
        /// Exact session id from `farhelm agent sessions`.
        #[arg(long = "session", allow_hyphen_values = true)]
        session: String,
    },
    /// Restart an explicitly named session using its advertised mode.
    Restart {
        /// Exact session id from `farhelm agent sessions`.
        #[arg(long = "session", allow_hyphen_values = true)]
        session: String,
        /// Restart behavior selected from the session's OFFER column.
        #[arg(long, value_enum)]
        mode: AgentRestartMode,
        /// Permit stopping the target only if it is still running when the
        /// owning supervisor handles this request.
        #[arg(long)]
        stop_if_running: bool,
    },
    /// Create a session on any host; prints its id.
    Create {
        /// Working directory for the new session, on the TARGET host.
        ///
        /// Required, and never defaulted to this session's own directory:
        /// a create that silently inherited it would be a clone wearing
        /// another verb's name, and `clone` is right there.
        ///
        /// A plain `String`, not a `PathBuf`: this path is interpreted on
        /// whichever host the session lands on, so resolving it against
        /// THIS process's current directory — which is what `farhelm
        /// spawn` correctly does for its own same-host create — would
        /// invent a path that means nothing over there.
        #[arg(long, value_name = "DIR", allow_hyphen_values = true)]
        cwd: String,
        /// Host to create on, by the name `farhelm agent hosts` shows.
        #[arg(long, value_name = "NAME", allow_hyphen_values = true)]
        host: String,
        /// Agent profile name, resolved in the helm-wide catalog.
        #[arg(
            long,
            value_name = "NAME",
            conflicts_with_all = ["profile_id", "invocation"],
            required_unless_present_any = ["profile_id", "invocation"],
            allow_hyphen_values = true
        )]
        profile: Option<String>,
        /// Exact profile id from `farhelm agent profiles`.
        #[arg(long, value_name = "ID", conflicts_with_all = ["profile", "invocation"], required_unless_present_any = ["profile", "invocation"], allow_hyphen_values = true)]
        profile_id: Option<String>,
        /// Command line to run instead of a profile.
        #[arg(long, value_name = "CMD", conflicts_with_all = ["profile", "profile_id"], required_unless_present_any = ["profile", "profile_id"], allow_hyphen_values = true)]
        invocation: Option<String>,
        /// Display title; omitted derives one from the directory.
        #[arg(long, value_name = "TITLE", allow_hyphen_values = true)]
        title: Option<String>,
        /// Retry key: the same key creates the session only once.
        #[arg(long, value_name = "KEY", allow_hyphen_values = true)]
        idempotency_key: Option<String>,
    },
    /// Copy an explicitly named session onto any host; prints the new id.
    Clone {
        /// Exact source id from `farhelm agent sessions`.
        #[arg(long = "source-session", allow_hyphen_values = true)]
        source_session: String,
        /// Host to create on, by the name `farhelm agent hosts` shows.
        #[arg(long, value_name = "NAME", allow_hyphen_values = true)]
        host: String,
        /// Working directory; omitted copies this session's.
        #[arg(long, value_name = "DIR", allow_hyphen_values = true)]
        cwd: Option<String>,
        /// Display title; omitted copies this session's.
        #[arg(long, value_name = "TITLE", allow_hyphen_values = true)]
        title: Option<String>,
        /// Retry key: the same key creates the session only once.
        #[arg(long, value_name = "KEY", allow_hyphen_values = true)]
        idempotency_key: Option<String>,
    },
    /// Print how to use these verbs, for an agent that was told to.
    Instructions,
    /// The same as instructions; both spellings print it.
    Help,
}

impl AgentCmd {
    /// The relay question this verb asks, or `None` for one answered
    /// locally.
    ///
    /// The distinction is the whole reason this exists rather than a bare
    /// `match` at the call site: `instructions` and `help` must work with
    /// no supervisor, no credential, and no helm anywhere in sight. An
    /// agent that has just been handed the pointer line has no way to know
    /// whether its session is attached to anything, and the one command
    /// that teaches it how to find out must not itself be the command that
    /// fails.
    fn verb(&self) -> Option<farhelm_proto::AgentVerb> {
        match self {
            AgentCmd::Hosts { .. } => Some(farhelm_proto::AgentVerb::Hosts {}),
            AgentCmd::Sessions { .. } => Some(farhelm_proto::AgentVerb::Sessions {}),
            AgentCmd::Profiles { .. } => Some(farhelm_proto::AgentVerb::Profiles {}),
            AgentCmd::Rename {
                title,
                session,
                expected_title,
            } => Some(farhelm_proto::AgentVerb::Rename {
                session_id: Some(session.clone()),
                expected_title: Some(expected_title.clone()),
                title: title.clone(),
            }),
            AgentCmd::Stop { session } => Some(farhelm_proto::AgentVerb::Stop {
                session_id: Some(session.clone()),
            }),
            AgentCmd::Restart {
                session,
                mode,
                stop_if_running,
            } => Some(farhelm_proto::AgentVerb::Restart {
                session_id: Some(session.clone()),
                mode: (*mode).into(),
                stop_if_running: *stop_if_running,
            }),
            AgentCmd::Create {
                cwd,
                host,
                profile,
                profile_id,
                invocation,
                title,
                idempotency_key,
            } => Some(farhelm_proto::AgentVerb::Create {
                host: Some(host.clone()),
                cwd: cwd.clone(),
                // `--profile` on the command line, `profile_name` on the
                // wire: the flag is what a user types and the field says
                // what it IS: the helm resolves this human-facing selector
                // into the bundle sent to the target supervisor.
                profile_name: profile.clone(),
                profile_id: profile_id.clone(),
                invocation: invocation.clone(),
                title: title.clone(),
                intent_key: idempotency_key.clone(),
            }),
            AgentCmd::Clone {
                source_session,
                host,
                cwd,
                title,
                idempotency_key,
            } => Some(farhelm_proto::AgentVerb::Clone {
                source_session_id: Some(source_session.clone()),
                host: Some(host.clone()),
                cwd: cwd.clone(),
                title: title.clone(),
                intent_key: idempotency_key.clone(),
            }),
            AgentCmd::Instructions | AgentCmd::Help => None,
        }
    }
}

/// The agent CLI's spelling of the supervisor-owned restart modes.
///
/// This is deliberately a CLI enum rather than a second restart policy:
/// conversion preserves the protocol vocabulary, while clap gives the
/// human-facing `fallback-template` spelling and refuses invented modes
/// before any authenticated request is sent.
#[derive(Clone, Copy, ValueEnum)]
enum AgentRestartMode {
    Resume,
    Fresh,
    FallbackTemplate,
}

impl From<AgentRestartMode> for farhelm_proto::RestartMode {
    fn from(value: AgentRestartMode) -> Self {
        match value {
            AgentRestartMode::Resume => Self::Resume,
            AgentRestartMode::Fresh => Self::Fresh,
            AgentRestartMode::FallbackTemplate => Self::FallbackTemplate,
        }
    }
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
    /// View or change where fresh checkouts are created (the working-copy
    /// root) and what runs after cloning (an optional post-clone command),
    /// globally or per registered host. Edits stored configuration only:
    /// never contacts a host, creates a directory, or runs the hook.
    CheckoutConfig {
        #[command(subcommand)]
        command: CheckoutConfigCmd,
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
enum CheckoutConfigCmd {
    /// Print the stored values, the effective inheritance for the selected
    /// scope, and the current revision.
    Show {
        /// Registered host id to view that host's overrides; omit for the
        /// global settings.
        #[arg(long)]
        host: Option<i64>,
        /// State directory holding helm.db.
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// Set the working-copy root: an absolute path, `~`, or `~/...`,
    /// stored UNEXPANDED (only the target supervisor expands it).
    SetRoot {
        /// The root path. Hyphen-leading values (a leading-slash path is
        /// fine, but so is a command-like start) are taken verbatim.
        #[arg(allow_hyphen_values = true)]
        path: String,
        /// Registered host id to set that host's override; omit for the
        /// global setting.
        #[arg(long)]
        host: Option<i64>,
        /// State directory holding helm.db.
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// Remove the root setting: a host override goes back to inheriting
    /// the global value; the global setting is unset.
    ClearRoot {
        /// Registered host id to clear that host's override; omit for the
        /// global setting.
        #[arg(long)]
        host: Option<i64>,
        /// State directory holding helm.db.
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// Set the post-clone shell command. An EMPTY command on a host is an
    /// explicit disable that overrides a global hook.
    SetPostClone {
        /// The command. Allowed to start with `-` — it is shell text, not
        /// a flag.
        #[arg(allow_hyphen_values = true)]
        command: String,
        /// Registered host id to set that host's override; omit for the
        /// global setting.
        #[arg(long)]
        host: Option<i64>,
        /// State directory holding helm.db.
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// Remove the post-clone setting, with the same clear-to-inherit /
    /// global-unset split as clear-root.
    ClearPostClone {
        /// Registered host id to clear that host's override; omit for the
        /// global setting.
        #[arg(long)]
        host: Option<i64>,
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
        /// TEST ONLY: read this host's boot id from PATH instead of the
        /// kernel. Lets a test harness simulate a reboot against a real
        /// supervisor process by rewriting the file between restarts.
        ///
        /// Hidden because no production launch passes it: `farhelm helm
        /// setup`'s units, the desktop launcher, and a hand-started
        /// supervisor all read the real boot id, and a supervisor pointed at
        /// a file that never changes would never classify a genuine reboot.
        #[arg(long, hide = true, value_name = "PATH")]
        boot_id_file: Option<PathBuf>,
    },
}

/// The `--vendor` flag's spelling: exactly the wire enum's adapters,
/// so the CLI-to-wire mapping is total and cannot drift one variant at a
/// time. Some adapters are installed automatically, Grok is configured by
/// the user, and `internal goose-hook` supplies Goose internally; the flag
/// still mirrors the complete closed wire enum.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum HookVendor {
    Claude,
    Codex,
    Goose,
    Pi,
    Omp,
    Grok,
}

impl HookVendor {
    fn report_vendor(self) -> farhelm_proto::ReportVendor {
        match self {
            HookVendor::Claude => farhelm_proto::ReportVendor::Claude,
            HookVendor::Codex => farhelm_proto::ReportVendor::Codex,
            HookVendor::Goose => farhelm_proto::ReportVendor::Goose,
            HookVendor::Pi => farhelm_proto::ReportVendor::Pi,
            HookVendor::Omp => farhelm_proto::ReportVendor::Omp,
            HookVendor::Grok => farhelm_proto::ReportVendor::Grok,
        }
    }
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
    /// An agent hook adapter: read the vendor's JSON payload from stdin and
    /// report the conversation identity it names to the supervisor that
    /// launched this session.
    ///
    /// Farhelm injects `<farhelm_exe> internal hook` for vendors with a
    /// per-launch hook surface. Grok instead uses manually configured
    /// `SessionStart`, `UserPromptSubmit`, and `Stop` callbacks. Either way it
    /// runs below the agent, inside the user's terminal, with the session
    /// credential in its environment. Everything it needs to report arrives
    /// on stdin or in that environment. See `hook.rs` for the silence and
    /// budget contract this arm exists to honour.
    Hook {
        /// Print [`hook::POINTER_LINE`] on stdout after reporting, so the
        /// agent learns `farhelm agent instructions` exists.
        ///
        /// The one flag, and it exists because the decision is the
        /// SUPERVISOR's: `FARHELM_AGENT_INSTRUCTIONS` is read once when
        /// the supervisor starts, and a hook process launched by an
        /// already-running supervisor must obey the setting that
        /// supervisor started with rather than whatever the variable says
        /// by the time the agent gets around to firing a hook. Passing it
        /// on the injected command line is what pins it to the launch.
        #[arg(long)]
        announce: bool,
        /// Which vendor adapter this hook invocation is: the report
        /// envelope's discriminator, sourced from the installed entry
        /// point rather than inferred from the payload. Required, so an
        /// old hook command that predates the flag fails closed at CLI
        /// parse instead of reporting untagged. The Goose helper never
        /// takes this flag — `internal goose-hook` supplies its value
        /// internally, keeping the persisted declaration unchanged.
        #[arg(long, value_enum)]
        vendor: HookVendor,
    },
    /// The credential-free MCP reporter Goose retains in session metadata.
    GooseHook,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Uninstall(options) => uninstall::run(options),
        Cmd::Spawn {
            cwd,
            title,
            agent,
            profile_id,
            inherit_agent,
            parent,
            idempotency_key,
        } => {
            let child = runtime()?.block_on(spawn_session(
                &SessionEnv::from_env(),
                SpawnArgs {
                    cwd,
                    title,
                    agent,
                    profile_id,
                    inherit_agent,
                    parent,
                    idempotency_key,
                },
            ))?;
            println!("{child}");
            Ok(())
        }
        Cmd::Agent { command } => run_agent(command),
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
        Cmd::Helm {
            command: HelmCmd::CheckoutConfig { command },
        } => {
            init_tracing();
            let runtime = runtime()?;
            let output = runtime.block_on(run_checkout_config(command))?;
            // `print!`, not `println!`: the command layer already ends its
            // output with exactly one newline, and a show's exact stdout is
            // part of what the tests pin.
            print!("{output}");
            Ok(())
        }
        Cmd::Supervisor {
            command:
                SupervisorCmd::Run {
                    state_dir,
                    tmux,
                    exit_on_stdin_close,
                    boot_id_file,
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
            // The same rule again, one variable over: this is the ONLY
            // read of `FARHELM_AGENT_INSTRUCTIONS` in the codebase. It
            // decides whether the injected hook carries `--announce`, and
            // therefore whether an agent is ever told that `farhelm agent`
            // exists. Reading it once at startup is what makes a launch's
            // behaviour a property of the supervisor that made it rather
            // than of whatever the environment happened to say at the
            // moment a hook fired — which for Codex is the user's first
            // prompt, arbitrarily long after the launch.
            //
            // Unset is the default (`on`), silently. A non-UTF-8 value
            // gets a line for the same reason its neighbour above does:
            // nobody types one on purpose, `parse_agent_instructions`
            // cannot be shown it, and the fallback direction has to be the
            // default rather than a guess.
            let agent_instructions = match std::env::var("FARHELM_AGENT_INSTRUCTIONS") {
                Ok(value) => farhelm_supervisor::agent_kind::parse_agent_instructions(&value),
                Err(std::env::VarError::NotPresent) => {
                    farhelm_supervisor::agent_kind::AgentInstructions::default()
                }
                Err(std::env::VarError::NotUnicode(_)) => {
                    tracing::warn!(
                        "FARHELM_AGENT_INSTRUCTIONS is not valid UTF-8 and cannot be parsed; \
                         falling back to the default (on) rather than guessing what was meant"
                    );
                    farhelm_supervisor::agent_kind::AgentInstructions::default()
                }
            };
            let startup = farhelm_supervisor::service::SupervisorStartup {
                tmux_program: tmux,
                agent_hooks,
                agent_instructions,
                boot_id_file,
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
            InternalCmd::GooseHook => {
                std::panic::set_hook(Box::new(|_| {}));
                let enabled =
                    std::env::var_os(farhelm_supervisor::launch::GOOSE_REPORTER_ENABLED_ENV_VAR)
                        .as_deref()
                        == Some(std::ffi::OsStr::new("1"));
                if enabled {
                    // Same reading as the `Hook` arm below; see there.
                    let env = SessionEnv::from_env();
                    let hook_log = env.hook_log();
                    let credential = env.hook_credential();
                    if let Ok(goose_id) = std::env::var("AGENT_SESSION_ID") {
                        let payload = serde_json::to_vec(&serde_json::json!({
                            "session_id": goose_id,
                            "source": "goose"
                        }))?;
                        // The discriminator comes from the entry point
                        // itself — this helper IS the Goose adapter — so
                        // the persisted MCP declaration keeps invoking the
                        // same command with no new stored flag.
                        hook::run_with(
                            credential,
                            std::io::Cursor::new(payload),
                            std::time::Duration::from_secs(2),
                            hook_log,
                            farhelm_proto::ReportVendor::Goose,
                        );
                    }
                }
                let instructions =
                    (std::env::var_os(farhelm_supervisor::launch::GOOSE_INSTRUCTIONS_ENV_VAR)
                        .as_deref()
                        == Some(std::ffi::OsStr::new("1")))
                    .then_some(hook::POINTER_LINE);
                goose_hook::serve(
                    std::io::BufReader::new(std::io::stdin()),
                    std::io::stdout(),
                    instructions,
                );
                Ok(())
            }
            InternalCmd::Hook { announce, vendor } => {
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
                // Read through `SessionEnv`'s lenient hook readings rather
                // than its strict `dial`, which answers a different question
                // ("may `farhelm spawn` dial?") all-or-nothing. The log path
                // and the credential need different subsets of the values,
                // and a half-configured environment must still get its log
                // line; see `SessionEnv::hook_log`.
                let env = SessionEnv::from_env();
                let hook_log = env.hook_log();
                let credential = env.hook_credential();

                // Well under the timeout the injected hook config gives
                // the vendor, so the vendor never gets to time us out.
                const BUDGET: std::time::Duration = std::time::Duration::from_secs(2);
                hook::run_with(
                    credential,
                    std::io::stdin(),
                    BUDGET,
                    hook_log,
                    vendor.report_vendor(),
                );

                // AFTER the report, never before. The identity round trip
                // is the part the session's correctness depends on, and it
                // is the part with a deadline; the pointer is a nicety
                // that costs a small write to a pipe the vendor is
                // draining. Ordering it second means a stdout that
                // somehow will not take the line cannot delay the report.
                if announce {
                    hook::announce(&mut std::io::stdout());
                }

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
/// `FARHELM_TMUX` over `PATH`), the `FARHELM_AGENT_HOOKS` opt-out, and the
/// `FARHELM_AGENT_INSTRUCTIONS` switch — and is passed straight through.
/// Resolving any of them here instead would put the decision on both sides
/// of the tether branch, which is one place too many for values that must
/// be identical on every launch path.
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

/// Run one `farhelm agent` subcommand and print its answer.
///
/// `AgentCmd::verb` answers "what goes on the wire, if anything" —
/// `None` for `instructions`/`help`, which must work with no
/// supervisor, no credential, and no helm anywhere in sight (see its
/// own docs).
fn run_agent(command: AgentCmd) -> anyhow::Result<()> {
    let Some(verb) = command.verb() else {
        print!("{}", agent_instructions::text());
        return Ok(());
    };
    // The three listings hand off entirely to `print_agent_listing`,
    // which makes its own `agent_request` call and returns — there
    // is nothing left for this arm to do with their reply, unlike
    // the four lifecycle verbs below.
    if let AgentCmd::Hosts { json } | AgentCmd::Sessions { json } | AgentCmd::Profiles { json } =
        &command
    {
        return print_agent_listing(verb, *json);
    }
    // The lifecycle and creating verbs share one `agent_request`
    // round trip here, rather than each making its own the way the
    // listings do. The authenticated caller returned alongside the
    // reply is retained for attribution even though every
    // consequential target is now explicit.
    // A self restart tears down this process's own agent tree. The
    // session id is already an injected authentication input, so it
    // is available without a discovery round trip and lets the CLI
    // warn before it sends the destructive request. There is no
    // completion claim here: the request may cut this process off
    // before its acknowledgement returns.
    if let AgentCmd::Restart { session, .. } = &command {
        let asking = SessionEnv::from_env()
            .dial("farhelm agent restart")?
            .session_id;
        if session == &asking {
            eprintln!(
                "warning: restarting this session can interrupt the invoking CLI; \
                 acknowledgement can be lost, and resumed task continuation is not guaranteed"
            );
        }
    }
    let (_asking, reply) = runtime()?.block_on(agent_request(&SessionEnv::from_env(), verb))?;
    match command {
        // The four lifecycle verbs print one confirmation line
        // rather than a table — there is exactly one row to
        // report, and a script capturing stdout wants the plain
        // sentence SPEC.md's CLI contract promises, not a one-row
        // table with headers. A deliberate self-stop,
        // or self-restart can still terminate this CLI before it
        // prints, because the explicit target ID names the same
        // process tree carrying the credential.
        AgentCmd::Rename { .. } => {
            let AgentReply::Session { session } = reply else {
                // `agent_request` already checked the reply's tag
                // against `ReplyKind::of_verb(&Rename)` before
                // returning it, so this arm is unreachable in
                // practice; bailing rather than `unreachable!()`
                // keeps a defect here a clean error instead of a
                // panic, consistent with every other "the peer sent
                // something this decode did not expect" case above.
                anyhow::bail!("the helm answered rename with something other than a session");
            };
            println!(
                "renamed {} to {}",
                safe_cell(&session.id),
                quoted(&session.title)
            );
        }
        AgentCmd::Stop { session } => {
            // The reply carries no id (`AgentReply::Stopped` is
            // empty — see its own docs), so the confirmation's
            // target comes from the required `--session` selector.
            // Named for what it is used FOR — the printed
            // confirmation — rather than merely restating the flag.
            let target_for_reply = session;
            let AgentReply::Stopped {} = reply else {
                anyhow::bail!("the helm answered stop with something other than confirmation");
            };
            println!("stopped {}", safe_cell(&target_for_reply));
        }
        AgentCmd::Restart { .. } => {
            let AgentReply::Restarted { session } = reply else {
                anyhow::bail!(
                    "the helm answered restart with something other than a restarted session"
                );
            };
            println!("restarted {}", safe_cell(&session.id));
        }
        // The two creating verbs invert the stream convention the
        // three above follow, and deliberately: stdout carries the
        // new session's id and nothing else — `farhelm spawn`'s
        // contract, which an agent can capture and go on to use as
        // a `--session` target — while the sentence a human reads
        // goes to stderr beside them. A confirmation on stdout
        // would make the two verbs whose output is meant to be
        // captured as a single value the two it cannot be captured
        // from.
        AgentCmd::Create { .. } | AgentCmd::Clone { .. } => {
            let AgentReply::Created { session } = reply else {
                anyhow::bail!(
                    "the helm answered a creating verb with something other than a new \
                     session"
                );
            };
            println!("{}", session.id);
            // Every field is fleet-wide peer text — a title from
            // another host, a host name, a directory — so each one
            // goes through the same escaping the listings' table
            // cells get: `quoted` for the title, which is the one
            // field whose own quotes would otherwise close the
            // pair around it, and `safe_cell` for the rest.
            // `host_cell` because the helm may have no host name it
            // can vouch for even for a row it just created (see
            // `AgentSession::host`). Nothing here is the id printed
            // above: that line is the machine-readable one and is
            // left exactly as the helm sent it.
            //
            // Written through `write!` with its result DISCARDED
            // rather than through `eprintln!`, and that is the
            // whole point of the awkwardness: the macro panics on
            // an unwritable stderr, and the session has already
            // been created at this point with its id already on
            // stdout. Aborting here would turn a create that
            // succeeded into a command that failed, and a caller
            // that had already captured the id would be told to
            // retry a create it must not repeat. A confirmation
            // nobody can read is the acceptable loss; the id is
            // not.
            let _ = writeln!(
                std::io::stderr(),
                "created {} {} on {} in {}",
                safe_cell(&session.id),
                quoted(&session.title),
                safe_cell(&host_cell(&session)),
                safe_cell(&session.cwd)
            );
        }
        // `Hosts`/`Sessions` already returned above via
        // `print_agent_listing`, and `verb()` returns `None` for
        // `Instructions`/`Help` so the early return higher up
        // always fires first for those. This arm exists only to
        // keep the match exhaustive against a future `AgentCmd`
        // variant.
        AgentCmd::Hosts { .. }
        | AgentCmd::Sessions { .. }
        | AgentCmd::Profiles { .. }
        | AgentCmd::Instructions
        | AgentCmd::Help => {
            unreachable!("handled above before this match is reached")
        }
    }
    Ok(())
}

/// Print discovery as a human table or a versioned JSON envelope.
///
/// JSON retains exact caller identity and the reply's completeness fields
/// so agents can resolve explicit targets. Table truncation warnings go to
/// stderr to preserve the existing stdout table contract. A listing ends
/// after printing and never enters the lifecycle mutation path.
fn print_agent_listing(verb: farhelm_proto::AgentVerb, json: bool) -> anyhow::Result<()> {
    let (asking, reply) = runtime()?.block_on(agent_request(&SessionEnv::from_env(), verb))?;
    if json {
        let caller_host_id = match &reply {
            AgentReply::Hosts { caller_host_id, .. }
            | AgentReply::Sessions { caller_host_id, .. }
            | AgentReply::Profiles { caller_host_id, .. } => caller_host_id,
            _ => anyhow::bail!("only discovery replies can be printed as JSON"),
        };
        let envelope = serde_json::json!({
            "schema_version": 2,
            "caller": {
                "session_id": asking,
                "host_id": caller_host_id,
            },
            "reply": reply,
        });
        println!("{}", serde_json::to_string(&envelope)?);
        return Ok(());
    }
    print!("{}", render_agent_reply(&reply)?);
    // On stderr, so a script capturing stdout still gets nothing but the
    // table — the notice is about the ANSWER, not part of it.
    if let Some(notice) = truncation_notice(&reply) {
        eprintln!("{notice}");
    }
    Ok(())
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

/// Map the parsed `farhelm helm checkout-config` grammar onto the helm
/// crate's command layer. Purely mechanical: the grammar lives here so the
/// CLI surface evolves with the rest of `farhelm helm`, while the storage
/// and rendering rules stay behind the never-create, never-migrate open in
/// `farhelm_helm::checkout_config`.
async fn run_checkout_config(command: CheckoutConfigCmd) -> anyhow::Result<String> {
    use farhelm_helm::checkout_config::{CheckoutConfigAction, checkout_config_cli};
    let (host, state_dir, action) = match command {
        CheckoutConfigCmd::Show { host, state_dir } => {
            (host, state_dir, CheckoutConfigAction::Show)
        }
        CheckoutConfigCmd::SetRoot {
            path,
            host,
            state_dir,
        } => (host, state_dir, CheckoutConfigAction::SetRoot { path }),
        CheckoutConfigCmd::ClearRoot { host, state_dir } => {
            (host, state_dir, CheckoutConfigAction::ClearRoot)
        }
        CheckoutConfigCmd::SetPostClone {
            command,
            host,
            state_dir,
        } => (
            host,
            state_dir,
            CheckoutConfigAction::SetPostClone { command },
        ),
        CheckoutConfigCmd::ClearPostClone { host, state_dir } => {
            (host, state_dir, CheckoutConfigAction::ClearPostClone)
        }
    };
    checkout_config_cli(state_dir, host, action).await
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
    #[farhelm_testtrace::test]
    fn the_non_linux_setup_message_names_the_platform_and_the_alternative() {
        assert_eq!(
            NON_LINUX_SETUP_MESSAGE,
            "farhelm helm setup manages systemd user units and only runs on Linux; on macOS run \
             farhelm-desktop, which starts its own helm and supervisor"
        );
    }

    /// Both token verbs accept the state directory after the verb, matching
    /// the command shape the e2e harness and user-facing plan document.
    #[farhelm_testtrace::test]
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

    /// Every checkout-config verb accepts `--host` and `--state-dir`, and
    /// the two value-taking verbs take their value POSITIONALLY with
    /// hyphen-leading values intact — a post-clone command is shell text
    /// that may start with `-`, and the grammar must never mistake it for
    /// a flag.
    #[farhelm_testtrace::test]
    fn checkout_config_cli_parses_all_verbs_with_hyphen_leading_values() {
        let cli = Cli::try_parse_from([
            "farhelm",
            "helm",
            "checkout-config",
            "show",
            "--host",
            "7",
            "--state-dir",
            "/tmp/farhelm-test-state",
        ])
        .unwrap();
        let Cmd::Helm {
            command: HelmCmd::CheckoutConfig { command },
        } = cli.command
        else {
            panic!("expected the checkout-config subcommand");
        };
        assert!(matches!(
            command,
            CheckoutConfigCmd::Show { host: Some(7), .. }
        ));

        let cli = Cli::try_parse_from([
            "farhelm",
            "helm",
            "checkout-config",
            "set-root",
            "--",
            "-/leading-hyphen/path",
        ])
        .unwrap();
        let Cmd::Helm {
            command: HelmCmd::CheckoutConfig { command },
        } = cli.command
        else {
            panic!("expected the checkout-config subcommand");
        };
        let CheckoutConfigCmd::SetRoot { path, host, .. } = command else {
            panic!("expected set-root");
        };
        assert_eq!(path, "-/leading-hyphen/path");
        assert_eq!(host, None);

        let cli = Cli::try_parse_from([
            "farhelm",
            "helm",
            "checkout-config",
            "set-post-clone",
            "-rm -rf /tmp/x",
        ])
        .unwrap();
        let Cmd::Helm {
            command: HelmCmd::CheckoutConfig { command },
        } = cli.command
        else {
            panic!("expected the checkout-config subcommand");
        };
        let CheckoutConfigCmd::SetPostClone { command, .. } = command else {
            panic!("expected set-post-clone");
        };
        assert_eq!(command, "-rm -rf /tmp/x");
    }

    /// The CLI must never manufacture a target, a restart mode, or consent
    /// from the caller's discovery cache. This parser-level boundary catches
    /// the unsafe failure before any authenticated relay request exists.
    #[farhelm_testtrace::test]
    fn agent_restart_requires_its_explicit_target_and_mode() {
        assert!(Cli::try_parse_from(["farhelm", "agent", "restart", "--mode", "resume"]).is_err());
        assert!(Cli::try_parse_from(["farhelm", "agent", "restart", "--session", "s1"]).is_err());
    }
}
