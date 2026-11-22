# Migration comment’s reboot classification remains disputed

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The migration wording may imply that a Launching row avoids the interruption assigned to Running rows. The ensuing
reboot classification gap remains unverified.

## Details

**other.** Unverified after investigation. Source inspection. Main reference
`crates/farhelm-supervisor/src/store.rs:1868`.

store.rs:1863–1879 says Running would additionally mark a backfilled row Interrupted on the next reboot, after
contrasting it with Launching. Both are interruption inputs: record_boot's predicates at 6462 and 6477 include launching
and running, and the explicit legacy retirement transition at 542–547 applies to both. SPEC.md:2980–2984 and 3405–3417
describe Interrupted as a last-known-live outcome from either route. Keep the accurate reason for Launching, that
migration cannot assert a completed start, without implying it avoids interruption. Bucket: other (documentation
correctness). No migration logic defect established. Confidence: confirmed by pinned source. Possible coverage: recovery
interface.

Latest source check. Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad. TLDR: The migration conservatively
records Launching, and reboot or legacy retirement can interrupt it. Details: the changed text at store.rs:1870–1872
replaces the live-pane probe claim with startup retirement without a probe. Core.rs:6744–6748 and 6873–6887 confirms
that repair. The preceding claim that Running can become Interrupted is also true, as is Launching becoming Interrupted,
under store.rs:542–547,6462,6477 and the existing boot assertion 10650–10678. The comment does not explicitly claim that
Launching avoids interruption. Bucket: other. Confidence: confirmed by source, no runtime observation. proposed_drop:
set aside the inferred contrast. The repaired migration contract survives the pin checks and no independent mismatch was
established. Could not be highest. Possible coverage: recovery interface.

The independent check did not settle the proposed exclusion. AMBIGUOUS via 44137d0 store.rs:540–549 and 1861–1881.
Core.rs:6742–6750; SPEC.md:2978–2986; SPEC_impl.md:2587–2594. The no-pane-probe repair is supported, and legacy
retirement can interrupt either outcome. The disputed sentence is about next reboot. The reboot predicates or an
assertion covering both outcomes are missing.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l13`.
