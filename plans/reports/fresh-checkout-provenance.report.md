## What this was about

The plan covered two independent fresh-checkout problems. A test stopped reaching the checkout-settings refusal it was
meant to check after helm restarts began using fresh connection numbers. Separately, Replace with could briefly make a
host’s session refresh fail while deleting a session that had created a GitHub checkout. The Hosts panel could show “the
last session refresh failed” until the next refresh.

The maintainer chose to correct the test without changing the helm’s refusal order, and to fix the refresh race with
deterministic evidence.

## Things you should know

Accepted launch retries still use their original request bytes. Only the test’s never-accepted request is rebound to the
restarted helm’s current connection, while keeping its old checkout settings. It now reaches and verifies the intended
settings refusal.

The refresh error was reproduced by reading the session, deleting it through the real store operation while a borrower
kept the checkout registry alive, and then reading its origin record. That last read produced the reported provenance
mismatch. Observation now reads the session and its checkout origin under one store lock, so it cannot combine a
pre-deletion session with post-deletion bookkeeping. A deleted session has nothing left to classify.

Launch and recovery operations keep their existing strict checks. Actual provenance damage on a current accepted session
still fails observation; borrowers, pending terminals and old launch generations retain their existing exclusions.
Delete’s checkout ownership and archive rules are unchanged.

## Open questions and possible follow-ups

None. Both fixes stayed within the agreed scope.

## The PRs

- [#1522](https://github.com/scode/farhelm/pull/1522/changes): restore the stale-settings test after a helm restart.
- [#1523](https://github.com/scode/farhelm/pull/1523/changes): keep session refresh working during checkout deletion.
  Both PRs remain drafts.

## Checks run, reused and skipped

- Reproduced the test failure before editing it, recorder `af31f415-2e05-4fa6-b350-fa9f5380b50f`: the host-connection
  refusal arrived before the settings refusal. The repaired test passed, recorder
  `1c040847-ae19-41c3-924a-582b3eb1775e`, then passed two further recorded repetitions in batch
  `3d3892db-9098-4af1-af0a-45b4275096f8`.
- Reproduced the provenance error with the ordered Delete fixture, recorder `498e08db-202d-44ed-a1d1-81493b6d3009`. The
  two observer regressions then passed, recorder `ecb97481-66df-4676-9209-b4057cf75ea2`. Both pre-fix failures are
  retained evidence, not successful runs or new latent flakes.
- Five related store, membership, interrupted-preparation and recovery tests passed, recorder
  `0e4ed333-c1db-4d14-983a-6ea26a7251b7`. These ran at `2518b853` and include the real status/reload path on pinned
  tmux; no runtime substrate skip was reported. Their results remain applicable: the later changes only strengthened the
  borrower fixture premise and adjusted commit metadata; the final four-test run exercised that strengthened fixture.
- Final validation at `37cbc331`: all four selected tests passed, recorder `58436ba9-8941-431b-98dc-3fbde30d24cf` (both
  observer regressions and both checkout end-to-end cases). The 1,422 other tests were excluded by the selection; no
  selected test skipped its runtime substrate. Targeted Clippy passed for all supervisor targets and the CLI end-to-end
  target, along with Rust formatting, TODO formatting, changelog format (78 fragments), and the isolated test-delay
  check (274 delays, zero missing rationales).
- Rebased the two-PR stack onto `fe9915b7`, yielding `b673e2c7` and `a2fa998b`. The incoming changes only delivered
  another plan’s report and claimed the installer plan. Their full diffs changed no code or specification contract used
  here, and the rebase left both patches unchanged, so the existing runtime and lint checks were reused.
- Skipped full Rust/browser batteries, desktop runtime, installer and release checks: the changed behavior is confined
  to the supervisor’s observation read and the test’s request binding. Existing classification/recovery tests and the
  checkout end-to-end module cover the affected callers. A browser repetition would not establish the ordering that
  caused this race; the ordered store reproduction does.

## Review gate’s outcome

Each PR received the required fresh-context Opus 5.5 review at high effort. Neither had correctness or design findings.
The first review sharpened a comment to say the connection fence is satisfied, not cleared. The second made the borrower
test’s accepted-terminal and generation premises explicit. It confirmed that the single store call closes the race while
preserving strict mutation checks, and that the deterministic store test covers the failure without adding a scheduling
hook.

Commit and PR wording passed separate fresh cold reads. This report receives a separate fresh-context cold read before
delivery.
