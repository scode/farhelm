# Version-lock forwarder test for custom-agent report does not witness that the CLI did not start or that forwarding works when unlocked

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

The integration test for running a custom agent's conversation reporter through the app forwarder during an update or
uninstall does not actually prove that the forwarder prevented the CLI from starting, so a regression that bypassed the
lock or failed before locking would still pass.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/version_lock_forwarder.rs:254-295`.

In `crates/farhelm/tests/version_lock_forwarder.rs:254-295`
(`the_forwarder_keeps_a_custom_report_quiet_during_an_exclusive_version_hold`, introduced in
`7164b111339f33f9ec0b296488c38102191f6d9b`), the docstring states that if the exclusive lock blocks forwarding, 'the CLI
never starts and the forwarder itself must preserve the callback's silent success'. However, the test invokes
`&forwarder` with `env_clear().env("PATH", "/usr/bin:/bin")` and no `FARHELM_SESSION_ID`, `FARHELM_SESSION_TOKEN`, or
`FARHELM_SUPERVISOR_SOCK` variables, and asserts only exit code `0`, empty stdout, empty stderr, and
`version_is_held(&program)` (which remains true because `deleter` holds the lock). Because
`farhelm custom-agent report-conversation --id conv` without session environment variables also exits `0` silently
(`crates/farhelm/src/hook.rs:314-337`) without releasing `deleter`'s lock, and because the forwarder installs
`trap 'exit 0' 0` and `exec >/dev/null 2>&1` at the top of the script (`scripts/install.sh:352-357`), the test passes
under two competing failure modes: (1) the forwarder bypassed the version lock and exec'd the CLI anyway, or (2) the
forwarder failed before reaching the lock loop (e.g., failing to locate `Versions` or read `installed`) and exited `0`
via its trap. By contrast, `a_program_whose_version_is_held_exclusively_refuses_to_run` directly above it (lines
180-249) sends a JSON-RPC `ping` that `goose-hook` would answer if started.

How to verify: Inspect `crates/farhelm/tests/version_lock_forwarder.rs:254-295` alongside `scripts/install.sh:352-357`
and `crates/farhelm/src/hook.rs:314-337`.

Fix: Pass isolated `FARHELM_SESSION_ID`, `FARHELM_SESSION_TOKEN`, and `FARHELM_SUPERVISOR_SOCK` (pointing to a state
directory under `root.path()`) to the forwarder invocation, and assert that neither a report slot file nor a hook log is
created while `deleter` holds the lock (witnessing that the CLI never ran). Then drop `deleter` and repeat the same
forwarder invocation to assert that the report file and `written conv -` hook log are created when unlocked (witnessing
that the forwarder did not merely fail early before locking).

Reviewer metadata (g47_custom_agents_e2e_a/f2): suggested bucket: other (test quality gap in maintainer integration
test; no direct security or user-data-loss route in shipped code).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g47_custom_agents_e2e_a/f2`.
