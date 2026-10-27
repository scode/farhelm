# Shutdown regression does not witness the in-flight wait

Reviewed commit: 7eb9990be9c4f65397546f5b91555be2a62da983

## TLDR

The shutdown regression can pass even if shutdown stops waiting for a launch whose registration is still in flight.

## Details

`crates/farhelm-supervisor/src/service/core.rs:27188–27212` delays registration, asserts that the wait returns true,
then joins registration. An incorrect wait that immediately returns success also satisfies those observations. The test
needs an independent witness that registration is pending while the wait remains unfinished, followed by release and
completion. An elapsed delay alone is not that witness.

Suggested bucket: `other`. Confidence: confirmed test-oracle gap. Production admission counting remains correct; no
shutdown or process-loss defect is established. Possible cover: none exact found in the pinned coverage sources.
Provenance: `g005/f1`. Static inspection only; no mutation test or runtime reproduction ran.
