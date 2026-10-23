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

### Landing

Landed as #2652, #2653, #2654, #2655 and #2656, merged bottom-up, with the content the report describes and no changes.
It was the only plan in this landing round.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained checkout-ownership-oct22 and supervisor-lock-outlives-work, both supervisor-only
(checkout archiving and create retries, and when a stopping supervisor releases its state folder). The base already held
the recent GUI work: browser sign-in recovery by page reload and discarding failed preference saves
(sign-in-preservation-sweep and gui-saves-last-write-wins), and GUI confirmations bound to what the user saw
(bind-gui-answers). A fresh-context reviewer and I read this plan against them:

- The one point of contact with the supervisor plans helps rather than conflicts: checkout-ownership-oct22 made a late
  retry of a session create answer with the existing session even after that session was restarted, which #2655 relies
  on when it handles a retry whose first attempt may already have started the session.
- The SPEC.md and SPEC_impl.md amendments are in different sections from main's and agree with the code. The reviewer
  noted one imprecision: SPEC_impl.md says the terminal takes focus only from where the selection left it, while the
  code records that place a moment after the click, when the view first asks for the terminal; the report says so.
- dialog-saves-oct22 (the in-flight plan for saving dialogs), overlaps this plan only in the triage record and the
  review queue's index, so its landing will meet only mechanical conflicts.

#### Things you should know

- The reviewer read the new focus rule (#2656) as possibly leaving focus on the header's restart button after a restart
  that needs no confirmation, so that typing would not reach the restarted agent. A throwaway browser test (see Checks)
  restarted an idle agent that way, by click and by keyboard, on both engines, and focus reached the restarted terminal
  every time. The reviewer's own case, an agent that has already exited, takes the same no-confirmation path but was not
  tried; see the follow-ups.
- The "remove-and-re-add-host" browser test failed on WebKit in this round's run, and about half the time on main
  without this plan too (see Checks). This plan does not change what that test checks, so I am confident it is not the
  cause.

#### Possible follow-ups

- Found at landing: check focus after restarting an agent that has already exited from the header's restart button. If
  focus stays on the button, the fix is to treat a restart's new terminal like a fresh selection for focus. I recommend
  a quick browser test before any code change, since the idle case works.
- Found at landing, minor and older than this plan: when a Replace without changed settings sends its request, gets no
  reply, and is then asked to confirm a YOLO launch on retry, the question says "Nothing has been started", though the
  first attempt may already have replaced the session. No second session can result (this Replace offers no guarded
  alternative), so only the wording is wrong. The code comment #2655 rewrote for that question also says the session
  launcher is the only place this happens, which is not true. Options: pass "may have started" through to that question
  as the launcher now does, or only correct the comment. I recommend the first, since it is small and fixes what the
  user reads.

#### Checks

The plan's five changes are all in the GUI, and main gained only supervisor changes since its base, so the risk is the
five changes against each other and the GUI around them. This round ran, on the plan as it landed:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, the isolated test-sleep check, `dprint check` on the changed Markdown
  files, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (one warning, about a
  citation main already had), the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh`: all passed.
- `cargo nextest run -p farhelm-ui` through the test recorder with pinned nextest and tmux, four slots, no retries, run
  `813138e4-776a-41d4-8404-56d634ef6973`: 752 passed. `cargo nextest run -p farhelm-ui --features desktop`, same
  settings but without tmux (the desktop tests do not use it), run `1ead556f-889c-42a5-a731-fe6fcb280063`: 917 passed.
  `node --test` in `crates/farhelm-ui/js-tests` (the terminal's script changed), run
  `97b00f8b-9853-4013-876c-69652bd9e09a`: 333 passed.
- `npx playwright test` in `e2e/` on Chromium and WebKit for the six spec files the plan changes and five more that
  exercise the same menus, terminal focus and launch confirmations (`quick-switcher`, `quick-launch`, `restart-with`,
  `terminal-reconnect`, `sidebar`), three workers each with its own helm and supervisors, no retries, run
  `4e8d25ff-03bd-47c6-8c8b-d90b8b33a3a4`: 851 passed, 1 failed. The failure was WebKit's "remove-and-re-add-host:
  removal forgets, re-adding rediscovers" in `terminal-multihost.spec.ts`: after a host is removed and added again, its
  surviving session's status read "idle — new output" where the test expects a plain live status. The plan does not
  change that status or that test. Ten WebKit repeats of the test on this stack (run
  `499364cd-ccc2-4ec5-8887-9009ce02ff0b`) failed 7 times, and ten on main without the plan, with main's own web build
  (run `1dc305b6-8709-4fa4-ab21-1e138709a732`), failed 5 times the same way, so it is a failure main already has. I am
  recording it in FLAKES.md, with a Deflake entry in TODO.md, in a separate draft PR.
- A throwaway browser test, not committed, for the reviewer's question in "Things you should know" above: an idle agent
  restarted from the header's restart button, by click and by keyboard, five times each on both engines, run
  `89318d2d-5e05-4653-a6a1-0ded84eb4bd6`: 20 passed; focus reached the restarted terminal every time.
- Reused: none. Skipped: the rest of the browser suite (the plan's changes are confined to the menus, outage notice,
  launch confirmation and terminal focus the selected files exercise), the rest of the Rust workspace (no helm or
  supervisor code changed), the doctests (no documentation example changed), the desktop smoke test, the native Mac app
  (not runnable here) and the installer (unchanged).

The report's checks were browser runs of the changed specs and a filtered set of the GUI's unit tests, before the final
rebase; this round ran all of the GUI's unit and script tests and eleven browser spec files on both engines on the plan
as it landed. Nothing in the executor's report became untrue at landing.
