# Peer text normalization contract overstates retained scope

Reviewed commit: 8f51f83ef0a304d7ef568bae30ad98e892ac7c6c

## TLDR

The specification's universal normalization rule for retained peer text contradicts its explicit raw-byte identity and
diagnostic exceptions.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `SPEC_impl.md:3075-3077 at 8f51f83ef0a304d7ef568bae30ad98e892ac7c6c`.

Source assessment (read with the later corrections below) (review record g077/d4, event 37):

Reviewed commit 8f51f83ef0a304d7ef568bae30ad98e892ac7c6c. TLDR: the specification says all retained peer text receives
the same normalization, even though raw diagnostics and operational identities must retain their original bytes; this
needs a narrower diagnostic-copy wording. Suggested bucket other; confidence confirmed text-level contract clarity
defect. No shipped lossy identity transformation or security/data-loss/highest consequence is claimed. The actual
assigned text at SPEC_impl.md:3070-3088 universally says 'All peer-supplied text is normalized the same way wherever the
helm logs or retains it, including identifiers' after describing bounded Debug escaping. The same local context retains
raw diagnostic bytes before failure-display cleaning, and :3104-3108 preserves opaque hello identities without
truncation/shape rewriting. session_ref.rs:37-49,81-84 and UI peer.rs:39-49 likewise preserve raw operational
identities. These clauses make the intended diagnostic/operational distinction inferable; they do not make the universal
retained-text sentence literally accurate. The original d4 already acknowledged that context and expressly disclaimed an
implementation-corruption claim. Correction: my prior withdrawal refuted a hypothetical lossy runtime route rather than
the actual wording defect and therefore did not justify excluding d4. Parent reports independent drop23/verdicts.json
g077/d4 DISAGREED after checking those exact sources and pinned-tip SPEC_impl.md:3104-3125,3138-3142. Restore the
original finding under its original ID; preserve the withdrawal event as history. g027:g027-d3's correct
diagnostic-copy/operational-raw distinction remains useful context, not a proof of the universal sentence's accuracy.
Fix by qualifying logging and retained diagnostic/display copies while preserving raw working/protocol values.
proposed_drop false; possible_cover unknown, parent handles coverage. Exact wording remains at pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b:3109-3110, with raw diagnostics at :3116-3119. No new exclusion basis is
proposed. No runtime, process, browser, credential or network-endpoint experiment.

Recording judgment: drop23 DISAGREE; owner restored verified other wording finding at line37. Universal retention
sentence remains broader than adjacent raw-byte exceptions, without a shipped lossy-data-flow claim.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g077/d4`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
