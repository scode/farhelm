## What this was about

Some SSH servers report success even when a command failed. On a Mac running Tailscale SSH, that hid the result that
Farhelm uses to tell a host with no running supervisor from a failed connection. Add stopped before it could offer
installation. A later failure, such as an uploaded binary failing its checksum, could also be mistaken for success.

## Things you should know

Commands for Add, Update and uninstall now report how they ended. A missing or malformed report fails the step and
preserves the host diagnostic. SSH connection failures still keep their own status. When the first check already gets a
valid reply from a running supervisor, Farhelm can use the reply immediately. This works through the regular SSH client,
including when its server is Tailscale SSH. The live supervisor connection already speaks its own protocol, so a server
that loses exit codes can still give a generic error if that connection fails before it starts.

If a remote command dies from a signal, the shell now reports a number such as 137 for SIGKILL. A command's own exit 255
is distinguishable from a failed SSH login or connection.

## Open questions and possible follow-ups

The real Mac Add remains a post-landing check. From a build containing this change, add an Apple-silicon Mac that runs
Tailscale SSH through the normal Hosts panel. Check that a Mac without a supervisor reaches the install offer, and check
connection and disconnect after installation. Tailscale can require interactive reapproval in check mode, and may leave
sessions behind after disconnect. Those behaviors are separate from exit status. The ignored
`remote_macos_setup_update_crash_recovery_and_uninstall` test also accepts an explicitly owned fixture Mac through
`FARHELM_TEST_SSH_DESTINATION` and a staged Darwin payload.

The real Tensorlake gateway check was skipped because a registered SSH key for that disposable test was unavailable.
Tensorlake's command routing and failure handling ran in the local suite with a stand-in. The gateway follow-up is to
run one successful and one failing setup command in a disposable sandbox and confirm the gateway delivers both results.
The plan allows delivery without that check when a key is unavailable.

## Pull request

- Recover remote setup results from command output. [#4617](https://github.com/scode/farhelm/pull/4617/changes)

## Checks

- `cargo nextest run --locked -p farhelm-helm --lib -E 'test(/^provisioning::/)'` initially ran 387 tests at
  `ba4c8d31efaf`. Five fixtures misread the bound of a quoted script in the wrapper and failed to run their planned
  substitutions. The result is retained (recorder `dd491f6c-7603-44ed-8f0a-27316110553a`).
- `cargo nextest run --locked -p farhelm-helm --lib -E 'test(/remote_scripts_have_the_runtime_fallback_and_prepare_parses|conditional_settings_install_preserves_a_concurrent_save|agent_upload_temporary_is_private_before_finishing|failed_remote_upload_removes_partial_temporary|remote_upload_launches_no_other_command_until_transfer_exits|ssh_transport_255_takes_precedence_over_a_recorded_status|tagged_shell_keeps_the_full_stderr_diagnostic_budget|remote_put_fails_when_ssh_hides_the_upload_error|remote_install_rejects_tampering_after_upload/)'`
  passed all nine corrected or targeted cases at `b242f426cf1f` (recorder `574259a6-5384-4c6a-80e7-c36307ca31a4`).
- `cargo nextest run --locked -p farhelm-helm --lib -E 'test(/^provisioning::/)'` then passed all 390 selected tests at
  `b242f426cf1f` (recorder `3a5b8a71-a537-42e0-bb5b-05d4829d2d6b`). No selected tests returned an early runtime skip.
  The real localhost SSH case installed a supervisor, updated it while a session stayed usable, and uninstalled it.
- `cargo clippy --locked -p farhelm-helm --all-targets -- -D warnings` and `python -B scripts/check-test-sleeps.py`
  passed at `b242f426cf1f`. Formatting used `cargo fmt -p farhelm-helm` and
  `dprint fmt SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/host-setup-command-status.md`.
- The stack rebased onto `a75511f5b44d`. Main added terminal grid protocol 86 and compact checkout rendering. The
  provisioning implementation and fixtures are byte-identical to the validated revision. The framed hello and version
  refusal paths retain their contracts. Reused the passing nextest, Clippy and test-delay commands from `b242f426cf1f`.
- At the rebased draft `9f87c63b1e68`, `cargo fmt -p farhelm-helm --check`,
  `dprint check SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/host-setup-command-status.md`, `git diff --check` and
  `python3 releasing/check-changelog.py format` passed on the same tree before the push.
- Skipped a full workspace sweep because the change is confined to the helm's provisioning route. Mac and real
  Tensorlake coverage are described above. No desktop rendering changed.

## Review

The independent Sol high source gate found two issues that are fixed in the change. The status record now has its own
allowance inside the bounded stderr capture, so it cannot displace the host's diagnostic. The wrapper separates a
command's 255 from a real SSH transport 255. A new test checks that a zero-reporting SSH transport cannot hide a failed
upload. The corrected fixtures and all three review cases passed. The commit message was cold-read separately.
