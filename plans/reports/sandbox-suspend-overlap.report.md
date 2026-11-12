## What this was about

Suspending or resuming a Tensorlake sandbox used to lock the whole browser window until Tensorlake answered. That was
true from Hosts, from a sandbox session's row menu or the top of an open session, and through "Stop and suspend". For
that long the window refused every session launch on every host and every change to the host list, and a Resume can take
up to two and a half minutes. When session actions stopped blocking the window, this one lock was deliberately kept, and
that call went onto TODO.md's "Plan decisions to revisit" list. On the 2026-11-11 planning page you chose to let them
overlap, in two parts:

- **The host-scoped rule.** A Suspend or Resume, wherever it is started, is refused only while a launch in the same
  window is under way on that same host.
- **The wake hardening.** The overlap makes two races more likely, so a launch that has to wake its host should handle
  them. First, its Resume can be refused with "This host is busy" because another Suspend, Resume or host operation is
  still running. Second, the host can be suspended again while the launch waits for it to connect. That used to leave
  the launch waiting a full minute and then failing with a timeout that blamed the connection.

The plan blocked twice, both times on the size limit you set (about 400 changed lines). The landing monitor answered
both times on your behalf. The first time it raised the limit to about 1,000 lines. The second time it accepted the
finished stack at about 1,215 lines, roughly 600 per PR, not counting the changelog entry and the TODO.md edit. Both
answers are on TODO.md's "Plan decisions to revisit" list. Nothing waits on them; if you disagree, the remedy is a
follow-up that trims or splits the work.

What now lands:

- **Other hosts are not held up.** Launches on other hosts, and adding, editing, setting up, updating or removing other
  hosts, go ahead while a Suspend or Resume runs.
- **Same-host refusal.** Suspend and Resume are greyed out and refused for a host while a launch in this window is under
  way on it. That applies in Hosts, on a session's row and at the top of an open session, and to "Stop and suspend" in
  the Stop question asked from a session's row and from the top of an open session. The launch counts from the moment
  the user submits the launch form and the host has been checked as available, through waking the host if it was
  suspended, until the session is created or the launch fails. The refusal is checked when the button is clicked and
  again when a confirmation question is answered.
- **Unchanged:** adding, editing and removing hosts still wait for any launch in the window, as before.
- **Wake: busy retry.** If the launch's Resume is refused only because another operation is using the host, it is sent
  again every second. The launch carries on as soon as that operation ends, or as soon as someone else's Resume brings
  the host up. Any other refusal, such as a host that is not a sandbox or a missing API key, still fails the launch at
  once.
- **Wake: time budget.** All this waiting comes out of the one minute the launch already allowed for the host to
  connect. Only the time the accepted Resume request itself takes is left out, so a slow restore of up to two and a half
  minutes is never cut off.
- **Wake: suspended again.** If the host is suspended again while the launch waits for it, the launch fails at once and
  says the host was suspended again. That holds whoever suspended it: this window, another window or device, or
  Tensorlake's own idle suspend.
- **Paused container hosts.** A launch wakes a paused container host the same way, so the same busy handling applies
  there.

## Things you should know

- **The rule is per window.** Another window or device is not stopped from suspending a host that this window's launch
  is waking. When that happens, this window's launch now fails at once with "suspended again" rather than after a
  minute.
- **How "busy" is recognized.** The helm now labels its "This host is busy" refusal in a way only the helm can set, and
  the browser waits only on that label, never by matching the message text. The text is written for people, and neither
  Tensorlake's own messages nor a remote supervisor's reply should be able to make a launch wait.
- **Container hosts' busy refusal changed.** A busy container host used to fail a Suspend or Resume with a generic
  server error. It now gives the same "This host is busy" refusal a sandbox gives, which is what lets the launch wait it
  out.
