# Launcher drafts and literal checkout folders

## What this was about

Four launcher and Settings actions could silently change unfinished input or a saved folder name. Selecting the current
agent again erased custom permission flags with an unfinished quote. Accepting a custom model could reset a supported
reasoning effort because those flags were empty or malformed. An unrelated configuration refresh could erase the
launcher's checkout-folder draft and keyboard focus. Saving an untouched checkout folder in Settings could strip a
significant trailing space from a name set through the command line.

All four approved fixes are implemented in separate draft PRs, in the requested order. The first two use the launcher's
shared New, Clone and Replace with paths. The checkout editor stays mounted during a same-host configuration recheck,
and Settings preserves an untouched stored folder name.

## Things you should know

All size gates passed: 34, 42 and 78 changed code/test/documentation lines for the first three outcomes, and 11 non-test
lines for the fourth, whose test adds 60 lines. The gates exclude feedback cleanup, execution bookkeeping and changelog
fragments. No outcome was discarded or blocked.

Changing agents still clears custom flags belonging to the previous agent. A model can still clear an effort it does not
support. The checkout editor disappears once a current answer establishes that setup is unnecessary, or when its host
connection or installation changes. Saving edited folder text still trims surrounding whitespace; saving an untouched
stored name preserves its spelling. The existing editor owns its draft; no general draft store or helm normalization
change was needed.

Each PR removes its feedback item, updates the execution ledger with its change, bookmark and PR, and includes a fixed
changelog fragment. All four remain drafts. Main advanced from the plan base `7355efcf` to `7d906537` only through plan
and TODO bookkeeping; no product code, specification or governing instruction changed, so no rebase was needed.

## Open questions and possible follow-ups

None required for these outcomes.

## PRs

- [#2389](https://github.com/scode/farhelm/pull/2389/changes): preserve custom flags when reselecting the current agent;
  head `fe77a2b7cdd149e8e88cb37fdd7f0f1d79f7cbdd`.
- [#2391](https://github.com/scode/farhelm/pull/2391/changes): preserve compatible effort while permission flags are
  incomplete; head `8b0b3fe9908686465b6f76379f46ea021276d015`.
- [#2394](https://github.com/scode/farhelm/pull/2394/changes): preserve the checkout setup draft and focus through
  refresh; head `114c5aa0d76133d3bf4e9795ede50987d12acbbb`.
- [#2397](https://github.com/scode/farhelm/pull/2397/changes): preserve untouched literal checkout folder names; head
  `4e44f63ddd70f0bb002d05aea33189e86d4dc5c8`.

## Checks run, reused and skipped

Runtime commands below ran through `scripts/record-test-run.py`, with pinned tmux 3.7c. Rust used nextest 0.9.148, four
slots and zero retries; browser runs used one worker, zero retries, Chromium and WebKit. Successful browser runs had no
skips. Rust skip counts were filter exclusions; no selected test reported a missing runtime substrate.

- `cargo nextest run -p farhelm-ui --lib -E 'test(harness_transition_) | test(permission_)'`: 11 passed, run
  `f12b3509-7715-4b69-82c6-5825dacabc1e`.
- `cargo nextest run -p farhelm-ui --lib -E 'test(effort_options_follow_) | test(a_missing_catalog_) | test(the_effort_row_)'`:
  3 passed, run `3021bf21-c2ad-4122-a179-3be3dc2b8c50`.
- `cargo nextest run -p farhelm-ui --lib -E 'test(github_checkout::tests::)'`: 5 passed, run
  `126a69b3-2b8f-44b3-9a6e-a40cdc69703d`.
- `npx playwright test 'custom-permissions\.spec\.ts'`: 14 passed, seven per engine, run
  `441e33c4-d889-4e6d-a128-b572e651894e`.
- `npx playwright test 'custom-permissions\.spec\.ts' -g 'custom model acceptance'`: 2 passed, one per engine, run
  `416505f9-5f2c-4526-ab06-d3b25b35ff7d`.
- `npx playwright test 'github-checkout-composer\.spec\.ts' -g 'inline checkout folder setup'`: 2 passed, one per
  engine, run `c504ee71-5ab1-4ee2-a783-26982b1190fd`.
- `npx playwright test 'settings\.spec\.ts|github-checkout-composer\.spec\.ts' -g 'settings preserves untouched literal|settings saves and clears the all-hosts checkout folder|inline checkout folder setup'`:
  6 passed, three per engine, run `a174090f-8db2-44b3-9fc3-41d8ae2729eb`. This final-stack run covers literal and edited
  folder saves, refusals, clearing, retained focus during refresh, and effective host overrides.

The first effort browser attempt, run `b2e7797e-ea2d-48d5-9f95-b675d6706a5a`, failed both cases before reaching the
behavior: the test's `high` suffix selector also matched `xhigh`. The selector now requires an exact match. That failed
run and its traces/screenshots remain retained; the later two-case pass is a separate observation.

`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the changed Markdown's `dprint check`,
`python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and
`python -B scripts/check-test-sleeps.py` passed. The sleep checker used its pinned isolated parser environment and found
304 delays, zero without a rationale. The stack's whitespace check passed. `cargo build` passed, and each outcome used a
fresh successful `dx build --package farhelm-ui --platform web --release` before its browser check.

Earlier unit and browser evidence remains applicable to the final heads: later outcomes leave the flags, effort and
checkout suggestion checks they covered unchanged. The final run checks the interaction between retaining the checkout
editor and tracking whether its text was edited. URL-only amendments change no tested behavior. The backend build was
reused because this stack changes no backend code or dependencies.

The full workspace and browser suites, native desktop shell, installer and release tests were skipped: the selected
regressions directly cover these shared UI changes, and no backend, desktop-shell, installation or release behavior
changed. CI was not dispatched; it does not run automatically for these PRs.

## Review gate

Each PR passed one fresh-context gpt-6.1-sol review at high effort with the required general charter and verbatim test
authoring checklist. No findings remain. The second PR's reviewer also checked the exact-selector correction after the
retained failure. Commit messages and PR text passed their separate cold reads. No scope expansion review was needed:
all four fixes stayed within the plan's outline and size gates.
