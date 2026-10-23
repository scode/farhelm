## What this was about

Five places where the GUI could act on something other than what the user saw or meant, each a triage outcome you
accepted on 2026-10-22:

1. A session menu or notification list left open while the session list failed to load came back open, unasked, when the
   list recovered, and if the recovery arrived before the error had been drawn it could reappear at its row's old
   position, next to a different session. Now a recovering list closes it.
2. In "Replace with", pressing Enter on a recent setup in the session launcher submits the replace, which stops and
   deletes the session being replaced, but the row said "⏎ launch" and screen readers heard nothing about replacing. Now
   the row says "⏎ replace", its screen-reader name starts with "replace:", and it points at the same "still running"
   warning as the replace button.
3. In a short window (under 80px of terminal height) the outage card that explains a down host or connection covered the
   whole window, sidebar included, so nothing in the sidebar could be clicked. The card now starts after the sidebar and
   its resize handle and still gets the full window height.
4. After a launch whose reply was lost (so it may have started), if the host had meanwhile been set to ask before YOLO
   launches, the YOLO question said "Nothing has been started" and offered "start guarded session instead", which sends
   a new request and could start a second session. Now that question says an earlier attempt may already have started
   the launch and offers no guarded answer. "Start anyway" still retries the original request, which the helm matches to
   the session it may have started.
5. A terminal that finished loading took keyboard focus from any button, including a header button the user moved to
   while it loaded, so an Enter meant for that button could reach the agent. Now it takes focus only from where focus
   was when the session or tab was selected. Select-then-type still works.

## Things you should know

