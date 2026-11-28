# Report: terminal tabs stay visible and reachable when they overflow

## What this was about

When a session had more shell terminal tabs than fit beside the session header's toolbar, the tab strip simply scrolled.
The main Agent (or Shell) tab scrolled out of reach with the rest, and nothing showed that tabs were hidden. A new tab,
a restored one or the tab chosen after a close could land out of view, and only some keyboard paths scrolled the chosen
tab into view.

You decided on: the main tab pinned at the left so only shell tabs scroll; the active tab always scrolled into view;
and, while tabs overflow, subtle edge fades, `<` and `>` scroll buttons and a `▾` button opening a list of every tab
with switching and closing in one click. Two PRs build it.

### What changed for someone using Farhelm

- **The main tab stays put.** Agent (or Shell) sits at the left of the strip and New terminal at the right; only the
  shell tabs between them scroll. In a very narrow strip (650 px beside a tunnel pill, as SPEC.md allows) both ends join
  the scrolling part, so the toolbar and the tunnel pill stay whole.
- **The shown tab is always scrolled into view:** when you open a tab with New terminal or Cmd+T, click or switch to
  one, close one (the tab chosen next), choose one from the list, or reopen a session that remembers its tab. Scrolling
  by hand afterwards is never pulled back, unless the strip's width changes enough to switch between the pinned and the
  narrow layout, which scrolls the shown tab into view again.
- **Overflow controls.** While the shell tabs do not fit:
  - each edge that hides tabs fades out;
  - `<` and `>` step the strip by most of its visible width and are disabled at the ends;
  - `▾` opens a list of every tab, the shown one ticked, with a separate close button on each shell tab. The controls
    disappear as soon as the tabs fit again. In tighter strips `<` and `>` go first and `▾` stays. Closing a tab from
    the list closes the list first and asks under `▾` (or in the middle of the window), following "Skip session
    confirmations" as the tab's own close button does.
- **Keyboard.** The scroll buttons are ordinary tab stops; when a step reaches the end, focus moves to the other scroll
  button rather than falling into the terminal. The list keeps focus inside it, starts on the shown tab, and Escape
  returns to `▾`. Screen readers hear which row is the shown tab.

SPEC.md's session view section, SPEC_impl.md and the changelog entry describe all of this. The changelog entry opens
with the release note you settled in planning.

## Things you should know

- **The `▾` button shows a chevron only.** The plan gave "tab count + chevron" as an example. A visible count roughly
  doubles the button's width, and at common pane widths that pushes it out of the strip entirely, so the count is in the
  button's tooltip and accessible name instead.
- **New terminal unpins at somewhat more widths than before.** The strip now pins both ends only when there is room for
  the main tab, New terminal and about one tab. Below that (roughly where a strip beside a tunnel pill reaches its
  narrowest), New terminal scrolls too. A middle layout (New terminal pinned, main tab scrolling) would leave less room
  than one tab there, so I kept the two layouts.
- **No `▾` in the narrowest strips (a departure from the plan).** The plan says tight strips drop `<` and `>` first
  "while keeping `▾` so 1-click navigation stays available". That holds until the strip gets very narrow. In two cases
  the strip shows only the edge fades and no `▾`:
  - a band about 28 px of strip width wide where the main tab and New terminal are still pinned but the room between
    them is less than one tab plus the button;
  - the narrow layout, where the main tab and New terminal scroll with the rest (as at 650 px beside a tunnel pill, a
    strip of about 112 px).

  In both, a `▾` would take most of the room of the only visible tab. The tabs stay reachable by scrolling and by the
  tab-switching shortcuts. Whether a `▾` should be squeezed in anyway is a follow-up below.
- **A late tunnel pill.** Testing showed that the tunnel pill can appear a few frames after a session opens and narrow
  the strip into the other layout. The strip now notices that its visible part moved and scrolls the shown tab into view
  again, which is also why a layout switch re-reveals the tab (see above).
