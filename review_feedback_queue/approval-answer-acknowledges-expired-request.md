# Approval answers can acknowledge a request that expired

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

An answer near an approval's expiry can be acknowledged as delivered even though the request expires unanswered. The
card disappears without the notice that the answer was too late. The asking agent receives a refusal; this does not
establish execution without approval or false success for the requested action.

## Details

`crates/farhelm-helm/src/approvals.rs:707–715` removes the pending entry, ignores the result of sending its answer, and
returns HTTP 204. The waiting request independently selects between receiving the answer, its deadline and connection
changes at lines 431–439. An answer can be queued successfully while the deadline is also ready, then lose that
selection. A successful table removal therefore does not prove that the waiting request accepted the answer. A failed
send is another case the endpoint currently acknowledges, but checking send failure alone would leave the
successful-send/expiry-wins case.

The endpoint's contract at `approvals.rs:571–574` reserves 204 for an answer that reached a waiting request and 410 for
a request no longer waiting. `crates/farhelm-ui/src/api.rs:4779–4785` maps a successful response to `Delivered`.
`crates/farhelm-ui/src/approvals.rs:242–247` displays the late-answer notice only for `Gone`, so this acknowledgment
suppresses the notice. `SPEC_impl.md:5500–5508` promises 410 after the wait ends; lines 5559–5561 also describe that
notice for an Always allow answer overtaken by expiry.

To verify, hold the waiter from polling, enqueue an answer, make its deadline ready, then resume it and compare the HTTP
response, wait result and UI notice. Exercise the case where the send succeeds and expiry wins, as well as a receiver
already gone. The answer acknowledgment and the waiter's final disposition need a shared settlement rule; checking only
whether the sender accepted the value is insufficient.

Suggested bucket: **other**. Confidence: **likely**; the conflicting control paths and acknowledgment contract are
confirmed by static inspection, but the adverse schedule has not been reproduced. No runtime test was run for this
finding. The consequence is false success for answer delivery, not for executing the agent's action.

Possible cover: no matching Planned item, known bug, queue item or ledger decision was found. The rare-diagnostic filter
may apply if this is judged solely a missing notice; the review retained it because the shipped API reports successful
delivery, and that filter excludes success reported for a failed operation. The accepted persistence of an Always allow
setting after expiry does not explicitly accept a false delivery acknowledgment. No drop is proposed.

Provenance: prioritized persistent review, reviewer A09-approval-answers, finding A09-002; retained after synthesis and
parent source verification. Discovery pass 1 found no additional defect in this bounded group.
