## What this was about

Granting or removing Claude Code's trust for a managed checkout rewrites Claude's settings document. The JSON parser
could change an unrelated decimal during that rewrite: `0.9176884786531495` became `0.9176884786531496`, which Claude
reads as a different number. The maintainer chose to fix this rather than accept unrelated settings changes.

The workspace now enables the JSON library's precise float parser. Trust edits preserve the numeric values Claude's
JavaScript reader sees, and a regression exercises both granting and removing trust through the real file-edit path.

## Things you should know

The parser feature applies across the workspace, but keeps the existing JSON number representation. It adds no
dependency or lockfile change. Integer values within the library's integer range stay exact; other accepted numbers
round to the same double Claude reads. Documents outside the parser's supported numeric range still refuse before
publication. Number spelling and document formatting may change, as already allowed; numeric value is the contract.

Whether a consequential affected decimal occurs in real Claude configuration remains unverified. The regression uses
private fixture files. The accepted race with Claude's unlocked exit save and future vendor-format compatibility limits
remain unchanged.

## Open questions and possible follow-ups

None identified. The plan's precise-parser option worked, so no refusal fallback or broader JSON editor was needed.

## The PRs

- [#2089: preserve Claude settings numbers during trust edits](https://github.com/scode/farhelm/pull/2089/changes) — one
  draft PR with the feature, regression, changelog fragment and completed triage bookkeeping.

## Checks run, reused and skipped

- Ran the supervisor's six trust-file unit tests through the pinned nextest recorder: 6 passed, 1127 selected out, no
  failures or runtime skips; run `5db2a6ed-1b42-4906-88cb-404b75bd6e91`. Selection:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(agent_kind::claude::trust::tests::)'`, four global slots, zero
  retries and pinned tmux. The new test covers the reported decimal, a negative value, signed zero, a subnormal and an
  integer beyond the library's integer range, comparing against independently parsed double values.
- Ran `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`; both passed. Also
  passed final `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, targeted dprint on the
  ledger and feedback index, and isolated-interpreter `python -B scripts/check-test-sleeps.py` (301 delays, zero
  unexplained).
- Reused those results after the final ledger PR-link update, which changes no executable source. Assessed incoming main
  by its file inventory and non-bookkeeping diffs: new plans/reports, feedback-filter bookkeeping, and trash
  confirmation copy. None touches the trust editor, dependency declaration, number handling or relevant specification;
  no rebase or further runtime validation was needed for those changes.
- Skipped the full workspace runtime suite, browser, desktop and installer checks. The feature changes float parsing
  accuracy without changing number representation; both required compile/lint configurations and the focused real-file
  tests cover this change. No new UI, protocol or lifecycle behavior was introduced.

## Review gate's outcome

The required fresh-context gpt-6.1-sol review at high effort, through the external harness, found no actionable issues.
It read the full plan, triage decision, patch, test-authoring checklist and pinned library implementation, confirming
precise parsing without a number-representation change and an independent regression oracle. Commit/PR wording passed a
separate cold read. A separate documentation pass covered the dependency rationale, trust-code comment and test. No
scope departure or unresolved finding remains.
