## What this was about

A terminal link's hover panel shows where a link goes or what file Farhelm would download. A menu, dialog, or approval
card could cover part of the panel and the click could still go through. Even when Farhelm did refuse a click, the top
of an approval card could hide the explanation.

The planning decision was to refuse clicks when a surface that takes the pointer covers the panel, keep the old refusal
wording, and raise the explanation over approval cards. During implementation, a test showed that a popover can
disappear on press, so the file click preserves the refusal through release.

A covered destination panel now refuses the click, including opens through a tunnel and file downloads. The refusal says
the details are covered and asks you to move the pointer and hover again. While the pointer stays on the same link, the
explanation appears above approval cards.

## Things you should know

- Farhelm checks the whole panel as drawn, including its border. The pointer can be outside the covering surface and the
  panel can still be covered. If the panel also runs out of the window, the existing resize explanation wins.
- Display-only surfaces that let the pointer through, such as keyboard help and the launch pill, don't count as covers.
  An approval card still accepts clicks and keyboard input while the message is over it. An open confirmation has visual
  priority over the message.
- Moving within the same link can restart its hover while the explanation stays above the approval card. Leaving the
  link returns the explanation to its ordinary place. The click stays blocked even if a covering menu closes before the
  mouse button is released.

## Open questions and possible follow-ups

There is no decision outstanding.

## Pull requests

The three PRs were drafts at handoff. They are listed in landing order.

- Add the rendered-frame overlap check used to tell whether a panel is covered.
  [#4175](https://github.com/scode/farhelm/pull/4175/changes)
- Refuse covered web, tunnel, and file clicks, with wording for the kind of click.
  [#4180](https://github.com/scode/farhelm/pull/4180/changes)
- Make the explanation visible over approval cards and pin both appearances with screenshots.
  [#4186](https://github.com/scode/farhelm/pull/4186/changes)

## Checks

- The vendored GPUI test selection passed all five tests in recorder `f8ad2c83-668d-4cf5-a8cf-dd3561f63e5a`. It checks
  overlap, adjacency, clipped areas and hit areas that let the pointer through. The patch also reverse-applies cleanly.
- The reviewed stack passed 297 terminal, disclosure, file, layer and approval tests on Linux in
  `1e26e51e-c49f-45b7-8264-6930866c706e` with `cargo nextest run -p farhelm-gui --lib`, four slots and zero retries. The
  three targeted covered clicks also passed in `ba0f1681-0e03-424f-b295-550a2dd52322`. Earlier attempts exposed two test
  fixture mistakes. The tunnel test now grants consent before hover, and the file test's cover menu can actually close
  on the outside press. The test for Cmd-click models macOS on Linux.
- `python3 scripts/check-gui-goldens.py --update` captured 260 Linux reference images in
  `b38bed1a-0afa-447a-af5b-bf767b80b437`. The 258 existing references matched byte for byte. The two new images were
  inspected in light and dark, then published. A fresh fetch matched all 260 captures byte for byte.
- GUI Clippy with all targets and warnings denied, Rust formatting, dprint, whitespace, changelog format and the Rust
  test-delay check passed. The agent instructions check reported zero errors and two pre-existing heading warnings.
- The stack was rebased onto `99724fc9`. Main changed documentation screenshot tooling, TODO entries and plan
  bookkeeping. It changed none of the tested code or references. The final tip is `7ad077c6`. The existing evidence
  applies to the rebased stack.
- Skipped the full workspace and native macOS suites. The changes are in the shared desktop UI and its vendored hitbox
  check. The selected Linux tests exercise those paths and include the modeled Mac tunnel click.

## Review gate

All three drafts passed independent code and commit-message reviews. Review led to checking the full panel border,
retaining the press-time refusal across release, keeping one announced status, and letting a keyboard-taking overlay
such as a confirmation stay readable. It also caught a stale hover that could raise an explanation again after the
terminal left the screen. The scope recheck confirmed the new display slot uses the same layer manager as the cards.
