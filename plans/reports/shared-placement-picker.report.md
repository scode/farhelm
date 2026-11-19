## What this was about

The launcher and template editor already shared their controls for where a session runs and whether it opens a folder or
creates a managed checkout. Those controls lived inside the launcher dialog's code. They now have a documented home of
their own so changes to either screen can account for the other.

If the last host that can run containers disappears, both screens keep the selected New container choice visible. The
editor used to guard against an extra press on that choice. In the launcher, that press could clear the message
explaining why a launch was refused. The shared picker now ignores the repeat press when it has no host to offer.

## Things you should know

- The template editor keeps the selected container recipe and host, and the Host selector stays hidden while there are
  no eligible hosts. A launch refusal stays visible when the launcher receives the same repeat press. That visible
  effect warranted a fix and a changelog fragment.
- The launcher still checks whether its hosts are ready and shows its build and placement notes.

## Open questions and possible follow-ups

None for this round.

## Pull requests

- Preserve the refusal message and cover the template editor's retained choice.
  [#4130](https://github.com/scode/farhelm/pull/4130/changes)
- Move the shared destination controls, document their ownership and remove the two completed TODO entries.
  [#4133](https://github.com/scode/farhelm/pull/4133/changes)

## Checks

- All 129 selected Linux tests passed with no runtime skips in recorder run `af92e838-2d5f-4d31-8f3e-e0ed208ac099`. The
  recorded command was
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^launcher::environment_/) | test(/^launcher::checkout_tests::/) | test(/^templates::editor_tests::/)'`.
  It covered the launcher refusal and template editor repeat-press regressions through the shared controls in #4133.
- `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings` passed on Linux after an interrupted first
  attempt.
- `cargo fmt --all -- --check`, `dprint check` on the changed Markdown, `python -B scripts/check-test-sleeps.py` in the
  isolated checker environment, `python3 releasing/check-changelog.py format`, and `git diff --check` passed.
- All 51 tests in the preliminary launcher environment-view and template-editor selection passed with no runtime skips
  in recorder run `b5bbb717-65e0-42f9-8111-0b1eacfd551a`. The recorded command was
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^templates::editor_tests::/) | test(/^launcher::environment_view_tests::/)'`.
  The 129-case run supersedes it with the stronger editor test, the launcher refusal assertion and the controls in their
  new home.
- Skipped the full workspace suite and screenshot capture. The focused tests exercise both affected screens, and the
  controls draw the same rows.

## Review gate

Each PR passed independent code review. The first review identified the launcher refusal side effect and a weak
assertion in the editor test. The patch preserves the refusal and the editor test confirms that the repeat press reaches
the already-selected choice. The second confirmed the control definitions moved intact. The ownership docs now identify
where host readiness comes from, and the parity docs distinguish quick launch's preparation from its fallback dialog and
preserve the existing behavior notes. All findings were addressed. Both commit messages passed independent cold reads.

### Landing

#### What landed with it

While the PRs waited, main gained only review-queue and triage bookkeeping. While they were landing, two more PRs
reached main: one changes how the release notes are put together (#4136), and one is a planning document (#4137).
Neither touches the desktop app, so nothing interacted with this plan.

At your request this landing was reviewed with extra skepticism, both by me and by an independent reviewer. The code is
what the plan asked for and nothing more. The controls the launcher and the template editor share (Runs on, with a new
environment's Recipe and Container host, and the Folder or Managed checkout choice) moved out of the launcher's code
into a place of their own. Apart from one intended edit they are unchanged, which I confirmed by comparing the moved
code with the original mechanically. That edit: pressing New container (or New sandbox) when it is already selected and
there is nowhere left to create one now does nothing, in both the launcher and the template editor. Neither review found
a problem in the code. Both found that the code comments and the developer documents describing these controls had lost
detail or become unclear, which the landing fixed.

#### Fixes made while landing

- **The code comment explaining the Runs on switch had been cut down to a few short sentences.** Before this plan, the
  launcher code holding the shared controls opened with a summary of the switch's rules. It said:
  - when a choice is hidden;
  - what choosing New container or New sandbox picks;
  - that pressing the choice already selected keeps the user's recipe and place;
  - that the template editor keeps hosts that cannot run containers right now selectable;
  - that the shared controls never name a provider themselves, so provider wording stays in one place.

  It also named the SPEC.md sections the code follows. The plan asked for that summary to be split between the old place
  and the new one. Instead most of it was dropped. I restored it: the switch's rules now open the shared code, and the
  launcher's own part opens the launcher code. That part covers the container host checks it asks for as it opens, its
  lookup of the Tensorlake account, and the notes it shows under the controls. I dropped only a sentence about a helm
  route that no longer exists, and the description of the press this plan removed.
- **The new entry in the GUI ownership document (`docs/gpui/foundation-ownership.md`) did not follow the pattern of its
  neighbours.** It did not say who owns the shared controls, who shares them, or which ones, and it described quick
  launch confusingly. It now reads like the entries around it. The launcher owns them. The template editor draws both
  the Runs on rows and the Folder or Managed checkout choice. Quick launch draws neither.
- **Smaller wording fixes.**
  - `docs/gpui/parity/templates-and-quick-launch.md` lists two ways the Runs on switch differs from the previous app's.
    Its wording for them is back to the earlier, more precise version, and it now names the new test of the repeated
    press.
  - A new launcher test's comment gave the wrong reason for its result. Two code comments said the shared controls edit
    a draft, which they do not hold.

#### Things you should know that the report does not say

- **The launch refusal fix covers only the case this plan touched.** As the report says, pressing New container again
  when no container host was left used to clear the launcher's message saying why a launch was refused. In every other
  case, pressing the choice that is already selected still clears that message, even though nothing changes. That means
  pressing Existing host when it is already chosen, or New container while a container host is still available. The
  launcher clears the message on every press of the Runs on switch. Possible follow-up, small and low impact: make any
  press on the choice already selected leave the message alone.
- **#4130 is titled as a bug fix and adds a changelog fragment, though the plan expected neither.** The plan's "no
  release note" decision assumed nothing a user could see would change. The executor found the refusal effect above, and
  the repository's rules call for a fragment for it. The entry is narrow, so release-time curation may well drop it.
- **No screenshots this time.** Nothing drawn changed, and the reference screenshots (goldens) passed on the PRs as
  delivered.

#### Checks

The stack changes only the desktop app's GUI crate and its documentation, so the landing ran the lints, that crate's
whole test suite and the reference screenshots (goldens), all locally. The suite is short enough that the boat sandboxes
would not have saved time.

On the PRs as delivered (main had gained only bookkeeping since their base):

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and
  `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`: passed.
- `cargo nextest run -p farhelm-gui --lib`: 3098 passed, and its 2 ignored tests were skipped (the goldens, which run
  through their own script below, and a helper that writes harness mark pictures)
  (`a58e165e-072e-4043-9baf-66abec89686b`). That covers the launcher and template editor tests, the two new ones
  included.
- `python3 scripts/check-gui-goldens.py`: passed (`ca8c70ce-9a09-4088-83f2-cf7dc6e86c22`).

On the fixed tree, which differs from the delivered one only in comments and Markdown:

- `cargo fmt --all -- --check`, `dprint check` and `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`:
  passed.
- The test suite and goldens runs above were reused, since no code changed.

Skipped:

- The rest of the Rust workspace and the desktop process's tests: the change stays in the GUI crate.
- The app's end-to-end smoke: none of its steps uses the Runs on switch or the destination choice.
- The Mac-only checks: the change touches no Mac-specific code, and nothing drawn changed.
