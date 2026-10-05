//! Goose's launch-time hook injection: recognizing Goose's interactive
//! command shape and adding (or, on resume, re-enabling or disabling) the
//! persisted `farhelm-reporter` extension.
//!
//! Goose's identity capture otherwise needs no Farhelm-side reader: Goose
//! reports its own durable session id through that extension (see the
//! `AgentKind::Goose` docs), so this file is the whole of Goose-specific
//! launch behavior.

use std::path::Path;

use super::{HookInjection, HookPolicy, effective_program_index, with_launch_environment};

/// Goose's hook decision (the Goose arm of
/// [`super::AgentIntegration::inject_hooks`]).
///
/// Unlike every other kind, Goose checks the invocation's shape BEFORE the
/// `FARHELM_AGENT_HOOKS` opt-out, and a resuming launch is rewritten even
/// with hooks disabled: the reporter extension is persisted in Goose's own
/// session metadata, so a resume has to carry the launch-local control that
/// turns it off, and the `session` subcommand a bare `goose` needs.
pub(crate) fn inject_hooks(mut argv: Vec<String>, policy: &HookPolicy<'_>) -> HookInjection {
    let Some(shape) = goose_launch_shape(&argv) else {
        return HookInjection::skipped(
            argv,
            "Goose invocation is a utility command or is ambiguous",
        );
    };
    if shape.reporter_collision {
        return HookInjection::skipped(argv, "Goose invocation already declares farhelm-reporter");
    }
    let enabled = policy.hooks.allows(farhelm_proto::AgentKind::Goose) && policy.exe.is_some();
    if !enabled && !shape.resuming {
        return HookInjection::skipped(
            argv,
            if policy.exe.is_none() {
                super::EXE_NOT_UTF8_REASON
            } else {
                super::HOOKS_DISABLED_REASON
            },
        );
    }
    let Some(exe) = policy.exe else {
        return HookInjection::skipped(argv, super::EXE_NOT_UTF8_REASON);
    };
    if shape.needs_session_subcommand {
        let program = effective_program_index(&argv)
            .expect("a recognized Goose launch has an effective program");
        argv.insert(program + 1, "session".to_string());
    }
    if enabled && !shape.resuming {
        argv.extend([
            "--with-extension".to_string(),
            REPORTER_EXTENSION.to_string(),
        ]);
    }
    let controls = [
        format!(
            "{}={}",
            crate::launch::GOOSE_REPORTER_ENABLED_ENV_VAR,
            u8::from(enabled)
        ),
        format!(
            "{}={}",
            crate::launch::GOOSE_INSTRUCTIONS_ENV_VAR,
            u8::from(enabled && policy.instructions.announces())
        ),
        format!("{}={exe}", crate::launch::GOOSE_REPORTER_EXE_ENV_VAR),
    ];
    argv = with_launch_environment(argv, &controls);
    // A resume with hooks disabled was rewritten above but is not HOOKED,
    // and it logs nothing: nothing was skipped that the operator did not ask
    // for, and nothing was injected.
    HookInjection {
        argv,
        hooked: enabled,
        log: super::HookLog::Silent,
    }
}

/// Goose's `{farhelm_args}` (the Goose arm of
/// [`super::AgentIntegration::farhelm_args`]).
///
/// A start gets the `farhelm-reporter` extension as an argument. A resume
/// gets no argument: Goose persisted that extension in the session it
/// resumes, so the reporter is switched on or off only through
/// `FARHELM_GOOSE_REPORTER_ENABLED`, which is why both phases carry the
/// reporter's environment even with hooks disabled. That is SPEC.md's one
/// accepted use of the environment to control integration. An agent launch
/// already names Goose's `session` subcommand, so nothing is inserted.
pub(crate) fn farhelm_args(
    phase: super::LaunchPhase,
    policy: &HookPolicy<'_>,
) -> super::FarhelmArgs {
    let Some(exe) = policy.exe else {
        return super::FarhelmArgs::skipped(super::EXE_NOT_UTF8_REASON);
    };
    let enabled = policy.hooks.allows(farhelm_proto::AgentKind::Goose);
    let resuming = phase == super::LaunchPhase::Resume;
    if !enabled && !resuming {
        return super::FarhelmArgs::skipped(super::HOOKS_DISABLED_REASON);
    }
    let args = if enabled && !resuming {
        vec![
            "--with-extension".to_string(),
            REPORTER_EXTENSION.to_string(),
        ]
    } else {
        Vec::new()
    };
    super::FarhelmArgs {
        args,
        env: vec![
            (
                crate::launch::GOOSE_REPORTER_ENABLED_ENV_VAR.to_string(),
                u8::from(enabled).to_string(),
            ),
            (
                crate::launch::GOOSE_INSTRUCTIONS_ENV_VAR.to_string(),
                u8::from(enabled && policy.instructions.announces()).to_string(),
            ),
            (
                crate::launch::GOOSE_REPORTER_EXE_ENV_VAR.to_string(),
                exe.to_string(),
            ),
        ],
        hooked: enabled,
        log: super::HookLog::Silent,
    }
}

/// The `--with-extension` value that declares Farhelm's reporter: an MCP
/// stdio server that runs `farhelm internal goose-hook` through the
/// executable the launch environment names.
pub(crate) const REPORTER_EXTENSION: &str =
    "farhelm-reporter:sh -c 'exec \"${FARHELM_GOOSE_REPORTER_EXE:-farhelm}\" internal goose-hook'";

#[derive(Clone, Copy)]
/// Facts needed to inject once on fresh Goose launches and reuse its saved
/// reporter on resume without overriding a user-supplied reporter of that name.
struct GooseLaunchShape {
    resuming: bool,
    reporter_collision: bool,
    needs_session_subcommand: bool,
}

/// Recognize only Goose's interactive command, including the structured
/// launcher's leading `env NAME=value ...` prefix.
fn goose_launch_shape(argv: &[String]) -> Option<GooseLaunchShape> {
    let program = effective_program_index(argv)?;
    if Path::new(&argv[program]).file_name()?.to_str()? != "goose" {
        return None;
    }
    let args = &argv[program + 1..];
    if !args.is_empty() && args.first().map(String::as_str) != Some("session") {
        return None;
    }
    let mut resuming = false;
    let mut reporter_collision = false;
    let mut index = usize::from(args.first().map(String::as_str) == Some("session"));
    while index < args.len() {
        let argument = &args[index];
        if matches!(
            argument.as_str(),
            "--help" | "-h" | "--version" | "-V" | "--"
        ) {
            return None;
        }
        if matches!(argument.as_str(), "--resume" | "-r" | "--fork" | "--edit") {
            resuming = true;
            index += 1;
            continue;
        }
        let takes_value = matches!(
            argument.as_str(),
            "--name"
                | "-n"
                | "--session-id"
                | "--id"
                | "--path"
                | "--provider"
                | "--model"
                | "--system"
                | "--max-turns"
                | "--with-extension"
                | "--with-builtin"
                | "--with-streamable-http-extension"
                | "--mode"
        );
        if takes_value {
            let value = args.get(index + 1)?;
            if argument == "--with-extension" && value.starts_with("farhelm-reporter:") {
                reporter_collision = true;
            }
            index += 2;
            continue;
        }
        if argument.starts_with("--with-extension=")
            && argument["--with-extension=".len()..].starts_with("farhelm-reporter:")
        {
            reporter_collision = true;
        }
        if !argument.starts_with('-') {
            return None;
        }
        index += 1;
    }
    Some(GooseLaunchShape {
        resuming,
        reporter_collision,
        needs_session_subcommand: args.is_empty(),
    })
}
