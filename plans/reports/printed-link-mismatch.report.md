## What this was about

A program could extend a printed web address onto a neighboring wrapped line while a resting hover kept the old
destination. Even after the terminal redrew, clicking the displayed `https://bank.com` could still open
`https://bank.co`. The maintainer chose best effort for a program racing a click, but no lasting mismatch.

The fix makes printed-link hovers remember the neighboring rows used to find the address. Changing one of those rows
invalidates the old link. The regression reproduced the old destination opening in Chromium and WebKit before the fix
and opens the updated address in both afterwards.

Two related decisions are recorded separately: opening newly written text before its next paint remains accepted under
the existing sub-second redraw allowance, and a hyperlink mismatch warning is judged when the hover starts. The warning
need not follow later redraws of other rows while the pointer rests; the displayed real destination stays authoritative.

## Things you should know

The code and regression are below the agreed 100-line size gate. No process that keeps a terminal, wire protocol,
program-supplied hyperlink warning logic, or paint/click ordering changed. All three feedback files and index entries
are removed, with execution recorded in their existing triage outcomes. The PRs remain drafts for the landing monitor.

The full JavaScript harness has one existing failure: the stylesheet token test rejects a literal `8px` corner radius.
The stylesheet and that test are unchanged from the plan's base, `59ef9722`. This plan leaves that unrelated failure
visible rather than claiming a clean full-suite result.

## Open questions and possible follow-ups

No maintainer decision is needed for this plan. The existing stylesheet token failure remains a separate follow-up.
Native Mac interaction was not exercised; WebKit provides browser-engine coverage, not a native desktop test.

## The PRs

- [#3018: refresh printed links after neighboring edits](https://github.com/scode/farhelm/pull/3018/changes), draft.
- [#3019: retire the before-redraw terminal URL finding](https://github.com/scode/farhelm/pull/3019/changes), draft.
- [#3021: clarify terminal link warning timing](https://github.com/scode/farhelm/pull/3021/changes), draft.

## Checks run, reused and skipped

- Ran the new browser regression before the fix with generic test recording, two workers and zero retries: both engines
  failed by opening the old prefix after the changed neighboring row was painted. Run
  `2584e6da-4cd0-420a-8289-6a6c1cbd9670` is retained as failure evidence. After the fix, the same selection passed
  twice, once per engine, in `5cc882d6-60b9-4860-af15-7f2c11538349`.
- Ran `npx playwright test --config grid-client/playwright.config.ts grid-attach.spec.ts --workers 2 --retries 0`
  through the recorder's generic mode: 74 passed, 37 per engine, with no skips or retries. Run
  `3f447182-8847-46ae-80b2-6e8dbcd21d30` exercises the shipped assets against a fake terminal socket, including plain
  links, program-supplied hyperlink warnings, file links, tunnels, repaint and activation. It makes no real-stack or
  controlled-runner claim.
- Ran `node --test` in the UI JavaScript harness through the recorder: 515 passed, one existing stylesheet-token test
  failed, zero skipped. Run `405da099-af1a-4662-aba1-31b8de56dad2` is retained. The focused
  `node --test grid-buffer.test.js` passed all 35 tests in `2a40c17c-8dc5-4893-834b-b63773d24d25`.
- Ran the isolated-interpreter test-sleep checker: 412 delays inspected, zero missing rationales. Changed Markdown
  formatting, changelog format and whitespace checks passed. The agent-instructions checker passed with zero errors and
  three existing citation warnings outside this work.
- The post-fix browser and JavaScript runs above tested base `59ef9722` plus the recorded working-tree snapshot
  `df8fc91767f81d6272ae62455934e11829697da088d6801609f6dba13045360f`. Their passing browser and buffer results still
  cover the final stack: later edits only explain the hover dependency contract or change specification and review
  bookkeeping. The source snapshots, output EOF and complete untruncated retained output were verified for every
  recorded run. Upstream changes through `2cbfdc77` affect unrelated Rust test fixtures, flake tracking and plan state;
  their full diff was inspected and does not change this coverage.
- Skipped Rust builds/tests, live-stack browser tests, the rest of the static grid suite, CI and native desktop tests:
  no Rust, build wiring, transport, rendering or fit behavior changed. The buffer unit tests and attachment spec cover
  the affected URL search and hover/click path. The two documentation-only PRs required no runtime tests.

## Review gate's outcome

Each PR passed the required single fresh-context gpt-6.1-sol review at high reasoning effort. The first review found one
stale explanation that limited hover dependency rows to program-supplied hyperlinks; it was corrected and the reviewer
confirmed it resolved. No functional findings remain. Independent commit-message cold reads completed, and the delivery
report was also cold-read before delivery.
