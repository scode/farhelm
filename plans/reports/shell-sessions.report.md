## Since the last review

The landing check found six failures after adding Bash and Zsh sessions. The landing monitor sent the plan back to fix
each failure at its cause, finish the shell wording, and run the full workspace suite. All six now pass in that run. The
three existing drafts remain in order. #4333 contains the test fixes, and #4336 keeps the landing monitor's tab and
Delete wording changes and finishes the remaining review items. The final draft now removes the TODO entry previously
removed in #4334. The website changes in #4334 are the same.

## What this was about

Starting a standalone shell used to require a custom command. The launcher now offers Bash and Zsh in a Shells group.
Type its name and press Enter twice. A shell session has no agent permission mode and no conversation to resume. Replace
starts a fresh shell. The shell must be on the host's PATH. You can run `farhelm spawn` inside it.

## Things you should know

- The wrapper test now checks named starts only for launch types that offer them. It checks that Bash and Zsh refuse a
  named start and that a configured wrapper still surrounds a fresh shell start.
- The Shells row can put some launcher fields below the fold. The tests were clicking the reported coordinates of
  clipped controls and sometimes hit an action in the pinned footer. The two escaped-text tests now scroll to their
  controls before checking what the launcher shows and what a key changes. The two custom-flags tests scroll to the
  field and verify focus before editing. The guarded-launch test scrolls to the prompt and confirms the edit reached the
  draft before releasing the waiting preview. The launcher's existing scrolling and cancellation behavior needed no
  product change.
- Shell sessions show Shell on their main terminal tab. Delete identifies the shell in the confirmation and shows
  “Stopping shell…” on the row, in the header and over the terminal while it waits. The spec now describes this wording
  and the shell activity display. Farhelm shows Running while the screen changes and Idle once it has been still a few
  seconds, even if a quiet command is still running.
- Protocol 81 adds the two launch types. Verify that the helm and the supervisors on its hosts use matching builds
  during release testing.

## Open questions and possible follow-ups

If Farhelm is showing only part of the session list and the open shell is left out, Delete shows “Stopping…” in the
header and over the terminal. That wording describes the operation while the row is unavailable. No decision is needed
for landing.

The tests verify Zsh selection and command composition. A real Zsh process was not started because Zsh is unavailable on
the local test host.

## Pull requests

- [#4333](https://github.com/scode/farhelm/pull/4333/changes) adds Bash and Zsh sessions and corrects the six test
  premises.
- [#4334](https://github.com/scode/farhelm/pull/4334/changes) documents the shell types on the supported-agents, Shell
  sessions, Start a session, Launch templates, Session list, and Stop, restart and resume pages.
- [#4336](https://github.com/scode/farhelm/pull/4336/changes) identifies the shell in the tab and throughout Delete,
  clarifies the spec and release note, and removes the TODO.

## Checks

- Ran `cargo nextest run --workspace --exclude farhelm-desktop` through the recorder, run
  `9c00e455-03d3-4ad7-94bf-e859810bf7ad`, revision `962903a5`. Nextest reported 8,165 passed and 74 skipped by
  selection. Two setup scripts also passed. No runtime skip messages appeared, and the real local Podman lifecycle test
  passed. The original workspace and local narrow failures are retained as `f7071529-2199-4c53-a7ad-dffedde2b750` and
  `6fc8e654-4f59-4478-a0e4-0ba10b54a81a`.
- Before the final workspace run, a focused Helm and GUI selection passed all ten cases in recorder run
  `32b0f623-475c-4106-a0d9-b3fa38cda898`, revision `111f69c8`. It covered the six failing tests plus closely related
  flag editing and Delete cases.
- Linux GUI screenshot comparison passed all 298 reference images in recorder run
  `ca93ffba-dff0-4403-88c3-a93b2a0eae93`.
  `cargo clippy --locked -p farhelm-helm -p farhelm-gui --all-targets -- -D warnings` passed on the same revision.
- Ran Rust formatting, Markdown formatting checks on the spec and fragment, the changelog format and test-sleep checks,
  and the stack's whitespace check. The test-sleep checker found zero missing explanations.
- Rebased the drafts onto `f869aa2a` after checks. Main only gained queue bookkeeping and the desktop PATH plan marker
  in TODO. The product, tests and website at the rebased tip `e544d3b4` are identical to the tested source.
- Reused the initial round's website build. The website content is identical to the previously built revision
  `456803b9`. Skipped another desktop smoke run and separate desktop-package suite. This follow-up changes launcher test
  interactions, session Delete wording and Markdown. The workspace suite and Linux screenshot comparison cover the
  changed GUI crate.

## Review

An independent source reviewer confirmed the launcher tests exercise real scrolling and pointer input and that the final
draft removes the TODO entry. The review prompted corrections to the Delete spec and Rustdoc, a stronger live-terminal
test premise, and neutral progress wording if the session row is missing. The revised #4336 title and description passed
a separate cold read.
