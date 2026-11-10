## Since the last review

Both existing draft PRs, #3276 and #3278, were amended. No PRs were added, dropped or merged. The landing review found
that opening Rename during a launch could discard its result and leave progress counting forever. Rename now keeps that
launch's identity, and the launch's result waits for Rename to close. Retired launcher openings also settle their
pending progress and the source session's Restart or Replace state.

Tests that expected the old locked page now check the submitted launcher's hidden, busy state. Some failures described
as stale were valid checks of a returned dialog: cancelling a YOLO question must return to search, and a refused Restart
must return to its primary action. Those focus regressions were reproduced and fixed while preserving the assertions.
Local YOLO fixtures now explicitly choose the host whose policy they control. A Replace test now selects its source
before expecting the replacement to be selected.

The fresh reviews found three further races, which were fixed before delivery. A listing can see Replace's source gone
before its successful reply reaches the browser; that refresh now preserves the source selection until the reply, while
a deliberate switch still wins. A failed request to stop asking about YOLO stays with its source if another session's
question has appeared. A retired launcher's late reply removes only its own launcher and cannot reveal it over another
dialog or close a newer draft.

Session-facing sandbox Suspend and Resume again exclude pending launches and host changes, using the same exclusion as
the Hosts panel. Stop and suspend keeps it through both operations; ordinary Stop remains independent. This follows the
landing monitor's recorded judgment. The row's Stopping status, Replace's YOLO answers, older test comments and the
changelog fragment were also corrected.

## What this was about

Stopping, restarting or replacing a session could make you wait before opening another session because switching would
tear down the work waiting for its reply. Starting a session also kept the launcher over the page for the whole wait.
The requested change keeps other sessions usable while each action retains its own result and retry state.

Pending session actions allow sidebar and quick-switcher navigation, marking and actions on other sessions. The acted-on
session still refuses competing actions. Stop shows a spinner and **Stopping…** on its row and at the top of the open
session. Failures, unknown outcomes and partial results remain on the source row and appear at the top when you return.
Switching away no longer cancels the request waiting for that result.

## Things you should know

Submitted launches, including Restart with and Replace with, hide the launcher and show a progress pill after about two
seconds. You can switch sessions and type in their terminals. A failure, unknown outcome or question brings back the
same draft; it waits if another dialog is open. The launcher cannot start a second request or accept another draft while
busy. Switching to another host does not change the submitted destination or its retry key. Preparation and unanswered
confirmations remain modal. Launches and host changes still exclude each other.

A successful New or Clone still opens its new session. Replace and Replace with open the replacement only while the
original remains selected; otherwise it appears in the list while you stay where you are. Automatic selection after
Delete waits for every admitted Delete, including queued bulk rows. Pending Stop batches do not delay it.

The implementation retains the existing request owners and exclusions. It adds no backend acknowledgement, deadline
change, operation queue or general modal framework. The original TODO entry is removed in the final PR.

## Open questions and possible follow-ups

No open design decision. One narrow WebKit run failed the Trash dialog's keyboard traversal at desktop width; a later
run passed both widths on both engines with the same product source. Its cause is unestablished. The earlier failure is
retained, and its assertions and product behavior were left unchanged. The final broader run also passed both widths on
both engines. These observations do not establish the earlier failure's cause.

Browser validation uses Chromium and WebKit with real local and SSH fixtures. Tensorlake actions use the controlled
presentation fixtures; these checks establish neither native macOS desktop-shell nor real cloud lifecycle coverage.

## PRs

