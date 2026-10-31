# A pending-create retry can delete initialized container files

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

After a crash and reboot during initial creation, a failed retry can remove a container that already holds the agent's
files.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The shim can initialize the container and run the agent before ConfirmRunning persists its pane. Supervisor death
followed by reboot before a successful reload removes tmux/scope evidence while the writable layer survives. Recovery's
pending-generation evidence checks omit that layer. A compatible retry followed by launch-spec publication failure calls
cleanup_failed_container before retaining or discarding the row, removing the initialized layer. Settled, recorded-pane
and advanced-generation cases are excluded. SPEC.md:1498–1501,1515–1527 accepts some startup process loss, but does not
explicitly accept this later persistent-file removal; SPEC_impl.md:5793–5794 remains possible coverage. Source ordering
is established; the combined crash/retry schedule was not executed.

Source locations: `crates/farhelm-supervisor/src/service/core.rs:9355-9424,10621-10641,14929-14931`. Historical revision
qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g113/f3`, `a003/g113/f3`, `g125/f7`. These are
original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts
remain in the private review evidence. No fix or triage decision was executed.
