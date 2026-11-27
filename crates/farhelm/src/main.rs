//! The `farhelm` multi-call binary.
//!
//! One artifact carries every role — the helm's stored configuration,
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
//! spawn creates on the host it runs on, while `agent create` can name any
//! host in the fleet. Both are relayed to the helm that owns this host,
//! which asks the user first. The first is the scripting primitive, the
//! second is the fleet-aware one.
//!
//! `farhelm agent instructions` is the exception on both counts, and
//! deliberately so: its stdout is prose for a language model rather than a
//! table, and it needs no session at all. It is the command an agent runs
//! first, on the strength of one line the identity hook printed at it, so
//! it must work in a session whose relay is broken — see
//! [`agent_instructions`].

use agent_client::{SessionEnv, SpawnArgs, SpawnLaunch, agent_request, spawn_session};
use anyhow::Context as _;
use clap::{Parser, Subcommand, ValueEnum};
use farhelm_proto::AgentReply;
use render::{host_cell, quoted, render_agent_reply, safe_cell, truncation_notice};
use std::io::Write;
use std::path::{Path, PathBuf};

mod agent_client;
mod agent_instructions;
mod goose_hook;
mod hook;
mod render;
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
    /// Create a session on the host that runs this one.
    ///
    /// The child runs this session's own launch (`--inherit-agent`), or the
    /// launch its launch flags and templates describe. Either way the request
    /// goes to the helm that owns this host, which asks the user first.
    // `.multiple(true)`: the group is "at least one of these", and any number
    // of launch flags may come together; `--inherit-agent`'s own conflicts
    // keep it apart from them. `--conversation` alone describes no launch, so
    // it is not a member.
    #[command(
        after_help = "Describe the child's launch with --inherit-agent, --template NAME, \
        --agent TYPE, or --command CMD with --yolo or --no-yolo; the other launch flags edit one of \
        those."
    )]
    #[command(group(
        clap::ArgGroup::new("spawn_launch")
            .required(true)
            .multiple(true)
            .arg("inherit_agent")
            .args(LAUNCH_FLAG_IDS)
    ))]
    Spawn {
        /// Child working directory. Relative paths resolve against this
        /// process's real current directory before crossing the wire;
        /// `~` and `~/path` are forwarded as written and expand on the
        /// supervisor (`~user` forms are refused there). Required with
        /// `--inherit-agent`; otherwise a template may set the folder.
        #[arg(long, value_name = "DIR")]
        cwd: Option<PathBuf>,
        /// Optional display title; omitted derives from the directory.
        #[arg(long)]
        title: Option<String>,
        /// Start the child on an existing conversation the agent can reopen.
        #[arg(long, value_name = "ID")]
        conversation: Option<String>,
        /// Removed with agent profiles; its parser refuses every value, so
        /// a caller following older instructions gets a refusal naming the
        /// replacement rather than clap's "unexpected argument".
        #[arg(
            long,
            hide = true,
            num_args = 0..=1,
            default_missing_value = "",
            value_parser = refuse_profile_id,
        )]
        profile_id: Option<String>,
        /// Run this session's own stored launch in the child. Exclusive
        /// with every launch flag and --conversation.
        #[arg(long, conflicts_with_all = LAUNCH_FLAG_IDS, conflicts_with = "conversation")]
        inherit_agent: bool,
        #[command(flatten)]
        launch: LaunchFlags,
        /// Organizational parent id. Never defaults to this session.
        #[arg(long)]
        parent: Option<String>,
        /// Retry key, valid only for the child session's lifetime.
        #[arg(long, value_name = "KEY")]
        idempotency_key: Option<String>,
        /// Removed: an agent cannot override the YOLO confirmation (SPEC.md,
        /// Agent-spawned sessions). Its parser refuses every use, under both
        /// of its old names, so a caller following older instructions gets
        /// the reason rather than clap's "unexpected argument".
        #[arg(
            long = "confirm-yolo",
            alias = "allow-yolo-on-sensitive-host",
            hide = true,
            num_args = 0..=1,
            default_missing_value = "",
            value_parser = refuse_confirm_yolo,
        )]
        confirm_yolo: Option<String>,
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
    /// Read the custom-agent contract or report a custom agent's conversation.
    CustomAgent {
        #[command(subcommand)]
        command: CustomAgentCmd,
    },
    /// Edit the helm's stored configuration. The helm itself runs inside the
    /// desktop app; nothing here starts one.
    Helm {
        #[command(subcommand)]
        command: HelmCmd,
    },
    /// Run the per-host supervisor.
    Supervisor {
        #[command(subcommand)]
        command: SupervisorCmd,
    },
    /// Print the license notices for third-party code and dependencies in farhelm.
    // Ported and derived code carries its upstream notice with every copy, and this is how the binary carries it: the
    // texts are embedded at build time and printed verbatim; nothing is read from disk. A plain comment, not a doc
    // comment, so it stays out of `farhelm licenses --help`.
    Licenses,
    /// Internal commands: machinery, not user surface. Hidden from help;
    /// scripts and other farhelm processes are the only callers.
    #[command(hide = true)]
    Internal {
        #[command(subcommand)]
        command: InternalCmd,
    },
}

/// The public entry points for user-authored integrations.
#[derive(Subcommand)]
enum CustomAgentCmd {
    /// Print the custom-agent contract as Markdown. Works outside a Farhelm session.
    Spec,
    /// Record a conversation ID from the current custom agent session.
    ReportConversation {
        #[arg(long, required = true)]
        id: Option<String>,
    },
}

/// The verbs `farhelm agent` currently carries.
///
/// `Instructions` and `Help` render locally; the other verbs are relayed
/// to the helm that owns this host. See [`AgentCmd::verb`] for that boundary.
///
/// The creating verbs print DIFFERENTLY from every other verb here, and
/// the difference is a contract rather than a style: their stdout is the
/// new session's id and nothing else, exactly as `farhelm spawn`'s is, so
/// a caller can capture it. The human-readable confirmation goes to
/// stderr. That is the opposite of the lifecycle verbs, whose confirmation
/// IS their stdout — those act on a session the caller already named, so
/// there is no new identifier to hand back.
///
/// Free-text options and positionals here (session ids, titles, host names,
/// directories, template names, idempotency keys, shell text) take a value
/// starting with `-` only when it is joined with `=` (`--title=-draft`) or,
/// for a positional, placed after `--`. A host name, a directory or a title
/// may legally begin with `-`, because something DOWNSTREAM decides the
/// legality of these strings (the helm's registry, the target supervisor's
/// filesystem, the relay's byte caps), and both spellings still carry such a
/// value there unchanged. What clap no longer does is take a separate
/// hyphen-leading word as a value: with that allowance a forgotten value
/// silently swallowed the next flag (`host add dev --alias --tailscale`
/// asked for a host aliased `--tailscale`), and only some of those reached a
/// refusal at the far end. Without it the omission is a usage error here.
///
/// The exception is `--prompt` (here and on [`LaunchFlags`]): a message is
/// prose that often starts with a Markdown bullet or `---`, and clap's
/// refusal of one is a cryptic "unexpected argument '- '", so it still takes
/// a separate hyphen-leading value. Its cost is the old one: `--prompt` with
/// its text forgotten takes the next flag as the message.
///
/// On a verb with a positional, clap's refusal of an option's hyphen-leading
/// value adds the tip "use '-- -x'", which is the wrong fix for an option:
/// after `--` everything is positional. Those verbs say the right spelling in
/// their help ([`HYPHEN_VALUE_HELP`]).
///
/// Each variant's `///` doc comment is AGENT-FACING PROSE, not only help
/// text: [`agent_instructions`] walks this enum's clap definition and
/// prints each verb's `about` as its one-line meaning. Write them as
/// sentences that stand alone, because that is where they are read.
#[derive(Subcommand)]
enum AgentCmd {
    /// Open a web link in the user's browser through Farhelm, subject to the host's link permissions.
    OpenUrl {
        /// Absolute http or https URL. Use -- before a URL beginning with a hyphen.
        url: String,
    },
    /// Tell the user this session is blocked and cannot finish what they asked: Farhelm lists the session as waiting for them in the app and, if they turned blocked alerts on, sends them an ntfy alert.
    Notify {
        /// One short sentence: what is blocked and why, for example "deploy: the staging credentials are missing".
        reason: String,
    },
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
    /// List built-in and registered custom agents that can be used with --agent.
    Agents {
        /// Print the versioned machine-readable discovery envelope.
        #[arg(long)]
        json: bool,
    },
    /// List the launch templates the helm holds and the fields each sets.
    Templates {
        /// Print the versioned machine-readable discovery envelope.
        #[arg(long)]
        json: bool,
    },
    /// Removed with agent profiles. Kept, hidden, only so an agent still
    /// following older instructions gets a refusal saying so rather than
    /// clap's "unrecognized subcommand"; [`run_agent`] refuses it before
    /// anything is dialed. `--json` is accepted for the same reason.
    #[command(hide = true)]
    Profiles {
        #[arg(long, hide = true)]
        json: bool,
    },
    /// Rename an explicitly named session if its title is unchanged.
    #[command(after_help = HYPHEN_VALUE_HELP)]
    Rename {
        /// The new title, forwarded to the helm verbatim.
        ///
        /// TWO refusals stand between it and the target, both enforced by
        /// the supervisor rather than here: SPEC.md's control-character
        /// rule, and the 64 KiB field cap (`CREATE_FIELD_CAP`) the relay's
        /// own `validate_agent_verb` applies at the first hop. Anything
        /// else — leading hyphens, punctuation, the empty string — is a
        /// legal title. A title starting with `-` goes after `--`
        /// (`rename --session ID --expected-title OLD -- -draft`), since a
        /// separate hyphen-leading word is read as a flag.
        title: String,
        /// Exact session id from `farhelm agent sessions`.
        #[arg(long = "session")]
        session: String,
        /// Exact title observed in the discovery result, including empty.
        #[arg(long = "expected-title")]
        expected_title: String,
    },
    /// Stop a named session's agent and close its terminal tabs.
    Stop {
        /// Exact session id from `farhelm agent sessions`.
        #[arg(long = "session")]
        session: String,
    },
    // Hidden while the helm refuses agent prompts (`AGENT_PROMPTS_ENABLED` in
    // the helm's `agent_requests`; TODO.md, "Re-assess letting agents prompt
    // running sessions"). Hiding keeps it out of `--help` and out of the
    // generated agent instructions, so agents are never taught a verb that
    // always fails; valid requests still parse, so a caller who already
    // knows it gets the helm's refusal. Unhide it with the
    // re-enable.
    /// Send a prompt to another session's running Claude or Codex agent.
    #[command(hide = true)]
    #[command(group(clap::ArgGroup::new("prompt_source").required(true).args(["prompt", "prompt_file"])))]
    Prompt {
        /// Exact target id from `farhelm agent sessions`; never this session.
        #[arg(long = "session")]
        session: String,
        /// Message to submit, queued after the current turn if the agent is busy.
        /// Must contain non-whitespace text. Newlines and tabs are allowed;
        /// other control characters are refused.
        #[arg(long, value_name = "TEXT", allow_hyphen_values = true, value_parser = parse_agent_prompt)]
        prompt: Option<String>,
        /// Read the message from a UTF-8 file, or - for stdin (64 KiB cap).
        /// Cannot be combined with --prompt; takes contents, not a target-host path.
        // Read after parsing, into `prompt`, by [`resolve_prompt_files`].
        #[arg(long, value_name = "PATH")]
        prompt_file: Option<PathBuf>,
    },
    /// Delete an explicitly named session and its stored state; never this session.
    Delete {
        /// Exact session id from `farhelm agent sessions`.
        #[arg(long = "session")]
        session: String,
        /// Permit stopping the target's agent and terminal tabs before deletion.
        #[arg(long)]
        stop_if_running: bool,
    },
    /// Restart an explicitly named session, resuming its own conversation.
    Restart {
        /// Exact session id from `farhelm agent sessions`.
        #[arg(long = "session")]
        session: String,
        /// Removed: every restart resumes the session's own conversation.
        /// Kept, hidden, only so an agent still following older
        /// instructions gets a refusal saying so rather than clap's generic
        /// "unexpected argument" (SPEC.md, Agent-spawned sessions). Its
        /// parser refuses every value, so a parsed command never carries
        /// one.
        #[arg(
            long,
            hide = true,
            num_args = 0..=1,
            default_missing_value = "",
            value_parser = refuse_restart_mode,
        )]
        mode: Option<String>,
        /// Permit stopping the target's agent if it is working when the
        /// owning supervisor handles this request; without it, a working
        /// agent is refused, while an idle, waiting, or unknown-status one
        /// is stopped and restarted anyway.
        #[arg(long)]
        stop_if_running: bool,
    },
    /// Create a session on any host; prints its id.
    Create {
        /// Working directory for the new session, on the TARGET host.
        ///
        /// Required for an ordinary create unless a template or --gh sets the
        /// destination. A recipe launch defaults to the new environment's
        /// home. For a recipe launch, --cwd accepts only the literal '~'.
        /// Use --gh to clone a repository there.
        ///
        /// A plain `String`, not a `PathBuf`: this path is interpreted on
        /// whichever host the session lands on, so resolving it against
        /// THIS process's current directory — which is what `farhelm
        /// spawn` correctly does for its own same-host create — would
        /// invent a path that means nothing over there.
        #[arg(long, value_name = "DIR")]
        cwd: Option<String>,
        /// Create a fresh managed checkout of this GitHub repository on the target host.
        #[arg(long, alias = "github", value_name = "REPO", conflicts_with = "cwd", value_parser = parse_checkout_repo)]
        gh: Option<String>,
        /// Start on an existing conversation the selected agent can reopen.
        #[arg(long, value_name = "ID", conflicts_with_all = ["gh", "recipe"])]
        conversation: Option<String>,
        /// Host to create on, by the name `farhelm agent hosts` shows.
        /// Required for a fleet host unless a template names it. Omit it
        /// for a recipe launch, including through a template. An agent in
        /// a sandbox or container can omit it to create there.
        #[arg(long, value_name = "NAME")]
        host: Option<String>,
        /// Create the session in a new sandbox or container built from this
        /// recipe, behind a card that always asks; a Tensorlake sandbox unless
        /// --container-host is given. Folder is the new home (--gh clones there).
        #[arg(
            long,
            value_name = "NAME",
            conflicts_with_all = ["host", "idempotency_key"]
        )]
        recipe: Option<String>,
        /// With --recipe: create a container on this host, by the name
        /// `farhelm agent hosts` shows, instead of a Tensorlake sandbox.
        #[arg(
            long,
            value_name = "NAME",
            requires = "recipe",
            conflicts_with = "host"
        )]
        container_host: Option<String>,
        /// Removed with agent profiles. Kept, hidden, only so an agent still
        /// following older instructions gets a refusal naming what to do
        /// instead; its parser refuses every value.
        #[arg(
            long,
            hide = true,
            num_args = 0..=1,
            default_missing_value = "",
            value_parser = refuse_create_profile,
        )]
        profile: Option<String>,
        /// Removed with agent profiles; refused the way `--profile` is.
        #[arg(
            long,
            hide = true,
            num_args = 0..=1,
            default_missing_value = "",
            value_parser = refuse_profile_id,
        )]
        profile_id: Option<String>,
        /// Replaced by `--command` with launch kinds. Kept, hidden, only so
        /// an agent still following older instructions gets a refusal
        /// naming the replacement; its parser refuses every value.
        #[arg(
            long,
            hide = true,
            num_args = 0..=1,
            default_missing_value = "",
            value_parser = refuse_invocation,
        )]
        invocation: Option<String>,
        #[command(flatten)]
        launch: LaunchFlags,
        /// Display title; omitted derives one from the directory.
        #[arg(long, value_name = "TITLE")]
        title: Option<String>,
        /// Retry key for creating the session only once. Fleet creates need
        /// an explicit `--host`. Recipe launches do not accept retry keys.
        #[arg(long, value_name = "KEY")]
        idempotency_key: Option<String>,
        /// Removed: an agent cannot override the YOLO confirmation (SPEC.md,
        /// Agent-spawned sessions). Its parser refuses every use, under both
        /// of its old names, so a caller following older instructions gets
        /// the reason rather than clap's "unexpected argument".
        #[arg(
            long = "confirm-yolo",
            alias = "allow-yolo-on-sensitive-host",
            hide = true,
            num_args = 0..=1,
            default_missing_value = "",
            value_parser = refuse_confirm_yolo,
        )]
        confirm_yolo: Option<String>,
    },
    /// Copy an explicitly named session onto any host; prints the new id.
    Clone {
        /// Exact source id from `farhelm agent sessions`.
        #[arg(long = "source-session")]
        source_session: String,
        /// Host to create on, by the name `farhelm agent hosts` shows.
        /// Omit it for a sandbox or container session: its copy is a new
        /// sandbox or container from the same recipe, behind a card that
        /// always asks.
        #[arg(long, value_name = "NAME")]
        host: Option<String>,
        /// Working directory; omitted copies this session's.
        #[arg(long, value_name = "DIR")]
        cwd: Option<String>,
        /// Display title; omitted copies this session's.
        #[arg(long, value_name = "TITLE")]
        title: Option<String>,
        /// Retry key: the same key creates the session only once.
        #[arg(long, value_name = "KEY")]
        idempotency_key: Option<String>,
        /// Removed: an agent cannot override the YOLO confirmation (SPEC.md,
        /// Agent-spawned sessions). Its parser refuses every use, under both
        /// of its old names, so a caller following older instructions gets
        /// the reason rather than clap's "unexpected argument".
        #[arg(
            long = "confirm-yolo",
            alias = "allow-yolo-on-sensitive-host",
            hide = true,
            num_args = 0..=1,
            default_missing_value = "",
            value_parser = refuse_confirm_yolo,
        )]
        confirm_yolo: Option<String>,
    },
    /// Write launch templates; each waits for the user's approval.
    #[command(subcommand)]
    Template(TemplateCmd),
    /// Add, change or remove hosts; each always waits for the user's explicit Allow.
    #[command(subcommand)]
    Host(HostCmd),
    /// Print how to use these verbs, for an agent that was told to.
    Instructions,
    /// The same as instructions; both spellings print it.
    Help,
}

/// The help line that gives the right spelling of a hyphen-leading value on
/// the verbs that also take a positional, where clap's own refusal suggests
/// `--` for an option's value too (see [`AgentCmd`]).
const HYPHEN_VALUE_HELP: &str = "A value starting with - is joined to its option with =, as in \
     --option=-value. A positional one goes last, after --.";

/// `farhelm agent template`'s verbs: an agent's template writes, each shown to
/// the user on an approval card with the whole resulting template, command
/// text included (SPEC.md, Agent-spawned sessions).
///
/// The fields are `farhelm agent create`'s flags, minus `--template`: a
/// template built from other templates would copy their withheld command text
/// into one the agent wrote, so the CLI refuses it rather than resolving it.
#[derive(Subcommand)]
enum TemplateCmd {
    /// Create a launch template; refused if the name is taken.
    #[command(after_help = HYPHEN_VALUE_HELP)]
    Create {
        /// The new template's name.
        name: String,
        #[command(flatten)]
        fields: TemplateFieldFlags,
        /// Mark this template for the ⚡ quick-launch shortcut.
        #[arg(long)]
        quick_launch: bool,
        /// Give this template a physical number key, 1–9 or 0.
        #[arg(long, value_name = "DIGIT", value_parser = TEMPLATE_KEYS)]
        key: Option<String>,
    },
    /// Set the given fields on an existing template; others stay as they are.
    #[command(after_help = HYPHEN_VALUE_HELP)]
    Edit {
        /// The template to change.
        name: String,
        #[command(flatten)]
        fields: TemplateFieldFlags,
        /// Move the ⚡ quick-launch mark to this template.
        #[arg(long, conflicts_with = "no_quick_launch")]
        quick_launch: bool,
        /// Remove the ⚡ mark from this template.
        #[arg(long, conflicts_with = "quick_launch")]
        no_quick_launch: bool,
        /// Give this template a physical number key, 1–9 or 0.
        #[arg(long, value_name = "DIGIT", value_parser = TEMPLATE_KEYS, conflicts_with = "no_key")]
        key: Option<String>,
        /// Remove this template's current number key.
        #[arg(long, conflicts_with = "key")]
        no_key: bool,
    },
    /// Delete a launch template.
    Delete {
        /// The template to delete.
        name: String,
    },
}

