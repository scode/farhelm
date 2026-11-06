# Checkpoint image triage rootless path unresolved

Reviewed commit: d81a3980548e354bcf2ff8a06d28235722f15ba4

## TLDR

The prior checkpoint-image assessment's root-only exclusion does not establish what happens through the plain-run
inspection path. Actual rootless workload restoration remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `TRIAGE_OUTCOMES.md:21080 at d81a3980548e354bcf2ff8a06d28235722f15ba4`.

Source assessment (read with the later corrections below) (review record g027/g027-d2, event 32):

Reviewed commit d81a3980548e354bcf2ff8a06d28235722f15ba4. TLDR: the checkpoint-image assessment leaves a real dependency
path unverified and states a root-only exclusion whose applicability to that path is not established. Source now
establishes that Farhelm's image inspection can enter Podman's checkpoint restore dispatch; whether a supported rootless
runtime can actually resume the saved workload remains unverified after investigation. This is additional
record-accuracy evidence on the already known checkpoint-image-inspection-restores-unowned-workload issue, not a claimed
new implementation root defect, restored workload or security incident. Mechanism: container/account.rs:341-364 runs
podman run --rm --pull=never --network none --user 0:0 --entrypoint bash IMAGE. Exact Podman v4.9.3
pkg/domain/infra/abi/containers.go:1091-1125 detects a checkpoint-image annotation and calls ContainerRestore directly,
forwarding only Name/Pod into RestoreOptions; its :662-758 imports the checkpoint and calls Container.Restore.
cmd/podman/containers/restore.go:102-104 rejects rootless use in the separate restore CLI handler, which this
ContainerRun route does not call. libpod/container_api.go:1059-1074 calls c.restore, and
container_internal_common.go:1151-1160,1384-1418 checks CRIU/runtime support and imports a checkpoint without the CLI's
rootless refusal. Thus the plain-run dispatch premise is source-confirmed, and the distinct CLI check does not itself
settle the run path. Correction to the preceding event: the ledger does not identify the source basis for its root-only
assertion; this review found a CLI-only check but does not claim the triage author relied on that specific code. Actual
restored process execution is still missing: no exact selected host OCI runtime/CRIU combination or rootless restoration
capability has been established, and upstream CLI restriction may reflect a lower-level capability failure. The broad
supported environment does not pin one such combination; no runtime/provisioning experiment was permitted. Evidence that
would settle it: an exact supported runtime/CRIU source guarantee that rejects this route before workload execution, or
an authorized retained reproduction that restores a benign checkpoint via precisely Farhelm's inspection argv.
SPEC.md:3924 requires rootless Podman; :93 says user units, never root. SPEC_impl.md:7342-7424 specifies a short-lived
no-network/no-mount account-inspection container before the checkout bind, not resurrection of prior workload settings.
Suggested bucket other for this maintainer triage-evidence/qualification problem; do not promote to highest without
proving the rootless execution/capability boundary, and do not drop the underlying concern using this narrow CLI check.
Confidence likely with missing rootless-execution premise, explicitly unverified after investigation. proposed_drop:
null. possible_cover: exact assigned checkpoint-image ledger item, whose underlying implementation concern this extends;
parent owns matching and original finding ownership. No queue/plan/filter searches. Primary pinned sources:
https://github.com/containers/podman/blob/v4.9.3/pkg/domain/infra/abi/containers.go ;
https://github.com/containers/podman/blob/v4.9.3/cmd/podman/containers/restore.go ;
https://github.com/containers/podman/blob/v4.9.3/libpod/container_internal_common.go . No runtime, build, installation,
browser, process or network experiment; read-only primary-source retrieval only. Temporal caveat: ledger wording at d81
records what was then not shown; this evidence does not retroactively prove an observed restore. Temporal follow-up:
bounded git log from d81a3980548e354bcf2ff8a06d28235722f15ba4 through 8a8c9579e04299410652f0e4681975a7198dfc7b shows no
changes to container/account.rs or container.rs; the SPEC.md/SPEC_impl.md diff contains no checkpoint-image or
rootless-inspection policy change. Additional exact-version reads of cmd/podman/containers/run.go show ContainerRun
dispatch at 214 without the restore CLI refusal; pkg/specgen/generate/container.go contains CompleteSpec but no
checkpoint/rootless-specific refusal. These do not settle OCI-runtime or CRIU execution capability. Novelty
qualification: this is investigated-unverified additional evidence on the assigned known checkpoint-image concern, not a
separately established new root defect; count it as a duplicate/record extension unless the parent finds an
independently actionable assessment defect. No external coverage search performed.

Recording coverage: The exact underlying checkpoint-image issue has a recorded triage decision, but new plain-run
dispatch evidence materially challenges its root-only premise. Keep the new investigated uncertainty and identify prior
decision; do not imply restore succeeded.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g027/g027-d2`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
