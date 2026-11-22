# Delete skips hangup for a live tab it knows only by its pinned child

Reviewed commit: fc2b0b46e6211c7ca4001d05d90221f35ee5bd97

## TLDR

Deleting a non-shell session can forcibly terminate a known terminal tab without giving its shell the tab's graceful
hangup.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/sweep.rs:1198`.

tip session_hosts.rs:494-519 explicitly pins known tab children even when markers cannot be read. Tip
teardown.rs:530-539 combines those roots with the main root and passes WholeSession. On a non-shell main launch,
sweep.rs:374-376,473-495,1198-1207 restricts HUP to processes with readable tab markers. The identified tab remains
eligible for TERM and force. core.rs:13976-13985 and launch.rs:1284-1308 use the tab child for the interactive shell.
SPEC_impl.md:4822-4838 documents absent environment data for Apple platform binaries on modern macOS, where tabs have no
systemd scopes. Delete ends the host only after the process sweep at teardown.rs:541-560. SPEC_impl.md:4713-4716
promises HUP for tab shells. The marker-only condition predates this patch in exact parent
4bd7bf366c56d7552c285e36d4d37fb3c8f54f94 sweep.rs:1201-1210, and persists in the changed branch. Tip sweep.rs:3482-3546
asserts HUP for the marked-tab case and verifies ordinary agent TERM. Tip end-to-end host_tabs.rs:1055-1086 explicitly
proves an unmarked tab root is reaped by Delete, but that fixture ignores HUP and asserts death only. Preserve the live
tab root's role in portable HUP selection. The missing assertion is hangup delivery for a live, pinned, marker-invisible
tab under WholeSession with a non-shell main. Bucket: other for the source-established omission, with no actual history
loss established. Confidence: source-supported for an identifiable tab with invisible markers. The macOS test in
procs.rs:2237-2289 checks marker visibility for a platform binary, not the exit of any particular stock shell. Possible
coverage: lifecycle_unit, lifecycle_e2e.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `stop/n3`.
