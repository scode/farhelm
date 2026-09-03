# Flakes

An append-only log of LATENT flakes: tests that pass on the machine they were written on and fail elsewhere, or
sometimes, or only under load, whose cause was not obvious the moment they failed. It exists so that flakiness can be
read as a whole rather than as whatever failed most recently: a scan of this file should show which seams of the test
suites keep producing flakes, at what rate, and whether the fixes hold.

NOTE: This is not a record of every red test. A test that was written, flaked in the same working session, and was fixed
before it left that session is not a latent flake and does not belong here. TODO.md holds the per-test deflake entries
that are still open and the "Systematic deflake" bucket that is meant to retire them as a class; this file keeps the
history either way, including the fixed ones, because a fixed flake that recurs is the most useful thing this file can
show.

Each entry is one paragraph under a dated heading naming the test and its file, without line numbers (they move). Say
what was observed, where (which machine or runner, what load), what the cause turned out or is suspected to be, and the
disposition: fixed in which PR, ignored with what reason, or open. When a fixed flake recurs, add a new dated entry
rather than editing the old one.

## 2026-09-02 — `agent_relay::a_helm_that_dies_mid_upcall_ends_the_request_at_once` (crates/farhelm/tests/e2e)

Fails with `the supervisor never answered the agent request: Elapsed(())`, the peer's 20 s `answer()` budget running
out. Predates the 0.3.0 stack: on a 4-vCPU sandbox running the whole e2e binary at `--test-threads=4` it failed in 2 of
5 runs against main and 3 of 7 against the stack, while 16 runs alone passed on both and every local run on a 6-core
machine passed. A load flake in the helm-death detection racing the request, or in the budget itself; the relay
mechanism is not suspected. Disposition: open (TODO.md); marked `#[ignore]` in the 0.3.0 stack after it blocked release
gates, to be un-ignored when deflaked.

## 2026-09-02 — `terminal_backpressure::a_paused_replay_detaches_relative_to_the_first_pause_despite_pause_spam` (crates/farhelm/tests/e2e)

Failed once on a GitHub-hosted runner in CI's `test` job for a PR that touched nothing on the terminal path, and passed
on the re-run; the panic is in the file's shared wait ("timed out waiting for FLOOD-…"). Predates that PR. Disposition:
open (TODO.md); `#[ignore]` as above.

## 2026-09-02 — `session_lifecycle::input_bytes_survive_verbatim_through_hexecho` (crates/farhelm/tests/e2e)

Under load on a 4-vCPU sandbox, 1 of 7 full-binary runs: the transcript read `ESC[2;1H61 7f 62 …`, every byte present
but the first hex token glued to a cursor-address sequence, which the whitespace tokenizer dropped whole. Cause: the
attach snapshot ends with tmux's synthesized cursor restore, and when the fixture's READY line was captured into the
snapshot the wait for it returned with that tail still queued in front of the live hex output. Which side of the
snapshot READY lands on is scheduling. Disposition: fixed in #333 (the tokenizer strips escape sequences before
splitting). One of four flakes with the same root cause, the attach-snapshot-versus-live seam; see TODO.md's "Systematic
deflake" bucket.

## 2026-09-02 — `session_lifecycle::non_utf8_terminal_output_survives_live_stream` (crates/farhelm/tests/e2e)

Under load, 1 of 5 full-binary runs on main and 1 of 7 on the 0.3.0 stack: `live_bytes.contains(0xff)` false. The
`binary` fixture wrote its 0xff at startup; when it won the race against the test's attach, the byte arrived through the
capture-pane snapshot, where tmux is allowed to canonicalize invalid bytes, instead of the live stream the test makes
its claim about. Disposition: fixed in #333 (the fixture emits on request, the test asks through its own attachment) and
hardened in #339 after the v0.3.0-rc.1 release gate failed twice on the request itself: the fixture waited for a LINE in
the pty's canonical mode, and on the release runner the line never completed; it now reads one byte in raw mode, the way
the hexecho fixture does.

## 2026-09-03 — `hook_identity::farhelm_agent_instructions_off_suppresses_announce_through_the_real_cli` and `wrapper_launch::a_wrapper_profile_receives_the_sessions_directory` (crates/farhelm/tests/e2e)

