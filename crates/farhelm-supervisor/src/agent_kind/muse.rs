//! Muse's per-launch JSON hooks, without touching its configuration or plugins.
//!
//! Muse 1.4.14 accepts the same event handler shape as Claude, inside a JSON
//! `-c hooks=...` override. Its SessionStart waits for the first prompt and feeds
//! stdout to developer context; Stop supplies the current plain session UUID.

use super::*;

impl AgentIntegration for MuseIntegration {
    fn record_header_has_prelude(&self, _first_line: &str) -> bool {
        false
    }

    fn parse_record(&self, _text: &str) -> anyhow::Result<Option<RecordCorrelators>> {
        // Observed same-process reminder children have marked, separate child
        // events. SessionStart and Stop need no transcript or index-file check.
        Ok(None)
    }

    fn farhelm_args(&self, _phase: LaunchPhase, policy: &HookPolicy<'_>) -> FarhelmArgs {
        hook_tail_args(self, AgentKind::Muse, policy)
    }

    fn inject_hooks(&self, argv: Vec<String>, policy: &HookPolicy<'_>) -> HookInjection {
        inject_hook_argv_tail(self, AgentKind::Muse, argv, policy, hook_refusal)
    }

    /// The callbacks carry their report destination because Muse can scrub every Farhelm variable
    /// from hook children. The session token belongs to authenticated requests and stays off this command.
    fn hook_argv(
        &self,
        hook_exe: &str,
        policy: &HookPolicy<'_>,
    ) -> Result<Vec<String>, &'static str> {
        let socket = policy
            .supervisor_socket
            .ok_or("the supervisor socket path is not UTF-8")?;
        let destination = format!(
            " --session-id {} --supervisor-socket {}",
            farhelm_proto::text::shell_quote(policy.session_id),
            farhelm_proto::text::shell_quote(socket),
        );
        // Shell quoting precedes JSON serialization. Only SessionStart carries the instruction pointer and any extra
        // instructions.
        let with_destination = |instructions, extra| {
            hook_command(
                hook_exe,
                instructions,
                extra,
                farhelm_proto::ReportVendor::Muse,
            ) + &destination
        };
        let command = with_destination(policy.instructions, policy.extra_instructions);
        let stop = with_destination(AgentInstructions::Off, None);
        let hooks = serde_json::json!({
            "SessionStart": [{"hooks": [{"type": "command", "command": command, "timeout": 60}]}],
            "Stop": [{"hooks": [{"type": "command", "command": stop, "timeout": 60}]}]
        });
        Ok(vec!["-c".into(), format!("hooks={hooks}")])
    }
}

