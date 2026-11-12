## What this was about

In the desktop app, Cmd-click (Ctrl-click on Linux) on a file path in a terminal downloads the file and also opens it in
your default app, but only when its type is on the **File types to open with Cmd-click** list in Settings. That list
defaulted to `md, txt, json`. Your TODO entry asked for common image formats to be added to that default, with SVG left
out because it is HTML-like and already warned about. It also noted that the settings dialog's warnings had to treat the
new types as safe, or the dialog would warn about its own default. On the 2026-11-10 planning page you chose png, jpg,
jpeg, gif, webp, bmp and avif, and turned down adding heic, tif and tiff, and also turned down adding only png, jpg and
jpeg.

What now lands:

- **New default.** The default list is `md, txt, json, png, jpg, jpeg, gif, webp, bmp, avif`.
- **No warnings for the new image types.** These seven types used to get Settings' generic warning for unrecognized
  types: that Farhelm does not restrict what the default app does with the file and that opening an agent-written file
  may run code. They now count as safe and get no warning wherever they appear, so the default is warning-free. SVG
  keeps its warning that a web page can run scripts.
- **Specs, docs and changelog.** SPEC.md and the docs page "Read the session list" (its "Terminal links" section) say
  the same, and a changelog entry describes the change. The TODO entry is removed.

## Things you should know

- **Saved lists are left alone.** Only a helm whose list was never saved picks up the new default; nothing migrates
  saved lists. The plan required this, and the planning page stated it without objection from you. Settings shows the
  default in the field, and pressing **Save** stores whatever the field shows, edited or not. So anyone who ever pressed
  Save on the untouched field has the old three types stored and will not get the images. The changelog entry says so
  and tells them to add the types themselves.
- **No warning for hand-typed image types either.** Because the seven types now count as safe, someone who types them
  into their own list also no longer sees a warning beside them. The planning page stated this, and you did not object.
- **A dated SPEC decision was amended.** SPEC.md's confirmed decision of 2026-10-18 about Cmd-click opening, under
  "Local authority and trust between hosts", fixed the default at `md, txt, json` and the warning-free types at those
  plus `log`. It was amended in place. A dated note says the change was made at your request, both for the default list
  and for which types get no warning.

## Open questions and possible follow-ups

- **Saving the untouched field freezes the old default.** If you want everyone who pressed Save without editing to get
  the images too, a follow-up could make a save of the unedited default keep following the default instead of storing
  it. Nothing in this change depends on that.
- **Empty TODO.md heading.** The "Trivial" subsection of TODO.md's "Near term" list is now empty; its heading was left
  in place for you to keep or remove.

## PRs

- #3351 feat: open common image types on Cmd-click by default.

## Checks

Run now, on the final change:

- **Unit tests, open list and downloads:**
  - Command:
    `record-test-run.py --runner nextest -- cargo nextest run -p farhelm-proto -p farhelm-helm --lib -E 'test(/^open_extensions::/) | test(/^downloads::/)'`,
    run 51152a4b: 22 passed.
  - Covers a new test that every default type is safe and SVG is not.
  - Covers the helm's open check, extended with image files (upper and lower case), an SVG that must not open and a
    `.png.command` file that must not open.
- **Unit tests, settings warnings:**
  - Command:
    `record-test-run.py --runner nextest -- cargo nextest run -p farhelm-ui --features desktop --lib -E 'test(/open_extensions_tests::/)'`,
    run 603824d5: 4 passed.
  - These tests build only with the desktop feature. They now also check that the default shows no warnings.
- **Static checks:**
  - `cargo fmt --all -- --check`
  - `cargo clippy -p farhelm-proto -p farhelm-helm --all-targets -- -D warnings`
  - `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`
  - `check-test-sleeps.py`
  - `releasing/check-changelog.py format`
  - `dprint check`

  All passed.

Reused: none.

Skipped:

- **The browser suite and the desktop app's end-to-end smoke tests.** The change is a default value and the list of
  which file types count as safe, both covered directly by the unit tests above. No browser test checks which file types
  open, since opening only happens in the desktop app.
