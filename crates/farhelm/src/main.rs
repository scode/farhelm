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
//! (`rename`/`stop`/`archive`), or — for the two creating verbs
//! (`create`/`clone`) — the NEW SESSION'S ID and nothing else, with the
//! human-readable confirmation on stderr beside it. That last shape is
//! `spawn`'s contract deliberately: the created id is the one agent output
//! meant to be captured as a SINGLE VALUE, since it is what a caller goes
//! on to pass as `--session`. The listings are machine-readable too — a
//! fixture and a README example both parse the hosts table — but they are
//! parsed as a table, and a table that grew a column would still be one.
//! An id that grew a confirmation line beside it would not. They share the
//! injected-environment contract
//! (`spawn_environment`) and nothing else — a spawn is answered by the
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

use clap::{Parser, Subcommand};
use farhelm_proto::AgentReply;
use std::io::Write;
use std::path::PathBuf;

mod agent_instructions;
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
        /// Agent profile name; omitted reuses the asking session's stored
        /// agent bundle, including its profile snapshot when present.
        #[arg(long)]
        agent: Option<String>,
        /// Organizational parent id. Never defaults to this session.
        #[arg(long)]
        parent: Option<String>,
        /// Retry key, valid only for the child session's lifetime.
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    /// Ask the helm about the fleet, or act on it, from inside a Farhelm
    /// session — `hosts`/`sessions` are read-only questions,
    /// `rename`/`stop`/`archive` are fleet-wide lifecycle actions, and
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
/// Nine in total: the two read-only listings (`Hosts`, `Sessions`), the
/// three lifecycle verbs SPEC.md's "A session can also ASK" paragraph
/// promises (`Rename`, `Stop`, `Archive`), the two CREATING verbs
/// (`Create`, `Clone`), and the two that print the manual itself
/// (`Instructions`, `Help`) — see [`AgentCmd::verb`] for why the last two
/// are answered LOCALLY while the other seven are relayed to the helm.
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
    Hosts,
    /// List the sessions the helm knows, marking this one.
    Sessions,
    /// Rename a session — the asking one by default.
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
        /// Act on this session instead of the one asking — any session id
        /// the helm knows, on any host, not only ones this session could
        /// otherwise name.
        #[arg(long = "session")]
        session: Option<String>,
    },
    /// Stop a session's agent process tree — the asking one by default.
    Stop {
        /// Act on this session instead of the one asking — any session id
        /// the helm knows, on any host, not only ones this session could
        /// otherwise name.
        #[arg(long = "session")]
        session: Option<String>,
    },
    /// Archive a session — the asking one by default.
    Archive {
        /// Act on this session instead of the one asking — any session id
        /// the helm knows, on any host, not only ones this session could
        /// otherwise name.
        #[arg(long = "session")]
        session: Option<String>,
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
        host: Option<String>,
        /// Agent profile name, resolved in the helm-wide catalog.
        #[arg(
            long,
            value_name = "NAME",
            conflicts_with = "invocation",
            allow_hyphen_values = true
        )]
        profile: Option<String>,
        /// Command line to run instead of a profile.
        #[arg(long, value_name = "CMD", allow_hyphen_values = true)]
        invocation: Option<String>,
        /// Display title; omitted derives one from the directory.
        #[arg(long, value_name = "TITLE", allow_hyphen_values = true)]
        title: Option<String>,
        /// Retry key: the same key creates the session only once.
        #[arg(long, value_name = "KEY", allow_hyphen_values = true)]
        idempotency_key: Option<String>,
    },
    /// Copy this session onto any host; prints the new id.
    Clone {
        /// Host to create on, by the name `farhelm agent hosts` shows.
        #[arg(long, value_name = "NAME", allow_hyphen_values = true)]
        host: Option<String>,
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
            AgentCmd::Hosts => Some(farhelm_proto::AgentVerb::Hosts {}),
            AgentCmd::Sessions => Some(farhelm_proto::AgentVerb::Sessions {}),
            AgentCmd::Rename { title, session } => Some(farhelm_proto::AgentVerb::Rename {
                session_id: session.clone(),
                title: title.clone(),
            }),
            AgentCmd::Stop { session } => Some(farhelm_proto::AgentVerb::Stop {
                session_id: session.clone(),
            }),
            AgentCmd::Archive { session } => Some(farhelm_proto::AgentVerb::Archive {
                session_id: session.clone(),
            }),
            AgentCmd::Create {
                cwd,
                host,
                profile,
                invocation,
                title,
                idempotency_key,
            } => Some(farhelm_proto::AgentVerb::Create {
                host: host.clone(),
                cwd: cwd.clone(),
                // `--profile` on the command line, `profile_name` on the
                // wire: the flag is what a user types and the field says
                // what it IS: the helm resolves this human-facing selector
                // into the bundle sent to the target supervisor.
                profile_name: profile.clone(),
                invocation: invocation.clone(),
                title: title.clone(),
                intent_key: idempotency_key.clone(),
            }),
            AgentCmd::Clone {
                host,
                cwd,
                title,
                idempotency_key,
            } => Some(farhelm_proto::AgentVerb::Clone {
                host: host.clone(),
                cwd: cwd.clone(),
                title: title.clone(),
                intent_key: idempotency_key.clone(),
            }),
            AgentCmd::Instructions | AgentCmd::Help => None,
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
    /// terminal, with the session credential in its environment.
    /// Everything it needs to report arrives on stdin or in that
    /// environment. See `hook.rs` for the silence and budget contract this
    /// arm exists to honour.
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
    },
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
        /// The `agent-relay` script is the one that reads the injected tail
        /// rather than merely tolerating it — but it reads
        /// `std::env::args()` directly rather than this field, on purpose:
        /// what it is standing in for is a VENDOR parsing the argv the
        /// supervisor built, so it should see exactly that rather than a
        /// re-parse of it. What this field still does for it is the same
        /// thing it does for every other script — keep clap from rejecting
        /// the whole command line before `run` is ever reached.
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
        // `AgentCmd::verb` answers "what goes on the wire, if anything" —
        // `None` for `instructions`/`help`, which must work with no
        // supervisor, no credential, and no helm anywhere in sight (see its
        // own docs).
        Cmd::Agent { command } => {
            let Some(verb) = command.verb() else {
                print!("{}", agent_instructions::text());
                return Ok(());
            };
            // The two listings hand off entirely to `print_agent_listing`,
            // which makes its own `agent_request` call and returns — there
            // is nothing left for this arm to do with their reply, unlike
            // the three lifecycle verbs below.
            if matches!(command, AgentCmd::Hosts | AgentCmd::Sessions) {
                return print_agent_listing(verb);
            }
            // The lifecycle and creating verbs share one `agent_request`
            // round trip here, rather than each making its own the way the
            // listings do, because `Stop`'s reply carries no id — the
            // confirmation it prints needs the ASKING session, which only
            // this shared call reports back (see `agent_request`'s docs).
            let (asking, reply) = runtime()?.block_on(agent_request(verb))?;
            match command {
                // The three lifecycle verbs print one confirmation line
                // rather than a table — there is exactly one row to
                // report, and a script capturing stdout wants the plain
                // sentence SPEC.md's CLI contract promises, not a one-row
                // table with headers. That promise has one carve-out this
                // match cannot enforce: a bare `stop`/`archive` (no
                // `--session`) targets the ASKING session itself, and
                // stopping or archiving oneself ends the whole process
                // tree the sweep reaches by environment marker — this CLI
                // process included — which can SIGTERM it before the
                // `println!` below ever runs. See
                // `tests/e2e/agent_listing_real_stack.rs`'s lifecycle test
                // for where that race was confirmed and why it is routed
                // around there rather than fixed here.
                AgentCmd::Rename { .. } => {
                    let AgentReply::Session { session } = reply else {
                        // `agent_request` already checked the reply's tag
                        // against `ReplyKind::of_verb(&Rename)` before
                        // returning it, so this arm is unreachable in
                        // practice; bailing rather than `unreachable!()`
                        // keeps a defect here a clean error instead of a
                        // panic, consistent with every other "the peer sent
                        // something this decode did not expect" case above.
                        anyhow::bail!(
                            "the helm answered rename with something other than a session"
                        );
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
                    // target comes from whatever this process itself
                    // resolved: the `--session` the caller gave, or else
                    // the ASKING session `agent_request` reports back,
                    // exactly the substitution rule the helm applies on
                    // its own side. Named for what it is used FOR — the
                    // printed confirmation — rather than merely restating
                    // that it is the caller's `--session`.
                    let target_for_reply = session;
                    let AgentReply::Stopped {} = reply else {
                        anyhow::bail!(
                            "the helm answered stop with something other than confirmation"
                        );
                    };
                    let target = target_for_reply.unwrap_or(asking);
                    println!("stopped {}", safe_cell(&target));
                }
                AgentCmd::Archive { .. } => {
                    let AgentReply::Session { session } = reply else {
                        anyhow::bail!(
                            "the helm answered archive with something other than a session"
                        );
                    };
                    println!("archived {}", safe_cell(&session.id));
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
                AgentCmd::Hosts | AgentCmd::Sessions | AgentCmd::Instructions | AgentCmd::Help => {
                    unreachable!("handled above before this match is reached")
                }
            }
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
            InternalCmd::Hook { announce } => {
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
///
/// `command` is the user-facing name of the command being run — `farhelm
/// spawn` or `farhelm agent`. It is a parameter rather than a literal
/// because both commands share this validation, and an error that named the
/// wrong one sends a user to diagnose a feature they did not invoke.
fn spawn_environment(command: &str) -> anyhow::Result<(String, String, PathBuf)> {
    use anyhow::Context;

    let session_id = std::env::var_os(farhelm_supervisor::launch::SESSION_ID_ENV_VAR);
    let session_token = std::env::var_os(farhelm_supervisor::launch::SESSION_TOKEN_ENV_VAR);
    let supervisor_sock = std::env::var_os(farhelm_supervisor::launch::SUPERVISOR_SOCK_ENV_VAR);
    if session_id.is_some() && session_token.is_none() {
        anyhow::bail!(
            "this session predates spawn support (it carries no injected session credential) \
             and must be restarted before running {command}"
        );
    }
    let socket = supervisor_sock
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} is required; {command} will not guess which supervisor to dial",
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

    let (session_id, token, socket) = spawn_environment("farhelm spawn")?;
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
            profile_name: args.agent,
            title: args.title,
            cols: 80,
            rows: 24,
            intent_key: args.idempotency_key,
            agent_kind: None,
            resume_template: None,
            source_profile: None,
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

/// Ask `verb` (`Hosts` or `Sessions`) and print the answer exactly the way
/// both of `farhelm agent`'s read-only listings are printed: the table on
/// stdout, then a truncation warning on stderr if the fleet did not fit.
///
/// Separate from the lifecycle verbs' path because a listing is finished
/// the moment it is printed: the asking session's own id has no use here
/// (`_asking`), so this makes its own round trip and returns, while the
/// lifecycle verbs share one call in `main` because each needs that id or
/// the reply's own row.
fn print_agent_listing(verb: farhelm_proto::AgentVerb) -> anyhow::Result<()> {
    let (_asking, reply) = runtime()?.block_on(agent_request(verb))?;
    print!("{}", render_agent_reply(&reply)?);
    // On stderr, so a script capturing stdout still gets nothing but the
    // table — the notice is about the ANSWER, not part of it.
    if let Some(notice) = truncation_notice(&reply) {
        eprintln!("{notice}");
    }
    Ok(())
}

/// Ask the helm one question, or tell it to act, from inside a session, and
/// return the asking session's own id alongside the answer.
///
/// Mechanically `spawn_session`'s twin — same injected environment
/// (validated by the same [`spawn_environment`], so the two commands can
/// never disagree about what a Farhelm session guarantees), same
/// authenticated handshake, one request, one reply — and semantically its
/// opposite. A spawn is answered by the supervisor on the other end of the
/// socket. This is answered by the HELM, which is not on the other end of
/// anything this process can reach: the session's host has no route,
/// address, or credential back to the machine the user is sitting at, so
/// the supervisor forwards the question up the connection the helm itself
/// opened and relays the answer back. See `farhelm-supervisor`'s
/// `service::agent_relay`.
///
/// The asking session's id travels back out because it is the one thing a
/// caller cannot otherwise recover after this call: `Stop`'s reply
/// ([`farhelm_proto::AgentReply::Stopped`]) carries no fields at all, so a
/// confirmation naming WHICH session stopped — when the caller sent no
/// `--session` and meant "this one" — has nowhere else to read that id
/// from. `Rename` and `Archive` do not need it; their replies are the
/// updated row.
///
/// NO TIMEOUT here, deliberately. The supervisor bounds the upcall
/// (`AGENT_UPCALL_TIMEOUT`) and is the only party that can tell "no helm is
/// attached" from "a helm has it and is slow"; a deadline on this side
/// would collapse those into one unactionable failure and would fire first,
/// hiding the specific error the relay was about to send.
async fn agent_request(request: farhelm_proto::AgentVerb) -> anyhow::Result<(String, AgentReply)> {
    use anyhow::Context;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake_with_session_auth, parse_control};
    use farhelm_proto::{AgentOutcome, ControlMsg, SessionAuth};

    // Captured before the request goes out, because the reply's own tag is
    // the only thing that can be checked against it — see [`ReplyKind`].
    let expected = ReplyKind::of_verb(&request);
    // Also captured before `request` is moved into the frame below, for the
    // other question this function can no longer answer afterwards: whether
    // the thing that went out CHANGES something. See [`lost_reply`].
    let mutating = request.is_mutating();
    let (session_id, token, socket) = spawn_environment("farhelm agent")?;
    let stream = tokio::net::UnixStream::connect(&socket)
        .await
        .with_context(|| format!("connecting to supervisor socket {}", socket.display()))?;
    let (read, write) = tokio::io::split(stream);
    let mut reader = FrameReader::new(read);
    let mut writer = FrameWriter::new(write);
    // The hello this sends carries `role: "spawn"` — the one spelling
    // `handshake_with_session_auth` has, shared with the conversation hook,
    // which is not a spawn either. It is deliberately harmless: `role` is
    // diagnostic free text and never an authorization input (see
    // `ControlMsg::Hello::role`); presence of `auth` is what selects
    // restricted admission. Left as-is rather than widened here so all
    // three session-authenticated callers keep one handshake.
    handshake_with_session_auth(
        &mut reader,
        &mut writer,
        SessionAuth {
            session_id: session_id.clone(),
            token,
        },
    )
    .await
    .context("performing the authenticated supervisor handshake")?;

    const REQUEST_ID: u64 = 1;
    writer
        .write_control(&ControlMsg::AgentRequest {
            req_id: REQUEST_ID,
            session_id: session_id.clone(),
            request,
        })
        .await
        .context("sending the agent request")?;

    // Past the write, every way of not getting a reply is an ending
    // [`lost_reply`] has to classify: the request is already on the socket,
    // and the supervisor may have forwarded it before dying.
    let frame = match reader.read_frame().await {
        Ok(Some(frame)) => frame,
        Ok(None) => {
            return Err(lost_reply(
                "the supervisor closed before answering",
                mutating,
            ));
        }
        Err(error) => {
            return Err(lost_reply(
                &format!("reading the agent reply failed: {error}"),
                mutating,
            ));
        }
    };
    let reply = match parse_control(&frame) {
        Ok(reply) => reply,
        // A frame arrived and could not be read. That is not the same as no
        // frame arriving, but it licenses exactly the same conclusion: the
        // request went out and nothing came back that says what became of
        // it. Classified rather than returned as a decode error for the
        // reason [`lost_reply`] exists — `Internal`-flavoured prose about
        // JSON tells the reader nothing about whether their stop landed.
        Err(error) => {
            return Err(lost_reply(
                &format!("the agent reply could not be decoded: {error}"),
                mutating,
            ));
        }
    };
    match reply {
        ControlMsg::AgentResponse {
            req_id: REQUEST_ID,
            outcome,
        } => match outcome {
            AgentOutcome::Ok { reply } => {
                // The tag exists precisely so this can be checked. A
                // response is handed back by `req_id` alone across two
                // hops, so a peer that correlated a sessions listing with a
                // hosts request would otherwise have that listing printed
                // under `farhelm agent hosts` — authoritative-looking output
                // answering a question nobody asked.
                let got = ReplyKind::of(&reply);
                if got != expected {
                    // Verb- and shape-neutral wording. Two of the five
                    // requests this can report on are not questions and
                    // three of the four replies are not listings, so the
                    // old "answered the X question with a Y listing" was
                    // wrong for most of the pairs it could actually print
                    // — a rename answered with a stop confirmation being
                    // the plainest case.
                    //
                    // Routed through `lost_reply` rather than bailed
                    // outright: a peer that answered a `stop` with a
                    // session row is broken, but a broken peer is at least
                    // as likely to have stopped the session and then
                    // mis-answered as to have done nothing, and this
                    // process cannot tell those apart. The remedy belongs
                    // to every post-write ending of a mutation, not only
                    // the tidy ones.
                    return Err(lost_reply(
                        &format!(
                            "the helm answered with a {} where a {} was expected",
                            got.noun(),
                            expected.noun()
                        ),
                        mutating,
                    ));
                }
                Ok((session_id, reply))
            }
            // The TEXT is rendered verbatim whoever wrote it — the
            // supervisor's relay, or the helm's own listing — because
            // SPEC.md's actionable-error rule applies to both hops and
            // neither side's prose improves by being paraphrased here. The
            // BYTES are not, though: with the lifecycle verbs landed, this
            // is the first `AgentOutcome::Err` that can carry a TARGET
            // supervisor's own free-text refusal (a rejected rename title,
            // say) rather than only this build's own fixed sentences, and
            // `main`'s default `Result` printer puts an uncaught `bail!`
            // string on stderr with no escaping of its own — unlike every
            // successful confirmation, which already runs its dynamic
            // fields through `safe_cell`. `safe_error_message` gives this
            // path the same floor.
            AgentOutcome::Err { message, .. } => anyhow::bail!(safe_error_message(&message)),
        },
        // Still possible, and not a protocol violation: the supervisor
        // sends an uncorrelated `Error` when it refuses the CREDENTIAL,
        // before any request has been read (see
        // `io::handshake_with_session_auth`).
        //
        // Deliberately NOT routed through `lost_reply`, unlike every other
        // arm here: this is the peer's own definitive statement that the
        // request failed, in its own words, which is precisely the thing an
        // outcome-unknown ending exists for the absence of. Appending "the
        // outcome is unknown" to a refusal the supervisor authored would
        // manufacture doubt it did not express.
        ControlMsg::Error {
            req_id: 0 | REQUEST_ID,
            message,
            ..
        } => anyhow::bail!(message),
        // Everything else is a reply this process cannot correlate or
        // interpret — an `AgentResponse` carrying somebody else's `req_id`,
        // an unrelated control message, an `Error` for a request that was
        // never made. The request itself went out, so a mutation's ending
        // here is outcome-unknown for the same reason a decode failure's is.
        unexpected => Err(lost_reply(
            &format!("the supervisor sent an unexpected agent reply: {unexpected:?}"),
            mutating,
        )),
    }
}

/// The error for a reply that never usably arrived, in the two vocabularies
/// that situation has once the request itself is known to have gone out.
///
/// EVERY post-write ending but one comes through here, and the breadth is
/// the point rather than an accident of where the calls happen to sit: the
/// socket dying, a frame that will not decode, a response correlated with
/// somebody else's `req_id`, a control message that answers nothing, a
/// success reply of the wrong shape. They look nothing alike and they all
/// license exactly one conclusion — the request went out, and nothing came
/// back that says what became of it. The single exclusion is a
/// `ControlMsg::Error` the supervisor itself wrote, which IS a statement
/// about the outcome and must not have doubt appended to it.
///
/// The local socket dying takes every `ErrorKind` with it — there is no
/// `AgentOutcome` left to carry a classification — so this is the one place
/// the distinction can still be made, and the CLI is the one party that can
/// still make it: it knows the verb it sent and that its write completed.
/// The facts are identical either way (the request reached the supervisor's
/// socket; the supervisor may have forwarded it to a helm, which may have
/// applied it on another host; the answer was lost), and what differs is
/// what the reader should do next. A listing has nothing to double-apply
/// and gets the plain transport wording, which already reads as "ask
/// again". A MUTATION — a rename/stop/archive, or a create/clone that may
/// by now have a session running on some host — may ALREADY have taken
/// effect, so it gets
/// the same "look before you retry" remedy the relay's own delivered-but-
/// unanswered endings carry — one sentence across every hop, rather than a
/// helpful answer that stops at the process boundary.
fn lost_reply(cause: &str, mutating: bool) -> anyhow::Error {
    if !mutating {
        return anyhow::anyhow!("{cause}");
    }
    anyhow::anyhow!(
        "{cause}; the request had already been sent, so the outcome is unknown — {}",
        farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY
    )
}

/// Which reply shape a verb must be answered with.
///
/// A retained expectation, because the protocol's `reply` tag is only
/// useful to a client that remembers what it asked. The relay hands a
/// response back by `req_id` across two hops and nothing on either hop
/// re-checks the shape, so this is the only place a mismatch can be caught.
///
/// `Session` covers BOTH `Rename` and `Archive` — the wire's own choice
/// (`AgentReply::Session` is one shape for two verbs; see that variant's
/// docs) — so this check can confirm "the helm answered with a session row"
/// but not "with THIS verb's own effect"; a helm that renamed when asked to
/// archive would still pass it. That is an acceptable gap: it is a bug in
/// the helm's own dispatch, not a wire-shape mismatch, and nothing this
/// process can observe would tell the two apart without re-deriving policy
/// that belongs to the helm alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplyKind {
    Hosts,
    Sessions,
    Session,
    Stopped,
    Created,
}

impl ReplyKind {
    fn of_verb(verb: &farhelm_proto::AgentVerb) -> ReplyKind {
        match verb {
            farhelm_proto::AgentVerb::Hosts {} => ReplyKind::Hosts,
            farhelm_proto::AgentVerb::Sessions {} => ReplyKind::Sessions,
            farhelm_proto::AgentVerb::Rename { .. } | farhelm_proto::AgentVerb::Archive { .. } => {
                ReplyKind::Session
            }
            farhelm_proto::AgentVerb::Stop { .. } => ReplyKind::Stopped,
            // `Created`, not `Session`: the two payloads are identical and
            // the tag is the only thing separating "a row that did not
            // exist" from "the row you changed". Checking it here is what
            // stops this CLI printing an EXISTING session's id as though
            // it had just created one — a target an agent might then go on
            // to stop.
            farhelm_proto::AgentVerb::Create { .. } | farhelm_proto::AgentVerb::Clone { .. } => {
                ReplyKind::Created
            }
            // This is an internal supervisor-to-helm query, never a CLI
            // verb. Classifying it keeps a malformed peer reply recoverable
            // instead of letting a new wire variant abort this process.
            farhelm_proto::AgentVerb::ResolveProfile { .. } => ReplyKind::Created,
        }
    }

    fn of(reply: &AgentReply) -> ReplyKind {
        match reply {
            AgentReply::Hosts { .. } => ReplyKind::Hosts,
            AgentReply::Sessions { .. } => ReplyKind::Sessions,
            AgentReply::Session { .. } => ReplyKind::Session,
            AgentReply::Stopped {} => ReplyKind::Stopped,
            AgentReply::Created { .. } => ReplyKind::Created,
            AgentReply::ResolvedProfile { .. } => ReplyKind::Created,
        }
    }

    /// What this reply shape is CALLED in an error a user reads.
    ///
    /// A noun phrase for the reply itself, not for the verb that asked for
    /// it: the mismatch message names two of these and has no idea which
    /// one was the request, so anything verb-flavored reads backwards half
    /// the time.
    fn noun(self) -> &'static str {
        match self {
            ReplyKind::Hosts => "hosts listing",
            ReplyKind::Sessions => "sessions listing",
            ReplyKind::Session => "session row",
            ReplyKind::Stopped => "stop confirmation",
            // "created", not "new": the whole point of this noun is to
            // read differently from `Session`'s in a message that names
            // both, and a reader who sees "session row" against "created
            // session row" can tell which end of the mismatch is which.
            ReplyKind::Created => "created session row",
        }
    }
}

/// The one-line warning a cut listing owes its reader, or `None` for a
/// complete one.
///
/// Separate from the table because it does not belong on stdout: the table
/// is the machine-readable answer and this is a statement about that
/// answer's completeness. It exists at all because a truncated listing is
/// shaped exactly like a whole one, so without it "no such session" and
/// "past the cut" are the same output.
fn truncation_notice(reply: &AgentReply) -> Option<String> {
    match reply {
        AgentReply::Sessions {
            sessions,
            truncated: true,
        } => Some(format!(
            "warning: this is not the whole fleet; the listing was cut at {} sessions",
            sessions.len()
        )),
        _ => None,
    }
}

/// One listing as a plain aligned table on stdout.
///
/// A table rather than JSON because the consumer is a language model
/// reading its own shell output: columns survive being quoted into a
/// conversation, and an agent that wanted structure would be parsing prose
/// out of `message` on the failure path anyway. The `*` column is the one
/// piece of information that has no other spelling — which row is the
/// asking session, and which host it is on.
///
/// Only ever called with the reply to `Hosts` or `Sessions` — the three
/// lifecycle verbs print their own one-line confirmation instead (see
/// `main`'s `Rename`/`Stop`/`Archive` arms) and the two creating verbs
/// print an id on stdout with their confirmation on stderr — which is why
/// the lifecycle and `Created` tags are an ERROR here rather than tables of
/// their own.
///
/// A `Result` rather than a panic on those tags, deliberately. The
/// precondition is real and holds today, but it is a fact about this
/// program's own dispatch rather than anything the type system carries, and
/// a `unreachable!()` turns a future routing mistake into a CLI that
/// aborts. Every other "this is not the shape I expected" case in this file
/// — a reply whose tag disagrees with its question, a lifecycle verb
/// answered with the wrong variant — already ends in a `bail!`, and this
/// belongs in the same family.
///
/// Returns the text rather than printing it so the shape is testable
/// without a process.
fn render_agent_reply(reply: &AgentReply) -> anyhow::Result<String> {
    match reply {
        AgentReply::Hosts { hosts } => {
            let mut rows = vec![vec![
                String::new(),
                "NAME".to_string(),
                "KIND".to_string(),
                "STATE".to_string(),
            ]];
            rows.extend(hosts.iter().map(|host| {
                vec![
                    marker(host.current),
                    host.name.clone(),
                    host.kind.clone(),
                    host.state.clone(),
                ]
            }));
            // NAME (column 1) is printed WHOLE, exempt from the clamp every
            // other non-final column takes, because this column is not a
            // description of a host — it is the SELECTOR for one. `create`
            // and `clone` take `--host <NAME>` and match it exactly, so a
            // name the listing cut at 48 characters and marked with `…` is
            // a host an agent can see and can never target. The clamp's
            // amplification argument does not transfer here either: host
            // names are operator-registered ssh destinations, one per
            // machine in a fleet, not the per-session user text
            // `MAX_CELL_WIDTH` was written for. The sessions table below
            // keeps the clamp on every one of its columns for exactly that
            // reason.
            Ok(aligned(&rows, &[1]))
        }
        AgentReply::Sessions { sessions, .. } => {
            let mut rows = vec![vec![
                String::new(),
                "ID".to_string(),
                "HOST".to_string(),
                "TITLE".to_string(),
                "CWD".to_string(),
                "AGENT".to_string(),
                "STATUS".to_string(),
            ]];
            rows.extend(sessions.iter().map(|session| {
                vec![
                    marker(session.current),
                    session.id.clone(),
                    host_cell(session),
                    session.title.clone(),
                    session.cwd.clone(),
                    session.agent.clone(),
                    session_status_cell(session),
                ]
            }));
            Ok(aligned(&rows, &[]))
        }
        // Refused rather than rendered: a lifecycle or creating reply has
        // one row and no table to be, and printing an empty one would read
        // as an empty fleet. `main` never routes one here — see this
        // function's docs for why that is stated as an error and not
        // asserted.
        AgentReply::Session { .. }
        | AgentReply::Stopped {}
        | AgentReply::Created { .. }
        | AgentReply::ResolvedProfile { .. } => {
            anyhow::bail!("only hosts and sessions listings are rendered as a table")
        }
    }
}

/// The HOST cell: the name, or an explicit stand-in when the helm had none
/// to vouch for.
///
/// The absent case is unreachable from a LISTING today — `AgentSession::
/// host` is only ever `None` on a mutating verb's own reply, whose host is
/// pinned to the connection the mutation routed through and can stop being
/// current before the row is projected — and spelled out anyway rather than
/// defaulted to an empty cell. An empty cell in a column of host names
/// reads as a rendering bug; this reads as what it is, and pairs with the
/// `(stale)` the status cell puts on the same row.
///
/// Shared with the creating verbs' stderr confirmation, which is not a
/// table at all: "created … on (unknown)" is an odd sentence, and it is
/// still the honest one — the session exists and the helm cannot name the
/// machine it landed on, which a reader must not be told is a machine
/// called nothing.
fn host_cell(session: &farhelm_proto::AgentSession) -> String {
    session
        .host
        .clone()
        .unwrap_or_else(|| "(unknown)".to_string())
}

/// The STATUS cell: the status word, overridden for an archived session and
/// annotated for a stale one.
///
/// `archived` REPLACES the word rather than joining it, because a live
/// status is meaningless for an archived session: whatever the helm last
/// saw is history the user has already filed away, and printing `running`
/// beside an archive marker invites an agent to go and interact with it.
/// `stale` is additive instead — the word is still the last thing anyone
/// observed, and what the reader needs is to know it may be old.
fn session_status_cell(session: &farhelm_proto::AgentSession) -> String {
    if session.archived {
        return "archived".to_string();
    }
    if session.stale {
        return format!("{} (stale)", session.status);
    }
    session.status.clone()
}

/// The "this one is you" column: `*` for the asking session and its host,
/// empty otherwise.
fn marker(current: bool) -> String {
    if current { "*" } else { "" }.to_string()
}

/// The widest a non-final column is allowed to get.
///
/// This bound is what keeps output linear in the ROW COUNT rather than in
/// row count times the longest field. Alignment pads every cell in a column
/// to the widest one, and session fields are user text bounded only by the
/// supervisor's create-time cap (tens of kilobytes), so one pathological
/// title in a middle column would otherwise add that many spaces to every
/// other row — turning a valid, bounded reply into hundreds of megabytes
/// before a single byte is printed.
///
/// Forty-eight is chosen to fit the values that actually matter whole —
/// session ids, host names, ordinary titles, most working directories — on
/// a terminal that can still show the columns after it.
const MAX_CELL_WIDTH: usize = 48;

/// Pad every column to its widest cell, one row per line, with every cell
/// first made safe and bounded.
///
/// Three things happen here, and each is load-bearing:
///
/// **Escaping.** Every cell is arbitrary text from somewhere else on the
/// fleet — titles, working directories, host names, status words — printed
/// straight to a terminal. A newline in one cell forges a row; a tab shifts
/// the columns; an ESC introduces a control sequence that can repaint the
/// screen, hide what is above it, or reach terminal features that have
/// nothing to do with printing. So control characters become visible
/// escapes, and one cell can only ever produce one line.
///
/// **Clamping.** Non-final columns are cut to [`MAX_CELL_WIDTH`] with a
/// trailing `…`, for the amplification reason that constant documents. The
/// final column is neither padded nor clamped: nothing follows it, so it
/// costs its own length and no more. Columns named in `verbatim` are
/// exempt, which is a statement about what the column IS rather than a
/// formatting preference — see the hosts listing's own call site.
///
/// **Widths in `char`s, not bytes.** A title or directory is arbitrary user
/// text, and byte counting would misalign every row after the first
/// non-ASCII one. It remains an approximation — a wide CJK glyph occupies
/// two terminal cells and counts as one char here — and that is the right
/// approximation for this surface: the reader is a model parsing columns,
/// not a person eyeballing a grid, and the alternative is a Unicode
/// width-table dependency for a debug-shaped listing.
///
/// The last column is never padded, so no line carries trailing spaces.
fn aligned(rows: &[Vec<String>], verbatim: &[usize]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    // Sanitized ONCE, before anything measures: a width taken from the raw
    // text and then applied to the escaped text would misalign every row
    // that contained anything to escape.
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(column, cell)| {
                    let cell = safe_cell(cell);
                    if column + 1 == row.len() || verbatim.contains(&column) {
                        cell
                    } else {
                        clamp(cell)
                    }
                })
                .collect()
        })
        .collect();
    let widths: Vec<usize> = (0..columns)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut out = String::new();
    for row in &rows {
        let mut line = String::new();
        for (column, cell) in row.iter().enumerate() {
            if column + 1 == row.len() {
                line.push_str(cell);
            } else {
                line.push_str(cell);
                let pad = widths[column].saturating_sub(cell.chars().count());
                line.extend(std::iter::repeat_n(' ', pad + 1));
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// One field as a quoted, unambiguously delimited token.
///
/// For the rename confirmation, whose title is the one place this CLI puts
/// peer-supplied text inside literal quotes on stdout. [`safe_cell`] alone
/// was not enough there: it neutralizes control characters but leaves a `"`
/// as a `"`, so a title of `x" and then some` printed as
/// `renamed s1 to "x" and then some"` — three quotes, and anything reading
/// the line for a quoted field sees the title end where the attacker chose.
///
/// The two escapes happen BEFORE `safe_cell`, and the order is what makes
/// the result decodable. Escaping first means a literal backslash in the
/// title is already doubled by the time `safe_cell` writes its own
/// backslash escapes, so `\n` in the output is unambiguously a newline that
/// was escaped and never a title that literally contained a backslash and
/// an n.
fn quoted(field: &str) -> String {
    let mut escaped = String::with_capacity(field.len());
    for c in field.chars() {
        if c == '"' || c == '\\' {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    format!("\"{}\"", safe_cell(&escaped))
}

/// One cell as a single printable line.
///
/// Every control character is replaced by a visible escape rather than
/// dropped, so a cell that contained one still says so — a silently
/// stripped newline turns two forged rows into one plausible row, which is
/// worse than an ugly one. C0, DEL and C1 are all covered: C1 is the eight-
/// bit form of the same escape sequences ESC introduces, and a terminal in
/// a legacy encoding acts on it.
fn safe_cell(cell: &str) -> String {
    let mut out = String::with_capacity(cell.len());
    for ch in cell.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                // `is_control` is Unicode's Cc category: C0, DEL, and C1,
                // all of which fit in two hex digits.
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Cut `cell` to [`MAX_CELL_WIDTH`] characters, marking the cut with `…`.
///
/// The ellipsis replaces the last kept character rather than being appended
/// to it, so a clamped cell is exactly the width the bound names — a cell
/// that could exceed its own limit would defeat the point of having one.
fn clamp(cell: String) -> String {
    clamp_to(cell, MAX_CELL_WIDTH)
}

/// The longest an [`agent_request`] error message is ever printed at,
/// before this process's own `Result`-printing takes over.
///
/// Sized for a SENTENCE rather than a table cell, unlike [`MAX_CELL_WIDTH`]:
/// an error is prose meant to be read whole, not a column meant to stay
/// aligned with its neighbors, so it gets a far more generous cap of its
/// own rather than reusing one built for a different job.
const MAX_ERROR_MESSAGE_CHARS: usize = 4096;

/// A peer-supplied error message, made safe for the same reason
/// [`safe_cell`] exists: escaped so an embedded control character cannot
/// forge terminal output, and bounded so a pathologically large refusal
/// cannot flood the screen.
///
/// Exists because an `AgentOutcome::Err`'s `message` — unlike every
/// successful confirmation's fields, which already go through
/// [`safe_cell`] — used to reach `anyhow::bail!` (and from there, this
/// process's own unescaped `Result` printer) with neither protection. That
/// was a latent gap even for the helm's own fixed refusal strings, and the
/// lifecycle verbs made it a real one: a rename/stop/archive refusal can
/// now carry a TARGET supervisor's own free-text prose (a rejected title,
/// say), which this process never validated on the way out.
fn safe_error_message(message: &str) -> String {
    clamp_to(safe_cell(message), MAX_ERROR_MESSAGE_CHARS)
}

/// [`clamp`]'s general form, for a width other than [`MAX_CELL_WIDTH`].
///
/// `clamp` itself is left as the table-rendering path's own name for this
/// same operation at its own fixed width, rather than folded into this one
/// with an extra argument at every call site.
fn clamp_to(cell: String, width: usize) -> String {
    if cell.chars().count() <= width {
        return cell;
    }
    let mut out: String = cell.chars().take(width - 1).collect();
    out.push('…');
    out
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

    // ---------------------------------------------------------------
    // `farhelm agent`'s table. The process-level contract lives in
    // tests/agent_cli.rs; what follows is the rendering itself, where an
    // exact expected string is cheap and a spawned binary is not.
    // ---------------------------------------------------------------

    fn agent_session(id: &str, title: &str) -> farhelm_proto::AgentSession {
        farhelm_proto::AgentSession {
            id: id.to_string(),
            host: Some("h".to_string()),
            title: title.to_string(),
            cwd: "/w".to_string(),
            agent: "claude".to_string(),
            status: "running".to_string(),
            current: false,
            archived: false,
            stale: false,
        }
    }

    fn sessions(rows: Vec<farhelm_proto::AgentSession>) -> AgentReply {
        AgentReply::Sessions {
            sessions: rows,
            truncated: false,
        }
    }

    /// Spec: column widths are counted in characters, so a multibyte but
    /// single-width character does not shift the columns after it.
    ///
    /// The formatter deliberately uses `chars().count()` instead of
    /// `len()`, and nothing else notices the difference: every other table
    /// fixture in this repo is ASCII, where the two agree exactly. A
    /// regression to byte length would misalign every row containing an
    /// accented name or title while all existing tests stayed green.
    ///
    /// Wide CJK glyphs are deliberately kept out of this case. They are a
    /// different problem with a different right answer (display width, not
    /// character count), and folding them in here would turn one regression
    /// test into an argument about which approximation is being pinned.
    #[test]
    fn column_widths_count_characters_not_bytes() {
        // "café" is 4 characters and 5 bytes; "tea" is 3 of each. Under
        // byte counting the first row's TITLE column would be padded one
        // column too wide and CWD would not line up.
        let rendered = render_agent_reply(&sessions(vec![
            agent_session("s1", "café"),
            agent_session("s2", "tea"),
        ]))
        .expect("a sessions listing renders as a table");
        assert_eq!(
            rendered,
            [
                " ID HOST TITLE CWD AGENT  STATUS",
                " s1 h    café  /w  claude running",
                " s2 h    tea   /w  claude running",
                "",
            ]
            .join("\n")
        );
    }

    /// Spec: control characters in a cell become visible escapes, so no
    /// value from the fleet can forge a row or drive the terminal.
    ///
    /// Every dynamic cell here is text from somewhere else on the fleet,
    /// printed straight to a terminal a person and a model are both reading.
    /// A newline forges a row that looks exactly like a real one; an ESC
    /// opens a control sequence that can repaint the screen, hide the lines
    /// above it, or reach terminal features that have nothing to do with
    /// printing. Escaping rather than stripping is deliberate: a silently
    /// removed newline turns two forged rows into one plausible row.
    #[test]
    fn control_characters_in_a_cell_are_escaped_into_one_visible_line() {
        let rendered = render_agent_reply(&sessions(vec![agent_session(
            "s1",
            "real\n  s2 forged\ttab\x1b[31m",
        )]))
        .expect("a sessions listing renders as a table");
        assert_eq!(
            rendered.lines().count(),
            2,
            "a cell must never produce a second row: {rendered:?}"
        );
        assert!(rendered.contains("real\\n"), "{rendered:?}");
        assert!(rendered.contains("forged\\ttab"), "{rendered:?}");
        assert!(rendered.contains("\\x1b[31m"), "{rendered:?}");
        assert!(
            !rendered.contains('\x1b'),
            "no raw ESC may reach the terminal: {rendered:?}"
        );
    }

    /// Spec: a non-final column is cut to [`MAX_CELL_WIDTH`] with a `…`,
    /// while the final column is left whole.
    ///
    /// This is a resource bound, not a cosmetic one. Alignment pads every
    /// cell of a column to the widest one in it, and session titles, paths
    /// and invocations are user text bounded only by the supervisor's
    /// create-time cap — so one long value in a middle column multiplies by
    /// the row count into an output far larger than the reply that produced
    /// it. The final column is exempt because nothing is padded to it.
    #[test]
    fn non_final_columns_are_clamped_and_the_last_is_not() {
        let long = "x".repeat(MAX_CELL_WIDTH * 3);
        let mut row = agent_session("s1", &long);
        row.status = long.clone();
        let rendered = render_agent_reply(&sessions(vec![row]))
            .expect("a sessions listing renders as a table");
        let title = rendered
            .lines()
            .nth(1)
            .expect("a data row")
            .split_whitespace()
            .nth(2)
            .expect("the TITLE cell");
        assert_eq!(title.chars().count(), MAX_CELL_WIDTH);
        assert!(title.ends_with('…'), "the cut must be marked: {title}");
        assert!(
            rendered.contains(&long),
            "the final column carries its value whole"
        );
    }

    /// Spec: STATUS says `archived` for an archived session and appends
    /// `(stale)` to a cached one.
    ///
    /// Both facts are ones the status word alone cannot carry. A cached
    /// `running` from a host that went offline overnight is byte-identical
    /// to one observed a second ago, and SPEC.md requires such rows to be
    /// clearly marked. An archived session's live status is worse than
    /// uninformative: it is history the user filed away, so showing
    /// `running` there invites an agent to go and interact with it.
    #[test]
    fn the_status_column_reports_archive_and_staleness() {
        let mut archived = agent_session("s1", "t");
        archived.archived = true;
        let mut stale = agent_session("s2", "t");
        stale.stale = true;
        let rendered = render_agent_reply(&sessions(vec![archived, stale]))
            .expect("a sessions listing renders as a table");
        assert_eq!(
            rendered,
            [
                " ID HOST TITLE CWD AGENT  STATUS",
                " s1 h    t     /w  claude archived",
                " s2 h    t     /w  claude running (stale)",
                "",
            ]
            .join("\n")
        );
    }

    /// Spec: a truncated listing produces a warning naming the cut; a
    /// complete one produces none.
    ///
    /// The notice is the only thing distinguishing a partial fleet from a
    /// whole one — the table itself is shaped identically either way, so
    /// without it "no such session" and "past the cut" are the same answer.
    #[test]
    fn a_truncated_listing_warns_and_a_complete_one_does_not() {
        assert!(truncation_notice(&sessions(vec![agent_session("s1", "t")])).is_none());
        let notice = truncation_notice(&AgentReply::Sessions {
            sessions: vec![agent_session("s1", "t")],
            truncated: true,
        })
        .expect("a truncated listing must say so");
        assert!(notice.contains("not the whole fleet"), "{notice}");
        assert!(notice.contains('1'), "the notice names the count: {notice}");
    }
}
