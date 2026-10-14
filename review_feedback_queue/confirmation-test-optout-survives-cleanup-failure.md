# Failed test cleanup leaves session confirmations disabled

Reviewed commit: e440591fd6e95188fe171d0e46d4c27703477386

## TLDR

A browser-test cleanup failure can leave session confirmations disabled for later tests, causing unrelated failures.

## Details

Suggested bucket: `other`. Confidence: Confirmed static trace; not reproduced. Shared test-state contamination only.
Source: dedicated `gpt-6.1-sol` high adversarial review of this commit. Proposed drop: none.

`e2e/tests/delete-precondition.spec.ts:139–140` restores `skip_session_confirmations` only after `cleanupSession`
succeeds; that helper throws on non-404 refusal (`e2e/tests/helpers/fleet.ts:520–530`). The opted-out test has already
persisted true. Later tests expect a confirmation (`delete-precondition.spec.ts:169–175`) and instead delete directly.

The settings fixture has the same dependency: restoration (`e2e/tests/settings.spec.ts:527`) is skipped if owned-host
cleanup (`525`) or another preference restoration (`526`) throws. Neither file has an independent reset hook; the
evidence fixture does not reset preferences.

Restore this preference in an independent finally while keeping cleanup errors visible, or use an independent reset hook
as `session-confirmations.spec.ts` already does.

Possible cover: Ledger headings `history-fixtures-policy-restoration-skip-remaining-cleanup.md` and
`keyboard-launch-fixtures-policy-restoration-skip-session-deletion.md` cover analogous different fixtures.

Current-main status: persists by inspection at `4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703`; no fixing commit found. The
reviewer read the full commit diff and relevant traces; this is not an exhaustive review of unchanged surrounding code.

## Additional affected scope

Reviewed commit: e440591fd6e95188fe171d0e46d4c27703477386

## TLDR

A failed cleanup can leave session confirmations disabled in the shared test helm, causing unrelated tests to fail
afterward.

## Details

Suggested bucket: `other`. Confidence: confirmed. Source: dedicated `gpt-6-astra` high adversarial review. Proposed
drop: none.

e2e/tests/delete-precondition.spec.ts:138–140 restores the preference only after cleanupSession succeeds. That helper
explicitly throws on unsuccessful cleanup responses (helpers/fleet.ts:520–529), so the new skip=true case can leave its
persisted opt-out behind. Later tests in the same file expect a confirmation (delete-precondition.spec.ts:169–175).
settings.spec.ts:524–527 has the same dependency: host deletion or restoration of the host preference can prevent
restoration of the session preference. Neither file registers an independent preference-reset hook;
helpers/evidence.ts:31–65 supplies none, and playwright.config.ts:83–100 confirms the shared-stack arrangement. Also,
delete-precondition.spec.ts:68–70 now reads preferences after creating its session but before entering try, allowing
that read's failure to bypass session cleanup entirely. Arm cleanup immediately and attempt independent
cleanup/restoration steps even when another fails. Scope is browser-test isolation, not shipped product behavior.

Possible cover: TRIAGE_OUTCOMES.md headings history-fixtures-policy-restoration-skip-remaining-cleanup.md and
keyboard-launch-fixtures-policy-restoration-skip-session-deletion.md address analogous failures in sidebar.spec.ts, not
these newly changed fixtures. No exact coverage found. The rare-glitch filter does not fully match: the shared
preference remains durably changed.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. Both affected test files are unchanged
from the reviewed commit; no fixing commit.

Review coverage: full commit diff read: true. No commit diff hunks remain unread. Initial aggregate output truncation
was resolved by smaller rereads. Unchanged files were inspected selectively, not exhaustively. Most TRIAGE_OUTCOMES.md
entry bodies and queue files unrelated by the pinned index were not read. Backend lifecycle implementations beyond the
cited guard paths were not exhaustively re-reviewed. No Sol reports or working-copy queue additions were read.
Independent review of the proposed highest-candidate drop remains pending.

Runtime evidence: None, as requested. Conclusions are static traces. Applied the supplied test-authoring checklist to
changed tests and fixtures. No checkout files or scratch artifacts were created, modified or deleted; no fetch, rebase
or helper agents.
