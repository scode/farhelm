# Cancelled upload commit retains its reply entry

Reviewed commit: 7d85af70b9ce54d90f48f8a106075aa9ee5739c7

## TLDR

Repeatedly cancelling uploads during their final save can accumulate reply bookkeeping until the host connection ends.
Practical memory impact is unmeasured; no file loss or wrong-target action is established.

## Details

Suggested bucket: **other**. Confidence: **confirmed conditional source mechanism**; incidence and retained-byte impact
were not measured. This behavior predates the exclusive review base and is an uncovered residual in the upload-guard
cleanup being reviewed, not a regression introduced by this commit.

`crates/farhelm-helm/src/client.rs:3987–4004` creates a separate commit request ID and registers it through `2532–2581`.
Guard Drop removes the admission ID only (`4110–4128`). Cancellation reaches the supervisor's commit signal receiver
(`crates/farhelm-supervisor/src/service/handlers.rs:2494–2503,2945–2948`; `service/uploads.rs:632–634`). When the reply
queue is full, the cancellation reply can be deliberately omitted (`service/uploads.rs:342–353, 1104–1154,1216–1255`).
If the connection subsequently drains and stays live, no reply removes the commit entry.

Retire the separately registered commit waiter when its owner is cancelled, without weakening cancellation ownership or
making abort wait on full writer capacity. A focused check should keep the connection live, withhold reply capacity at
cancellation, then prove both admission and commit bookkeeping retire after capacity returns.

Possible cover: `TRIAGE_OUTCOMES.md:12867–12878`, `upload-cancellation-drops-final-reply.md`, discards a narrower case
where the target session is being deleted and the client wait never finishes. This case concerns an abandoned commit
waiter after ordinary cancellation with a surviving connection. The malicious/buggy-supervisor nonresponse exception
does not clearly cover deliberate shipped cancellation behavior. No exact duplicate was found. Proposed drop: none;
preserve the old-code attribution and partial acceptance for triage.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, discovery `g148/f3`; source and the exact
deletion-specific decision were checked by the parent.
