# Ended-runtime seed cancellation test accepts a lost emulator

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The regression test for keeping an ended runtime's emulator readable also passes when read reports that the emulator was
lost.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed test-oracle
defect by source; no current production loss or measured false-pass mutation claimed.

Source locations: crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs:2913-2916 at
ca9b0a19856dc136f63ad2d14f1076027edce1eb.

a_cancelled_seed_of_an_ended_runtime_leaves_the_emulator_readable asserts only timeout(WAIT, host.read(|_|
())).await.is_ok(); this accepts Ok(Err(EmulatorLost)). At 32e20f84647951a8ea717b3c712a037c231d92dd,
crates/farhelm-supervisor/src/hosts/terminal.rs:2050-2083 documents and implements read returning Err(EmulatorLost) for
Slot::Lost, separately from a timeout. Unlike the running-runtime cancellation test, this test has no later successful
seed/output check that rejects loss. Require both the timeout and inner Result to succeed (and preferably inspect the
retained history).

The regression test for keeping an ended runtime's emulator readable also passes when read reports that the emulator was
lost. The assertion checks only timeout(...).await.is_ok(), which accepts Ok(Err(EmulatorLost)); tip
hosts/terminal.rs:2052-2084 documents/implements that distinct failure. Tip host_terminals.rs:2885-2917 has no
subsequent successful read/seed/output assertion to reject the loss. Require the inner Result to succeed too.

Possible coverage and caveats: none exact. Full Restarting-a-host-agent paragraph at tip SPEC_impl.md:2120-2194 was
revisited in synthesis: :2140-2145 explicitly accepts restarting empty when the host/runtime ended, and :2192-2194
accepts leaving an intentionally aborted runtime's emulator away. Neither calls an Err(EmulatorLost) successful readable
access in this named regression, so these product omissions do not cover this test-only oracle gap. Initial full
coverage checks and ledger/queue searches remain applicable to the same pinned tip.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g141/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb.
Introduced by 32e20f84647951a8ea717b3c712a037c231d92dd:2903-2906.. This was a source-only review; runtime occurrence and
frequency were not measured.