- **What counts as "suspended again".** The launch starts from whatever suspension the host list showed when it began,
  if any. It treats the host as suspended again when the list shows a suspension with a later suspension time, any
  suspension when the host was not suspended at the start, or a suspension after the list had already shown the host
  connecting or connected. The list's "suspending" and "resuming" states are ignored. When a Resume finishes, the helm
  drops "resuming" just before the host starts connecting, and for that moment the list shows the original suspension
  again, unchanged. Counting "resuming" as progress, as the plan first suggested, would have failed every such launch. A
  browser test covers this stale reading.
- **Launches that create a new sandbox or container do not grey out Suspend or Resume anywhere.** The host does not
  exist until the launch creates it, and its session's controls appear only after that.
- **One request per host from Hosts.** Hosts now also refuses a second Suspend or Resume of a host that already has one
  in flight from this window. The window lock used to keep those apart.
- **A Suspend refused while you answer its question.** If a launch on the same host starts while the Hosts Suspend
  confirmation is open, answering it shows the refusal on the host's row instead of doing nothing.
- **Two tests the plan called for were done differently:**
  - No browser test shows a session's own Suspend refused during a same-host launch. The only launches that can target a
    session's own host are its "Restart with" and "Replace with", and those also lock the session itself, so a test
    could not tell the two refusals apart. A unit test of how the launch records its host and the Hosts browser test
    (Resume refused while a launch wakes the host) cover the rule instead.
  - The test that host changes still wait for a launch checks that a host's Settings are unavailable during the launch.
    It does not exercise a host software update.
- **Test fixture change.** The fake Tensorlake service in the browser tests used to report a new suspension time on
  every read. No real helm does that, and the new check would read it as a fresh suspension. Its suspension times are
  now stable.

## Open questions and possible follow-ups

- **Container busy refusal untested.** Sandboxes and container hosts produce the busy refusal through one shared step in
  the helm, but only the sandbox's is exercised by a test. A helm test for a busy container host would close that gap.
- **Two declined review suggestions**, both judged small against a stack already over its size limit:
  - A browser test for the race where a launch starts while the Hosts Suspend confirmation is open. The disabled button
    is backed by the same check when the answer is acted on.
  - A separate unit test for the wake's time budget. Its rule is one line and explained where it lives.
- **Spec wording.** One older sentence in the implementation spec still says the window lock excludes "host mutations".
  The paragraph these PRs added there says Suspend and Resume are not changes to the host list and never take the lock,
  so the meaning is clear. The older wording could be aligned in a later docs pass.

## PRs

- #3339 fix: let sandbox Suspend and Resume run beside launches on other hosts (the host-scoped rule).
- #3341 fix: let a launch wake its host while other Suspend or Resume work runs (the wake hardening; removes the TODO.md
  entry).

## Checks

Run now, on the final stack after rebasing onto current main:

- **Browser:**
  `record-test-run.py --runner playwright -- npx playwright test tests/tensorlake-lifecycle.spec.ts tests/tensorlake-sandbox-sessions.spec.ts`,
  Chromium and WebKit, run d86ffc4c: 92 passed. This covers every new browser test of both PRs and the existing tests of
  both spec files.
- **Unit tests:**
  `record-test-run.py --runner nextest -- cargo nextest run -p farhelm-helm -p farhelm-ui --lib -E 'test(/lifecycle_/) | test(/http_error_/) | test(/wake_watch_tests::/) | test(/^ops::/) | test(/session_ops::/) | test(/sandbox_sessions::/) | test(/^hosts::/)'`,
  run fc3ebfa7: 190 passed. This includes the helm test that Suspend and Resume busy refusals carry the busy label and
  other refusals do not, and the wake's suspended-again cases.
- **Static checks:** `cargo fmt --all -- --check`;
  `cargo clippy -p farhelm-helm -p farhelm-ui -p farhelm-proto --all-targets -- -D warnings`; `check-test-sleeps.py`;
  `releasing/check-changelog.py format`; `dprint check`. All passed.

Reused: none. Every check above ran on the final code.

