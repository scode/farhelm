# Cancellation during completed-download cleanup retains a route

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

Cancelling a completed download at a narrow point in cleanup can leave its bookkeeping in the helm until the old host
client object is discarded.

## Details

`crates/farhelm-helm/src/client/downloads.rs:179–180` sets `finished = true` before awaiting removal of the download
route. Cancellation during that wait makes Drop skip cleanup (`:218–224`). The wait can contend: the demultiplexer
notifies EOF while holding the same mutex (`client.rs:1841–1850`), so an awakened consumer on another worker can block
on it.

`fail_all` does not drain this route map (`client.rs:1603–1607`). Subsequent unexpected data might remove the closed
receiver, but honest EOF supplies none; re-polling the finished guard also skips removal. Dropping the last
SupervisorClient reference frees it. Transport tasks hold weak references.

Disarm cleanup only after route removal. A focused cancellation test can hold the route mutex, observe EOF delivery,
cancel the waiting receiver and verify eventual removal.

Suggested bucket: **other**. Confidence: **confirmed cancellation-unsafe ordering and reachable lock contention**;
occurrence and impact are unmeasured. The window is small. File bytes have drained; this retains no file contents,
pending admission or live supervisor transfer.

Possible cover: `download-cancel-retains-pending-admission.md` concerns the earlier admission request, not this
completed-transfer route. Persistent bookkeeping retention does not fully match the rare self-correcting-glitch filter.
No exact accepted specification, Planned item, BUGS entry or queue item was found.

Proposed drop: none. Provenance: persistent Astra/high review, `helm-wire-client-1/f2` and
`supervisor-file-transfers-1/f3`.
