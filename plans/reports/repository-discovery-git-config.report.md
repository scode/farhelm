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