- [#3276: keep session actions from blocking navigation](https://github.com/scode/farhelm/pull/3276/changes).
- [#3278: free the page while sessions launch](https://github.com/scode/farhelm/pull/3278/changes).

## Checks run, reused and skipped

The required 23-file browser run finished with 1,056 passed and eight failed in 43.9 minutes, recorder
`4cba187d-1c4f-4b1d-946c-284fbd2ce434`. Each engine ran 532 cases: 528 passed and four failed. The command exited 1. All
eight failures are the main-caused reader-count and host-settings footer mismatches identified in the landing decision
and covered by [the separate draft fix #3275](https://github.com/scode/farhelm/pull/3275/changes). Its complete diff
matches these failures; that fix remains separate from this stack. No other case failed.

NOTE: The recorder's automatic report collection is incomplete: the raw JSON is 19,338,808 bytes, above its 16 MiB
inspection limit. The counts above were independently checked against the complete raw report and console, rather than
supplied by the recorder's count validator. Raw results show one execution per case, zero retries, no skips, flaky
cases, timeouts, interrupted or unstarted cases, or global errors. The completed policy record confirms three workers,
zero retries and one execution. The source record is complete at published tip
`de483c273927a582587ab4cadeaa0816fcef9ef0`, fingerprint
`120d97cab68c6ebb861483a89f2809070d25548d0c9864d3cc5d9457cd3dc025`. Console retention has EOF, no truncation or runtime
skip marker, no forced cleanup, and all three stack cleanup witnesses. The whole failed run is retained.

The all-spec source survey found no additional obsolete pending-launch modal expectations. The final selection includes
the original eleven files and every file named by the landing decision, plus the touched session-claims spec and the
monitor-recommended queued-Cancel Restart case: 23 spec files on both engines. The child command, from `e2e`, is:

```sh
npx playwright test 'tests/(terminal|quick-switcher|sidebar|header|bulk-session-actions|stop-tabs|replace|tensorlake-sandbox-sessions|launch-progress|nonblocking-launch|restart-replace-progress|restart-with|yolo-guard|tensorlake-lifecycle|chrome|launcher-template-shortcuts|session-confirmations|terminal-multihost|trash-dialog|tensorlake-recipes|feed|session-view-claims|terminal-restart)\.spec\.ts'
```

The final focused browser run passed all 26 cases, 13 per engine, in recorder `c5566106-ebe6-4e8f-a13a-80ffcd734427`. It
covers Rename and actual denied-return settlement, queued Templates admission, repeated Restart focus, and both Replace
paths with a committed source-removal refresh before the held reply. Replace's success cases use actual backend
replacements; its refusal cases use controlled replies after checked external deletion. Output, source, policy and
reports are complete, with EOF, no truncation or forced cleanup and all three stack cleanup witnesses.

The full UI Rust suite passed all 942 tests with no skips in recorder `3a87c41f-0730-4f2f-ac98-82d631d10c8e`, at
fingerprint `624c554af404e31c1beeaf1f1f40e9ce43cdd50baa926c7d242b6655b1375e91`. That includes sandbox token cancellation
and Stop-and-suspend handoff tests and the existing lifecycle and row-rendering contracts. After the last corrections,
recorder `4ebcc8b8-99cf-49e3-a90e-44119d338027` passed all 13 selected Replace and launcher-retirement tests, with the
other 931 selected out. Both runs have complete source, reports and output, EOF, no truncation or forced cleanup, and no
runtime skips.

The final focused Rust and browser runs share fingerprint
`5facdc6eb1ff451d65f3db64f1f30c6ee95fbaced4c426fd3cb540a593feaed5`. They tested the complete working tree before its
changes were folded into the two existing commits. A full tree comparison proved that published tip
`de483c273927a582587ab4cadeaa0816fcef9ef0` contains exactly that source. The earlier full UI run is reused for unchanged
unit contracts; the last delta's controller, ownership and selection behavior has the focused Rust and browser coverage
above. No aggregate count across these commands is claimed.

`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, formatting,
the instruction-link checker, test-delay checker and changelog-format lint passed. The delay checker inspected 429
delays with no missing rationale. The instruction checker retained one existing triage-record citation warning and no
errors. The pinned web release build passed on the final tested source; its matching bundle is reused after the exact
tree comparison. The first PR also passed its standalone library compilation. The CLI build is reused from the original
base because backend, protocol and CLI sources did not change. The intervening main changes affect queue records, TODO
entries, one historical assessment and review-feedback records; they change no product or test inputs.

The first narrow rerun, `4a2d4352-17c3-4469-bbe5-b7ab870b51eb`, passed 11 of 14 cases. Both engines failed the Replace
test's unselected-source premise, which was corrected. WebKit failed the Trash traversal once. Bulk Stop's engaged-row
case, moved-host intent, unseen-output color and recipe snapshot cases passed on both engines. These are observations
within a failed command, not a passing command. A preceding wrong-directory recorder refusal,
`8e58e7db-2b88-4d45-bb23-f84797d0771e`, started no tests.

The focused pre-fix run `daa8b46e-0606-4381-9e67-d5ef80cb60ee` passed six cases and failed eight: the two returned-focus
contracts above, with three Restart variants, failed on both engines. Rename and all four Trash cases passed. The whole
failed run was retained before fixing focus; later success does not replace it. Earlier-round failures remain retained,
and the historical 584-case pass is not counted as current validation.

The next focused run, `afc02121-58da-4525-9065-d0027af8d2e2`, passed 44 of 46 cases. Two repeated-attempt Restart cases
still focused Cancel in WebKit because its existing body recovery and the returned dialog chose different controls.
Those paths now agree on the primary action after a refused attempt. The final 26-case run above includes both cases;
the earlier failed command remains retained.

Workspace-wide Rust, unrelated browser specs, native desktop runtime, provisioning, installer, release and real cloud
tests were skipped because their behavior is unchanged. No CI run was dispatched; CI is manual and browser CI disabled.

## Review gate outcome

Each amended PR received one fresh `gpt-6.1-sol` review at high effort. All findings were fixed and both correction
reviews are clear. The first PR's standalone partition also passed the same reviewer's focused check. Fresh design
reassessments kept the correction on existing owners and tokens. The unchanged PR messages retain their earlier fresh
cold reads.