- **The shared menu row gained one option (a departure from the plan's note).** The plan said not to generalize the
  shared menu row component and to build the list's rows locally. The rows' layout is local, but to let screen readers
  hear which row is the shown tab, the shared row gained a "report as pressed" option. It is off by default, so every
  other menu that uses the row is unchanged.
- **Fades never cover the focused control.** A tab or close button reached by keyboard at the strip's edge is not faded.
- **Golden screenshots.** I added two new golden scenes, the overflow controls and the open list, each in light and
  dark. The 650 px toolbar scene changed: its condensed row now shows a right-edge fade. All of them were checked by eye
  and published to the goldens storage. In the toolbar scene, the chosen tab scrolled to the right edge has its last 16
  px faded; I judged that acceptable, but it is the one picture worth a look.
- **Not run on a Mac.** All checks ran on Linux. How the fades and the list render on macOS was not checked.

## Open questions and possible follow-ups

- **A `▾` in the narrowest strips.** It could take the visible tab's room in the band, or New terminal's place in the
  narrow layout, at the cost of seeing at most part of one tab. I recommend leaving it out unless narrow panes beside a
  tunnel pill with many tabs turn out to be common for you.
- **A visible tab count on `▾`.** If you want it despite the width cost, it is a small change; I recommend leaving it in
  the tooltip.

## PRs

- #5176: feat: pin the main terminal tab and reveal the chosen tab.
- #5177: feat: add overflow controls to the terminal tab strip (also removes the TODO entry).

## Checks

Run now, on the final stack just before its rebase onto `main`:

- The whole desktop GUI crate's tests (`cargo nextest run -p farhelm-gui --lib`): 3625 passed (recorded run
  8fde238a-19a9-4ce1-a436-981bdc76e643). They include the new tests of pinning, both layouts at 650 px beside a tunnel
  pill, a new tab revealed on its first frame, a restored tab revealed beside a late pill, the controls appearing and
  going away, stepping and its ends, focus handoff at an end, the list's contents, choosing and closing from it, and the
  close question's centered fallback.
- The golden screenshot comparison of the terminal and toolbar scenes: no differences from the published references
  (recorded run 061687ed-d50d-420b-81f9-7d887ab428a0). The full update that produced and published the new and changed
  pictures ran on #5177 before its review fixes (recorded run c0a15a6d-1ee0-405c-8266-f68db19ea820); the later
  comparison shows the fixes changed no pixel.
- `cargo clippy -p farhelm-gui --all-targets`, the GUI lint check, `cargo fmt --check`, the test-sleep check,
  `dprint check` on the changed Markdown and the screenshot scene list, the changelog format check and the screenshot
  batch self-test: clean.

Reused:

- **The rebase onto `main`.** The stack was rebased after these runs. What landed meanwhile is three drift-test PRs (the
  conversation rotation and persistent hook categories), which touch no GUI code, so the runs above still apply.

Skipped:

- The GUI end-to-end smoke and the rest of the batteries. The smoke's steps do not exercise terminal tabs, and the
  change is confined to the desktop app's tab strip, which the crate's own tests and the goldens cover.

## Review gate

Each PR had a fresh-context Opus 5.5 review at high effort, with the plan's charter and the test-authoring checklist.
Same-model sub-agents implemented the code and the fixes; I reviewed, committed and pushed.

- **#5176:** one round. No high findings. The medium: when the strip switched between its pinned and narrow layouts in
  the same frame a tab was chosen, the chosen tab could end up out of view; fixing it uncovered the late tunnel pill
  above, and both are now handled and tested. Five lows were fixed or answered:
  - a keyboard reveal looking at the old layout;
  - New terminal unpinning at more widths (kept two layouts, as above);
  - a restore test that did not prove the tab had been hidden;
  - the first-frame reveal not being tested;
  - re-choosing a half-hidden tab not redrawing. Of seven nits, one (moving a test helper) was left, because it would
    rewrite lines #5177 builds on.
- **#5177:** one round. No high findings. Two mediums were fixed, each with a test that fails without the fix:
  - a chosen or restored tab left clipped on the frame the controls first appear;
  - focus falling into the terminal when a scroll button disabled at its end. Five lows were fixed:
  - the controls not appearing after a strip widened out of the narrowest band until something else redrew;
  - screen readers not hearing which list row is the shown tab;
  - a fade covering a keyboard-focused close button;
  - three test gaps;
  - an overstated comment. Four of five nits were fixed; the list keeps borrowing the notifications bell's height cap,
    which other menus share.

### Landing

#### What landed with it

