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

### Landing

Landed as #2389, #2391, #2394 and #2397, merged bottom-up, unchanged from what the report describes. It was the second
of sixteen plans in this landing round, after launcher-retry-identity.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans (among them host-entry-session-ids, which changed how
sessions are identified), launcher-retry-identity from this round, and documentation and plan bookkeeping. A
fresh-context reviewer and I read them and the other plans of this round against this plan. launcher-retry-identity also
changes the launcher form: it decides when retrying a launch reuses the earlier attempt and when a changed launch starts
a new one. This plan changes what the agent, effort and checkout-folder fields keep, not that decision. The new browser
checks here identify hosts the same way as before host-entry-session-ids, which changed only how sessions are
identified, and the checkout-folder draft is kept per host the same way as before. No other plan in the round touches
agent reselection, effort, the checkout editor or the Settings folder field. The only conflicts were in the review
feedback queue's index, where each plan removes its own entries; all removals are kept.

#### Fixes made during the landing

None.

#### Checks

Every plan in this round was executed on a main from before round 107's session-identity change, so none had been tested
against it. That is why this round ran the whole Rust workspace and the full browser suite rather than a selection. All
of the following ran now, on all sixteen plans of this round stacked together on main, including the fixes made while
landing plans in this round:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and its `--self-test`,
  `python3 releasing/notes-review-page.py --self-test`, `scripts/check-desktop-assets.sh`, and the website build
  (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `40f2b98b-b160-4239-a653-44cd0bcdcf96`: 3948 passed.
  `cargo nextest run -p farhelm-ui --features desktop`, run `d288b68b-249f-4f21-9fe0-d4d6d1c4a738`: 717 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `f82f83f8-67c9-4e07-99ed-467c9830dc9f`: 317 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, run
  `81d76a96-912f-47ed-8e1f-57423edbeb32` (kept): 2183 passed and 14 skipped. Of the skips, 9 are the WebKit clipboard
  cases, because Playwright cannot grant WebKit clipboard permissions; 4 run only against a real, signed-in agent; and 1
  is a pixel capture that runs only on Chromium. One test failed: on WebKit, "model-only number key opens New with a
  temporary composition explanation" in `templates.spec.ts` timed out at its first step, waiting 60 seconds for the
  header's Templates button to settle before clicking it. No plan in this round changes that button or that test, and
  the file's other WebKit tests that open the same dialog passed.
  `npx playwright test --project=webkit-templates -g 'model-only number key opens New with a temporary composition explanation' --repeat-each=20`
  then passed all 20 (run `6c382916-77a1-4ecb-b4ee-a1dae8cac848`). The cause is probably machine load during the full
  run, not something this round broke, but that is not confirmed; on one observation it is not recorded as a flake.
- Skipped: the native Mac app (not runnable here) and the installer (unchanged).

The report was written on a main from before round 107's session-identity change, so its statement that no rebase was
needed and its focused checks no longer describe what landed; the combined runs above supersede them. Nothing else in
the executor's report became untrue at landing.