Earlier observation: after the second PR's review fixes, browser run 7ba8210f had 71 passed and 1 failed. The failing
test, on WebKit only, was "another surface suspending the host does not make the launcher available". It timed out
clicking the launcher's Command tab, before any launch was submitted, in code these PRs do not change, while the shared
machine was heavily loaded. That test alone, repeated ten times on WebKit, passed every time (run 2c4972c1), and it
passed again in run d86ffc4c. It was not logged as a flake on one observation.

Skipped:

- **The full browser suite and the workspace Rust tests.** The change is confined to the browser's Suspend and Resume
  controls, the launch's wake and the helm's busy refusal, which the selected specs and unit tests cover.
- **Real Tensorlake and container calls.** The browser tests use the suite's fake Tensorlake service; no real sandbox or
  container was suspended or resumed.

## Review gate

A fresh gpt-6.1-sol reviewer at high effort, once per PR:

- **#3339:** found the size-limit excess and one real bug: the Hosts Suspend confirmation stayed clickable during a
  same-host launch. The bug is fixed. The suggested race test was declined, as above.
- **#3341:** found two problems, both fixed:
  - Time spent on busy refusals was not counted against the minute, so a slow helm could stretch the wait well past it.
    Now only the accepted Resume's own time is left out.
  - The new browser tests could pass even if the session was never created. They now require the real create to succeed.

  The suggested time-budget unit test was declined, as above.

Neither PR was reviewed again after its fixes; the fixed code is covered by the checks above.

### Landing

#### What landed with it

Nothing else landed in this round. Since the plan's work was started from main, main gained only the v0.38.0 release
notes (#3344), which fold the released changelog fragments into CHANGELOG.md; this plan's own fragment is new and
unaffected. Both PRs rebased with no change.

#### Fixes made while landing

The landing's review and an independent one found nothing that had to block, and two things worth fixing, both in #3341
(the wake hardening). A second independent review checked the fixes.

- First fix: a launch waiting out "this host is busy" could still end at once with "Tensorlake could not resume this
  host" when the host already carried a failed Resume from earlier. The wake read that old failure, still shown while
  its own Resume was refused, as its own. The case is a host whose last Resume failed, where the user presses Resume in
  Hosts and then launches on it before the list shows it resuming. The launch now ignores a failure the host already
  showed when it started, and fails only on one that appears later. A unit test pins it. Before this plan the same
  launch failed at once with "busy", so this was a gap in the new waiting, not a regression.
- First fix, then a second wording pass after the second review: the report, SPEC_impl.md and the code explained the
  wake's tolerance of the suspension it started from by saying the helm briefly shows the old suspension again right
  after a successful Resume. The helm does not do that: after a successful Resume it shows "resuming" until the
  connection reports "connecting". The tolerance is still needed, for other reasons. The old suspension keeps showing
  while the launch's own Resume is refused as busy behind an update or an edit, and a second Suspend of a host Farhelm
  had already suspended publishes the same suspension again. The explanations, the two unit tests that model it, and the
  browser test's description now say so. Behavior is unchanged. The report's "What counts as suspended again" item is
  wrong on this point.

#### Possible follow-ups

- When another window's Resume of the same host fails while a launch waits for it, the launch fails at once (as it
  should) but says the host was "suspended again" rather than that the Resume failed. Only the wording is off.
- A Resume running elsewhere can take up to two minutes, and a launch waits at most one minute for it, then fails with
  "stayed busy". The plan chose that budget; it covers the common case, not a slow restore started elsewhere.
- SPEC.md's new sentence says Suspend and Resume are refused "only" while this window's launch targets the host. Hosts
  also refuses them while that host already has one of its own requests pending. The wording is narrower than the code.
- From one window, a Suspend from a session's row and a host operation in Hosts on the same host can now both start; the
  helm's own lock refuses the second with "This host is busy" instead of the page refusing it up front. Nothing ends up
  in a wrong state.
- The browser test for the busy case serves both the busy text and the helm's busy label, so it cannot show that the
  page waits on the label alone. The unit tests and the code do pin that, but no browser test checks that a busy-looking
  refusal without the label is not retried.
