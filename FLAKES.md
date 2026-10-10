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

Each entry has one observation paragraph under a dated heading naming the test and its file, without line numbers (they
move). Say what was observed, where (which machine or runner, what load), what the cause turned out or is suspected to
be, and the disposition: fixed in which PR, ignored with what reason, or open. When a fixed flake recurs, add a new
dated entry rather than editing the old one.

New entries also identify the retained run or release job, tested commit and dirty-tree status, exact selection and
concurrency, tmux version and executable SHA256 (or explicitly unavailable), locale, and relevant `FARHELM_*` variable
names. Use portable runner descriptions and redacted paths/commands; never put hostnames, usernames, credentials, or raw
private manifests here. Distinguish a recorded executable identity from a configured or reported version. The recorder
and retention rules are in `docs/test-run-evidence.md` and AGENTS.md.

Record as much of the following as can be had without a new run, and say "unavailable" rather than guessing. Each item
is something that has been easy to note at the time and hard or impossible to recover later:

- The other tests that failed in the same run, or that this one failed alone. A cluster points at a shared cause; a lone
  failure points at the test.
- How long the failing attempt took next to a typical pass, whether it failed fast on an assertion or ran into a
  timeout, and which step the run's own timestamps show stalling.
- What else was running on the machine: other test runs or builds, load average and memory pressure where available, and
  the tests sharing its slot. "Under load" is often suspected here and rarely measured.
- For a dirty tree, what the uncommitted change was (a diffstat or a one-line description), and the versions of the
  tools involved: compiler, Node, Playwright and browser builds, OS and kernel.
- Each reproduction attempt's command, conditions (alone, under added load, with a stand-in) and result as failures out
  of attempts. "Did not reproduce" is evidence only with its count and conditions.
- Hypotheses ruled out, each with the observation that ruled it out.
- The latest changes to the test, its fixture, and the code it covers, or that there were none recently.
- Earlier entries for the same test, and how often it has passed since it was last seen, when that is known.
- Where the full evidence lives (a retained run on one machine, or a hosted artifact) and whether it expires; copy what
  matters into the entry before it does.
- Values that vary between runs and could matter: ports, process IDs, seeds, test order.

Quote the failure itself when the agent recording the entry has it at hand: the assertion or panic message, the
Playwright error, or a script's `FAIL` lines, copied verbatim from the console, the retained run's output, or a CI log
it can read. Put it in a fenced block right after the observation paragraph, trimmed to the lines that say what failed,
with paths and identities redacted as above; no quoted line may start with two hash signs and a space, which would read
as a new entry. Do not reconstruct a message that was never captured; when the run lost it (an interrupted run, a
truncated log), say so in the paragraph instead. Retained runs are private to the machine that made them and hosted CI
artifacts expire, so without the quote a later reader often cannot tell what actually failed, or whether two
observations are the same failure.

After the paragraph (and the quoted failure, if any), add `Class: <class>` and `Cause: <confidence>` as separate
paragraphs. Classes are `readiness`, `replay-live`, `fixture-premise`, `peer-lifecycle`, `process-interference`,
`budget`, `ambiguous-observable`, `pointer-focus`, `substrate`, `product`, `deterministic-regression`, or `unknown`.
Confidence is `established`, `hypothesis`, or `unknown`; `Cause:` is the last line. These fields apply only to new
entries. Missing historical fields are missing evidence, not an implicit cause classification.

## 2026-09-29 — `a tab list past the island cap is listed in full but only partly attached` (e2e/tests/terminal-tabs.spec.ts)

