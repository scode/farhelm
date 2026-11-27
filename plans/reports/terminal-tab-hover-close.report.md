### What this was about

In an open session's top row, each shell tab ("Terminal 1", "Terminal 2", ...) had an always-visible × on its right, and
the last tab's × sat right next to `+` (new terminal tab). You kept closing a terminal tab by accident when you meant to
open a new one. You decided on a Ghostty-style design: each shell tab gets a fixed slot on its left that shows a
terminal icon at rest and turns into the × when the pointer is over the tab, keyboard focus is on the tab or its ×, the
tab's close question is open, or its close is already under way, with nothing in the row moving when it does. Closing
otherwise behaves as before: Skip session confirmations still skips the question, and the question still opens under the
× with Confirm close focused. No new keyboard shortcut.

What was built: exactly that. The main terminal's tab (Agent or Shell) has no icon and no ×, as before. The ×'s place in
the keyboard order is unchanged (each tab, then its ×, then the next tab, and `+` last), so Tab reaches a × even when it
is not showing. In a very narrow session pane the focused tab or × is still scrolled into view. On macOS, dragging the
window by the gaps between tabs still works. SPEC.md's "Session view" describes it.

### Things you should know

The first three were decided during execution; each is one you may want to revisit. The last two are for your
information.

- **The × always exists, just invisibly at rest.** It is drawn over the left slot and only fades in, rather than being
  added when the pointer arrives. That keeps the row from shifting, keeps it reachable by keyboard, and avoids flicker.
  A screen reader still announces "Close Terminal N" when it is reached, which is right for a control Tab can reach.
- **In a very narrow pane, a focused × is scrolled into view by its own edge.** Scrolling to the start of its tab left
  only a sliver of the × visible beside the port-forward (tunnel) badge, so the strip now moves just enough to show the
  × with its focus ring, and does not move at all when it is already visible.
- **Clicking a tab's × does not switch to that tab**, as before, although the × now sits on top of the tab: the tab you
  were on stays showing while the close question is open, and if you cancel. On macOS you can't start dragging the
  window from a ×, the same as with any other button.
- **On a tab that isn't selected, the × turns red under the pointer**, as before. On the selected tab, the × is white on
  the blue fill and gets a red fill on hover. The review caught that the red ink had been lost in the move; it is fixed,
  but no automated test can see the drawn colour, and no screenshot shows the hover state.
- **The release note** is your wording from the plan, used as the changelog entry.

### Open questions and possible follow-ups

- Possible follow-up: add a test reference screenshot of a tab with its × revealed by hover. Today only the resting look
  is captured, so nothing would catch it if the red × on hover, or the tab's hover background continuing under the ×,
  regressed again. Recommended, since the red hover was already lost once in this work.
- The macOS-only window-drag test was updated but could not be compiled or run here (the work ran on Linux). It uses
  only helpers that test already used. It stays unverified, including whether it compiles, until someone runs CI or a
  release build on macOS. Possible follow-up: run CI on macOS for this change before the next release.

### The PRs

- #5096 `feat: move terminal tab close to a left hover slot`: the left slot and hover-revealed ×, the tests, the two
  refreshed test reference screenshots (the session toolbar and a cut-off session row), SPEC.md, the changelog entry,
  and the TODO.md entry's removal.

### Checks

Run now, on the change rebased onto the latest main:
`cargo clippy -p farhelm-gui -p farhelm-desktop --all-targets -- -D warnings` (clean), and through the recorder (run
baaf8e0f-c39c-46e9-844e-556ae31c34df, 452 passed):

```
cargo nextest run -p farhelm-gui -p farhelm-desktop --lib -E 'test(/terminal::set::/) | test(/session_pane::header_tests::/) | test(/title_band|drag/) | test(/components::/) | test(/goldens::/) | test(/context_menu/) | test(/layers/)'
```

That covers the tab strip's tests (the × at each reveal state: rest, hover (including when the pointer is still over the
tab after you have been typing, and no reveal while the window takes no input), focus on the tab, focus on the ×, the
question open, the close under way; nothing moving between rest and reveal; the keyboard order; a press on the × not
selecting its tab), the narrow-pane test, the window-drag tests that run on Linux, the button styling test, and the
right-click menu and overlay tests that landed on main meanwhile. Also on the rebased change: a comparison of the two
changed screenshots (the session toolbar and a cut-off session row) against the newly published reference pictures,
which matched.

Reused, from before the rebase (main's changes since touch other parts of the app, not the tab strip or the button): the
screenshots were re-rendered, inspected in light and dark and published after a full update that showed only those two
scenes changed; a full comparison of every screenshot after the review's fixes, which all matched;
`cargo fmt --all -- --check`, the test-sleep check (no unannotated delays), `dprint check` on SPEC.md and TODO.md, and
`python3 releasing/check-changelog.py format`.

Skipped: the workspace-wide Rust run and the end-to-end suites, since only the desktop app's tab strip and its button
styling changed; the macOS-only window-drag test (Linux machine); the desktop app's end-to-end smoke test, whose steps
do not open shell tabs.

### Review gate

Reviewed by a fresh-context Opus 5.5 agent at high effort. It agreed with the three execution-time choices above (the
always-present invisible ×, scrolling to the ×'s own edge, and a press on the × stopping there) and found one real bug
(the × on an unselected tab had stopped turning red on hover), two weak test checks (a macOS test that did not assert
the close question opened; a keyboard-order check that could pass vacuously, now dropped in favour of the strict one),
one refinement (scroll only when the × is not already in view) and wording issues in SPEC.md and comments. All were
fixed; a verification pass by the same reviewer confirmed them, leaving two comment-wrapping nits, also fixed.
