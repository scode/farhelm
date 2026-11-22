## What this was about

Switching sessions closes the previous session's terminal and interrupts its uploads. The message used to wait for you
to reopen that session. If you moved on, or the session was deleted or replaced, the file could seem to disappear.

## Things you should know

When you switch away, or the app moves on after a Delete or Replace, Farhelm now shows a window banner for any uploads
it interrupted. The banner names the interrupted files and their original session. Files still waiting were never sent.
For a request that was sent, Farhelm cannot confirm whether the file arrived. Files with the same outcome share a
banner, with up to three names and a count for the rest. The session title is omitted if it was unavailable when the
upload was queued.

The banner survives a lost connection to the helm and stays until dismissed. Returning to the session does not show the
interruption again. If the same session stays open while the connection resets, its next visible terminal shows the
interruption in its own strip. Attachment failures that were already shown still follow that session. New banners scroll
into view without taking keyboard focus, and older banners remain reachable by scrolling.

## Open questions and possible follow-ups

A title or file name long enough to make a banner taller than the notice area can put the start of the message above the
visible area. The outcome and Dismiss are shown at the end of the newest notice. The full text and earlier notices
remain reachable by scrolling. This is the behavior chosen for this change.

## Pull request

- [#4589](https://github.com/scode/farhelm/pull/4589/changes) reports interrupted uploads after leaving their session.

## Checks

- `cargo nextest run -p farhelm-gui --lib -E …` ran 110 focused GUI tests. They covered uploads, selection and deletion
  under a filter, tab restoration and attach limits, reconnect, Dismiss, and scrolling a full notice area. All passed
  with no runtime skips. Recorder run `dd5596ea-0bb1-4483-aef7-66bedbdf2c21` retains the exact selector.
- `python3 -B scripts/check-gui-goldens.py --update --jobs 4` changed only the two new Linux reference pictures, both
  byte-identical to the Opus-approved pair. Published those two pictures with `python3 scripts/gui-goldens.py publish`.
  Recorder run `61a3d18e-6d2e-4294-8125-fc0ebbb8161d`.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings` passed. Scoped rustfmt and dprint, changelog format,
  whitespace, and the test-sleep checker passed. `python3 scripts/check-agents-md.py` passed with five existing citation
  warnings.
- After the launcher host changes landed, a rebased `cargo nextest run -p farhelm-gui --lib -E …` passed all 12 selected
  tests with no runtime skips. This checked filtered Delete, upload notice scrolling and reconnect, and launcher host
  wake behavior through the shared controls. Recorder run `8d8efca0-edc9-4fd2-b6d9-8e5323fccd11`.
- `python3 -B scripts/check-gui-goldens.py --scene interrupted-upload-notices --scene notice-banner --scene terminal-status`
  matched all six pictures on the rebased head. Recorder run `7ffb6e77-10fc-4042-b596-aa9f7e0b7b89`. The rebased scene
  registry and batch-harness suite also passed 47 tests with `python3 -B scripts/test-gui-screenshot-batch.py`. Recorder
  run `34be4e98-29a5-49fd-ba3c-b5d6df9d34d9`.
- Reused the original focused GUI run, scoped Clippy and the full golden update above from `2027d78c` for the unchanged
  upload/notice patch. The focused rebased checks cover the shared prompt, menus and golden harness.
- Skipped the workspace-wide suite because the change is local to the desktop's upload handoff and notice rendering.
  Skipped docs screenshots because their scenarios do not draw these banners or use affected park or crop targets.

## Review

The source gate passed after Sol high's findings on retaining an unseen notice, filtered Delete, and keeping the notice
stack reachable were addressed. The final source review raised the long-banner edge described above. Opus 5.5 high
approved both appearances after the wrapped Dismiss alignment was corrected. The full golden update listed above
verified the approved pictures and confirmed the existing banners matched their references.
