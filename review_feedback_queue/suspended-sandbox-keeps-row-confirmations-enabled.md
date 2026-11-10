# Retained row questions remain enabled after sandbox suspension

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

A Stop or Replace question opened before suspension can remain answerable after the normal menu actions disable.
Accepting it dismisses the question and sends a request the backend refuses.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/list/row.rs:2355`. Locations below use that revision unless another is named; the assessment was
checked through the reviewed tip.

Open a sidebar Stop or Replace question on a running sandbox, then suspend the sandbox elsewhere. The list-owned
question survives because reconciliation prunes only absent sessions (list/view.rs:1913-1927); closing and reopening the
same row menu does not clear it (:3654-3659). Even after stale state converges, the retained question remains current.

row.rs:1137-1148 derives sandbox_frozen, and ordinary menu actions use it, but confirm-stop, Stop-and-suspend and
confirm-replace at :2336-2375,2457-2511 disable only on busy. Their current-token handlers (view.rs:2522-2572,3040-3051)
also omit frozen availability. Thus correct token binding does not repair this separate predicate. SPEC.md:1465-1467
requires Stop/Restart/Replace disabled while suspended.

Backend routing refuses the request, with no automatic Resume or established process loss. Preserve Cancel and the
question's ownership while disabling/refusing inapplicable answers until Resume restores availability. Suggested bucket
other, confirmed stable source state. The related retained-header/interrupted-card item has different controls and
source locations; menu-owner loss and stale-token bugs are separate again.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g056/g056-inline-frozen`, exclusive
base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private
findings history, dispositions and independent consultations preserve original confidence, suggested buckets and every
revision. No fix or triage decision was executed.
