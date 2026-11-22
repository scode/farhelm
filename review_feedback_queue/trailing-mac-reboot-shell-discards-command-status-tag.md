# Manual remote-Mac reboot test helper discards the command-status tag

Reviewed commit: 72dcafbc581259b7411c16f24995dab9f25f0986

## TLDR

The manual remote-Mac reboot measurement test ignores the new exit-status wrapper when running reboot and post-reboot
SSH checks. Against an SSH server that loses exit codes (such as Tailscale SSH on macOS), a refused reboot command can
look successful and stall the test for up to 30 minutes instead of failing immediately.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/tests/macos_real_tests.rs:908-924`.

In `crates/farhelm-helm/src/provisioning/tests/macos_real_tests.rs:908-924`, `mac_reboot_shell` was updated to
destructure `let (mut command, _) = fixture.backend.remote_command(route, script.into()).await...` and then calls
`capture_child(child, budget, "the reboot observation")` without passing the `Tag` or calling `resolve_remote_status`.
Because `remote_command` now wraps the script with `Tag::wrap`, `mac_reboot_shell` both (1) ignores the tagged exit
status record and reads the raw SSH exit code instead, and (2) omits the tag's stderr-cap allowance and leaves the raw
`farhelm-command-status-v1` line in `stderr`. On a lossy SSH server (e.g. Tailscale SSH on macOS), a failed
`sudo -n shutdown -r now` at line 1128 will report `Some(0)`, pass the `assert!(matches!(result.code, Some(0 | 255)))`
guard at line 1129 instead of failing with `MEASUREMENT_UNAVAILABLE`, and then poll `mac_boot_observation` for the
30-minute budget; similarly, a failed native inspection at line 1039 will enter `Some(0)` and panic with
`malformed reboot observation` rather than `post-reboot native inspection failed; this is not job absence`. Fix
direction: expose `SystemBackend::run_shell` as `pub(super)` (or a `#[cfg(test)]` wrapper) and have `mac_reboot_shell`
call `fixture.backend.run_shell(&fixture.target, script, budget).await.ok()`, which already preserves SSH exit 255 as
`Ok(CommandResult { code: Some(255), .. })` while resolving tagged exit codes and accounting for the record bytes.

Reviewer metadata (g33-remote-supervisor-exit-status/f1): suggested bucket: other (could NOT be in highest; confined to
an ignored maintainer test helper); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g33-remote-supervisor-exit-status/f1`.
