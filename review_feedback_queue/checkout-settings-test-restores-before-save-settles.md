# The save-and-clear test can restore configuration before its pending write finishes

Reviewed commit: 126bbe267d31f256678f7b9a0466860ebd5a5c50

## TLDR

A failed browser test can leave its checkout-folder setting behind and affect later tests.

## Details

Suggested bucket: `other`. Confidence: likely; the missing completion barrier is confirmed, but a failure with the
original PUT overtaking restoration was not reproduced. Source: dedicated `gpt-6-astra` high adversarial review.
Proposed drop: none.

e2e/tests/settings.spec.ts:56–57,77–85: if an assertion fails while a submitted save remains pending, finally
immediately sends the restoration PUT without draining the original request. That request can subsequently overwrite
restoration. Its lifetime is explicitly app-owned (checkout_root.rs:85–88), and API requests have a 60-second deadline
(api.rs:1034,1097–1100). The adjacent pending-save test already handles this ownership boundary by awaiting the
submitted request before restoration at settings.spec.ts:131–140. Apply equivalent tracking to the first test.

Possible cover: No matching Planned item, BUGS entry, original queue item, or ledger decision found. The supplied test
checklist requires cleanup to respect outstanding fixture operations.

Status at frozen endpoint: Persists at pinned main, e2e/tests/settings.spec.ts:345–407, particularly 374–375,395–405.

Review coverage: full commit diff read: true. No changed diff lines unread. TRIAGE_OUTCOMES.md was searched and relevant
material inspected, not read linearly in its entirety. Unchanged surrounding modules, other queue findings, and
unrelated later changes were not exhaustively reviewed. No Sol reports or working-copy queue additions were opened.
Truncated relevant output was reread in smaller selections.

Runtime evidence: None. Static review only; no browser, network-ordering, or filesystem reproduction. Applied the
supplied checklist to changed tests. No checkout or scratch files created, modified, or deleted; no fetch, rebase, or
helper agents.
