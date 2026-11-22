# Setup loses its durable warning-eligibility bit on retained creates

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

An agent that survives an interrupted create can permanently lose the warning that its installed hooks have gone silent.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d
crates/farhelm-supervisor/src/service/core.rs:10484-10528,11654-11673,11848-11891,14718-14729,7291-7297;
crates/farhelm-supervisor/src/store.rs:5784-5803; crates/farhelm-supervisor/src/service/capture.rs:355-384

A surviving agent can appear to have installed hooks and never warn that Restart lacks a conversation. Reviewed commit
1a53d9d35889d7dde96e31320cc1681dd013737d, introduced by 47140bd63486498b7af024b05fa8ade7f8ea849e. Both the unavailable
initial attach branch and the failed confirmation with unconfirmed teardown can call publish_retained_launch. It clears
launch_hooked in SQLite and the mirror while retaining Installed. Reload restores the cleared bit, and the watchdog
never gets to the input or capture conditions. This is the same finding as hook-persistence hp-retained-create-disarms.
No existing warning row needs to be lost. The durable eligibility bit for a future HookSilent warning is erased, which
is different from HS-01's failure to create a HookNotAdded row. SPEC.md:3145-3155 and SPEC_impl.md:4512-4515. The
reviewer suggested other, confirmed by source for a surviving terminal that receives qualifying input and does not
report. No incidence asserted. ; Possible coverage: hp-retained-create-disarms; consolidate these two records.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-supervisor/HS-03`, with
`hook-persistence/hp-retained-create-disarms`.
