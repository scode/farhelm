## What this was about

Farhelm could swallow an Enter pressed right after a dialog closed. It could also deliver a fresh Enter to the terminal
after a quick-launch sequence and still switch sessions when the launch finished, because it counted the two presses as
one. Bulk actions and clearing marks could send keyboard focus to the session list's sort control when the focused
toolbar disappeared.

## Things you should know

If you press a fresh key or click while a launch is pending, its result leaves the selection where you put it. On
success, Farhelm lists the new session and shows a Started notice. Fresh input goes to the focused control after a
dialog or menu closes. A held Enter can also reach the terminal. Only fresh presses advance the input count.

After Escape closes the last dialog or menu, another tap within half a second is suppressed so it cannot interrupt the
agent. A different key or a click ends that guard. With two dialogs open, separate Escape presses can still dismiss them
in turn.

When bulk Stop or Delete, Clear or Escape removes the toolbar that held focus, the keyboard returns to the open
session's terminal. A stopped session's card or the New button takes focus when it is the window's resting place.

The confirmation change is a spec clarification. It names permanent trash deletion and ordinary confirmations for
session lifecycle, closing a tab, and host setup, removal and uninstall as starting on their enabled confirming button.
Warnings about unexpected risk start on Cancel, as when a host asks for confirmation before a YOLO launch.

The landing monitor approved about 200 changed lines so the dialog input rule, spec and regression tests could land
together. The input rule changed in two lines. Its complete diff is 229 changed lines, including 143 deletions.

## Open questions and possible follow-ups

The product decisions are settled. Native macOS keyboard behavior and the large clipboard paste remain outside the Linux
runtime coverage.

## Pull requests

Delivered as four open drafts for the landing monitor.

