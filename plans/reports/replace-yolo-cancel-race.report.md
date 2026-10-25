# Replace YOLO Cancel race

## What this was about

Clicking Cancel immediately after “don't ask again” on a session's Replace YOLO question could hide the question while
the Replace continued. If saving the host setting failed, nothing was replaced but the reason had nowhere visible to
appear. The agreed fix was to refuse Cancel while that Replace runs: the host setting write cannot be undone.

## Things you should know

Cancel now does nothing during the save and Replace, including clicks dispatched before the buttons appear disabled. A
failed save leaves the question open with its reason and sends no confirmed Replace. Once the request settles, Cancel
works normally. The sidebar's question keeps its existing behavior.

The regression test reaches the question through Replace at the top of the session, avoiding the interrupted card's
known click flake. It verifies that Cancel was still enabled and connected when clicked in the same burst as the first
answer. The fix remains a guard on this session's running state; unrelated operations do not disable an idle question's
Cancel. The matching TODO entry is removed and a changelog fragment is included.

## Open questions and possible follow-ups

None identified. The agreed scope and behavior are complete.

## The PRs

- [#2707: keep replace yolo save errors visible](https://github.com/scode/farhelm/pull/2707/changes), draft.

## Checks run, reused and skipped

Run on commit `b2b31b184ee5d5db3102b18ae18f968befe3c264` or identical source content before its commit description was
set:

- Recorded Playwright selection across `terminal-restart.spec.ts` and `yolo-guard.spec.ts`: the new Cancel burst and
  neighbouring session/sidebar Replace YOLO answers, hostless questions and captured preconditions. Run
  `142a417e-1c7a-413d-848f-02dde06560ae`: 18 passed, nine per engine on Chromium and WebKit, zero failed or skipped,
  zero retries, three worker-owned stacks and required pinned tmux. The report and source capture are complete.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` and the wasm32 web-feature UI Clippy run passed, checking
  the shared handler under both compile configurations.
- Native `cargo build` and Dioxus's release web build passed, preparing the actual application driven by the browser
  run.
- `cargo fmt --all -- --check`, dprint on changed Markdown, changelog format, and the isolated test-delay checker
  passed; the latter inspected 315 delays with zero missing rationale. Dprint excludes browser TypeScript, so it
  supplies no TypeScript formatting evidence.

Reused: no earlier runtime results. Skipped: the full Rust/browser batteries and native desktop smoke, because this
surface-local handler change is covered by the focused browser integration run on both renderer families, and both UI
compile configurations passed. No desktop shell behavior changed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues in correctness, design, idiomatic
code or test quality. It received the plan's acceptance criteria and the full test-authoring checklist. The commit/PR
wording passed its separate cold read. A separate documentation pass covered every touched file.
