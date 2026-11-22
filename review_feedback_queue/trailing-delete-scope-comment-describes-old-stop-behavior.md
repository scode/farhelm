# Delete scope comment describes the old Stop behavior

Reviewed commit: df6199142d6495a95b2d008400ba2686efb775c6

## TLDR

The cleanup source misstates what happens to terminal tabs when a session is stopped.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/teardown.rs:370`.

teardown.rs:370-375 says Delete is the only lifecycle operation that takes tabs down and attributes Stop leaving them
running to SPEC.md. SPEC.md:2067-2073 explicitly requires Stop to close the tabs while Restart keeps them. The useful
distinction here is that Delete reaps the whole session in one sweep, while Stop reaps the main launch and then closes
each tab separately (SPEC_impl.md:4674-4689). Correct the comment to explain that distinction. Bucket: other, source
documentation only. Confidence: confirmed by source and contract. Possible coverage: none.

Additional source record at `44137d0659d28b9cadf90a8ffe96a65930035aad`, `crates/farhelm-supervisor/src/launch.rs:105`.

stop/f1 at inclusive tip df6199142d6495a95b2d008400ba2686efb775c6 identifies teardown.rs:370–375. Both comments conflict
with SPEC.md:2067–2073 and SPEC_impl.md:4671–4689. The shipped Stop calls the agent-only operation at
handlers.rs:945–952 and then closes tabs at 1156 through core.rs:13566–13589. Restart preserves tabs. Delete's
WholeSession path includes their roots in one teardown. The #4529 launch hunks repair inherited marker flow at
launch.rs:110–118 and 1225–1229, leaving the preceding operation description at 105–108 unchanged. That nearby line
still deserves correction, as an additional location for stop/f1. Bucket: other. Confidence: confirmed source and
contract, no runtime observation. proposed_drop: set aside as a separate finding and combine this location with stop/f1.
Could not be highest. Possible coverage: stop/f1.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `stop/f1`, `recovery_contract/f8`.
