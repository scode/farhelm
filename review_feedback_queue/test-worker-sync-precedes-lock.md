# A second test request can change a worker's active checkout

Reviewed commit: `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`

## TLDR

Starting a second test run on the same SSH worker can change the files the first run is testing. The second run can then
report that the worker is already in use while the first run continues against a changed tree.

## Details

The driver copies its tools, syncs and prunes the tree, and records a source snapshot before asking the worker to take
its run lock. The lock therefore protects only execution. Hold it across the source staging used by that run, or stage
each run in a tree it owns. The driver order is in `scripts/test-worker/test-worker.sh:157`. The lock starts in
`scripts/test-worker/remote/with-lock.sh:12`.

Introduced by `663b6c9ce` and present after the snapshot rebuild change. The command order establishes the overlap by
source. No affected test run was observed.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g065/l6.
