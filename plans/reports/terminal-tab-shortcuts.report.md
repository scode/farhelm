### What this was about

A session's shell tabs (Terminal 1, Terminal 2, ...) could only be opened and closed with the mouse. On a Mac, Cmd+W,
the chord terminal apps close a tab with, deleted the whole open session instead, killing the agent and every shell tab
with it. With **Skip session confirmations** on, it did that on the first press with no question. Your three TODO
entries asked for Cmd+T to open a tab, for Cmd+W on a shell tab to close only that tab, and for a warning before Cmd+W
on the main agent tab deletes a session that still has tabs, even with confirmations skipped. You merged them into this
one plan and decided:

- Cmd+T opens a tab on a Mac, and Ctrl+Shift+T on Linux.
- Cmd+W closes the shown shell tab on a Mac, and Ctrl+Shift+W does so on Linux. On Linux, Ctrl+Shift+Backspace stays the
  Delete shortcut.
- Both close chords follow Skip session confirmations.
- Deleting a session with open shell tabs always asks, from the shortcut and from clicking Delete in the header or the
  row menu.

What was built, in two PRs:

- **Delete asks while tabs are open.** Deleting a single session that has at least one shell tab open now always asks
  first, even with Skip session confirmations on, whether the Delete comes from the row menu, the header's Delete button
  or the Delete shortcut. The question is the existing one that counts the open tabs. The header Delete button's tooltip
  keeps "after you confirm" in that case. Deleting several marked sessions at once still follows the switch, as before.
- **The tab shortcuts.** Cmd+T / Ctrl+Shift+T opens a shell tab exactly as the New terminal button does, from anywhere
  in the window, including a focused terminal, and the new tab is shown. Cmd+W on a Mac with a shell tab shown, or
  Ctrl+Shift+W on Linux, closes that tab as its × does. It asks under the tab's ×, scrolling the tab into view first if
  the strip had scrolled it away, or closes at once when confirmations are skipped. On the main terminal, a Mac's Cmd+W
  deletes the session (with the question above when tabs are open), and Linux's Ctrl+Shift+W does nothing.
- **Where the shortcuts are listed.** The New terminal button's tooltip names its shortcut. The Keyboard shortcuts
  dialog lists "New terminal tab" and "Close terminal tab" under Terminal, so on a Mac it shows ⌘W twice, for Delete
  session and Close terminal tab, since one chord does both.

None of the chords ever reaches the terminal's program. Holding a chord down acts once, not once per key repeat. The
chords do nothing while a dialog or menu has the keyboard or while an input method is composing text. SPEC.md, the docs
pages Keyboard shortcuts, Work in a session, Stop, restart and resume, and Settings, and the changelog draft describe
the new behavior. The three TODO.md entries are removed.

### Things you should know

- **The Settings switch's own help still says it deletes on the first click.** Its tooltip and description in Settings
  read "Stop, delete, replace, restart and close terminal tabs on the first click". Deleting a session that has shell
  tabs open now asks anyway, so someone who turned the switch on may be surprised by the question. The plan listed only
  SPEC.md and the docs pages to update, and changing that text would also change the Settings screenshots, so it was
  left as it was. The docs Settings page, SPEC.md and the changelog draft all mention the exception, and the changelog
  draft flags the help text for curation. The same help already left out the sandbox and container Delete it never
  skipped.
- **A Mac Cmd+W pressed again while the tab's close is still under way does nothing.** It does not fall through to
  deleting the session. That keeps a double tap with confirmations skipped from ending the session after its tab. When a
  card covers the terminals (an unavailable host, say), the tab strip takes no presses, so Cmd+W deletes the session as
  it does from the main terminal, still asking first when the session has tabs open.
- **"Always asks" judges the session as the window drew it.** If a tab is opened elsewhere (from another client, say) in
  the moment between the window's last frame and the press, a skipped Delete of a running session ends that tab without
  asking. Closing that gap would need a new kind of precondition on the Delete request.
- **The Mac's Cmd+W was tested on Linux only.** No Mac ran this work. The tests drive the Mac's rule through the same
  code path the Mac uses, with the platform passed in, so a Linux run checks it. What only a Mac would show is whether
  Cmd+T and Cmd+W reach the app at all. The shortcut-table test checks that the Mac's chords map to the right actions.

### Open questions and possible follow-ups

- Update the Settings switch's help text to say a session with open shell tabs still asks before Delete, with the
  matching SPEC.md sentence and the Settings screenshots. Say if you want this as a Plan follow-ups entry.

### The PRs

- #5164 `feat: always ask before deleting a session with open terminal tabs`
- #5166 `feat: open and close terminal tabs with Cmd+T and Cmd+W` (stacked on #5164)

### Checks

All on Linux, before the final rebase onto the latest main. The rebase brought in only the file hover card's "too large
to copy" hint and the launcher's dead-code cleanup, neither touching what these runs cover. The only conflict was in
TODO.md, between removed entries.

