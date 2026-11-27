### Since the last review

The first round was delivered and then blocked at landing: with `+` (New terminal) right after the last tab, pressing
`+` twice without moving the pointer put the second press on the new tab's ×, which then asked to close the terminal
just opened (or, with Skip session confirmations on, closed it at once). The landing monitor answered that block on your
behalf by pinning `+` at the right end of the tab area, rather than putting `+` before the shell tabs (next to the first
tab's × slot), disarming a new tab's × until the pointer leaves it, or landing as it was; it recorded that call in
TODO.md's "Plan decisions to revisit". The one PR, #5096, was changed in place: `+` is pinned, a test covers the double
press, SPEC.md and the changelog entry say so, and three reference screenshots were re-taken. Pinning `+` also fixes the
newer TODO entry "Keep the new-terminal button reachable when terminal tabs overflow" (with more tabs than fit, `+` used
to scroll out of view), so the PR removes that entry too, with a test. Nothing else built in the first round changed.

### What this was about

In an open session's top row, each shell tab ("Terminal 1", "Terminal 2", ...) had an always-visible × on its right, and
the last tab's × sat right next to `+` (new terminal tab). You kept closing a terminal tab by accident when you meant to
open a new one. You decided on a Ghostty-style design: each shell tab gets a fixed slot on its left that shows a
terminal icon at rest and turns into the × when the pointer is over the tab, keyboard focus is on the tab or its ×, the
tab's close question is open, or its close is already under way, with nothing in the row moving when it does. Closing
otherwise behaves as before: Skip session confirmations still skips the question, and the question still opens under the
× with Confirm close focused. No new keyboard shortcut.

What was built: exactly that, plus `+` pinned at the right end of the tab area, just left of the tunnel pill (the
port-forward badge a session shows while it has tunnels open) when one shows, so opening a tab never moves it. New tabs
fill in from the left; when there are more than fit, the tabs scroll and `+` stays in view. The main terminal's tab
(Agent or Shell) has no icon and no ×, as before. The keyboard order is unchanged (each tab, then its ×, then the next
tab, and `+` last), so Tab reaches a × even when it is not showing. In a very narrow session pane the focused tab or ×
is still scrolled into view. On macOS, dragging the window by the gaps between tabs, and by the empty space between the
last tab and `+`, still works. SPEC.md's "Session view" describes it.

### Things you should know

The first five were decided during execution; each is one you may want to revisit. The last two are for your
information.

- **In the narrowest case, `+` is not pinned.** With a tunnel pill showing in a 650-pixel session pane (the narrowest
  the top row supports), the tab area is only about 15 pixels wide, narrower than `+` itself. Pinning `+` there would
  leave the tabs no room at all, and SPEC.md requires every tab to stay reachable at 650 pixels, so in a tab area too
  narrow for `+` plus one control's width, `+` goes back to following the last tab, scrolling with the tabs. The double
  press can still land on a new tab's × in that layout, though in a 15-pixel area hardly anything is clickable anyway.
- **That choice is made from the previous frame's width.** When the tab area changes size across that threshold (the
  pill appearing in a narrow pane), the app draws one extra frame so it never stays with `+` in the wrong place. Without
  it, a pill appearing in a narrow pane would leave the tab area showing only `+` and no tabs until something else
  redrew the top row. The tests cannot see that extra frame being asked for (their drawing helper always draws several
  frames), so it rests on reading the code and on the same approach the approval cards and the host setup progress card
  already use.
- **The × always exists, just invisibly at rest.** It is drawn over the left slot and only fades in, rather than being
  added when the pointer arrives. That keeps the row from shifting, keeps it reachable by keyboard, and avoids flicker.
  The tab's hover background also runs on under the ×, so the tab does not look split while the pointer is on it. A
  screen reader still announces "Close Terminal N" when it is reached, which is right for a control Tab can reach.
- **In a very narrow pane, a focused × is scrolled into view by its own edge.** Scrolling to the start of its tab left
  only a sliver of the × visible beside the tunnel pill, so the strip now moves just enough to show the × with its focus
  ring, and does not move at all when it is already visible.
- **Clicking a tab's × does not switch to that tab**, as before, although the × now sits on top of the tab: the tab you
  were on stays showing while the close question is open, and if you cancel. On macOS you can't start dragging the
  window from a ×, the same as with any other button.
