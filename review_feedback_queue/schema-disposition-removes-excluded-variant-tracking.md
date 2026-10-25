# Schema cleanup removes tracking of an excluded variant

Reviewed commit: 544ba3c2cc188a5b6dfb1abe1bd2f20c767ff19f

## TLDR

An explicit queue-removal instruction also removes an equal-number schema concern excluded from the planned fix. Whether
that means deliberate whole-finding discard is ambiguous; no concrete incompatible migration pair or corruption route
was established.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `TRIAGE_OUTCOMES.md:18704-18707`. Locations and revisions in the assessment below identify the reviewed
source; later main is outside this review's pinned range.

Synthesis complete; g211/f1 remains AMBIGUOUS on whole-record exclusion, not a verified product defect. Reviewed commit
544ba3c2cc188a5b6dfb1abe1bd2f20c767ff19f. TLDR: the closed queue record includes an excluded concern without a clear
substantive disposition. Refine earlier wording: removal loses active queue visibility, not all historical
documentation; the ledger itself preserves the unverified same-number variant. Deleted feedback parent lines
12-14,46-49,60-63 carry that scope. Ledger :18693-18694 preserves uncertainty, :18704-18705 excludes the variant from
fix code, :18706-18707 explicitly orders full removal. Parent plan :34-43 specifies numeric comparison and excludes the
variant; :77-78 requires removal; Decisions :293-365 approve the larger bounded fix and waive size, without expressly
discarding the excluded variant. Thus AGREE that execution/removal was authorized; AMBIGUOUS that it resolves the
variant's disposition. Historical queue AGENTS.md:24-44 requires partial fixes to narrow remainder and defines discard
separately. Call sites in provisioning/service.rs:1080-1139,1860-1930 apply the same strictly-newer-number helper at
planning and confirmation for both protocol cases. The added provisioning.rs test :2206-2303 explicitly expects equal or
missing numbers to plan and higher numbers to refuse, including revalidation before backend operations. This supports
a009's intentional numeric-guard scope; it neither proves compatibility between different same-number schemas nor
supplies the missing harmful migration pair. No new test-gap or call-site defect is asserted. Suggested bucket OTHER for
conditional queue-disposition issue; potential HIGHEST only if concrete incompatible migrations plus a shipped
misinterpretation/corruption route are established. Those remain missing SOURCE premises, not runtime-frequency caveats.
Confidence: confirmed record removal and guard scope, ambiguous disposition, underlying product harm unverified.
possible_cover: exact ledger and plan removal requirements, acceptance of the bounded fix. proposed_drop: not agreed;
retain conditionally for parent resolution. Existing pinned six-source searches and SPEC/FILTER boundaries still apply;
no new candidate required additional coverage searches. No runtime actions or checkout changes.

The later empty discovery occurred after the quoted synthesis. It introduced no new finding and did not establish a
concrete incompatible migration pair. The original record's removal was explicitly authorized; this asks whether that
authorization also discarded the excluded variant's substance, not whether the executor ignored the instruction.

Provenance: `a070/f1`, `g211/f1`, `g203/f2`, `g289/f1`, `g290/f1`. These are reviewer-local identities, not additional
findings. Proposed drop: none for the retained remainder.
