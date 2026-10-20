# Repository suggestions under Git configuration overrides

## What this was about

Existing local repositories could disappear from the session launcher's suggestions when the supervisor inherited
`GIT_CONFIG`, an environment setting that selects a separate Git configuration file. Git refused the query Farhelm uses
to read each repository's origin. Manual repository selection remained available.

The maintainer approved the production fix and a regression test within a 100-line gate. An earlier change had removed
the same setting only from test-fixture commands.

## Things you should know

Repository inspection now removes the bare setting and prevents child-only configuration inputs from restoring it. The
real-Git regression checks that the unsanitized query refuses the fixture and that discovery still returns its
repository. The existing environment-isolation test also recognizes the bare setting. Tests do not change their own
process environment. Clone credentials and clone behavior are unchanged.

The code, tests and explanatory comments total 39 added/deleted lines, within the agreed gate. The feedback item and its
index entry were removed, and the outcome ledger records the draft PR. No scope expansion or discard was needed.

## Open questions and possible follow-ups

None.

## The PRs

- [#2412 — preserve repository suggestions under git config overrides](https://github.com/scode/farhelm/pull/2412/changes).
  Open draft, head `f96c03a6dad62315e11830caf9c9a03d17cb82cc`, based on main.

## Checks run, reused and skipped

Run for this change:

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`: passed.
- Recorded nextest selection `cargo nextest run -p farhelm-supervisor --lib -E 'test(repository_discovery::)'`: 16
  passed, run `7f84a6f7-9cc1-4303-8f84-1d5191abed9b`.
- The new real-repository regression alone with an externally supplied, owned `GIT_CONFIG` file: one passed, run
  `d08cba8a-d6ba-4b9c-9b58-c62edc938125`. The input was supplied before recorder startup; no test mutated process
  environment. This separately exercised removal of inherited configuration.
- Both runtime runs used Linux, pinned nextest 0.9.148, four global slots, zero retries and matching pinned tmux 3.7c.
  Their source head was `39919e081cd4bcdd4f797420b549cd6b5298d0f1` with the recorded working diff. The module run's
  source fingerprint is `5e8d517a0015bc4076259c9e3dc2cacde7586ade4adee7f21e1dadef53c57e8e`; the inherited-input run's is
  `f17026b6f34e5f8cf138a3be708ba4bfa94a56324591bad38b3afa39e77390d5`. Production code and tests match the final PR;
  later changes only completed and formatted its outcome ledger. Reports and source/lifecycle evidence are complete,
  output is untruncated, and no selected test reported a runtime skip. Nextest excluded 1153 and 1168 unrelated tests
  respectively. These are 17 executions, including two executions of the new regression.
- Test-delay source checker: 304 inspected, zero missing rationales. Changelog-fragment format, Markdown checks of the
  ledger and feedback index, and agent-instruction checker passed. Two ledger wrapping errors were corrected before the
  final Markdown pass.

Reused: the runtime and Clippy results above cover the final head because only ledger text changed after those checks;
no earlier plan's runtime evidence was used. The final main comparison contained only another plan's queue claim, which
does not interact with repository inspection; no rebase or runtime rerun was necessary.

Skipped: broad Rust suites, browser and desktop runtime/build checks, and installer/provisioning checks. The change only
affects the supervisor's local Git inspection environment; the focused module and inherited-input check cover its
concrete risks. No CI run was dispatched.

## Review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found no correctness, design, idiomaticity,
test-authoring or size-gate issues. The commit message passed its separate cold read. A separate documentation pass
checked the changed function, regression and fixture oracle for their contracts and rationale.

### Landing

Landed as #2412, with the same content the report describes, rebased onto this round's main (see the end of these
notes). It was the eleventh of sixteen plans in this landing round, after the ten listed in the earlier plans' notes,
most recently planning-probe-bound.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the ten plans earlier in this round, and documentation and
plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan. It
changes only how the supervisor inspects local repositories for the launcher's suggestions, and no other change in the
round or since its base touches that code, the clone path or the environment-isolation test. Its behavior matches what
SPEC_impl.md says about the Git environment the supervisor passes to repository commands. The only conflicts were in the
review feedback queue's index, where each plan removes its own entries; all removals are kept.

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
