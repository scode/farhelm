//! Grok's foreground corridor: which walked process chains attribute a hook
//! report to the session's Grok process, including its owned backend.

use super::*;

/// Attribute a manual Grok hook to the one supported native runtime under
/// the owned pane.
///
/// Grok reuses the same bounded ancestry walk and shell-trampoline corridor
/// as Codex. Its extra proof is argv-specific: the native executable must
/// carry `--no-leader` before any end-of-options boundary, so the hook runs
/// inside the owned runtime rather than a shared backend.
pub(crate) fn foreground_grok_emitter(
    peer: ProcessIdentity,
    pane_pid: u32,
) -> Result<ProcessIdentity, String> {
    let chain = walk_to_pane(peer, pane_pid)?;
    grok_corridor(&chain)
}

/// Recognize the native Grok image from the already captured executable
/// path. Linux appends ` (deleted)` after an in-place upgrade; that suffix
/// does not change which executable the process is running.
pub(super) fn is_grok_image(exe: &[u8]) -> bool {
    let name = exe.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
    matches!(
        name.strip_suffix(b" (deleted)").unwrap_or(name),
        b"grok" | b"grok-linux-x86_64"
    )
}

/// Require the private-leader flag in Grok's option region.
pub(super) fn grok_has_owned_backend(argv: &[Vec<u8>]) -> bool {
    let option_end = argv
        .iter()
        .position(|argument| argument.as_slice() == b"--")
        .unwrap_or(argv.len());
    argv.iter()
        .take(option_end)
        .skip(1)
        .any(|argument| argument.as_slice() == b"--no-leader")
}

/// Grok's restrictive corridor over the shared bounded ancestry walk.
pub(super) fn grok_corridor(chain: &[ChainLink]) -> Result<ProcessIdentity, String> {
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
        if is_grok_image(&link.exe) {
            if emitter.is_some() {
                return Err("a nested Grok process cannot report for the foreground".to_string());
            }
            let argv = link.argv.as_deref().ok_or_else(|| {
                "the native Grok emitter's command line is unavailable".to_string()
            })?;
            if !grok_has_owned_backend(argv) {
                return Err(
                    "the native Grok emitter was not launched with --no-leader before its option boundary"
                        .to_string(),
                );
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
        if link
            .argv
            .as_deref()
            .is_some_and(|argv| is_hook_trampoline(&link.exe, argv))
        {
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
    emitter.ok_or_else(|| "the hook has no attributable Grok executable".to_string())
}
