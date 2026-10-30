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

### Landing

Landed as delivered, with one test fix at landing (below). A keyboard shortcuts overview now lists every app-wide
shortcut in one dialog. It opens with Cmd+/ on a Mac (app or browser) and Ctrl+Shift+/ elsewhere, or from a new
**Keyboard shortcuts** item in the **?** menu, shows the chords the current client actually uses (the Mac app, a Mac
browser, or other clients), and covers the keys inside a launch sequence, including your assigned launch templates. A
new docs page, Keyboard shortcuts, carries the same tables, and five guide pages link to it.

- #2827: the dialog, its shortcut and menu item (pressed during a launch sequence, the chord cancels the sequence, as
  any key the sequence does not know already does), its tests, the specification text, the docs pages and a changelog
  entry.

#### What landed since the stack was based, and whether it interacted

Only the v0.32.0 release-notes draft (#2824) and plan bookkeeping landed since; nothing interacted. settings-gear-shape
landed right after this plan in the same round; the two share nothing but bookkeeping.

#### Fixes made at landing

- The docs-screenshot visual review (`e2e/docs-shots/visual-review.spec.ts`, which is not part of the browser suite)
  still expected five items in the web **?** menu; with **Keyboard shortcuts** there are six, so the next docs
  screenshot capture would have failed at its first menu picture. The count and its comment now say six. The executor
  had updated the browser test that counts them but not this one. The fix went in after the browser suite had run, and
  the visual review itself was not run: it needs passwordless SSH to the screenshot scenario's remote hosts and is not a
  gate; the change is a count that the browser suite's own menu test (which passed) checks the same way.

#### Things you should know

- Safari binds Cmd+/ to Show/Hide Status Bar, and the browser tests cannot show whether a page still receives it there.
  The docs and the specification point to the **?** menu as the other way in; the changelog entry does not mention it.
- Nobody has pressed Cmd+/ in the native Mac app yet; the review found no binding in its native menu that would take it.
  A devbuild check would settle it.
- typed-quick-launch (the plan for launching by typed template names), now in flight, tells its executor to add its
  typed-name keys to this dialog but not to the new docs page's "Inside a launch sequence" table; landing it needs a
  check of both.
- The Send feedback docs page walks through every **?** menu item except the new one.
- Smaller edges: the dialog writes the text-size chords as "⌘+Shift+−" and "⌘+Shift++" while the docs spell them two
  other ways; the new menu item reuses the icon of **SSH needs 2FA?** two rows below it; the docs page has no screenshot
  of the dialog; the browser test that checks every control has hover text never opens the new dialog.
- If v0.32.0 is cut from a main that includes this, its already-drafted release notes will not mention the overview
  unless the release step picks up the new changelog entry.

#### Checks

This round landed shortcuts-overview and settings-gear-shape, stacked in that order on main `cb426c6a`. On that stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint, the binaries' lint,
  the desktop-build checks, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (three warnings, all about citations already on main), the docs website build
  (`bun run build` in `website/`), the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh`: all passed. The check that every test delay states its reason
  (`python -B scripts/check-test-sleeps.py`) first stopped on its own time budget while the machine was heavily loaded,
  then passed when rerun alone.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `32aa6a58-e03d-48bc-bdc9-9f9d045c3e6b`: all 825 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `fb44ab43-fa4d-4466-a39a-9ca2b6a8e263`: all 1008 passed. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `db8e29b7-e456-4fee-bd94-756914f90cbd`: all 372 passed.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `8f80714f-9f10-46f5-af67-be60e573be31`: 2818 passed, 14 skipped by design, 2 failed,
  both on WebKit and both in parts these plans do not change. "composer folder dropdown preserves keyboard launch and
  grouped history" is a recorded open flake. In the Send feedback dialog's test, "the dialog refuses blank and over-long
  messages without sending" failed on the disabled Send button's color, exactly as it did in an earlier full run on a
  stack without these plans (run `0a494db0-73b5-462c-8e8d-33510baf9df6`, 2026-10-29); the shortcuts overview edits the
  same spec file but only its Help menu test. Repeated five times alone on WebKit, runs
  `756ea4b1-29d9-4954-b8a0-31d9cae25dee` and `06d4ac29-871d-480f-9f75-6e1b2364758f`, each passed every time. Recording
  the Send feedback case as a latent flake is a follow-up to this landing.
- The shortcuts overview's visual-review count fix at landing went in after the browser suite had run. Nothing was rerun
  for it: the file is outside the suite, and the visual review needs passwordless SSH to the screenshot scenario's
  remote hosts.
- Reused: the whole Rust workspace run of 2026-10-30, `f0b300d3-7298-4a62-acbe-29bcb28a82ac`, on revision `b5cbad6f`
  (4555 passed, 41 skipped by design, and the known flakes "shallow pause resumes without reset or replay" and "a hung
  save releases waiters at their deadline" failed), for everything outside the UI crate. Since that revision main gained
  only UI-crate changes (#2816, #2817), browser tests (#2818), docs and bookkeeping, and both plans change Rust only in
  the UI crate, all covered above. Skipped: the desktop smoke tests (no desktop-specific code changed) and the native
  Mac app (not runnable on Linux).

The report's checks were the executor's selected runs; this landing ran the whole browser suite with both plans stacked.
Nothing in the executor's report became untrue at landing.
