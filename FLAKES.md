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

## 2026-09-02 — `terminal_backpressure::a_paused_replay_detaches_relative_to_the_first_pause_despite_pause_spam` (crates/farhelm/tests/e2e; renamed `a_paused_flood_detaches_relative_to_the_first_pause_despite_pause_spam` in #357)

Failed once on a GitHub-hosted runner in CI's `test` job for a PR that touched nothing on the terminal path, and passed
on the re-run; the panic is in the file's shared wait ("timed out waiting for FLOOD-…"). Predates that PR. Disposition:
fixed in #357. The wait matched one of the first 100 records, and every failing transcript had already received records
through 799999: the initial prefix of the burst had aged out of tmux history before the attachment started draining, so
no budget could have found it. The flood now starts behind an input gate after the attachment is ready, making that
initial-prefix wait a valid setup oracle again.

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
delete to fail on the artifact it cannot remove; at the time, the suspicion was that under load the shim consumed the
planted spec before the delete ran, leaving nothing to fail on. Disposition: fixed in #355. The race was the test's, and
not with the planted file: it planted `launch/<id>.json`, a name delete had not recognized since per-launch generations
arrived (#37), so the test passed only when it ran ahead of the shim's read-and-unlink of the real `launch/<id>.0.json`;
under load the shim won, delete found nothing to remove, and succeeded. The test now plants at the real path after that
consume.

## 2026-09-03 — `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay` (crates/farhelm/tests/e2e)

Failed in the same rc.2 gate run, in the same shared wait its sibling above timed out in the day before
(`timed out waiting for FLOOD-000000; 11619657 bytes seen`). Two members of one file failing at one wait under load
points at the wait's budget rather than either test. Disposition: fixed in #357, with the entry above; the wait's oracle
was wrong (the initial prefix had aged out), not its budget. The gated start described there restores that prefix as a
loud premise instead of accepting a retained tail.

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

## 2026-09-03 — `session_lifecycle::non_utf8_terminal_output_survives_live_stream` (crates/farhelm/tests/e2e), recurrence

Under load on a 4-vCPU sandbox (a `cargo build` of the workspace looping beside the tests, the attach-boundary deflake's
"before" proof run), 2 of 10 single-test runs against 0.3.0 timed out after 40 s waiting for `BINARY-MARKER`, the marker
the `binary` fixture writes after its one-byte read, and 1 of 3 full-binary runs at `--test-threads=4` failed the same
way. Not the attach-shape seam the two entries above were fixed for: the fixture is in raw mode before it prints READY,
and the 0xff assertion never ran. What the timeout shows is only that the request-and-reply round trip (the test's
`send_input`, the supervisor's `send-keys` exchange, the fixture's read and write, the output's trip back) did not
complete within the budget; the transcript that would localize it was not kept. The input path, the behavior #339's
hardening was aimed at, is the first hypothesis, not an established cause. The load was harsher than a release runner's,
where the build finishes before the tests start, so the measured rate is not directly representative. Then it failed the
same way on a GitHub-hosted runner, in CI's `test` job for a docs-only PR of the same stack (run 33716079614, the only
failure in 337 tests), so the stall is not an artifact of the sandbox's load. Disposition: open (TODO.md).

## 2026-09-03 — `hook_identity::a_hook_outside_a_farhelm_session_does_nothing_silently` (crates/farhelm/tests/e2e)

Same loaded sandbox runs as the entry above: 1 of 3 full-binary runs on 0.3.0 and 1 of 10 loaded `hook_identity::`
module runs on the attach-boundary stack, `write the payload: Broken pipe` from `assert_silent`. The hook under test has
nothing to do and exits without reading its stdin; under load it was gone before the test wrote the payload, so the
write hit a closed pipe and the test panicked on the very behavior it asserts. Disposition: fixed in #353 (the payload
write tolerates a broken pipe; the exit status and captured output still decide).

## 2026-09-03 — `terminal_backpressure::a_deep_pause_ends_correctly_under_either_tmux_flow_control_behavior` and `terminal_backpressure::memory_stays_flat_while_a_viewer_is_stalled` (crates/farhelm/tests/e2e)

Same loaded sandbox, full binary at `--test-threads=4` on 0.3.0: the deep-pause test failed 2 of 3 runs in the file's
shared wait (`timed out waiting for FLOOD-000000; ~12 MB seen, last records [799999, ...]`), the same wait and the same
fingerprint as the two `#[ignore]`d siblings above; the memory test failed 1 of 3. Neither failed in the three loaded
full runs on the attach-boundary stack the same day, so the rate is noisy. A third member at the same wait strengthens
the reading that the wait's budget under load, not any one test, is the thing to look at. Disposition: the deep-pause
test is fixed in #357 with the two entries above (same wait, same cause: the first 100 records had aged out of history);
its gated start now makes the initial-prefix premise deterministic. The memory test's failure was its RSS assertion, not
that wait, and it stays open (TODO.md).

