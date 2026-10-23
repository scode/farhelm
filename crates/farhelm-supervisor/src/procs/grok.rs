//! Grok's foreground corridor: which walked process chains attribute a hook
//! report to the session's Grok process, including its owned backend.

use super::*;

/// Recognize the native Grok image from the already captured executable
/// path. Linux appends ` (deleted)` after an in-place upgrade; that suffix
/// does not change which executable the process is running.
pub(super) fn is_grok_image(exe: &[u8]) -> bool {
    let name = basename(exe);
    matches!(
        name.strip_suffix(b" (deleted)").unwrap_or(name),
        b"grok" | b"grok-linux-x86_64"
    )
}

/// Whether a process that is not the recognised emitter may still be Grok:
/// an image basename `grok-*` (another platform's build), or argv[0] basename
/// `grok` (a `grok` link to an executable with an unfamiliar name).
fn looks_like_grok_runtime(link: &ChainLink) -> bool {
    basename(&link.exe).starts_with(b"grok-")
        || link
            .argv
            .as_deref()
            .and_then(|argv| argv.first())
            .is_some_and(|arg| basename(arg) == b"grok")
}

/// The last path component of a captured executable or argument.
fn basename(path: &[u8]) -> &[u8] {
    path.rsplit(|byte| *byte == b'/').next().unwrap_or_default()
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
pub(crate) fn grok_corridor(chain: &[ChainLink]) -> Result<ProcessIdentity, String> {
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
        if index == 0 {
            continue;
        }
        // The pane process is accepted by position (a resident wrapper), but not
        // one that looks like a Grok runtime this file does not recognise: the
        // session's own Grok under another image name would otherwise let a
        // recognised nested Grok below it become the only emitter.
        if index + 1 == chain.len() {
            if looks_like_grok_runtime(link) {
                return Err(
                    "the session pane looks like a Grok runtime that is not the recognised emitter"
                        .to_string(),
                );
            }
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
