# Work-start spec still says Rename replaces its sampling state

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The implementation spec gives conflicting directions about whether a title-only change should preserve the current
screen reading.

## Details

**other.** Source inspection. Main reference `SPEC_impl.md:4102`.

#4529 rewrites the run-baseline section at SPEC_impl.md:4090–4100 and reaffirms that Rename preserves its activity cell
at 3522–3527 and 3553–3555, plus ticker.rs:370–373. Its immediate work-start continuation at 4102–4107 still groups
Rename and Restart as transferring history into fresh sampler state without old evidence. The pinned Rename path goes
from core.rs:13762 through renamed_entry at 3226–3229 and same_launch_entry at 3238–3251, which clones shared cells and
preserves generation. Only Restart uses the replacement concept documented and implemented at ticker.rs:516–532. The
changed run-sampling and Rename language makes the contradictory continuation directly relevant. Bucket: other.
Confidence: confirmed by source, no runtime observation. Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l5`.
