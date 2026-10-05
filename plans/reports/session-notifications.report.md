## What this was about

Farhelm notices several ways it can lose track of the agent conversation in a session, and each one means Restart will
later be unable to resume that conversation. Until now those problems were only written to the supervisor's log, so a
user found out much later, when they pressed Restart and it could not pick the conversation up, and their agent's
context was effectively lost for that session. The lead example: a session launched with Farhelm's conversation hook
(Claude, Codex) gets typed into, and a minute later the agent still has not told Farhelm which conversation it is in, so
Restart will not be able to resume it.

These problems now reach the user on the session itself. A session with notifications shows a bell on its sidebar row,
just left of the activity time. The bell is grey when everything has been read and red and filled while something is
unread. Clicking it lists the session's notifications, newest first, with the new ones marked. Closing the list (a click
elsewhere, Escape, or the bell again) marks them read, and **clear all** removes them. Read and cleared state is kept by
the helm and is the same in every window connected to it. Each session keeps its 10 most recent notifications, and they
survive supervisor and helm restarts.

What notifies in this version:

- a hooked launch that, a minute after the user first pressed Enter in it, still has not reported its conversation
  (Claude, Codex, Goose and Grok; not Pi or OMP, see below);
- a launch Farhelm could not add its conversation hook to (for example an older launch whose command already passes its
  own `--settings`), except when the user turned hooks off for that agent themselves;
- a Codex or Grok conversation record that Farhelm re-checked and found missing or inconsistent, which withdraws the
  resume offer;
- an OMP launch whose conversation reporter is older than the running Farhelm, or whose installed reporter file differs
  from the one this build ships.

## Things you should know

- **Refused conversation reports almost never notify.** You asked for a notification only when a refused report points
  at a real problem, not for the routine refusals. Every place the supervisor can refuse a report was checked. All of
  them turned out to be the system working as intended: a subagent's report, a report from an agent process the
  session's agent started itself (such as `claude -p` run inside the session), a report overtaken by a relaunch or a
  delete, or a passing error that the next report recovers from. The one exception is the OMP reporter mismatch above,
  which does notify. So no other refusal produces a notification.
- **The minute now starts at the first Enter, not at any input.** The terminal in the app answers the agent's own
  queries (cursor position, colours) automatically, and those answers used to start the clock for a session nobody had
  typed into. Every automatic answer the app's terminal can send was checked, and none contains an Enter. An Enter
  inside a paste and Shift+Enter insert newlines and do not count; an Enter that only answers the agent's own prompt or
  picker does count.
- **A launch that still carries a conversation Farhelm captured earlier is never told Restart cannot resume it**,
  because Restart can. This also closes the millisecond race between a report arriving and the one-minute check.
- **Pi and OMP are never told their hook is silent.** While this was in review, main gained a record of when each agent
  normally reports its conversation, and Pi and OMP report only after the agent's first reply is saved. A first turn can
  run for minutes, so a minute of silence proves nothing for them, and the notification would often be false. The
  supervisor's log line for them stays.
- **Only launches this supervisor started are checked** (also listed as a possible follow-up below). After a supervisor
  restart, the running launches it picked up are not checked for a silent hook, because only the start of a launch knows
  whether a hook was added.
- **Notification texts.** Each says what happened and what the user can do (usually starting the session over with
  Replace with), or, when nothing can be done, what they lose. None points at a log.
- **No protocol version bump.** The list travels on the session's existing listing record as an optional field; an older
  helm shows no bell and an older supervisor sends none.
- **A missing OMP reporter file does not notify**, since it may be mid-reinstall; only a file that is there with
  different contents does. Notifications are history: when a Codex or Grok record that withdrew the resume offer comes
  back, the old notification stays until cleared.
- **The bell's popover is placed beside the bell**, using the same floating placement and outside-click dismissal as the
  row menus, so it is never clipped by the sidebar. Opening it moves keyboard focus into it so Escape works however the
  bell was clicked.
- **The sidebar row's layout changed underneath, with no visible difference.** A row's whole first line is one button
  (the one that opens the session), and a button cannot contain another button, so the bell is a separate control laid
  over the row in the spot the row keeps free for it. That needed the row's open button and its `⋯` menu button to sit
  in a two-column grid instead of side by side in a flex row. Rows without a bell look exactly as before; the full
  sidebar browser suite, which checks row geometry, passed on both engines after the change.
