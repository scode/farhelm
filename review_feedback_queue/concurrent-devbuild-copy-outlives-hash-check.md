# Concurrent devbuilds can replace bytes after verification

Reviewed commit: be29e118a353b20b8e40331b6abc0a815fb1bcdf

## TLDR

Overlapping direct builds in one checkout can copy tmux bytes replaced after their digest was checked. This
maintainer-tooling authenticity concern is conditional on an ambiguous concurrency contract; practical race control and
catastrophic harm are unproved.

## Details

Suggested bucket: **high**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `be29e118a353b20b8e40331b6abc0a815fb1bcdf:scripts/devbuild-mac.sh:232-242,558-623`. Locations and
revisions in the assessment below identify the reviewed source; later main is outside this review's pinned range.

New discovery candidate; preserve the initial f5 event. Reviewed commit be29e118a353b20b8e40331b6abc0a815fb1bcdf. TLDR:
if two direct devbuild-mac.sh builds overlap in one checkout, a second download can replace cached tmux after the first
build authenticated its digest but before it copies the executable into a runnable payload stage. Both can enter :570's
missing-file branch before A publishes the valid binary. B can pause before :600, then download different bytes into
.tmp and rename over the cached executable at :601 after A finishes :613 and before A executes :619. Both can use
genuine signed metadata. With --build-only --arch x86_64, B's later refusal/cache deletion need not prevent A from
copying, chmodding and writing STAGED at :623. The actual same-script writer is established; no hypothetical malicious
local writer is assumed. Guard :232-242 and its call :287-290 only check farhelm-desktop, not overlapping build scripts,
and no build-wide lock exists. Source inspection only: interleaving incidence and practical race controllability remain
unmeasured, and actual execution/remote harm is not asserted. Suggested bucket conditional high for maintainer-tooling
authenticity failure, not highest/catastrophic. Confidence likely static interleaving; supported direct-build
concurrency is an unresolved contract premise. Coverage search across pinned SPEC.md, SPEC_impl.md, TODO.md Planned,
BUGS.md, queue INDEX and TRIAGE_OUTCOMES.md found no exact direct-build concurrency/authentication acceptance or
duplicate. The helm payload-worker ledger at :15989-16035 concerns runtime/state-lock lifetime and different cache
writers, not these shell builds. Important partial possible_cover: reviewed scripts/devbuild-github.sh:21-31 explicitly
says two wrapper runs at the same time are unsupported; .agents/devbuild.md:12-27 sends the maintainer through that
wrapper. Root AGENTS.md:80 and .agents/machine.md:21 constrain agents to one checkout/workspace each. Do not treat those
as blanket acceptance for all direct operator invocations of devbuild-mac.sh (its own usage supports direct invocation
and does not state the concurrency exclusion). SPEC_impl.md:4866-4886's operator-directory trust answers the helm, not
whether concurrently downloaded bytes were authenticated by the staging script. proposed_drop: wrapper-mediated parallel
invocation portion may be excluded by the explicit wrapper contract, pending independent judgment; no approved drop for
the direct-build remainder, retain conditionally. No FILTER exclusion: a source-established unsigned-byte staging
consequence is not a safe retry-only failure. Serializing the stage or verifying a private immutable copy before
publishing would close this byte-binding gap. No tests, builds, product/verifier execution, downloads, network access or
reproduction performed. This does not expand the stale-CI published-payload audit assigned to a079.

Provenance: `a061/f5`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