- **The docs website build.** One sentence on an existing page changed, with no links or headings touched.

## Review gate

A fresh Opus 5.5 reviewer at high effort found nothing blocking. It raised two real wording problems, both fixed:

- **A caveat the changelog entry missed.** Saving the field unedited keeps the old list.
- **A SPEC sentence that could be read the wrong way round.** In the amended decision, it could be read as giving the
  image types a warning rather than exempting them.

Its point about whether the SPEC amendment's date should be the UTC or the local date was left as is: it uses the UTC
date, as this plan's recorded decisions do. The change was not reviewed again after the fixes; the checks above ran on
the fixed code.

### Landing

#### What landed with it

It landed in one round with session-action-leftovers (two fixes to how a session remembers a finished Restart and
Replace with's progress) and version-dblclick-flake (a browser test waits for the terminal's focus); neither touches the
Cmd-click list. While the round ran, a docs refresh (#3349) landed on main and edited the same docs page, the session
list's, in other paragraphs; the two edits do not overlap and the page reads correctly with both. A change to a
docs-screenshot test (#3347) also landed and touches nothing here.

The landing's review checked that the new default list (`md, txt, json, png, jpg, jpeg, gif, webp, bmp, avif`) and the
types that show no warning are the same everywhere they are stated: the shared protocol code, the helm, Settings, the
three places in SPEC.md (the confirmed decision is amended with a dated note rather than rewritten), the docs page and
the changelog entry. No fixes were made while landing.

#### Possible follow-ups

- One helm test still checks only that the message the helm gives for a file type not on the list contains
  `md, txt, json`. It passes because the new default starts with those; making it check the whole list would close that.

#### Checks

The round's three plans were checked together, stacked in queue order on main (session-action-leftovers, then
version-dblclick-flake, then cmd-click-images; their TODO.md conflicts were resolved by keeping each plan's removal of
its own entry). Browser runs were on Tensorlake sandboxes at the maintainer's request; the sandboxes run Node 24.16
where the development machine runs 24.20, which these tests do not depend on, and each shard is recorded in the
recorder's mode for arbitrary commands.

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, the docs website build (`bun install --frozen-lockfile` and
  `bun run build`), `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo check -p farhelm-ui --features desktop` passed.
- `cargo nextest run -p farhelm-helm -p farhelm-proto -p farhelm-ui --lib`: 2,978 passed (run
  `64f04ec9-18bc-4bb4-bdd1-fb2503b7cbb9`).
- The whole browser suite, `npx playwright test` split by project over ten sandboxes, because three plans' interactions
  were checked in one run and session-action-leftovers changes how every session view keeps a finished Restart: 3,296
  passed, 14 skipped, none failed (runs `4f802ef7-91db-4b47-9622-41cf69ad4216`, `ff5047d5-a356-4d6d-bd9e-e6d79ddb5e00`,
  `97413f07-ea76-496b-8a26-64ff1045e28b`, `ee9ea862-1e4c-437f-bedd-f90e8c07713f`,
  `8e430cf6-69d0-4c89-9e08-09f390b77eea`, `5dd8460f-f24d-4845-94ea-23b92f1697dd`,
  `87bed0c7-b106-4e86-9a62-9122c45e4de0`, `7f818c77-32af-40f4-afeb-2dc08cc60aaf`, `8bb6cc96-5151-42f9-a4c4-9385246c5f1d`
  and `517ea6a3-9db7-4f5b-ac17-f5139f5ffae6`). The 14 skips are as many as in earlier whole-suite runs.

Main differs from the tested tree only by the docs refresh (#3349) and the docs-screenshot test change (#3347), which
landed while the round ran; the docs refresh and cmd-click-images both edit the session list's docs page, in separate
paragraphs, and the website was not rebuilt with both edits together. Skipped: the workspace Rust tests beyond the three
changed crates' library tests (nothing else reads what changed), the old-terminal browser set (nothing here touches old
terminals), and the desktop app itself (the Cmd-click list is shared code the library tests cover).