/// `farhelm agent host`'s verbs: an agent's changes to the hosts themselves,
/// each shown to the user on a card that only the user's own Allow approves,
/// whatever the host's "run farhelm commands from this host without asking"
/// setting says (SPEC.md, Agent-spawned sessions).
///
/// `<HOST>` is the display name `farhelm agent hosts` shows, resolved by the
/// helm exactly as `farhelm agent create --host` is: exact, and refused when
/// ambiguous. An edit reaches only the alias, icon and color; nothing here
/// can change a host's destination or its permissions.
#[derive(Subcommand)]
enum HostCmd {
    /// Ask to add a host. Accepted means approved, not installed: adding may
    /// fail, need setup, or be cancelled. Check `farhelm agent hosts` and its
    /// state before relying on the host.
    #[command(after_help = HYPHEN_VALUE_HELP)]
    Add {
        /// SSH destination the helm can reach.
        #[arg(value_name = "DEST")]
        destination: String,
        /// Optional name shown instead of the destination.
        #[arg(long, value_name = "NAME")]
        alias: Option<String>,
        /// Ask for the dialog's Tailscale host option; DEST must then be a node
        /// name, DNS name or IP address, optionally login@.
        #[arg(long)]
        tailscale: bool,
    },
    /// Change a host's alias, icon or color; the card shows each old and new value.
    #[command(after_help = HYPHEN_VALUE_HELP)]
    Edit {
        /// Host to change, by the name `farhelm agent hosts` shows.
        #[arg(value_name = "HOST")]
        host: String,
        /// New alias, which becomes the host's name in listings and --host.
        #[arg(long, value_name = "NAME", conflicts_with = "no_alias")]
        alias: Option<String>,
        /// Remove the alias, so the host is named by its destination again.
        #[arg(long, conflicts_with = "alias")]
        no_alias: bool,
        /// New icon, one of the words the host settings dialog offers.
        #[arg(long, value_name = "ICON", value_parser = host_icon_parser())]
        icon: Option<farhelm_proto::host_appearance::HostIcon>,
        /// New color, one of the words the host settings dialog offers.
        #[arg(long, value_name = "COLOR", value_parser = host_color_parser())]
        color: Option<farhelm_proto::host_appearance::HostColor>,
    },
    /// Remove a host as the Farhelm UI does; never this machine or this session's host.
    Remove {
        /// Host to remove, by the name `farhelm agent hosts` shows.
        #[arg(value_name = "HOST")]
        host: String,
    },
}

/// Parse `--icon` against the host icon vocabulary, offered to clap as its
/// possible values so `--help` and a refusal both list every word, generated
/// from the vocabulary itself so neither can drift from it.
fn host_icon_parser()
-> impl clap::builder::TypedValueParser<Value = farhelm_proto::host_appearance::HostIcon> {
    use clap::builder::TypedValueParser as _;
    use farhelm_proto::host_appearance::HostIcon;
    clap::builder::PossibleValuesParser::new(HostIcon::ALL.map(HostIcon::as_str)).map(|word| {
        word.parse::<HostIcon>()
            .expect("clap admitted only the vocabulary's words")
    })
}

/// Parse `--color` against the host color vocabulary; see [`host_icon_parser`].
fn host_color_parser()
-> impl clap::builder::TypedValueParser<Value = farhelm_proto::host_appearance::HostColor> {
    use clap::builder::TypedValueParser as _;
    use farhelm_proto::host_appearance::HostColor;
    clap::builder::PossibleValuesParser::new(HostColor::ALL.map(HostColor::as_str)).map(|word| {
        word.parse::<HostColor>()
            .expect("clap admitted only the vocabulary's words")
    })
}

/// The fields a template write sets: a create's folder, title and host, and
/// its launch edit flags. A create's `--template` is not among them; a
/// hidden field of the same name refuses it ([`TemplateCmd`]).
#[derive(clap::Args)]
struct TemplateFieldFlags {
    /// Folder sessions launched from the template start in, on their host.
    #[arg(long, value_name = "DIR")]
    cwd: Option<String>,
    /// Create a fresh managed checkout of this GitHub repository when applying the template.
    #[arg(long, alias = "github", value_name = "REPO", conflicts_with = "cwd", value_parser = parse_checkout_repo)]
    gh: Option<String>,
    /// Session name sessions launched from the template get.
    #[arg(long, value_name = "TITLE")]
    title: Option<String>,
    /// Reusable base name or exact fixed name; an edit without this keeps its kind.
    #[arg(long, value_name = "base|fixed", value_parser = template_title_kind_parser())]
    title_kind: Option<farhelm_proto::launcher::TemplateNameKind>,
    /// Host the template launches on, by the name `farhelm agent hosts` shows.
    #[arg(long, value_name = "NAME")]
    host: Option<String>,
    #[command(flatten)]
    edits: LaunchEditFlags,
    /// Refused (see [`TemplateCmd`]). Parsed, hidden, only so a caller who
    /// carries `farhelm agent create`'s `--template` over gets the reason
    /// rather than clap's "unexpected argument"; its parser refuses every
    /// value, so a parsed command never carries one.
    #[arg(
        long = "template",
        hide = true,
        num_args = 0..=1,
        default_missing_value = "",
        value_parser = refuse_template_on_template_write,
    )]
    template: Option<String>,
}

/// The refusal for `--template` on `farhelm agent template create` and
/// `edit`, whatever value it was given; see [`TemplateCmd`].
fn refuse_template_on_template_write(_value: &str) -> Result<String, String> {
    Err(
        "a template write takes no --template: give the fields themselves, so the template \
         holds only what you wrote"
            .to_string(),
    )
}

impl TemplateFieldFlags {
    /// The fields as the helm's template verbs carry them. Shape validation
    /// belongs after the helm's merge: a kind-only edit can use the stored
    /// session name.
    fn fields(&self) -> farhelm_proto::launcher::TemplateFields {
        let mut fields = self.edits.fields(self.cwd.clone(), self.title.clone());
        fields.name_kind = self.title_kind;
        if let Some(repo) = &self.gh {
            fields.destination = Some(farhelm_proto::launcher::TemplateDestination::Github(
                repo.clone(),
            ));
        }
        fields
    }
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
            AgentCmd::Agents { .. } => Some(farhelm_proto::AgentVerb::Agents {}),
            AgentCmd::Templates { .. } => Some(farhelm_proto::AgentVerb::Templates {}),
            AgentCmd::OpenUrl { url } => {
                Some(farhelm_proto::AgentVerb::OpenUrl { url: url.clone() })
            }
            AgentCmd::Notify { reason } => Some(farhelm_proto::AgentVerb::Notify {
                reason: reason.clone(),
            }),
            // Refused in `run_agent` before this is asked; answering
            // `None` here would print the instructions instead.
            AgentCmd::Profiles { .. } => unreachable!("run_agent refuses agent profiles first"),
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
            AgentCmd::Prompt {
                session,
                prompt,
                prompt_file,
            } => {
                debug_assert!(
                    prompt_file.is_none(),
                    "resolve_prompt_files reads --prompt-file before the verb is built"
                );
                Some(farhelm_proto::AgentVerb::Prompt {
                    session_id: Some(session.clone()),
                    prompt: prompt
                        .clone()
                        .expect("clap requires one prompt source, and a file is read into prompt"),
                })
            }
            AgentCmd::Delete {
                session,
                stop_if_running,
            } => Some(farhelm_proto::AgentVerb::Delete {
                session_id: Some(session.clone()),
                stop_if_running: *stop_if_running,
            }),
            AgentCmd::Restart {
                session,
                stop_if_running,
                ..
            } => Some(farhelm_proto::AgentVerb::Restart {
                session_id: Some(session.clone()),
                stop_if_running: *stop_if_running,
            }),
            AgentCmd::Create {
                cwd,
                gh,
                host,
                recipe,
                container_host,
                launch,
                title,
                conversation,
                idempotency_key,
                ..
            } => {
                let edits = {
                    let mut edits = launch.edits.fields(cwd.clone(), title.clone());
                    if let Some(repo) = gh {
                        edits.destination = Some(
                            farhelm_proto::launcher::TemplateDestination::Github(repo.clone()),
                        );
                    }
                    edits
                };
                // A recipe is its own verb, so a helm that predates it
                // refuses the request instead of creating an ordinary session.
                Some(match recipe {
                    Some(recipe) => farhelm_proto::AgentVerb::CreateEnvironment {
                        recipe: recipe.clone(),
                        container_host: container_host.clone(),
                        templates: launch.templates.clone(),
                        edits,
                    },
                    None => farhelm_proto::AgentVerb::Create {
                        host: host.clone(),
                        templates: launch.templates.clone(),
                        edits,
                        conversation: conversation.clone(),
                        intent_key: idempotency_key.clone(),
                        confirm_yolo: false,
                        spawn: None,
                    },
                })
            }
            AgentCmd::Clone {
                source_session,
                host,
                cwd,
                title,
                idempotency_key,
                ..
            } => Some(farhelm_proto::AgentVerb::Clone {
                source_session_id: Some(source_session.clone()),
                host: host.clone(),
                cwd: cwd.clone(),
                title: title.clone(),
                intent_key: idempotency_key.clone(),
                confirm_yolo: false,
            }),
            // Built and printed by `run_template_write`, whose verbs carry
            // a quick-launch mark and number key besides the fields.
            AgentCmd::Template(_) => None,
            // Built and printed by `run_host_change`, which `run_agent`
            // hands it to before asking this.
            AgentCmd::Host(_) => None,
            AgentCmd::Instructions | AgentCmd::Help => None,
        }
    }
}

/// The number keys a template write may give a template: the ten physical
/// digits the launcher listens for, in keyboard order. Offered to clap as its
/// possible values, so a refusal lists them.
const TEMPLATE_KEYS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];

/// Parse `--title-kind` against its two words, offered to clap as its
/// possible values as [`host_icon_parser`] does. Name policy belongs to
/// template writes only; parsing to the shared enum here keeps omission
/// distinct from explicitly selecting Base on an edit. A local parser rather
/// than a `clap::ValueEnum` on the enum, which would make `farhelm-proto`
/// depend on clap.
fn template_title_kind_parser()
-> impl clap::builder::TypedValueParser<Value = farhelm_proto::launcher::TemplateNameKind> {
    use clap::builder::TypedValueParser as _;
    use farhelm_proto::launcher::TemplateNameKind;
    clap::builder::PossibleValuesParser::new(["base", "fixed"]).map(|word| match word.as_str() {
        "base" => TemplateNameKind::Base,
        "fixed" => TemplateNameKind::Fixed,
        _ => unreachable!("clap admitted only the two words"),
    })
}

/// Validate repository spellings with the launcher's parser while retaining
/// the original request text for keyed retry comparison.
fn parse_checkout_repo(value: &str) -> Result<String, String> {
    farhelm_proto::parse_github_repo(value)
        .map(|_| value.to_string())
        .map_err(|error| error.to_string())
}

/// The refusal for the removed `farhelm agent restart --mode`, whatever
/// value it was given.
///
/// Earlier releases required `--mode resume|fresh|fallback-template`, and
/// agents in sessions started then may still type it. Restart now has one
/// kind, so the message names what replaced the flag (nothing: drop it)
/// instead of letting clap call it an unknown argument.
fn refuse_restart_mode(_value: &str) -> Result<String, String> {
    Err(
        "farhelm agent restart no longer takes --mode: every restart resumes the \
         session's own conversation, so drop --mode. A session whose sessions --json \
         restart_offer is not resume cannot be restarted"
            .to_string(),
    )
}

/// The refusal for `--confirm-yolo` (and its older name
/// `--allow-yolo-on-sensitive-host`) on `farhelm spawn`, `farhelm agent
/// create` and `farhelm agent clone`.
///
/// The flag let an agent assert the user's YOLO approval itself. The host's
/// YOLO setting decides what an agent may launch (SPEC.md, Agent-spawned
/// sessions). The helm ignores the field even if a modified CLI sends it.
fn refuse_confirm_yolo(_value: &str) -> Result<String, String> {
    Err(
        "--confirm-yolo is no longer accepted: an agent cannot confirm a YOLO launch for the \
         user. On a host that asks before YOLO launches, an agent may not start a YOLO or \
         command launch at all. The user can \
         allow that by turning on \"Start YOLO sessions without asking\" in the host's \
         settings"
            .to_string(),
    )
}

/// The refusal for `--profile-id` on `farhelm spawn` and `farhelm agent
/// create`, whatever value it was given.
///
/// Earlier releases resolved the value as an agent profile. Profiles were
/// removed outright (SPEC.md, the launch-kinds upgrade paragraph), so the
/// message says what replaced them rather than letting clap call the flag
/// unknown.
fn refuse_profile_id(_value: &str) -> Result<String, String> {
    Err(
        "--profile-id is no longer accepted: agent profiles were removed. Name the launch \
         with --agent <TYPE> (and --model, --effort, --permissions), with \
         --command <CMD> and --yolo or --no-yolo, or with --template <NAME>; farhelm \
         agent templates lists the templates"
            .to_string(),
    )
}

/// The refusal for `farhelm agent create --profile`, whatever value it was
/// given; see [`refuse_profile_id`].
fn refuse_create_profile(_value: &str) -> Result<String, String> {
    Err(
        "farhelm agent create no longer takes --profile: agent profiles were removed. Name \
         the launch with --agent <TYPE>, with --command <CMD> and --yolo or --no-yolo, or \
         with --template <NAME>; farhelm agent templates lists the templates"
            .to_string(),
    )
}

/// The refusal for `farhelm agent create --invocation`, whatever value it
/// was given: launch kinds replaced it with `--command`, which also needs
/// the YOLO assertion the old flag never asked for.
fn refuse_invocation(_value: &str) -> Result<String, String> {
    Err(
        "--invocation was replaced by --command: pass the command line with --command <CMD> \
         and say whether it bypasses safety checks with --yolo or --no-yolo"
            .to_string(),
    )
}

/// `--agent` can name a built-in or the stable ID of a helm registration. The CLI has no
/// catalog connection while parsing flags, so the helm checks that ID before use.
#[derive(Debug, Clone)]
struct AgentTypeChoice {
    harness: farhelm_proto::LaunchHarness,
    custom_agent: Option<String>,
}

/// Preserve built-in wire spellings and aliases before interpreting the token as a custom catalog ID.
fn parse_agent_type(value: &str) -> Result<AgentTypeChoice, String> {
    if let Ok(harness) = serde_json::from_value(serde_json::Value::String(value.to_owned())) {
        return Ok(AgentTypeChoice {
            harness,
            custom_agent: None,
        });
    }
    if farhelm_proto::custom_agents::validate_id(value).is_ok() {
        return Ok(AgentTypeChoice {
            harness: farhelm_proto::LaunchHarness::Custom,
            custom_agent: Some(value.to_owned()),
        });
    }
    // A display name typed in place of an ID (`--agent "Acme Code"`) lands here before any helm contact, so the
    // refusal names the listing that shows registered IDs. The custom catalog marker is not a type to pass.
    Err({
        let types: Vec<String> = farhelm_proto::LaunchHarness::BUILT_INS
            .iter()
            .filter_map(|harness| match serde_json::to_value(harness) {
                Ok(serde_json::Value::String(name)) => Some(name),
                _ => None,
            })
            .collect();
        format!(
            "{value:?} is not an agent type or custom agent ID; built-in types are {}; \
             run farhelm agent agents to list registered custom agent IDs",
            types.join(", ")
        )
    })
}

/// The refusal for the removed `farhelm agent profiles` listing.
const AGENT_PROFILES_REMOVED: &str = "farhelm agent profiles was removed with agent profiles; \
     launch templates replaced them, and farhelm agent templates lists them";

/// A launch choice a flag can also reset: `default` puts the choice back to
/// the agent type's own default, undoing a template that set it (SPEC.md:
/// the flags "act as further edits" after the templates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Choice<T>(Option<T>);

/// Keep one shell-style command line as one clap value. A bare `Vec<String>`
/// is a repeated flag to clap, even when its parser returns a word list.
#[derive(Debug, Clone)]
struct WrapperPrefix(Vec<String>);

/// Parse the wrapper once at the CLI boundary so invalid quoting is a clap
/// refusal, before any launch request can reach the helm.
fn parse_wrapper_prefix(value: &str) -> Result<WrapperPrefix, String> {
    farhelm_proto::split_wrapper_prefix(value).map(WrapperPrefix)
}

/// Bound effort text at the CLI. The helm validates its spelling once it knows the agent and model,
/// which can come from a template and determine the choices a caller can retry with.
fn parse_effort(value: &str) -> Result<Choice<farhelm_proto::LaunchEffortValue>, String> {
    if value == "default" {
        return Ok(Choice(None));
    }
    if value.is_empty() || value.len() > 256 {
        return Err("effort must be 1 to 256 bytes".into());
    }
    if value
        .chars()
        .any(farhelm_proto::text::is_presentation_unsafe)
    {
        return Err("effort cannot contain control or invisible formatting characters".into());
    }
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
        .map(|effort| Choice(Some(effort)))
        .map_err(|error| error.to_string())
}

