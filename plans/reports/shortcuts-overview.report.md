# Keyboard shortcuts overview

## What this was about

Farhelm's shortcuts were scattered across individual controls and documentation pages. You can now open a grouped
Keyboard shortcuts dialog with Cmd+/ on a Mac, Ctrl+Shift+/ elsewhere, or Keyboard shortcuts immediately after
Documentation in the ? menu. It lists session, launching and terminal keys, including the current marked and numbered
templates. Close, Escape and a click outside return focus to the terminal or Help button that opened it.

## Things you should know

The dialog shows the keys this client handles. A browser on a Mac uses Command for the quick switcher, template list,
marked-template shortcut, overview and terminal text size, but Ctrl+Shift for Delete, Rename, numbered templates and the
launch-sequence prefix. New's Cmd+N appears only in the Mac desktop app. Existing shortcut matching and tooltip wording
are unchanged. Template assignments refresh once per opening, preserving cached names if the read fails.

The new Keyboard shortcuts documentation page covers the Mac desktop app, Mac browsers and other clients, with links
from Start a session, Session list, Launch templates, Work in a session and Stop, restart and resume. The specification,
changelog fragment and original TODO entry are updated. Typed template names in launch sequences are not on the main
branch used here, so their additional keys are outside this overview for now.

## Open questions and possible follow-ups

No required behavior was omitted. Browser validation exercises the web build with both platform forms; it does not
establish native Mac keyboard dispatch or override keys a browser reserves. The ? menu provides the alternative entry.

## The PRs

- [#2827 — feat: add keyboard shortcuts overview](https://github.com/scode/farhelm/pull/2827/changes), draft, head
  `f2e1849a537e6693dc8b126f7021f2fb6b3db38e`.

## Checks run, reused and skipped

The two reused runs below covered working-copy revision `630b097c748d8dc7aed94f18ed2cf26bb435a136` on base
`ea09a05f113823408c6bca6f0ea64be8e1178d20`, with recorder source fingerprint
`d9994a6a8429e7b023834e9164ad53517a453fb513cc01e87d7fd116e5a17fef`. The later overview runs covered the PR's final head.

- Recorded `cargo nextest run -p farhelm-ui --lib -E '<label and Help-order selections>'`: three selected tests passed,
  821 selected out, run `027605f4-85c6-45f4-84bf-79cc602a46f2`. This covers the client platform/build distinction,
  shared labels and Help ordering. Reused after the generic number row was added: none of those tested functions or
  expectations changed.
- Recorded `npx playwright test '<feedback, tooltip-coverage, modal-trap and modal-focus specs>'`: 42 passed, 21 per
  engine, run `35778ee1-5f2d-4309-b363-6252646c8faf`. Reused after that presentation-only row addition; the four
  regression specs and their exercised menu, tooltip and modal behavior did not change.
- Recorded `npx playwright test 'keyboard-shortcuts.spec.ts'`: run `cbeb6d56-1e0f-47d1-9984-6ad35eb97a8e` was
  interrupted when the recorder received SIGTERM, with 12 passes, one interrupted WebKit case and one unstarted WebKit
  case. The cause of the signal is unknown. Cleanup required escalation; the recorder reported no remaining cleanup
  limitation. This is not a passing batch, and its complete record, screenshots and trace are retained privately.
- The exact authentication/preferences and binding-refresh cases were then selected with `-g` on both engines in run
  `56b6f4b3-8857-467b-a656-a1ef26c9f762`: four passed. Both attempts used the same final source and pinned tmux 3.7c.
  Together they provide passing observations for all 14 new cases without replacing the interrupted result. Reports and
  output are complete, with no omitted bytes or runtime SKIPPED markers. Chromium and WebKit screenshots were inspected.
- `cargo fmt --all`, UI all-target Clippy, final UI library Clippy with warnings denied, the backend and release web
  builds, Markdown dprint, changelog formatting, and the website build/link validation passed. The final isolated
  `python -B scripts/check-test-sleeps.py` inspected 327 delays with no missing rationale. An earlier repeat of that
  source check exceeded its unchanged time budget; the later pass is a separate observation.
- The full test battery, unrelated Rust targets, desktop runtime and separate JS unit harness were skipped: no affected
  behavior needs those scopes, no JS asset changed, and the focused browser runs exercise the new inline listener.
  Quick-switcher matching was unchanged. The project supplies no TypeScript compiler or TypeScript dprint plugin, so
  neither was claimed as validation. No preview server was started for this unattended run.

## Review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort passed the final change. Its one finding was addressed:
the direct number shortcut now remains discoverable even when no templates have assigned keys, with individual rows only
for assigned templates. An independent cold reader accepted the commit/PR wording. A separate documentation pass covered
every touched file. Fresh main changes were release-note curation and queue bookkeeping, with no product-code
interaction requiring a rebase.
