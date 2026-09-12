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

After the paragraph, add `Class: <class>` and `Cause: <confidence>` as separate paragraphs. Classes are `readiness`,
`replay-live`, `fixture-premise`, `peer-lifecycle`, `process-interference`, `budget`, `ambiguous-observable`,
`pointer-focus`, `substrate`, `product`, `deterministic-regression`, or `unknown`. Confidence is `established`,
`hypothesis`, or `unknown`; `Cause:` is the last line. These fields apply only to new entries. Missing historical fields
are missing evidence, not an implicit cause classification.

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

## 2026-09-03 — `the sidebar app bar shows the helm build and client tooltip` (e2e/tests/sidebar.spec.ts), localized

Seen on `[webkit-sidebar]` during a full four-project sidebar/terminal-multihost run (`--repeat-each=3`) on a 4-vCPU
sandbox for PR #363 (the sidebar locality-glyph change): `Error: route.fulfill: Route is already handled!` inside
`forceBuildSkew` (`helpers/fleet.ts`), which rewrites every `**/api/**` reply's build-stamp header (`route.fetch()` then
`route.fulfill({ response, headers })`). Neither this test nor `fleet.ts` was touched by PR #363. `forceBuildSkew` has
four callers in the target snapshot — three in `feed.spec.ts` and this sidebar test — not the broader set of every test
that reads a build stamp. Reproduced alone:
`--project=webkit-sidebar -g "the sidebar app
bar shows the helm build and client tooltip" --repeat-each=20`, single
worker, no other test in play, failed 1 of 20 with the identical error. The shape reads as the route handler racing
itself — WebKit re-issuing or re-dispatching the intercepted request so the handler runs twice concurrently for one
navigation, and the second `fulfill` finds the route already answered. Not chased further: no hypothesis yet for why
WebKit produces the second dispatch, and the other three `forceBuildSkew` callers (`feed.spec.ts`) passed throughout
both runs, so whatever triggers it is rare and not obviously tied to this one test's own body. Disposition: open
(TODO.md).

## 2026-09-03 — `launch_sentinel_error_status::a_planted_malformed_spec_sentinel_classifies_error_with_its_detail` (crates/farhelm/tests/e2e/launch_sentinel_error_status.rs)

Seen during the host-alias feature's finishing-work run on a 4-vCPU sandbox:
`cargo test -- --show-output
--test-threads=4` failed this one test (338 passed, 1 failed) with "a consumed sentinel is
deleted once its Error outcome commits durably". The launch-sentinel and supervisor code this test exercises was
untouched by that work. Reproduced alone immediately after —
`cargo test -p farhelm --test e2e
launch_sentinel_error_status::a_planted_malformed_spec_sentinel_classifies_error_with_its_detail -- --exact
--show-output --test-threads=1`
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
`right:
["/tmp/.tmpHAvtBc/fh-it.live01"]`. That is the test's SECOND assertion — after the test drops its own flock,
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

`a client that stops draining is detached with the stall reason after the full stall interval; reattaching afterward
replays`,
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
as a same-substrate baseline. This bookkeeping correction does not claim a new reproduction or a fix.

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
full-run failures therefore do not establish lost Farhelm input. Disposition: open fixture portability follow-up in
TODO.md; require a live-output dumper without weakening the complete byte-sequence oracle.

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
provenance. Disposition: open in TODO.md; retain the original failure separately from the stale-credential cleanup
consequence.

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
carries network, DOM, and console. Disposition: still open in TODO.md.

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
Disposition: still open; the TODO entry is left to the maintainer.

Class: budget

Cause: hypothesis

## 2026-09-12 — popup disable-blur dismissal race fixed (e2e/tests/profiles.spec.ts, crates/farhelm-ui/src/app_bar.rs)

Chromium run `45efb275-84b9-4aab-9dd2-550fd45d4e7a` left the popup mounted after the toggle close in
`a popup-created
profile is offered on every host` with the profile registered server-side. Batch
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
(`disabling the focused save control does not
dismiss the popup`) now scripts the disable-blur directly: run
`d08fc2fa-9de0-4c9c-9474-fa8fb0c34cd6` fails it against the pre-fix bundle (the popup unmounts inside the observation
window), and batch `23367a9f-dc59-4761-ac1d-3b4ab0c2bee1` passes it twice per engine against the fixed one. Disposition:
fixed in this PR; the TODO.md entry is removed.

Class: product

Cause: established

## 2026-09-12 — `composer path actions keep typing inert and browse the selected remote host` never matched the remote log receipt (e2e/tests/sidebar.spec.ts)

The test's final predicate polls the remote supervisor's log for `received directory browse request cwd=<path>` and
timed out on every attempt ever recorded: Chromium run `1024be47-ca56-4651-afd6-a56d01a5c890`, then runs
`2489c509-e3e3-4e9d-b6b5-f860a7fbd565` and `ce1a9bec-cf6b-453b-bf1b-61f3e4ec297d` in both engines, then five isolated
reproductions on the current tree and three on the pre-composer baseline. The retained failure evidence left three
hypotheses: the log never flushed, the test read a different path than the supervisor wrote, or the browse was never
forwarded. A reproduction with a temporary dump of the file's tail disproved all three: the receipt was present in the
exact file the test reads, written immediately, with the routing it asserts — but tracing decorates the field with ANSI
SGR sequences, so the raw bytes read `received directory browse request ESC[3mcwd ESC[0m ESC[2m= ESC[0m/tmp/...` and the
bare `request cwd=` needle could never match on any substrate with ANSI-enabled tracing. Forwarding itself was
independently verified: a manual browse-directory call against the same stack produced the same receipt in the same
file. The predicate now strips SGR sequences before matching; three exact repetitions passed on the fixed tree (the same
selection failed 100% before). Disposition: fixed in this PR; the TODO.md entry is removed.

Class: fixture-premise

Cause: established