- The guarded answer change (4) amends SPEC.md, Creation, in the same PR (#2655): the launcher no longer offers the
  guarded launch when the refused request retries an attempt whose outcome is unknown. Your triage decision called for
  the behavior; the spec sentence that promised the guarded answer unconditionally needed the exception.
- The guarded answer PR (4) is about 130 changed lines, against the plan's limit of about 100 changed lines per PR, over
  which the plan says to stop and ask you. The fix itself stayed small; the excess is a browser test the review asked
  for (it loses the first launch's reply on the wire and checks the question that follows) plus the spec sentence. I
  judged that within "about" rather than blocking; the outcome's entry in TRIAGE_OUTCOMES.md records the size and the
  reason.
- Focus (5) needed two follow-on changes. First, after a reload the page selects a session by itself, so the user's
  click on that already-selected row would not have counted as the selection, and typing went nowhere; an existing
  browser test caught this. A click on a sidebar row or tab now always counts as selecting, and so do Enter and Space on
  them, which the browser turns into the same click; merely tabbing past them does not. Picking the already-selected
  session from the quick switcher leaves focus on no particular element, and the terminal always takes focus from there.
  Second, a successful "Restart with" used to hand focus back to its own header button, which the restarted terminal
  would now leave alone, so after a successful restart the dialog no longer puts focus anywhere and the restarted
  terminal takes it when it finishes loading (Cancel still returns to the button). That success path has no browser
  test, because the existing "Restart with" tests use a stand-in session with no real terminal.
- "Where the selection left focus" is recorded a moment after the click, once the helm has sent the selected session's
  terminal, not at the click itself. So a user who moves focus to another button within that moment (usually well under
  a second) still loses it to the terminal, as before this fix. Opening a new session depends on that timing, because
  the launcher puts focus on the "new" button just before the new session's view takes over, so closing the gap would
  need its own change.
- Closing session menus on recovery (1) means a Delete or Replace question that was open in a session's menu during a
  failed load is now hidden with the menu. The question is kept: reopening the menu shows it again, and an existing
  browser test now asserts exactly that.
- The outage card (3): in a window too narrow for the sidebar plus the main pane's 320px minimum, the card keeps 320px
  and overlaps the sidebar's edge instead of squeezing the explanation.

## Open questions and possible follow-ups

- In the Mac desktop app's narrow-window mode, the app bar stretches across the whole window, and its settings and help
  buttons sit to the right of the sidebar. A short-window outage card still covers them there, as it did before. A
  possible follow-up: start the card below the app bar in that mode. I could not test it here, so I left it alone.
- The short gap described under "things you should know" (focus moved within a moment of selecting a session) could be
  closed by recording focus at the click and giving the new-session path its own handling. I do not recommend it unless
  it shows up in practice.
- A browser test for focus after a successful "Restart with" would need a real restartable session in that spec. A
  possible follow-up if that path matters to you.
- When the selected terminal changes by itself (a shell tab exits and the view falls back to the agent terminal that is
  already showing), the terminal still takes focus from a plain button the user is on. That is the same risk as (5),
  through a path this outcome did not cover. I recommend this one as a follow-up, since it is the same
  Enter-reaches-the- agent risk.

## The PRs

- #2652 fix: close session menus when the session list recovers
- #2653 fix: say "replace" on recent setups in Replace with
- #2654 fix: keep the sidebar clickable under the short-window outage card
- #2655 fix: offer no guarded launch after a YOLO launch that may have started
- #2656 fix: keep focus a user moved while a terminal was loading

## Checks

All browser runs were on Chromium and WebKit through the test recorder; counts are per test and engine. The final review
fixes landed before the last browser runs; after those runs the only changes were the rebase onto main and the PR links
in TRIAGE_OUTCOMES.md.

Run in this plan:

- Browser, on the stack with every review fix except the focus PR's (#2656):
  - `cd e2e && npx playwright test <popover-header-movement, terminal, replace, terminal-multihost> -g <the changed tests>`:
    run 29d2120f, 10/10 passed.
  - `yolo-guard.spec.ts`, whole file: run ba28200d, 26/26 passed.
- Browser, on the final code before the rebase onto main:
  - quick-switcher, quick-launch, restart-with and terminal-replay-rename, whole files: run 93cb0bdd, 222 tests, 219
    passed, none failed; the 15-minute timeout of my own test wrapper stopped it before the last three.
  - Those three tests plus the multi-host "focus returns when replay precedes fresh detail" test: run 31f5badc, 8/8
    passed.
- Browser, earlier in the plan:
  - The whole popover-header-movement file: run 12709777, 20/20 passed.
  - Run f3f7b269: 10 tests, 6 passed and 4 failed, all failures caused by this stack and fixed (the reload focus
    regression above, and the outage test needing the scrolled sidebar back in view). Rerun 47f7711e: 10/10 passed.
- Unit tests: `cargo nextest run -p farhelm-ui --lib -E 'test(guarded) | test(yolo)'`, run 434de61e, 18/18 passed, on
  the stack before the focus PR's review fixes (which do not touch the launcher).
- Static checks:
  - `cargo clippy --all-targets -- -D warnings` passed at the tip, both before and after the rebase onto main.
  - On the final code before the rebase: `cargo fmt --all -- --check` and `cargo check -p farhelm-ui --features desktop`
    passed.
  - `python -B scripts/check-test-sleeps.py`: 0 delays lack a rationale.
  - `python3 releasing/check-changelog.py format` passed.
  - `dprint check` passed on the changed Markdown. It flags one file this stack does not touch, already on main.

Reused: every run above except the second clippy predates the final rebase onto main and is reused for the pushed code.
What landed in between changed tunnel ports, link classification, supervisor session attribution, checkout ownership and
documentation, none of it in the code these PRs change. SPEC.md and SPEC_impl.md merged without conflict; main's changes
there are in other sections.

Skipped: the full browser suite and the full Rust battery. All five changes are in the GUI. The specs that exercise them
and the focus paths around them ran instead.

## Review gate

One fresh Opus 5.5 review at high effort per PR, with the test-authoring checklist:

- #2652 (session menus): found that the change broke an existing test, which expected the Delete question to reappear
  without reopening the menu. It also found that the new test could race the list's own retry after a failed load. Both
  were fixed, and the comment was corrected to cover both orderings.
- #2653 (Replace with): found that the "⏎ replace" hint never reached screen readers, because the row's accessible name
  replaces its visible text. That led to the "replace:" name. Test and changelog wording were tightened.
- #2654 (outage card): confirmed the new test fails against the old layout. It led to clearing the sidebar's resize
  handle, the narrow-window minimum, and the test asserting which area had scrolled. The Mac narrow-window app bar
  remains, as noted above.
- #2655 (guarded answer): found that the YOLO question could still offer the guarded launch when the lost reply came
  back after the user had switched the launcher to another host or setup and then returned to the first. Fixed. It also
  led to the spec exception, the browser test, and rewording the error shown when "start, and don't ask again" fails
  ("This answer started nothing" instead of "Nothing was started", which contradicted the new wording).
- #2656 (focus): found the Restart with focus return described above. It also suggested counting only actual clicks on a
  row or tab, so Tabbing past them does not count, which I adopted.

No finding was declined. Each review ran once, and no reviewer looked at the fixes it led to; those are checked only by
the reruns listed under Checks.