/// A caller-owned hooks override must remain wholly theirs: Muse rejects clashing
/// config keys. Unrelated settings are harmless. Current structured launches place
/// `{farhelm_args}` explicitly and never parse commands; this is the legacy seam.
fn hook_refusal(argv: &[String]) -> Option<&'static str> {
    let mut elements = argv.iter().peekable();
    while let Some(element) = elements.next() {
        let value = if element == "-c" || element == "--config" {
            elements.peek().map(|value| value.as_str())
        } else {
            element
                .strip_prefix("-c")
                .or_else(|| element.strip_prefix("--config="))
        };
        if value.is_some_and(|value| value.starts_with("hooks=") || value.starts_with("hooks.")) {
            return Some("invocation already configures muse hooks");
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> HookPolicy<'static> {
        HookPolicy {
            hooks: &AgentHooks::All,
            instructions: AgentInstructions::On,
            exe: Some("/opt/farhelm"),
            vendor_extension: None,
            codex_endpoint: None,
            session_id: "session-1",
            supervisor_socket: Some("/state path/with'quote\"and\\slash/supervisor.sock"),
            extra_instructions: None,
        }
    }

    /// Start and resume both attach reporters through an explicit placeholder;
    /// opting out removes both the arguments and the silent-hook tripwire.
    #[farhelm_testtrace::test]
    fn muse_placeholder_arguments_honor_hook_policy_on_start_and_resume() {
        let policy = policy();
        for phase in [LaunchPhase::Start, LaunchPhase::Resume] {
            let active = MuseIntegration.farhelm_args(phase, &policy);
            assert!(active.hooked);
            assert!(active.env.is_empty());
            assert_eq!(
                active.args,
                MuseIntegration.hook_argv("/opt/farhelm", &policy).unwrap()
            );
            let disabled = MuseIntegration.farhelm_args(
                phase,
                &HookPolicy {
                    hooks: &AgentHooks::None,
                    ..policy
                },
            );
            assert!(!disabled.hooked);
            assert!(disabled.args.is_empty());
            assert!(matches!(
                disabled.log,
                HookLog::Skipped(HOOKS_DISABLED_REASON)
            ));
            let unreadable = MuseIntegration.farhelm_args(
                phase,
                &HookPolicy {
                    supervisor_socket: None,
                    ..policy
                },
            );
            assert!(!unreadable.hooked);
            assert!(unreadable.args.is_empty());
            assert!(matches!(
                unreadable.log,
                HookLog::Skipped("the supervisor socket path is not UTF-8")
            ));
        }
    }

    /// Both quoting layers preserve a hostile executable path, while Stop does
    /// not repeat the instruction pointer at the end of every turn.
    #[farhelm_testtrace::test]
    fn muse_hook_argv_roundtrips_json_and_shell_quoting() {
        let exe = "/opt/a path/with'quote\"and\\slash/farhelm";
        for instructions in [AgentInstructions::On, AgentInstructions::Off] {
            let policy = HookPolicy {
                instructions,
                ..policy()
            };
            let args = MuseIntegration.hook_argv(exe, &policy).unwrap();
            assert_eq!(args[0], "-c");
            let hooks: serde_json::Value =
                serde_json::from_str(args[1].strip_prefix("hooks=").unwrap()).unwrap();
            for event in ["SessionStart", "Stop"] {
                let hook = &hooks[event][0]["hooks"][0];
                assert_eq!(hook["timeout"], 60);
                let argv = shell_words::split(hook["command"].as_str().unwrap()).unwrap();
                let mut expected = vec![exe, "internal", "hook", "--vendor", "muse"];
                if event == "SessionStart" && instructions.announces() {
                    expected.push("--announce");
                }
                expected.extend([
                    "--session-id",
                    policy.session_id,
                    "--supervisor-socket",
                    policy.supervisor_socket.unwrap(),
                ]);
                assert_eq!(argv, expected);
            }
        }
    }

    /// Every accepted override spelling protects user hooks; unrelated overrides
    /// still receive Farhelm's reporter. No process environment is consulted.
    #[farhelm_testtrace::test]
    fn muse_legacy_injection_preserves_user_hook_overrides() {
        let policy = policy();
        for words in [
            vec!["muse", "-c", "hooks={}"],
            vec!["muse", "--config", "hooks.SessionStart=[]"],
            vec!["muse", "-chooks={}"],
            vec!["muse", "--config=hooks={}"],
            vec!["muse", "-chooks.SessionStart=[]"],
            vec!["muse", "--config=hooks.SessionStart=[]"],
        ] {
            let argv: Vec<String> = words.into_iter().map(str::to_owned).collect();
            let injected = MuseIntegration.inject_hooks(argv.clone(), &policy);
            assert_eq!(injected.argv, argv);
            assert!(!injected.hooked);
            assert!(matches!(
                injected.log,
                HookLog::Skipped("invocation already configures muse hooks")
            ));
        }
        for words in [
            vec!["muse"],
            vec!["muse", "-c", "model=\"echo\""],
            vec!["muse", "--config=hooks_other={}"],
        ] {
            let argv: Vec<String> = words.into_iter().map(str::to_owned).collect();
            let injected = MuseIntegration.inject_hooks(argv.clone(), &policy);
            assert!(injected.hooked);
            assert_eq!(&injected.argv[..argv.len()], &argv);
            assert_eq!(
                &injected.argv[argv.len()..],
                MuseIntegration.hook_argv("/opt/farhelm", &policy).unwrap()
            );
        }
    }
}