/// Parse explicit permissions without advertising the older `default` alias.
///
/// The alias still means request omission, so existing callers keep the
/// harness's admission default (guarded where offered, YOLO for Goose,
/// Pi and OpenCode).
fn parse_permissions(value: &str) -> Result<Choice<farhelm_proto::LaunchPermission>, String> {
    if value == "custom" {
        return Err(
            "custom permission flags are available through launch templates, not --permissions"
                .into(),
        );
    }
    if value == "default" {
        return Ok(Choice(None));
    }
    serde_json::from_value(serde_json::Value::String(value.to_string()))
        .map(|choice| Choice(Some(choice)))
        .map_err(|_| {
            format!(
                "{value:?} is not a permission; the choices are {}",
                farhelm_proto::LaunchPermission::ALL
                    .iter()
                    .filter(|permission| **permission != farhelm_proto::LaunchPermission::Custom)
                    .map(|permission| permission.wire_word())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

/// The ids of every [`LaunchFlags`] argument. `spawn --inherit-agent`
/// conflicts with each, and `spawn` needs one of them or `--inherit-agent`:
/// a spawn either reuses its own launch or describes one.
const LAUNCH_FLAG_IDS: [&str; 13] = [
    "templates",
    "prompt",
    "prompt_file",
    "agent",
    "model",
    "effort",
    "permissions",
    "wrapper",
    "command",
    "yolo",
    "no_yolo",
    "resume_command",
    "no_resume_command",
];

/// The launch flags of `farhelm agent create` and `farhelm spawn` (SPEC.md,
/// Agent-spawned sessions): the templates to apply and the edits after them.
///
/// The flags travel as launcher edits, not as a launch: the helm applies
/// the `--template`s first, in order, then the [`LaunchEditFlags`] as further
/// edits, and validates the result exactly like a GUI launch
/// (`farhelm-helm`'s `agent_launch`). Nothing is checked against a launch
/// here, because a template may supply what a flag leaves out.
#[derive(clap::Args, Debug, Clone, Default)]
struct LaunchFlags {
    /// Apply a launch template by its exact name; repeat to apply several,
    /// in order. `farhelm agent templates` lists them.
    #[arg(long = "template", value_name = "NAME")]
    templates: Vec<String>,
    #[command(flatten)]
    edits: LaunchEditFlags,
}

/// The launcher's fields as flags: what a launch's edits set, shared by
/// [`LaunchFlags`] and a template write ([`TemplateFieldFlags`]), which takes
/// everything a create does but `--template`.
///
/// Each flag may appear once; clap refuses a repeat, and `--yolo` with
/// `--no-yolo`, rather than resolving them by order.
#[derive(clap::Args, Debug, Clone, Default)]
struct LaunchEditFlags {
    /// First message, sent once when the new session starts. Newlines and
    /// tabs are allowed; other control characters are refused.
    #[arg(long, value_name = "TEXT", allow_hyphen_values = true, value_parser = parse_initial_prompt)]
    prompt: Option<String>,
    /// Read the first message from a UTF-8 file, or - for stdin (64 KiB
    /// launch budget). Cannot be combined with --prompt.
    // A path, not its contents: the file is read by [`resolve_prompt_files`]
    // once the whole command line has parsed, which moves the text into
    // `prompt` and leaves this `None`. Read by the parser instead, a `-`
    // would wait for stdin before a later flag's usage error could show.
    #[arg(long, value_name = "PATH", conflicts_with = "prompt")]
    prompt_file: Option<PathBuf>,
    /// Built-in agent type, such as claude or codex, or a registered custom agent ID.
    /// With --command, declares its agent type. The command needs {farhelm_args}.
    #[arg(long, value_name = "TYPE", value_parser = parse_agent_type)]
    agent: Option<AgentTypeChoice>,
    /// Model for an agent launch, as the agent type names it; `default`
    /// leaves it to the agent type. A custom agent's value starting with -
    /// must be joined with =, as in --model=-fast.
    // Like every free-text option here but `--prompt` ([`AgentCmd`]),
    // `--model` and `--effort` do not take a separate value starting with
    // `-`. They were the first to lose that allowance, because for them a
    // forgotten value was worst: nothing downstream refused the swallowed
    // flag, and `--effort --yolo` launched a custom agent with `--yolo` as its
    // effort. Without it clap reports the missing value, and the `=` form
    // still carries a custom agent's hyphen-leading identifier.
    #[arg(long, value_name = "ID")]
    model: Option<String>,
    /// Reasoning effort for an agent launch, or `default`. A custom agent's
    /// value starting with - must be joined with =, as in --effort=-fast.
    #[arg(long, value_name = "LEVEL", value_parser = parse_effort)]
    effort: Option<Choice<farhelm_proto::LaunchEffortValue>>,
    /// Permission mode for an agent launch (guarded, yolo, approve,
    /// smart_approve, chat), where supported. `default` remains an alias for
    /// omission; agents that offer guarded default to it, while Goose, Pi
    /// and OpenCode default to YOLO.
    #[arg(long, value_name = "MODE", value_parser = parse_permissions)]
    permissions: Option<Choice<farhelm_proto::LaunchPermission>>,
    /// Bash text placed before Farhelm's composed agent command.
    /// Quotes and escapes keep their shell meaning; no Farhelm placeholders.
    #[arg(long, value_name = "COMMAND", value_parser = parse_wrapper_prefix)]
    wrapper: Option<WrapperPrefix>,
    /// Make this a command launch interpreted by non-interactive Bash.
    /// Shell expansion and operators apply. A custom agent declaration counts as YOLO.
    /// When writing a template, pair --command with --yolo or --no-yolo.
    #[arg(long, value_name = "CMD")]
    command: Option<String>,
    /// The command bypasses its approval/classifier/sandbox safety checks. Farhelm believes this
    /// and never reads the command to check it.
    #[arg(long, conflicts_with = "no_yolo")]
    yolo: bool,
    /// The command retains a safety check that can refuse or ask (it is not YOLO).
    #[arg(long)]
    no_yolo: bool,
    /// Command that resumes a conversation, with {conversation} and
    /// {farhelm_args}; needs --agent. Without it, no Restart.
    #[arg(long, value_name = "CMD")]
    resume_command: Option<String>,
    /// Drop a resume command a template set, so the session cannot be
    /// restarted.
    #[arg(long, conflicts_with = "resume_command")]
    no_resume_command: bool,
}

impl LaunchEditFlags {
    /// The flags, with the verb's folder and title, as the launcher edits
    /// the helm applies after the templates. `--command` is what makes the
    /// launch a command launch; the agent-launch flags set no launch kind,
    /// so on a command template they are refused naming the flag, as the
    /// same edit is refused in the GUI.
    ///
    /// Asked only after [`resolve_prompt_files`] moved a `--prompt-file`'s
    /// contents into `prompt`.
    fn fields(
        &self,
        cwd: Option<String>,
        title: Option<String>,
    ) -> farhelm_proto::launcher::TemplateFields {
        use farhelm_proto::launcher::{LauncherKind, TemplateDestination, TemplateFields};
        // A plain `assert!`: a verb added later that flattens these flags
        // without an arm in `resolve_prompt_files` would otherwise launch,
        // in a release build, without the message the caller gave.
        assert!(
            self.prompt_file.is_none(),
            "resolve_prompt_files reads --prompt-file before the edits are built"
        );
        TemplateFields {
            custom_agent: self
                .agent
                .as_ref()
                .filter(|agent| agent.harness.uses_custom_catalog())
                .map(|agent| agent.custom_agent.clone()),
            target: None,
            prompt: self.prompt.clone(),
            permission_flags: None,
            kind: self.command.as_ref().map(|_| LauncherKind::Command),
            agent: self.agent.as_ref().map(|agent| agent.harness),
            model: self
                .model
                .as_ref()
                .map(|model| (model != "default").then(|| model.clone())),
            effort: self.effort.as_ref().map(|choice| choice.0.clone()),
            permissions: self.permissions.map(|choice| choice.0),
            wrapper_prefix: self.wrapper.as_ref().map(|prefix| Some(prefix.0.clone())),
            container: None,
            // The helm derives trust from Settings and the resolved destination.
            // Keep the wire field absent so CLI launches cannot request it.
            workspace_trust: None,
            command: self.command.clone(),
            yolo: if self.yolo {
                Some(true)
            } else if self.no_yolo {
                Some(false)
            } else {
                None
            },
            resume_command: if self.no_resume_command {
                Some(None)
            } else {
                self.resume_command.clone().map(Some)
            },
            host: None,
            recipe: None,
            destination: cwd.map(TemplateDestination::Folder),
            name: title,
            name_kind: None,
        }
    }
}

/// Keep CLI and wire validation on the same literal-message contract.
fn parse_initial_prompt(text: &str) -> Result<String, String> {
    farhelm_proto::launcher::check_initial_prompt(text)?;
    Ok(text.to_string())
}

/// Keep running-agent submission stricter than a new session's first message.
/// The command must not send a submit key for an empty or whitespace-only paste.
fn parse_agent_prompt(text: &str) -> Result<String, String> {
    farhelm_proto::launcher::check_agent_prompt(text)?;
    Ok(text.to_string())
}

/// Materialize one bounded UTF-8 message for initial or running-agent prompts.
/// Reading the cap plus one byte also bounds stdin. Approval and any create
/// retry digest see these contents, never a path that could change while waiting.
fn read_initial_prompt(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let reader: Box<dyn Read> = if path == Path::new("-") {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(path).map_err(|e| format!("cannot read prompt file: {e}"))?)
    };
    let mut text = String::new();
    reader
        .take((farhelm_proto::launcher::TEMPLATE_FIELDS_CAP + 1) as u64)
        .read_to_string(&mut text)
        .map_err(|e| format!("cannot read a UTF-8 prompt: {e}"))?;
    parse_initial_prompt(&text)
}

/// Apply the running-agent rule to the materialized file or stdin contents.
/// Reusing the initial reader preserves its UTF-8 and byte limits without
/// changing what a new session accepts. The message itself is never trimmed.
fn read_agent_prompt(path: &Path) -> Result<String, String> {
    let text = read_initial_prompt(path)?;
    farhelm_proto::launcher::check_agent_prompt(&text)?;
    Ok(text)
}

/// Read every `--prompt-file` the command names into the `--prompt` beside
/// it, once the whole command line has parsed and before anything is dialed.
///
/// The read happens here rather than in clap's value parser because a parser
/// runs as clap reaches the flag: `--prompt-file -` waited for stdin there,
/// and a usage error later on the line (an unknown flag, a conflict) showed
/// only once stdin closed, or never, when the caller was waiting for the
/// command to fail. A content error is reported from here, with exit status
/// 1, rather than as a usage error.
fn resolve_prompt_files(command: &mut Cmd) -> anyhow::Result<()> {
    fn initial(edits: &mut LaunchEditFlags) -> anyhow::Result<()> {
        if let Some(path) = edits.prompt_file.take() {
            edits.prompt =
                Some(read_initial_prompt(&path).map_err(|error| {
                    anyhow::anyhow!("--prompt-file {}: {error}", path.display())
                })?);
        }
        Ok(())
    }
    match command {
        Cmd::Spawn { launch, .. }
        | Cmd::Agent {
            command: AgentCmd::Create { launch, .. },
        } => initial(&mut launch.edits),
        Cmd::Agent {
            command:
                AgentCmd::Template(
                    TemplateCmd::Create { fields, .. } | TemplateCmd::Edit { fields, .. },
                ),
        } => initial(&mut fields.edits),
        Cmd::Agent {
            command:
                AgentCmd::Prompt {
                    prompt,
                    prompt_file,
                    ..
                },
        } => {
            if let Some(path) = prompt_file.take() {
                *prompt = Some(read_agent_prompt(&path).map_err(|error| {
                    anyhow::anyhow!("--prompt-file {}: {error}", path.display())
                })?);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[derive(Subcommand)]
enum HelmCmd {
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
        /// The root path, taken verbatim.
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
        /// The command, as shell text. It may start with `-`, but then goes
        /// after `--` (`set-post-clone -- -x`), since a separate
        /// hyphen-leading word is read as a flag.
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
        /// Exit when the spawning parent closes the pipe it gave as stdin.
        ///
        /// Hidden because ordinary foreground and systemd supervisors own
        /// their own lifetime. The desktop app used it to tie its supervisor
        /// to itself and no longer does (it starts the supervisor detached;
        /// `farhelm_supervisor::managed_supervisor`); tests still use it.
        #[arg(long, hide = true)]
        exit_on_stdin_close: bool,
        /// TEST ONLY: read this host's boot id from PATH instead of the
        /// kernel. Lets a test harness simulate a reboot against a real
        /// supervisor process by rewriting the file between restarts.
        ///
        /// Hidden because no production launch passes it: the desktop
        /// launcher, remote provisioning's units and a hand-started
        /// supervisor all read the real boot id, and a supervisor pointed at
        /// a file that never changes would never classify a genuine reboot.
        #[arg(long, hide = true, value_name = "PATH")]
        boot_id_file: Option<PathBuf>,
    },
}

/// The `--vendor` flag's spelling: exactly the wire enum's adapters,
/// so the CLI-to-wire mapping is total and cannot drift one variant at a
/// time. Some adapters are installed automatically, Grok and Antigravity are
/// configured by the user, and `internal goose-hook` supplies Goose internally; the flag
/// still mirrors the complete closed wire enum.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum HookVendor {
    Claude,
    Codex,
    Muse,
    Goose,
    Pi,
    Omp,
    Grok,
    Antigravity,
}

impl HookVendor {
    /// Keep the CLI entry point authoritative over the callback's vendor field.
    fn report_vendor(self) -> farhelm_proto::ReportVendor {
        match self {
            HookVendor::Claude => farhelm_proto::ReportVendor::Claude,
            HookVendor::Codex => farhelm_proto::ReportVendor::Codex,
            HookVendor::Muse => farhelm_proto::ReportVendor::Muse,
            HookVendor::Goose => farhelm_proto::ReportVendor::Goose,
            HookVendor::Pi => farhelm_proto::ReportVendor::Pi,
            HookVendor::Omp => farhelm_proto::ReportVendor::Omp,
            HookVendor::Grok => farhelm_proto::ReportVendor::Grok,
            HookVendor::Antigravity => farhelm_proto::ReportVendor::Antigravity,
        }
    }
}

#[derive(Subcommand)]
enum InternalCmd {
    /// Relay one local OpenSSH question through the private socket of its owning helm.
    Askpass { question: std::ffi::OsString },
    /// Proxy stdio to the local supervisor's unix socket. This is the
    /// remote end of the helm's ssh transport: `ssh host farhelm
    /// internal stdio` yields a byte pipe to the supervisor.
    Stdio {
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// End an open but silent connection after this many seconds. The
        /// caller must send heartbeats independently of application work.
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        stdin_idle_limit_secs: Option<u64>,
    },
    /// Run a terminal host (the `terminal-host` crate's entry point): the
    /// child spec on stdin, `--socket` on the command line, nothing else.
    /// Started from the supervisor's own program, never through a forwarder,
    /// and it holds no version of Farhelm: a host lives for weeks and must
    /// keep neither pruning nor uninstall from proceeding.
    TerminalHost {
        #[arg(long)]
        socket: PathBuf,
    },
    /// A development tool, with no compatibility promise: run one program in
    /// a real terminal host the way a supervisor runs an agent in a new
    /// terminal (the supervisor's emulator answering its queries, the
    /// production child environment), and serve its screen, title and input
    /// on `<run dir>/control` until asked to stop. Prints `ready` on stdout
    /// once the program runs. The backend of `scripts/agent-tui.py` and
    /// `scripts/capture-agent-screens.py`; see
    /// `farhelm_supervisor::agent_screen`.
    AgentScreen {
        /// The run's own directory, absolute and short (the scripts make
        /// one directly under `/tmp`): the host's state directory and the
        /// control socket's home.
        #[arg(long, value_parser = absolute_path_parser())]
        run_dir: PathBuf,
        /// The program's working directory, absolute.
        #[arg(long, value_parser = absolute_path_parser())]
        cwd: PathBuf,
        /// The terminal's size, `COLSxROWS`.
        #[arg(long, value_parser = parse_terminal_size)]
        size: (u16, u16),
        /// The program and its arguments, after `--`.
        #[arg(last = true, required = true)]
        argv: Vec<std::ffi::OsString>,
    },
    /// End every terminal host with a socket in a state directory's `hosts`
    /// directory: `Shutdown` through the protocol, or the verified SIGTERM
    /// for a host that cannot take it, removing the socket files of hosts
    /// that are gone. See `farhelm_supervisor::hosts::stop`.
    ///
    /// Exit status 0 is the only success: every host whose socket the
    /// listing found directly in `<state dir>/hosts` was confirmed stopped,
    /// and that directory was listed whole as a real directory (or does not
    /// exist). The directory is listed once, so a host a supervisor still
    /// running there starts later is not covered. Any other status means a
    /// host may still run: 1 when one could not be confirmed stopped or the
    /// listing was refused or incomplete, 2 for a usage error (a relative or
    /// empty `--state-dir` among them, and a `farhelm` from before this
    /// command), anything else for a failure of the command itself. The
    /// whole command takes at most `COMMAND_BUDGET` (30 seconds). One line
    /// per host goes to stderr, for people and logs; stdout stays empty.
    ///
    /// The command line and the meaning of exit status 0 are part of the
    /// contract with every helm from this release on, which runs it on
    /// remote hosts whose installed `farhelm` may be older than the helm
    /// (SPEC_impl.md, "Terminal host"). A caller decides whether to run it
    /// at all by whether `<state dir>/hosts` exists.
    StopTerminalHosts {
        /// The state directory, as an absolute path: the command has no
        /// default, and a relative one would depend on the caller's
        /// working directory.
        #[arg(long, value_parser = absolute_path_parser())]
        state_dir: PathBuf,
    },
    /// Print this build's version and whether it carries the release mark
    /// (`farhelm_helm::RELEASE_BUILD`), for the release workflow's check of
    /// the published binary.
    ///
    /// The output is a contract: with exit status 0, stdout is exactly these
    /// two lines, in this order, each `key=value` with no spaces, and nothing
    /// else:
    ///
    /// ```text
    /// version=<the version `farhelm --version` names>
    /// release-build=yes   (or release-build=no)
    /// ```
    ///
    /// `.github/workflows/sign-sums.yml` compares those bytes exactly, so
    /// adding a line or changing a spelling must update that check in the
    /// same change.
    BuildInfo,
    /// The launch shim: exec a LaunchSpec's argv, recording exec failure
    /// to the spec's status file. See farhelm_supervisor::launch for why
    /// this exists (zsh terminates on failed exec; a shell-side sentinel
    /// can never fire there).
    Launch { spec: PathBuf },
    /// Clone a spare after the user's login shell established its environment.
    BuildSpare { spec: PathBuf },
    /// Bound that shell and ordinary descendants to the supervisor's lifetime.
    BuildSpareGuard { spec: PathBuf },
    /// Record shell handoff before execing the scope wrapper. The exact executable
    /// path is carried through so a bundle forwarder retains its launch semantics.
    LaunchWrapped {
        spec: PathBuf,
        #[arg(long)]
        farhelm_exe: PathBuf,
        #[arg(last = true, required = true)]
        wrapper: Vec<std::ffi::OsString>,
    },
    /// Enter the session directory before starting a tab's login shell. This runs
    /// inside the tab scope, so shell startup retains the tab's cleanup contract.
    TabShell { directory: PathBuf, shell: PathBuf },
    /// An agent hook adapter: read the vendor's JSON payload from stdin and
    /// drop a report of the conversation identity it names for the
    /// supervisor that launched this session to apply.
    ///
    /// Farhelm injects `<farhelm_exe> internal hook` for vendors with a
    /// per-launch hook surface. Grok and Antigravity use callbacks configured
    /// by the user. The hook runs below the agent inside the user's terminal.
    /// Muse gets its session and socket on the launch-local command because
    /// it scrubs the hook's environment. Other commands get their destination
    /// from the environment. See `hook.rs` for the silence and budget contract.
    Hook {
        /// Print [`hook::POINTER_LINE`] on stdout after reporting, so the
        /// agent learns `farhelm agent instructions` exists.
        ///
        /// The decision is the SUPERVISOR's: `FARHELM_AGENT_INSTRUCTIONS` is read once when
        /// the supervisor starts, and a hook process launched by an
        /// already-running supervisor must obey the setting that
        /// supervisor started with rather than whatever the variable says
        /// by the time the agent gets around to firing a hook. Passing it
        /// on the injected command line is what pins it to the launch.
        #[arg(long)]
        announce: bool,
        /// Instructions to print after the pointer, on their own line: the helm's for this launch (today the
        /// blocked-agent instruction). Like `--announce`, set by the supervisor on the injected command, which is how
        /// it survives a vendor that scrubs the hook's environment; ignored without `--announce`.
        #[arg(long, allow_hyphen_values = true)]
        extra_instructions: Option<String>,
        /// Which vendor adapter this hook invocation is: the report
        /// envelope's discriminator, sourced from the installed entry
        /// point rather than inferred from the payload. Required, so an
        /// old hook command that predates the flag fails closed at CLI
        /// parse instead of reporting untagged. The Goose helper never
        /// takes this flag — `internal goose-hook` supplies its value
        /// internally, keeping the persisted declaration unchanged.
        #[arg(long, value_enum)]
        vendor: HookVendor,
        /// The Farhelm session that owns this launch-local report. The socket names its state directory.
        #[arg(long)]
        session_id: Option<std::ffi::OsString>,
        /// The supervisor's socket path. Its parent holds reports even when the supervisor is offline.
        #[arg(long)]
        supervisor_socket: Option<PathBuf>,
    },
    /// The credential-free MCP reporter Goose retains in session metadata.
    GooseHook,
    /// The forwarder's claim on this version: take a shared lock on the
    /// inherited descriptor `fd` (this very program, opened by the
    /// forwarder), check it, and report through the exit status (see
    /// `farhelm_supervisor::version_lock::run_helper`). The lock belongs to
    /// the open file the forwarder keeps, so it outlives this process and is
    /// carried into the program the forwarder then starts.
    ///
    /// The command line is part of the contract with every forwarder an
    /// installer has written since; an older `farhelm` rejects it as an
    /// unknown subcommand (exit status 2), which is how the forwarder tells
    /// it has no version locks.
    VersionLock { fd: i32 },
    /// The hold step a session's window starts with in the Mac app's
    /// version layout: hold this version through the login shell's startup
    /// by handing the lock to a keeper, mark the launch "held", then become
    /// `command` (the login shell and its payload). See
    /// `farhelm_supervisor::session_hold`.
    Hold {
        spec: PathBuf,
        #[arg(last = true, required = true)]
        command: Vec<std::ffi::OsString>,
    },
    /// The keeper the hold step starts: keep the adopted version lock until
    /// the launch shim marks the launch "launched" with `watch`'s pid, or
    /// `watch` exits. See `farhelm_supervisor::session_hold::run_keeper`.
    KeepVersion {
        #[arg(long)]
        watch: i32,
        #[arg(long)]
        ready_fd: i32,
        #[arg(long)]
        launched: PathBuf,
    },
    /// The locked part of the Mac installer, run by `scripts/install.sh`
    /// from the release it just downloaded and checked (see
    /// `farhelm_supervisor::installation::install`). The script reads the
    /// outcome from two files in the staging folder rather than from this
    /// program's output, so its own error formatting stays in one place.
    ///
    /// The command line is part of the contract with every `install.sh`
    /// from this release on; an older `farhelm` rejects it as an unknown
    /// subcommand (exit status 2), which the script reports as a release
    /// too old for it.
    Install(InstallArgs),
}

/// `farhelm internal install`'s arguments; see
/// `farhelm_supervisor::installation::install::InstallRequest` for each.
#[derive(clap::Args)]
struct InstallArgs {
    #[arg(long)]
    staging: PathBuf,
    #[arg(long)]
    version: String,
    #[arg(long)]
    applications: PathBuf,
    #[arg(long)]
    bin_dir: PathBuf,
    #[arg(long)]
    link: PathBuf,
    #[arg(long)]
    running_record: Vec<PathBuf>,
    #[arg(long)]
    update_only: bool,
    /// The inode of the app folder `install.sh` found to be an older
    /// installer's (see `InstallRequest::replace_legacy_bundle`).
    #[arg(long)]
    replace_legacy_bundle: Option<u64>,
    /// The app updater's expectation (`FARHELM_INSTALL_EXPECT`), in
    /// `farhelm_supervisor::installation::Expectation`'s text form. A missing
    /// installation ID leaves a leading dash in the separate argument.
    #[arg(long, allow_hyphen_values = true)]
    expect: Option<String>,
}

/// Run the locked install and leave its outcome where `install.sh` reads
/// it: `install-result` (`updated` or `installed` on the first line, then
/// the notes for the closing message as they are to be printed) on
/// success, `install-failure` (the error
/// line, then advice lines) on a refusal. A program that cannot even write
/// those reports on stderr, which the script shows as is. A refusal exits
/// with its own status (`STATE_CHANGED` when the updater's expectation no
/// longer holds), which the script passes on.
fn run_install(args: InstallArgs) -> anyhow::Result<()> {
    use farhelm_supervisor::installation::install::{InstallRequest, install};
    let request = InstallRequest {
        staging: args.staging.clone(),
        version: args.version,
        applications: args.applications,
        bin_dir: args.bin_dir,
        link: args.link,
        running_records: args.running_record,
        update_only: args.update_only,
        replace_legacy_bundle: args.replace_legacy_bundle,
        lock_wait: farhelm_supervisor::installation::TERMINAL_LOCK_WAIT,
        expect: match args.expect.as_deref() {
            None => None,
            Some(text) => Some(
                farhelm_supervisor::installation::Expectation::parse(text)
                    .ok_or_else(|| anyhow::anyhow!("--expect {text:?} is not an expectation"))?,
            ),
        },
    };
    match install(&request, &mut |_| Ok(())) {
        Ok(done) => {
            let mut report = if done.updated { "updated" } else { "installed" }.to_string();
            report.push('\n');
            for note in done.notes {
                report.push_str(&note);
                report.push('\n');
            }
            std::fs::write(args.staging.join("install-result"), report)?;
            Ok(())
        }
        Err(refusal) => {
            let mut report = refusal.message;
            for line in refusal.advice {
                report.push('\n');
                report.push_str(&line);
            }
            report.push('\n');
            std::fs::write(args.staging.join("install-failure"), report)?;
            std::process::exit(refusal.status)
        }
    }
}

/// `internal agent-screen --size`: `COLSxROWS`, each from 1 to 10,000 (the
/// clamp a supervisor applies to a terminal's size).
fn parse_terminal_size(text: &str) -> Result<(u16, u16), String> {
    let parse = |part: &str| {
        part.parse::<u16>()
            .ok()
            .filter(|n| (1..=10_000).contains(n))
    };
    text.split_once('x')
        .and_then(|(cols, rows)| Some((parse(cols)?, parse(rows)?)))
        .ok_or_else(|| format!("{text:?} is not COLSxROWS"))
}

/// Parse a path the internal commands take only as an absolute one
/// (`stop-terminal-hosts --state-dir`, `agent-screen --run-dir` and `--cwd`),
/// which also refuses an empty one. Refused as a usage error (exit status
/// 2), before anything is looked at: an empty value from a caller's unset
/// variable would otherwise act on whatever the working directory holds
/// (stop-terminal-hosts would stop the hosts of its `./hosts`).
fn absolute_path_parser() -> impl clap::builder::TypedValueParser<Value = PathBuf> {
    use clap::builder::TypedValueParser as _;
    clap::builder::PathBufValueParser::new().try_map(|path: PathBuf| {
        if path.is_absolute() {
            Ok(path)
        } else {
            Err(format!("{path:?} is not an absolute path"))
        }
    })
}

/// Hold this program's own version for the rest of the process, when it runs
/// from the Mac app bundle's version layout (see
/// `farhelm_supervisor::version_lock`). A no-op everywhere else.
///
/// Runs before any command does its work, so a version a running Farhelm
/// program needs can never be deleted under it. The forwarder's handoff
/// variable is removed here, before anything can start a child, whether or
/// not it was adopted. A version that is gone, or still held exclusively by
/// an update or uninstall past the bounded wait, is never run unprotected:
/// every command refuses, agent hooks included. Hooks do it silently and
/// with exit status 0, because their stderr is the agent's terminal and a
/// failure would surface as a hook error; they lose that one report, which
/// the next hook replaces. Only a filesystem that cannot lock at all runs
/// without one, as before version locks existed.
fn hold_own_version(command: &Cmd) -> anyhow::Result<()> {
    use farhelm_supervisor::version_lock::{self, LOCK_FD_ENV, ProcessHold};
    let inherited = std::env::var_os(LOCK_FD_ENV);
    if inherited.is_some() {
        // SAFETY: called first thing in `main`, before this process starts
        // any thread (no runtime exists yet), so nothing reads the
        // environment concurrently.
        unsafe { std::env::remove_var(LOCK_FD_ENV) };
    }
    if matches!(
        command,
        Cmd::Internal {
            command: InternalCmd::VersionLock { .. } | InternalCmd::TerminalHost { .. }
        }
    ) {
        // The helper locks the forwarder's descriptor itself; it takes no
        // second lock of its own. A terminal host holds none at all, like a
        // running agent: it lives for weeks and must keep neither pruning
        // nor uninstall's exclusive lock from proceeding (its second stage
        // also closes any descriptor it inherited, a forwarder's lock
        // included). The lock variable is already cleared above.
        return Ok(());
    }
    let Ok(exe) = std::env::current_exe() else {
        return Ok(());
    };
    let quiet = matches!(
        command,
        Cmd::Internal {
            command: InternalCmd::Hook { .. } | InternalCmd::GooseHook
        } | Cmd::CustomAgent {
            command: CustomAgentCmd::ReportConversation { .. },
        }
    );
    match version_lock::hold_for_process(&exe, inherited.as_deref()) {
        Ok(ProcessHold::NotInLayout | ProcessHold::Held | ProcessHold::Unsupported) => Ok(()),
        Err(_) if quiet => {
            // A hook that cannot hold its version (an update or uninstall
            // holds the lock) skips the report, but its vendor still reads
            // stdout: Antigravity expects its JSON reply on every exit, so
            // answer exactly as the hook's own path would without
            // `--announce`.
            if let Cmd::Internal {
                command: InternalCmd::Hook { vendor, .. },
            } = command
            {
                hook::write_response(&mut std::io::stdout(), vendor.report_vendor(), false, None);
            }
            std::process::exit(0)
        }
        Err(error) => {
            // A session's hold step or launch shim that cannot hold its
            // version records why in the launch's status file, as every
            // other failure to start an agent does, so the session shows an
            // error rather than an ordinary exit.
            if let Cmd::Internal {
                command: InternalCmd::Hold { spec, .. } | InternalCmd::Launch { spec },
            } = command
            {
                farhelm_supervisor::session_hold::record_start_failure(spec, error.to_string());
            }
            Err(anyhow::anyhow!(error))
        }
    }
}

/// The Unicode License V3 notice for the Unicode-derived property tables in the vendored terminal emulator
/// (`vendor/alacritty_terminal`, see `THIRD_PARTY_NOTICES.md`), which every `farhelm` binary links through the
/// supervisor. The vendored tree is upstream code changed only through its patch files, so the notice is embedded here
/// rather than beside the tables.
const UNICODE_NOTICE: &str = include_str!("../../../vendor/UNICODE-LICENSE.txt");

/// The website and offline CLI carry the same contract, including its change history.
const CUSTOM_AGENT_CONTRACT: &str =
    include_str!("../../../website/src/content/docs/docs/agents/custom-agent-contract.md");

/// Strip the website header and the one stability aside's delimiters. Keep the notice text and fenced examples.
fn custom_agent_spec() -> String {
    let body = CUSTOM_AGENT_CONTRACT
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map_or(CUSTOM_AGENT_CONTRACT, |(_, body)| body);
    body.trim()
        .replacen(":::caution[Stability notice]\n", "", 1)
        .replacen("\n:::\n", "\n", 1)
}

/// This pins the printed contract to the implemented argument grammar and validation limits.
#[test]
fn custom_agent_spec_keeps_the_contract_and_omits_site_markup() {
    const UPDATE: &str = "Update custom-agent-contract.md and its ## Contract changelog";
    let spec = custom_agent_spec();
    assert!(spec.starts_with("This specification defines"), "{UPDATE}");
    assert!(
        !spec.lines().any(|line| line.starts_with(":::")),
        "{UPDATE}"
    );
    assert!(spec.contains("no stability guarantee yet"), "{UPDATE}");
    let (_, changelog) = spec.split_once("## Contract changelog\n").expect(UPDATE);
    let entries = changelog.split("\n## ").next().unwrap_or_default();
    assert!(
        entries.lines().any(|line| line.starts_with("- **`v1`")),
        "{UPDATE}"
    );
    for term in farhelm_proto::conversation::RESERVED_LOCATOR_PREFIXES
        .iter()
        .copied()
        .chain([
            "--model",
            "--effort",
            "--resume",
            "256",
            "128",
            "farhelm custom-agent report-conversation --id",
        ])
    {
        assert!(spec.contains(term), "Missing {term:?}. {UPDATE}");
    }
}

/// The license notices of every third-party crate compiled into Farhelm's shipped binaries (`farhelm` and
/// `farhelm-desktop`), generated from `Cargo.lock` by `scripts/dependency-notices.py` and kept current by its `--check`
/// in the ordinary checks. One file covers both binaries, so this binary also carries notices for crates only the
/// desktop app links; that over-covers, it never under-covers.
const DEPENDENCY_NOTICES: &str = include_str!("../notices/dependencies.txt");

/// What `farhelm licenses` prints: every notice that must travel with this binary.
///
/// First the notices for code Farhelm ported or derived from other projects (the supervisor's xterm.js port and the
/// vendored terminal emulator's Unicode-derived tables, checked against the linked binary when this was written), then
/// the dependencies' notices. Code ported or derived later and linked into `farhelm` adds its notice here, so the
/// command keeps covering everything the binary contains (SPEC_impl.md, "Dependencies and licenses").
fn licenses_text() -> String {
    format!(
        "{}\n{}\n{}",
        farhelm_supervisor::grid_input::PORTED_CODE_NOTICES,
        UNICODE_NOTICE,
        DEPENDENCY_NOTICES
    )
}

/// Whether the words after `farhelm custom-agent report-conversation` are
/// exactly one `--help` or `-h`, the only malformed invocation of it that
/// prints anything (see the custom-agent contract's changelog). Any other
/// shape, `--id --help` and `--help --id x` included, stays the silent report
/// a custom agent's wrapper relies on.
fn is_lone_help_request(mut rest: impl Iterator<Item = std::ffi::OsString>) -> bool {
    rest.next()
        .is_some_and(|word| word == "--help" || word == "-h")
        && rest.next().is_none()
}

/// What `farhelm internal build-info` prints, in the form its doc comment
/// promises. Takes the version and the mark as arguments so the tests can
/// render a marked build's answer: a test build is never marked, and the
/// mark is fixed at compile time.
fn build_info_text(version: &str, release_build: bool) -> String {
    let mark = if release_build { "yes" } else { "no" };
    format!("version={version}\nrelease-build={mark}\n")
}

fn main() -> anyhow::Result<()> {
    let mut arguments = std::env::args_os();
    let program = arguments.next();
    let invoked_as = program
        .as_deref()
        .and_then(|program| Path::new(program).file_name());
    if invoked_as.is_some_and(|name| name == farhelm_helm::ASKPASS_JUMP_PROGRAM_NAME) {
        return farhelm_helm::run_askpass_jump(arguments).context("starting SSH for a jump host");
    }
    let mut cli = if invoked_as.is_some_and(|name| name == farhelm_helm::ASKPASS_PROGRAM_NAME) {
        // OpenSSH supplies a single argument, which may begin with a dash or contain multiple lines.
        let question = arguments.next().context("missing SSH question")?;
        anyhow::ensure!(
            arguments.next().is_none(),
            "unexpected SSH helper arguments"
        );
        Cli {
            command: Cmd::Internal {
                command: InternalCmd::Askpass { question },
            },
        }
    } else if arguments.next().is_some_and(|word| word == "custom-agent")
        && arguments
            .next()
            .is_some_and(|word| word == "report-conversation")
        && !is_lone_help_request(std::env::args_os().skip(3))
    {
        // Admission recognizes this exact argv shape. Keep malformed invocations quiet and
        // update the contract changelog in custom-agent-contract.md if the grammar changes.
        // The one exception is a lone `--help` or `-h`, which a person types, never a
        // wrapper reporting a conversation: it goes to clap below, which prints the help.
        let id = if arguments.next().is_some_and(|word| word == "--id") {
            arguments
                .next()
                .and_then(|word| word.into_string().ok())
                .filter(|_| arguments.next().is_none())
        } else {
            None
        };
        Cli {
            command: Cmd::CustomAgent {
                command: CustomAgentCmd::ReportConversation { id },
            },
        }
    } else {
        Cli::parse()
    };
    hold_own_version(&cli.command)?;
    resolve_prompt_files(&mut cli.command)?;
    match cli.command {
        Cmd::CustomAgent {
            command: CustomAgentCmd::Spec,
        } => {
            println!("{}", custom_agent_spec());
            Ok(())
        }
        Cmd::CustomAgent {
            command: CustomAgentCmd::ReportConversation { id },
        } => {
            std::panic::set_hook(Box::new(|_| {}));
            let env = SessionEnv::from_env();
            let (credential, hook_log) = env.hook_inputs(None, None);
            hook::run_custom_report(credential, id.as_deref(), hook_log.as_deref());
            Ok(())
        }
        Cmd::Uninstall(options) => uninstall::run(options),
        Cmd::Licenses => {
            print!("{}", licenses_text());
            Ok(())
        }
        Cmd::Spawn {
            cwd,
            title,
            conversation,
            inherit_agent,
            launch,
            parent,
            idempotency_key,
            ..
        } => {
            let child = runtime()?.block_on(spawn_session(
                &SessionEnv::from_env(),
                SpawnArgs {
                    cwd,
                    title,
                    conversation,
                    // clap's `spawn_launch` group requires one or the other.
                    launch: if inherit_agent {
                        SpawnLaunch::Inherit
                    } else {
                        SpawnLaunch::Flags {
                            templates: launch.templates.clone(),
                            edits: Box::new(launch.edits.fields(None, None)),
                        }
                    },
                    parent,
                    idempotency_key,
                },
            ))?;
            println!("{child}");
            Ok(())
        }
        Cmd::Agent { command } => run_agent(command),
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
            // startup, and this is the ONLY read of `FARHELM_AGENT_HOOKS` in
            // the codebase (plan D5). Everything downstream consults the parsed seam value, so
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
                agent_hooks,
                agent_instructions,
                session_start_path: std::env::var_os(
                    farhelm_supervisor::service::SESSION_START_PATH_ENV,
                ),
                boot_id_file,
            };
            run_supervisor(&dir, startup, exit_on_stdin_close)
        }
        Cmd::Internal { command } => match command {
            InternalCmd::Askpass { question } => {
                farhelm_helm::run_askpass(&question).context("relaying an SSH sign-in question")
            }
            // Fresh synchronous processes: the guard starts its own OS watches
            // before user rc files; neither command needs an async runtime.
            InternalCmd::BuildSpare { spec } => farhelm_supervisor::build_spare(&spec),
            InternalCmd::BuildSpareGuard { spec } => farhelm_supervisor::guard_spare_build(&spec),
            InternalCmd::Stdio {
                state_dir,
                stdin_idle_limit_secs,
            } => {
                // No tracing init — belt and braces, not necessity:
                // tracing goes to stderr by construction (init_tracing),
                // but this process's stdout IS the protocol channel, so
                // the proxy stays as close to a bare pipe as possible
                // rather than trusting every future logging tweak to
                // keep stdout clean. The connected marker and any error
                // diagnostic are explicit stderr writes in the proxy.
                let dir = match state_dir {
                    Some(dir) => dir,
                    None => farhelm_supervisor::default_state_dir()?,
                };
                runtime()?.block_on(stdio_proxy(
                    &dir,
                    stdin_idle_limit_secs.map(std::time::Duration::from_secs),
                ))
            }
            InternalCmd::Launch { spec } => {
                // Holding its own version now, the shim lets the window's
                // keeper go (farhelm_supervisor::session_hold). Outside the
                // version layout there is no keeper and nothing to mark.
                if farhelm_supervisor::version_lock::process_hold().is_some() {
                    farhelm_supervisor::session_hold::mark_launched(&spec);
                }
                // On success exec never returns; reaching here is failure.
                Err(farhelm_supervisor::launch::exec_launch_spec(&spec))
            }
            InternalCmd::LaunchWrapped {
                spec,
                farhelm_exe,
                wrapper,
            } => Err(farhelm_supervisor::launch::exec_wrapped_launch(
                &spec,
                &farhelm_exe,
                &wrapper,
            )),
            InternalCmd::TabShell { directory, shell } => Err(
                farhelm_supervisor::launch::exec_tab_shell(&directory, &shell),
            ),
            InternalCmd::Hold { spec, command } => {
                let exe =
                    anyhow::Context::context(std::env::current_exe(), "locating this program")?;
                // On success the hold step becomes the shell and never
                // returns; reaching here is failure, already recorded.
                Err(farhelm_supervisor::session_hold::run_hold(
                    &exe, &spec, &command,
                ))
            }
            InternalCmd::KeepVersion {
                watch,
                ready_fd,
                launched,
            } => farhelm_supervisor::session_hold::run_keeper(watch, &launched, ready_fd),
            InternalCmd::Install(args) => run_install(args),
            InternalCmd::BuildInfo => {
                // CARGO_PKG_VERSION is the version clap's `--version` prints
                // too, so the two answers cannot disagree.
                print!(
                    "{}",
                    build_info_text(env!("CARGO_PKG_VERSION"), farhelm_helm::RELEASE_BUILD)
                );
                Ok(())
            }
            InternalCmd::TerminalHost { socket } => {
                // First thing after argument parsing, with no thread or
                // runtime started: the host forks.
                std::process::exit(terminal_host::main_entry([
                    std::ffi::OsString::from("--socket"),
                    socket.into_os_string(),
                ]))
            }
            InternalCmd::AgentScreen {
                run_dir,
                cwd,
                size: (cols, rows),
                argv,
            } => {
                use farhelm_supervisor::agent_screen;
                // Absolute by its parser; whether it is short enough depends
                // on the socket names the run makes inside it.
                anyhow::ensure!(
                    agent_screen::run_dir_fits(&run_dir),
                    "--run-dir {} is too long for the sockets it must hold",
                    run_dir.display()
                );
                let farhelm_exe =
                    anyhow::Context::context(std::env::current_exe(), "locating this program")?;
                runtime()?.block_on(agent_screen::run(agent_screen::AgentScreen {
                    run_dir,
                    cwd,
                    cols,
                    rows,
                    argv,
                    farhelm_exe,
                }))
            }
            InternalCmd::StopTerminalHosts { state_dir } => {
                use farhelm_supervisor::hosts::stop;
                let (lines, stopped) = match runtime() {
                    Ok(runtime) => {
                        let report = runtime.block_on(stop::stop_terminal_hosts(&state_dir));
                        // Not the runtime's ordinary drop, which waits for
                        // blocking work: a search for hosts the budget cut
                        // off must not stretch the run past it.
                        runtime.shutdown_background();
                        (report.lines(), report.all_stopped())
                    }
                    Err(error) => (vec![format!("cannot start: {error:#}")], false),
                };
                let lines = lines
                    .into_iter()
                    .map(|line| format!("farhelm internal stop-terminal-hosts: {line}"))
                    .collect();
                // Bounded, and exiting right after: a stderr nobody drains
                // must not hold the exit status back (see
                // `write_lines_within`).
                stop::write_lines_within(std::io::stderr(), lines, stop::REPORT_WAIT);
                std::process::exit(if stopped { 0 } else { 1 })
            }
            InternalCmd::VersionLock { fd } => {
                // Silent by contract: the forwarder runs this with every
                // stream pointed at /dev/null and reads only the status.
                let status = match std::env::current_exe() {
                    Ok(exe) => farhelm_supervisor::version_lock::run_helper(&exe, fd),
                    Err(_) => farhelm_supervisor::version_lock::HELPER_RETRY,
                };
                std::process::exit(status)
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
                    // A missing (or non-UTF-8) AGENT_SESSION_ID still goes
                    // through the reporter; see `report_payload`.
                    let payload = serde_json::to_vec(&goose_hook::report_payload(
                        std::env::var("AGENT_SESSION_ID").ok(),
                    ))?;
                    // The discriminator comes from the entry point
                    // itself — this helper IS the Goose adapter — so
                    // the persisted MCP declaration keeps invoking the
                    // same command with no new stored flag.
                    hook::run_with(
                        credential,
                        std::io::Cursor::new(payload),
                        hook_budget(),
                        hook_log,
                        farhelm_proto::ReportVendor::Goose,
                    );
                }
                // The pointer, and after it any extra instructions the launch
                // carries, only when the launch asked for instructions at all.
                let instructions =
                    (std::env::var_os(farhelm_supervisor::launch::GOOSE_INSTRUCTIONS_ENV_VAR)
                        .as_deref()
                        == Some(std::ffi::OsStr::new("1")))
                    .then(|| {
                        // Empty is the launch saying it has none.
                        let extra = std::env::var(
                            farhelm_supervisor::launch::GOOSE_EXTRA_INSTRUCTIONS_ENV_VAR,
                        )
                        .ok()
                        .filter(|extra| !extra.is_empty());
                        hook::instructions_text(extra.as_deref())
                    });
                goose_hook::serve(
                    std::io::BufReader::new(std::io::stdin()),
                    std::io::stdout(),
                    instructions.as_deref(),
                );
                Ok(())
            }
            InternalCmd::Hook {
                announce,
                extra_instructions,
                vendor,
                session_id,
                supervisor_socket,
            } => {
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
                // line; see `SessionEnv::hook_inputs`.
                let env = SessionEnv::from_env();
                let (credential, hook_log) =
                    env.hook_inputs(session_id.as_deref(), supervisor_socket.as_deref());

                let vendor = vendor.report_vendor();
                hook::run_with(
                    credential,
                    std::io::stdin(),
                    hook_budget(),
                    hook_log,
                    vendor,
                );

                // AFTER the report, never before. The report is the part
                // the session's correctness depends on, and its stdin read
                // is the part with a deadline; the pointer is a nicety that
                // costs a small write to a pipe the vendor is draining.
                // Ordering it second means a stdout that somehow will not
                // take the line cannot delay the report.
                hook::write_response(
                    &mut std::io::stdout(),
                    vendor,
                    announce,
                    extra_instructions.as_deref(),
                );

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

/// Select the hook's process-local budget.
///
/// Production launches use [`hook::HOOK_BUDGET`]. The private environment
/// override exists only for child-process tests: the test puts it on the
/// spawned hook command itself, never on the test runner, so a short fixture
/// budget cannot leak into sibling tests or a live supervisor session.
fn hook_budget() -> std::time::Duration {
    #[cfg(debug_assertions)]
    {
        let value = std::env::var("FARHELM_TEST_HOOK_BUDGET_MS").ok();
        hook_budget_from(value.as_deref())
    }
    #[cfg(not(debug_assertions))]
    {
        hook::HOOK_BUDGET
    }
}

/// Convert the optional child-test override into a bounded hook budget.
///
/// Invalid, empty, and zero values use production behavior; a positive test
/// value can only shorten the production budget, never extend it.
#[cfg(any(debug_assertions, test))]
fn hook_budget_from(value: Option<&str>) -> std::time::Duration {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .map(std::time::Duration::from_millis)
        .filter(|budget| *budget > std::time::Duration::ZERO)
        .map(|budget| budget.min(hook::HOOK_BUDGET))
        .unwrap_or(hook::HOOK_BUDGET)
}

/// Run a supervisor with the optional desktop-app lifetime tether.
///
/// Child destructors are not reliable when a GUI framework terminates its
/// process directly. A pipe is an operating-system lifetime primitive: every
/// exit path closes the desktop parent's write end, so EOF releases the child
/// even when Rust cleanup never runs. EOF is handed to `service::run` as its
/// stop signal, so the child still closes its terminal-output clients in
/// order on the way out, exactly as it does for SIGTERM. The blocking read lives on a detached OS
/// thread, not Tokio's blocking pool: if the supervisor itself fails first,
/// runtime shutdown cannot wait forever for an uncancellable stdin read.
///
/// `startup` arrives ALREADY resolved — the `FARHELM_AGENT_HOOKS` opt-out
/// and the `FARHELM_AGENT_INSTRUCTIONS` switch — and is passed straight
/// through.
/// Resolving any of them here instead would put the decision on both sides
/// of the tether branch, which is one place too many for values that must
/// be identical on every launch path.
///
/// Synchronous because `service::run` owns its runtime, so that the state
/// folder stays claimed until that runtime's blocking work has finished.
fn run_supervisor(
    state_dir: &std::path::Path,
    startup: farhelm_supervisor::service::SupervisorStartup,
    exit_on_stdin_close: bool,
) -> anyhow::Result<()> {
    use anyhow::Context as _;

    if !exit_on_stdin_close {
        return farhelm_supervisor::service::run(state_dir, startup, std::future::pending());
    }
    let (stdin_closed_tx, stdin_closed) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("farhelm-supervisor-stdin-tether".to_string())
        .spawn(move || {
            let result = std::io::copy(&mut std::io::stdin(), &mut std::io::sink()).map(|_| ());
            let _ = stdin_closed_tx.send(result);
        })
        .context("starting desktop supervisor stdin watcher")?;
    // The tether is handed to `run` as its stop signal rather than raced
    // against it here: racing would drop the supervisor mid-stream on EOF,
    // skipping the orderly output shutdown `run` performs on every stop.
    let tether = async move {
        stdin_closed
            .await
            .context("desktop supervisor stdin watcher stopped without reporting EOF")?
            .context("reading the desktop supervisor stdin tether")
    };
    farhelm_supervisor::service::run(state_dir, startup, tether)
}

/// `farhelm agent template create|edit|delete`: one relayed request, which
/// waits for the user's approval, and one confirmation line on stdout naming
/// the template, as the lifecycle verbs confirm on stdout.
fn run_template_write(command: &TemplateCmd) -> anyhow::Result<()> {
    let (verb, done) = match command {
        TemplateCmd::Create {
            name,
            fields,
            quick_launch,
            key,
        } => (
            farhelm_proto::AgentVerb::TemplateCreate {
                name: name.clone(),
                fields: fields.fields(),
                host: fields.host.clone(),
                shortcuts: farhelm_proto::TemplateShortcutEdit {
                    quick_launch: quick_launch.then_some(true),
                    key: key.clone().map(farhelm_proto::TemplateKeyEdit::Set),
                },
            },
            "created",
        ),
        TemplateCmd::Edit {
            name,
            fields,
            quick_launch,
            no_quick_launch,
            key,
            no_key,
        } => (
            farhelm_proto::AgentVerb::TemplateEdit {
                name: name.clone(),
                fields: fields.fields(),
                host: fields.host.clone(),
                shortcuts: farhelm_proto::TemplateShortcutEdit {
                    quick_launch: if *quick_launch {
                        Some(true)
                    } else {
                        no_quick_launch.then_some(false)
                    },
                    key: key
                        .clone()
                        .map(farhelm_proto::TemplateKeyEdit::Set)
                        .or_else(|| no_key.then_some(farhelm_proto::TemplateKeyEdit::Clear)),
                },
            },
            "changed",
        ),
        TemplateCmd::Delete { name } => (
            farhelm_proto::AgentVerb::TemplateDelete { name: name.clone() },
            "deleted",
        ),
    };
    let (_asking, reply) = runtime()?.block_on(agent_request(&SessionEnv::from_env(), verb))?;
    let name = match reply {
        AgentReply::TemplateWritten { template } => template.name,
        AgentReply::TemplateDeleted {} => match command {
            TemplateCmd::Delete { name } => name.clone(),
            _ => anyhow::bail!("the helm answered a template write with a delete confirmation"),
        },
        // `agent_request` already checked the reply's tag against the verb.
        _ => anyhow::bail!("the helm answered a template write with something unexpected"),
    };
    println!("{done} template {}", quoted(&name));
    Ok(())
}

/// `farhelm agent host add|edit|remove`: one relayed request, which waits for the
/// user's explicit Allow, and one confirmation line on stdout naming the host,
/// as the template writes confirm on stdout.
///
/// An edit that names nothing to change is refused here, before anything is
/// sent (the helm refuses it too). An edit's line names the host by the
/// display name the helm reports after the edit, since an alias change gives
/// the host a new one; a removal's names it as the caller did. An add names
/// the destination the user approved, which may differ from the argument,
/// says when the user chose Tailscale (which may differ from `--tailscale`
/// too), and warns that accepting the request does not establish
/// installation.
fn run_host_change(command: &HostCmd) -> anyhow::Result<()> {
    let verb = match command {
        HostCmd::Add {
            destination,
            alias,
            tailscale,
        } => farhelm_proto::AgentVerb::HostAdd {
            destination: destination.clone(),
            alias: alias.clone(),
            tailscale: *tailscale,
        },
        HostCmd::Edit {
            host,
            alias,
            no_alias,
            icon,
            color,
        } => {
            let alias = match (alias, no_alias) {
                (Some(alias), _) => Some(farhelm_proto::HostAliasEdit::Set(alias.clone())),
                (None, true) => Some(farhelm_proto::HostAliasEdit::Clear),
                (None, false) => None,
            };
            if alias.is_none() && icon.is_none() && color.is_none() {
                anyhow::bail!(
                    "farhelm agent host edit needs at least one of --alias, --no-alias, --icon \
                     or --color"
                );
            }
            farhelm_proto::AgentVerb::HostEdit {
                host: host.clone(),
                alias,
                icon: *icon,
                color: *color,
            }
        }
        HostCmd::Remove { host } => farhelm_proto::AgentVerb::HostRemove { host: host.clone() },
    };
    let (_asking, reply) = runtime()?.block_on(agent_request(&SessionEnv::from_env(), verb))?;
    match (reply, command) {
        (
            AgentReply::HostAddAccepted {
                destination,
                tailscale,
            },
            HostCmd::Add { .. },
        ) => {
            println!("{}", host_add_accepted_line(&destination, tailscale));
        }
        (AgentReply::HostEdited { host }, HostCmd::Edit { .. }) => {
            println!("changed host {}", quoted(&host.name));
        }
        (AgentReply::HostRemoved {}, HostCmd::Remove { host }) => {
            println!("removed host {}", quoted(host));
        }
        // `agent_request` already checked the reply's tag against the verb.
        _ => anyhow::bail!("the helm answered a host change with something unexpected"),
    }
    Ok(())
}

/// The confirmation `farhelm agent host add` prints once the user approved.
///
/// It names what the user approved, not what the agent asked for: the user
/// can edit the destination and toggle the Tailscale host option in the add
/// dialog, so an agent that asked for Tailscale learns here whether the host
/// is being added that way. A plain SSH add says nothing about Tailscale,
/// which keeps the line unchanged for callers that never asked.
fn host_add_accepted_line(destination: &str, tailscale: bool) -> String {
    let route = if tailscale { " with Tailscale" } else { "" };
    format!(
        "accepted; Farhelm is adding {}{route}. Accepted is not installed: adding may fail, need setup, or be cancelled. Check farhelm agent hosts and the host's state before relying on it.",
        quoted(destination)
    )
}

/// Run one `farhelm agent` subcommand and print its answer.
///
/// `AgentCmd::verb` answers "what goes on the wire, if anything" —
/// `None` for `instructions`/`help`, which must work with no
/// supervisor, no credential, and no helm anywhere in sight (see its
/// own docs).
fn run_agent(command: AgentCmd) -> anyhow::Result<()> {
    if let AgentCmd::Profiles { .. } = command {
        anyhow::bail!(AGENT_PROFILES_REMOVED);
    }
    if let AgentCmd::Template(template) = &command {
        return run_template_write(template);
    }
    if let AgentCmd::Host(host) = &command {
        return run_host_change(host);
    }
    let Some(verb) = command.verb() else {
        print!("{}", agent_instructions::text());
        return Ok(());
    };
    // A discovery command prints its own reply and never enters mutation dispatch.
    if let AgentCmd::Hosts { json }
    | AgentCmd::Sessions { json }
    | AgentCmd::Agents { json }
    | AgentCmd::Templates { json } = &command
    {
        return print_agent_listing(verb, *json);
    }
    let env = SessionEnv::from_env();
    // A restart can kill the CLI carrying this request. Resolve a possible
    // full self reference before dispatch, while this process can still warn
    // about the acknowledgement it may never receive. Discovery supplies only
    // identity here; it must not infer consent or predict whether restart works.
    if let AgentCmd::Restart { session, .. } = &command
        && runtime()?.block_on(is_self_restart(&env, session))?
    {
        eprintln!(
            "warning: restarting this session can interrupt the invoking CLI; \
             acknowledgement can be lost, and resumed task continuation is not guaranteed"
        );
    }
    let (_asking, reply) = runtime()?.block_on(agent_request(&env, verb))?;
    match command {
        AgentCmd::OpenUrl { .. } => {
            let AgentReply::UrlOpened {} = reply else {
                anyhow::bail!(
                    "the helm answered open-url with something other than an opened link"
                );
            };
        }
        // One line, so an agent's transcript shows the notification went out.
        AgentCmd::Notify { .. } => {
            let AgentReply::Notified {} = reply else {
                anyhow::bail!(
                    "the helm answered notify with something other than a notification confirmation"
                );
            };
            println!("Notified the user.");
        }
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
        AgentCmd::Prompt { session, .. } => {
            let AgentReply::Prompted {} = reply else {
                anyhow::bail!("the helm answered prompt with something other than confirmation");
            };
            // This confirms terminal input acceptance, not the agent's work.
            // The empty reply leaves the exact requested target as attribution.
            println!("prompt sent to {}", safe_cell(&session));
        }
        AgentCmd::Delete { session, .. } => {
            let AgentReply::Deleted { outcome } = reply else {
                anyhow::bail!("the helm answered delete with something other than confirmation");
            };
            println!("deleted {}", safe_cell(&session));
            if let Some(notice) = outcome.notice {
                eprintln!("{}", safe_cell(&notice));
            } else if outcome.archived {
                eprintln!("managed checkout archived to the trash");
            }
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
        | AgentCmd::Agents { .. }
        | AgentCmd::Templates { .. }
        | AgentCmd::Profiles { .. }
        | AgentCmd::Template(_)
        | AgentCmd::Host(_)
        | AgentCmd::Instructions
        | AgentCmd::Help => {
            unreachable!("handled above before this match is reached")
        }
    }
    Ok(())
}

/// Recognize a deliberate self restart before sending the request that can kill this CLI.
///
/// The supervisor credential stays bare, including when its opaque ID resembles a full
/// reference: exact equality therefore wins before parsing. Other targets need discovery
/// only when their bare suffix could be ours. The entry returned by that read-only request
/// distinguishes a full self reference from another host's session with the same bare ID.
/// A discovery failure aborts before mutation; it cannot produce an outcome-unknown restart.
async fn is_self_restart(env: &SessionEnv, target: &str) -> anyhow::Result<bool> {
    let asking = env.dial("farhelm agent restart")?.session_id;
    if target == asking {
        return Ok(true);
    }
    let Ok(reference) = target.parse::<farhelm_proto::SessionRef>() else {
        // The helm owns target refusals and their discovery remedy. A target
        // that cannot name this caller does not need an extra round trip here.
        return Ok(false);
    };
    if reference.bare_id() != asking {
        return Ok(false);
    }
    let (_, reply) = agent_request(env, farhelm_proto::AgentVerb::Hosts {}).await?;
    let AgentReply::Hosts { caller_host_id, .. } = reply else {
        anyhow::bail!("the helm answered hosts with something other than a hosts listing");
    };
    Ok(reference == farhelm_proto::SessionRef::new(&caller_host_id, &asking)?)
}

/// Print discovery as a human table or a versioned JSON envelope.
///
/// JSON retains exact caller identity and the reply's completeness fields
/// so agents can resolve explicit targets. Table truncation warnings go to
/// stderr to preserve the existing stdout table contract. A listing ends
/// after printing and never enters the lifecycle mutation path.
fn print_agent_listing(verb: farhelm_proto::AgentVerb, json: bool) -> anyhow::Result<()> {
    let (asking, mut reply) = runtime()?.block_on(agent_request(&SessionEnv::from_env(), verb))?;
    if json {
        let caller_host_id = match &reply {
            AgentReply::Hosts { caller_host_id, .. }
            | AgentReply::Sessions { caller_host_id, .. }
            | AgentReply::Agents { caller_host_id, .. }
            | AgentReply::Templates { caller_host_id, .. } => caller_host_id.clone(),
            _ => anyhow::bail!("only discovery replies can be printed as JSON"),
        };
        // Authentication remains bare on the unchanged supervisor relay. Only
        // the helm knows this session's entry, so qualify the caller using the
        // reply's identity rather than guessing from a session row or marker.
        let caller_session_id = farhelm_proto::SessionRef::new(&caller_host_id, &asking)?;
        // Bumped whenever a value an agent reads changes meaning or
        // disappears. 3: `restart_offer` lost `fresh_only` and
        // `fallback_template` when restart came to mean resume only; `resume`
        // kept its spelling and meaning. 4: a session's `agent` is always its
        // agent type's word or `custom`, never a profile name, and the
        // profiles listing is gone. 5: `restart_offer` may be
        // `no_resume_command`, a command launch without a resume command.
        // 6: the templates listing, whose reply is new (nothing earlier
        // changed meaning).
        // 7: session IDs bind to their host entry, including caller.session_id;
        // host IDs are the entry UUID rather than the helm's integer row ID.
        // 8: agent discovery exposes built-in and custom launch choices.
        // Spell out the default in this discovery view, where an agent needs
        // to know what a named template does. Persistence and write replies
        // still omit Base; changing their normalized shape would dirty editors.
        if let AgentReply::Templates { templates, .. } = &mut reply {
            for template in templates {
                if template.fields.name.is_some() {
                    template
                        .fields
                        .name_kind
                        .get_or_insert(farhelm_proto::launcher::TemplateNameKind::Base);
                }
            }
        }
        let envelope = serde_json::json!({
            "schema_version": 8,
            "caller": {
                "session_id": caller_session_id.to_string(),
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
///
/// A log line that cannot be written is dropped silently
/// (`log_internal_errors(false)`). By default the subscriber reports a failed
/// write with `eprintln!` to the same stderr, which panics when stderr is a
/// terminal that has gone away. That is exactly a hand-started supervisor's
/// situation after its terminal closes or its ssh connection drops: the
/// first log line after the hangup panicked the process before its orderly
/// shutdown could run, the abrupt exit that shutdown exists to avoid.
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .log_internal_errors(false)
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
/// An opted-in caller's silence also ends the proxy after `stdin_idle_limit`:
/// a gateway may retain its TCP peer after a sleeping helm stops sending,
/// so EOF alone is not a reliable liveness signal. Legacy callers have no
/// deadline. The Tensorlake transport opts in only after a capability check,
/// and its connection writer sends independent heartbeats every 30 seconds.
/// Refresh/cache/database work is not a liveness signal and may take longer.
///
/// Failure to make the initial local socket connection exits 75 only for
/// not-found/refused. Provisioning treats that status as positive absence;
/// every other failure remains an ordinary error and must never offer setup.
/// A successful socket connection emits one stderr marker before either pump
/// starts. It reports transport progress without adding bytes to the framed
/// stdout stream or claiming that the supervisor has answered the hello.
async fn stdio_proxy(
    state_dir: &std::path::Path,
    stdin_idle_limit: Option<std::time::Duration>,
) -> anyhow::Result<()> {
    use anyhow::Context;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
            // The dial names no remedy, so this path adds how to start a
            // supervisor by hand. It is read in the helm's log and by someone
            // running `internal stdio` by hand; a remote host's row shows the
            // helm's own start command for exit 75, written from the host
            // entry. `--state-dir` is always spelled out: `internal stdio` is
            // normally reached over ssh with a state dir the remote's default
            // would not match, and a remedy that silently drops it starts a
            // supervisor in the wrong place. The quoting is for spaces, so the
            // line survives a paste into a shell; `to_string_lossy` is no more
            // lossy than the `Path::display` the dial already uses for the
            // socket path, and this whole string is advice for a human.
            let advice = if absent {
                format!(
                    "; start it with `farhelm supervisor run --state-dir {}`",
                    shell_words::quote(&state_dir.to_string_lossy()),
                )
            } else {
                String::new()
            };
            eprintln!("farhelm internal stdio: {error:#}{advice}");
            std::process::exit(if absent { 75 } else { 1 });
        });
    eprintln!("{}", farhelm_proto::io::SUPERVISOR_CONNECTED_MARKER);
    let (mut sock_r, mut sock_w) = tokio::io::split(stream);
    // The watchdog lives outside the pump: a blocked supervisor write must
    // not suspend the silence deadline. None means stdin reached clean EOF,
    // which keeps the existing half-close-and-drain contract.
    let (activity, mut input_activity) =
        tokio::sync::watch::channel(Some(tokio::time::Instant::now()));
    let mut upstream = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut bytes = [0; 16 * 1024];
        loop {
            let count = stdin
                .read(&mut bytes)
                .await
                .context("reading stdin from helm")?;
            if count == 0 {
                activity.send_replace(None);
                break;
            }
            activity.send_replace(Some(tokio::time::Instant::now()));
            sock_w
                .write_all(&bytes[..count])
                .await
                .context("copying stdin to supervisor")?;
        }
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
    let idle = async {
        let Some(limit) = stdin_idle_limit else {
            return std::future::pending::<anyhow::Result<()>>().await;
        };
        loop {
            let Some(last) = *input_activity.borrow_and_update() else {
                return std::future::pending::<anyhow::Result<()>>().await;
            };
            tokio::select! {
                biased;
                changed = input_activity.changed() => {
                    if changed.is_err() {
                        return std::future::pending::<anyhow::Result<()>>().await;
                    }
                }
                () = tokio::time::sleep_until(last + limit) => {
                    anyhow::bail!("no input from the helm within the idle limit; assuming the connection is gone");
                }
            }
        }
    };
    tokio::pin!(idle);

    // A clean stdin EOF only half-closes the upstream and keeps waiting
    // for in-flight replies. An upstream failure is different: no more
    // request bytes can reach the supervisor, so reporting success—or
    // leaving a read-only proxy parked on downstream output—would hide
    // the broken transport.
    let result = tokio::select! {
        result = &mut idle => result,
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
    /// The updater writes a leading dash for a two-field installation record. The shell passes
    /// that expectation as a separate word, so clap must leave it intact for the installer.
    #[farhelm_testtrace::test]
    fn install_cli_accepts_an_expectation_without_an_id() {
        use super::{Cli, Cmd, InternalCmd};
        use clap::Parser as _;
        use farhelm_supervisor::installation::Expectation;

        let cli = Cli::try_parse_from([
            "farhelm",
            "internal",
            "install",
            "--staging",
            "staging",
            "--version",
            "1.2.4",
            "--applications",
            "Applications",
            "--bin-dir",
            "bin",
            "--link",
            "bin/farhelm",
            "--expect",
            "-:0:1.2.3",
        ])
        .expect("the installer CLI accepts a separate expectation with a leading dash");
        let Cmd::Internal {
            command: InternalCmd::Install(args),
        } = cli.command
        else {
            panic!("expected the internal install subcommand");
        };
        assert_eq!(
            args.expect.as_deref().and_then(Expectation::parse),
            Some(Expectation {
                id: None,
                generation: 0,
                installed: Some("1.2.3".into()),
            }),
        );
    }

    /// Spec: `farhelm agent create --recipe` becomes the environment verb,
    /// with `--container-host` only beside `--recipe`; `--recipe` refuses
    /// `--host` and `--idempotency-key`; `clone` may omit `--host` (SPEC.md,
    /// "Agents and environments"). Help explains why a recipe launch
    /// can leave the folder unset.
    ///
    /// Why: sending a recipe as an ordinary create would let a helm that
    /// predates environment launches create an ordinary session somewhere
    /// else, and a keyed environment launch could never be found again.
    #[test]
    fn recipe_flags_send_the_environment_verb_and_refuse_placement_mixes() {
        use super::{Cli, Cmd};
        use clap::Parser as _;
        let agent = |args: &[&str]| {
            let mut full = vec!["farhelm", "agent"];
            full.extend_from_slice(args);
            Cli::try_parse_from(full)
        };
        let Err(help) = agent(&["create", "--help"]) else {
            panic!("expected create help");
        };
        assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = help
            .to_string()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(help.contains("A recipe launch defaults to the new environment's home."));
        assert!(help.contains("--cwd accepts only the literal '~'."));
        let Cmd::Agent { command } = agent(&[
            "create",
            "--recipe",
            "dev",
            "--container-host",
            "build-box",
            "--command",
            "true",
            "--no-yolo",
        ])
        .unwrap()
        .command
        else {
            panic!("agent create")
        };
        let Some(farhelm_proto::AgentVerb::CreateEnvironment {
            recipe,
            container_host,
            edits,
            ..
        }) = command.verb()
        else {
            panic!("an environment launch")
        };
        assert_eq!(recipe, "dev");
        assert_eq!(container_host.as_deref(), Some("build-box"));
        assert_eq!(edits.command.as_deref(), Some("true"));
        for refused in [
            &["create", "--recipe", "dev", "--host", "h"][..],
            &["create", "--recipe", "dev", "--idempotency-key", "k"],
            &["create", "--recipe", "dev", "--conversation", "existing"],
            &["create", "--container-host", "build-box", "--host", "h"],
            &["create", "--container-host", "build-box"],
        ] {
            assert!(agent(refused).is_err(), "{refused:?}");
        }
        let Cmd::Agent { command } = agent(&["clone", "--source-session", "id"]).unwrap().command
        else {
            panic!("agent clone")
        };
        assert!(matches!(
            command.verb(),
            Some(farhelm_proto::AgentVerb::Clone { host: None, .. })
        ));
    }

    /// A delayed helper invocation from the retired container model must fail
    /// parsing instead of retaining authority to create or enter a container.
    #[test]
    fn retired_container_helpers_have_no_cli_entry_point() {
        use clap::Parser;
        // This positional argv was valid for the historical creator. Its
        // refusal must identify removal of the subcommand itself.
        let prepare = super::Cli::try_parse_from([
            "farhelm",
            "internal",
            "container-prepare",
            "old.json",
            "0",
        ])
        .err()
        .expect("the retired container creator is absent");
        assert_eq!(prepare.kind(), clap::error::ErrorKind::InvalidSubcommand);
        assert!(prepare.to_string().contains("container-prepare"));
        let tab = super::Cli::try_parse_from([
            "farhelm",
            "internal",
            "tab-shell",
            "/",
            "/bin/sh",
            "--container",
            "old.json",
        ])
        .err()
        .expect("the retired container tab option is absent");
        assert_eq!(tab.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    /// The release workflow accepts a published binary only if `farhelm
    /// internal build-info` prints a marked build's answer byte for byte, and
    /// no test build is marked, so this is the one test of that spelling
    /// before a release meets it.
    ///
    /// Spec: a marked build of version `v` prints exactly
    /// `version=<v>\nrelease-build=yes\n` (the `BuildInfo` contract).
    #[test]
    fn build_info_renders_a_marked_build_exactly() {
        assert_eq!(
            super::build_info_text("1.2.3", true),
            "version=1.2.3\nrelease-build=yes\n"
        );
    }

    /// `farhelm licenses` is the only place the xterm.js port's MIT notice
    /// ships since the web bundle that carried it was removed; losing it
    /// would ship ported code without its required attribution.
    ///
    /// Spec: the command parses and is listed in help, and what it prints
    /// holds the xterm.js copyright line and the MIT permission notice.
    #[test]
    fn licenses_prints_the_xterm_js_notice() {
        use clap::{CommandFactory, Parser};
        let cli = super::Cli::try_parse_from(["farhelm", "licenses"]).unwrap();
        assert!(matches!(cli.command, super::Cmd::Licenses));
        let listed = super::Cli::command()
            .get_subcommands()
            .any(|sub| sub.get_name() == "licenses" && !sub.is_hide_set());
        assert!(listed, "`farhelm licenses` must be visible in help");
        let text = super::licenses_text();
        assert!(
            text.contains(
                "Copyright (c) 2017-2019, The xterm.js authors (https://github.com/xtermjs/xterm.js)"
            ),
            "the xterm.js copyright line is missing"
        );
        assert!(
            text.contains("Permission is hereby granted, free of charge"),
            "the MIT permission notice is missing"
        );
        assert!(
            text.contains("UNICODE LICENSE V3")
                && text.contains("Copyright © 1991-2026 Unicode, Inc."),
            "the Unicode notice for the terminal emulator's derived tables is missing"
        );
        // The generated dependency notices follow, with a crate every farhelm binary links named under its license.
        assert!(
            text.contains(
                "Third-party license notices for Farhelm's farhelm and farhelm-desktop binaries"
            ),
            "the dependency notices are missing"
        );
        // Anywhere in a wrapped "Used by:" list, not only first on its line.
        assert!(
            text.contains(" tokio 1."),
            "a linked dependency (tokio) is not named in the dependency notices"
        );
        // A crate whose own notice cargo-about could not match on its own (a `clarify` entry fixes it) carries its real
        // copyright line, and no SPDX template placeholder reached the output.
        assert!(
            text.contains("Copyright (c) 2019-2025 Frank Denis"),
            "minisign-verify's own copyright line is missing"
        );
        // The holder half of the template is the tell: a bare `<year>` is legitimate, because FreeType's license
        // (shipped as its own notice) quotes the credit line it asks users to fill in.
        assert!(
            !text.contains("<copyright holders>") && !text.contains("<owner>"),
            "an SPDX template placeholder reached the notices"
        );
    }

    /// New helpers cannot impose liveness on old helms. Absence of the
    /// opt-in argument must decode as no deadline, including future changes
    /// to clap defaults that a short process observation could not detect.
    #[test]
    fn stdio_idle_deadline_is_opt_in_and_advertised() {
        use clap::{CommandFactory, Parser};
        let cli = super::Cli::try_parse_from(["farhelm", "internal", "stdio"]).unwrap();
        assert!(matches!(
            cli.command,
            super::Cmd::Internal {
                command: super::InternalCmd::Stdio {
                    stdin_idle_limit_secs: None,
                    ..
                }
            }
        ));
        let mut command = super::Cli::command();
        let help = command
            .find_subcommand_mut("internal")
            .unwrap()
            .find_subcommand_mut("stdio")
            .unwrap()
            .render_long_help()
            .to_string();
        assert!(
            help.contains("--stdin-idle-limit-secs"),
            "the transport's capability probe must find the opt-in flag"
        );
    }

    /// Trust is a managed-checkout setting, so every CLI launch-writing
    /// surface must reject the retired flag instead of accepting an inert edit.
    #[test]
    fn launch_and_template_commands_reject_per_launch_trust() {
        for prefix in [
            vec!["farhelm", "agent", "create", "--cwd", "/work"],
            vec!["farhelm", "spawn", "--cwd", "/work"],
            vec!["farhelm", "agent", "template", "create", "review"],
            vec!["farhelm", "agent", "template", "edit", "review"],
        ] {
            let mut args = prefix;
            args.extend(["--agent", "codex", "--trust", "true"]);
            let Err(error) = Cli::try_parse_from(args) else {
                panic!("--trust was removed");
            };
            assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
            assert!(error.to_string().contains("--trust"));
        }
    }

    /// The CLI has no custom-flags text field. Accepting the enum's wire word
    /// here would create an incomplete selection; templates remain the supported
    /// CLI path for a complete Custom choice.
    #[test]
    fn custom_permissions_need_a_template_in_the_cli() {
        assert!(
            super::parse_permissions("custom")
                .unwrap_err()
                .contains("templates")
        );
        assert!(super::parse_permissions("guarded").is_ok());
        assert!(super::parse_permissions("yolo").is_ok());
    }

    /// The same raw shell words must reach create, spawn and template writes;
    /// stripping quotes here would change later Bash expansion.
    /// Inheritance remains exclusive because it copies the source launch.
    #[test]
    fn wrapper_flag_is_a_shared_launch_edit() {
        let words = vec![
            "with-endpoint".to_string(),
            "--profile".into(),
            "'work space'".into(),
            "--".into(),
        ];
        let command = "with-endpoint --profile 'work space' --";
        let Cmd::Agent { command: create } = Cli::try_parse_from([
            "farhelm",
            "agent",
            "create",
            "--cwd",
            "/work",
            "--agent",
            "codex",
            "--wrapper",
            command,
        ])
        .unwrap()
        .command
        else {
            panic!("agent create")
        };
        let Some(farhelm_proto::AgentVerb::Create { edits, .. }) = create.verb() else {
            panic!("create intent")
        };
        assert_eq!(edits.wrapper_prefix, Some(Some(words.clone())));

        let Cmd::Spawn { launch, .. } = Cli::try_parse_from([
            "farhelm",
            "spawn",
            "--cwd",
            "/work",
            "--agent",
            "codex",
            "--wrapper",
            command,
        ])
        .unwrap()
        .command
        else {
            panic!("spawn")
        };
        assert_eq!(
            launch.edits.fields(None, None).wrapper_prefix,
            Some(Some(words.clone()))
        );
        assert!(
            Cli::try_parse_from([
                "farhelm",
                "spawn",
                "--cwd",
                "/work",
                "--inherit-agent",
                "--wrapper",
                command,
            ])
            .is_err()
        );

        let Cmd::Agent {
            command: AgentCmd::Template(TemplateCmd::Create { fields, .. }),
        } = Cli::try_parse_from([
            "farhelm",
            "agent",
            "template",
            "create",
            "wrapped",
            "--agent",
            "codex",
            "--wrapper",
            command,
        ])
        .unwrap()
        .command
        else {
            panic!("template create")
        };
        assert_eq!(fields.fields().wrapper_prefix, Some(Some(words)));
    }
    use super::*;

    /// The hook budget override is a child-only testing seam: absent, invalid,
    /// and zero values must preserve production behavior, while positive values
    /// may shorten but never lengthen the shared 30-second budget.
    #[farhelm_testtrace::test]
    fn hook_budget_override_is_bounded_and_fail_closed() {
        assert_eq!(hook_budget_from(None), hook::HOOK_BUDGET);
        assert_eq!(hook_budget_from(Some("garbage")), hook::HOOK_BUDGET);
        assert_eq!(hook_budget_from(Some("0")), hook::HOOK_BUDGET);
        assert_eq!(
            hook_budget_from(Some("5000")),
            std::time::Duration::from_secs(5)
        );
        assert_eq!(hook_budget_from(Some("999999")), hook::HOOK_BUDGET);
    }

    /// Every checkout-config verb accepts `--host` and `--state-dir`, and
    /// the two value-taking verbs take their value POSITIONALLY, a
    /// hyphen-leading one intact after `--`, while a separate hyphen-leading
    /// word is refused as an unknown flag.
    ///
    /// Why: a post-clone command is shell text that may start with `-`, so
    /// `--` must carry it verbatim; but taking any hyphen-leading word as the
    /// value would also take a mistyped flag as the command to run after
    /// every clone.
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
            "--host",
            "3",
            "--",
            "-rm -rf /tmp/x",
        ])
        .unwrap();
        let Cmd::Helm {
            command: HelmCmd::CheckoutConfig { command },
        } = cli.command
        else {
            panic!("expected the checkout-config subcommand");
        };
        let CheckoutConfigCmd::SetPostClone { command, host, .. } = command else {
            panic!("expected set-post-clone");
        };
        assert_eq!(command, "-rm -rf /tmp/x");
        assert_eq!(host, Some(3));

        for verb in ["set-root", "set-post-clone"] {
            let error = Cli::try_parse_from(["farhelm", "helm", "checkout-config", verb, "-x"])
                .err()
                .unwrap_or_else(|| panic!("{verb} took a separate hyphen-leading word"));
            assert_eq!(
                error.kind(),
                clap::error::ErrorKind::UnknownArgument,
                "{verb}: {error}"
            );
        }
    }

    /// Spec: `--confirm-yolo`, and its older name
    /// `--allow-yolo-on-sensitive-host`, are refused on every creating verb
    /// with a message saying an agent cannot confirm a YOLO launch, and
    /// neither appears in help.
    ///
    /// Why: the flag let an agent assert the user's approval itself, which is
    /// the honour system SPEC.md's permission prompts replace. Scripts and
    /// agents following older instructions may still pass it; a refusal that
    /// says why beats clap's "unexpected argument", and the helm ignores the
    /// field anyway.
    #[farhelm_testtrace::test]
    fn the_yolo_override_is_refused_under_both_names() {
        for flag in ["--confirm-yolo", "--allow-yolo-on-sensitive-host"] {
            for argv in [
                &["farhelm", "spawn", "--cwd", "/w", "--inherit-agent", flag][..],
                &[
                    "farhelm",
                    "agent",
                    "create",
                    "--host",
                    "h",
                    "--cwd",
                    "/w",
                    "--command",
                    "p",
                    "--no-yolo",
                    flag,
                ],
                &[
                    "farhelm",
                    "agent",
                    "clone",
                    "--source-session",
                    "s1",
                    "--host",
                    "h",
                    flag,
                ],
            ] {
                let Err(error) = Cli::try_parse_from(argv) else {
                    panic!("{argv:?}: the flag must be refused");
                };
                let words = error.to_string();
                assert!(
                    words.contains("cannot confirm a YOLO launch"),
                    "{argv:?}: {error}"
                );
                assert!(words.contains("On a host that asks before YOLO launches"));
                assert!(!words.contains("before any agent launch"));
            }
        }
        let mut command = <Cli as clap::CommandFactory>::command();
        for verb in [&["spawn"][..], &["agent", "create"], &["agent", "clone"]] {
            let mut sub = &mut command;
            for name in verb {
                sub = sub.find_subcommand_mut(name).unwrap();
            }
            let help = sub.render_long_help().to_string();
            assert!(!help.contains("confirm-yolo"), "{verb:?}: {help}");
            assert!(!help.contains("sensitive-host"), "{verb:?}: {help}");
        }
    }

    /// The CLI must never manufacture a target or consent from the
    /// caller's discovery cache. This parser-level boundary catches the
    /// unsafe failure before any authenticated relay request exists.
    #[farhelm_testtrace::test]
    fn agent_restart_requires_its_explicit_target() {
        assert!(Cli::try_parse_from(["farhelm", "agent", "restart"]).is_err());
        assert!(Cli::try_parse_from(["farhelm", "agent", "restart", "--session", "s1"]).is_ok());
    }

    /// The removed `--mode` is refused, with or without a value, by a
    /// message saying restart has one kind and the flag should be dropped.
    ///
    /// Why: agents in sessions started before this release were taught to
    /// pass `--mode resume` (SPEC_impl.md, "What running sessions hold
    /// across versions"). SPEC.md decides the retirement and requires the
    /// refusal to name what replaced the flag, so an agent can recover from
    /// the message alone.
    #[farhelm_testtrace::test]
    fn agent_restart_refuses_the_removed_mode_flag_by_name() {
        for args in [
            &[
                "farhelm",
                "agent",
                "restart",
                "--session",
                "s1",
                "--mode",
                "resume",
            ][..],
            &[
                "farhelm",
                "agent",
                "restart",
                "--session",
                "s1",
                "--mode",
                "fresh",
            ][..],
            &["farhelm", "agent", "restart", "--session", "s1", "--mode"][..],
        ] {
            let error = match Cli::try_parse_from(args) {
                Ok(_) => panic!("{args:?} must be refused"),
                Err(error) => error.to_string(),
            };
            assert!(
                error.contains("no longer takes --mode") && error.contains("drop --mode"),
                "{args:?}: {error}"
            );
        }
    }

    /// Spec: the profile selectors removed with agent profiles —
    /// `farhelm spawn --profile-id`, and `farhelm agent create --profile`
    /// and `--profile-id` — are refused at parse with a message saying
    /// profiles were removed and naming what to pass instead. (`spawn
    /// --agent` took a profile name then; it is now the agent type flag.)
    ///
    /// Why: agents in sessions started before the upgrade may still follow
    /// instructions that used these flags (SPEC_impl.md, "What running
    /// sessions hold across versions"). SPEC.md decides the retirement and
    /// requires the refusal to name the replacement, so an agent recovers
    /// from the message alone instead of meeting clap's "unexpected
    /// argument". A bare flag with no value is refused the same way.
    #[farhelm_testtrace::test]
    fn removed_profile_selectors_are_refused_by_name() {
        let spawn = ["farhelm", "spawn", "--cwd", "/w"];
        let create = ["farhelm", "agent", "create", "--host", "h", "--cwd", "/w"];
        let cases: [(&[&str], &[&str], &str); 4] = [
            (&spawn, &["--profile-id", "builtin-claude"], "--template"),
            (&create, &["--profile", "claude"], "--command"),
            (&create, &["--profile"], "--command"),
            (&create, &["--profile-id", "builtin-claude"], "--command"),
        ];
        for (base, extra, replacement) in cases {
            let args: Vec<&str> = base.iter().chain(extra).copied().collect();
            let error = match Cli::try_parse_from(&args) {
                Ok(_) => panic!("{args:?} must be refused"),
                Err(error) => error.to_string(),
            };
            assert!(
                error.contains("agent profiles were removed") && error.contains(replacement),
                "{args:?}: {error}"
            );
        }
    }

    /// Spec: `farhelm agent profiles` still parses, with or without
    /// `--json`, and running it is refused with a message saying profiles
    /// were removed, before any supervisor is dialed.
    ///
    /// Why: the same compatibility reason as the selector refusals above.
    /// The refusal happens in `run_agent` rather than at parse because a
    /// subcommand has no value parser to refuse with; checking it here
    /// keeps a later refactor from turning the hidden verb back into
    /// clap's "unrecognized subcommand" or, worse, into the instructions
    /// text that `verb() == None` prints.
    #[farhelm_testtrace::test]
    fn agent_profiles_is_refused_before_dialing() {
        for args in [
            &["farhelm", "agent", "profiles"][..],
            &["farhelm", "agent", "profiles", "--json"][..],
        ] {
            let Cmd::Agent { command } = Cli::try_parse_from(args).expect("still parses").command
            else {
                panic!("{args:?} parses as an agent command");
            };
            let error = run_agent(command).expect_err("refused").to_string();
            assert!(
                error.contains("farhelm agent profiles was removed")
                    && error.contains("farhelm agent templates"),
                "{args:?}: {error}"
            );
        }
    }

    /// Spec: `farhelm spawn --inherit-agent` is refused beside any launch
    /// flag, `--yolo` with `--no-yolo` is refused, and a launch flag given
    /// twice is refused, all at parse.
    ///
    /// Why: SPEC.md makes `--inherit-agent` exclusive with every launch flag
    /// and requires a repeated or contradicted command flag to be refused
    /// "rather than resolved by order"; clap's defaults would otherwise keep
    /// the last value, launching something other than one of the two the
    /// caller wrote.
    #[farhelm_testtrace::test]
    fn contradicting_or_repeated_launch_flags_are_refused() {
        let spawn = ["farhelm", "spawn", "--cwd", "/w"];
        let create = ["farhelm", "agent", "create", "--host", "h", "--cwd", "/w"];
        let cases: [(&[&str], &[&str]); 7] = [
            (&spawn, &["--inherit-agent", "--agent", "claude"]),
            (&spawn, &["--inherit-agent", "--template", "t"]),
            (&spawn, &["--inherit-agent", "--command", "sh", "--no-yolo"]),
            (&create, &["--command", "sh", "--yolo", "--no-yolo"]),
            (
                &create,
                &["--command", "sh", "--command", "bash", "--no-yolo"],
            ),
            (&create, &["--agent", "claude", "--agent", "codex"]),
            (
                &create,
                &["--agent", "claude", "--effort", "high", "--effort", "low"],
            ),
        ];
        for (base, extra) in cases {
            let args: Vec<&str> = base.iter().chain(extra).copied().collect();
            assert!(
                Cli::try_parse_from(&args).is_err(),
                "{args:?} must be refused"
            );
        }
        assert!(
            Cli::try_parse_from(["farhelm", "spawn", "--cwd", "/w", "--inherit-agent"]).is_ok()
        );
    }

    /// Agent IDs and permissions have local spelling checks. Effort has a local size bound so the
    /// helm can handle ordinary typos with the resolved agent's vocabulary.
    #[farhelm_testtrace::test]
    fn an_unknown_launch_choice_lists_the_choices() {
        let create = |extra: &[&str]| {
            let args: Vec<&str> = ["farhelm", "agent", "create", "--host", "h", "--cwd", "/w"]
                .iter()
                .chain(extra)
                .copied()
                .collect();
            match Cli::try_parse_from(args) {
                Ok(_) => panic!("{extra:?} must be refused"),
                Err(error) => error.to_string(),
            }
        };
        let agent = create(&["--agent", "Not An ID"]);
        assert!(
            agent.contains("is not an agent type")
                && agent.contains("codex, claude")
                && agent.contains("farhelm agent agents")
                && !agent.contains("custom,")
                && !agent.contains(", custom;"),
            "{agent}"
        );
        let long = "x".repeat(257);
        let effort = create(&["--agent", "codex", "--effort", &long]);
        assert!(effort.contains("1 to 256 bytes"), "{effort}");
        let permissions = create(&["--agent", "codex", "--permissions", "free"]);
        assert!(
            permissions.contains("smart_approve") && permissions.contains("guarded"),
            "{permissions}"
        );
        assert!(
            !permissions.contains("or default"),
            "the compatibility alias is not the visible permission vocabulary: {permissions}"
        );
    }

    /// The agent ID and effort compose with templates, while the start selector travels only on Create.
    #[farhelm_testtrace::test]
    fn custom_create_keeps_the_named_start_outside_template_edits() {
        let cli = Cli::try_parse_from([
            "farhelm",
            "agent",
            "create",
            "--agent",
            "acme-code",
            "--effort",
            "deep_work",
            "--conversation",
            "conv-1",
            "--cwd",
            "/work",
            "--host",
            "this machine",
        ])
        .unwrap();
        let Cmd::Agent { command } = cli.command else {
            panic!("not the agent command")
        };
        let Some(farhelm_proto::AgentVerb::Create {
            edits,
            conversation,
            ..
        }) = command.verb()
        else {
            panic!("not a create verb");
        };
        assert_eq!(edits.agent, Some(farhelm_proto::LaunchHarness::Custom));
        assert_eq!(edits.custom_agent, Some(Some("acme-code".into())));
        assert_eq!(
            edits.effort,
            Some(Some(farhelm_proto::LaunchEffortValue::Custom(
                "deep_work".into()
            )))
        );
        assert_eq!(conversation.as_deref(), Some("conv-1"));
    }

    /// Spec: `--effort` and `--model` refuse a separate value that starts with `-` as a usage error, on every verb that
    /// shares the launch flags (`agent create`, `spawn`, and a template write once its name is given). A following
    /// flag such as `--yolo` leaves the option without a value; `-fast` is read as the unknown short option `-f`.
    ///
    /// Why: when these options took any separate value, a caller who forgot the value got the next flag consumed as
    /// it, and for a custom agent nothing downstream refused that: `--effort --yolo` launched it with `--yolo` as its
    /// effort and without the YOLO assertion the caller meant. A parse error naming the option (or `-f`), rather than a
    /// parse that put the next token in the edits, is what shows it was not swallowed; pinning the kind and the named
    /// argument keeps an unrelated parse failure from passing for it.
    #[farhelm_testtrace::test]
    fn effort_and_model_refuse_a_separate_hyphen_leading_value() {
        use clap::error::{ContextKind, ErrorKind};
        for verb in [
            ["agent", "create"].as_slice(),
            ["spawn"].as_slice(),
            ["agent", "template", "create", "t"].as_slice(),
        ] {
            for (flag, value, kind, named) in [
                (
                    "--effort",
                    "--yolo",
                    ErrorKind::InvalidValue,
                    "--effort <LEVEL>",
                ),
                ("--effort", "-fast", ErrorKind::UnknownArgument, "-f"),
                ("--model", "--yolo", ErrorKind::InvalidValue, "--model <ID>"),
                ("--model", "-fast", ErrorKind::UnknownArgument, "-f"),
            ] {
                let mut args = vec!["farhelm"];
                args.extend_from_slice(verb);
                args.extend(["--agent", "acme-code", flag, value]);
                let error = match Cli::try_parse_from(args.iter().copied()) {
                    Ok(_) => panic!("{args:?} parsed; {value} was taken as the {flag} value"),
                    Err(error) => error,
                };
                assert_eq!(error.kind(), kind, "{args:?}: {error}");
                assert_eq!(
                    error
                        .get(ContextKind::InvalidArg)
                        .map(|arg| arg.to_string())
                        .as_deref(),
                    Some(named),
                    "{args:?}: {error}"
                );
            }
        }
    }

    /// Spec: every free-text option but `--prompt` refuses an omitted value as a usage error, whether a following flag
    /// leaves it without one (`host add dev --alias --tailscale`) or a separate hyphen-leading word stands where its
    /// value would be (read as the unknown short option `-q`). The options covered are the session, title, host,
    /// directory, recipe, template, prompt-file, idempotency-key and shell-text options of `farhelm agent`'s verbs, a
    /// template write's fields, and the launch flags on `agent create`, `spawn` and a template write. Each of them
    /// still takes a hyphen-leading value joined with `=` (`--alias=-v`).
    ///
    /// Why: while these options took any separate value, a forgotten value took the next flag as the value: an alias
    /// named `--tailscale` with the Tailscale option silently dropped, a title of `--yolo` on a launch that then
    /// lacked the YOLO assertion. Pinning the kind and the argument the error names keeps an unrelated parse failure
    /// (a missing required argument, say) from passing for the refusal.
    #[farhelm_testtrace::test]
    fn free_text_options_refuse_an_omitted_value_instead_of_taking_the_next_flag() {
        use clap::error::{ContextKind, ErrorKind};
        const CREATE: &[&str] = &["agent", "create"];
        const CLONE: &[&str] = &["agent", "clone"];
        const TEMPLATE: &[&str] = &["agent", "template", "create", "t"];
        const SPAWN: &[&str] = &["spawn"];
        // (verb, option, a real flag of that verb that may follow it)
        let mut cases: Vec<(&[&str], &str, &str)> = vec![
            (&["agent", "rename", "t"], "--session", "--expected-title"),
            (&["agent", "rename", "t"], "--expected-title", "--session"),
            (&["agent", "stop"], "--session", "--session"),
            (&["agent", "prompt"], "--session", "--prompt"),
            (&["agent", "prompt"], "--prompt-file", "--session"),
            (&["agent", "delete"], "--session", "--stop-if-running"),
            (&["agent", "restart"], "--session", "--stop-if-running"),
            (CREATE, "--cwd", "--yolo"),
            (CREATE, "--host", "--yolo"),
            (CREATE, "--recipe", "--yolo"),
            (CREATE, "--container-host", "--yolo"),
            (CREATE, "--title", "--yolo"),
            (CREATE, "--idempotency-key", "--yolo"),
            (CLONE, "--source-session", "--title"),
            (CLONE, "--host", "--title"),
            (CLONE, "--cwd", "--title"),
            (CLONE, "--title", "--cwd"),
            (CLONE, "--idempotency-key", "--title"),
            (&["agent", "host", "add", "dev"], "--alias", "--tailscale"),
            (&["agent", "host", "edit", "dev"], "--alias", "--color"),
            (TEMPLATE, "--cwd", "--quick-launch"),
            (TEMPLATE, "--title", "--quick-launch"),
            (TEMPLATE, "--host", "--quick-launch"),
        ];
        // A template write's `--template` is refused whatever follows it
        // (`a_template_write_refuses_template_however_it_is_given`).
        cases.extend([
            (CREATE, "--template", "--yolo"),
            (SPAWN, "--template", "--yolo"),
        ]);
        for verb in [CREATE, SPAWN, TEMPLATE] {
            for flag in [
                "--prompt-file",
                "--wrapper",
                "--command",
                "--resume-command",
            ] {
                cases.push((verb, flag, "--yolo"));
            }
        }
        for (verb, flag, next) in cases {
            for (value, kind, named) in [
                (next, ErrorKind::InvalidValue, flag),
                ("-q", ErrorKind::UnknownArgument, "-q"),
            ] {
                let mut args = vec!["farhelm"];
                args.extend_from_slice(verb);
                args.extend([flag, value]);
                let error = match Cli::try_parse_from(args.iter().copied()) {
                    Ok(_) => panic!("{args:?} parsed; {value} was taken as the {flag} value"),
                    Err(error) => error,
                };
                assert_eq!(error.kind(), kind, "{args:?}: {error}");
                let invalid = error
                    .get(ContextKind::InvalidArg)
                    .map(|arg| arg.to_string())
                    .unwrap_or_default();
                // An option's name comes with its value name (`--alias <NAME>`).
                assert!(
                    invalid == named || invalid.starts_with(&format!("{named} ")),
                    "{args:?} named {invalid:?}, not {named}: {error}"
                );
            }
            // The `=` form carries the value. A verb may still fail for a
            // reason of its own (a required argument these argv leave out);
            // what must not happen is
            // `-v` read as a flag or the option left without a value.
            let joined = format!("{flag}=-v");
            let mut args = vec!["farhelm"];
            args.extend_from_slice(verb);
            args.push(&joined);
            if let Err(error) = Cli::try_parse_from(args.iter().copied()) {
                assert!(
                    !matches!(
                        error.kind(),
                        ErrorKind::UnknownArgument | ErrorKind::InvalidValue
                    ),
                    "{args:?} refused the joined value: {error}"
                );
            }
        }
    }

    /// Spec: in the whole CLI, only `--prompt` (on `agent prompt` and with the launch flags wherever they appear),
    /// `internal install --expect` and `internal hook --extra-instructions` take a separate value starting with `-`.
    ///
    /// Why: that allowance is what let a forgotten value take the next flag. The behavioral tests above cover the
    /// options that lost it; this one walks the command tree, so an option added later with the allowance fails here
    /// unless it is added to this list on purpose.
    #[farhelm_testtrace::test]
    fn only_the_listed_arguments_take_a_separate_hyphen_leading_value() {
        use clap::CommandFactory as _;
        fn walk(command: &clap::Command, path: &str, found: &mut Vec<String>) {
            for arg in command.get_arguments() {
                if arg.is_allow_hyphen_values_set() {
                    found.push(format!("{path} {}", arg.get_id()));
                }
            }
            for sub in command.get_subcommands() {
                walk(sub, &format!("{path} {}", sub.get_name()), found);
            }
        }
        let mut found = Vec::new();
        walk(&Cli::command(), "farhelm", &mut found);
        found.sort();
        assert_eq!(
            found,
            [
                "farhelm agent create prompt",
                "farhelm agent prompt prompt",
                "farhelm agent template create prompt",
                "farhelm agent template edit prompt",
                "farhelm internal hook extra_instructions",
                "farhelm internal install expect",
                "farhelm spawn prompt",
            ]
        );
    }

    /// Spec: the free-text positionals (a rename's title, a template write's or delete's name, a host add's
    /// destination, a host edit's or remove's host) read a separate hyphen-leading word as an unknown flag, and take a
    /// hyphen-leading value verbatim after `--`.
    ///
    /// Why: these positionals used to take any word, so a mistyped flag in their place (`host remove --alias`) became
    /// the name the request acted on. `--` is the one spelling left for a name that really starts with `-`, so it must
    /// still carry one through to the parsed command.
    #[farhelm_testtrace::test]
    fn free_text_positionals_read_a_hyphen_word_as_a_flag_and_take_one_after_double_dash() {
        use clap::error::{ContextKind, ErrorKind};
        let verbs: [&[&str]; 7] = [
            &["agent", "rename", "--session", "s", "--expected-title", "o"],
            &["agent", "template", "create"],
            &["agent", "template", "edit"],
            &["agent", "template", "delete"],
            &["agent", "host", "add"],
            &["agent", "host", "edit"],
            &["agent", "host", "remove"],
        ];
        for verb in verbs {
            let mut refused = vec!["farhelm"];
            refused.extend_from_slice(verb);
            refused.push("-q");
            let error = match Cli::try_parse_from(refused.iter().copied()) {
                Ok(_) => panic!("{refused:?} took -q as its positional"),
                Err(error) => error,
            };
            assert_eq!(
                error.kind(),
                ErrorKind::UnknownArgument,
                "{refused:?}: {error}"
            );
            assert_eq!(
                error
                    .get(ContextKind::InvalidArg)
                    .map(|arg| arg.to_string())
                    .as_deref(),
                Some("-q"),
                "{refused:?}: {error}"
            );

            let mut taken = vec!["farhelm"];
            taken.extend_from_slice(verb);
            taken.extend(["--", "-draft"]);
            let Cmd::Agent { command } = Cli::try_parse_from(taken.iter().copied())
                .unwrap_or_else(|error| panic!("{taken:?}: {error}"))
                .command
            else {
                panic!("{taken:?} is an agent verb");
            };
            let value = match command {
                AgentCmd::Rename { title, .. } => title,
                AgentCmd::Template(
                    TemplateCmd::Create { name, .. }
                    | TemplateCmd::Edit { name, .. }
                    | TemplateCmd::Delete { name },
                ) => name,
                AgentCmd::Host(HostCmd::Add { destination, .. }) => destination,
                AgentCmd::Host(HostCmd::Edit { host, .. } | HostCmd::Remove { host }) => host,
                _ => panic!("{taken:?} parsed as another verb"),
            };
            assert_eq!(value, "-draft", "{taken:?}");
        }
    }

    /// Spec: a custom agent's model or effort that starts with `-` still reaches the create request verbatim when
    /// joined to its option with `=`.
    ///
    /// Why: custom agents may use hyphen-leading identifiers, and the `=` form is the one way left to pass them once
    /// a separate hyphen-leading value is refused.
    #[farhelm_testtrace::test]
    fn custom_create_accepts_a_hyphen_leading_effort_and_model_joined_with_equals() {
        let Cmd::Agent { command } = Cli::try_parse_from([
            "farhelm",
            "agent",
            "create",
            "--agent",
            "acme-code",
            "--effort=-fast",
            "--model=-local",
        ])
        .unwrap()
        .command
        else {
            panic!("agent create")
        };
        let Some(farhelm_proto::AgentVerb::Create { edits, .. }) = command.verb() else {
            panic!("create intent")
        };
        assert_eq!(edits.custom_agent, Some(Some("acme-code".into())));
        assert_eq!(
            edits.effort,
            Some(Some(farhelm_proto::LaunchEffortValue::Custom(
                "-fast".into()
            )))
        );
        assert_eq!(edits.model, Some(Some("-local".into())));
    }

    /// Guarded is explicit intent, while the old default spelling remains
    /// omission for callers and templates written before the rename. Keeping
    /// it absent is necessary for Goose, Pi and OpenCode's YOLO admission
    /// default.
    #[farhelm_testtrace::test]
    fn guarded_permissions_and_the_legacy_default_alias_reach_create_intent() {
        for (word, permission) in [
            ("guarded", Some(farhelm_proto::LaunchPermission::Guarded)),
            ("default", None),
        ] {
            let Cmd::Agent { command } = Cli::try_parse_from([
                "farhelm",
                "agent",
                "create",
                "--host",
                "h",
                "--cwd",
                "/w",
                "--agent",
                "codex",
                "--permissions",
                word,
            ])
            .unwrap()
            .command
            else {
                panic!("agent create")
            };
            let Some(farhelm_proto::AgentVerb::Create { edits, .. }) = command.verb() else {
                panic!("create intent")
            };
            assert_eq!(edits.permissions, Some(permission), "{word}");
        }
    }

    /// Managed checkout flags express repository intent only on create and
    /// template writes. Pin both spellings and the folder conflict here so
    /// spawn and clone cannot accidentally inherit this new authority.
    #[farhelm_testtrace::test]
    fn managed_checkout_flags_are_scoped_to_create_and_template_writes() {
        use farhelm_proto::launcher::TemplateDestination;
        for flag in ["--gh", "--github"] {
            let Cmd::Agent { command } = Cli::try_parse_from([
                "farhelm", "agent", "create", "--host", "h", flag, "acme/web", "--agent", "codex",
            ])
            .unwrap()
            .command
            else {
                panic!("agent create");
            };
            let Some(farhelm_proto::AgentVerb::Create { edits, .. }) = command.verb() else {
                panic!("create request");
            };
            assert_eq!(
                edits.destination,
                Some(TemplateDestination::Github("acme/web".into()))
            );
            for verb in ["create", "edit"] {
                let Cmd::Agent {
                    command: AgentCmd::Template(template),
                } = Cli::try_parse_from([
                    "farhelm", "agent", "template", verb, "t", flag, "acme/web",
                ])
                .unwrap()
                .command
                else {
                    panic!("template command");
                };
                let fields = match template {
                    TemplateCmd::Create { fields, .. } | TemplateCmd::Edit { fields, .. } => fields,
                    _ => panic!("template write"),
                };
                assert_eq!(
                    fields.fields().destination,
                    Some(TemplateDestination::Github("acme/web".into()))
                );
                assert!(
                    Cli::try_parse_from([
                        "farhelm", "agent", "template", verb, "t", flag, "acme/web", "--cwd", "/w",
                    ])
                    .is_err()
                );
            }
            assert!(
                Cli::try_parse_from([
                    "farhelm", "agent", "create", flag, "acme/web", "--cwd", "/w"
                ])
                .is_err()
            );
            assert!(Cli::try_parse_from(["farhelm", "spawn", flag, "acme/web"]).is_err());
            assert!(
                Cli::try_parse_from([
                    "farhelm",
                    "agent",
                    "clone",
                    "--source-session",
                    "s",
                    "--host",
                    "h",
                    flag,
                    "acme/web"
                ])
                .is_err()
            );
        }
        assert!(Cli::try_parse_from(["farhelm", "agent", "create", "--gh", "not-a-repo"]).is_err());
    }

    /// Template edits must preserve omission: a title-only edit retains Fixed,
    /// while an explicit Base can replace it. A kind-only edit needs the helm's
    /// stored name, so the CLI carries it without requiring a new title.
    /// Create and spawn keep their own titles exact and expose no policy flag.
    #[farhelm_testtrace::test]
    fn template_title_kind_is_optional_and_scoped_to_template_writes() {
        use farhelm_proto::launcher::TemplateNameKind;
        for verb in ["create", "edit"] {
            for (extra, expected_name, expected_kind) in [
                (vec!["--title", "review"], Some("review"), None),
                (
                    vec!["--title", "review", "--title-kind", "base"],
                    Some("review"),
                    Some(TemplateNameKind::Base),
                ),
                (
                    vec!["--title", "review", "--title-kind", "fixed"],
                    Some("review"),
                    Some(TemplateNameKind::Fixed),
                ),
                (
                    vec!["--title-kind", "base"],
                    None,
                    Some(TemplateNameKind::Base),
                ),
            ] {
                let args = ["farhelm", "agent", "template", verb, "t"]
                    .into_iter()
                    .chain(extra)
                    .collect::<Vec<_>>();
                let Cmd::Agent {
                    command: AgentCmd::Template(template),
                } = Cli::try_parse_from(args).unwrap().command
                else {
                    panic!("template write")
                };
                let fields = match template {
                    TemplateCmd::Create { fields, .. } | TemplateCmd::Edit { fields, .. } => {
                        fields.fields()
                    }
                    _ => panic!("template write"),
                };
                assert_eq!(fields.name.as_deref(), expected_name);
                assert_eq!(fields.name_kind, expected_kind);
            }
            let error = Cli::try_parse_from([
                "farhelm",
                "agent",
                "template",
                verb,
                "t",
                "--title-kind",
                "other",
            ])
            .err()
            .expect("unknown kind refused")
            .to_string();
            assert!(error.contains("[possible values: base, fixed]"), "{error}");
        }
        for args in [
            vec!["farhelm", "agent", "create", "--title-kind", "base"],
            vec!["farhelm", "spawn", "--title-kind", "base"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }

    /// Spec: the launch flags become the launcher edits the helm applies
    /// after the templates: `--command` makes a command launch with the
    /// YOLO answer given, `--cwd` is the folder and `--title` the name, and
    /// `default` resets a choice (sent as `null`), as `--no-resume-command`
    /// resets the resume command, while an omitted flag sends nothing.
    ///
    /// Why: absent-versus-reset is what lets a flag undo a template's choice
    /// without the CLI knowing the template ("the other flags then act as
    /// further edits"); collapsing the two would make every omitted flag
    /// wipe what a template set.
    #[farhelm_testtrace::test]
    fn launch_flags_become_the_launcher_edits() {
        use farhelm_proto::launcher::{LauncherKind, TemplateDestination};
        let parse = |extra: &[&str]| {
            let args: Vec<&str> = ["farhelm", "agent", "create"]
                .iter()
                .chain(extra)
                .copied()
                .collect();
            let Cmd::Agent {
                command: command @ AgentCmd::Create { .. },
            } = Cli::try_parse_from(args).expect("parses").command
            else {
                panic!("a create");
            };
            let Some(farhelm_proto::AgentVerb::Create {
                host,
                templates,
                edits,
                ..
            }) = command.verb()
            else {
                panic!("a create verb");
            };
            (host, templates, edits)
        };
        let (host, templates, edits) = parse(&[
            "--template",
            "a",
            "--template",
            "b",
            "--agent",
            "codex",
            "--effort",
            "default",
            "--model",
            "gpt-6-luna",
        ]);
        assert_eq!(host, None, "no --host leaves it to a template");
        assert_eq!(templates, ["a", "b"], "templates keep their order");
        assert_eq!(edits.kind, None, "agent-launch flags set no launch kind");
        assert_eq!(edits.agent, Some(farhelm_proto::LaunchHarness::Codex));
        assert_eq!(edits.model, Some(Some("gpt-6-luna".to_string())));
        assert_eq!(edits.effort, Some(None), "default resets");
        assert_eq!(edits.permissions, None, "an omitted flag sends nothing");
        assert_eq!(edits.destination, None);

        let (_, _, edits) = parse(&[
            "--host",
            "h",
            "--cwd",
            "/w",
            "--title",
            "t",
            "--command",
            "sh",
            "--no-yolo",
            "--resume-command",
            "sh -r {conversation}",
        ]);
        assert_eq!(edits.kind, Some(LauncherKind::Command));
        assert_eq!(edits.command.as_deref(), Some("sh"));
        assert_eq!(edits.yolo, Some(false));
        assert_eq!(
            edits.resume_command,
            Some(Some("sh -r {conversation}".to_string()))
        );
        assert_eq!(
            edits.destination,
            Some(TemplateDestination::Folder("/w".to_string()))
        );
        assert_eq!(edits.name.as_deref(), Some("t"));

        let (_, _, edits) = parse(&["--command", "sh", "--no-yolo", "--no-resume-command"]);
        assert_eq!(
            edits.resume_command,
            Some(None),
            "--no-resume-command resets"
        );
    }

    /// Spec: parsing a command line reads no `--prompt-file`: a path that names no file parses, on every verb that
    /// takes one, and the file's contents reach `--prompt` only through `resolve_prompt_files`, which `main` runs once
    /// the whole line has parsed. A usage error later on the line is reported by the parse itself.
    ///
    /// Why: the file used to be read by clap's value parser as clap reached the flag, so `--prompt-file -` waited for
    /// stdin before a later unknown flag or conflict could be reported, and a caller waiting for that error waited
    /// until stdin closed. That a missing file parses is what shows the parser did not open it.
    #[farhelm_testtrace::test]
    fn prompt_files_are_read_after_the_command_line_parses() {
        let temp = farhelm_teststate::tempdir().unwrap();
        let file = temp.path().join("message.txt");
        std::fs::write(&file, "- first bullet\n").unwrap();
        let missing = temp.path().join("missing.txt");
        let missing = missing.to_str().unwrap();
        let verbs: [&[&str]; 5] = [
            &["spawn", "--cwd", "/w"],
            &["agent", "create"],
            &["agent", "template", "create", "t"],
            &["agent", "template", "edit", "t"],
            &["agent", "prompt", "--session", "s"],
        ];
        for verb in verbs {
            let argv = |path: &str| {
                let mut argv = vec!["farhelm"];
                argv.extend_from_slice(verb);
                argv.extend(["--prompt-file", path]);
                argv.into_iter().map(str::to_string).collect::<Vec<_>>()
            };
            Cli::try_parse_from(argv(missing))
                .unwrap_or_else(|error| panic!("{verb:?} read the file while parsing: {error}"));
            // With stdin at /dev/null this guards only the order of the two
            // errors; `prompt_file_dash_does_not_wait_for_stdin_before_a_usage_error`
            // in `tests/agent_cli.rs` runs it against a stdin left open.
            let mut unknown = argv("-");
            unknown.push("--bogus".into());
            let error = Cli::try_parse_from(unknown)
                .err()
                .unwrap_or_else(|| panic!("{verb:?} took --bogus"));
            assert_eq!(
                error.kind(),
                clap::error::ErrorKind::UnknownArgument,
                "{verb:?}: {error}"
            );

            let mut cli = Cli::try_parse_from(argv(file.to_str().unwrap())).unwrap();
            resolve_prompt_files(&mut cli.command).unwrap();
            let prompt = match cli.command {
                Cmd::Spawn { launch, .. }
                | Cmd::Agent {
                    command: AgentCmd::Create { launch, .. },
                } => {
                    assert_eq!(launch.edits.prompt_file, None, "{verb:?}");
                    launch.edits.prompt
                }
                Cmd::Agent {
                    command:
                        AgentCmd::Template(
                            TemplateCmd::Create { fields, .. } | TemplateCmd::Edit { fields, .. },
                        ),
                } => {
                    assert_eq!(fields.edits.prompt_file, None, "{verb:?}");
                    fields.edits.prompt
                }
                Cmd::Agent {
                    command:
                        AgentCmd::Prompt {
                            prompt,
                            prompt_file,
                            ..
                        },
                } => {
                    assert_eq!(prompt_file, None, "{verb:?}");
                    prompt
                }
                _ => panic!("{verb:?} parsed as another command"),
            };
            assert_eq!(prompt.as_deref(), Some("- first bullet\n"), "{verb:?}");

            let mut cli = Cli::try_parse_from(argv(missing)).unwrap();
            let error = resolve_prompt_files(&mut cli.command)
                .expect_err("a missing file is refused once it is read");
            assert!(
                error.to_string().contains("--prompt-file"),
                "{verb:?}: {error}"
            );
        }
    }

    /// Spec: `farhelm agent template create` and `edit` refuse `--template` as a usage error naming the reason, with a
    /// value, with none, and with a hyphen-leading one, and it stays out of their help; `agent create` still takes
    /// it, and `spawn --inherit-agent` with it is a conflict.
    ///
    /// Why: a template written from other templates would copy their withheld command text into one the agent wrote,
    /// so the flag must be refused rather than resolved. It is parsed at all only so a caller who carries it over from
    /// `agent create` learns why, instead of clap's "unexpected argument".
    #[farhelm_testtrace::test]
    fn a_template_write_refuses_template_however_it_is_given() {
        use clap::CommandFactory as _;
        use clap::error::ErrorKind;
        for verb in ["create", "edit"] {
            for tail in [
                &["--template", "other"][..],
                &["--template"][..],
                &["--template=-other"][..],
                &["--template", "--yolo"][..],
            ] {
                let mut argv = vec!["farhelm", "agent", "template", verb, "t"];
                argv.extend_from_slice(tail);
                let error = Cli::try_parse_from(argv.iter().copied())
                    .err()
                    .unwrap_or_else(|| panic!("{argv:?} parsed"));
                assert_eq!(
                    error.kind(),
                    ErrorKind::ValueValidation,
                    "{argv:?}: {error}"
                );
                assert!(
                    error.to_string().contains("takes no --template"),
                    "{argv:?}: {error}"
                );
            }
            let help = Cli::command()
                .find_subcommand_mut("agent")
                .and_then(|agent| agent.find_subcommand_mut("template"))
                .and_then(|template| template.find_subcommand_mut(verb))
                .expect("the template verb")
                .render_long_help()
                .to_string();
            assert!(!help.contains("--template"), "{verb}: {help}");
        }
        let Cmd::Agent {
            command: AgentCmd::Create { launch, .. },
        } = Cli::try_parse_from(["farhelm", "agent", "create", "--template", "a"])
            .unwrap()
            .command
        else {
            panic!("agent create")
        };
        assert_eq!(launch.templates, ["a"]);
        let error = Cli::try_parse_from([
            "farhelm",
            "spawn",
            "--cwd",
            "/w",
            "--inherit-agent",
            "--template",
            "a",
        ])
        .err()
        .expect("--inherit-agent with --template");
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict, "{error}");
    }

    /// Spec: `farhelm spawn` needs `--inherit-agent` or at least one launch flag, which clap reports as a usage error
    /// before anything is dialed; any number of launch flags may come together; `--conversation` alone is not a
    /// launch, and `--inherit-agent` conflicts with it as with every launch flag.
    ///
    /// Why: the same rule used to be checked after parsing, which made a missing launch exit 1 with a message of its
    /// own instead of clap's usage error. `--conversation` used to sit in the list of launch flags only so that
    /// `--inherit-agent` conflicted with it; it is no launch, so it must neither satisfy the requirement nor lose
    /// that conflict.
    #[farhelm_testtrace::test]
    fn spawn_requires_inherit_agent_or_a_launch_flag() {
        use clap::error::ErrorKind;
        for (argv, kind) in [
            (&["--cwd", "/w"][..], ErrorKind::MissingRequiredArgument),
            (
                &["--cwd", "/w", "--conversation", "c"][..],
                ErrorKind::MissingRequiredArgument,
            ),
            (
                &["--cwd", "/w", "--inherit-agent", "--conversation", "c"][..],
                ErrorKind::ArgumentConflict,
            ),
            (
                &["--cwd", "/w", "--inherit-agent", "--yolo"][..],
                ErrorKind::ArgumentConflict,
            ),
        ] {
            let mut full = vec!["farhelm", "spawn"];
            full.extend_from_slice(argv);
            let error = Cli::try_parse_from(full.iter().copied())
                .err()
                .unwrap_or_else(|| panic!("{full:?} parsed"));
            assert_eq!(error.kind(), kind, "{full:?}: {error}");
        }
        for argv in [
            &["--cwd", "/w", "--inherit-agent"][..],
            &[
                "--cwd",
                "/w",
                "--inherit-agent",
                "--title",
                "t",
                "--parent",
                "p",
            ][..],
            &["--template", "a"][..],
            &["--agent", "codex", "--model", "m", "--prompt", "go"][..],
            &["--command", "sh", "--no-yolo", "--conversation", "c"][..],
        ] {
            let mut full = vec!["farhelm", "spawn"];
            full.extend_from_slice(argv);
            Cli::try_parse_from(full.iter().copied())
                .unwrap_or_else(|error| panic!("{full:?}: {error}"));
        }
    }

    /// Spec: `--key` takes one of the ten digits and `--title-kind` one of `base` and `fixed`, refused otherwise as an
    /// invalid value whose message lists the choices; the parsed values are the digit and the shared name kind.
    ///
    /// Why: clap's possible values are what make the refusal and `--help` list the choices, so a caller can correct
    /// the value from the error alone, and the parsed kind is what keeps an edit's omission distinct from Base.
    #[farhelm_testtrace::test]
    fn template_keys_and_title_kinds_are_closed_choices() {
        use clap::error::ErrorKind;
        use farhelm_proto::launcher::TemplateNameKind;
        for (flag, value) in [("--key", "12"), ("--key", "a"), ("--title-kind", "other")] {
            let error =
                Cli::try_parse_from(["farhelm", "agent", "template", "create", "t", flag, value])
                    .err()
                    .unwrap_or_else(|| panic!("{flag} {value} parsed"));
            assert_eq!(error.kind(), ErrorKind::InvalidValue, "{error}");
            let message = error.to_string();
            assert!(message.contains("possible values"), "{message}");
        }
        for (key, kind, expected_kind) in [
            ("0", "base", TemplateNameKind::Base),
            ("9", "fixed", TemplateNameKind::Fixed),
        ] {
            let Cmd::Agent {
                command:
                    AgentCmd::Template(TemplateCmd::Edit {
                        key: parsed,
                        fields,
                        ..
                    }),
            } = Cli::try_parse_from([
                "farhelm",
                "agent",
                "template",
                "edit",
                "t",
                "--key",
                key,
                "--title-kind",
                kind,
            ])
            .unwrap()
            .command
            else {
                panic!("template edit")
            };
            assert_eq!(parsed.as_deref(), Some(key));
            assert_eq!(fields.fields().name_kind, Some(expected_kind));
        }
    }

    /// Spec: `farhelm internal agent-screen` refuses a relative `--run-dir` or `--cwd`, and `stop-terminal-hosts` a
    /// relative or empty `--state-dir`, as a usage error at parse time (a relative one naming the value); absolute
    /// paths parse.
    ///
    /// Why: these commands act on what the path names, and a relative or empty one (a caller's unset variable, say)
    /// would act on whatever the working directory holds. One shared parser keeps the rule and its message the same
    /// for all three.
    #[farhelm_testtrace::test]
    fn internal_paths_must_be_absolute_at_parse_time() {
        use clap::error::ErrorKind;
        let screen = |run_dir: &str, cwd: &str| {
            Cli::try_parse_from([
                "farhelm",
                "internal",
                "agent-screen",
                "--run-dir",
                run_dir,
                "--cwd",
                cwd,
                "--size",
                "80x24",
                "--",
                "sh",
            ])
        };
        let stop = |state_dir: &str| {
            Cli::try_parse_from([
                "farhelm",
                "internal",
                "stop-terminal-hosts",
                "--state-dir",
                state_dir,
            ])
        };
        // An empty value is refused by clap's own path parser, before ours.
        assert_eq!(
            stop("").err().expect("an empty path is refused").kind(),
            ErrorKind::InvalidValue
        );
        for result in [screen("run", "/w"), screen("/tmp/run", "w"), stop("state")] {
            let error = result.err().expect("a relative path is refused");
            assert_eq!(error.kind(), ErrorKind::ValueValidation, "{error}");
            assert!(
                error.to_string().contains("is not an absolute path"),
                "{error}"
            );
        }
        screen("/tmp/run", "/w").unwrap();
        stop("/tmp/state").unwrap();
    }

    /// Spec: only `custom-agent report-conversation` followed by exactly one `--help` or `-h` counts as a help
    /// request; `--id --help`, `--help --id x`, `--help -h` and nothing at all do not.
    ///
    /// Why: every other shape must stay the silent report a custom agent's wrapper relies on (custom-agent contract),
    /// so the carve-out is the exact one-word shape a person types and nothing a wrapper could produce by accident.
    #[farhelm_testtrace::test]
    fn only_a_lone_help_word_asks_report_conversation_for_help() {
        let words = |words: &[&str]| {
            words
                .iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>()
                .into_iter()
        };
        assert!(is_lone_help_request(words(&["--help"])));
        assert!(is_lone_help_request(words(&["-h"])));
        for rest in [
            &[][..],
            &["--id", "--help"][..],
            &["--help", "--id", "x"][..],
            &["--help", "-h"][..],
            &["--id", "x"][..],
            &["help"][..],
        ] {
            assert!(!is_lone_help_request(words(rest)), "{rest:?}");
        }
    }
}
