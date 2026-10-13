//! Codex's foreground corridor: which walked process chains attribute a
//! hook report to a native Codex process and record its Codex ancestors.

use super::*;

/// The reporting native Codex and the Codex processes above it, nearest first.
/// The corridor proves who reported; admission's durable pin decides whether
/// that process may replace this launch's conversation.
#[derive(Debug)]
pub(crate) struct CodexAttribution {
    pub(crate) emitter: ProcessIdentity,
    pub(crate) ancestors: Vec<ProcessIdentity>,
}

/// Attribute a hook to its innermost native Codex, preserving strict hook-side proof.
/// The reporter must be the supported hook invocation, and every link before
/// the emitter must be a narrow hook trampoline. Everything above the emitter
/// is launch ancestry: resident launchers need not look like hook trampolines.
/// Codex ancestors are retained so admission can correct a child-first pin.
///
/// Pure over the chain so the shapes are unit-testable without live
/// processes; admission supplies the report's recorded chain, anchored at the
/// session's pane.
pub(crate) fn codex_corridor(chain: &[ChainLink]) -> Result<CodexAttribution, String> {
    let reporter = chain
        .first()
        .ok_or_else(|| "the hook ancestry is empty".to_string())?;
    match reporter.argv.as_deref() {
        Some(argv) if is_hook_invocation_argv(argv) => {}
        _ => {
            return Err("the reporting process is not the supported hook invocation".to_string());
        }
    }
    if !chain.iter().any(|link| imp::is_codex_image(&link.exe)) {
        return Err("the hook has no attributable Codex executable".to_string());
    }
    let mut emitter = None;
    let mut ancestors = Vec::new();
    for (index, link) in chain.iter().enumerate() {
        if imp::is_codex_image(&link.exe) {
            let identity = ProcessIdentity {
                pid: link.pid,
                start: link.start,
            };
            if emitter.is_some() {
                ancestors.push(identity);
            } else {
                emitter = Some(identity);
            }
            continue;
        }
        if emitter.is_some() || index == 0 {
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
    emitter
        .map(|emitter| CodexAttribution { emitter, ancestors })
        .ok_or_else(|| "the hook has no attributable Codex executable".to_string())
}
