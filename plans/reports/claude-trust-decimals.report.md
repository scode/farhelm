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

### Landing

Landed as #2089, the first of the round's four plans; checkout-recovery-host-binding (#2086),
template-editor-unsafe-values (#2087) and codex-guide-hook-warning (#2091) landed after it, and all four were tested
stacked on top of main. Their only overlap was the review feedback index, where each removed its own entry; all removals
were kept.

#### Review

An independent fresh-context review found nothing blocking. The change turns on serde_json's `float_roundtrip` feature
for the whole workspace, including the browser and desktop builds. That feature only makes float parsing correctly
rounded; it does not change how numbers are represented, so the known `arbitrary_precision` breakages in untagged,
flattened and internally tagged types do not apply, and the lockfile is unchanged. Protocol frames are JSON; the only
other format is TOML. The new test compares against Rust's own float parser and checks negative zero through its bits.

#### Fixes made while landing

- The Claude trust feature this follows (#2070) is not released yet. The repository rule is that a follow-up to
  unreleased work edits that work's changelog fragment rather than adding a second entry, so this PR's separate "fixed"
  fragment was removed and the feature's fragment now says Farhelm changes only the folder's trust entry and leaves
  every other value as Claude wrote it.
- The test's values all took the parser's fast path; two values with more than 19 significant digits or at the largest
  finite double now exercise the long path, where the default parser was least accurate.

#### Left as is

- A number outside the double range (such as `1e400`) makes the edit refuse with "Claude config is not JSON", which is
  misleading; JavaScript never writes such a value.
- The plan read "preserve every unrelated number exactly" as "the same double Claude reads", so `-0` is written as
  `-0.0` and integers beyond 64 bits in float spelling.
- Suspected, not measured: the precise parser adds some size to the browser build.

#### Checks

- Run on the four plans stacked on main: format, lint, changelog and sleep checks;
  `cargo clippy --all-targets -- -D warnings` and the other clippy configurations; `cargo check -p farhelm-desktop`; the
  protocol, helm and supervisor packages in full through the recorder with pinned tmux, after rebuilding the CLI, run
  `69e07000-0f82-4cef-9e98-4ba2fdd30993`: 2386 passed; the UI crate with the desktop feature, run
  `00253155-266a-4b7c-82d2-a9289e13eb9b`: 651 passed; the JS harness and the website build; the templates, quick-launch
  and tooltip-coverage specs on Chromium and WebKit, run `d1693e60-8499-40e2-b168-278df93a2f5d`: 128 passed.
