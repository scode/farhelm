## What this was about

The session summary already opened beside the session list, but hover help for nearby marks still covered the list. The
maintainer chose the same beside-the-list placement for every row target, including the notification bell, with a
pointer notch and a thin ring identifying the hovered target. The summary card also needed to centre on its row.

Every session-row hover now follows that design. The box centres on its target and shifts only to fit the window; the
notch stays level with the target except where its rounded corners require a small inset. Moving between targets,
dismissing help or removing a row clears the previous ring and notch.

## Things you should know

When the window has no room beside the list, help keeps the established above/below fallback without a notch or ring.
The current session uses an inverse ring so it remains visible on the selected row, including in the light theme. The
browser checks exercised both row densities, both themes and the notification bell. Screenshots from Chromium and WebKit
were inspected, including the selected-row filled status dot and the summary card.

## Open questions and possible follow-ups

None required for this plan.

## The PRs

- [#3128: move session-row tooltips beside the list](https://github.com/scode/farhelm/pull/3128/changes).

## Checks run, reused and skipped

Run now on the final behavior:

- The complete `tooltip.spec.ts` and `tooltip-coverage.spec.ts` selection through the recorder: 40 passed, 20 per engine
  on Chromium and WebKit, with zero failures or skips. Run `bc53cbf2-5764-4704-943c-a80eec2cf351`, Playwright 1.64.0,
  three workers, zero retries and repository-built pinned tmux. The report and output are complete; cleanup finished.
- `cargo check -p farhelm-ui`, `cargo check -p farhelm-ui --features desktop` and
  `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed. CLI and web builds passed to supply the browser run.
- `cargo fmt --all -- --check`, changed-Markdown `dprint check`, changelog format and diff whitespace checks passed. The
  isolated pinned-parser delay checker inspected 420 delays with no missing rationale.

Reused within this plan: the full UI JavaScript harness passed 526 tests in run `33525315-b8c4-4cab-93f1-095199034968`;
the focused tooltip geometry/lifecycle selection passed 20 in `fd29c6ce-7fa3-44e5-8cec-0969f5974fec`. Their JavaScript
implementation and assertions remained unchanged after those runs; the later changes corrected CSS and added browser
assertions, covered by the final browser run. The full-harness result covers revision `5b101ca2e8c1`; final reviewed
behavior is `27c6ac61076a`, published as `f271267b54a3`.

Broader Rust runtime suites, desktop execution, installer checks and CI were skipped: the change affects shared UI
render attributes and tooltip styling/lifecycle, covered by compilation, the JS harness and both browser engines. Native
desktop compilation and WebKit browser coverage do not claim a manual macOS app check. Main advanced only by queue claim
changes, whose complete diff was inspected; no interaction required a rebase or another test run.

## The review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found two visual defects: low ring contrast on selected
light-theme rows and status shapes painting over an inset ring. Both were corrected, with selected-row contrast and
status-outline assertions added to the browser test. The same reviewer checked the correction delta and reported no
further findings. A fresh cold reader accepted the final concise PR title; the self-explanatory description is empty.
