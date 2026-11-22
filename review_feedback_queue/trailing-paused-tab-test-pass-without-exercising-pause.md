# Paused-tab test can pass without exercising Pause

Reviewed commit: 06bc10e8e3038b241c01e298aa07121adee0564f

## TLDR

The tab-isolation test can pass without showing that the tab ever stopped receiving output.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/terminal_tabs.rs:1506`.

terminal_tabs.rs:1503-1536 sends Pause, starts an output producer and waits on the agent alone. It first checks the tab
receiver after sending Resume. crates/farhelm-helm/src/client.rs:4263-4285 explicitly provides no acknowledgement, and
harness.rs:2177-2197 accepts any matching text already queued. SPEC_impl.md:2339-2350 says a paused grid sender sends
nothing. Establish a paused phase with evidence that the shell has produced a distinct marker and its viewer has yet to
receive it, then assert delivery after Resume on the same channel. Bucket: other, because the source establishes a test
gap. Confidence: confirmed by source. Possible coverage: tab_lifecycle_edges.rs:598-650 establishes the distinct
stall-detach and reattach path. Lower-level host terminal pause tests may cover part of the contract.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_e2e/l3`.
