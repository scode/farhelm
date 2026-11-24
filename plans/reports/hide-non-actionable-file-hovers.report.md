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