Both failed the v0.3.0-rc.1 release gate with "the argv line filled the pane and may have wrapped; raise WIDE_COLS" on a
484-column line for a 245-character argv. The argv-width guard exists to catch a wrapped marker line; what it caught was
a snapshot row padded with spaces to the full pane width, the shape the marker line takes when it arrives through the
attach snapshot rather than live. Same seam as the two entries above. Disposition: fixed in #338 (the guard trims
trailing blanks before measuring; a row that really wrapped is full of argv characters to its last column, so the bound
still holds).

## 2026-09-03 — `session_lifecycle::delete_fails_closed_when_a_launch_artifact_cannot_be_removed` (crates/farhelm/tests/e2e)

Failed once in the v0.3.0-rc.2 release gate on a GitHub-hosted runner: the delete that must fail closed succeeded
instead. It passed three rc.1 gate runs on the same runner type, a 4-vCPU sandbox, and locally. The test waits for the
launch shim to consume the session's spec, plants a replacement, makes the launch directory read-only, and expects the
delete to fail on the artifact it cannot remove; under load the shim can plausibly consume the planted spec before the
delete runs, leaving nothing to fail on. Disposition: open (TODO.md); `#[ignore]` as above.

## 2026-09-03 — `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay` (crates/farhelm/tests/e2e)

Failed in the same rc.2 gate run, in the same shared wait its sibling above timed out in the day before
(`timed out waiting for FLOOD-000000; 11619657 bytes seen`). Two members of one file failing at one wait under load
points at the wait's budget rather than either test. Disposition: open (TODO.md); `#[ignore]` as above.

## 2026-09-03 — profiles popup, three cases (e2e/tests/profiles.spec.ts)

`the profiles popup follows its focus and Escape dismissal contract`,
`unknown then transit waits for the pending
focus request`, and
`stale focus-out classifiers cannot clear newer obligations` fail only on a loaded 4-vCPU sandbox running the spec with
the default worker count beside a live helm, supervisor, and both browsers, Chromium only; all three pass locally in
both engines, repeatedly. The first is a product policy under load: when the page cannot learn where focus went within
its settlement budget it declines to close the popup, and the retries added for that case were not enough on that box.
The second is the test for that retry racing the test hooks that drive it. The third is the harness's stubbed feed never
seeing the page's socket within its wait. Disposition: open (TODO.md, with the fingerprints and first steps).

## 2026-09-03 — `a client that stops draining is detached with the stall reason after the full stall interval` (e2e/tests/terminal-flood.spec.ts)

WebKit, same loaded sandbox: the poll "the attachment must cross HIGH_WATER and pause before the stall clock can start"
saw zero pauses in 30 s, so the stall clock never started. The spec and the backpressure code were untouched by the
stack that surfaced it; the load is the difference. Disposition: open (TODO.md).

## 2026-09-03 — `a keyboard-focused selected tab keeps its accent fill instead of the neutral hover tint` (e2e/tests/terminal-tabs.spec.ts)

WebKit, same loaded sandbox: `toHaveCSS` read the hover tint where the accent fill was expected, for 5 s. The pointer
was presumably still over the tab from the click that selected it, so hover won the cascade; whether that is the test's
sequencing or a real precedence bug is unsettled. The tab styles were untouched by the stack that surfaced it.
Disposition: open (TODO.md).

## 2026-09-03 — `replay-stale-mount` and `reattach-lands-at-tail (tab)` (e2e/tests/terminal-replay-rename.spec.ts), NOT a flake

Recorded here because it looked like one: three failures in both engines on the loaded sandbox, all "waiting for … to be
holding its catch-up open". It was a deterministic regression, not a flake: a profiles-popup fix round had made the
browser suite's replay hold one-shot per page, so specs where several terminal islands mount (a tab session; a remount
after navigating away) never held the island the test waited on. Fixed in #340 before the tag. Kept as a reminder that
"fails only under load" is a hypothesis to test, not a diagnosis: this one failed under load first only because the
loaded run was the first run of that spec after the change.
