//! `farhelm agent instructions`: the agent-facing manual, printed on
//! demand.
//!
//! An agent running inside a Farhelm session learns about `farhelm agent`
//! from one line the identity hook prints at `SessionStart` (see
//! [`crate::hook::POINTER_LINE`]). That line says nothing except "run this
//! command"; everything the agent actually needs to use the CLI lives
//! here, and is paid for in context only by a session that goes and asks.
//! A session where the user never writes `$farhelm ...` pays one line and
//! nothing else, which is the whole reason the split exists.
//!
//! ## Why the verb list is generated
//!
//! The verbs come from [`crate::AgentCmd`]'s own clap definition, walked
//! at runtime — never from a list transcribed here. The failure this
//! avoids is specific and silent: a verb added to the enum but forgotten
//! here is a verb no agent ever discovers, and nothing about the build or
//! the tests would say so. Generating from clap means a new verb, its
//! arguments, and its one-line meaning (the doc comment clap already reads
//! for `--help`) appear in this text the moment the variant exists.
//!
//! The consequence to know about: the `///` doc comment on an `AgentCmd`
//! variant is agent-facing prose, not just help text. Write it as a
//! sentence that stands on its own.

use crate::AgentCmd;
use clap::{Command, Subcommand};

/// Longest the rendered instructions may get, in lines.
///
/// A bound rather than a style preference. This text is read by a language
/// model that has just been told to run the command, so every line of it
/// competes for the same context the user's actual work needs — and text
/// that is expensive to read is text an agent starts skimming. Forty lines
/// is roughly one screen and comfortably more than the current content
/// needs; if a future verb list pushes past it, the answer is to cut prose,
/// not to raise the number.
///
/// Test-only, because the text is a constant this crate writes: there is
/// nothing to check at runtime that a test cannot check at build time, and
/// a runtime bound would have to decide what to DO when it was exceeded.
#[cfg(test)]
const MAX_LINES: usize = 40;

/// Longest the rendered instructions may get, in bytes.
///
/// The second half of the same bound, because [`MAX_LINES`] alone can be
/// satisfied by forty very long lines. Four kibibytes is about a thousand
/// tokens. Test-only, for [`MAX_LINES`]'s reason.
#[cfg(test)]
const MAX_BYTES: usize = 4096;

/// The instructions, ready to print, ending in a newline.
///
/// Built fresh on each call rather than cached: this runs once per process,
/// in a command whose entire job is to print it.
pub fn text() -> String {
    render(&agent_command())
}

/// [`crate::AgentCmd`]'s clap definition, built and ready to introspect.
///
/// Two details are load-bearing and both mirror how `farhelm agent` is
/// actually declared in `main.rs`.
///
/// `Command::build` is what populates the derived state this module reads —
/// argument value ranges in particular are inferred there, not at
/// declaration — so introspecting an unbuilt command reports a boolean flag
/// as if it took a value.
///
/// `disable_help_subcommand` is the same setting the real `agent` command
/// carries, and for the same reason: without it clap synthesizes its own
/// `help` subcommand during the build, which would then appear in this
/// listing as a verb with clap's boilerplate description, alongside
/// farhelm's own [`crate::AgentCmd::Help`]. Keeping the two commands
/// configured alike is what makes this listing a description of the real
/// CLI rather than of a near-copy.
fn agent_command() -> Command {
    let mut agent =
        AgentCmd::augment_subcommands(Command::new("agent").disable_help_subcommand(true));
    agent.build();
    agent
}

