## What this was about

Launching a session that checks out a GitHub repository onto a suspended Tensorlake sandbox wakes the sandbox first. The
launch then waits until the window's own host list shows the woken sandbox connected, gets a checkout preview from it,
and creates the session, all while the session launcher stays hidden. The landing review of nonblocking-session-actions
(the change that frees the page while sessions launch, released in v0.38.0) found two ways this could leave the page
stuck, and they landed unfixed as narrow cases:

- A quick launch (or a template launched by its number key) submits a second time by itself once the wake is done and
  the window has the preview. When that second submit was refused without an error message (for example because the host
  was no longer available for launch), the launcher stayed invisible but in charge of the page. Every click and key went
  to it until you pressed Escape.
- If the window never saw the woken host connect, the launch waited with no limit. Meanwhile New, quick launches and
  adding, editing or removing hosts stayed refused in that window, and nothing could cancel the hidden launch.

When planning, you chose a 60-second limit on only the wait for this window to see the woken host's connection (not on
the wake's own request, and not on the whole hidden launch including the checkout preview and the create), with the
exact error "The host resumed, but this window did not see it connect within 60 seconds. The session was not launched;
try again."

Both are fixed. A refused second submit now shows the launcher with its draft and the reason it was refused ("Not
launched. Choose a connected host before launching" in the case the test stages). The wait to see the woken host connect
now ends after 60 seconds. The launch is dropped, the launcher comes back with its draft and your error, and New, quick
launches and host changes work again. A connection that shows up after that creates nothing. A launch that saw the
connection within the 60 seconds goes on as before, and the waits after it (the checkout preview and the create) keep
their own deadlines. SPEC.md "Creation" and SPEC_impl.md ("GUI: Dioxus" and "Tensorlake hosts") now state the limit.
Each fix has its own changelog entry. The fragment the plan said to edit had already gone out in the v0.38.0 notes, so
these are new "fixed" entries as the plan's fallback said.

## Things you should know

- The error reads twice as "not launched". The launcher puts "Not launched." in front of every refusal, so the alert
  says "Not launched. The host resumed, but this window did not see it connect within 60 seconds. The session was not
  launched; try again." I kept your wording exactly.
- The 60-second limit runs once per wake. If the window sees the host connect and the host then shows Connecting again
  before the launch has its checkout preview, that second wait has no limit, as before, and the window is stuck the way
  the original bug left it: the launch stays hidden, New, quick launches and host changes are refused, and nothing
  cancels it. The reviewer found this case. It needs the preview to still be loading when the window sees the
  connection, and the host to show Connecting again before the preview finishes. SPEC_impl.md says so rather than
  claiming the wait is always bounded. I read acceptance criterion 2 ("the page's wait for its own host list to show the
  woken host's new connection lasts at most 60 seconds") as met: the wait until the window first shows the connection is
  bounded. The later drop back to Connecting is a new reconnect that behaved the same way before this plan. If you read
  the criterion as covering it too, that is the follow-up below.
- A host that is suspended again while the launch waits to see it connect still ends in the same 60-second error. That
  error says the window did not see it connect, not that the host was suspended. That is as planned; nothing reports the
  suspension faster.
- The launcher now carries a small marker in the page saying whether a launch is in this wait
  (`data-awaiting-woken-connection`, next to the launcher's other diagnostic attributes). The browser test reads it
  before it moves the page's clock. The same check decides the timeout, so the test and the product cannot disagree
  about it.
- The browser test shows that the limit spares a launch that already saw the connection by holding its create past the
  60-second mark. It does not hold the checkout preview, because the window asks for that preview while the host still
  shows Resuming. That preview's own 60-second deadline would then run out at the same moment as the limit, and a launch
  dropped because its preview timed out looks the same as one the limit dropped, so the test could not tell whether the
  limit spared it. A unit test covers the case of a launch still waiting for its preview.
- Only one of the refusals that come without an error message is staged in a browser test: the host stopping being
  available for launch. The fix sits where all of them end up. In one very unlikely case the launcher comes back
  visible, showing its settings still loading, but with no refusal message: when it was opened with a template or a
  Clone or Replace draft whose settings are still loading at the moment the second submit runs. Before this fix it
  stayed invisible in that case too.
- The two test builds that disabled each fix failed the new browser tests on both engines. So the tests catch both bugs
  and do not pass for some unrelated reason.

## Open questions and possible follow-ups

- The timeout error repeats "not launched". The options: drop "The session was not launched;" from the message, so it
  reads "Not launched. The host resumed, but this window did not see it connect within 60 seconds; try again."; leave
  the "Not launched." prefix off this one error, which would make it the only refusal without it; or keep it as it is. I
  recommend the first: it removes one clause from one message, and the alert still says plainly that nothing was
  launched. The wake's own failure, which can come earlier, reads "Resuming the host failed: The host resumed but did
  not connect within 60 seconds; try again when it connects"; the two now sit side by side, one about the host and one
  about this window.
- The narrow case where the host shows Connecting again after the launch saw it connected still leaves the window stuck
  as described above. Bounding it would need the timer to remember that the limit already passed and discard the launch
  when it goes back to waiting. I recommend leaving it unless it is seen in practice: it needs the connection to drop
  while the launch's checkout preview is still loading, and the fix is a new mechanism.
- The launcher that comes back with its settings still loading and no refusal message, described above. Showing a
  message there would need that one early refusal to be treated like the others, so its message shows. I recommend
  leaving it: the settings would have to still be loading after the whole wake, which normally takes far longer than
  loading them.

## The PRs

- [#3356](https://github.com/scode/farhelm/pull/3356/changes): show a quick launch that was refused without an error
  after waking its host.
- [#3357](https://github.com/scode/farhelm/pull/3357/changes): limit the wait to see a woken host connect to 60 seconds,
  with the spec text, and remove this plan's TODO.md entry.

## Checks run, reused and skipped

Browser runs went through the test-run recorder's browser mode (Chromium and WebKit, three workers, no retries), with
the selection `npx playwright test 'tensorlake-lifecycle\.spec\.ts' -g '<test names>'`. Unit tests went through its
nextest mode, `cargo nextest run -p farhelm-ui --lib -E 'test(/^list::create_form::/)'`. Each browser run followed a
fresh web build of the revision it tested.

- New silent-refusal test, with the first fix: 2 of 2 passed (`41177e76-712b-4c70-9c1d-2f26c89716bc`). With the fix
  disabled: failed on both engines because the launcher stayed hidden (`5e2e36b8-dd8b-4758-8908-6a70b9ac3763`), as
  intended. Two earlier runs failed on mistakes in the new test itself: a role query that skips hidden elements, and
  expecting no alert where the launcher shows its prerequisite refusal as one (`ce1ff9b2-51c0-4a99-abec-bf4870523a16`,
  `f80d41c9-6aac-49cf-b876-280c5d8b9051`).
- New timeout tests plus the silent-refusal test, with both fixes: 6 of 6 passed
  (`ca44925d-2800-4dd8-8a82-d1d865c0e61e`). With the timeout's discard disabled: the "connection never seen" case failed
  on both engines because the launcher stayed hidden, and the "seen within the limit" case passed
  (`fdd7823f-4571-49af-bdfb-8395e49c1204`), as intended.
- The whole Tensorlake lifecycle spec file, both engines: 78 of 78 passed (`0cd088c4-f81f-4d94-867c-523970e648fa`). This
  ran on both fixes before the review fixes and before the rebase, and is reused for the final revision as described
  below.
- After the review fixes: the new tests, 6 of 6 passed (`b67d1062-9172-43e1-adb4-dac84e26b22c`). Launcher unit tests:
  122 of 122 passed (`18ab07d6-388f-4d8b-a029-7b302cc5a3d6`).
- After rebasing onto main, which now includes the "Replace with" progress change in the same launcher code, with the
  test stack's own helm and web build rebuilt from the rebased revision: the new tests, the "Replace with" wake test and
  every mode of "a suspended fresh checkout uses one press" passed, 20 of 20 (`f4b29b4c-82a5-4dd7-93d9-0893794c05e9`).
  Launcher unit tests: 122 of 122 passed (`b56c5c76-c503-4de6-888d-ca156da7f3f0`).
- Static checks on the final stack: `cargo fmt --all -- --check`,
  `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `dprint check`, `python3 releasing/check-changelog.py format` and `python -B scripts/check-test-sleeps.py`, all clean.
- Reused: the full Tensorlake lifecycle spec run above, for the parts of that file not re-run on the final revision
  (Suspend and Resume from Hosts and session banners, the suspend confirmation, existing-folder wakes). The review fixes
  after it changed how the timeout's check is shared, the tests and wording, not what the launcher does. The rebase
  brought in the "Replace with" change, which touches only the create step after a wake; the final run re-ran every test
  of that step (the "Replace with" wake test and all modes of the one-press fresh checkout) and the new tests.
- Skipped: the rest of the browser suite and the Rust workspace suites. Besides documentation (SPEC.md, SPEC_impl.md,
  the changelog entries and TODO.md), only the launcher's web code, its unit tests and the Tensorlake lifecycle spec
  changed, and both fixes act only after a launch has woken a host, which is what that spec file exercises. Other specs
  that drive the launcher (quick launch, launch progress, the create form) never wake a host.

## Review gate

Each PR had one fresh-context review by Opus 5.5 at high effort, as the plan required.

- #3356: no bugs. Two comment corrections were applied. I chose not to act on two notes: the unlikely unexplained return
  above, and staging only one silent refusal.
- #3357: no blocking bug. Applied: one shared helper behind both the timeout and the page marker, with unit cases for a
  launch that already saw its connection; the softened SPEC_impl.md sentence; a repeated "nothing was created" check at
  the end of the test; a corrected test comment; and a unit test that the error's "60 seconds" matches the timer. Not
  applied: remembering that the limit passed (the narrow case above).
