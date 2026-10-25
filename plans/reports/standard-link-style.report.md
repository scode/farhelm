## What this was about

The help link in settings used the browser's default blue against a dark dialog and was hard to read. The maintainer
chose one standard treatment for app links: a readable blue, always underlined, with the same appearance after a visit.
Buttons presented as inline links share it, and links opening pages outside the app end in ↗.

That treatment now covers settings help, the welcome card's guide and feedback actions, and Add host's SSH help action.
A global rule also styles future anchors. The settings help link gains the external-page marker.

## Things you should know

Links have their own color role, separate from selection and status. The underline makes links identifiable without
relying on color alone. Keyboard focus keeps its existing accent ring. Terminal links, menus, the docs website and
frozen release screenshots are unchanged. The matching TODO entry is removed and a fixed changelog fragment is included.

The visited and unvisited states share the same rule. Browser privacy hides visited styles from computed-style reads, so
equality was checked in the stylesheet review rather than claimed from a browser assertion.

## Open questions and possible follow-ups

None identified within this plan's scope.

## The PRs

- [#2710: make app links readable](https://github.com/scode/farhelm/pull/2710/changes), open draft.

## Checks run, reused and skipped

- `npx playwright test 'settings\.spec\.ts' 'welcome\.spec\.ts' 'ssh-help\.spec\.ts' --grep 'prepared checkout settings preserve saved choices across reads, refusals and close|welcome links, modal isolation and short-window dismissal|send feedback now records seen and hands focus to feedback|add host opens ssh help for its draft without losing the form'`,
  through the recorder, selecting the prepared-checkout settings read/refusal/close case, welcome
  links/modal/short-window case, feedback handoff, and Add host's SSH help draft/focus case. Run
  `a75315d3-f0e3-4f09-876e-a1aca65e7dee`: eight passed, four each on Chromium and WebKit, zero failures or skips, zero
  retries, three worker-owned stacks and required pinned tmux. Source capture, runner policy, report and retained output
  are complete. Assertions cover all four rendered link surfaces, resting and hovered style, and the shared keyboard
  ring. The welcome screenshot was also visually checked.
- `node --test` in the UI asset JavaScript harness, through the recorder: 333 passed, zero failures or skips. Run
  `2f62720c-f308-45b3-8294-c25c17507558` has complete source capture and untruncated output. Its contrast and token
  checks cover the new link role on the actual dialog background.
- Native `cargo build`, Dioxus's `dx build --package farhelm-ui --platform web --release`, and
  `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed. The initial web-build attempt selected an unrelated
  `dx` executable on PATH and did not build the app; using the installed Dioxus CLI explicitly succeeded.
- `cargo fmt --all -- --check`, dprint on the changed Markdown, and `python3 releasing/check-changelog.py format`
  passed. Dprint has no CSS, JavaScript or TypeScript plugin and excludes fragments, so it supplies no formatting
  evidence for those files; the separate changelog lint covers the fragment. The isolated interpreter's
  `python -B scripts/check-test-sleeps.py` inspected 315 delays with zero missing rationale.

The checks covered main `2a7b20c7` plus this plan's complete working diff. They and the independent review apply to PR
commit `0127d474e957a043fd0183c4d512072f5b4895b4`: the rebased patch is byte-identical, and every upstream change
through `3b071c8a` was inspected and contains only queue bookkeeping and another plan's report. No older runtime results
were reused; these current checks were retained across that bookkeeping-only rebase without repeating them.

The full Rust/browser batteries, desktop smoke and installer/release checks were skipped because all current visible
link surfaces and their existing actions are covered by the focused browser run on both renderer families, with no
desktop shell or backend change. No instruction file, heading or file location changed, so the agent-instruction checker
was not needed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no findings in correctness, design,
idiomatic code or tests. It received the full charter, acceptance criteria and verbatim test-authoring checklist. The
commit and PR wording passed a separate cold read. A separate documentation pass covered every touched file. There were
no rejected findings, scope departures or weakened guarantees.
