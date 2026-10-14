# Automatic install warnings omit the failure reason

Reviewed commit: 77069b3acaf0cecf7cc1741956451039dd23772e

## TLDR

Users see that an automatic update failed, but must find the app log to learn why.

## Details

Suggested bucket: `other`. Confidence: confirmed. Source: dedicated `gpt-6-astra` high adversarial review. Proposed
drop: none.

In crates/farhelm-ui/src/desktop/updater.rs:963–999, non-verification errors are logged and discarded. Lines 997–999
replace every cause with “installing Farhelm … did not finish (the app's log says why)”; the new automatic-notice path
at 874–881 publishes that text. For example, a failed checksum download and a local filesystem error produce the same
hover, despite SPEC.md:322–323 requiring the install-failure reason there and Errors and diagnostics:2058 requiring
concrete, actionable errors. Preserve a safe, bounded explanation of the failed stage/cause while continuing to judge
installation success from the Installed record. The generic manual-check message predates this commit; this finding
concerns its reuse for the newly specified automatic warning. The changed test at updater.rs:2299–2310 asserts only the
generic wording and misses this contract.

Possible cover: No matching queue entry, Planned item, BUGS entry, or relevant ledger decision found. The
rare-diagnostics filter does not fully match: ordinary download and installation failures suffice, without a narrow
timing window.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. All three changed Rust files are
identical to the reviewed commit; current SPEC.md:325–326 retains the reason-in-hover requirement. No fixing commit.

Review coverage: full commit diff read: true. No commit-diff content remains unread. Unchanged portions of large
source/test files outside the listed ranges were not exhaustively read. The 13,089-line triage ledger was searched for
relevant decisions rather than read end to end; unrelated queue finding bodies were not opened because INDEX identified
no matching item. An aggregated tool response truncated unrelated TODO/ledger output; required Planned/BUGS content and
relevant ledger matches were reopened before reliance. No Sol reports or working-copy queue additions were read.

Runtime evidence: None, as requested. Static review only; no checkout or scratch files created or changed. No
potentially-highest finding requiring independent drop review was identified.
