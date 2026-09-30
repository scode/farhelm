//! Claude Code's foreground corridor: the positional check that a hook
//! report came from the session's foreground Claude process.

use super::*;

/// Attribute a Claude hook to the session's foreground by position: the
/// process that ran the hook must be the owned pane process or its direct
/// child. See [`claude_corridor`] for the rule and why it recognizes no
/// executable.
pub(crate) fn foreground_claude_emitter(
    peer: ProcessIdentity,
    pane_pid: u32,
) -> Result<ProcessIdentity, String> {
    let chain = walk_to_pane(peer, pane_pid)?;
    claude_corridor(&chain)
}

/// Claude's corridor over an already-walked chain: a positional rule, not
/// an image rule. The reporter (first link) must be the supported hook
/// invocation; the links directly above it that are narrow
/// [`is_hook_trampoline`] shells are skipped; the next link is the process
/// that ran the hook (the emitter), and it must be the pane anchor (last
/// link) or the anchor's direct child (second-to-last link).
///
/// The contract it enforces is narrow on purpose: a `claude` started
/// underneath the session's foreground Claude — a shelled-out sub-agent
/// that inherited the session credential and loaded a reporting hook from
/// somewhere — cannot replace the foreground's conversation. Such a child
/// always sits at least two links below the pane, because the foreground's
/// Bash tool runs it through a shell that does not `exec` it. A plain
/// profile makes the pane process Claude itself (the login shell, any
/// `systemd-run --scope`, and the launch shim all `exec`), and a supported
/// one-level wrapper profile makes Claude the pane's direct child, so both
/// keep reporting. Native sub-agents never reach this: they fire no
/// `SessionStart`, and the doorway refuses any report carrying `agent_id`.
///
/// It deliberately recognizes no executable and reads no argv beyond the
/// hook and trampoline shapes. Claude's native binary is named after its
/// version (`.../claude/versions/<version>`) and npm installs run under
/// `node`, so an image rule would have to track install layouts, and the
/// injected `--settings` hook is a vendor detail that may change on its
/// own. The accepted costs, recorded in SPEC_impl.md: a wrapper chain
/// deeper than one level loses hook capture and falls back to the record
/// scan, and a child the foreground spawns WITHOUT an intermediate shell
/// in a wrapperless launch would be admitted (not observed in practice).
///
/// Trampoline skipping is load-bearing, not tidiness: whether the hook's
/// `sh -c` survives as a link or `exec`s the hook depends on the shell,
/// and the position of the emitter must not depend on that.
pub(super) fn claude_corridor(chain: &[ChainLink]) -> Result<ProcessIdentity, String> {
    let reporter = chain
        .first()
        .ok_or_else(|| "the hook ancestry is empty".to_string())?;
    match reporter.argv.as_deref() {
        Some(argv) if is_hook_invocation_argv(argv) => {}
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
                    .is_some_and(|argv| is_hook_trampoline(&chain[index].exe, argv))
        })
        .ok_or_else(|| "the hook reporter is itself the pane process".to_string())?;
    if anchor - emitter > 1 {
        return Err(
            "the hook was run by a process nested below this session's foreground; only the \
             pane process or its direct child may report"
                .to_string(),
        );
    }
    let link = &chain[emitter];
    Ok(ProcessIdentity {
        pid: link.pid,
        start: link.start,
    })
}
