# Work-start guide omits deferral while the lifecycle claim is busy

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

A newly sampled question or work burst can update the badge before the recency key changes, and an observation deferred
during relaunch can lose its chance to update the key.

## Details

**other.** Source inspection. Main reference `SPEC_impl.md:4095`.

the #4529 work-start paragraph at SPEC_impl.md:4090–4095 says badge and order cannot drift. Its continuation at
4103–4108 says a start writes immediately. SPEC.md:2704–2716 defines the order using observed transitions. The ticker
updates the reading and hints the status before calling persist_work_started (ticker.rs:1376–1404). A busy lifecycle
claim parks the start without advancing the key (1430–1451). It reserves a key only after validating the current run
(1452–1472). The deferred flag is run-scoped and discarded at replacement (440–451,516–532), whereas an allocated key
awaiting SQLite is preserved. The existing busy-claim test at 4276–4348 asserts no key or pending write under contention
and writes only after release. Describe the deferred/accepted boundary and why the ticker cannot wait. Bucket: other,
documentation mismatch with the deliberate nonblocking path. Confidence: source-confirmed behavior and assertions, no
observed display timing. Possible coverage: l5 edits the same section but concerns Rename state ownership, a separate
claim.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l19`.