- **How read and cleared are stored.** The helm keeps, per session, two numbers: "read up to notification N" and
  "cleared up to notification N" (each notification has a number that only grows). They only ever move forward, so a
  window with an older view cannot make a quiet bell loud again, and they are capped at the newest notification the helm
  has seen for that session, so closing a list can never hide a notification that arrives a moment later. The helm also
  shows at most the 10 newest entries itself, whatever a host sends.
- **The helm's database gains one table**, added automatically by the usual upgrade step when an existing helm starts;
  nothing to do by hand. Its version number became 41 rather than 40 because main took 40 for another change while this
  was in review.

## Open questions and possible follow-ups

- **A docs screenshot of the bell.** The new "When a row shows a bell" section of Read the session list describes the
  bell in words only. The editorial rules ask for an annotated screenshot; docs screenshots are captured and published
  for every page at once by the separate refresh procedure, so the shot is best added at the next refresh.
- **Keyboard gap: opening the help or update menu at the top of the sidebar from the keyboard leaves an open
  notification list open**, so two floating panels show at once until one is closed. A mouse click closes the list as it
  should, and opening a row's `⋯` menu or a host's menu from the keyboard closes it too. I left this out because the fix
  needs the top bar to know about the list; you may want it fixed as a follow-up.
- **Launches picked up after a supervisor restart are not checked for a silent hook.** The check needs to know whether
  that launch got the hook, which only the start of the launch knows today. Recording that on the session would close
  the gap, if you want it.
- **A notification is history, not a live state.** When a Codex or Grok record that withdrew the resume offer comes back
  and the offer returns, the old notification stays until cleared. Withdrawing notifications was out of this plan's
  scope; it could be added if stale ones turn out to confuse.
- **A limit to keep in mind if the UI changes.** The read and cleared numbers are capped at what the helm's session list
  holds. The view of one open session can briefly show a newer notification than that list; the bell never reads from
  that view, so nothing is affected today, and the code notes what a future change would have to handle.
- **No end-to-end test** of a supervisor recording a notification and the app showing it in one run. The pieces are
  covered separately: the supervisor's recording, the fixed sample of the session list's wire format that both the helm
  and the app are tested against, the helm's handling, and the browser behavior.

## PRs

- #1619 — the spec describes session notifications (SPEC.md Status, Session list, Errors; SPEC_impl.md storage, marks).
- #1620 — the supervisor records notifications; the one-minute clock starts at the first Enter; the listing field.
- #1621 — the helm keeps shared read and cleared marks, with two endpoints to set them.
- #1622 — the bell and its list in the UI, the browser spec, user docs, the changelog entry, the TODO entry removed.

They are stacked in this order, each on the one before, and land bottom-up.

## Checks

Run on this stack. The ids are the test recorder's run ids, kept with each run's full output.

- Supervisor and session-record test suites in full, before the rebases: 1155 of 1156 passed on the first run; the one
  failure was this change's own (a missing OMP reporter file was treated as a mismatch, now fixed so it records
  nothing), and the affected tests passed after the fix. The supervisor's notification-related tests ran again after
  each review round and each rebase (counted in the runs below).
