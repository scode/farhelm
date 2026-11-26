# Report: make Wayland text-field paste nonblocking

## What this was about

On Linux under Wayland, pasting into one of the desktop app's text fields (the SSH sign-in prompt, a session's title,
the launcher's fields, Settings) could freeze the whole window. When another app holds the clipboard, the text arrives
through a pipe that app writes whenever it gets around to it. The field waited for it on the window's own thread, for up
to four seconds per paste. SPEC.md ("Terminal experience") already said Farhelm must stay responsive during clipboard
operations, so this was a bug against the spec. The terminal's paste was already asynchronous; only the text fields were
not. There was no triage decision or earlier Decisions entry beyond the plan's own: split the clipboard API in the
vendored GPUI rather than rebuild the paste handling of GPUI Kit (the toolkit the text fields come from) in Farhelm, one
Opus 5.5 high review gate, and no release note of the plan's own.

What the PR does, in product terms:

- A text field's paste no longer waits on the window's thread. GPUI Kit pastes through the synchronous clipboard read
  and only falls back to an asynchronous read when that gives nothing. On Wayland the synchronous read now answers only
  Farhelm's own copy. For another app's clipboard it gives nothing, so the field takes the asynchronous read and the
  window stays live while the text arrives.
- On a clipboard holding several kinds of thing at once, text fields still get the text. Two examples are a file
  manager's copy, which carries both the files and their paths as text, and a browser's image, which comes with a
  caption. The asynchronous read the terminal used puts files and images ahead of text, so the vendored GPUI now has two
  asynchronous reads:
  - the text read, for text fields: text first, then an image, with Windows-style line endings made Unix ones as before;
  - the payload read, for the terminal: files first, then an image, then text.
- The terminal now uses the payload read, so its paste behaves as it did: a copied file is attached, not typed as a
  path.
- Both ways of pasting into a field go through this path: the keyboard shortcut and the Edit menu's Paste.

The vendored GPUI changes went into the existing patch layer that already owned the clipboard read order. Its patches
were regenerated and reproduced from the published crates. The PR also documents the Linux clipboard tests' command and
removes the TODO entry.

## Things you should know

- The plan asked for no release note of its own, but every `fix` PR carries a changelog fragment, so the PR adds one. It
  says that pasting into a text field on Linux no longer freezes the window while the app you copied from is slow, and
  that such a paste is dropped if you type, move the cursor, leave the field or paste again before the text arrives.
- New behavior, now in SPEC.md: on Wayland, a paste into a text field is dropped if, before the text arrives, the user
  does any of these:
  - types in the field;
  - moves its cursor;
  - leaves the field;
  - pastes again.

  This is GPUI Kit's own check against text landing somewhere the user no longer means. With the old blocking read it
  could never come up. One effect is that a doubled or held Ctrl+V pastes once rather than twice. The text normally
  arrives within milliseconds, so this should rarely show.
- The macOS side of the vendored change could not be compiled here, since that tree builds only on a Mac. It is a small
  change: the files-first read moves from one method to the other, and text fields there keep reading text first, as
  they always did on macOS. The release's macOS build would catch a compile error, as would a devbuild on the Mac.
  Nothing on a Mac has run this code yet.
- No test ran against a real Wayland compositor or another real app's clipboard. Three things exercise the change
  instead:
  - the vendored Wayland code's unit tests, which check the order in which each read asks for the clipboard's content
    types, using stand-in clipboard contents;
  - the desktop app's tests, through a new test-only hook in the vendored GPUI that makes the test clipboard behave like
    another program's Wayland clipboard (nothing at once, contents later);
  - a terminal test that would fail if the terminal used the text read. A deliberate mutation check confirmed it does
    fail then.
- Anything that reads the clipboard synchronously on Wayland now gets nothing while another app holds it. Nothing in
  Farhelm does so today, outside tests. GPUI Kit offers a paste hook that uses only the synchronous read, and Farhelm
  does not use it. The vendored read's documentation now says to use the asynchronous reads.
- Middle-click paste from the primary selection still reads synchronously on Wayland. Farhelm does not read the primary
  selection anywhere, so it was left alone.

## Open questions and possible follow-ups

None that need a decision. Running the macOS build, or a devbuild on the Mac, before the next release would confirm the
uncompiled macOS change.

## PRs

- #5069 — fix: keep wayland text-field paste from freezing the window (draft).

## Checks

Run now, on the PR's commit or a version that differed only as described:

- `cargo nextest run -p farhelm-gui --lib` on the text field tests (`components::tests::text_input::`), recorded run
  3e5d3563-ca28-4f9f-bde9-be601e19336d on the final commit: 21 passed.
- The same selection plus the terminal's paste tests, recorded run 4d8880a2-99f8-44a1-b36a-07019d76d6b2: 29 passed,
  including the new terminal test. Since then only the text field test file changed, by a type alias.
- `cargo test -p gpui-pre-linux --lib attachment_clipboard`, the vendored Wayland clipboard tests, recorded run
  c1b3246e-25ec-4bfa-9147-189ec53891c1: 7 passed, the two new ones included. Since then that file changed only by
  formatting.
- Mutation check, recorded run d95d05f7-920d-48a6-9f93-68ed08b1417f: with the terminal switched back to the text read,
  the new terminal test failed at "the file wins over the text beside it", as intended. The change was then reverted.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`: clean on the final commit.
- `cargo fmt -p farhelm-gui -- --check` and `rustfmt --edition 2024 --check` on the touched vendored files: clean.
- Reproducing the three vendored GPUI trees from the published crates and every patch layer in the README's order, then
  comparing each with the checkout: identical, from the PR's patch files.
- `dprint check` on the changed Markdown, `python3 scripts/check-agents-md.py` (0 errors; its one warning is an old one
  about a frozen file) and `python3 releasing/check-changelog.py format`: passed.

Skipped:

- The full workspace and end-to-end batteries: the change is confined to the clipboard path the targeted tests cover.
- The desktop app's end-to-end smoke: its one paste step runs only on a Mac.
- The vendored GPUI core's own test selection: this change touches none of the code it covers.
- macOS builds and tests: they need a Mac.

## Review gate

A single fresh-context Opus 5.5 reviewer at high effort, using the plan's general charter and the test-authoring
checklist, reviewed the PR in two rounds.

- Round 1 found no correctness bug, and one medium and four low findings, all fixed:
  - Medium: nothing pinned that the terminal used the payload read. The test clipboard is now order-aware, and a
    terminal test was added.
  - Low: SPEC.md and the release note did not mention that a second paste is dropped.
  - Low: the synchronous read's documentation did not say what it now does on Wayland.
  - Low: a test step did not check that focus actually moved.
  - Low: a test helper's description had become wrong.
- Round 2 found nothing.