- Count a fresh key press or click as new input during a pending launch.
  [#4206](https://github.com/scode/farhelm/pull/4206/changes)
- Deliver Enter after a dialog closes and keep the short guard for Escape.
  [#4231](https://github.com/scode/farhelm/pull/4231/changes)
- Clarify the specification for confirmation focus, including permanent trash deletion and unexpected-risk warnings.
  [#4216](https://github.com/scode/farhelm/pull/4216/changes)
- Return focus home when Stop, Delete, Clear or Escape removes the focused toolbar for marked sessions.
  [#4230](https://github.com/scode/farhelm/pull/4230/changes)

## Checks run, reused and skipped

### Run

- On stack tip `76ff98d6` over `c9164885`, recorded `cargo nextest run -p farhelm-gui --lib` passed 161 selected tests.
  The selection covered focus, launcher results, bulk operations, marking, Shift-drag, Enter after a quick-launch
  sequence and the model-family Enter cases. The Escape tests first show an active guard and then release it with a
  different key or a click. The launcher test starts from a focused terminal before the second Enter. Run
  `c02ad97e-2605-42a3-ae4d-05a68d3fecb3`.
- After rebuilding the debug desktop app with the automation bridge, `farhelm` and `farhelm-fixtures`, recorded
  `python3 -B scripts/gui-e2e-smoke.py --no-build` passed 49 steps on Linux and skipped one. The harness stopped the
  test app and its terminal hosts. Run `2b78326b-70f0-4f7e-a9d6-6d844a378b44`.
- `cargo clippy -p farhelm-gui --tests -- -D warnings`, `cargo fmt --all -- --check`,
  `dprint check SPEC.md SPEC_impl.md`, `git diff --check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` and `python -B scripts/check-test-sleeps.py` passed. The agent-instructions check
  reported four existing reference warnings.

- The final rebase brought in Shift-drag, model-family selection, and folder and terminal refusal fixes. The draft
  product patches kept their earlier behavior. The focused suite reran the combined Escape and Enter paths on the
  current base, and the real app smoke covered terminal attach and keyboard focus.

### Reused

- Reused the GPUI automation command-line binary from the ownership plan's Linux smoke. The vendored command-line
  source, lockfile and Rust toolchain pin are identical between `099c6958`, that plan's starting base, and this stack.
  The desktop app and fixture programs in this run were freshly built.

### Skipped

- Headless Mutter does not give the app clipboard ownership. Its large clipboard paste step was the one Linux skip.

## Review gate

One independent GPT-6.2 Sol review at high effort covered each draft. The fence review found that the two tests for
releasing the Escape guard had to start with Escape to test the active guard. The final tests do that and verify the
standard launcher returns to terminal focus before the follow-up Enter. The bulk review found an after-frame case where
a vanished control could keep focus. The fix checks that the same control still holds focus before recovering. The
marked-list tests exercise the normal recovery paths. All four drafts cleared the review gate after these fixes.

### Landing

#### What landed with it

It landed in a round of two plans, gui-ux-enter-and-focus and settings-app-version. Since its base, main gained the
previous round's plans, among them drawn-frame-actions (session actions and the launcher act only on what the user was
shown) and one-helm-per-supervisor, and a repair of the triage archive, which an earlier landing had garbled. The Enter
change fits drawn-frame-actions' rule: without the half-second window in which a second Enter was swallowed, a quick
second Enter reaches whatever has focus, and if that is a control that acts only on what was shown, the control itself
still refuses a press before its frame is shown. The reviewer found no other interaction.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The rule change itself is
small, as the block's answer expected, and the regression tests check that the terminal had focus before the second
Enter and would fail on the old code. The Enter PR came to 229 changed lines against the allowance of about 200 given
when it blocked; I accepted it, since most of it replaces old Enter expectations in tests.

#### Fixes made while landing

- **A held Enter typed into the terminal.** The plan exempted Enter from the rule that a held key's auto-repeat never
  crosses into the control focus returns to. So the auto-repeat of the Enter that answered a question or pressed Launch
  reached the terminal, and if the agent's prompt held a draft, that repeat submitted it. The outcome asked only for a
  quick second press of Enter to get through, and auto-repeat is not a new press. A held Enter's repeat is now swallowed
  again; a fresh Enter still goes straight through. The test and SPEC_impl.md say so.
- **A comment was left false.** A comment on closing a session header's caret menu with Tab said that any other way of
  closing it would swallow a second Tab pressed within half a second. Since this plan, only Escape has that guard. The
  comment now says so, and why closing with Tab is still used.

#### Things you should know that the report does not say

- **A new recovery step is untested.** When a control that has keyboard focus disappears from the screen without the
  framework noticing, focus now moves on to where the app sends it after a closed bar or dialog. The case was found by
  reading the UI framework's code rather than reproduced, it came from the executor's own review, and no test fails
  without it. Its comment also contradicts the paragraph above it ("never takes focus from a control that holds it").
  Possible follow-up: a test, or removing it.
- **Choosing Pin, Mute or Mark read from the menu for marked sessions still moves focus to the sort-order heading above
  the session list,** while Clear, Escape, and bulk Stop or Delete now move it back to the open session. Users now get
  two landing spots. This was outside the plan's outcome.

#### Checks

The two plans change the desktop app's GUI crate, the specs, two docs pages and the triage archive. The landing ran the
lints, the GUI crate's whole suite, the app's end-to-end smoke, the reference screenshots (goldens) and the website
build. Builds ran locally; the GUI suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and
  `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings` once without and once with `--features agent`:
  passed.
- `cargo nextest run -p farhelm-gui`, from a locally built archive, split in three across boats: 1097, 1036 and 1030
  passed, 3163 in all, with no runtime skips (`2b47e057-bb12-44ed-9b4e-0eb107bd7f93`,
  `33b0a8db-29ec-4348-81d9-7fe3eb4f3c59`, `da09c3c6-1297-47b9-839b-2960e7aaf280`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed and the clipboard
  paste step was skipped, as always on Linux (`0d719710-e8fc-4f7d-a99e-4a98147bf820`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`f2fafc15-3eab-4786-8d26-dee83ba73b58`).
- The docs website build (`bun install --frozen-lockfile && bun run build` in `website/`): passed.

On the fixed tree (a held Enter's repeat, the Settings help line and its test, comments and SPEC wording), rebased onto
main with the archive repair:

- `cargo fmt --all -- --check`, `dprint check` and `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`:
  passed.
- The focus, Settings, session pane, marked-rows, bulk action, launcher launch and quick launch tests, locally: 607
  passed (`5edad3a8-c790-4866-86f9-51d2b58187ca`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`0b4c779b-c1c6-4b49-af6e-c450b6a5bc5e`).

The boat suites, the smoke and the website build were reused for the fixed tree: after them only the changes above,
which the focused runs cover, and the archive repair, which only restores text. The sleep check and the clippy run with
`--features agent` were reused too: the changed tests only changed assertions and added no delays, and no feature-gated
code changed.

Skipped:

- The rest of the Rust workspace: the plans stay in the GUI crate.
- The Mac-only checks: no Mac-specific code changed.