- Helm test suite, full: 965 of 968 passed on the first run; the 3 failures were test fixtures this change had to update
  (an old-database sample and the session list's fixed wire-format sample) and passed after updating them.
- UI library, full: 411 passed.
- Browser, both engines: run 5bb30c2c-4639-463b-a67a-e6d254fe0da3, the notification spec, tooltip coverage and the full
  sidebar suite, 224 of 224 passed. Two earlier runs of this change's own new spec failed on test premises and a missing
  focus handoff, and were fixed (runs bc464676-09eb-4919-8cae-931f5404bb47, 6ae36e4e-e34a-452f-9211-7ebbe1e22cd4).
- After the first rebase onto the latest main: run b38b74b7-7377-4212-a641-5bd82683f2c8, the helm and UI test suites in
  full plus the supervisor's notification-related tests, 1502 of 1502 passed; browser run
  620a5e75-88a8-4dff-bdde-700b41fbf4e9 (the notification tests, the hover-help coverage test, and the sidebar's menu,
  compact and hover tests, both engines), 72 of 72 passed.
- After the second rebase and the Pi/OMP fix: run 5303cb6c-66c5-4bc7-a962-469b5be70dfa, the same suites plus the session
  record's own tests and the supervisor's restart tests, 1662 of 1662 passed. The browser run above was reused: the only
  change that landed in between is about the open session's Restart wording, not the sidebar or the bell.
- `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo clippy -p farhelm --bins -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, `dprint check`, the changelog format check, the test-sleep check, and
  the docs website build (all internal links valid).

Skipped: the Rust end-to-end battery, doctests, the desktop smoke test, and the installer and host-setup tests, because
nothing in this stack touches them beyond the session record, whose wire format the fixed-sample tests cover.

## Review gate

Two fresh reviewers per PR, Claude Opus 5.5 at high effort and gpt-6-astra at high effort.

- PR 1 and PR 2: findings on clock wording, which refusals notify, the race against a captured conversation,
  notification wording and tense, and several recording-site details. All fixed in the spec and code.
- PR 3: no blocking finding. Low-severity fixes applied (a misbehaving host's duplicate or out-of-range notification
  numbers, stronger test setups, a test for marking while a host is down). I declined one finding, the limit about the
  open-session view described above, with a note in the code.
- PR 4: fixed four real problems. Closing the list could mark as read a notification that had arrived in the same
  refresh and was never shown, or mark nothing if a filter had just hidden the row. Escape could fail to close the list
  in the Mac app, because a mouse click there does not give the bell keyboard focus. The bell's name for screen readers
  used the session title without the escaping the visible title gets. And the list stayed open, floating in the wrong
  place, after a compact toggle or a keyboard-opened host menu moved its row. I declined two findings, both listed as
  follow-ups above: the docs screenshot and the top-bar menus' keyboard gap.

### Landing

Landed on 2026-10-05 (UTC) as four squash commits on main, in stack order: #1619 (the spec), #1620 (the supervisor
records notifications), #1621 (the helm keeps read and cleared marks) and #1622 (the bell, its list, the docs and the
TODO entry's removal). Nothing else reached main while they merged.

#### What else was on main

Between the commit the stack was built on and the landing, main gained documentation only: the docs website's written
pages were updated to match the app (#1623, which also adjusted three docs screenshot scripts and the list of published
screenshots), the README screenshot was refreshed (#1626), TODO.md gained one entry and a plan pointer on another, and a
new plan joined the queue. No product code, SPEC.md or SPEC_impl.md, and no browser test changed. The rebase applied
without conflict.

The website update rewrote the two pages this stack also edits, so a separate reviewer that had not seen the work read
the combined pages before anything merged. On "Read the session list", the new "When a row shows a bell" section follows
the rewritten text coherently, names the menu item as the app now labels it ("replace with"), and its links resolve; the
four screenshots the page shows are all still published. On "Agent hook injection", the two changes edit different
paragraphs and the page reads consistently. The docs website build (which checks every internal link and anchor) passed
on the combined result.

The same reviewer checked what the bell's new row layout could break outside the stack's own tests, including the README
screenshot, the docs screenshots and the demo video, which no ordinary test run exercises. None depends on the old
layout. The reviewer checked one way a bell could appear in them, the most likely one: a hooked session that has not
reported its conversation a minute after an Enter. Those scripts either press no Enter or end a few seconds after the
one they press. It did not check the other triggers (a launch Farhelm could not hook, a Codex or Grok record found
inconsistent, an outdated OMP reporter) against the staged sessions those scripts use; if one of them fired, the next
screenshot or video refresh would show a bell, which whoever runs it looks at anyway. It also confirmed that the helm
database version this stack takes (41) is not claimed by any other open change, and that the plan still in progress that
changes how retried agent requests are matched ("agent-retry-by-request") will need its own rebase onto the new main,
which should apply cleanly.

#### A fix made while landing

One small one, found by that reviewer. In the sidebar list's code, a new type for the open notification list had been
inserted between an existing function and the comment describing that function, so the comment described the wrong
thing. The landing moved the type above the comment; no behavior changed. It went into #1622 before anything merged.

#### Checks

- Run now, on the final stack after rebasing onto the latest main: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, `dprint check`, `python -B scripts/check-test-sleeps.py` (no
  unannotated delays), `python3 releasing/check-changelog.py format`, and the docs website build. All clean. After the
  comment fix, `cargo fmt --all -- --check` and `cargo check -p farhelm-ui` again, clean.
- Reused: the test runs in the report's Checks section. What main gained since is documentation and a screenshot, and
  the landing's only change moved a comment.
- Skipped: running Rust or browser tests again, for the same reason.

Nothing in the report above was made untrue by the landing.
