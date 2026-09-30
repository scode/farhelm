//! Codex's foreground corridor: which walked process chains attribute a
//! hook report to the session's one native Codex process.

use super::*;

/// Attribute a hook connection to the one Codex executable under the owned pane.
///
/// A package-manager wrapper may sit between the pane and native Codex. A nested
/// Codex has two native Codex ancestors instead and cannot report for its parent.
/// This establishes process provenance only; transcript metadata must separately
/// exclude vendor threads sharing the foreground process.
///
/// The walk is the shared [`walk_to_pane`] mechanics; the corridor applied
/// below is Codex's instance of the restrictive rule.
pub(crate) fn foreground_codex_emitter(
    peer: ProcessIdentity,
    pane_pid: u32,
) -> Result<ProcessIdentity, String> {
    let chain = walk_to_pane(peer, pane_pid)?;
    codex_corridor(&chain)
}

/// Codex's instance of the restrictive corridor, over an already-walked
/// chain: the reporter (first link, the kernel-attributed socket peer)
/// must be the supported hook invocation; exactly one native `codex`
/// image is the emitter (a nested native Codex refuses); the pane anchor (last link, the
/// owned foreground) is accepted by position; every other link must be
/// a narrow [`is_hook_trampoline`] trampoline. Any other session-hosting
/// runtime or unclassified intermediary refuses.
/// This deliberately restricts surviving launch wrappers above Codex too;
/// only the pane anchor is exempt, not every ancestor of the emitter.
///
/// Pure over the chain so the shapes are unit-testable without live
/// processes; [`foreground_codex_emitter`] supplies the walked chain.
pub(super) fn codex_corridor(chain: &[ChainLink]) -> Result<ProcessIdentity, String> {
    let reporter = chain
        .first()
        .ok_or_else(|| "the hook ancestry is empty".to_string())?;
    match reporter.argv.as_deref() {
        Some(argv) if is_hook_invocation_argv(argv) => {}
        _ => {
            return Err("the reporting process is not the supported hook invocation".to_string());
        }
    }
    let mut emitter = None;
    for (index, link) in chain.iter().enumerate() {
        if imp::is_codex_image(&link.exe) {
            if emitter.is_some() {
                return Err("a nested Codex process cannot report for the foreground".to_string());
            }
            emitter = Some(ProcessIdentity {
                pid: link.pid,
                start: link.start,
            });
            continue;
        }
        if index == 0 || index + 1 == chain.len() {
            continue;
        }
        let trampoline = link
            .argv
            .as_deref()
            .is_some_and(|argv| is_hook_trampoline(&link.exe, argv));
        if trampoline {
            continue;
        }
        if is_other_session_runtime(&link.exe) {
            return Err(
                "another session-hosting runtime sits between the reporter and the foreground"
                    .to_string(),
            );
        }
        return Err(
            "an unclassified intermediary sits between the reporter and the foreground".to_string(),
        );
    }
    emitter.ok_or_else(|| "the hook has no attributable Codex executable".to_string())
}
