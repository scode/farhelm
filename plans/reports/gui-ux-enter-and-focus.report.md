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
