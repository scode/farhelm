//! Pi's launch-time hook injection: recognizing Pi's interactive command
//! shape and loading Farhelm's reporter extension into it.
//!
//! Pi's typed-locator vocabulary lives in the parent module beside OMP's,
//! since the two share it.

use std::path::Path;

use super::{HookInjection, HookPolicy, effective_program_index, with_launch_environment};

/// Pi's hook decision (the Pi arm of
/// [`super::AgentIntegration::inject_hooks`]): shape first, then the
/// opt-out, then the executable and extension artifact.
pub(crate) fn inject_hooks(mut argv: Vec<String>, policy: &HookPolicy<'_>) -> HookInjection {
    if !pi_interactive_invocation(&argv) {
        return HookInjection::skipped(argv, "Pi invocation is a utility command or is ambiguous");
    }
    if !policy.hooks.allows(farhelm_proto::AgentKind::Pi) {
        return HookInjection::skipped(argv, "disabled by FARHELM_AGENT_HOOKS");
    }
    let (Some(exe), Some(extension)) = (policy.exe, policy.vendor_extension) else {
        return HookInjection::skipped(
            argv,
            if policy.exe.is_none() {
                "farhelm executable path is not utf-8"
            } else {
                "Pi extension artifact is unavailable"
            },
        );
    };
    argv.extend(["-e".to_string(), extension.to_string()]);
    if policy.instructions.announces() {
        argv.extend([
            "--append-system-prompt".to_string(),
            super::INSTRUCTIONS_POINTER.to_string(),
        ]);
    }
    let controls = [format!("{}={exe}", crate::launch::PI_REPORTER_EXE_ENV_VAR)];
    argv = with_launch_environment(argv, &controls);
    HookInjection::hooked_silently(argv)
}

/// Keep Pi's utility commands untouched and treat option values as opaque.
/// Injection is confined to recognizable interactive command shapes.
fn pi_interactive_invocation(argv: &[String]) -> bool {
    let Some(program) = effective_program_index(argv) else {
        return false;
    };
    if Path::new(&argv[program])
        .file_name()
        .and_then(|name| name.to_str())
        != Some("pi")
    {
        return false;
    }
    let args = &argv[program + 1..];
    if args.first().is_some_and(|argument| {
        matches!(
            argument.as_str(),
            "install" | "remove" | "uninstall" | "update" | "list" | "config" | "auth"
        )
    }) {
        return false;
    }
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if matches!(
            argument.as_str(),
            "--help" | "-h" | "--version" | "-v" | "--list-models" | "--export" | "--"
        ) {
            return false;
        }
        if matches!(
            argument.as_str(),
            "--provider"
                | "--model"
                | "--api-key"
                | "--thinking"
                | "--append-system-prompt"
                | "--system-prompt"
                | "--tools"
                | "-t"
                | "--exclude-tools"
                | "-xt"
                | "--session-dir"
                | "--session"
                | "--session-id"
                | "--fork"
                | "--name"
                | "-n"
                | "--models"
                | "--mode"
                | "-e"
                | "--extension"
                | "--skill"
                | "--prompt-template"
                | "--theme"
                | "--use-theme"
                | "--tui-mode"
        ) {
            if index + 1 >= args.len() {
                return false;
            }
            index += 2;
            continue;
        }
        index += 1;
    }
    true
}