Nothing reached main between the executor's last rebase of this plan and its landing, apart from the queue's own
bookkeeping. It was the only plan in this round.

An independent reviewer and I read the change against what was already on main, mainly the shell tab shortcuts that the
terminal-tab-shortcuts plan added on 2026-11-27. The new-tab shortcut and the tab-switching shortcuts now bring the
chosen tab into view the same way a click does. The close-tab shortcut (Cmd+W on a Mac) still scrolls the close button
of the tab it is about to close into view before asking, counting tabs correctly now that the main tab sits outside the
scrolling part. While the all-tabs list is open, those shortcuts do nothing, as for any open menu. Closing a tab from
the list asks, or skips the question when "Skip session confirmations" is on, exactly as the tab's own close button
does. The shared menu row's new option is off unless asked for, so every other menu is unchanged. The session header's
width fitting is untouched, and the strip takes the same width with or without the overflow controls, so the 650 px
layout beside a tunnel pill still fits. We found nothing that had to be fixed before landing.

Worth knowing, from the review:

- No `▾` in the narrowest strips, which the report already raises as a follow-up and SPEC.md now describes. The case
  that matters most is a 650 px pane beside a tunnel pill: there the tabs are reachable only by scrolling and the
  tab-switching shortcuts.
- The edge fade also covers the chosen tab. A tab brought into view lands flush with the strip's edge, so when more tabs
  lie beyond it, the 16 px fade washes that edge of it: at the left edge its mark and highlight but not its name, at the
  right edge the last few pixels of its name too. Fades are left out only for the control keyboard focus is on. The
  report mentions the right-edge case in one golden picture; it happens at the left edge too.
- A close question asked from the list hangs under `▾`. If the tabs stop overflowing while it is open (another tab's
  shell exits, say), `▾` goes and the question closes with it, and you have to ask again. Nothing is lost, but the tab's
  own close button does not behave that way.
- Apart from Escape closing it, the new tests drive the list by clicks only: choosing a row with Enter or Space, moving
  through it with the arrow keys, and Tab staying inside it are not tested.
- One sentence in SPEC_impl.md, on what the strip scrolls into view when keyboard focus reaches it, lists New terminal
  alongside the tabs as if it could be out of view in the normal layout. It moves with the tabs, and so needs scrolling
  to, only in the narrow layout. A one-line doc fix.

Of these, I would take up the untested keys and the SPEC_impl.md sentence as small follow-ups, and leave the fade and
the vanishing question unless they bother you in use.

#### Fixes made while landing

None.

#### What the landing made untrue in the report

Nothing.

#### Checks

Run once, on the plan's two PRs as they landed (main had gained only queue bookkeeping). The GUI crate's tests, the
goldens and the other checks ran locally; the desktop app's end-to-end smoke ran on one boat:

- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`: clean. `cargo clippy -p farhelm-desktop --all-targets`
  without `-D warnings`, because one warning already on main since #5120 (an unused method in the helm's ntfy settings
  code, outside this plan) would fail that flag. That was the only warning.
- The desktop GUI crate's tests (`cargo nextest run -p farhelm-gui`), recorded run 59ab28b8-43f6-46fd-8b5f-0b08430a521e:
  3625 passed, none failed.
- The screenshot goldens (`python3 scripts/check-gui-goldens.py`), recorded run 8b42d061-a52e-4f44-a913-8d9c28d6c8f3: no
  differences from the published references, the plan's new and changed pictures included.
- The desktop end-to-end smoke (`scripts/gui-e2e-smoke.py`, headless on Linux), recorded run
  74cbb83a-886e-4387-af35-fb9cb72b7444: 49 ran and passed, 1 skipped (the paste test, which cannot own the clipboard in
  a headless session).
- The screenshot scene list's self-test (`python3 -B scripts/test-gui-screenshot-batch.py`, since the list gained the
  plan's scenes): 47 passed.
- `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`), `dprint check` on SPEC.md,
  SPEC_impl.md, TODO.md, the changelog fragment and the scene list, and `python3 releasing/check-changelog.py format`:
  passed.

Reused: nothing. Skipped: the rest of the Rust workspace's tests, since nothing outside the desktop app changed; and any
run on a Mac, since none was available to the landing, so how the fades and the list look on macOS is still unchecked.
