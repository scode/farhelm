# Container lifecycle wording bypasses provider-aware UI text

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Container lifecycle wording bypasses provider-aware UI text

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g031/f1`.

### g031/f1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 30 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462; later assigned 1d24c6af0b6fdbbec854623ff53f90281b8271ab and tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db checked. Original impact retained: inline Hosts progress calls a Podman start Resuming and says asked Tensorlake; Podman pause/resume also name Tensorlake. Expanded detail correctly attributes the request to the container host but still calls a start resume. Production source: environment/podman.rs:673-684 publishes starting = verb == start and Resuming on start; hosts.rs:4758-4762 drops starting/provider and invokes TensorlakeChangingStatus:4151-4165, whose literals say lifecycle_label(direction) and asked Tensorlake. host_state_detail:1209-1225 correctly uses ProviderUi but drops starting via lifecycle_verb(direction). Additional local diagnostic site verified this discovery: Hosts Suspend confirmation at this revision:3528,3544 says 'Cancel: keep the sandbox running' and 'Suspend: pause this sandbox until it is resumed' for Podman too. These tooltip literals persist at tip:3608,3624. This augments the existing local wording issue under f1, not a new discovery ID or a wrong-provider mutation finding. The operation still selects the stored provider; no unauthorized dispatch is alleged. Whole-range attribution for the added site: base UI lib.rs:936-940 permits lifecycle controls only for Tensorlake, while 350a4b33 lib.rs:954-959 explicitly admits Podman. Thus existing sandbox-specific words become wrong on the new supported container path within this range, rather than being an independent pre-range bug. Contract: SPEC_impl.md Disposable environments places provider wording/marks/help in one exhaustive UI module; ProviderUi already offers correct per-provider wording. Suggested bucket other, incorrect operation/provider diagnostics. Confidence confirmed by source only; no runtime execution. proposed_drop false. possible_cover none for local paths; supplied g024/c4 is a related separate backend formatter/caller with its own owner, not merged or dropped here. Could-be-highest no established security/work-loss route. Refutation preserved: ordinary unavailable-card Starting headline/explanation IS correct via sandbox_sessions::unavailable_notice:126-135 and session_view:2022-2024 at the reviewed revision; do not report its main Starting text as wrong. Its inherited detail still has the same helper's resume wording. Later in-range source has no repair for inline/helper paths or the newly noted confirmation tooltips. All useful new-site source premises settled. No parent drop/lowering accepted; all original consequences and correction history preserved.

Returned proposal (claims-018, event 30): **retain-verified**, suggested bucket **other**.

The supported Podman path reaches local text that still assumes Tensorlake and does not distinguish start from resume. These are incorrect operation/provider diagnostics, not a wrong-provider mutation.

Original stronger consequence: Earlier history also alleged the ordinary unavailable card's main Starting text was wrong. The provider wrapper refutes that part; keep the correction explicit. No security or user-work-loss route was asserted.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the inline progress, expanded/inherited detail and Suspend-tooltip scope together. The supplied reference to a separate backend formatter is not an alias established by this batch.

Existing parent disposition (event 30): verified. Parent explicitly accepted named retain-verified proposal from claims-018 after inspecting it. The supported Podman path reaches local text that still assumes Tensorlake and does not distinguish start from resume. These are incorrect operation/provider diagnostics, not a wrong-provider mutation. Retain the inline progress, expanded/inherited detail and Suspend-tooltip scope together. The supplied reference to a separate backend formatter is not an alias established by this batch. Checked source/results: ["350a4b33b6d4476f4a78e43cea124bc2b828f462:crates/farhelm-helm/src/environment/podman.rs:636-684 sets starting from verb == start and uses Resuming on start.","350a4b33:crates/farhelm-ui/src/hosts.rs:4758-4762 drops starting/provider; TensorlakeChangingStatus:4151-4165 prints lifecycle_label(direction) and literal asked Tensorlake. host_state_detail:1209-1225 uses provider attribution but passes lifecycle_verb(direction), which yields resume.","350a4b33:hosts.rs:3528,3544 contains sandbox-specific Suspend tooltips. UI lib.rs:954-959 permits Podman lifecycle controls; exclusive base lib.rs:936-940 permitted only Tensorlake.","350a4b33:sandbox_sessions.rs:104-137 overrides the Starting cause/explanation correctly. 222bf625:session_view.rs:2020-2032 uses that wrapper. The original claim against the ordinary card's main Starting message is refuted; inherited detail still uses resume.","Tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db hosts.rs retains asked Tensorlake at 4251 and sandbox tooltips at 3608,3624. git diff 222bf625..00de0b6c for hosts.rs is empty. Source only; no runtime observation."] Current coverage/results: ["Current SPEC.md Container sessions:4806-4865 distinguishes Stopped/Starting and container lifecycle; it does not accept these text errors. SPEC_impl.md Disposable environments:5929-5983 requires per-provider wording in one exhaustive UI module.","TODO.md Planned:25-26 is empty. plans/queue/INDEX.md has no unlanded plan entries; podman-environments is listed landed in plans/REPORTS.md:21.","BUGS.md covers only a retained ssh-output install/update hang. INDEX.md and queue content searches for asked Tensorlake, start/resume, provider wording and host lifecycle found no exact item. Active TRIAGE_OUTCOMES.md has no matching diagnostic decision.","FILTER.md does not match: normal supported container lifecycle is sufficient; no rare trigger was established."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Earlier history also alleged the ordinary unavailable card's main Starting text was wrong. The provider wrapper refutes that part; keep the correction explicit. No security or user-work-loss route was asserted."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g031 record](records/review-through-00de0b6c/groups/g031.json). Record ID: `f1`.