## 2026-09-03 — `session_lifecycle::attach_with_degenerate_size_still_works` (crates/farhelm/tests/e2e)

Same loaded sandbox: 1 of 3 full-binary runs on 0.3.0 (before the locale fix, so alongside four locale-caused failures)
and 1 of 3 on the attach-boundary stack, `timed out waiting for "FAKE-AGENT READY"` after the test's attach at a
degenerate pane size. Never alone. Nothing about the cause is known beyond the fingerprint. Disposition: open (TODO.md).

## 2026-09-03 — `agent_relay::a_helm_that_dies_mid_upcall_ends_the_request_at_once` (crates/farhelm/tests/e2e), diagnosed

Reproduced on a loaded 4-vCPU sandbox with the `#[ignore]` lifted for the run: 1 of 10 four-thread full-binary runs
beside a looping `cargo build`, the usual `Elapsed(())` at the peer's 20 s read. A sandbox-only diagnostic then widened
only that read to 60 s and kept the test's 10 s promptness assertion; on its second loaded run the test received
`ErrorKind::Timeout` where it requires `Unavailable`. That is the supervisor relay's own upcall-answer budget expiring
before the helm connection-loss path had run, so the helm's death was not observed in time under load. The budget and
the oracle are therefore not the flake; the product's detection latency is. Disposition: open (TODO.md, with the
proposed product direction); still `#[ignore]`d.

## 2026-09-03 — `a client that stops draining is detached with the stall reason after the full stall interval` (e2e/tests/terminal-flood.spec.ts), not reproduced

Thirty loaded WebKit runs of the test on a 4-vCPU sandbox (a `cargo build` looping beside Playwright) all passed, and a
temporary timer around the gate-to-first-pause interval read 1.3 to 2.7 s loaded over ten runs, 1.3 s unloaded on WebKit
and 0.9 s on Chromium, against the poll's 30 s budget. The single sighting's Playwright output was not kept.
Disposition: open (TODO.md); no change made, for lack of a reproduction or an evident cause.

## 2026-09-03 — `session_lifecycle::non_utf8_terminal_output_survives_live_stream` (crates/farhelm/tests/e2e), localized

Twenty loaded single runs on a 4-vCPU sandbox: 8 failed, each after the full 40 s wait for `BINARY-MARKER`, with only
`FAKE-AGENT READY` in the transcript. A temporary test-side barrier, a `ListSessions` request queued immediately after
`send_input` (the helm writer keeps frame order and `handle_connection` finishes the input handler, including its tmux
`send-keys` exchange, before reading the next frame), replied in 6 ms on a failing run while `send_input` itself had
queued in 11 µs; the marker still never came. So the stall is not a slow supervisor exchange and not a short budget:
tmux acknowledged the `send-keys` and the raw-mode fixture never produced its reply. Disposition: open (TODO.md, with
the proposed product direction).

## 2026-09-03 — profiles popup, three cases (e2e/tests/profiles.spec.ts), attempted

A loaded before leg on a 4-vCPU sandbox (10 full-spec runs, both engines, a `cargo build` looping beside Playwright)
reproduced two of the three: `unknown then transit waits for the pending focus request` 5/10 Chromium and 1/10 WebKit,
`stale focus-out classifiers cannot clear newer obligations` 2/10 and 2/10; the focus-and-Escape case 0/10. The
attempted fix (an exhausted `Unknown` retried on the next focus event, ordinal-named test hooks, a quiescence wait
before arming holds, a 30 s `stubFeed` socket wait) passed the stale case but made the pending-focus case fail 11/20 on
Chromium, so it was not shipped. Disposition: open (TODO.md, with the attempt's shape and rates).

## 2026-09-03 — `session_rename::a_renamed_title_survives_a_supervisor_restart` (crates/farhelm/tests/e2e)

Loaded 4-vCPU sandbox, full binary at `--test-threads=4` beside a looping `cargo build`: 1 of 3 runs panicked in the
restart helper's own setup assertion in `create_idempotency.rs`, "the replacement must hold the state directory's claim,
or it reconciles nothing and this test would pass for the wrong reason". The replacement supervisor did not hold the
claim when the helper checked, under load. Never seen alone; nothing else known. Disposition: open (TODO.md).

## 2026-09-03 — two more profiles cases (e2e/tests/profiles.spec.ts)

A full browser-suite run on a 4-vCPU sandbox (both engines, no load beside the suite):
`only layout changes after a
profiles opening invalidate its geometry` failed once on Chromium and
`a saved profile is what the next editor sees,
before the re-read lands` once on WebKit, beside the three known profiles
cases; the latter had also failed twice in ten loaded runs earlier that day. Disposition: open (TODO.md).