The deflake sweep's full browser battery failed this test in WebKit (`webkit-terminal-tabs`) in run
`a7cb2f9a-820d-43fe-aa89-02e8a36e5ffd`, and the first of three classification reruns failed the same way in run
`6c0d66ef-a4db-4194-907e-e79def8cb87d`; the other two reruns, `76017a36-c24e-44a2-a14b-6787ec415fe2` and
`957cfe2f-b7eb-4894-9421-f2f9b3a3d15e`, passed. Both failures were the 20s agent-readiness budget expiring with
`mounted=true, open=false, socketMatches=true, revealed=false`: the agent socket never reported open, which is the
pre-retry shape rather than the `open=true, revealed=false` residual TODO.md recorded after the 2026-09-17 retry fix.
Linux x86_64, clean tested commit `3c5996cdddaccc72cc1d83f9374e3153c27e1c6d`, selection `browser suite, both engines`
narrowed by the sweep to this test for the reruns, one browser worker and zero retries, pinned tmux 3.7c executable
SHA256 `75ede1768324817dc386aee550c8e7ca68e98530af54762fcb9df0b46491b071`, locale `C.UTF-8`, ambient `FARHELM_*`
scrubbed with only the recorder-owned `FARHELM_PLAYWRIGHT_POLICY_FILE` and `FARHELM_TEST_TRACE_DIR` supplied. Machine
load during the runs was not recorded. Disposition: fixed by two changes (#1292, #1293). The first mount built all 33
terminals in one task, which held WebKit's main thread for 8–10s, and each terminal's 5s connect watchdog expired inside
that hold and discarded sockets the helm had already accepted. The fake tabs' attaches were refused as not found, which
the browser retried as outages, rebuilding terminals until the page starved. Unfixed, the test failed 4 of 10 WebKit
repetitions (run `a80c1f02-52f3-4744-92ee-d0e25f92fe58`); with both changes it passed 30 of 30 (runs
`cd27799e-55e1-4f11-9403-44385a7ea138` and `a945da61-0981-4c41-b27a-92324e9ec46f`) at load averages of 9–21.

Class: readiness

Cause: established

## 2026-09-25 — composer controls inside the initial viewport (e2e/tests/sidebar.spec.ts)

`composer keeps launch and cancel inside the initial viewport at default and narrow width` failed in Chromium during
interrupted full browser run `946f3bb1-6398-4bbc-9ece-7ab6232be092` on Linux x86_64, then passed unchanged among the
exact failing-test selections in run `51f444c3-d0db-4e33-bd27-32b6d429c3a7`. The full run used clean release commit
`c47e3463ec09121492ecf90f8184c69278d59f37`; the rerun used clean test-fix commit
`2c25bf3ba22719fbe3896b18270d1c0194696379` with unchanged RC application builds. Both used one browser worker and zero
retries, locale `C.UTF-8`, and scrubbed ambient `FARHELM_*`; only recorder-owned `FARHELM_TEST_TRACE_DIR` was supplied.
Tmux executable identity is unavailable because these runs did not record it. The original interruption left no final
assertion report, so at the time the failed geometry condition and cause were unknown. Disposition: ignored on
2026-10-01 as most likely not a failure at all. A later review of the retained run found that it ended by interruption:
its manifest records outcome `interrupted` and recorder exit 130, and the generic recorder mode forwarded SIGINT to the
run and SIGKILLed it two seconds later. Playwright 1.62's list reporter marks a test that was running when the run is
interrupted with the same `✘` and elapsed time as a failure, and prints the "interrupted" summary only after webServer
teardown, which the SIGKILL cut off. This test is the last test line in the console, and no replacement worker started
after it, as one did after every real failure earlier in that log. The test passed in the rerun above and in both full
deflake sweeps on 2026-09-29, and its source is unchanged since. A real fast failure in the same 1.8 s is not ruled out,
but nothing supports it. Browser runs through `record-test-run.py --runner playwright` record per-test outcomes, which
tells an interrupted test from a failed one.

Class: ambiguous-observable

Cause: hypothesis

## 2026-09-25 — `working_copies::tests::reconcile_fails_closed_when_a_stranger_holds_the_destination_and_the_source_is_gone` (crates/farhelm-supervisor/src/working_copies.rs)

The workspace nextest battery failed this identity-mismatch test once on the Linux x86_64 worker, while the exact test
passed immediately afterward in isolation. Retained full run `131912d5-0ee2-4313-a204-38edf6fc942c`; exact rerun
`c44ac1bf-bcce-49b0-8a44-5d9633c5172f`; tested commit `6f238ccab7ed8420f78c896ba1ed207947b03cdd` with the
lifecycle-harness tree dirty. Selection was `workspace Rust targets` versus the exact test, both with four nextest slots
and zero retries, pinned tmux 3.7c executable SHA256 `9a78dcb53a791edaf7de8ba3a9a65544d14c5a88e99bd69d3f1f12b60fc41e11`,
locale `C.UTF-8`, and ambient `FARHELM_*` scrubbed. At the time the cause was unknown, and the entry asked to
investigate the concurrent filesystem premise before changing reconciliation behavior. Disposition: fixed by #1046,
which landed on 2026-09-27, after this run. The test removes the checkout directory and then creates the "stranger" at
the journaled destination with `create_dir_all`, which first creates an intermediate `.archive` directory. On its own,
`.archive` takes the removed checkout's freed inode; but when a concurrent test frees a lower-numbered inode at any
point after the checkout was created, `.archive` takes that one instead and the stranger gets the removed checkout's
inode number. The code at this commit, which identified a directory by device and inode alone, took the stranger for the
already-archived checkout and returned `Ok(MetadataComplete)` instead of `IdentityMismatch`. A 2026-10-01 analysis
reproduced it at `6f238ccab7ed8420f78c896ba1ed207947b03cdd` by running the whole `working_copies` test module rather
than the test alone (exact test 0 of 20; module 5 of 60 in hunt batch `8571e023-cbb0-4b96-87a3-c0c1e3e6c171`, and 3 of
40 with the test instrumented to print identities in batch `52db9b77-7098-4607-9003-d15a35013a9e`); in the instrumented
batch all three failures printed the stranger's identity equal to the recorded one and the result
`Ok(MetadataComplete)`. #1046 added the directory's birth time to its identity, and the same module passed 60 of 60 at
`541cc3dbee0e22e71ccad1b0b396561a6a523899` (batch `52d9f40f-448e-424f-a98d-1f5c7c1a352d`). The residual exposure is two
directories created within one coarse clock tick, or a filesystem that reports no birth time, where identity falls back
to device and inode and this test is as exposed as before; the tick case matters most to
`a_recreated_checkout_directory_is_a_different_object`, which saw inode reuse in 45 of those 60 runs and no failure.

Class: product

Cause: established

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
`unknown then transit waits for the pending focus request`, and
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
claim when the helper checked, under load. Never seen alone; nothing else known. Disposition (2026-09-17): fixed by the
scoped correction the TODO entry prescribed — the helper's claim probe now releases with an explicit unlock instead of a
close. The inherited-description mechanism was re-confirmed for THIS probe with a bare-fork probe (parent holds the
flock, forks without exec, closes its descriptor: the lock stays held until the child's copy closes; an explicit LOCK_UN
releases immediately despite the child's copy) — the same close-waits-for-inherited-copies defect demonstrated for the
teststate sweep fixture in #384. The ownership assertion is retained. Twenty recorded exact repetitions of the rename
test passed after the fix; the original loaded failure was never reproduced, so the fix rests on the confirmed
mechanism, not on a reproduced cure.

## 2026-09-03 — two more profiles cases (e2e/tests/profiles.spec.ts)

A full browser-suite run on a 4-vCPU sandbox (both engines, no load beside the suite):
`only layout changes after a profiles opening invalidate its geometry` failed once on Chromium and
`a saved profile is what the next editor sees, before the re-read lands` once on WebKit, beside the three known profiles
cases; the latter had also failed twice in ten loaded runs earlier that day. Disposition: open (TODO.md).

## 2026-09-03 — `the sidebar app bar shows the helm build and client tooltip` (e2e/tests/sidebar.spec.ts), localized

Seen on `[webkit-sidebar]` during a full four-project sidebar/terminal-multihost run (`--repeat-each=3`) on a 4-vCPU
sandbox for PR #363 (the sidebar locality-glyph change): `Error: route.fulfill: Route is already handled!` inside
`forceBuildSkew` (`helpers/fleet.ts`), which rewrites every `**/api/**` reply's build-stamp header (`route.fetch()` then
`route.fulfill({ response, headers })`). Neither this test nor `fleet.ts` was touched by PR #363. `forceBuildSkew` has
four callers in the target snapshot — three in `feed.spec.ts` and this sidebar test — not the broader set of every test
that reads a build stamp. Reproduced alone:
`--project=webkit-sidebar -g "the sidebar app bar shows the helm build and client tooltip" --repeat-each=20`, single
worker, no other test in play, failed 1 of 20 with the identical error. The shape reads as the route handler racing
itself — WebKit re-issuing or re-dispatching the intercepted request so the handler runs twice concurrently for one
navigation, and the second `fulfill` finds the route already answered. Not chased further: no hypothesis yet for why
WebKit produces the second dispatch, and the other three `forceBuildSkew` callers (`feed.spec.ts`) passed throughout
both runs, so whatever triggers it is rare and not obviously tied to this one test's own body. Disposition: open
(TODO.md).

## 2026-09-03 — `launch_sentinel_error_status::a_planted_malformed_spec_sentinel_classifies_error_with_its_detail` (crates/farhelm/tests/e2e/launch_sentinel_error_status.rs)

Seen during the host-alias feature's finishing-work run on a 4-vCPU sandbox:
`cargo test -- --show-output --test-threads=4` failed this one test (338 passed, 1 failed) with "a consumed sentinel is
deleted once its Error outcome commits durably". The launch-sentinel and supervisor code this test exercises was
untouched by that work. Reproduced alone immediately after —
`cargo test -p farhelm --test e2e launch_sentinel_error_status::a_planted_malformed_spec_sentinel_classifies_error_with_its_detail -- --exact --show-output --test-threads=1`
— and it passed. One isolated pass does not distinguish a test race from a load-triggered product or harness defect, so
no cause is claimed beyond the observed fingerprint: fails under a loaded `--test-threads=4` full-binary run, passes
alone. Not chased further. Disposition: open (TODO.md).

## 2026-09-03 — `the sidebar app bar shows the helm build and client tooltip` (e2e/tests/sidebar.spec.ts), also on Chromium

The identical failure the entry above this one already tracks — `Error: route.fulfill: Route is already handled!` inside
`forceBuildSkew` — recurred during the host-alias feature's finishing-work run, this time on
`chromium-sidebar --repeat-each=3` (1 of 168 executions) rather than the WebKit-only sighting previously logged. Same
test, same helper, same error text and call site (`helpers/fleet.ts:696`); `sidebar.spec.ts` was touched only by adding
two new, unrelated tests at the end of the file in that run. This widens the earlier entry's engine scope from
WebKit-only to both engines, which the next person chasing it should know before assuming it is WebKit-specific.
Disposition: still open (TODO.md); the existing entry's diagnosis stands.

## 2026-09-03 — `tests::sweep_never_reaps_a_held_lock` (crates/farhelm-teststate/src/lib.rs)

One `cargo test` run of the whole workspace at `--test-threads=4` on a 4-vCPU sandbox: `left: []`,
`right: ["/tmp/.tmpHAvtBc/fh-it.live01"]`. That is the test's SECOND assertion — after the test drops its own flock,
`sweep`'s second call is expected to reap the now-genuinely-dead directory (`assert_eq!(outcome.reaped, vec![live])`).
An empty `reaped` list on the left means the sweep FAILED TO REAP a directory whose lock had actually been released, not
that it wrongly reaped one still held — the opposite of what an earlier version of this entry claimed. Never reproduced:
5 solo repetitions and 3 repetitions of the whole `farhelm-teststate` crate under its own `--test-threads=4` all passed;
only the full-workspace run (every crate's test binaries competing for real `/tmp` and process-table activity at once)
has shown it, once. Cause not established: the workspace-only reproduction points at some form of contention specific to
a full-suite run, but nothing in this test's own logic identifies a mechanism, and no cross-test interaction has
actually been demonstrated — a claim to that effect in an earlier version of this entry was speculation, not a finding.
Disposition: open (TODO.md).

## 2026-09-05 — final-client cleanup with an unanswered upcall

`agent_relay::a_helm_that_dies_mid_upcall_ends_the_request_at_once` in `crates/farhelm/tests/e2e/agent_relay.rs`
simulates helm death by dropping its last client handle. An unanswered handler's owner retained a writer sender, so
dropping the client did not close a quiet connection; a later terminal frame could incidentally wake the demultiplexer
and hide the leak. A deterministic quiet-peer unit test failed against the prior implementation with the transport still
open after two seconds. Final-owner cleanup now aborts registered answers and signals both transport halves; the unit
test verifies EOF and handler release. Review found that an already blocked frame write also needed to observe that
signal. A separate gated-writer regression reproduced the two-second failure before that correction. The ten-test relay
module and twenty focused helm-death runs with two CPU-load children passed on a 4-vCPU, 8-GiB sandbox with pinned tmux
3.7c. The helm-death test is no longer ignored. This corrects the client-lifetime failure in the simulated death; it
does not establish scheduler starvation in the supervisor's EOF handler as the historical cause. The other release-gate
flakes remain separate work in TODO.md.

## 2026-09-05 — build-skew interceptor removal

`the sidebar app bar shows the helm build and client tooltip` in `e2e/tests/sidebar.spec.ts` failed on repetition 48 of
50 in WebKit at `route.fulfill`, with `Route is already handled!`. The trace shows the test removing interception while
provisioning fetch handlers were still awaiting responses; their later fulfilments failed before the navigation. This
establishes an interceptor-removal race, rather than the previously suspected duplicate dispatch. Waiting for active
route handlers before removing them fixes that boundary without swallowing errors. Fifty repetitions per engine passed
on a 4-vCPU, 8-GiB sandbox with pinned tmux 3.7c and one browser worker, as did the three shared-helper feed callers in
each engine. Disposition: fixed; removed from TODO.md.

## 2026-09-05 — sweep fixture lock inherited before exec

`tests::sweep_never_reaps_a_held_lock` in `crates/farhelm-teststate/src/lib.rs` reproduced in the first parallel 16-test
crate run on a 4-vCPU, 8-GiB sandbox with pinned tmux 3.7c. Temporary diagnostics showed `live=1` after the fixture
dropped its file, with the mtime already in the past: the sweep found a contended lock, rather than refusing a future
timestamp or failing removal. Concurrent crate tests spawn tmux. Close-on-exec does not prevent a child from briefly
inheriting the parent's open file description, and a pipe-barrier fork probe confirmed that the flock survives the
parent's close until the child exits. The fixture now unlocks explicitly before asserting reaping; the initial held-lock
safety assertions and production sweep are unchanged. Twenty subsequent parallel crate runs passed. Forty earlier
isolated runs, split equally with and without a controlled agent marker, also passed; this sweep does not read process
environments. Earlier entries' claim that Cargo runs separate crate test binaries concurrently was incorrect; the
relevant concurrency is inside this crate's test process. Disposition: fixed; removed from TODO.md.

## 2026-09-05 — profiles focus fixtures race the opening handoff

`the profiles popup follows its focus and Escape dismissal contract`,
`unknown then transit waits for the pending focus request`, and
`stale focus-out classifiers cannot clear newer obligations` in `e2e/tests/profiles.spec.ts` reproduced against the
near-term baseline on a 4-vCPU, 8-GiB sandbox with pinned tmux 3.7c. An event trace showed a reopened popup becoming
visible before opening had placed focus inside it: moving directly from the toggle to an outside control then emitted no
popup focus-out event, leaving the classifier count unchanged. The unknown/transit fixture instead settled as `missing`
after its 250-ms placement deadline, before the test's 400-ms observation; hiding a known target did not produce the
unknown evidence the assertion required. The three scenarios now wait for initial popup focus before injecting events,
and the unknown fixture delays its placement observation beyond the deadline. All original dismissal, Escape, and
stale-result assertions remain. Six initial cases passed across Chromium and WebKit; 120 subsequent repetitions (20 per
case per engine) passed with one browser worker and two CPU-load children. Disposition: the reproduced harness races are
fixed; TODO.md retains the unexplained historical startup/bridge symptoms. This does not establish that all historical
failures shared these causes: the older missing feed-socket symptom did not recur, and the earlier exhausted-Unknown
product diagnosis remains unproven. Review added an explicit `focusSettled === "unknown"` assertion, which passed twenty
repetitions per engine without extra CPU load.

## 2026-09-05 — profile editor focus moves during fixture input

`a saved profile is what the next editor sees, before the re-read lands` in `e2e/tests/profiles.spec.ts` failed on the
tenth diagnostic WebKit repetition on the same near-term baseline and 4-vCPU, 8-GiB sandbox with pinned tmux 3.7c. The
recorded POST and confirmed reply both contained the old invocation and a profile name with `edited-invocation`
appended: the save had faithfully stored what the fixture sent. Opening the editor asynchronously focuses its name
field, which could interrupt Playwright filling the invocation field. The test now waits for that initial focus handoff
before filling the other field; it still blocks catalog reads and verifies that the save reply alone updates the row and
the reopened editor. Twenty corrected repetitions per engine passed without extra CPU load. Disposition: harness
correction; the separate layout-epoch case remains in TODO.md.

## 2026-09-05 — binary reply is flushed but absent from the live attachment

`session_lifecycle::non_utf8_terminal_output_survives_live_stream` in `crates/farhelm/tests/e2e/session_lifecycle.rs`
failed on the fifth exact execution against `d71a87fb`, with READY but no BINARY-MARKER after forty seconds. On a
four-CPU, 8-GiB worker with pinned tmux 3.7c, a temporary fixture recorded both consuming the request byte and flushing
the binary reply in a reproduced loaded failure (sixteen passes, then the seventeenth failed, with two CPU-load
children). Twenty quiet diagnostic runs passed. Keeping the fixture alive after flushing passed twenty loaded runs;
restoring immediate exit with failure-only pane diagnostics also passed twenty, so no failing pane capture was obtained.
The receipt disproves the older missing-input localization for this occurrence and supports investigating the
output/exit handoff, without establishing which layer lost progress. No product change or fixture keep-alive was
shipped. Disposition: remains ignored and open under Difficult deflake in TODO.md, with the next
raw-tmux/forwarder/writer/terminal-end measurements recorded there.

## 2026-09-05 — forced-pause client listing lacks the expected tab

`replay_marker::a_tmux_pause_catch_up_replays_without_a_marker` and
`terminal_backpressure::a_forced_tmux_pause_is_recovered_through_the_real_attachment`,
`terminal_backpressure::a_forced_tmux_pause_recovers_an_alternate_screen_pane`, and
`terminal_backpressure::a_forced_tmux_pause_restores_modes_and_cursor_state`, in
`crates/farhelm/tests/e2e/replay_marker.rs` and `crates/farhelm/tests/e2e/terminal_backpressure.rs`, failed in the
four-thread native run at `aa333815` on a four-CPU, 8-GiB worker with pinned tmux 3.7c. Their shared helper could not
find an output control client even though the printed listing contained `pause-after=5`: the name/flags separator was an
underscore, while the parser splits at a tab. The exact replay-marker case reproduced on untouched `d71a87fb` on a
second worker with the same pin, one test thread, and no extra load. The helper and test bodies are unchanged across
that comparison. This establishes a pre-existing test/substrate compatibility failure before the catch-up assertions,
not a new product regression. Disposition: open under Difficult deflake in TODO.md; inspect delimiter bytes and correct
the helper without weakening its positive output-client discriminator.

## 2026-09-05 — large terminal paste reaches the reply deadline

`an over-one-megabyte message does not drop the terminal socket` in `e2e/tests/terminal-flood.spec.ts` failed in the
combined WebKit run on a four-CPU, 8-GiB Ubuntu worker with pinned tmux 3.7c. The socket-open/drained assertion passed,
but the fifteen-second `echo:after-big-message` wait timed out. Its retained trace shows steadily growing echoed input
and the exact reply arriving at the deadline, with the inner assertion succeeding just after the outer poll timed out.
Twenty fresh-stack exact candidate executions passed; restoring and rebuilding untouched `d71a87fb` on the same worker
reproduced the same reply-wait failure on execution thirteen, after twelve passes, without extra CPU load. This proves
the failure predates the current fixes. Processing 4,097 tmux send-key commands and returning the whole megabyte-scale
terminal buffer leave little assertion margin; the relative costs still need measurement. Disposition: open under
Difficult deflake in TODO.md. No timeout or product behavior was changed, and the failed full WebKit command is not
counted as a clean gate.

## 2026-09-05 — two distinct profiles focus handoffs fail

`an inert sidebar click dismisses the profiles popup` and `a popup-created profile is offered on every host`, in
`e2e/tests/profiles.spec.ts`, failed in Chromium at `6903cf90` on a four-CPU, 8-GiB Ubuntu 24.04 worker with pinned tmux
3.7c and no extra load. The inert-click trace observes body focus, then a late opening handoff returns focus to the
new-profile button and the popup stays mounted. The create-profile trace shows a pending editor handoff redirecting
invocation text into the name; native required-field validation refuses the empty invocation, no POST reaches the route,
and the catalog wait expires. A correctly pinned candidate batch passed inert once and failed profile creation once.
Separate exact baseline batches on untouched `d71a87fb` failed inert on the first attempt and profile creation on the
second, after one pass. The popup production code is unchanged. The inert click exposes a pre-existing product
focus-obligation defect: the late opening request overrides the user's newer outside destination. Preserve the immediate
outside click as regression coverage and make that obligation invalidate or override stale opening focus; waiting past
the handoff would hide the defect. Profile creation is a separate fixture race, for which the existing editor-name-focus
precondition is the next correction to try. Disposition: both remain open under Difficult deflake in TODO.md; no
product, timeout, or retry change was made for these failures in this pass.

## 2026-09-05 — WebKit menu entry loses focus to initial terminal reveal

`opening the actions menu enters it, and Tab leaves it`, in `e2e/tests/sidebar.spec.ts`, failed in the full WebKit run
at `6903cf90` on a four-CPU, 8-GiB Ubuntu 24.04 worker with pinned tmux 3.7c and no extra load. The toggle-focus
assertion passed, but the ArrowDown action snapshot about 23 ms later showed terminal focus and the menu stayed closed.
The trace places this at initial attach/reveal, after `__farhelmTermReady` became true. The test, menu handler, and
`terminal.js` are unchanged from frozen `d71a87fb`; twenty quiet exact baseline executions passed. This therefore
appears pre-existing, but is not baseline-reproduced, and the changed layout may affect its frequency. Disposition: open
under Difficult deflake in TODO.md. Check actual replay-reveal settlement before focusing the toggle, keeping keyboard
entry and Tab exit covered; do not treat terminal readiness alone as proof that focus has settled.

## 2026-09-05 — WebKit stalled-client case detaches before observing a pause

`a client that stops draining is detached with the stall reason after the full stall interval; reattaching afterward replays`,
in `e2e/tests/terminal-flood.spec.ts`, reproduced the historical zero-pause failure in the full WebKit run at `6903cf90`
on a four-CPU, 8-GiB Ubuntu 24.04 worker with pinned tmux 3.7c and no extra load. Pause count stayed zero for thirty
seconds, but the trace already showed the stalled-detach banner about 404 ms after gate send, much earlier than the
supervisor's sixty-second stall interval. The unchanged helm outgoing-channel backstop may have won before the browser
paused; the trace establishes the ordering, not that queue-level cause. Disposition: still open under Difficult deflake
in TODO.md. Add detach-reason and queue receipts to the existing gate/write/replay measurements before repeating the
expensive scenario or changing its budget.

## 2026-09-06 — forced-pause baseline substrate caveat

The 2026-09-05 forced-pause entry in `crates/farhelm/tests/e2e/replay_marker.rs` and
`crates/farhelm/tests/e2e/terminal_backpressure.rs` records real failed observations, but does not establish
deterministic failure on the same executable used by CI. Rechecking
[CI run 34006471792](https://github.com/scode/farhelm/actions/runs/34006471792), test job 101414574943, shows all four
named cases passing at `2069e0c83e8a7775daf68798fff08a85528d5e4a` with four libtest threads and the successful pinned
tmux builder on PATH. The available worker record does not retain resolved executable hashes; the CI log supplies the
pin/build assertion rather than an executable digest or a dirty-tree fingerprint. Locale and ambient FARHELM variable
names were not retained in these receipts either. Those inputs cannot be reconstructed from a reported version alone.
Disposition: the delimiter failure remains open in TODO.md, but the worker substrate and underlying cause are
unverified; obtain a retained command, raw delimiter bytes, and executable identity before treating the worker failure
as a same-substrate baseline. Disposition (2026-09-17): closed as not reproducible on the supported substrate —
hexdumping `list-clients -F "#{client_name}\t#{client_flags}"` from the checkout's verified pinned tmux 3.7c build, with
a control client carrying the supervisor's exact `!no-output,pause-after=5` cutover flags, shows a real 0x09 tab between
name and flags — exactly what `force_tmux_pause`'s `split_once('\t')` parses — so the entry's own condition for an
unambiguous separator was never met. The failing workers' tmux identities were never hashed and cannot be reconstructed;
a recurrence under a recorder run would retain them. No fix claimed.

Class: substrate

Cause: unknown

## 2026-09-07 — `session_lifecycle::attach_with_degenerate_size_still_works` (crates/farhelm/tests/e2e/session_lifecycle.rs)

A full workspace nextest run on a 4-CPU Linux worker failed this case while 2,301 others passed: receipt
`9edbb6f2-61a7-4103-ad96-966b1ae075f4`, clean `c2fcabf53525ec91f5e383de87890c8bdc7d3498`,
`cargo nextest run --locked --workspace --exclude farhelm-desktop`, four global slots and zero retries. The retained
transcript contains the complete READY marker wrapped one character per row; pane diagnostics show a live agent and the
expected 1x1 dimensions. The byte-substring wait mistook replay row boundaries for missing output. This change ignores
CR/LF only when recognizing that marker, retains the independent geometry assertion, and explicitly uses the mid-launch
fixture to preserve the test's original boundary. The run used pinned tmux 3.7c, executable SHA256
`eec88f3db9d844d72f5ff2a13ef73e95f2386945cc7e9e8adca9ba57053ba630`, `LC_CTYPE=C.UTF-8`, with `LANG` and `LC_ALL` unset.
No ambient `FARHELM_*` variables were present; the recorder supplied `FARHELM_TEST_TRACE_DIR`. Disposition: corrected by
the accompanying test change; the earlier failed receipt remains retained.

Class: ambiguous-observable

Cause: established

## 2026-09-08 — profiles opening focus overrides an outside click

The inert-click product defect recorded on 2026-09-05 was reproduced with the controlled companion
`an outside click overrides a delayed opening focus commit` in `e2e/tests/profiles.spec.ts`. Run
`db4afac4-d335-43c4-b64e-24968828bc1f`, at `92090716d559` with only the regression fixture changed, used
`npx playwright test --project chromium-profiles -g 'an outside click overrides a delayed opening focus commit' --workers 1 --retries 0`.
The trace recorded body focus after the trusted click, then opening focus returned to the new-profile button and left
the popup mounted. A browser commit already dispatched before the outside choice had no synchronous veto. The
accompanying popup correction supplies that veto and preserves unresolved dismissal intent across renderer failures. The
final test uses an explicit commit hold and pointer/deadline receipts rather than a fixed delay. Batch
`2ded0438-5af3-45c2-a58d-a95fb650615e` passed all 24 cases across three attempts, selecting
`profiles\.spec\.ts -g 'inert sidebar click|outside click overrides|unresolved outside focus|outside focus recovery survives'`
under Chromium and WebKit, one worker and zero retries. Both runs used a non-root Ubuntu 26.04 container with a six-CPU
quota, a 24 GiB memory limit and no added load; tmux 3.7c executable SHA256 was
`1151ac9d3217afd8c4bc07e54c9fa01d3c71d70357688b6e296094d8ef3deeb3`, `LC_CTYPE=C.UTF-8`, with `LANG` and `LC_ALL` unset.
No ambient `FARHELM_*` variables were present; the recorder supplied `FARHELM_TEST_TRACE_DIR` and, for the strict
repetitions, `FARHELM_PLAYWRIGHT_POLICY_FILE`. Disposition: fixed in #527. The separate profile-creation fixture race
and historical startup/bridge symptoms remain open in TODO.md.

Class: product

Cause: established

## 2026-09-08 — overdue focus fixture assumes a transient attempt count

`an overdue focus commit expires before its side effect` in `e2e/tests/profiles.spec.ts` failed in WebKit in run
`86c17b1c-5f2c-482c-9b44-1f3491996063`, at `92090716d559` with the popup correction in progress: it observed two commit
attempts where the unchanged test expected exactly one. The exact selection was `profiles\.spec\.ts` with
`-g 'focus and Escape|delayed and superseded|overdue focus|focus evaluation errors|unknown then transit|stale focus-out|profile form transitions|busy profile work|synthetic outside|Tab to an outside|Tab leaving|same-turn programmatic|late busy claim|inert sidebar click|outside click overrides|unresolved outside focus'`,
both engines, one worker and zero retries. The unchanged worker permits another Expired attempt before its Rust budget
ends; the fixture's shorter browser deadline cannot establish an exact attempt count. The correction requires at least
one dispatched commit and retains the expiry, Unknown settlement and absence-of-focus assertions. Exact both-engine run
`7f3ea0ee-184c-4463-a25a-e1e0cdbfb7d6` and final related run `70d4e08d-026d-4675-a585-a9d61cfab8f7` passed. The failing
run used a non-root Ubuntu 26.04 container with a six-CPU quota, a 24 GiB memory limit and no added load; tmux 3.7c
executable SHA256 was `1151ac9d3217afd8c4bc07e54c9fa01d3c71d70357688b6e296094d8ef3deeb3`, `LC_CTYPE=C.UTF-8`, with
`LANG` and `LC_ALL` unset. No ambient `FARHELM_*` variables were present; the recorder supplied `FARHELM_TEST_TRACE_DIR`
and `FARHELM_PLAYWRIGHT_POLICY_FILE`. Disposition: fixed in #527; the product retry loop is unchanged.

Class: fixture-premise

Cause: established

## 2026-09-08 — browser gate follow-up on terminal and profile fixtures

Run `03f274cb-017a-40ef-b362-02c693ffa3b9` executed `npx playwright test` on clean
`606396a49c405271947ff201d396dad41cc63b87`: 889 passed, 13 expected skips, 14 failed. It used Chromium and WebKit, one
worker, zero retries, a non-root Ubuntu 26.04 container with six CPUs and 24 GiB, and no added CPU-load process. Pinned
tmux 3.7c SHA256 was `1151ac9d3217afd8c4bc07e54c9fa01d3c71d70357688b6e296094d8ef3deeb3`; `LC_CTYPE=C.UTF-8`, `LANG` and
`LC_ALL` unset. No ambient `FARHELM_*` names were present; the recorder supplied `FARHELM_TEST_TRACE_DIR` and
`FARHELM_PLAYWRIGHT_POLICY_FILE`. The following observations share this substrate and retained failure record. They do
not turn that failed command into a clean gate.

Class: unknown

Cause: unknown

### Raw-byte sentinels depend on the byte dumper

Four cases in `e2e/tests/terminal-keys.spec.ts` failed in each engine: Shift+Enter, plain Enter, Ctrl+Shift+Enter, and
plain Enter after the chord all reached `RAWREADY` but missed the final sentinel. The default `od` was uutils coreutils
0.8.0. Direct pty comparison `c99b4526-07a0-41e1-8449-64a5297a940e` observed no live output from its unbounded
`od -v -An -tx1 -w1` after CR and `z`, while installed GNU od 9.7 emitted both bytes immediately. Selecting GNU od only
in the isolated runner made all ten key cases pass in `25c38eeb-c7f5-4e6e-af1b-50d6934f8db6`, which selected
`terminal-keys.spec.ts profiles.spec.ts -g 'Shift.Enter|plain Enter|Ctrl.Shift.Enter|outside click overrides a delayed|profile edited in another browser'`
on the same clean revision, both engines, one worker and zero retries. No product or fixture source changed. The
full-run failures therefore do not establish lost Farhelm input. Disposition: closed by the per-byte `dd`/`od` pipeline
(`RAW_DUMP_INVOCATION` in `e2e/tests/terminal-keys.spec.ts`): each byte's od input ends at EOF, which forces a live row
out of uutils od as well as GNU od; the sentinel waits and `bytesIn` were made tolerant of od's leading-space variance.
Verified against a real pty pair on GNU od 9.4; uutils od remains verified only by the EOF argument, not by execution.
The TODO entry this disposition pointed at is removed with the fix.

Class: substrate

Cause: established

### Large paste times out on the unchanged baseline too

`an over-one-megabyte message does not drop the terminal socket` and the following
`real backspace erases; real ctrl-c kills the fake agent`, in `e2e/tests/terminal-flood.spec.ts`, failed in both engines
in the full run above. Both backspace cases failed during `resetStack` session-deletion setup: `deleted.ok()` was false,
before either input assertion ran. Narrow candidate run `184477eb-d9b5-490b-bc26-a8f5bbae75b2` selected
`terminal-flood.spec.ts -g 'over-one-megabyte|real backspace'`, both engines, one worker and zero retries: both paste
waits failed and both backspace cases passed. After rebuilding clean baseline
`722a690f4ee06dbf7350519811ddec0bcc3e5413`, run `ee79924b-b525-4f7e-ba9f-f105f3faa1f7` selected
`profiles.spec.ts terminal-flood.spec.ts -g 'outside click overrides a delayed|profile edited in another browser|over-one-megabyte|real backspace'`
with the same policy. Both paste waits failed again; the six other cases passed. Both narrow runs used the substrate
above with GNU od selected. This extends the earlier WebKit paste observation to Chromium and establishes that the paste
failure predates the host-selector stack. The backspace failure's relationship to the preceding paste remains unproven;
a narrow pass does not erase it. Disposition: paste remains in its existing Difficult deflake entry; retain a separate
backspace follow-up and inspect the deletion response and session lifecycle evidence before changing deadlines or
behavior.

Class: unknown

Cause: unknown

### Profile focus fixtures miss their intended boundary

In WebKit, `an outside click overrides a delayed opening focus commit` in `e2e/tests/profiles.spec.ts` failed its
unexpired-commit premise in the full run: the trusted click arrived after the held deadline. Narrow run `25c38eeb` above
failed the release-within-deadline assertion instead. The full run also failed
`a profile edited in another browser reaches this one over the real feed` while opening the popup, before exercising the
profile update: the new-profile button never acquired focus. That case passed narrowly on the candidate; both profile
cases passed once on clean baseline `722a690f` in `ee79924b`. These are non-reproductions, not proof that the failures
predate the stack or that the earlier outside-click product defect returned. The new sidebar layout may affect timing.
Disposition: open in TODO.md; preserve pointer/deadline receipts and popup focus readiness while locating the missing
fixture boundary. Do not remove the assertions or extend the product focus deadline to make the tests pass.

Class: pointer-focus

Cause: unknown

## 2026-09-10 — printable paste delays supervisor control traffic

The previously recorded `an over-one-megabyte message does not drop the terminal socket`, in
`e2e/tests/terminal-flood.spec.ts`, failed again in full browser run `7fd44a19-ce3f-42fb-a3df-410da327634a` on frozen
composer candidate `f0aa71e488d1cb2f55099865b85c23025c106edf`. The command selected the full Chromium/WebKit suite with
one worker and zero retries. An independent tmux receiver probe confirmed the parsing cost: equal 256 KiB inputs took
8.66 seconds with per-byte hex arguments and 0.386 seconds with one literal argument per printable chunk, with both
receiver counts verified. Serial input handling delayed session-list replies enough to retire the helm connection. The
repair in #542 uses escaped, option-terminated literal arguments only for printable ASCII; arbitrary bytes retain the
hex path. The browser fixture now inspects the buffer inside the page and checks the same live socket before and after
the paste. Focused browser run `dd8e52e9-2546-45d7-a8a9-dc5e3c84bc9d` passed all four paste/control-key cases in both
engines; it preceded the option-termination correction. Corrected native run `1f004419-8f6c-4847-9b8c-1e4166d854dd` on
`ee5dcbaa4348fd697a1ef5bab71e3e7ad11bc430` passed the four exact byte-preservation, option-looking-input, chunk-ordering
and reconstruction cases with four nextest slots and zero retries. These local Linux runs overlapped isolated validation
jobs. The recorded tmux 3.7c executable SHA256 was `2981fc785ff7ac6236d1c13ebbf1eb3169fa316ed94169e786d75df44a692bcb`;
`LANG`, `LC_ALL` and `LC_CTYPE` were `C.UTF-8`. Ambient `FARHELM_*` inputs were scrubbed; the recorder supplied
`FARHELM_TEST_TRACE_DIR`. Pure Jujutsu and copied browser fixtures lacked strict Git-root attestation; retained source
identities and executable hashes supplement that gap. Disposition: printable-paste repair in #542; the separate
historical backspace setup failure remains in TODO.md.

Final corrected-input browser shards also passed both oversized-paste and backspace cases in both engines. Their
combined 998-case selection had 978 passes, 14 skips and six unrelated or separately repaired fixture failures; this
focused coverage does not make the broad gate green.

Class: product

Cause: established

## 2026-09-10 — profile focus premises fail again in the browser gate

WebKit run `a612fb9b-803b-4de1-ad55-0be77ebc8699` selected `npx playwright test --shard=3/4 --workers=1 --retries=0`:
256 passed, three skipped and three failed. Two failures in `e2e/tests/profiles.spec.ts` occurred before their intended
behavior: `Tab leaving the document preserves busy dismissal intent` lacked editor-name focus, and
`a profile edited in another browser reaches this one over the real feed` lacked the first client's popup focus. The
third failure was a new composer stub-handshake mistake, repaired separately, and is not a latent flake. The immutable
fixture combined corrected native source `ee5dcbaa4348fd697a1ef5bab71e3e7ad11bc430` with browser sources captured in
`638e83bb71f8cc297e1a5e480117a4a8471f368f`; it did not change during execution. The local Linux runner overlapped other
isolated validation jobs, with one worker per browser shard. It used the same recorded tmux executable, locale and
scrubbed-variable policy as the paste entry above; strict Git-root attestation was unavailable for the copied fixture.
This extends the earlier popup-readiness observations without proving a regression in profile saving or feed delivery.
Disposition: open in TODO.md; preserve the focus assertions and investigate the failed premise before expanding repairs.

Class: pointer-focus

Cause: unknown

## 2026-09-10 — provisioning attach misses a listening supervisor

`provisioning::tests::local_provisioning_and_update_preserve_a_running_session`, in
`crates/farhelm-helm/src/provisioning.rs`, passed exact run `568e8782-afa5-4e2f-9ac3-64dd0c227de4`, then failed the
workspace run `de55db2b-7b22-475a-9465-35212dd8de5a` on recorded Jujutsu snapshot
`cab86367182889c97e15170bf2b69a1638651fcc`. The latter selected workspace Rust targets excluding desktop, four nextest
slots and zero retries, while isolated browser validation also ran. The manager had exhausted its active retry ladder.
Its provisioning-triggered single probe preceded the supervisor's listening log, and the next 45-second reprobe would
fall outside the 30-second attach deadline. These timings and unchanged provisioning/manager control flow support a
startup-race hypothesis; there was no pre-composer reproduction. Both commands used the local Linux substrate, actual
tmux 3.7c SHA256 `2981fc785ff7ac6236d1c13ebbf1eb3169fa316ed94169e786d75df44a692bcb`, and `C.UTF-8` for `LANG`, `LC_ALL`
and `LC_CTYPE`. Ambient `FARHELM_*` inputs were scrubbed and the recorder supplied `FARHELM_TEST_TRACE_DIR`. Strict
Git-root attestation was unavailable. The broad JUnit overwrote the exact run's shared report path; the exact console
and recorder remain, but its original JUnit is missing. Disposition: open in TODO.md; do not weaken attach readiness or
increase its deadline based on the exact pass.

Class: readiness

Cause: hypothesis

## 2026-09-10 — sidebar scrolling passes before teardown exhausts its budget

`the sidebar app bar stays pinned while the session list scrolls`, in `e2e/tests/sidebar.spec.ts`, failed Chromium run
`45efb275-84b9-4aab-9dd2-550fd45d4e7a`. Its trace passed all nine scrolling/geometry expectations, then the last session
DELETE raced request-context disposal when the 60-second test budget expired. The fixture's serial cleanup behavior was
unchanged. The command used an explicit 250-case test list preserving the original first shard minus authentication
rotation, which ran separately, with one worker and zero retries. Its immutable source assembly and local Linux
tmux/locale/variable policy were the same as the profile-focus entry above, with other isolated validation jobs running.
This is evidence about cleanup duration, not a failed sticky-bar assertion or a proven composer regression. Disposition:
open in TODO.md; preserve geometry assertions and establish teardown ownership separately.

Class: budget

Cause: established

## 2026-09-10 — authentication recovery assertions vary between runs

`rotation logs out an open client and drops its feed and terminal sockets`, in `e2e/tests/auth.spec.ts`, failed the
original first browser shard `7653ea10-0f7d-411d-88cc-872ac7d1906b` on an aborted recovery detail read. The shared
credential file is updated only after recovery assertions, so later cases used stale credentials and produced a cascade;
that shard was canceled as invalid evidence. Exact candidate run `498f139d-3aa6-41a3-8f0f-00b10a1a4a62` instead failed a
sidebar-row assertion after a successful detail read. A prior composer build and exact candidate run
`62dbfa15-2692-4217-91af-79c69ec4215f` passed. No authentication source or test was changed. Candidate fixtures used the
immutable source assembly, local Linux substrate, actual tmux hash, locale and scrubbed-variable policy recorded in the
profile-focus entry above. Each exact command selected only this Chromium test, one worker and zero retries, alongside
isolated validation jobs. This establishes intermittent observations, not the cause of recovery failure or pre-composer
provenance. Disposition: closed by the read-timeout split (`send_read`/`READ_TIMEOUT` in
`crates/farhelm-ui/src/api.rs`); the recovery assertions were never weakened, and the stall's location remains
unestablished, but an unanswered read now fails into the retry ladder inside the budget instead of starving it. The
mechanism being unproven, a recurrence under the new deadline reopens this — the `fetch_session` detail read
deliberately keeps the sixty-second deadline (it drains a remote supervisor live), so only the sessions and hosts reads
are covered by the split.

Class: unknown

Cause: unknown

## 2026-09-10 — Chromium profile boundaries fail before the create picker

Run `45efb275-84b9-4aab-9dd2-550fd45d4e7a`, with the exact source assembly, selection and substrate recorded in the
sidebar-scrolling entry above, also failed two existing `e2e/tests/profiles.spec.ts` cases. In
`an outside click overrides a delayed opening focus commit`, the trusted click reached a held commit after its deadline;
the fixture's unexpired-boundary assertion failed. In `a popup-created profile is offered on every host`, the saved
profile was registered successfully, but `closeProfiles` left the popup mounted after its toggle click, before opening
the create picker. The latter differs from the older editor-fill failure despite sharing a test title. Production
`profiles.rs` is unchanged from the preserved host-selector parent. Disposition: both observations remain in TODO.md;
retain deadline, save-settlement and focus receipts without treating a persistent popup as proof of a failed save.

Class: fixture-premise

Cause: unknown

## 2026-09-12 — `a popup-created profile is offered on every host` editor-fill race closed (e2e/tests/profiles.spec.ts)

The editor-focus fixture race recorded on 2026-09-05 never recurred after #417 converted this test to `openNewProfile`,
which waits for the editor's name field to own focus before either field is filled. A confirmation batch
`2fe38e66-080b-4cbe-b4fb-2eac4d0ee3a7` ran the exact test twenty times on clean
`073d0bb907612afaf33c09dc1a199c456a2c9014`, both engines, one worker and zero retries: all forty executions passed.
Focus-event traces are retained by the existing `recordPage` calls in `openNewProfile` and `openProfiles`. The run used
an 18-CPU Ubuntu 24.04 Linux host with pinned tmux 3.7c, executable SHA256
`b58c5c9f6bc31f8a5fa4cfba183b9342b447c3365e0a77a3c21f7ce31a192ce5`, `LANG=C.UTF-8`, ambient `FARHELM_*` names scrubbed
and the recorder supplying `FARHELM_TEST_TRACE_DIR` and `FARHELM_PLAYWRIGHT_POLICY_FILE`. Disposition: closed; the
TODO.md entry is removed. The separate popup-close observation in this same test stays open in TODO.md.

Class: fixture-premise

Cause: established

## 2026-09-12 — scrolling teardown sweeps over lanes under its own step (e2e/tests/sidebar.spec.ts)

`the sidebar app bar stays pinned while the session list scrolls` passed all nine geometry assertions in Chromium run
`45efb275-84b9-4aab-9dd2-550fd45d4e7a`, then exhausted its 60-second budget deleting eighteen fixture sessions one by
one (stop plus DELETE each, thirty-six sequential requests); the timeout then raced request-context disposal on the last
DELETE. `cleanupAll` now sweeps over six lanes with each session's stop-before-delete order preserved, every session
attempted, errors aggregated in creation order, and the sweep wrapped in its own `teardown:` report step so a trace
shows teardown as teardown rather than as a failed geometry assertion. Geometry checks are unchanged. Run `5732a9ee`
passed all four scrolling-fixture tests on both engines (8/8); run `9d6cc14c` passed the new partial-allocation and
bounded-teardown ownership test on both engines; batch `0d1c73f3` passed the app-bar test and the mounted-fixture
ownership test ten times on both engines (40/40). All ran on `61817891` with this fix's own uncommitted sidebar changes,
one worker and zero retries, on an 18-CPU Ubuntu 24.04 Linux host with pinned tmux 3.7c, executable SHA256
`b58c5c9f6bc31f8a5fa4cfba183b9342b447c3365e0a77a3c21f7ce31a192ce5`, `LANG=C.UTF-8`, ambient `FARHELM_*` names scrubbed
and the recorder supplying `FARHELM_TEST_TRACE_DIR` and `FARHELM_PLAYWRIGHT_POLICY_FILE`. Other jobs overlapped on the
same host. Disposition: fixed; the TODO.md entry is removed.

Class: budget

Cause: established

## 2026-09-12 — canceled runs strand detached stacks; the fixture shell now watches its spawner (e2e/start-stack.sh)

Canceled recorder-backed Playwright shards left their `start-stack.sh` shells, helms, and supervisors alive, holding
ports and state locks. The layering is now established: Playwright (pinned 1.62.0) spawns its web server detached into
its own process group and SIGTERMs that group on its graceful paths — but the runner handles SIGINT only, so a SIGTERM,
SIGKILL, or crash of the Playwright leader skips teardown entirely. The recorder owns the leader's group, not the
detached one, so no layer reaped the stack; the live shell kept holding the run lock the sweep would otherwise reap.
`start-stack.sh` now watches its spawner and delivers the same TERM a graceful shutdown would have, converging every
spawner death on the existing trap. Delivery is guarded by a live `ps` identity check on the script pid, because a
background subshell inherits its parent's PPID instead of its own, which a PPID comparison cannot see. Review then found
the watcher was watching the wrong pid on Linux: Playwright launches the command as `/bin/sh -c`, and that shell is
dash, which stays resident — so the script's parent was a dash wrapper in the detached session that survives the
leader's death, while the test's direct-child spawner never modeled that hop. The webServer command now carries an
`exec` prefix so the shell becomes the script (one layering on every platform), and the test's new layering guard pins
the configured command's `exec` prefix plus the shell honoring it before the kill phases assume the direct-child shape.
Run `e8a8e126` passed the spawner-SIGKILL, spawner-SIGTERM, and script-SIGTERM phases on `1a356896` with uncommitted
working-copy changes; a repeat run on 2026-09-12 passed the extended script with the added guard, each kill phase
asserting helm liveness before its kill, with an unrelated tmux server, process, and state lookalike surviving all three
— on an 18-CPU Ubuntu 24.04 Linux host with pinned tmux 3.7c, executable SHA256
`b58c5c9f6bc31f8a5fa4cfba183b9342b447c3365e0a77a3c21f7ce31a192ce5`, `LANG=C.UTF-8`, and ambient `FARHELM_*` names
scrubbed. The ssh control mux still lingers up to its 60s ControlPersist window on every path including the old normal
one; it holds no port or lock and is out of scope. Disposition: fixed; the TODO.md entry is removed.

Class: harness

Cause: established

## 2026-09-12 — provisioning attach dials through the supervisor restart (crates/farhelm-helm)

`provisioning::tests::local_provisioning_and_update_preserve_a_running_session` failed its first update's
`attach-supervisor` step in workspace run `de55db2b` after passing standalone twice: the manager had burned its whole
retry ladder dialing a not-yet-installed supervisor, the update restarted the unit, and the attach step's single
`retry_now` probe landed before the restarted supervisor listened. Worse than a lost race, the nudge consumed the
remaining re-probe hold, so the next dial sat a full 45-second cadence out — past the step's 30-second deadline. This
was a product robustness gap, not a test defect: any slow supervisor restart failed provisioning the same way.
`AttachSupervisor` now retries with a fresh window (`retry_now_with_fresh_window`, the same evidence class as a registry
edit), dialing the ladder through the restart latency; the attach deadline is unchanged, and user retry, probes, and
adopt stay single-probe. One user-visible side effect: the fresh window publishes `Connecting (attempt n)` while it
dials, for up to 60 seconds — 30 past the attach step's own deadline — where the row previously kept reading
Unreachable. SPEC.md pins no backoff display, so this is a note, not a conflict. New virtual-clock tests pin the exact
14-attempt schedule and the mid-ladder recovery (a host back after the immediate dial connects on the next step with no
further dials). Run `7fce9e61` passed all 182 manager and provisioning lib tests including both heavy real-provisioning
legs, batch `549113fc` passed the exact local test five times, and run `27ce0af6-00d4-490f-a6dd-263099add19b` passed the
full 713-test helm lib suite with the added recovery test. All ran on an 18-CPU Ubuntu 24.04 Linux host with pinned tmux
3.7c, executable SHA256 `b58c5c9f6bc31f8a5fa4cfba183b9342b447c3365e0a77a3c21f7ce31a192ce5`, `LANG=C.UTF-8`, and ambient
`FARHELM_*` names scrubbed. Disposition: fixed as a product change out of the Deflake bucket; the TODO.md entry is
removed.

Class: product

Cause: established

## 2026-09-12 — rotation recovery still unreproduced; cascade contained (e2e/tests/auth.spec.ts)

Follow-up to the 2026-09-10 authentication entry. Neither shape reproduced locally: batch `f45de613` passed the exact
rotation test fifteen times on both engines, and batch `55d47716` passed the full auth spec five times on both engines
(a sixth attempt died on setup when another session's stack took the shared port mid-batch — environmental, not a test
outcome). All ran with pinned tmux 3.7c, `LANG=C.UTF-8`, and scrubbed `FARHELM_*` on an 18-CPU Ubuntu 24.04 Linux host.
The code read found the recovery path sound on its face — selection survives the token gate, mount reads fire
unconditionally, the default view's reads are not authoritative for absence, and dropped futures cannot reopen the
prompt — so no mechanism is claimed. What did land: the shared suite credential refresh moved from the test's end to
right after the exchange, so a post-exchange failure (both observed shapes) no longer leaves later tests
unauthenticated; the TODO.md entry is narrowed to the recovery provenance itself. The next failure's retained trace
carries network, DOM, and console. Disposition: closed by the read-timeout split (`send_read`/`READ_TIMEOUT` in
`crates/farhelm-ui/src/api.rs`), the maintainer's fork choice of 2026-09-17; see the 2026-09-10 entry above.

Class: unknown

Cause: unknown

## 2026-09-12 — `profile focus counts delayed evaluations against one deadline` misses its budget on loaded WebKit (e2e/tests/profiles.spec.ts)

WebKit run `897e85bb-47b1-404f-a6cf-5f727048dcfe` (full profiles spec, both engines, one worker, zero retries, recorded
pinned tmux 3.7c executable SHA256 `b58c5c9f6bc31f8a5fa4cfba183b9342b447c3365e0a77a3c21f7ce31a192ce5`, `LANG=C.UTF-8`,
ambient `FARHELM_*` scrubbed): the opening focus commit never landed within the 2 s premise wait, so the 2-attempt/250
ms timing assertions never ran; 119/120 passed. A concurrent full two-engine gate overlapped the run on the same host. A
recorded narrow `--repeat-each=5` webkit loop on the same tree, run `8e961b39-960a-46f8-b289-0ac548710259`, then passed
4/5 with the fourth repeat missing the same opening focus commit (`toBeFocused` failed), so the miss reproduces in a
narrow loop. The test injects 180 ms of evaluation delay against a 250 ms wall-clock budget, leaving about 70 ms for all
WebKit bridge overhead. No path in this test claims the operation lock, so no control is ever disabled and the
concurrent disable-blur repair cannot execute here; the miss reads as load, not as a regression from that change.
Disposition: closed by the 2026-09-17 `FOCUS_SETTLE_MS` raise to 1000 ms (`crates/farhelm-ui/src/profiles.rs`) — the
injected 180 ms plus bridge overhead now fits with wide margin; the test's two-attempt assertion is unchanged, and its
elapsed bound was re-derived from the budget (≤1000 ms) so a loaded renderer that focuses inside the deadline passes
instead of tripping the old 250 ms constant.

Class: budget

Cause: hypothesis

## 2026-09-12 — popup disable-blur dismissal race fixed (e2e/tests/profiles.spec.ts, crates/farhelm-ui/src/app_bar.rs)

Chromium run `45efb275-84b9-4aab-9dd2-550fd45d4e7a` left the popup mounted after the toggle close in
`a popup-created profile is offered on every host` with the profile registered server-side. Batch
`acc5a4cf-2087-4f2a-a4f0-9c2d4450d927` ran the exact test twenty times per engine with the strengthened fixture premise
but the product unmodified, reproducing the underlying event twice (attempts 1-2, Chromium): the popup dismissed between
the save click and the close (form trivially gone, saved row never rendered, failure screenshot shows no popover). The
mechanism is the product's own busy-disabling: the save claims the operation lock, the focused save button disables,
focus falls to `body`, and the popup `focusout` listener reported that blur as an ordinary outside intent. When the
dismissal classifier sampled `body` after the lock released but before completion focus committed, the popup closed and
killed completion focus with it; the later toggle click then reopened the dismissed popup instead of closing it. The
listener now ignores a focus-out whose target is disabled (a control that was already disabled cannot hold focus, so any
such event is a disable-blur), while trusted pointer and Tab provenance report exactly as before. The test pins its
close to the save completion contract (POST receipt, unmounted form, rendered row) and `closeProfiles` asserts its open
premise with lock-state receipts. Batch `39f81e52-2277-4e55-b584-9a15c0d48653` ran the exact test twenty times per
engine with zero failures; run `4627195c-2fb2-472c-8e8c-557400527b2f` passed the whole profiles spec 120/120 on both
engines with pinned tmux 3.7c executable SHA256 `b58c5c9f6bc31f8a5fa4cfba183b9342b447c3365e0a77a3c21f7ce31a192ce5`,
`LANG=C.UTF-8`, ambient `FARHELM_*` scrubbed. A deterministic pin test
(`disabling the focused save control does not dismiss the popup`) now scripts the disable-blur directly: run
`d08fc2fa-9de0-4c9c-9474-fa8fb0c34cd6` fails it against the pre-fix bundle (the popup unmounts inside the observation
window), and batch `23367a9f-dc59-4761-ac1d-3b4ab0c2bee1` passes it twice per engine against the fixed one. Disposition:
fixed in this PR; the TODO.md entry is removed.

Class: product

Cause: established

## 2026-09-13 — `events::tests::an_unanswered_keepalive_releases_the_subscriber_seat` (crates/farhelm-helm/src/events.rs)

The deflake sweep's workspace nextest battery failed this helm subscriber-seat test once: the keepalive assertion
(`the keepalive must be a WebSocket Ping`) reported `left: 1, right: 9`, so the frame in the keepalive slot carried
opcode 1 where a Ping (opcode 9) was expected. All three classification reruns of the exact test passed. Sweep failure
retained run `02cf3518-4309-473e-9ee7-b968451bf0d7`; reruns `3a79edca-26e0-4cf9-9902-81957bfa8824`,
`a9638c60-6170-4a57-8a33-9931ca275832`, `55bcbde4-fc2e-4b00-b7aa-60832bcb3569`. Tested commit `b475c5c6` with a clean
tree. Selection `workspace Rust targets` (`cargo nextest run --workspace --exclude farhelm-desktop`, minus the recorded
exclusions); concurrency `4 nextest slots; retries 0` with `--test-threads 4`, on a Linux x86_64 worker. Pinned tmux
3.7c executable SHA256 `c4d00d1d947c5e64fd7c4eada92b80a2a0230df32f725f8ae26ee6ac9d3a81c2`, `LANG=C.UTF-8`, ambient
`FARHELM_*` scrubbed (only `FARHELM_TEST_TRACE_DIR` present in the test process). Suspected frame interleaving ahead of
the keepalive Ping under full-suite load; not established. Disposition: fixed — see the 2026-09-13 fix entry below; the
TODO.md entry is removed in the fix PR.

Class: peer-lifecycle

Cause: hypothesis

## 2026-09-13 — `events::tests::an_unanswered_keepalive_releases_the_subscriber_seat` fixed (crates/farhelm-helm/src/events.rs)

The interleaving was a genuine revision racing the keepalive, and the test's premise was wrong rather than its subject.
The server resets the idle window on every revision write, so a revision that lands after the handshake read arrives as
the next frame; the test asserted Ping on the first frame unconditionally. The revision the sweep saw came from the
local row's connect ladder walking its attempts under the test's clock (the retained trace shows the phase transitions
mid-test), and a later instrumented run caught a fourth attempt — a ladder restart — landing mid-test the same way. The
test now settles the ladder first, then pins the same event deterministically: a direct `bump()` after the handshake
read, the same revision through the same watch channel, read back as exactly `handshake + 1` — which failed the old
assertion with the sweep's exact signature (`left: 1, right: 9`) in retained run `fca722a5-d691-4ec8-a7f7-94caa1b144f5`.
An adversarial review of that first fix showed it spent the release window reaching the Ping and reintroduced a network
round trip whose loopback IO stalled under the paused clock (retained run `37ebc934-e43f-41cd-aa2c-0558ff5ea6e7`: one
poll attempt, no retry, NOTICE fired). The committed shape instead consumes text revisions across capped intervals, then
reads the seat through the same counter the endpoint gates on — held after the Ping, free within a bounded silence — so
a late ladder restart only costs intervals, never correctness, and the caps fail loud past any sane burst. Fixed test
passed 20/20 recorded repetitions, and the full `events::` module passed 21/21. Tested commit `3d057550` with the
uncommitted fix (the diff is retained in each manifest's porcelain). Selection `keepalive burst-tolerant test`
(`cargo nextest run -p farhelm-helm --lib -E` with the exact test); concurrency `4 nextest slots; retries 0`, no tmux (a
helm unit test), `LANG=C.UTF-8`, ambient `FARHELM_*` scrubbed. Disposition: fixed in this PR; the TODO.md entry and the
`deflake/known-flakes.txt` line are removed.

Class: peer-lifecycle

Cause: established

## 2026-09-13 — `profile CRUD round-trips from the app-bar popup to the helm` (e2e/tests/profiles.spec.ts)

The deflake sweep's browser battery failed this profiles test once on Chromium (`chromium-profiles` project): the 60 s
test timeout fired in the `openProfileEditor` helper (profiles.spec.ts:214), whose click on `.profile-edit` inside the
freshly created profile row never became actionable. All three classification reruns passed. Sweep failure retained run
`fc4285c8-b667-4f2d-86c1-97dfe260aded` (failure screenshot and trace.zip retained under `playwright-artifacts`); reruns
`b04195c1-6ad7-4c2d-a779-924ba0342b8a`, `cebdd484-c94a-4df4-8b0b-61175ba99b00`, `36a54b08-c525-4bdb-9570-d9517a0d8630`.
Tested commit `b475c5c6` with a clean tree. Selection `browser suite, both engines`; concurrency
`one browser worker; retries 0`. Pinned tmux 3.7c executable SHA256
`c4d00d1d947c5e64fd7c4eada92b80a2a0230df32f725f8ae26ee6ac9d3a81c2`, `LANG=C.UTF-8`, only
`FARHELM_PLAYWRIGHT_POLICY_FILE` and `FARHELM_TEST_TRACE_DIR` present in the test process. No attributed cause; the
trace was not opened for this record. Disposition: open (TODO.md).

Class: unknown

Cause: unknown

## 2026-09-14 — `agent_listing_real_stack::an_authenticated_agent_clone_starts_a_structured_successor` (crates/farhelm/tests/e2e/agent_listing_real_stack.rs)

The deflake sweep's workspace nextest battery failed this structured-clone test once: the parent-generation argv
assertion ("the structured parent must reach the owned fake executable") saw an empty observed argv, even though the
retained trace shows the fake's `STRUCTURED-LAUNCH-GENERATION:1` marker and `FAKE-AGENT READY`. Classification reruns of
the exact test passed twice and failed once, the failure at the clone-generation argv assertion ("clone argv lost
model") with an empty argv as well. Sweep failure retained run `bab77953-df15-4df6-88fc-83f4ac2ee18e`; reruns
`f09afa88-3506-482a-b17a-afd08570917d`, `69a2a2b2-c1c8-4f22-9e3e-131d88ac618c`, `23f578e3-bba7-4932-bc13-13809d162fe2`.
Tested commit `f4c3840de3aff5fefb5c4c2ae10483314886fa4f` with a clean tree. Selection `workspace Rust targets`
(`cargo nextest run --workspace --exclude farhelm-desktop`, minus the recorded exclusions); concurrency
`4 nextest slots; retries 0` with `--test-threads 4`, on a Linux x86_64 worker. Pinned tmux 3.7c executable SHA256
`c4d00d1d947c5e64fd7c4eada92b80a2a0230df32f725f8ae26ee6ac9d3a81c2`, `LANG=C.UTF-8`, ambient `FARHELM_*` scrubbed (only
`FARHELM_TEST_TRACE_DIR` present in the test process). Suspected observation race: the generation-argv wait returned
empty before the marker arrived, at two different generations, rather than either launch losing the model; not
established. Disposition: open (TODO.md).

Class: readiness

Cause: hypothesis

## 2026-09-14 — `agent_listing_real_stack::an_authenticated_agent_clone_starts_a_structured_successor` fixed (crates/farhelm/tests/e2e/structured_launches.rs)

The open entry above guessed an observation race; the mechanism turned out to be two defects in the generation-argv
decoder, both in how it reads the attach replay. The replay carries terminal rows, not the wrapper's byte stream: rows
arrive with cursor-positioning escapes and padding, and a snapshot taken mid-render holds a payload-less argv prefix
row. First, the decoder accepted the first (stale) prefix row, returning an empty argv for a healthy launch — the
sweep's empty-argv failures. Second, the hex join stopped at any non-hex line, so a payload row wearing a positioning
escape (`\x1b[2;28H` ahead of the hex, as retained) was invisible and the wait sat pending until its deadline even
though the failure transcript carries the full boundary. That second shape reproduced 1/20 and 5/20 in isolation hunts
(batches `562ec3f5-21ef-45cd-a0d4-244a82685e46`, `785d243b-457a-4245-a8ca-9fdafb39c864`) with the generation-2 timeout
signature, always at the clone observation. The decoder now normalizes rows through the harness's own
`normalize_pane_text` before parsing, resolves to the most recent witness (matching the generation marker's own
last-occurrence rule), joins the payload across the replay's not-yet-rendered rows, and treats an empty decode as a
payload still in flight. Adversarial review then found the join still stopped at the first blank, truncating a snapshot
cut mid-payload, so blanks filter throughout the run and the wrapper's trailing NUL terminator witnesses completeness.
One investigation note for the next reader: the retained failure logs strip raw ANSI escapes, so every timeout
transcript looked clean; only a Debug-escaped buffer dump showed the `\x1b[2;28H` prefix. Tested commit `08182855` with
the uncommitted fix. Selection `structured clone argv observation` (decoder unit tests 10/10; exact flake test 20/20 in
batch `3d4163fa-3168-47d8-b5d9-a2607f7d3de2`; `structured_launches` plus `agent_listing_real_stack` modules 17/17);
concurrency `4 nextest slots; retries 0`, on a Linux x86_64 worker. Pinned tmux 3.7c executable SHA256
`c4d00d1d947c5e64fd7c4eada92b80a2a0230df32f725f8ae26ee6ac9d3a81c2`, `LANG=C.UTF-8`, ambient `FARHELM_*` scrubbed (only
`FARHELM_TEST_TRACE_DIR` present in the test process). Disposition: fixed in this PR; the TODO.md entry and the
`deflake/known-flakes.txt` line are removed.

Class: readiness

Cause: established

## 2026-09-15 — `a detail read that starts during a restart cannot describe the previous run` (e2e/tests/m6-5-debts.spec.ts)

The deflake sweep's browser battery failed this restart-seam test once on Chromium: the test holds a detail read issued
during a restart, parses the recorded reply, and re-signs it — but the recorded body was the plaintext
`no such session: ...` error, so `JSON.parse` threw
`SyntaxError: Unexpected token 'o', "no such ses"... is not valid JSON`. The page snapshot shows the client itself
detached with the same `no such session` error. All three classification reruns passed. Sweep failure retained run
`963d54d5-217a-4798-be58-331e80ae571b` (failure screenshot, trace.zip, and error-context.md retained under
`playwright-artifacts`); reruns `77e34f5b-79f4-4d7d-b695-a4fb251881d8`, `7d01057a-6205-46d7-bf8c-0984d9758e3d`,
`e3b617a7-c99b-4091-b040-43a89d274e4b`. Tested commit `204b5cb964315ccf15aa97fa719d9a558c30c33b` with a clean tree.
Selection `browser suite, both engines` (Playwright with the recorded exclusions, `--workers=1 --retries=0`);
concurrency `one browser worker; retries 0`, on a Linux x86_64 worker. Pinned tmux 3.7c executable SHA256
`c4d00d1d947c5e64fd7c4eada92b80a2a0230df32f725f8ae26ee6ac9d3a81c2`, `LANG=C.UTF-8`, only
`FARHELM_PLAYWRIGHT_POLICY_FILE` and `FARHELM_TEST_TRACE_DIR` present in the test process. Hypothesis: the read was
served after the restart discarded the old session, so the server answered the teardown error rather than the held
detail JSON; not established. Disposition: open (TODO.md).

Class: peer-lifecycle

Cause: hypothesis

## 2026-09-15 — `a detail read that starts during a restart cannot describe the previous run` fixed (e2e/tests/m6-5-debts.spec.ts)

The open entry above guessed the read was served after the restart discarded the session; the mechanism is narrower and
documented: `Supervisor::relaunch` takes the session off the map between the generation claim and the republication, so
a read landing in that gap is honestly answered 404 (`get_session`'s missing-from-drain arm). The test re-signed the
held reply by parsing it as JSON, and the plaintext `no such session` body crashed the parse. The product behavior is
intended — an admitted 404 only raises the `.refresh-stale` notice and a later good reply clears it — so the fix stays
on the test side: 404 gap answers are released and re-read (every attempt still starts inside the restart, so each
carries the same first-bump epoch) until a 200 JSON reply is in hand, with no new reads issued after 30 s; any other
status fails loudly with the capture index, status, and a body excerpt. A temporary probe forcing one retry iteration
passed once on Chromium (run `1983e6aa-3aba-40e5-a38f-13bc82449f94`); the committed shape passed the exact test 20/20 on
Chromium (run `da6e41ed-7f8e-413b-a096-875a70673406`) and 3/3 on WebKit (run `5228471f-4021-443b-b43e-d6b52a7cb518`).
Tested commit `204b5cb964315ccf15aa97fa719d9a558c30c33b` with the uncommitted fix. Selection
`exact mid-restart read on one engine` (Playwright `--project=chromium-m6-5-debts` / `--project=webkit-m6-5-debts` with
`-g` on the test title); concurrency `one browser worker; retries 0`, on a Linux x86_64 worker. Pinned tmux 3.7c
executable SHA256 `62c79831e9ffb46570aaee6381c36e045d8c131eff03a34f2bdd7ddc35b01ce4` (this checkout's own `.ci-tmux`
build), `LANG=C.UTF-8`, ambient `FARHELM_*` scrubbed (only `FARHELM_TEST_TRACE_DIR` present in the test process).
Disposition: fixed in this PR; the TODO.md entry and `deflake/known-flakes.txt` line written during classification never
left the working copy, so none remains to remove.

Class: peer-lifecycle

Cause: established

## 2026-09-19 — `stalling one tab's writes pauses only that tab; the agent and a sibling stay live` (e2e/tests/terminal-tabs.spec.ts)

Third sighting of the never-paused shape TODO.md watches. Under 9 busy-loop children (about half of an 18-core host),
repeat 3 of 20 loaded WebKit executions failed: the stalled tab's `pauseCount` stayed at 0 for the full 60 s HIGH_WATER
poll ("the stalled tab must cross HIGH_WATER and pause"), while the other 19 repeats in the same batch passed. The
retained console carries no product receipts — detach reasons and queue states are not instrumented — and the retained
trace and error context preserve only the page state, so whether this occurrence also had the recorded early socket
close from run `7fd44a19` is not established. Tested tree: commit `74910dca` (main plus seven earlier doc-only sweep
commits) with pending doc-only sweep edits; recorder run `4c1de15e-9977-482f-a9c7-e02bfd79df0f` (playwright artifacts of
the failing repeat copied beside it). Selection: `--project=webkit-terminal-tabs` with `-g` on the test title,
`--repeat-each=20`; concurrency `one browser worker; retries 0`. Pinned tmux 3.7c, executable SHA256
`b3f11c4f45d7672243ad0a1e0e5a60ba7e335c4a56c2e3ddf48582aa28de6de6` (this checkout's own `.ci-tmux` build),
`LANG=C.UTF-8`, ambient `FARHELM_*` scrubbed (only `FARHELM_TEST_TRACE_DIR` in the test process).

Disposition: closed on 2026-09-20 by making the pause mark lowerable from the fixture, after instrumenting the failure
answered what three sightings could not. The shape is a race between two bounds, not an ambiguous observable. A stalled
tab asks the supervisor to pause only after four megabytes of undrained output; the supervisor cuts a viewer that stops
consuming loose with a visible stall reason, and on a loaded machine that arrives first. Three reproductions in twenty
loaded WebKit executions all carried the same fingerprint: the socket closed at 4.0-5.4 s with code 1006 and the banner
"Detached: terminal stopped consuming output (stalled)", with only 1.4-2.2 MB undrained — so the crossing `pauseCount`
records was unreachable rather than late. The same receipts cleared the fixture, which no earlier sighting could: 7757
held write callbacks, every one matched to the stalled island, and no remount. With the mark lowered to 64 KiB the
crossing takes milliseconds and lands far inside the supervisor's window; 20 of 20 passed under the load that had failed
3 of 20 twice.

Class: budget

Cause: established

## 2026-09-21 — `deleting_refuses_to_move_a_stranger_at_the_recorded_source` (crates/farhelm-supervisor/src/service/teardown.rs)

The checkout-ownership fixture failed while removing its inode-allocation filler directory: on this filesystem the first
replacement already had a different identity, so no fillers had been created. The failure reproduced in the exact-test
selection. Cleanup now runs only when the fixture created fillers; the different-identity premise and foreign-directory
preservation assertions remain. Fixed in #834. Observed on a Linux x86_64 container with four CPU slots, at combined
stack source `43bc4d2a` (recorded Git base `58b2fc5c5730` plus dirty source), in workspace run
`be8d017f-3d7b-41fe-9daf-4c2b644dc628` and focused run `35e269ac-5115-49d3-b476-9ca487e22409`. The latter selected
fourteen exact failures from the workspace, with four nextest slots and zero retries. Both used pinned tmux 3.7c,
executable SHA256 `ebc01bf8f9226634bda074fc7daf1c13dfbe2a89858eb3c6c32c96342d02f43b`, `LC_CTYPE=C.UTF-8` with `LANG` and
`LC_ALL` unset, and only recorder-owned `FARHELM_TEST_TRACE_DIR` in the test environment.

Class: fixture-premise

Cause: established

## 2026-09-25 — parent SIGTERM cleanup (scripts/test-start-stack-cleanup.sh)

The isolated browser-stack cleanup check left its HTTP port, state directory, stack-info file and service processes
alive after its spawner received SIGTERM in run `f2355071-3c67-4a7b-ba05-37f853c4a6b3`; parent SIGKILL and direct script
SIGTERM passed. The unchanged script passed all phases in repetition `a1b6e9b1-a246-4272-8d7b-89452a3f4c45`. Both ran on
Linux x86_64 with one stack phase at a time and pinned tmux 3.7c, executable SHA256
`9a78dcb53a791edaf7de8ba3a9a65544d14c5a88e99bd69d3f1f12b60fc41e11`, locale `C.UTF-8`, and ambient `FARHELM_*` scrubbed;
the recorder supplied only `FARHELM_TEST_TRACE_DIR`. The working tree contained pending browser-test, lint and
formatting changes above `2c25bf3ba22719fbe3896b18270d1c0194696379`; application builds used the main-source version.
Neither run captured watcher state at the failed boundary, so the cause remains unknown. Disposition: open (TODO.md);
retain both observations. The script and the stack now record lifecycle markers and, on a failed phase, the stack's
process tree and log tails, so the next occurrence shows which startup step the TERM landed in and whether the trap ran.
A 2026-10-01 review of this run's timestamps put the phase-2 kill about half a second to a second after the helm
started, inside the script's foreground startup steps, where bash defers the TERM trap until the running command
returns; that is a hypothesis, not established.

Class: unknown

Cause: hypothesis

## 2026-09-25 — Replace refusal row (e2e/tests/terminal-restart.spec.ts)

`Replace confirms inline, can cancel, selects the fresh session, and surfaces refusal` timed out after 60 seconds in
WebKit while locating the injected refusal row in full browser run `bd3fa658-dcf7-4820-8b68-67be5e4b89ed`. The same test
passed unchanged on both engines in focused run `8b2b2c31-7608-4b6d-97b7-b11353f26554`. The full run started at
`ac4e0f30ea54300748ab10e4cf1675669a2fcbba` with pending test and lint edits; the focused selection started clean at
`25b0cc531551a8abb5e6be06faffdfb2c0c453d9`. Both ran on Linux x86_64 with one browser worker, zero retries, pinned tmux
3.7c, executable SHA256 `9a78dcb53a791edaf7de8ba3a9a65544d14c5a88e99bd69d3f1f12b60fc41e11`, and locale `C.UTF-8`.
Ambient `FARHELM_*` was scrubbed; the recorder supplied `FARHELM_TEST_TRACE_DIR` and `FARHELM_PLAYWRIGHT_POLICY_FILE`.
The failed boundary does not establish whether the injected listing was delivered; the cause remains unknown.
Disposition: open (TODO.md); retain the trace and inspect listing interception and refresh on recurrence.

Class: unknown

Cause: unknown

## 2026-09-25 — Replace refusal row reproduces in whole-file WebKit runs (e2e/tests/terminal-restart.spec.ts)

`Replace confirms inline, can cancel, selects the fresh session, and surfaces refusal` failed the same way as the
earlier entry (60-second timeout locating the second, route-injected refusal row) in every one of three runs of the
whole `terminal-restart.spec.ts` file on the WebKit project alone at clean `main` commit
`1867aeddbd997df0c6fdba9d34c3f0cb826442bf`, first run `85814340-1ac0-48a4-beb1-1ed468c26d7b`, and in three more such
runs on an unmerged UI stack built on it (first run `b4279fc7-97db-4003-8eed-f0795f84b2d3`). In the same session the
test passed six times out of six on both engines when selected alone (repetition
`51a974c0-332d-40c5-9ac4-85fb18da8d15`), and failed twice in larger multi-file WebKit selections. So the failure depends
on the tests that run before it in the file, not on load: it is reproducible on demand with the whole file on WebKit,
which is the useful step for the open TODO.md entry. Linux x86_64, one browser worker, zero retries, pinned tmux 3.7c,
executable SHA256 `49acd3312738da4447a018cf1c37e23270404b90fc53f6a396f2c25e113f2190`, locale `C.UTF-8`; ambient
`FARHELM_*` scrubbed by the recorder, which ran in generic mode with tmux validation downgraded to a warning because it
could not locate the repository-owned tmux from a jj workspace. Disposition: open (existing TODO.md entry).

Class: unknown

Cause: unknown

## 2026-09-29 — webview authentication at desktop smoke boot (scripts/desktop-smoke.sh)

The native desktop smoke failed with `FAIL: the webview JavaScript stack did not authenticate its event socket` in both
attempts of the v0.19.0-rc.2 release job (GitHub Actions run `36591492646`, x86_64 Linux job, runner image
`ubuntu-24.04` 20260920.314.1, WebKitGTK 2.52.6). The retained recorder run `bf8b80c8-54ee-41e3-b7f6-6d44c7207e2f`
covers the first attempt: clean tag commit `af8f5ab65f15078a1295e724758fa0abc3837421`, selection
`native desktop smoke legs`, sequential legs, pinned tmux 3.7c with executable SHA256
`74c2614b1b48280e9d4c767a09fa5aee07fc66ae0d437dc3282b7bc56dc59884`, locale `C.UTF-8`, no ambient `FARHELM_*` (only the
recorder-owned `FARHELM_TEST_TRACE_DIR`). In both failures the helm and managed supervisor came up and connected, WebKit
printed its usual Xvfb EGL warnings, the window stayed black, and the desktop asset handler logged no request at all
within the 30-second wait, so the page never started loading its assets. It is not a regression: a throwaway bisect
workflow (run `36603454788`, same hosted runner type, one smoke per job, no gate steps before it) failed the same way at
the untouched v0.18.0 base `03a3051815234e45260c567719f09bf2804b5469` in one of two jobs, and at two of six intermediate
commits, while `af8f5ab6` passed twice; 5 of 12 CI runs failed in all. The v0.18.0 release job itself passed. Locally on
Linux x86_64 the same commit passed 6 of 6, including runs pinned to 4 and 2 CPUs and one without a session bus.
Disposition: fixed in #1227. The page was late, not missing: one of the bisect failures served its assets 29.8 seconds
after launch, just past the wait. A diagnostic run on the same runner type (Actions run `36607548613`, 10 jobs of 5
launches of the prebuilt app, launched the way the smoke's first boot does) put every launch but one per job at 30 to 36
seconds to the first asset request, against about 3 locally, and gdb stacks taken 12 seconds in showed the app's main
thread inside `g_application_register` → `g_dbus_proxy_new_sync`, polling with a 25-second timeout. That is GTK 3's
`GtkApplication` startup auto-starting `org.freedesktop.portal.Desktop`, before tao's event loop, and so the window,
exists. `libwebkit2gtk-4.1-0` recommends `xdg-desktop-portal-gtk`, so the gate's `apt-get install` put the portal on the
runner; the user journal showed systemd starting it, its GTK backend exiting at once for want of a display, and the
portal timing out on backend lookups instead of claiming its name. Local runs never paid this because no portal was
installed, so activation failed in milliseconds. The gate now purges both portal packages after installing its
dependencies.

Class: substrate

Cause: established — a D-Bus activation of the desktop portal that cannot start on the headless runner blocks GTK
startup for D-Bus's 25-second default, which puts the first page load right at the smoke's 30-second wait.

## 2026-09-29 — `restricted_inherited_create_waits_for_parent_restart_then_inherits` (crates/farhelm-supervisor/src/service/handlers.rs)

The test failed with `parent mutation and restricted create must both finish: Elapsed(())`, its 10-second bound on the
parent Restart and the inherited create completing after the test releases the parent's lifecycle claim, in the third
attempt of the v0.19.0-rc.2 release job (GitHub Actions run `36591492646`, x86_64 Linux job, runner image `ubuntu-24.04`
20260920.314.1). Retained recorder run `202dfc97-363a-42d2-bf94-81da09526533`: clean tag commit
`af8f5ab65f15078a1295e724758fa0abc3837421`, selection `workspace except farhelm and farhelm-desktop`, 4 nextest slots
across binaries with retries 0, pinned tmux 3.7c with executable SHA256
`f924697f00d247c2c38baaad49ba00041dc36d5f546750928d567bf847ed001c`, locale `C.UTF-8`, no ambient `FARHELM_*` (only the
recorder-owned `FARHELM_TEST_TRACE_DIR`). The same commit passed this test in the job's first two attempts. Locally on
Linux x86_64, 30 repetitions of the test and its Delete sibling, selected alone under the same pinned tmux, all passed
in well under a second each. Disposition: fixed in #1228, with the Delete entry below. Both handlers reach
`reap_process_tree`, whose first `ScopeManager::available` call probes the host's systemd user manager with a real
transient scope under a 15-second budget, and the fixture used the default, real manager. The hosted runner has a user
manager, and the probe logs its verdict at INFO either way; that line is missing from both retained traces, so the probe
had not returned when the 10-second bound fired. The local runs never paid this: the stand-ins answered at once, and a
local manager answers in milliseconds. Stand-ins that model a scope which never starts (`systemd-run` exits 1,
`systemctl show` answers `not-found` successfully) fail both tests every time at 10.17 seconds with exactly the retained
two-line trace. Why the runner's probe ran past 10 seconds is not established; a failed scope start and a manager slowed
by the four-slot battery both fit. The fixture now uses a disabled scope manager, since the test is about lock ordering
between the handlers and not about cgroup scopes, and its completion bound is 30 seconds, above what the fallback sweep
and a restart may legitimately spend.

Class: budget

Cause: established — the test's 10-second bound covered a real systemd user-manager probe whose own bound is 15 seconds.

## 2026-09-29 — `restricted_inherited_create_waits_for_parent_delete_then_refuses` (crates/farhelm-supervisor/src/service/handlers.rs)

The Delete sibling of the entry above failed the same way
(`parent mutation and restricted create must both finish: Elapsed(())` after 10 seconds) in the fourth attempt of the
same v0.19.0-rc.2 release job (GitHub Actions run `36591492646`, retained recorder run
`b6db54a7-c603-47ad-94e8-7b57e22c609b`, clean commit `af8f5ab65f15078a1295e724758fa0abc3837421`, same selection, 4
nextest slots with retries 0, pinned tmux 3.7c with executable SHA256
`00dddfb2de5c68b3efc121f9222c86076d6ba9f620a95ec2194689d874e12f49`, locale `C.UTF-8`, no ambient `FARHELM_*`). So the
pair failed in two of the job's four attempts. The captured trace ends at the teardown's debug line reporting no systemd
user manager, 46 ms into the test, and shows nothing for the remaining ten seconds. The user-manager probe had therefore
already answered quickly, and the silence falls after it, in upload cancellation or the process-tree sweep. Local
attempts to reproduce on Linux x86_64 all passed: 30 plain repetitions of the pair, 20 with `systemd-run` and
`systemctl` replaced by fast-failing stand-ins to force the sweep-only path CI takes, 1 with stand-ins that hang (the
probe's own five-second query bound held and the pair passed in about five seconds), and 25 with the pair and three busy
loops pinned to a single CPU. One untested lead: both tests use the literal session id `mutation-parent`, and a
sweep-only teardown selects every process on the host whose environment carries that id, so the two tests running at
once could reach each other's processes. Disposition: fixed in #1228; see the entry above. The trace's last line is
logged just before the sweep's first user-manager probe, not after it, so the silence is the probe itself; the
shared-session-id lead was not the cause, since nothing in either test carries that marker.

Class: budget

Cause: established — the same unfinished systemd user-manager probe as the entry above.

## 2026-09-29 — `session_lifecycle::non_utf8_terminal_output_survives_live_stream` (crates/farhelm/tests/e2e), recurrence after dismissal

`session_lifecycle::non_utf8_terminal_output_survives_live_stream` in `crates/farhelm/tests/e2e/session_lifecycle.rs`,
un-ignored and dropped from the deflake exclusions on 2026-09-19 after a clean sweep, failed again in deflake sweep run
`4a18b9ac-c53b-4b7b-bd70-5a1171a7bbc6`: READY arrived, then the live attachment received no bytes at all before the 40 s
wait for BINARY-MARKER expired. The three exact-test repetitions `46bba954-49cd-482e-942c-f68b356ac46d`,
`e2a38976-be77-4a58-a56e-09b95da2ed98`, and `4fbb16a4-6d12-4272-bffe-21bcae13e712` passed. The sweep ran the whole
workspace nextest battery (`cargo nextest run --workspace --exclude farhelm-desktop`, 4 nextest slots, retries 0) at
clean `main` commit `8e82d8b271b1015c25302bbbbb0df019b290ce76` on an 18-CPU Linux x86_64 host, with pinned tmux 3.7c,
executable SHA256 `75ede1768324817dc386aee550c8e7ca68e98530af54762fcb9df0b46491b071`, and locale `C.UTF-8`. Ambient
`FARHELM_*` was scrubbed; only the recorder-owned `FARHELM_TEST_TRACE_DIR` was supplied. Unlike the 2026-09-05 attempts,
the tmux fixture's teardown diagnostics captured the failing pane: it shows the marker rendered and the fixture exited
with status 0. So tmux received the reply, which extends the earlier receipt that the fixture flushed it; the bytes were
lost somewhere between tmux's control-mode output and the test's client, and which layer dropped them is not
established. Disposition: open (TODO.md Deflake), excluded from later sweeps.

Class: unknown

Cause: hypothesis

## 2026-09-29 — Replace refusal row in a full browser sweep (e2e/tests/terminal-restart.spec.ts)

`Replace confirms inline, can cancel, selects the fresh session, and surfaces refusal` hit the 60-second test timeout on
WebKit in deflake sweep run `0c787185-c40c-4070-a3fe-a69bc1a44a82`, the whole browser suite (`npx playwright test`, one
browser worker, zero retries), and passed in all three exact-test reruns `452368f2-ce11-42b3-8eb2-43b31854e1de`,
`7f8a9e9e-3f24-4fe1-913d-0df3e95bc2ab`, and `0c842a5b-e6aa-46ae-aa44-2147229374af`. All four ran at clean `main` commit
`8e82d8b271b1015c25302bbbbb0df019b290ce76` on an 18-CPU Linux x86_64 host with pinned tmux 3.7c, executable SHA256
`75ede1768324817dc386aee550c8e7ca68e98530af54762fcb9df0b46491b071`, and locale `C.UTF-8`; ambient `FARHELM_*` was
scrubbed, and the recorder supplied `FARHELM_TEST_TRACE_DIR` and `FARHELM_PLAYWRIGHT_POLICY_FILE`. The excerpt names
only the timeout, not the step that stalled. The pattern matches the earlier finding that the test fails behind the rest
of its file on WebKit and passes when selected alone. The test had an open TODO.md entry but no
`deflake/known-flakes.txt` line, so the sweep was not excluding it; this record adds that line. Disposition: open
(existing TODO.md entry).

Class: unknown

Cause: unknown

## 2026-09-30 — Replace refusal row fixed (e2e/tests/terminal-restart.spec.ts)

`Replace confirms inline, can cancel, selects the fresh session, and surfaces refusal`, logged three times above as a
WebKit timeout that reproduced whenever the whole file ran and passed when the test ran alone, waited for a row that
only a listing fetch could bring. Its second half installs a second page route that injects a refused interrupted
session into the session listing, but that session exists nowhere on the helm, so nothing prompts the page to fetch the
listing again. The retained trace of the 2026-09-29 sweep failure (full browser run
`0c787185-c40c-4070-a3fe-a69bc1a44a82`) shows the page's last `GET /api/sessions` completing before the route was
installed and none after it, so the click on the injected row waited out the 60-second test timeout. A pass depended on
some unrelated refresh happening to follow the route. Before the fix, the whole file on WebKit alone still failed on
`main` at `1205c0e7` plus #1253's unrelated `terminal.spec.ts` change, run `3ecf40b9-da83-4e0b-a41d-0ba6dc6ab266`. With
the test reloading the page after installing the route, three whole-file WebKit runs
(`9b4c601a-a52c-435a-8897-236d24bdeb18`, `c5a87151-3349-4765-a249-c773312ceb58`, `688a1f9a-f36b-49c3-a938-950785749d6f`)
and one whole-file Chromium run (`63de584c-3411-4d92-8a90-4b03ea346a72`) passed on the same base with the fix applied.
Every one of these trees was dirty with uncommitted supervisor debug-logging edits that were not built into the binaries
under test. All ran on an 18-CPU Linux x86_64 host with one browser worker, zero retries, pinned tmux 3.7c (executable
SHA256 `40812d9309ff36ac7aae468a62eb944c4df4fefab0df135814b1dfa0f34ecdf2`), locale `C.UTF-8`, and ambient `FARHELM_*`
scrubbed by the recorder, which ran the single-engine runs in generic mode. Why an incidental refresh arrives when the
test runs alone but not behind the rest of its file was not pursued; the fix removes the dependence on it. Disposition:
fixed by the PR that adds this entry; the TODO.md entry and its `deflake/known-flakes.txt` line are removed.

Class: fixture-premise

Cause: established

## 2026-09-30 — `session_lifecycle::non_utf8_terminal_output_survives_live_stream` (crates/farhelm/tests/e2e), cause located in tmux

`session_lifecycle::non_utf8_terminal_output_survives_live_stream` in `crates/farhelm/tests/e2e/session_lifecycle.rs`
reproduced on demand with the supervisor's new debug checkpoints on the output path built in: 6 of 40 exact-test
attempts failed in hunt batch `a8487127-853c-4926-badf-92095a1fcea1` (attempts 11, 13, 18, 20, 24, 25) and 3 of 20 in
batch `eb55af56-5e52-4315-af60-881fbfbf4a52` (attempts 8, 11, 13), each run under a transient systemd user scope limited
to `CPUQuota=400%` beside two CPU-bound `yes` processes, the 2026-09-05 shape of two load children on four CPUs. Every
attempt ran one selected test with the four-slot nextest budget and zero retries, at commit `58652a052dcd` (#1255's head
before the stack was rebased), with the checkpoint edits uncommitted in the tree, pinned tmux 3.7c (executable SHA256
`40812d9309ff36ac7aae468a62eb944c4df4fefab0df135814b1dfa0f34ecdf2`), locale `C.UTF-8`, and ambient `FARHELM_*` scrubbed,
with only the recorder-owned `FARHELM_TEST_TRACE_DIR` supplied. All nine failing traces are the same: the attach replay
and the READY line are decoded, queued, and written to the client, and then the supervisor's control client receives no
line of any kind (no `%output`, no command reply, no `%exit`) for the 40-second wait, while tmux's own pane capture
shows BINARY-MARKER rendered and the pane dead with status 0. Reading tmux 3.7c's source explains it:
`control_write_output` only queues a pane's new output for each control client and enables the client's write event, the
pane's EOF then runs `server_destroy_pane`, which frees the pane's input buffer and sets `wp->fd = -1`, and
`control_write_pending` discards every block still queued for a pane whose `fd` is -1. Output that arrives just before
the pane's process exits is therefore drawn on the screen but never sent to a control client, and load widens the window
between the queueing and the write callback. The fake agent prints the marker and exits at once, which lands in that
window. This is not specific to the test: an agent's last output before it exits can be missing from a live attachment,
which shows it only when the terminal is next attached and replayed. Disposition: open (TODO.md Deflake, with the fix
options), still excluded from the deflake sweep; the checkpoints ship so any later occurrence is traced.

Class: substrate

Cause: established

## 2026-09-30 — final output recovered after pane exit (crates/farhelm/tests/e2e/session_lifecycle.rs)

The final-output loss recorded for `session_lifecycle::non_utf8_terminal_output_survives_live_stream` is fixed by
catching an existing attachment up from retained pane history when the supervisor observes pane death. tmux 3.7c can
discard its queued live bytes at EOF; resetting before replay replaces already-delivered content rather than duplicating
it. The test still makes its producer write and exit immediately, now runs the serving supervisor's ticker, and accepts
raw `0xff` before recovery or its rendered replacement after a recovery reset (tmux canonicalizes that byte in history).
The exact selection `test(=session_lifecycle::non_utf8_terminal_output_survives_live_stream)` passed all 40 attempts in
hunt batch `994e88a8-adc1-4d75-9701-280ef02fd750` on Linux x86_64 under a transient user scope limited to
`CPUQuota=400%` beside two CPU-bound load processes, with four nextest slots, zero retries, and a one-hour batch cap.
Tested commit `36949b4d1f2d6afa445f3b334b1e7570f524df3e` with the fix uncommitted; recorded tmux 3.7c executable SHA256
`40812d9309ff36ac7aae468a62eb944c4df4fefab0df135814b1dfa0f34ecdf2`, locale `C.UTF-8`, ambient `FARHELM_*` scrubbed, and
only recorder-owned `FARHELM_TEST_TRACE_DIR` supplied. A separate deterministic supervisor regression suppresses live
delivery and proves reset plus final history on the same attachment. Disposition: fixed by the PR that adds this entry;
the TODO.md entry and its `deflake/known-flakes.txt` line are removed.

Class: substrate

Cause: established

## 2026-09-30 — `service::core::tests::an_ambiguous_planned_checkout_never_adopts_a_foreign_directory` (crates/farhelm-supervisor/src/service/core.rs)

The v0.20.0 release gate's workspace nextest step failed this one test on the hosted x86_64 Linux runner (GitHub Actions
run `36811756125`, recorder run `80893b36-8051-45bd-97f4-9ed8e8b451fb`, clean tested commit
`e0b9992760ed27896d90b9c6826ce2959ecead3e`, selection `workspace except farhelm and farhelm-desktop`, four nextest slots
across binaries with zero retries, pinned tmux 3.7c with executable SHA256
`0838fd84ec24dfb4c70e250f7a8db71e3c2a2095847ef8335aee8871c925c07a`, locale `C.UTF-8`, no ambient `FARHELM_*`, only
recorder-owned `FARHELM_TEST_TRACE_DIR` supplied); the other 2605 tests passed. It panicked after 15.17 s at "Delete
retires the committed refusal without a restart". The retained trace shows the reopened supervisor's systemd
user-manager probe starting about 130 ms into the test and reporting at 15.13 s that it did not finish within its 15 s
bound, after which teardown found no usable user manager and took the sweep-only path, and Delete returned an error. The
same code (the gate's source differs from v0.20.0-rc.4 only in the version, changelog and TODO.md) passed the rc.4 gate
on the same runner type, and the exact test passed all 20 attempts of local hunt batch
`dda5c41c-7b4e-4b78-af86-23bc4999924a` on Linux x86_64 at commit `e0b9992760ed27896d90b9c6826ce2959ecead3e`, where the
user manager answers in milliseconds. This is the same exposure as the 2026-09-29 entries above, a test exercising the
host's real user manager on a runner where it can stay silent for the full probe bound. Disposition: fixed in #1324,
which gave the test a disabled scope manager. The test runs two supervisors on one store. The first, on the default real
scope manager, found the user manager usable at 117 ms and recorded on the row that its launch was scoped before the
simulated crash; the reopened supervisor got a fresh manager whose probe, started as it reopened, timed out before
Delete ran. A recorded scope that cannot be checked is an unconfirmed cleanup, which Delete refuses by design (SPEC.md
"Lifecycle operations"); the trace's last line is that warning for the row's `-0.scope` unit. So the sweep-only path
itself did not fail: Delete refused on the scope evidence, as it does in production until the probe's one-minute backoff
passes and the manager answers again. Locally, a `systemd-run` stand-in that reaches the real manager once and then
models a scope that never appears failed the unchanged test with the same trace, and the fixed test passed under it in
0.34 s without invoking the stand-in at all.

Class: substrate

Cause: established — the test depended on the host's systemd user manager answering twice, and the second probe ran out
its 15-second bound.

## 2026-10-01 — `InstalledUninstall.test_confirmation` and `test_fresh_preview_and_remove` (scripts/test-uninstall.py)

The v0.21.0 release gate's macOS installed-uninstall step failed these two tests on the hosted macOS 15 arm64 runner
(image `20260828.587`, Darwin 24.6.0, 3 CPUs; GitHub Actions run `36870068282`, job `110395516935`, recorder run
`effd9cb4-3ec4-4998-89ee-4454ce9bf790`, clean tested commit `2e8aa753964559e8ce5583cac3f09fcad259c006`, selection
`macOS installed uninstall acceptance`, one fixture at a time, no tmux, locale `en_US.UTF-8`, no ambient `FARHELM_*`,
only recorder-owned `FARHELM_TEST_TRACE_DIR` supplied); the other seven tests passed and one skipped as Linux-only. Both
failures are the harness's 30-second timeout on a `farhelm uninstall` child (the debug build the step installs), in one
case answering a terminal prompt and in the other with `--yes`. In the prompted case the killed child's captured stdout
runs through the complete removal report and ends with "Farhelm uninstalled. User data was retained.", so uninstall had
finished its work and written its last output before the 30 seconds ran out; what did not happen in time is the child
exiting (or its pipes closing). Nothing in uninstall runs after that final write: its locks are released before the
report and it starts no other process. The two failures came about a minute apart and the run was slower throughout:
`test_confirmation` took 81 s and `test_fresh_preview_and_remove` 46 s, against 65 s and 37 s when both passed in the
v0.21.0-rc.1 gate (run `36827033494`) on the same runner type that morning, with the same shipped code (the gate's
source differs from rc.1 only in the version, the changelog, and docs and test-script commits outside this step). So
even a passing run spends a large share of the budget, but how much of it is the uninstall child itself is unavailable.
The uninstall code and this script last changed in #1330 (2026-09-30), which added the install, app, setup and runtime
locks. No earlier entry names this script. No reproduction was attempted; the hosted failure artifact expires
2026-12-30. v0.21.0 was re-cut as v0.21.1, whose gate (run `36877043811`, job `110419166903`, clean tested commit
`b13ddc71511b53f743e2e7f98e17b036ebd06e64`) failed the same way in `test_fresh_preview_and_remove`,
`test_install_lock_interplay` and `test_missing_flat_desktop`, all `uninstall --yes` children at 30 s. A diagnostic copy
of the script that timed each child and sampled it on timeout then separated the two hypotheses. Each uninstall verifies
every file it removes against its recorded SHA-256, and since #1330 it does so twice, once for the plan and again under
its locks; the step tests the debug build, whose `sha2` is unoptimized and whose CLI is hundreds of megabytes (354 MB on
Linux), and the CLI is hashed twice per pass on macOS (the flat copy and the app's copy). On an otherwise idle Linux
x86_64 machine a single pass (`--dry-run`) took 15.3 s and confirmed uninstalls 28 to 30 s, and one prompted uninstall
timed out there with only its plan printed (recorder run `cdfd5583-b185-4d84-b353-8534a2b2220d`). On a hosted macOS
runner outside the release job (CI run `36883649071`) passes took about 6.2 s and confirmed uninstalls about 12 s, and
every child exited within 0.7 s of its last output, so there is no stall on exit; the release job ran the whole suite
1.6 to 2 times slower (276 to 340 s against 173 s). Scaling by that alone puts a confirmed uninstall at 19 to 24 s,
short of the 30 s the failures show, so the hashing there was slowed more than the suite as a whole; by how much is
unmeasured. A kill landing just after the final report explains the complete output in the first failure. Disposition:
fixed by the PR that adds this paragraph, which compiles `sha2` at `opt-level = 3` in the dev profile; with that, every
uninstall child in the same Linux run finished in under 1 s and the suite passed (recorder run
`891a907b-c3a7-458c-ba4c-b69bd7e3ee89`). The TODO.md entry is removed.

```
FAIL: test_confirmation (__main__.InstalledUninstall.test_confirmation)
AssertionError: confirmation child timed out: b'... Remove this installation? [y/N] removed ... Farhelm uninstalled. User data was retained.\n' b''
ERROR: test_fresh_preview_and_remove (__main__.InstalledUninstall.test_fresh_preview_and_remove)
subprocess.TimeoutExpired: Command '['<fixture-home>/.local/bin/farhelm', 'uninstall', '--yes']' timed out after 30 seconds
```

Class: budget

Cause: established — the debug build's unoptimized SHA-256, run twice over a debug CLI since #1330, put a confirmed
uninstall at the harness's 30-second per-child limit on the release job's macOS runner.

## 2026-10-05 — `sidebar menu flyouts sit under no stacking context inside the sidebar` (e2e/tests/feedback.spec.ts)

The test #1598 added fails intermittently, on either engine, at its first `await expect(helpFlyout).toHaveCount(0)`: the
Escape it presses after opening the help menu does not close it. Seen on a Linux x86_64 development machine (kernel 6.8,
Node 24.20, Playwright 1.62.0, recorder-pinned tmux 3.7c with executable SHA256 prefix `3590826b41cd0671`, locale
`C.UTF-8`, no ambient `FARHELM_*`, one browser worker, retries 0) while validating the auto-update plan's stack rebased
on main `84c10b3b`, with that stack's own changes to the sidebar bar's menus (not on main; the tree was otherwise
clean): once in a run of the feedback, tooltip and tooltip-coverage specs (recorder run
`4c88e5c5-64f6-4ece-a1f9-c5b77c15c0e7`, chromium; the other 33 cases passed, this one failed after the 5 s expect
timeout), and then 1 of 3 focused attempts of this test alone on both engines (runs
`2a894aca-f5e1-468a-8320-19188305e5bf` and `2ff5d950-1b58-4808-befd-8c04e9370f08` passed;
`a959a160-1719-47e7-b0b9-d0afb068eb15` failed on webkit). Both failures' page snapshots show the selected session's
`Terminal input` focused, not the menu. Cause: established by reading the code, not by a run of main's own build — the
terminal takes focus when it reveals after connecting (`takesFocus` in `crates/farhelm-ui/assets/terminal.js` yields
only to an editable field, another terminal or a dialog, not to an open menu), and the test opens the help menu right
after `page.goto` and closes it with Escape, which only reaches the menu while focus is inside it; whenever the terminal
connects after the menu has opened, it takes the focus and swallows the Escape. Main's version of the test has the same
sequence and the same terminal rule, so the race is not introduced by the stack's menu changes, though the failures
above were all seen with them. Fixed in the auto-update stack's PR #1604 by waiting for the terminal to take focus
before the first menu opens; with that, 4 of 4 focused attempts on both engines passed (runs
`79c0dd79-1678-4604-be21-9a8b9d9d06e0`, `f7e21223-c3bb-4653-a682-d4650b32b520`, `363c37b5-3f62-4d5c-96b2-50418753b070`,
`5e56d6b7-987f-45b9-8ba3-0d6b64047c3d`). Whether the terminal stealing focus from an open sidebar menu is itself worth
fixing in the product is left open.

Class: readiness

Cause: established by code reading — the selected terminal's reveal takes focus from the just-opened help menu, so the
test's Escape lands in the terminal.

## 2026-10-06 — `unconfirmed_restart_is_refused_only_while_the_agent_is_working` (crates/farhelm-supervisor/src/service/core.rs)

Failed once in the v0.25.0-rc.3 release build's x86_64 Linux gate (release run 37571233310 attempt 1, retained recorder
run `fc591c2f-c3b8-4ebe-beda-9b64e26a6688` in the hosted `test-run-failure-37571233310-1-2` artifact, which expires), at
tested commit `7f46e0c6` with a clean tree, selection "workspace except farhelm and farhelm-desktop", 4 nextest slots
across binaries, retries 0, the recorder's pinned tmux 3.7c (executable SHA256 prefix `f6fc5de04bae852f`), locale
`C.UTF-8`, and only the recorder-owned `FARHELM_TEST_TRACE_DIR` in the child. It failed alone (2773 of 2774 passed),
fast (1.46 s) on its first premise assertion, about 0.3 s after the session was created; the same test passed in the
rc.1 and rc.2 gates on the same supervisor code. It reproduces readily on a quiet Linux x86_64 development machine
(kernel 6.8, 18 CPUs, recorder-pinned tmux 3.7c with executable SHA256 prefix `c8f5e37c5169b045`, `C.UTF-8`, ambient
`FARHELM_*` removed by the recorder): 5 of 20 attempts of this test alone at `7f46e0c6`, clean tree (first retained
failure `749a6ec8-99ff-4943-bcf4-232e23d6a819`). It also fails on v0.24.0's source, 2 of 20 alone, run directly with
cargo-nextest rather than through the recorder (the scratch copy was not a checkout) and with every `FARHELM_*` unset,
so it predates this release. Every failure shape is the test's `sleep 300` agent no longer being alive a second or two
after launch: the "agent is alive" premise fails, or the refused restart's follow-up finds no live agent, or the
unconfirmed restart is not refused because the session reads `Unknown` instead of working. A failing attempt's trace
shows the later stop reporting that the launch's transient systemd scope was already gone. Cause not established. The
suspicion is the launch's transient scope on the shared `systemd --user` manager, or the test's `sh -c` stand-in agent,
ending early; neither has been checked. Open; recorded in TODO.md's Deflake bucket and in `deflake/known-flakes.txt`.

```
thread 'service::core::tests::unconfirmed_restart_is_refused_only_while_the_agent_is_working' panicked at crates/farhelm-supervisor/src/service/core.rs:17822:18:
fixture premise: the agent is alive
```

Class: fixture-premise

Cause: unknown

## 2026-10-09 — `the session view gives every control hover text` (e2e/tests/tooltip-coverage.spec.ts)

The launcher template-save validation's three-spec browser run `0bcdfedf-2839-4732-b2c2-8accc96344a6` passed 37 of 38
cases and failed this existing WebKit case alone, before terminal interaction: the shared row did not appear within the
helper's default five-second assertion budget. An exact WebKit-only three-repeat run
`fb09f027-72c6-4384-b643-6573f1c895dd` passed twice and failed once at the same boundary, without the preceding sidebar
test. Both runs tested clean commit `3293195e`; the first selected templates, tooltip coverage and launcher tabs on both
engines, the second selected only this case, each with one browser worker and zero retries. Both failed traces contain
successful session listings with the shared session still present. In the first trace, browser-side processing of that
listing completed 4.80 seconds after the row assertion began, the assertion expired at 5.05 seconds, and the terminal
received focus at 6.78 seconds; the healthy fixture's initial rendering raced the assertion budget. Underlying
scheduling pressure is unproven: original load was not recorded; the narrow run's observed load averages were
9.51/11.81/13.97 on 18 logical CPUs, with no other owned build, but other executors' activity was not inventoried. Linux
x86_64, kernel 6.8.0-146-generic, Node 26.11.0, Playwright 1.62.0, pinned tmux 3.7c executable SHA256
`7913713d94756a96d6b6a7b63041d86ecf31fde6d878ad398bbc2f4fe75e8c2a`, locale `C.UTF-8`; ambient `FARHELM_*` was scrubbed.
The initial strict browser run supplied recorder-owned `FARHELM_PLAYWRIGHT_POLICY_FILE` and `FARHELM_TEST_TRACE_DIR`;
the generic narrow run supplied only `FARHELM_TEST_TRACE_DIR`. Browser executable identity and compiler version were not
extracted. Full evidence, including both failed traces, is retained privately under the run IDs above. Disposition: the
launcher template-save PR (#1717) gives the shared helper the sidebar fixture's existing 20-second row-readiness budget
and probes the session API on a timeout to distinguish disappearance from delayed rendering. Attachment and fake-agent
readiness remain separate oracles. With the correction, the same exact WebKit case passed three of three repetitions in
run `98ba4b12-ce93-414d-8911-c4d759db83a0`; the original failures remain retained.

```text
Error: expect(locator).toBeVisible() failed
Expected: visible
Timeout: 5000ms
Error: element(s) not found
```

Class: readiness

Cause: established

## 2026-10-08 — `borrowers retain the checkout until the final stopped session is deleted` (e2e/tests/github-checkouts.spec.ts)

The fresh-checkout cache plan's browser selection failed this Chromium scenario at its unchanged 60-second test limit in
run `687d4902-b93f-4b51-8aac-c992e53883c9`, clean commit `1aba59e805580ff20f2e1bc0ef6ab7b8e110055e`. That selection ran
only `github-checkouts.spec.ts`, both engines, one worker and zero retries: 11 passed, six timed out, and three WebKit
fixture premises failed. The timeouts were Chromium borrowers, replacement and clone defaults, and WebKit lost durable
refusal, structured checkout and clone lost race. The three later fixture failures followed a separate, established
cleanup defect: session cleanup throws before the Git fixture restores its URL mappings, so the next engine's reused
repository name resolves to the old fixture commit. Exact borrowers-only Chromium runs, with a fresh stack each time,
also timed out: one of one at four CPUs (`ef15cc41-c2b8-4631-8e67-6369ab1a0538`), one of one at eight CPUs
(`22f6e517-77a1-40b4-ab51-c6410d6cb0e5`), and one of one on rebuilt, clean main
`7b2c390b07c9a7654a13f73f2dbb877d00842adb` at eight CPUs (`0217e0f1-a5fc-4400-ac7c-99c94c008d62`). Their redacted child
command was
`npx playwright test --project=chromium-github-checkouts --workers=1 --retries=0 --grep 'borrowers retain the checkout until the final stopped session is deleted'`.
The timeout therefore predates the cache change; the other five timeouts have no clean-main control. Traces show several
64-character keyboard typing calls taking 5–10 seconds after real clones completed, but the cause is unknown. More CPU
capacity did not resolve it. The runner was an owned Linux x86_64 Ubuntu 26.04 sandbox, kernel 6.8.0-146-generic, 8 GiB
memory, initially four CPUs; no other build or test ran during the exact reproductions. Initial run CPU usage reached
its four-CPU cap; load average and typical passing duration are unavailable. Node 22.22.1, Playwright 1.62.0, native
Chromium build 1234 and WebKit build 2336, Rust 1.98.1 and Python 3.14.4; tmux 3.7c executable SHA256
`7ffdb77af092113ce3a777e581bbe9751c15504105404a68828a598cbf3e6bc1` was recorded. Locale `LC_CTYPE=C.UTF-8`, with `LANG`
and `LC_ALL` absent. Ambient `FARHELM_*` names were empty; the recorder supplied `FARHELM_TEST_TRACE_DIR`, and the
controlled spec run also supplied `FARHELM_PLAYWRIGHT_POLICY_FILE`. Browser test and UI sources were identical between
the tested cache commit and main. Full evidence, including traces copied before later runs could overwrite them, is
retained privately on the executing machine with no hosted expiry. Open: TODO.md's Deflake entry tracks the timeout and
cleanup cascade; only the borrowers scenario is excluded from the deflake sweep.

```
Test timeout of 60000ms exceeded.
```

Class: unknown

Cause: unknown

## 2026-10-09 — `composer keeps arbitrary model-first choices reviewable` (e2e/tests/sidebar.spec.ts)

The deflake sweep's browser phase failed this scenario in both Chromium and WebKit in run
`edf936f9-21f6-463b-8ecd-01a1b2eb1efd`, clean commit `0c2453de30267184405ce1868031a822344596fd` (empty porcelain). The
selection was the full browser suite on both engines, one worker and zero retries, with the redacted child command
`npx playwright test --grep-invert '<the known-flakes exclusions>'`; 1360 passed, 14 skipped and 4 failed in 3.0 hours.
The only other failures were `a wheel over the header or the sidebar edge never scrolls the document` in
`e2e/tests/shell-scroll.spec.ts`, in both engines, a deterministic cleanup failure fixed separately in #1740. Both
failing attempts here ended on a five-second assertion timeout, not the test limit: 7.4 seconds in Chromium and 14.2
seconds in WebKit, against isolated passes of 3.1–3.9 and 7.2–8.5 seconds. The summary showed `trust: true` throughout
the wait. The deflake driver's three classification reruns, each running the test alone in both engines, passed six of
six (`eec5659f-2500-45bd-8956-89110f94210a`, `877a67bb-3be6-457b-aac7-e9761a1db2fb`,
`46f837df-e174-4133-9411-155d45d2bade`), which is what made it look like a flake. It is order dependence instead. A
fresh launch dialog preselects the helm-wide remembered workspace trust. The quick-switcher test
`the pinned name action opens ordinary New with remembered defaults` sets that preference to `true`, and its spec's
`afterEach` calls the shared `resetPreferences` helper, which reset every preference except remembered workspace trust.
Projects run in file order with one worker, so `quick-switcher.spec.ts` runs before `sidebar.spec.ts` in each engine and
the sidebar test inherited `true`. The test already pinned its permissions choice for the same reason, but not trust.
Running only those two tests in that order on main `b7188aaa` failed the sidebar test in both engines on the first
attempt (`1b3eebcb-e7bf-4f11-8ad5-cab554017f3d`, one of one). Fixed in #1739: `resetPreferences` now clears remembered
workspace trust, and the test pins trust to default as it does permissions. With both changes the same pair passed four
of four (`f7bd1275-bcea-447c-b2d9-f78b11aa3903`). With only the test's pin, and the helper still leaking, it also passed
four of four (`a5851836-1b7f-41f3-889e-7ac7f4656bf2`). The full quick-switcher and tooltip-coverage specs passed 20 of
20 on both engines with the helper change (`ae8c5025-e05d-4eb0-909c-e4e3ad0dfe49`). The sweep ran on a Linux x86_64 host
with 18 CPUs, kernel 6.8.0-146-generic, Python 3.14.8, while another agent ran its own test batteries from a sibling
checkout; load average was not recorded. tmux 3.7c, executable SHA256
`9278b74aab5e012b9732f9f1af693929ba9edc7296c7b2a2c25818683adc2647`, was recorded. Locale `LANG=C.UTF-8` with `LC_ALL`
and `LC_CTYPE` absent. The recorder removed the ambient `FARHELM_AGENT_ID`, `FARHELM_SESSION_ID`,
`FARHELM_SESSION_TOKEN`, `FARHELM_SUPERVISOR_SOCK` and `FARHELM_TMUX` and supplied `FARHELM_PLAYWRIGHT_POLICY_FILE`.
Node, Playwright and browser build versions are unavailable here. The full evidence is retained privately on the
executing machine with no hosted expiry.

```
Expected: "model: reviewable-codex · effort: default · permissions: default · trust: default"
Received: "model: reviewable-codex · effort: default · permissions: default · trust: true"
Timeout:  5000ms
```

Class: fixture-premise

Cause: established

## 2026-10-09 — sidebar version-readout startup (e2e/tests/sidebar.spec.ts)

`the sidebar app bar explains the helm and window builds` failed in WebKit before reaching its hover assertions in run
`be42dc00-8cc4-4978-bcdc-8be93f9006d0`; Chromium passed. The pre-existing initial readout assertion (formerly in
`the sidebar app bar shows the helm build and client tooltip`) gave rendering only five seconds after navigation. The
exact WebKit case, unchanged, then failed two of three attempts in run `042d9e34-5ff8-4b24-ab7d-b3bda69c8c4a`. Both runs
used clean feature commit `3598ae3cf9f9b5fdd62046518a47654f7f5a9210`, one worker, zero retries and the exact
version-readout selection; the second repeated only `webkit-sidebar` three times. No original-product baseline was
rebuilt, so these runs alone do not establish the failure rate before the hover change. The failed trace shows
navigation ending at 34.15s, the assertion starting at 34.21s, preferences fulfillment at 39.16s and session-list
fulfillment at 39.97s, after the assertion's 39.50s deadline. A later snapshot contains the expected readout. This
establishes a startup-readiness failure; why startup took that long is unknown. Load average was observed around 21 on
18 CPUs, but CPU causation was not established; contemporaneous process inventory, memory pressure and typical passing
duration are unavailable. Linux x86_64, kernel 6.8.0-146-generic, Node 26.11.0, Playwright 1.62.0; browser build
identities and compiler version were not extracted. Both recorded tmux 3.7c executable SHA256
`7913713d94756a96d6b6a7b63041d86ecf31fde6d878ad398bbc2f4fe75e8c2a`, locale `C.UTF-8`. Ambient `FARHELM_*` names were
scrubbed; the recorder supplied `FARHELM_TEST_TRACE_DIR`, plus `FARHELM_PLAYWRIGHT_POLICY_FILE` in the first run. The
reproduction used generic recording; its attachments were copied before another run could overwrite them. Full evidence
is retained privately on the executing machine without hosted expiry. Disposition: fixed in #1733 by waiting for the
exact rendered stamp with the existing sidebar's 20-second readiness budget at both navigations, with a live helm probe
on failure; hover assertions retain their five-second budget. Corrected run `c7852c83-aefe-4964-9765-10af59ef9c0f`, the
same feature source plus that test-only correction, passed one Chromium and one WebKit case without retries. Those two
passes do not establish a long-term flake rate.

```
Error: expect(locator).toHaveText(expected) failed
Locator: locator('.app-version')
Expected: "9.9.9-forced-helm"
Timeout: 5000ms
Error: element(s) not found
```

Class: readiness

Cause: established

## 2026-10-10 — `structured_claude_resume_survives_supervisor_reconstruction` (crates/farhelm/tests/e2e/restart_with_resume.rs)

The bounded Linux x86_64 container workspace run `1c7d2fa4-5c88-4696-8e8b-6f13ceac682c` failed this existing fixture
while inspecting the reconstructed successor's argv: it extracted an 80-column prefix before establishing complete
output. The same unmodified test failed one of one exact attempts in `308f7324-73d1-4e12-a5fd-1ff4591f2d66`, using
`cargo nextest run --workspace --exclude farhelm-desktop -E 'test(=restart_with_resume::structured_claude_resume_survives_supervisor_reconstruction)'`.
The failures took 0.780 s and 3.250 s, respectively, at an assertion rather than a timeout. Both recorded HEAD
`fbdd8ad4a0ef8b86fe7ec5086f42ed56c945f4ae` plus dirty PR3 changes to settlement, cleanup, tests, SPEC_impl.md, TODO.md
and its fragment. The full run's other failures were Codex attribution (a same-session missing reconciliation boundary)
and the restart-consent fixture below; the exact attempt failed alone. Fixed in #1758 by attaching the structured branch
wide and waiting for `FAKE-AGENT READY` before extracting argv, retaining its post-replay live exchange. Corrected run
`0d7b4d8b-871b-4684-8aaa-6af53b1a7ea6` passed this case once in 2.979 s and the unstructured branch once; it still
failed the Codex case at a later migration boundary. The corrections do not establish how much wrapping versus
partial-frame timing contributed, or a long-term failure rate. The fixture predates this stack; no pre-stack product
baseline was rebuilt. Both failing runs used four global nextest slots and zero retries, a four-CPU/32 GiB container on
a shared 18-CPU host, kernel 6.8.0-146-generic, Rust 1.98.1, Python 3.14.4, nextest 0.9.143, and recorded tmux 3.7c
executable SHA256 `3590826b41cd0671f16fce6b08d11ef6ac75cc548373c7eb98204273e4f0539f`. Locale was `LC_CTYPE=C.UTF-8`,
with LANG and LC_ALL absent; no ambient `FARHELM_*` names were present, and the recorder supplied
`FARHELM_TEST_TRACE_DIR`. The container had no usable systemd user manager or passwordless SSH; host load and
overlapping process inventory were not recorded for the failures. Full console, JUnit and traces are retained privately
without hosted expiry.

```
the reconstructed Claude successor must use its default resume form: <owned fixture>/claude fake-agent --script hook-report --rec
```

Class: replay-live

Cause: established

## 2026-10-10 — `unconfirmed_restart_is_refused_only_while_the_agent_is_working` (crates/farhelm-supervisor/src/service/core.rs)

The known restart-consent flake from 2026-10-06 recurred in workspace run `1c7d2fa4-5c88-4696-8e8b-6f13ceac682c` and
failed one of one unchanged exact attempts in `d2b8c62d-0882-4efc-a251-a5968ac1fabd`, with
`cargo nextest run --workspace --exclude farhelm-desktop -E 'test(=service::core::tests::unconfirmed_restart_is_refused_only_while_the_agent_is_working)'`.
Both failed the live-agent premise, in 0.381 s and 1.466 s, rather than a timeout. The supervisor fixture passed
`dummy_exe()`, explicitly `/nonexistent/farhelm`, as its launch shim: the requested `sleep 300` never executes, and a
briefly live login shell can satisfy the PID check before dying. This establishes the defect in this recurrence; the
container had no systemd user manager, so one is not necessary for it, but that does not establish the cause of every
historical failure. Fixed in #1758 with an owned executable stand-in that `exec`s `sleep 300` and a pinned `/bin/sh`
launch shell, retaining same-PID refusal and different-live-PID accepted restart assertions for Idle and Waiting.
Corrected run `0d7b4d8b-871b-4684-8aaa-6af53b1a7ea6` passed once in 3.309 s; this is correction evidence, not a
long-term failure rate. The matching TODO and deflake exclusion are removed. The workspace run also failed the
reconstruction fixture above and a same-session Codex migration; the exact attempt failed alone. Both failing runs
recorded HEAD `fbdd8ad4a0ef8b86fe7ec5086f42ed56c945f4ae` plus dirty PR3 settlement, cleanup, tests, documentation and
fragment changes. Four global nextest slots, zero retries; Linux x86_64, four-CPU/32 GiB owned container on a shared
18-CPU host, kernel 6.8.0-146-generic, Rust 1.98.1, Python 3.14.4, nextest 0.9.143, recorded tmux 3.7c executable SHA256
`3590826b41cd0671f16fce6b08d11ef6ac75cc548373c7eb98204273e4f0539f`. Locale `LC_CTYPE=C.UTF-8`, LANG and LC_ALL absent;
no ambient `FARHELM_*` names, only recorder-owned `FARHELM_TEST_TRACE_DIR` in the child. The container also lacked
passwordless SSH. Host load and overlapping process inventory were not recorded. Full console, JUnit and traces remain
privately retained without hosted expiry; the earlier release artifact's expiry is unchanged.

```
fixture premise: the agent is alive
```

Class: fixture-premise

Cause: established

## 2026-10-10 — `remembered destination is rechecked after key minting` (e2e/tests/destination-authority.spec.ts)

WebKit failed this existing destination-authority case in retained run `219019be-3701-4a7f-a6e6-23e9bed45091`; the other
65 selected cases passed. The selection covered launcher host-picker, keyboard, destination-authority, sidebar,
switcher, checkout-preview and multi-host transitions on both engines, one worker and zero retries. The tested Git head
was `6c395a7326bf90a979da20f8917c69228ae65987`, with the uncommitted launcher host-picker change above it. The bounded
timeline records the stub feed closing 9.995 seconds after arrival, matching the client's ten-second silent-feed
deadline: the fixture never greeted its connection, although the real helm always does. Notification then had no live
peer. The failed case took 23.656 seconds; three unchanged exact WebKit repetitions in generic recorded run
`78f7a8c1-1d1d-47ae-bce7-b32191c18043` passed in 18.8, 22.4 and 13.7 seconds. Those passes do not invalidate the
recorded lifetime failure. This was Linux x86_64, Node 26.11.1 and Playwright 1.62.0, pinned tmux 3.7c with recorded
executable SHA256 `c84c73fdedbcf3fbcca8cc261e208c5795f7493609b027b39fe11e4af3a2fb04`, locale `C.UTF-8`. The recorder
removed ambient `FARHELM_AGENT_ID`, `FARHELM_SESSION_ID`, `FARHELM_SESSION_TOKEN`, `FARHELM_SUPERVISOR_SOCK` and
`FARHELM_TMUX`; its child carried recorder-owned `FARHELM_PLAYWRIGHT_POLICY_FILE` and `FARHELM_TEST_TRACE_DIR`. Other
process load and load averages were not captured. Full evidence remains in private local recorder storage. PR #1775
fixes both destination fixtures with the existing greeting facility and checks for a live feed before notifying, while
retaining the deliberately held key-creation boundary.

```text
Error: no feed socket is open to notify on
```

Class: peer-lifecycle

Cause: established

## 2026-10-10 — `borrowers retain the checkout until the final stopped session is deleted` (e2e/tests/github-checkouts.spec.ts)

The managed-checkout trash plan's final checkout/sidebar selection reproduced the existing 60-second borrower timeout on
WebKit in run `6a629bb4-ed2b-4d80-9a8e-c95f71f0d192`: 27 passed, this one timed out, with no skipped or unstarted cases.
The tested parent was `3edcbbc228c4f3ee0852a13cd3fe09b732802550`, with the final trash UI, docs and browser-fixture
changes uncommitted. Selection was `npx playwright test 'github-checkouts.spec.ts' 'sidebar-resize.spec.ts'`, both
engines, one worker and zero retries. Chromium's same case passed in 42.5 seconds; the WebKit result includes 66.4
seconds through teardown. Its trace shows continued progress through the final filesystem comparison, whose assertions
completed just after the total deadline; several terminal keyboard typing calls took 2–3 seconds. An exact fresh-stack
WebKit reproduction, run `7510f5f0-49da-4204-aba6-13a6aee817e0`, failed one of one at the same unchanged limit while
reaching the final borrower's stop, followed by request-context closure during cleanup. Its child command was
`npx playwright test --project=webkit-github-checkouts --workers=1 --retries=0 --grep 'borrowers retain the checkout until the final stopped session is deleted' --output '<private artifacts>'`.
An earlier anchored-title attempt, `bde25312-4a1f-417f-bb8c-fab07f764101`, selected no tests and is not a reproduction.
The owned Linux x86_64 sandbox had a four-CPU quota and 12 GiB memory cap; no other build or test ran inside it. A later
case's single sample reached 413% CPU and used 4 GiB, but load during the failing borrower and host-wide activity are
unavailable, so that sample does not establish causation. Recorded tools were Node 22.22.1, Playwright 1.62.0, Python
3.14.4 and kernel 6.8.0-146-generic; compiler and browser build identities were not captured in this run's manifest. The
recorded tmux executable was 3.7c with SHA256 `7ffdb77af092113ce3a777e581bbe9751c15504105404a68828a598cbf3e6bc1`. Locale
was `LC_CTYPE=C.UTF-8`, with `LANG` and `LC_ALL` absent. No ambient `FARHELM_*` names were present; the full selection
supplied recorder-owned `FARHELM_TEST_TRACE_DIR` and `FARHELM_PLAYWRIGHT_POLICY_FILE`, and generic reproduction supplied
only the trace name. The October 8 entry already establishes this timeout on clean main and tracks the associated
cleanup cascade. The new fixture close can restore configuration after its own trash-cleanup failure, but an earlier
session-cleanup exception can still skip calling it. Cause remains unknown; the existing Deflake TODO stays open, and no
test deadline was changed. Full traces and run records remain privately retained on the executing machine without hosted
expiry.

```text
Test timeout of 60000ms exceeded.
```

Class: unknown

Cause: unknown

## 2026-10-09 — `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay` (crates/farhelm/tests/e2e)

The deflake sweep's workspace nextest phase (run `cba04f06-39b5-4fd9-bfa7-3b2dcb1df22c`, retained run
`4d3f498f-ae89-4925-b95f-f313bf716980`) failed this test once on clean commit `42351f3a30de7c4dd807aa517ae91d31d552bc74`
(a dependency-update stack: refreshed Cargo.lock and tooling pins over main), selection `workspace Rust targets`, four
nextest slots, zero retries. It failed on its reset assertion after 11.9s; the three classification reruns
(`24210386-9d75-4b28-a0ca-5ffda7955268`, `b45bc530-d343-458c-8de0-2481a6a8e2ca`,
`61bfb8d8-ccdf-4e54-a7b2-61df5fba317f`), each the test alone, passed in 13.0 to 13.7s. It was the only failure in the
phase up to that point. The machine was shared with other agents' work, at load average about 20 shortly after; memory
pressure and the tests sharing its slot were not recorded. tmux 3.7c, executable SHA256
`f660bd3c43f0708a8580b64bbab53cc799885d37e56b4e70fa44b2b2a36c05d9`, locale `C.UTF-8`; ambient `FARHELM_*` names were
scrubbed and the recorder supplied `FARHELM_TEST_TRACE_DIR`. The trace ring had evicted the early events, so the pause
and resume timing was not extracted. The test's reset check covers everything delivered from the pause through the
10-second drain after resume, so a stall past tmux's `pause-after` window anywhere in that drain would cause a correct
reset; that is a hypothesis, not established. The 2026-09-03 entry for this test was a different failure (an initial
wait timeout, fixed in #357). Full evidence is retained privately on the executing machine. Disposition: open (TODO.md).

```
panicked at crates/farhelm/tests/e2e/terminal_backpressure.rs:1739:5:
a pause lifted inside tmux's pause-after window must not trigger a catch-up reset
```

Class: budget

Cause: hypothesis

## 2026-10-09 — `view-changes-do-not-postpone-a-recovery` (e2e/tests/terminal-reconnect.spec.ts)

The browser-only deflake sweep (run `0ca012d4-81ae-4d46-b75d-54892e9c69b0`, retained run
`a20f2a95-e3d7-4a0a-a340-055ca9547555`, selection `browser suite, both engines`, one browser worker, zero retries)
failed this test once in WebKit (`webkit-terminal-reconnect`) on clean commit `31200f2c2502ff3f62edda30c2f8810fbb04662b`
(a dependency-update stack over main, including Playwright 1.64.0 with its WebKit build, newer than the 1.62.0 the suite
ran on before). It failed fast on its timing assertion: the recovery landed 4185 ms after the socket was closed, against
a 4000 ms bound with a single 1.5-second reconnect rung. The three classification reruns
(`b421c4bd-45b0-49d5-a970-9ba0b6323efe`, `a30e12c5-bc71-48df-be05-71c954456dfa`, `85cd1460-b130-4ada-9dc9-ecd6450dd513`)
passed. The Chromium case passed in the same run. The same battery also failed two tests deterministically for
unrelated, now-fixed reasons (`buttons.spec.ts` and `composer-word-search.spec.ts`). The machine was shared with other
agents' work, at load average about 16 shortly after; memory pressure was not recorded. tmux 3.7c, executable SHA256
`f660bd3c43f0708a8580b64bbab53cc799885d37e56b4e70fa44b2b2a36c05d9`, locale `C.UTF-8`; ambient `FARHELM_*` names were
scrubbed and the recorder supplied `FARHELM_TEST_TRACE_DIR`. The bound is wall-clock time taken on the test's side and
spans four view changes with 250 ms pauses, so test-side latency under load counts against it; that the recovery was not
actually postponed is a hypothesis, not established, since the trace was not read. No earlier entry exists for this
test. Full evidence is retained privately on the executing machine. Disposition: open (TODO.md).

```
Error: the recovery must land on the schedule it was given, not on one the view kept resetting
expect(received).toBeLessThan(expected)
Expected: < 4000
Received:   4185
```

Class: budget

Cause: hypothesis

## 2026-10-10 — `G2-G4 (pre /latest): nothing but /latest was requested` (scripts/test-install-sh.sh)

The first attempt of an on-demand CI run on GitHub (run 38061432384, job `install-script`, job id 114240425122) failed
this check in the Alpine/BusyBox portability step (`alpine:3` container on the hosted runner): after the installer
refused a prerelease `/latest`, the fixture server's log held two `"GET ` lines for the case where exactly one was
expected. It was the only failure of 509 checks and failed fast, not on a timeout. The tested commit was a release-gate
rehearsal commit on top of a dependency-update stack whose changes do not touch `install.sh` or the harness. The same
check passed in the GNU leg of the same job, in a rerun of the failed job on the same commit (attempt 2), in the first
CI run of that stack the previous night, and in two local `alpine:3` runs. The Alpine image's exact version, tmux
identity (not used by this suite) and runner load are unavailable. The check prints only the count, not the counted
lines, so which extra request was logged is unknown. The fixture server writes each request's log line before it sends
the response, which argues against a late line from the preceding case being counted in this one; nothing else was
established. No earlier entry exists for this check. The hosted job log expires with GitHub's retention. Disposition:
open (TODO.md).

```
NOT OK - G2-G4 (pre /latest): nothing but /latest was requested
    condition failed: [ 2 -eq 1 ]
509 checks, 1 failed
```

Class: unknown

Cause: unknown