/// [`text`], with the verb source passed in.
///
/// Split out so the renderer can be exercised against a synthetic command
/// carrying the two argument shapes today's real verbs don't: a REQUIRED
/// long option and a boolean flag. `rename`/`stop`/`archive` already carry
/// a positional and an optional `--session`, and those shapes are pinned
/// against the real, clap-derived command instead — see the
/// `a_lifecycle_verb_with_arguments_renders_its_real_command_line` test for
/// that literal check. `agent` must already be built (see
/// [`agent_command`]).
fn render(agent: &Command) -> String {
    let mut out = String::new();
    out.push_str(
        "Farhelm supervises coding agents in real terminals: sessions on one or many hosts, all\n\
         of them visible in one UI that the person you are working with is looking at.\n\
         \n\
         When the user writes \"$farhelm ...\" in a message to you, they are asking you to use\n\
         the farhelm agent CLI below and to tell them what it said.\n\
         \n\
         Verbs:\n\
         \n",
    );
    for line in verb_lines(agent) {
        out.push_str("  ");
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str(
        "\n\
         You never have to pass a credential or a host: this session's own credential is\n\
         already in your environment, so the lines above ARE complete command lines as\n\
         written. A session id is different — omitting --session on rename/stop/archive\n\
         targets THIS session, not \"no session needed\". Stopping this session kills its\n\
         agent's process tree but leaves the session listed; archiving instead keeps it,\n\
         listed as archived. A self-stop can kill this command too, if it runs from the\n\
         agent rather than a terminal tab; use --session <SESSION> for a different target.\n\
         \n\
         Each listing is an aligned table on stdout, one row per line, under a header row. A\n\
         \"*\" in the first column marks you — your own session in the sessions listing, the\n\
         host you are running on in the hosts listing. Warnings and errors go to stderr\n\
         instead, so what lands on stdout is only ever the answer.\n\
         \n\
         The answers come from the helm currently attached to this session — not necessarily\n\
         the machine this host is running on, since a helm can reach a session over SSH from\n\
         elsewhere — so they cover the whole fleet: every host and every session the helm\n\
         knows, wherever those are running.\n\
         \n\
         One failure is worth recognizing. \"no helm is attached to this session\" does not\n\
         mean anything is broken: the question travels to whichever helm currently holds this\n\
         session open, and right now no client has it open. Ask the user to open this session\n\
         in the Farhelm UI, then run the command again.\n",
    );
    out
}

/// One line per verb: its complete command line, then what it does.
///
/// Padded into two columns so a reader can scan the meanings. The usage
/// half is built from clap's own view of the verb — name plus arguments —
/// so a verb that grows a `--cwd` shows it here without anyone
/// remembering to come back.
fn verb_lines(agent: &Command) -> Vec<String> {
    let usages: Vec<String> = agent
        .get_subcommands()
        .map(|verb| {
            let mut usage = format!("farhelm agent {}", verb.get_name());
            for arg in verb.get_arguments() {
                // clap synthesizes these onto every subcommand; they are
                // not part of what the verb asks for.
                if matches!(arg.get_id().as_str(), "help" | "version") {
                    continue;
                }
                usage.push(' ');
                usage.push_str(&arg_spelling(arg));
            }
            usage
        })
        .collect();
    let width = usages.iter().map(String::len).max().unwrap_or(0);
    agent
        .get_subcommands()
        .zip(usages)
        .map(|(verb, usage)| {
            // An `about` is the variant's own doc comment. A variant
            // without one renders as a bare usage line rather than a
            // dangling separator — ugly enough to notice in review, which
            // is the point.
            match verb.get_about() {
                Some(about) => format!("{usage:width$}  {about}"),
                None => usage,
            }
        })
        .collect()
}

/// How one argument of a verb is spelled in the instructions.
///
/// Deliberately not clap's own usage rendering: that spells value names in
/// caps, wraps at a terminal width this text has no business knowing, and
/// is tuned for a human scanning a help screen. This is one short token
/// per argument — `--cwd <PATH>`, `<SESSION>`, bracketed when optional —
/// which is what a reader building a command line off the line needs.
///
/// Ordering follows clap's declaration order, which for the derive is the
/// order the fields appear in the variant. Optional flags therefore sit
/// wherever the enum put them; nothing here sorts them, because the enum's
/// order is the one a maintainer chose.
fn arg_spelling(arg: &clap::Arg) -> String {
    let value = arg
        .get_value_names()
        .and_then(|names| names.first())
        .map(|name| format!("<{name}>"))
        .unwrap_or_else(|| format!("<{}>", arg.get_id().as_str().to_uppercase()));
    let takes_value = arg.get_num_args().is_none_or(|range| range.takes_values());
    let core = match (arg.get_long(), takes_value) {
        (Some(long), true) => format!("--{long} {value}"),
        (Some(long), false) => format!("--{long}"),
        (None, _) => value,
    };
    if arg.is_required_set() {
        core
    } else {
        format!("[{core}]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every verb the binary carries appears in the text it prints.
    ///
    /// This is the drift guard the whole module is shaped around. An agent
    /// only ever learns a verb exists by reading this text, so a verb
    /// missing from it is a feature nothing in the fleet can reach — and
    /// the failure is silent, because the verb still works perfectly for
    /// anyone who already knew its name. Enumerating through clap rather
    /// than through a literal list is what makes the assertion survive a
    /// new variant.
    #[test]
    fn every_agent_verb_appears_in_the_instructions() {
        let text = text();
        let agent = AgentCmd::augment_subcommands(Command::new("agent"));
        let verbs: Vec<String> = agent
            .get_subcommands()
            .map(|verb| verb.get_name().to_string())
            .collect();
        assert!(
            !verbs.is_empty(),
            "the fixture is only meaningful if clap reports verbs at all"
        );
        for verb in verbs {
            assert!(
                text.contains(&format!("farhelm agent {verb}")),
                "the instructions never mention `farhelm agent {verb}`:\n{text}"
            );
        }
    }

    /// The instructions stay inside their size bound.
    ///
    /// The bound is a context budget, not tidiness — see [`MAX_LINES`].
    /// Both halves are checked because either one alone is trivially
    /// satisfiable while the text balloons in the other dimension.
    #[test]
    fn the_instructions_stay_within_their_size_bound() {
        let text = text();
        let lines = text.lines().count();
        assert!(lines <= MAX_LINES, "the instructions grew to {lines} lines");
        assert!(
            text.len() <= MAX_BYTES,
            "the instructions grew to {} bytes",
            text.len()
        );
    }

    /// The text says the three things an agent cannot work out for itself.
    ///
    /// A model that reads these instructions has to leave with the
    /// `$farhelm` trigger, the `*` marker convention, and the one failure
    /// that has a remedy rather than a cause. Each is knowledge that
    /// exists nowhere else the agent can see — the trigger is a convention
    /// between the user and farhelm, the marker is a column with no
    /// header, and "no helm is attached" reads like a broken install until
    /// someone explains that it is not. Losing any of them in an edit is
    /// exactly the sort of thing a prose rewrite does without noticing.
    #[test]
    fn the_instructions_carry_the_conventions_nothing_else_teaches() {
        let text = text();
        for needle in ["$farhelm", "\"*\"", "no helm is attached to this session"] {
            assert!(
                text.contains(needle),
                "the instructions no longer mention {needle:?}:\n{text}"
            );
        }
    }

    /// Output ends in exactly one newline, so the caller can print it
    /// verbatim.
    ///
    /// The command prints this with `print!`, not `println!`: a text that
    /// owns its own trailing newline is one that cannot grow a blank line
    /// at the bottom when someone switches the two.
    #[test]
    fn the_text_ends_in_a_single_newline() {
        let text = text();
        assert!(text.ends_with('\n'));
        assert!(!text.ends_with("\n\n"));
    }

    /// A verb carrying arguments renders them, required and optional
    /// spelled differently.
    ///
    /// Today's real verbs (`rename`, `stop`, `archive`) only ever carry a
    /// positional and an optional `--session` — see
    /// [`a_lifecycle_verb_with_arguments_renders_its_real_command_line`]
    /// for the literal pin on those. This synthetic command exercises the
    /// two shapes none of them do: a REQUIRED long option and a boolean
    /// flag, standing in for the shape a still-hypothetical future verb
    /// (`clone`) would need. Without this test the required-option and
    /// boolean-flag branches of [`arg_spelling`] would ship unexercised
    /// until such a verb landed, at which point a bug here shows up as an
    /// instruction line telling an agent to run the wrong command.
    #[test]
    fn a_verb_with_arguments_renders_its_command_line() {
        // Built, and with clap's own help subcommand disabled, for the
        // reasons [`agent_command`] gives: an unbuilt command reports no
        // value range for a boolean flag, and a built one grows a `help`
        // verb that is not farhelm's.
        let mut agent = Command::new("agent")
            .disable_help_subcommand(true)
            .subcommand(
                Command::new("clone")
                    .about("Clone this session onto another host.")
                    .arg(clap::Arg::new("session").required(true))
                    .arg(
                        clap::Arg::new("host")
                            .long("host")
                            .value_name("NAME")
                            .required(true),
                    )
                    .arg(clap::Arg::new("cwd").long("cwd").value_name("PATH"))
                    .arg(
                        clap::Arg::new("detach")
                            .long("detach")
                            .action(clap::ArgAction::SetTrue),
                    ),
            );
        agent.build();
        let lines = verb_lines(&agent);
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0],
            "farhelm agent clone <SESSION> --host <NAME> [--cwd <PATH>] [--detach]  \
             Clone this session onto another host."
        );
    }

    /// The real `rename`/`stop`/`archive` verbs render the exact usage and
    /// meaning an agent reads, pinned byte-for-byte against production's
    /// own `AgentCmd`.
    ///
    /// [`a_verb_with_arguments_renders_its_command_line`] only proves the
    /// renderer's MECHANISM is correct against a synthetic fixture; nothing
    /// before this test independently confirmed that the real,
    /// clap-derived `rename`/`stop`/`archive` variants actually produce
    /// this text rather than, say, `<SESSION>` silently losing its
    /// brackets or a doc comment losing its trailing clause. A literal
    /// string here is what a careless edit to `AgentCmd`'s derive
    /// attributes or doc comments would break, where the synthetic test
    /// above cannot see it at all.
    #[test]
    fn a_lifecycle_verb_with_arguments_renders_its_real_command_line() {
        let lines = verb_lines(&agent_command());
        for expected in [
            "farhelm agent rename <TITLE> [--session <SESSION>]  \
             Rename a session — the asking one by default",
            "farhelm agent stop [--session <SESSION>]            \
             Stop a session's agent process tree — the asking one by default",
            "farhelm agent archive [--session <SESSION>]         \
             Archive a session — the asking one by default",
        ] {
            assert!(
                lines.iter().any(|line| line == expected),
                "expected exactly this rendered line, got:\n{lines:#?}\nwant: {expected:?}"
            );
        }
    }
}
