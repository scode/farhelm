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

### Landing

#### What landed with it

The plan's PR, #4589, landed last in this round, after the higher limit on queued window commands and the bulk bar plan,
with a landing fix, #4594. Since the plan's work was written, main had gained the launch that resumes a suspended host,
agent trust in every folder and other changes; none adds a notice or touches the terminal's uploads. The rebase had no
conflict besides TODO.md, and the new screenshot (light and dark) sits beside main's new ones in the list the screenshot
tests compare against.

An independent reviewer found no blocker: everything the plan required is in place. Uploads are reported once, a
reconnect that keeps the session open still shows the interruption in that session's terminal, and the new pictures,
their scene entry and the changelog fragment are present. Two additions the plan did not ask for came out of review, and
both are documented and tested: the stack of banners now scrolls once it reaches half the window's height, so piled-up
upload notices never cover the workspace, and banner buttons stay aligned when they wrap onto a second line.

#### Fixes made while landing

All in [#4594](https://github.com/scode/farhelm/pull/4594/changes), docs only. The implementation spec again says that
the window's banners are separate from the bell on session rows and the notices panel in the navigation rail (the plan's
rewrite had dropped that statement). A checklist mapping the old browser UI's terminal tests to the current app no
longer claims a test covers the old in-terminal message when you leave a session mid-upload; that case now shows a
banner instead.

#### Checks

This round landed three plans stacked in this order on main: the higher limit on queued window commands, the session
list's bulk bar acting on what it drew, and the notices for interrupted uploads. The checks ran on the combined result;
the worker runs were on a separate arm64 Linux test worker, the rest locally on x86_64.

On the three plans as delivered, stacked onto main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four nextest slots, no retries and four build jobs, on
  the test worker (recorder run `d61951d1-e6dd-4697-99c8-2dbbc0081c8f`): all 8316 passed.
- `cargo clippy --all-targets -- -D warnings` and both desktop app clippy runs (with and without `--features agent`), on
  the test worker, not through the recorder: passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `5d66957d-c54c-4bb7-b21d-a9cbefb575cd`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `52dc76ec-1e08-4292-99cb-daa8db90a1c8`):
  passed, the new upload notice scene included.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py` (in its isolated environment), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the interrupted upload notices' landing fix (#4594), which changes only Markdown: `dprint check`, locally: passed.
The runs above still apply.

Skipped: the separate tests for the desktop app's startup, state files and updater, which the workspace run leaves out,
since none of the plans touches those (the window's own tests, the session list and notices included, ran in the
workspace run above); macOS, since no Mac was part of this landing; CI, which runs only on demand.

#### Things you should know that the report does not say

- Nothing beyond the report.
