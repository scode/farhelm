# Read Trash when cleanup changes

## What this was about

Every session status update made each open window re-read every connected host's Trash. Those unnecessary reads used the
same limited browsing capacity as the launcher's folder picker. The maintainer chose to follow each host's cleanup
changes instead.

## Things you should know

The helm now reports a per-host Trash change counter through its existing host listing. Accepted cleanup or Retry and
changed cleanup notices advance it. Session status updates and fallback polling leave idle Trash readers alone.
Reconnects, opening Trash, archived session deletions and active-removal polling keep their existing reads. The bell's
notice refresh still follows the fleet feed.

As agreed, a change made through a different helm is noticed only on reconnect, app load or opening Trash. One helm per
host is the supported model. No supervisor protocol, notice resolution rule, endpoint or push channel changed. The
implementation is 290 added-plus-deleted lines excluding the TODO removal and changelog fragment, within the size gate.

## Open questions and possible follow-ups

None identified within the agreed scope. Browser tests model another window's cleanup through changed host observations;
they do not open a second helm. Separate Rust tests verify the actual cleanup and notice handlers publish those changes.

## The PRs

[3315: avoid Trash reads on session status updates](https://github.com/scode/farhelm/pull/3315/changes), draft,
`plan/bell-trash-reads/01-epoch-trigger`, head `2c69afbeeba2f53b2fa7f76efcd67a576bb16a2d`.

## Checks run, reused and skipped

- Rust: `cargo nextest run -p farhelm-helm -p farhelm-ui --lib` with the notices, Trash policy and host wire-contract
  filter passed 25 tests. Run `3f9da63c-8195-4a98-9a8c-d3c0e495d26a`, nextest 0.9.148, four slots, zero retries, pinned
  tmux 3.7c. The 2,605 unexecuted cases comprise 2,603 expression-filter exclusions and two ignored-test exclusions. No
  executed case returned a runtime `SKIPPED`. Source was `5bd6d3ad` plus this change; report, output and cleanup were
  complete, with no retained-output loss or forced cleanup.
- Browser: `npx playwright test 'rail-notices.spec.ts|trash-dialog.spec.ts'` passed all 46 cases, 23 on Chromium and 23
  on WebKit, with zero retries or skips. Run `4fa6a3e3-0781-438d-992d-98c5ef71f54d`; three owned stacks used terminal
  hosts and finished cleanup. Coverage includes zero Trash requests after a session status change, host isolation,
  coalescing, notices, Retry and the existing Trash dialog behavior. Source was `182a133f` plus this change, before a
  test-comment wrap that changes no executable behavior. Reports and retained output were complete, without loss or
  forced cleanup.
- `cargo clippy -p farhelm-helm -p farhelm-ui --all-targets -- -D warnings`, CLI and web builds,
  `cargo fmt --all -- --check`, targeted `dprint check`, `python3 releasing/check-changelog.py format` and the
  test-delay checker passed. The latter inspected 429 delays with no missing rationale. The close-pruner's classifier
  also passed its one test, run `a4701d48-b4fc-4db2-a8d9-bdba76ca7d14`; this is helper validation, not product coverage.
- The first Rust attempt, `7524f5af-d142-4281-9183-4f1d91f45575`, timed out during compilation before any tests ran. A
  queued attempt, `d83de020-607b-4a6d-bb0b-8fc97b092d0b`, was interrupted before compilation to avoid the same limit.
  Both are retained and excluded from passing evidence. The successful retry used a longer build allowance. Initial
  compile errors from fixture fields were corrected before the successful checks.
- Reused the completed checks after careful rebases onto `636f5105`. The intervening changes add an unrelated TODO
  entry, adjust Tensorlake SSH identity options and host-progress hover behavior, and record plan landings. Their full
  diffs were inspected: no Trash reader, notice handler, cleanup dispatch or tested request-count mechanism changed. The
  only conflict combined adjacent TODO removals; both plans' completed removals were kept. Formatting passed again.
- Skipped the rest of the workspace and browser suites because the targeted checks cover the changed behavior and the
  retained cleanup paths. No cloud calls, live-install actions or hosted CI were needed or performed.

## Review gate's outcome

The single required fresh-context gpt-6.1-sol reviewer at high effort found no issues, applying the full test-authoring
charter. A separate documentation pass covered every touched file. The commit-message cold read passed after its first
reader identified an ambiguous title; the rewritten title and body conveyed the intended behavior and limitation.
