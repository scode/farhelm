# Missing regression tests for Claude shim home guard wiring, pins_approval_cwd, and Antigravity trust independence

Reviewed commit: 6f9918a88100c8d8a69ae471fe12cfde4c7bf76d

## TLDR

Several trust-guard and trust-isolation behaviors introduced or updated in these commits lack end-to-end or complete
unit tests: nothing runs the launch shim in a child process to confirm it skips editing Claude's settings file in the
home directory, `pins_approval_cwd` has no direct unit test, and the trust-switch independence test omits Antigravity.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/launch.rs:2490`.

(Turn-3 revision: removed Turn-1 sub-item claiming `6f9918a8` should be tested for untrusted create surviving a target
reconnect during approval, because `approve_launch` at `crates/farhelm-helm/src/agent_requests.rs:2649, 2703-2710`
intentionally refuses target reconnects while any approval card waits.)

1. Commit `6f9918a8` wired `claude_trust_covers_too_much` into `exec_launch_spec_with_seam`
   (`crates/farhelm-supervisor/src/launch.rs:1226-1234`), but its only test
   (`claude_trust_skips_the_home_directory_and_the_root`, `launch.rs:2490-2505`) calls the pure helper directly and does
   not run the shim in a child process (as `plain_claude_launch_applies_saved_workspace_trust_before_exec` at
   `launch.rs:4654` does) to verify that launching Claude in `HOME` leaves `.claude.json` untouched and prints the
   terminal note.
2. `TrustResolvedLaunch::pins_approval_cwd` (`crates/farhelm-helm/src/workspace_trust.rs:53-58`), added in commit
   `6f9918a8` to gate both cwd pinning and `composition_incarnation` in `create_for_agent`, has no direct unit test in
   `workspace_trust.rs`.
3. `workspace_trust_settings_remain_independent` (`crates/farhelm-helm/src/sessions.rs:3048-3074`) iterates only
   `[Claude, Codex, Muse, Pi]` with `antigravity_trust: false`, never testing `antigravity_trust: true` to confirm it
   cannot leak into another harness. Fix direction: Add a child-process shim test in `launch.rs` exercising
   `spec.claude_workspace_trust = true` in `HOME`, add a unit test for `pins_approval_cwd` in `workspace_trust.rs`, and
   include `LaunchHarness::Antigravity` in `workspace_trust_settings_remain_independent`.

Reviewer metadata (g16-trust-sessions-and-claude-guard/l5): suggested bucket: other (could NOT be in highest: test
coverage gaps in product tests); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as
`g16-trust-sessions-and-claude-guard/l5`.
