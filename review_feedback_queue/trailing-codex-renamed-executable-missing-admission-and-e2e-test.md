# No admission or e2e regression test exercises resuming Codex from a renamed executable

Reviewed commit: 3f4f312202398d9eb42fe37c9db0f58ff80f6647

## TLDR

The change that enables Restart to resume Codex sessions launched from renamed or versioned executables adds no
admission or end-to-end test using a renamed Codex executable. A future regression that reintroduces a Codex
executable-name check in supervisor admission or the hook/restart flow would not be caught by the test suite.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/codex_identity.rs:20`.

Commit 3f4f312202398d9eb42fe37c9db0f58ff80f6647 (fix: resume codex sessions from renamed executables (#4728)) removes
crates/farhelm-supervisor/src/procs/codex.rs and switches Supervisor::report_codex_conversation
(crates/farhelm-supervisor/src/service/core/vendor/codex.rs:131) to crate::procs::positional_corridor. However, the only
test added for a renamed executable is a_renamed_codex_image_can_emit_the_hook in
crates/farhelm-supervisor/src/procs.rs:3048-3063, which calls the vendor-agnostic positional_corridor directly on four
hand-constructed ChainLink values and never touches report_codex_conversation, admit_hook_report, a live process walk,
or restart_session. In crates/farhelm-supervisor/src/service/core.rs:21376-21401, codex_pin_chain still hardcodes exe
b"/opt/test/codex" and argv [b"codex"] for every Codex pin unit test. In crates/farhelm/tests/e2e/codex_identity.rs, all
four e2e tests (resident_codex_launcher_captures_and_resumes_its_child at line 301,
child_first_codex_capture_corrects_and_resumes_the_parent at line 412,
a_shell_child_can_report_first_until_its_codex_parent_reports at line 518, and
nested_native_codex_reports_cannot_replace_the_foreground_conversation at line 653) continue to launch the fixture
through native_codex_image (lines 20-34), which hard-links/copies the binary strictly to "codex" (lines 559 and 845 even
assert CODEX-KERNEL-EXE:codex). Compare commit b14f3355 (#4725), which added versioned_grok_image
(grok-1.0.92-linux-x86_64) in crates/farhelm/tests/e2e/hook_identity.rs to test the end-to-end versioned-executable fix
for Grok. How to verify: inspect crates/farhelm/tests/e2e/codex_identity.rs and
crates/farhelm-supervisor/src/service/core.rs:21376-21716; no test invokes report_codex_conversation or Restart with a
non-"codex" executable basename. Fix: add a renamed-executable Codex e2e test in
crates/farhelm/tests/e2e/codex_identity.rs (or launch a renamed image such as codex-versioned through a resident wrapper
in an existing/new e2e test) that witnesses kernel exe != "codex", captures the conversation via real hooks, pins it
against a nested child, and verifies Restart resumes the renamed executable's conversation history.

Reviewer metadata (g57_positional_hooks_codex/f1): suggested bucket: other (test coverage gap only; source inspection
confirms production admission at crates/farhelm-supervisor/src/service/core/vendor/codex.rs:131 uses positional_corridor
and no remaining Codex executable-basename check blocks capture or resume, so there is no user-facing security,
data-loss, or UX failure today); confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g57_positional_hooks_codex/f1`.
