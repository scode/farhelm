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

### Landing

#### Since the previous landing attempt

The landing two rounds earlier stopped this plan because six existing tests failed, and sent it back with instructions
to fix each at its cause, finish the open review items and run the whole workspace suite. An independent reviewer and I
checked the redelivery against those instructions; all of it is done:

- The helm test now checks a named conversation start only for launch types that offer one, and checks that Bash and Zsh
  refuse it.
- The five launcher tests failed because of how the tests click, not because the launcher changed for users. The new
  Shells row makes the launcher's scrolling body one row longer, and the tests clicked at the middle of a field that was
  scrolled out of view, which in a test lands on the pinned footer (Cancel, in two of them). The body already scrolled
  before shells at the app's usual window size; a user scrolls or tabs to a field, and a pointer cannot click a field
  that is out of view. The tests now scroll a control into view and check it is visible before clicking. The guarded
  launch test still proves that an edit cancels a launch waiting on the guarded-mode questions.
- A shell session's Delete reads "Stopping shell…" in its row, in the open session's header and over its terminal, with
  a test of all three. When the session's row is not in the list at all, all sessions, agent ones included, now read a
  plain "Stopping…" there, not "Stopping agent…"; the report describes this as a shell case only.
- SPEC.md now says a shell reads Running while its screen changes and Idle once the screen has been still for a few
  seconds, so `top` reads Running and a command that prints nothing for a while reads Idle.

#### What landed with it

Main gained desktop-login-shell-path in the previous round: the desktop app now reads your login shell's PATH at startup
for the programs it runs, and hands sessions the PATH it was started with. Shell sessions start through the same path as
agent sessions, so they get that starting PATH too, and their login shell builds PATH once as before. The two plans
touch no common files, and the combined tree was built and tested as listed below. The report's sentence that main had
gained only bookkeeping since its base is out of date for that reason.

#### Fixes made while landing

One wording change, folded into [#4336](https://github.com/scode/farhelm/pull/4336/changes) (the earlier landing's own
fix PR, which the executor extended): the changelog entry's `farhelm spawn` sentence read like a feature, and now says
it works from inside the shell as it does from anything else run in a session, the caveat the earlier answer asked for.
It changes no code.

#### Checks

The plan changes the protocol, the helm's launches and approvals, the supervisor, the app's launcher, session rows,
header and Delete, and the docs, and main had gained desktop-login-shell-path since the executor's last run, so the
landing ran the whole workspace suite, the desktop app's tests, the app's end-to-end smoke and the four lint (clippy)
runs on the maintainer's own test machine (an arm64 Linux VM that builds and tests the tree itself), and the screenshot
comparisons, the docs website build and the small checks locally on x86_64.

On the plan as delivered, stacked on main with desktop-login-shell-path:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine
  (`63eededc-6a12-4ca6-8e43-5f139afe42cb`): all 8169 tests passed, with no runtime skips. The first attempt
  (`93fc9017-ab1a-4548-a467-c7ec04f92ed7`) never ran a test: the compiler building the helm's tests was killed, most
  likely by the test machine running out of memory, as also happened once in the earlier attempt. The rerun on the same
  tree built and passed.
- `cargo nextest run -p farhelm-desktop`, on the test machine (`43484ad3-42df-4a42-a17c-eb2dcca218e1`): all 180 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine (`4a4e8ba9-db9d-4b44-a78f-779792deebc7`): 49 of 50 steps
  passed and the paste step skipped, as it always does on Linux.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's two
  clippy runs (with and without the `agent` feature), on the test machine, not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py`, locally (`0d70c746-e6fc-454f-a71c-ea9382af216d`): passed.
- `cd website && bun install --frozen-lockfile && bun run build`, locally: passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and
  `python3 -B scripts/test-gui-e2e-smoke.py`, locally: passed.

The landing's wording change to the changelog entry changes no code, so the runs above were reused for it;
`dprint check` and the changelog check passed again locally.

Skipped: a run with a real Zsh, which the test machines lack (the tests check Zsh's selection and command line), and a
macOS run, which the release test covers.

#### Things you should know that the report does not say

- With Bash as your login shell, starting a Bash session runs your `.bashrc` twice (once for the login shell, once for
  the session's own Bash), so a `.bashrc` that adds to PATH unconditionally can list a directory twice in that session.
- No screenshot reference covers the launcher's Shells row or the shell marks; the earlier landing's note that the shell
  marks look soft at 1x still applies.
- The helm's refusal of a model choice says "a shell session does not accept a model choice", although the check would
  also apply to any future launch type without models. Harmless today.
