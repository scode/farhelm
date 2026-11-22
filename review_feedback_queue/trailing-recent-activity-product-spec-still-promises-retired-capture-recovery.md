# Recent-activity product spec still promises a retired capture-recovery baseline

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The recent-activity contract tells readers that a recovered screen always starts a fresh ordering baseline after capture
failure.

## Details

**other.** Source inspection. Main reference `SPEC.md:2710`.

SPEC.md:2704–2716 keeps that clause, while #4529 specifically removes the failed-capture reset from
SPEC_impl.md:4071–4095 and ticker.rs:262–266,416–423. The only sampled screen read is infallible with respect to
terminal transport (hosts/terminal.rs:2236–2246). ticker.rs:1311–1329 skips unreadable and not-yet-caught-up terminals
without clearing the cell; 599–605 uses the retained prior reading whenever one exists. Same-generation re-adoption
reuses the run cell (core/host_handover.rs:1695–1705). Existing ticker.rs:4653–4667 asserts that an unsampleable pass
keeps prior comparison evidence. Thus there is no capture-error reset on the shipped path, and a later informative
same-run reading can transition from the retained baseline. Update the product sort contract to distinguish a skipped
sample from a genuinely fresh run or supervisor sampler. Bucket: other, documentation. Confidence: source and assertions
only, no runtime observation. Possible coverage: l4 checks the implementation's intended baseline, not this residual
product clause.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l21`.
