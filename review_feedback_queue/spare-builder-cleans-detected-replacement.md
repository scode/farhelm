# Spare cleanup deletes an already detected building replacement

Reviewed commit: 8f5d016f04ab13c284530e759e828826790f2843

## TLDR

The prepared-checkout builder can detect that its temporary folder was replaced, refuse to publish it, then delete the
replacement during cleanup. Loss of user work is conditional on that replacement containing such work. No ordinary
Farhelm operation replacing this hidden temporary folder has been established.

## Details

Suggested bucket: provisionally **highest**, for the conditional deletion of replacement files. Confidence: **confirmed
source mechanism under the replacement premise; coverage is ambiguous**. Runtime occurrence and frequency are
unmeasured. The reserved generated name and absence of an established ordinary replacement actor are material caveats,
not evidence that publication refusal protects the replacement through cleanup.

`crates/farhelm-supervisor/src/spare_checkouts.rs:246–265,289–323` retains the building folder's original identity and
refuses a replacement at helper verification or publication. `service/spare_builder.rs:354–435` still reaches
post-attempt cleanup. That cleanup protects claims, but captures the current identity for strict building names and
removes them (`spare_checkouts.rs:148–164,540–554,596–608`). The replacement test at `656–683` establishes publication
refusal and stops before this cleanup. Replacing the building folder before verification is sufficient; no substitution
inside cleanup's check/rename window is required.

Normal creation is staged off at the reviewed revision. These source modules are unchanged at pinned tip
`2ea2826caa0eec867c4e2bddc01badbac7bbe6c7`, where the global setting defaults on. A held cache lock can postpone
cleanup; it does not change the later current-identity rule. This differs from a ready folder becoming a cleanup-owned
claim and from replacement during an identity check/rename race.

Possible cover: pinned `SPEC_impl.md:2407–2418` grants namespace authority over strict private building/claim/trash
names, including interrupted creation without a per-tree journal. However, `SPEC.md:1064–1067` denies replaced
directories deletion authority, `3505–3514` retains ordinary replacement guards, and `3185–3187` preserves specific
foreign-replacement protections despite same-account trust. These texts do not unambiguously settle deletion after the
live builder already detected a replacement. The archive-window decisions at `TRIAGE_OUTCOMES.md:12770–12802` require a
different trigger. No exact Planned, BUGS, queue or FILTER coverage was found. An independent auditor could not approve
exclusion. Proposed drop: none; reconcile the contract before choosing broader cleanup changes.

Provenance: trailing-review run `30f46c42-a021-475f-96a8-eac06930471b`, `g015/f4`, independently retained by `g053/f1`.