- **On a tab that isn't selected, the × turns red under the pointer**, as before. On the selected tab, the × is white on
  the blue fill and gets a red fill on hover. The first review caught that the red ink had been lost in the move; it is
  fixed, but no automated test can see the drawn colour, and no screenshot shows the hover state.
- **The release note** is your wording from the plan, used as the changelog entry. It does not mention the pinned `+`;
  the changelog draft's extra paragraphs do, for the release curation to decide.

### Open questions and possible follow-ups

- Possible follow-up: add a test reference screenshot of a tab with its × revealed by hover. Today only the resting look
  is captured, so nothing would catch it if the red × on hover, or the tab's hover background continuing under the ×,
  regressed again. Recommended, since the red hover was already lost once in this work.
- The macOS-only window-drag test was updated in both rounds but could not be run here (the work ran on Linux). It now
  compiles: it was type-checked on Linux by temporarily compiling it there. Whether it passes stays unverified until
  someone runs the GUI tests on a Mac (the release's macOS test leg does). Possible follow-up: run them on the test Mac
  before the next release.

### The PRs

- #5096 `feat: move terminal tab close to a left hover slot`: the left slot and hover-revealed ×, `+` pinned at the tab
  area's right end, the tests, the three refreshed test reference screenshots (the session toolbar, a cut-off session
  row, and the stop and suspend outcomes screen), SPEC.md, the changelog entry, and the removal of two TODO.md entries
  (the accidental closes, and `+` out of reach when tabs overflow).

### Checks

Run now, through the test recorder unless noted, on the change before its rebase onto the latest main (main had gained
only TODO entries, plan bookkeeping, review notes and a lore entry since, none of it code):

- The tab strip, the session pane and its header, the title band and window drag, shared components, the screenshot
  harness, menus and layers, and the session list
  (`cargo nextest run -p farhelm-gui -p farhelm-desktop --lib -E 'test(/terminal::set::/) | test(/session_pane::/) | test(/title_band|drag/) | test(/components::/) | test(/goldens::/) | test(/context_menu/) | test(/layers/) | test(/^session_list::/)'`):
  1141 passed (run 1e84abd2), after the second review's fixes (only the verification pass's one-comment wording fix came
  later). The test added after that, that many tabs keep `+` in view, passed on its own (run a668d9dc).
- The new tests were each seen failing with the behavior they pin taken away, at the intended assertion:
  - the two double-press tests (Skip session confirmations off and on), with `+` following the last tab again: with Skip
    off the second press opened the close question, with Skip on it closed Terminal 2 (run bacf878c);
  - the width-sharing test, with the code changed so the strip and its box took no width of their own before sharing
    (run 92939e2d);
  - the many-tabs test, with `+` following the last tab again (run 122abe11).

  The setup checks the second review added to the double-press tests were not seen failing on their own, and neither was
  the width-sharing test's other half, which checks that a tab area with no shell tabs keeps its minimum width while a
  tunnel pill shows.

Reused: none from the first round; everything above ran again after the `+` change.

Skipped: the workspace-wide Rust run and the end-to-end suites, since only the desktop app's tab strip and the header
layout around it changed; the macOS-only window-drag tests (Linux machine); the desktop app's end-to-end smoke test,
whose steps do not open shell tabs.

### Review gate

First round (before the landing found the double press): reviewed by a fresh-context Opus 5.5 agent at high effort. It
found one real bug (the × on an unselected tab had stopped turning red on hover), two weak test checks, one refinement
and wording issues; all were fixed and confirmed by the same reviewer.

This round: a fresh-context Opus 5.5 agent at high effort, given the changes since the first review and the whole PR. It
agreed with the narrow fallback and the extra frame. It found one small bug (a focused `+` that moved into the scrolling
tabs when a tunnel narrowed the strip was not scrolled into view), one undecided layout change (the way the strip was
made to grow let the tunnel pill keep its whole width while the tabs gave up theirs first, unlike before; now the two
give up width in proportion again, as before this PR, with a test), two test gaps (a missing check of the double-press
test's setup, and nothing checking that the tab strip keeps its minimum width while a tunnel pill is showing) and
wording issues. All were fixed; its verification pass confirmed each and found one wording nit, also fixed. The gate
passed.
