//! Positional attribution of a hook to its emitter for agents with no executable identity rule.
//! Admission pins that process; ancestry alone does not establish authority.

use super::*;

/// The two reporter command grammars that can occupy the start of a positional corridor.
#[derive(Clone, Copy)]
pub(crate) enum PositionalReporter {
    BuiltIn,
    Custom,
}

impl PositionalReporter {
    fn is_invocation(self, argv: &[Vec<u8>]) -> bool {
        match self {
            Self::BuiltIn => is_hook_invocation_argv(argv),
            Self::Custom => matches!(argv, [_, group, verb, flag, _]
                if group == b"custom-agent" && verb == b"report-conversation" && flag == b"--id"),
        }
    }

    fn is_trampoline(self, exe: &[u8], argv: &[Vec<u8>]) -> bool {
        match self {
            Self::BuiltIn => is_hook_trampoline(exe, argv),
            Self::Custom => is_custom_trampoline(exe, argv),
        }
    }
}

/// Skip the documented five-argument shell wrapper without treating more shell programs as report emitters.
///
/// The single supported expansion is the final quoted position parameter, supplied as its own argv element.
/// Keep this shape in sync with the contract changelog in
/// `website/src/content/docs/docs/agents/custom-agent-contract.md`.
fn is_custom_trampoline(exe: &[u8], argv: &[Vec<u8>]) -> bool {
    let name = exe.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
    if !matches!(name, b"sh" | b"bash" | b"dash") {
        return false;
    }
    let [_, flag, command, zero, id] = argv else {
        return false;
    };
    if flag != b"-c" || zero != b"_" {
        return false;
    }
    let Ok(id) = std::str::from_utf8(id) else {
        return false;
    };
    if farhelm_proto::conversation::validate_named_conversation(id).is_err() {
        return false;
    }
    let Some(prefix) = std::str::from_utf8(command)
        .ok()
        .and_then(|command| command.strip_suffix(r#" "$1""#))
    else {
        return false;
    };
    !has_unquoted_shell_syntax(prefix)
        && shell_words::split(prefix).is_ok_and(|words| {
            matches!(words.as_slice(), [exe, group, verb, flag]
                if !exe.is_empty() && group == "custom-agent" && verb == "report-conversation" && flag == "--id")
        })
}

/// The emitter and every possible correcting ancestor through the pane anchor.
/// Ancestors are nearest-first and exclude the emitter and hook trampolines
/// below it. These vendors have no image rule, so every ancestor can correct a pin.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PositionalAttribution {
    pub(crate) emitter: ProcessIdentity,
    pub(crate) ancestors: Vec<ProcessIdentity>,
}

/// Attribute an already-anchored chain by the position of the hook invocation.
/// The reporter (first link) must be the supported hook invocation; narrow
/// [`is_hook_trampoline`] shells immediately above it are skipped, and the
/// next link is the process that ran the hook (the emitter). The pane anchor
/// ends that search even if its argv resembles a trampoline.
///
/// Resident launch wrappers may put the emitter at any depth. Every link
/// above it is retained through the anchor so a later report from a parent
/// can correct a child-first pin. A nested child's report has the same
/// positional shape: admission, under the capture claim, must judge its
/// authority against the launch's durable pin before changing capture.
///
/// It deliberately recognizes no executable and reads no argv beyond the
/// hook and trampoline shapes. Claude's native binary is named after its
/// version (`.../claude/versions/<version>`) and npm installs run under
/// `node`, so an image rule would have to track install layouts, and the
/// injected `--settings` hook is a vendor detail that may change on its
/// own. Neither argv nor environment above the emitter identifies Claude.
/// Muse has the same constraint: its launcher execs a versioned binary and
/// may remove that binary on update. The hook's immediate nontrampoline parent
/// is its long-lived Muse process, not a separate per-hook host.
///
/// Trampoline skipping is load-bearing, not tidiness: whether the hook's
/// `sh -c` survives as a link or `exec`s the hook depends on the shell,
/// and the position of the emitter must not depend on that.
pub(crate) fn positional_corridor(
    chain: &[ChainLink],
    grammar: PositionalReporter,
) -> Result<PositionalAttribution, String> {
    let reporter = chain
        .first()
        .ok_or_else(|| "the hook ancestry is empty".to_string())?;
    match reporter.argv.as_deref() {
        Some(argv) if grammar.is_invocation(argv) => {}
        _ => {
            return Err("the reporting process is not the supported hook invocation".to_string());
        }
    }
    let anchor = chain.len() - 1;
    // The pane anchor ends the search by position even if its argv happens
    // to look like a trampoline: it is the owned foreground, whatever it is.
    let emitter = (1..=anchor)
        .find(|&index| {
            index == anchor
                || !chain[index]
                    .argv
                    .as_deref()
                    .is_some_and(|argv| grammar.is_trampoline(&chain[index].exe, argv))
        })
        .ok_or_else(|| "the hook reporter is itself the pane process".to_string())?;
    let link = &chain[emitter];
    Ok(PositionalAttribution {
        emitter: ProcessIdentity {
            pid: link.pid,
            start: link.start,
        },
        ancestors: chain[emitter + 1..]
            .iter()
            .map(|link| ProcessIdentity {
                pid: link.pid,
                start: link.start,
            })
            .collect(),
    })
}