- An older browser test, "a suspended session banner resumes its host" (WebKit), failed once in the landing's narrow run
  on this plan's build. Repeated 60 times on each build, it failed 4 times on main without this plan and once with it,
  every time with the test's fake service answering a request after the test had ended. That is an existing flake,
  recorded by the landing monitor right after this landing. One more failure on this plan's build, in 20 repeats, was
  different: the banner still said it was checking the host when the test gave up waiting for "suspended". It was seen
  once, and never on main's build in 80 runs, so whether this plan's change to the test's fake sandbox host has anything
  to do with it is unknown; a recurrence of that symptom is worth a narrow repeat run.

#### Checks

Browser runs were on Tensorlake sandboxes at the maintainer's request. The sandboxes run Node 24.16 where the
development machine runs 24.20, which these tests do not depend on, and each shard is recorded in the recorder's generic
mode.

On the stack as delivered (rebased onto main with no change):

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings` and `cargo check -p farhelm-ui --features desktop` passed.
- `cargo nextest run -p farhelm-helm -p farhelm-proto -p farhelm-ui --lib`: 2,972 passed (run
  `f455696a-441b-415e-8c3f-3b4d519cd1cc`).
- The whole browser suite, `npx playwright test` split by project over ten sandboxes, because the window lock this plan
  changes is on every launch path and the executor ran only the two Tensorlake spec files: 3,295 passed, 14 skipped and
  1 failed (runs `5f49c83a-591a-4a4c-9793-34f2503562a6`, `334cf0e4-589f-4440-962a-3fe8ab174f21`,
  `5d231295-136e-4c71-b98e-1757274172d2`, `04d78580-8229-49a7-8650-1fa16d5410e8`,
  `d1615bcb-319a-4729-bef4-dc0ebe41907e`, `8285fdb3-a27f-4990-ad6c-147d203131e3`,
  `4c410e4e-0ae3-4096-b769-132b57fc3e89`, `0d4a1650-9e5c-4a02-979b-d4bf61ea0dda`, `c76120c5-0984-4ca3-a456-eb982650f832`
  and `278212da-f5d8-42f4-b06c-9a416608ab58`). The failure was Chromium's stop-tabs
  `bulk Stop carries each target's tab precondition with skip false`, a flake already recorded (FLAKES.md, 2026-11-11).
  The 14 skips are as many as in earlier whole-suite runs.

After the landing fixes (the final tip):

- `cargo fmt --all -- --check`, the test-sleep check, `dprint check` and
  `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed.
- `cargo nextest run -p farhelm-ui --lib`: 969 passed (run `a48e3843-23e4-4664-98e6-bc5b7ba8904d`), including the new
  wake test.
- The Tensorlake lifecycle and sandbox-session spec files, whole, on both engines, from a fresh build of the first fix
  (the second pass changed only comments and unit-test inputs): 91 passed and 1 failed (runs
  `9589cbd3-17bf-44bf-afc1-f4714fa27a5a`, `1d76cd94-53e7-4aca-b431-b221853f5d32`, `191f0ad0-4701-4b31-b724-4d8a66d566d0`
  and `e93044f0-1a73-41de-85e7-3093c143754f`). The failure was WebKit's `a suspended session banner resumes its host`,
  investigated with repeats of that test alone on WebKit: 19 of 20 and 59 of 60 passed on this build (runs
  `9ddf3910-4fc9-46c3-ad35-a696c47c1856` and `bdfb7d00-7f84-4adf-8b00-23f6aa2ca134`), against 20 of 20 and 56 of 60 on
  main's build without this plan (runs `b523d0d4-6107-4f0b-aa6b-077720d84d01` and
  `b0544367-4b54-4044-9006-b59410204249`); see the follow-ups.

Skipped: the whole browser suite after the fixes (they change only the wake's handling of an old failure, comments and
unit-test inputs, which the two Tensorlake spec files and the UI tests cover), the workspace Rust tests beyond the three
changed crates' library tests (the helm change is its labelled busy refusal, which those cover; nothing else in the
workspace reads it), the old-terminal browser set (nothing here touches old terminals), and real Tensorlake or container
calls.
