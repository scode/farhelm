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
