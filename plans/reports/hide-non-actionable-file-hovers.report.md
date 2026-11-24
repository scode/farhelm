# Hide non-actionable terminal file hovers

## What this was about

Terminal output often contains paths that a user can read but cannot download. Passing the pointer over a missing path
or folder could show a checking card and then an error card. The maintainer chose to show the underline, pointing hand
and file card only after the session's host confirms that the file can be downloaded.

## Things you should know

- Hovering starts a silent check. Pending or failed checks, missing files, directories, unreadable files, non-regular
  files and files over the size limit stay quiet. A ready card shows the resolved file name, its host and directory, its
  size and the actions available.
- Pressing before the file card appears does nothing. If terminal output replaces a path while the mouse button is held,
  it takes a fresh click to download, even if the same text comes back. Clicking a path whose card appeared but was
  clipped or covered refuses the download. The terminal asks you to enlarge the window or move the pointer,
  respectively, then hover again.

## Open questions and possible follow-ups

None.

## The PRs

- Show terminal file hovers only for downloadable files. [#4777](https://github.com/scode/farhelm/pull/4777/changes)

## Checks run, reused and skipped

- Recorded nextest ran the 244 selected tests on `a2029e92` (recorder `6707178d-8a0d-4b4b-814c-231aa25f5cae`). All
  passed, including the staged screenshot hover, terminal output replacing the path during a click, cover and clipping,
  cancellation, and web links. The first run (recorder `dca0ece7-a677-471e-815e-2c81bc720c45`) exposed a missing cursor
  field in the new test fixture. The repaired test and subsequent focused runs passed.

  ```sh
  cargo nextest run --locked -p farhelm-gui --lib \
    -E 'test(/^terminal::files::/) | test(/^terminal::tests::/) | test(/^terminal::attached::tests::/)'
  ```

- `python3 scripts/check-gui-goldens.py --jobs 4 --update -j 4` rendered 420 images on `bf72319c` (recorder
  `67967381-ef56-4728-89a4-4092b72b48dd`). Only the light and dark file-card images changed. Both matched the images
  Opus reviewed exactly. `python3 scripts/check-gui-goldens.py --scene terminal-file-disclosures -j 4` passed against
  the published references on `a2029e92` (recorder `280983e3-a6f1-4dc3-b646-e741b013aa5f`).
- `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `dprint check` on
  the changed Markdown and screenshot catalog, `python3 scripts/check-gui-lints.py`,
  `python -B scripts/check-test-sleeps.py` with the isolated interpreter, and
  `python3 releasing/check-changelog.py format` passed on `bf72319c`. The later revision changed two
  screenshot-reference hashes. The draft's committed tree is identical to `a2029e92`.
- The workspace-wide suite was skipped because the change is limited to the desktop terminal. The staged hover test
  exercises the docs screenshot action, so a separate docs recapture was skipped. Reference captures were made on Linux.

## Review outcome

Sol's source review prompted the change that prevents a replacement file check from inheriting an earlier click. The
delta review cleared it. Opus approved the three file-card size tiers in both themes after adding room for their
shadows.

### Landing

#### What landed with it

This was the only plan landed this time: its PR, [#4777](https://github.com/scode/farhelm/pull/4777/changes), plus two
landing fixes, [#4779](https://github.com/scode/farhelm/pull/4779/changes) and
[#4780](https://github.com/scode/farhelm/pull/4780/changes). Main gained nothing but plan bookkeeping between the plan's
base and the landing, so the rebase was trivial and nothing else could interact with it.

#### Fixes made while landing

- **The docs page did not say that some paths now show nothing.** The page on working in a session told readers to hover
  over a path to download it, and nothing else. After this plan, a missing file, a directory, an unreadable or oversized
  file, or a path on a host that does not answer shows no underline and no card at all, where it used to show a card
  saying why. Someone whose relative path stopped matching after a `cd`, or whose host was offline, would see nothing
  and find no explanation. #4779 adds that to the page (Using Farhelm → Work in a session, the part on bringing files
  back) and to the changelog entry, which also now says the underline waits for the host's answer.
- **SPEC.md lost its sentence on which click downloads.** The plan removed the sentence saying which click on a path may
  download or open its file. The rest of the rule still implied it, but #4779 states it again in its new form: the mouse
  button must go down and come up while the same file card is on screen, with the path text unchanged since the hover
  began.
- **A screenshot test was already failing on main.** Besides comparing pictures, the desktop app has a page that shows
  each of its reference screenshots beside the old app's, built from a list of those screenshots. A test that checks
  every screenshot is on that list failed on main before this plan: an earlier change (#4766, which shows a web link's
  full destination on hover) added its screenshot without listing it. #4780 lists it, since the landing's checks ran
  into it and the fix is one entry.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. The plan changes only the
desktop app's terminal, so the landing ran all of the desktop GUI's tests, not just the terminal tests the executor
picked.

On the plan's PR rebased onto main (`1d261db5`), before the landing fixes:

- `cargo nextest run -p farhelm-gui`, on the test worker (recorder run `6d5082be-e134-4ba5-a9ed-7d8be8d053db`): all 3374
  passed; the 4 skipped are tests marked ignored (the screenshot comparisons, run separately below), and no test printed
  a runtime skip.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and the
  same with `--features agent` (the desktop app with its test-automation bridge), on the test worker: passed. Both
  desktop builds include the GUI code the plan changed.
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `b2e8f44f-9b9f-4ba9-9f99-3b9cf9e8c340`): every
  screenshot matched its reference, including the two file card pictures the plan updated.
- `python3 -B scripts/test-gui-screenshot-batch.py`, locally: one failure, the missing list entry described above,
  present on main before this plan; it passed after #4780.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fixes, which change only text (the docs page, SPEC.md, the changelog entry, code comments and a test's
description) and add one list entry: `cargo clippy --all-targets -- -D warnings` on the test worker, since clippy also
reads comments, and locally the website build (`bun run build` in `website`), the formatter, the sleep check,
`dprint check`, the changelog format check and the screenshot list test: passed. The GUI tests and the screenshot
comparison are reused from `1d261db5`, since comments and a test's description cannot change what they run.

Skipped: the workspace-wide Rust suite, since nothing outside the desktop app's terminal changed and every crate that
builds it in compiled cleanly under clippy; the end-to-end desktop smoke, none of whose steps hovers over a file path; a
fresh capture of the docs page's screenshot of a file card, since a GUI test that drives the same staged file hover
still passes; macOS, where the change has no platform-specific code beyond the ⌘/Ctrl hint the tests already cover for
both; and CI, which runs only on demand.

#### What the report says that is no longer true

Nothing. The review found the report accurate.

#### Things you should know that the report does not say

Below, a ready path is one whose host has confirmed the file can be downloaded. These three follow from the decision to
show nothing for any other path, and I suggest no change for now:

- **A failed check is completely silent.** If the host is offline, the check times out or the helm refuses the request,
  nothing on screen says why the path is not clickable. The docs page now says so.
- **On a slow host, the path is not underlined until the host answers.** Before, the underline and a checking card
  appeared at once.
- **A "not found" answer sticks while the pointer stays put.** If an agent finishes writing the file a moment after you
  hover, the card appears only once you move off the path and back.

One small change goes beyond what the plan asked for: while a menu or dialog holds the keyboard, a ready path gets its
underline and pointing hand but no card, and a click on it now does nothing. Before, the click showed the "full file
details are not visible" explanation.

Two odd behaviors were already there before this plan and are unchanged by it:

- The report's sentence about a card that "appeared but was clipped or covered" is about a card that is drawn but does
  not fit, or sits under another surface such as an approval card; a click then gets the explanation, as it says. In a
  window so small that the card could only be placed over the pointer, Farhelm instead takes the card away right after
  drawing it, and a click on the path then does nothing at all, with no explanation, though a web link in the same spot
  does get one.
- After Escape dismisses a file or web link card, a click tells you to enlarge the window, which is odd advice after
  Escape.

#### Possible follow-ups

- Giving a path that cannot be downloaded some quiet sign (a different pointer, or a short note after a moment), if the
  silence proves confusing in practice.
- Giving a click on a file path in a too-small window the same explanation a web link gets there.