- The session list's and session pane's tests in the desktop app, through the test recorder
  (`cargo nextest run -p farhelm-gui --lib -E 'test(/session_list::/) | test(/session_pane::/)'`): 807 passed (run
  e9fcc309) for the first PR. After review, its four new or changed tests and the reworked test beside them passed again
  (run 700f4f0a).
- For the second PR, the terminal tabs', shortcuts dialog's, keymap's, desktop window's, main view's, session pane's and
  session list's tests
  (`cargo nextest run -p farhelm-gui --lib -E 'test(/terminal::set::/) | test(/shortcuts::/) | test(/actions_tests::/) | test(/desktop::/) | test(/session_pane::/) | test(/session_list::/) | test(/main_view/)'`):
  949 passed (run 2c2ed46c). After review, the tabs', shortcuts' and keymap's tests
  (`cargo nextest run -p farhelm-gui --lib -E 'test(/terminal::set::/) | test(/shortcuts::/) | test(/actions_tests::/)'`),
  110 of them including the three new ones, passed (run fa5dd5cb).
- New tests seen failing as intended, each with the code it covers broken on purpose:
  - With the open-tabs rule turned off, all four of the first PR's new or changed tests failed (run d3d54cd0).
  - Without the scroll into view, the off-screen-tab test failed (run 1f97d057).
  - With the close shortcut never closing, all four close tests failed (run b3924a39).
  - With the in-flight guard, the held-key listing and the card checks removed, the three tests added in review failed
    (run 6c1bb70c). The two card checks were removed together, so that run shows the test catches losing both, not each
    alone.
- Not caught by a mutation: the new-tab shortcut's own check for a pending request on the session. With that check
  removed, its test still passed (run 3c6be042), because the app already refuses a second request on the session by
  itself; the check only keeps the shortcut in step with the button's disabled state.
- Screenshots (`python3 scripts/check-gui-goldens.py --update`): a full update rendered every scene, and only the
  Keyboard shortcuts dialog's two pictures changed, by the two new rows. Those two were published and the manifest
  updated.
- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-gui --all-targets -- -D warnings` (also on the rebased tree),
  the test-sleep check (`python -B scripts/check-test-sleeps.py`: 312 delays, none lacking a rationale), `dprint check`
  on the changed Markdown, `python3 releasing/check-changelog.py format`, and the docs website build
  (`bun install --frozen-lockfile`, `bun run build`): clean.

Skipped:

- Tests outside the desktop app: nothing outside it changed.
- The end-to-end app smoke test: the change is confined to key handling, the Delete rule and two tooltips, all covered
  by the tests above.
- Lint of the desktop app's own crate (`cargo clippy -p farhelm-desktop --all-targets`): it stopped before reaching that
  crate, on an unused-function lint in the helm's notification code, which this work does not touch. None of the desktop
  app crate's own files changed, and the GUI crate holding the change passed the same lint.

### Review gate

Each PR went to a fresh-context Opus 5.5 agent at high effort, which reviewed it and then verified the fixes. Both gates
passed, and neither review found a bug in the behavior.

- **First PR.** The review found the change needed its own changelog draft, since it is a feature on its own (the plan
  had put the draft in the second PR only). It also found tests that did not prove Skip session confirmations was on
  before checking that the question appeared, and asked for focus checks before Enter, stale comments to be updated, and
  clearer SPEC wording. It noted the drawn-frame limit above. All of it was applied.
- **Second PR.** The review confirmed:
  - the shortcuts' rules match the plan;
  - the kept close-button positions cannot go stale or leak;
  - the key bindings are consistent per platform.

  It asked for tests of a second Cmd+W during a close, of a held Cmd+T and of the covered-terminal case, a fix to the
  Stop, restart and resume page, which still said Cmd+W always deletes, a focus check in one test, and wording fixes.
  All of it was applied, and the new tests were seen failing with their guards removed.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained three changes, none in the desktop app: `farhelm agent hosts`
gained a `TAILSCALE` column (agent-hosts-tailscale-column, #5161), and the command line stopped letting a free-text
option swallow the next flag and had its parsing quirks cleaned up (cli-flag-parsing-audit, #5163 and #5165). In the
same round, after this plan, two drift-test plans landed: drift-base-isolation-and-turns (#5167, #5168) and
drift-permission-footer (#5169). They change only the opt-in real-agent tests, their docs and a test-only hook in the
supervisor, so none of these meets this plan. The overflow plan for the tab strip (terminal-tab-overflow-ux) has not
landed yet, so this plan's scrolling of a tab into view before its close question still works as tested.

An independent reviewer and I read the change and found nothing that must change before landing. The reviewer went
through what Cmd+W does with each part of the window focused: the main terminal, a shell tab, a tab already closing, a
card covering the terminals, no session, the launcher, Settings, a dialog, a menu, a question. No path deletes a session
that the window shows with open tabs without asking (the report's own caveat about a tab opened from elsewhere in the
moment before the press still applies). Cmd+T refuses exactly where the New terminal button is disabled. The Linux
chords clash with no existing shortcut. Plain Ctrl+T and Ctrl+W still reach programs in the terminal; only the Shift
variants are taken. Only the Linux Keyboard shortcuts dialog's two screenshots changed. I looked at the new light one: a
Terminal section lists "New terminal tab" and "Close terminal tab" with their Ctrl+Shift chords.

#### Fixes made while landing

- **A TODO.md conflict.** The round's plans each removed their own TODO entries, and one added a follow-up entry, so
  stacking them conflicted there. I kept main's TODO.md with each plan's own TODO change applied; no code changed.

#### What the landing made untrue in the report

Nothing changed by the landing itself, but the review found that the report overstates one point:

- **A fast double Cmd+W can still end the session.** The report says a second Cmd+W while the tab's close is under way
  does nothing, which "keeps a double tap with confirmations skipped from ending the session after its tab". That holds
  only while the tab's close is still in progress. With **Skip session confirmations** on, once the last shell tab has
  closed, a second Cmd+W lands on the main terminal, and with no tabs left the session is deleted without a question. So
  a quick double tap on the last tab can close it and then delete the session. That follows the decisions (Delete asks
  only while tabs are open), but it is the case the report says is prevented. Possible follow-up: ignore a Delete from
  Cmd+W for a moment after Cmd+W closed a tab.

#### Worth knowing

- **Two places on a Mac still show ⌘W as Delete while a shell tab is shown:** the header Delete button's tooltip and the
  row menu's Delete shortcut label. While a shell tab is in front, ⌘W now closes that tab instead. Possible follow-up:
  show those hints only while the main terminal is in front.
- **Some new tests would pass without what they check.** The test for a second Cmd+W during a tab's close fails if that
  press falls through to Delete, but would still pass if the guard were deleted outright. The test that Linux's
  Ctrl+Shift+W does nothing on the main terminal would pass with no close handling at all. No test checks that, with
  confirmations on, a second Cmd+W after the tab's question was answered, while the close is still in progress, does not
  ask about the same tab again, and none checks where focus goes after a close or a cancelled question.

#### Checks

Run once, on the round's three plans stacked on main as it stood when the landing started (terminal-tab-shortcuts, then
drift-base-isolation-and-turns, then drift-permission-footer), so each was tested with the others and with the earlier
changes above. The desktop plan got the desktop app's tests, screenshots and end-to-end smoke test. The drift plans'
real-agent cases are opt-in and spend vendor turns, so they were built and listed, not run, and the ordinary tests of
the drift code and of the supervisor's footer reading ran instead. The SSH test worker was unreachable, so the smoke ran
on a boat sandbox (an x86_64 Ubuntu 24.04 VM), and the rest ran locally:

- `cargo clippy --all-targets`, and `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent`
  (the build with the desktop app's test-automation hooks): each reported one warning and nothing else. That warning is
  the unused ntfy settings method in the helm, on main since #5120 and outside this round, which makes `-D warnings`
  fail, so these ran without it. `cargo clippy -p farhelm-gui --all-targets -- -D warnings` (the desktop app's GUI
  crate, which does not depend on the helm's code): clean.
- The GUI crate's whole default test suite (`cargo nextest run -p farhelm-gui`), recorded run
  790d4ebc-9b6a-46d4-b81b-12cdbc9a08f6: 3,603 passed, none failed, and it skipped its 4 opt-in tests, among them the
  screenshot test the next item runs.
- The full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run 9be720dc-7a79-40ae-b77a-eec843eafb24:
  passed.
- On a boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  94e567ef-c5b4-4bbd-9a83-c0bac7dd24de: 48 of its 49 steps passed, and the paste step was skipped, as on every Linux
  run. Its steps include Delete from a session's menu; none presses Cmd+T or Cmd+W.
- The supervisor's per-agent tests, the footer reader and its captured screens among them
  (`cargo nextest run -p farhelm-supervisor --lib -E 'test(/agent_kind::/)'`), recorded run
  7c2a5d10-f11a-46a9-a5d5-7ff23f798176: 112 passed, none failed.
- The ordinary tests of the real-agent test code, without the opt-in cases
  (`cargo nextest run -p farhelm --test e2e -E 'test(/real_agent_capture/)'`), recorded run
  56b688c2-75da-476f-baad-df4c39ca17a7: 50 passed, none failed, and no test reported that it skipped for a missing
  system service. Listing the opt-in drift cases
  (`cargo nextest list -p farhelm --test e2e --run-ignored only -E 'test(/real_agent_capture::drift::/)'`) built them
  and found 71.
- The docs website build (`bun run build` in `website/`): built. `python3 scripts/check-agents-md.py` for the drift
  tests' docs page, which the instruction files point to: passed.
- `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown: passed.

Reused: nothing. Not rerun: the drift plans' live runs against the real agents, described in their reports; the landing
ran none, since they spend vendor turns, so the drift plans have not run live together. Skipped: the rest of the Rust
workspace's tests, since nothing else changed; the other opt-in tests; and any run on a Mac, since none was available to
the landing. No Mac ran Cmd+T or Cmd+W: the Mac's rule is covered only by the tests that pass the platform in.
