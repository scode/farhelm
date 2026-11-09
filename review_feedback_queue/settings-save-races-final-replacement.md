# Final settings comparison still races a concurrent save

Reviewed commit: d137a2613f92b06e7332f12c02484b4a9faeec90

## TLDR

Credential delivery can silently overwrite an agent settings save made after its final digest check and before
replacement. The new comparison protects earlier edits but leaves this narrower window.

## Details

Suggested bucket: **high**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

TRIAGE_OUTCOMES.md:21552-21582 records the original overwrite fix and explicitly rates its recreatable settings-save
consequence HIGH, not HIGHEST. A fresh consultation disagrees that the residual race is fully covered or accepted, and
agrees that same HIGH rating applies. The original reviewer suggested HIGHEST; that provenance is preserved here. The
decision's principle forbids undoing any save after the read; the implementation still has a separate check and replace.
This is the incomplete-implementation remainder, not a second report of the already repaired pre-check window.

## Recorded source assessment

Reviewed d137a2613f92b06e7332f12c02484b4a9faeec90; synthesis also checked tip 9e3ab60b86025ca74948ededd09b99cd81a2477c.
TLDR: reconnect or credential replacement can silently discard an ordinary host settings save after hashing but before
replacement. At the original revision backend.rs:1152-1159 checks the old digest; 1170 subsequently chmods the temporary
and mv replaces the destination. Local metadata await at 1104 is likewise separate from rename at 1111-1112.
credential_delivery.rs:462-525 retries only explicit generation refusal and records success after a successful install;
post-check saves trigger no refusal. Confirmed source interleaving, suggested highest for lost user-owned configuration.
No runtime under source-only constraints. Synthesis checked tip backend.rs:1095-1255 and delivery.rs:390-535,756-798:
later changes preserve this contract; delivery stamps and cancellation-safe database guard protect Farhelm sequencing,
not external editors. New tests plant writes before the comparison, not in the remaining window. Fix needs
preservation/recovery or a protocol avoiding silent loss across this window, since digest-check plus rename is not
atomic compare-and-replace. SPEC.md:423-428 requires unrelated settings remain intact; SPEC_impl.md:6213-6229 describes
comparison/retry but does not explicitly accept losing concurrent saves. proposed_drop: none. possible_cover: parent
should assess whether SPEC_impl intended consciously best-effort behavior; no explicit acceptance found. Cross-group
input read in full: g010/c1,c2,s1; g023/c1,c2; g026/l1 and its refutation; g029/c1-c4 including revisions;
g039/c1,l1,c2,c3 including revisions. No duplicate of f1 in those durable records. g039/l1's SPEC.md:352-363 acceptance
concerns logical host entry retargeting, not overwriting unrelated file bytes on the same machine. g029's
preparation/one-shot transport changes do not add file-writer synchronization. g010's tunnel lease/shutdown and
g023/g026's state-path/probe issues do not alter this caller/callee path. Original IDs and proposed drops remain with
their owners; no independent drop verdict implied.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g022/f1`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
