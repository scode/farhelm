# Trash fixture cleanup compares canonical paths with lexical roots

Reviewed commit: 0640cfee761414b1fd92fba3f70bd5cc6f7238a4

## TLDR

On macOS, checkout tests can remove their fixture folders while leaving their trash records behind, contaminating later
trash listings.

## Details

Suggested bucket: `other`. Confidence: likely: path handling is confirmed by inspection; execution with the default
macOS /tmp alias was not performed.. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

e2e/tests/github-checkouts.spec.ts:119 selects owned trash using entry.path.startsWith(root + path.sep). The fixture
root comes from stackScratchDir (73–74), whose source preserves the stack's lexical state path
(helpers/scratch.ts:30–44). start-stack.sh:123 creates that path under /tmp and publishes it unchanged at 406–420. On
ordinary macOS, canonical paths use /private/tmp. The supervisor canonicalizes checkout roots
(service/core.rs:7432–7463), and trash paths use those canonical roots (checkout_trash.rs:107–109, 307, 320).
Consequently the prefix selection is empty and cleanup silently skips the records, while finally removes scratch at 130.
Subsequent passive reads retain missing-root issues rather than pruning them. Canonicalize the fixture root before
matching host-returned archive paths. Scope: browser-test fixtures, not product user files.

Possible cover: review_feedback_queue/agent-checkout-test-compares-lexical-tmp.md and TRIAGE_OUTCOMES.md's
checkout-recovery-compares-canonical-roots-unresolved-fixture.md concern different Rust assertions, not this browser
fixture's archive-record cleanup.

Status at frozen endpoint: Persists at pinned main: github-checkouts.spec.ts:164–165, 210 and 221 retain lexical-root
selection followed by scratch removal; start-stack.sh:123 still uses /tmp. No fixing commit identified.

Review coverage: full commit diff read: true. No commit-diff content remains unread. Initially truncated trash output
was replaced by complete numbered reads. The large triage ledger was searched and relevant sections read, not read end
to end. Unchanged surrounding modules and backend tests were inspected only in the listed ranges; no exhaustive backend
audit or visual browser inspection.

Runtime evidence: None. Static historical/current-source review only; no checkout files or reports created, modified or
deleted. Changed tests and fixtures were assessed against the supplied authoring checklist.
