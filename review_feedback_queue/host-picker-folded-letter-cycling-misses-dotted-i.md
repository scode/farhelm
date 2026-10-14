# Repeated-letter host navigation fails for İ

Reviewed commit: 164101bdffff7d0e7dfef79652a6abe9f22014f7

## TLDR

Repeatedly typing İ cannot cycle through hosts whose names begin with that letter.

## Details

Suggested bucket: `other`. Confidence: confirmed by source inspection; no browser reproduction. Source: dedicated
`gpt-6-astra` high adversarial review. Proposed drop: none.

crates/farhelm-ui/src/list/host_picker.rs:122–124 tests repetition by asking whether the newly lowercased letter starts
with every character of the previous prefix. İ lowercases to two characters, i followed by U+0307. After the first İ,
the second character fails starts_with, so another İ appends to the query instead of cycling. With hosts named İalpha
and İbeta, two successive İ keystrokes leave the first match active. SPEC_impl.md:4127–4128 promises case-insensitive
prefix matching and repeated-letter cycling. Compare the complete folded key with the previous repeated-key state rather
than testing its constituent characters. Scope is keyboard convenience; the committed destination does not change until
explicitly chosen.

Possible cover: None found. TRIAGE_OUTCOMES.md heading non-ascii-template-names-lose-exact-match-priority.md concerns a
different search and comparison.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: host_picker.rs is unchanged, and
SPEC_impl.md:4579–4580 retains the cycling requirement.

Review coverage: full commit diff read: true. No commit-diff content remains unread; truncated outputs were reopened.
The 13,089-line triage ledger was searched and relevant sections read, not read exhaustively. Unrelated queue finding
bodies, whole unchanged modules, and unchanged portions of large test files were not read. The historical-to-main CSS
comparison was inspected only through its first 350 diff lines; no complete later-CSS audit is claimed. Browser
geometry, assistive-technology behavior, native scrollbar focus, and actual locale-specific key delivery were not
runtime-verified.

Runtime evidence: None, as requested. Applied the supplied test-authoring checklist by source inspection. No checkout
files or scratch artifacts were created or modified; no agents were spawned.
