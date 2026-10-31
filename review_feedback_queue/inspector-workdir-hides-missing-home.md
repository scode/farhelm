# Inspector WORKDIR can hide a missing account home

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Image inspection can create the selected home path as its WORKDIR and then incorrectly decide that normal home
preparation is unnecessary.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The disposable inspector's implicit WORKDIR creation changes the image facts being inspected. If the image WORKDIR is
the missing home or its descendant, home_exists becomes true and preparation is skipped; the later keeper can use a
different checkout WORKDIR and still lack that prepared home. Matching Podman source confirms the conditional mismatch.
This differs from keep-id passwd synthesis and does not establish damage to existing host files.

Source locations: `crates/farhelm-supervisor/src/container/account.rs:252-255,224-232,344-361`. Historical revision
qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Defensive discovery after discovery3. Reviewed d6645f771bee8ed03a66486ef8755834937d759d; introduced
f73589c5201a0408f2f1555c18a4efca6f6057be. TLDR: a valid image whose missing account home is also its WORKDIR can be
marked ready with HOME still absent and no skel seeded. Preconditions are an existing image account at the host UID, its
absolute home absent in the image, image WORKDIR equal to that home or below it, no usable host-home owner rule, and a
checkout/cwd elsewhere. This does not require keep-id defaults, automatic passwd injection, a race, malformed passwd
data, or confidential content. Podman v4.9.3 generate/container_create.go508-515 automatically enables creation for the
inherited image WORKDIR. libpod/container_internal.go1027-1029 calls resolveWorkDir before the OCI process;
container_internal_common.go740-799 creates the missing directory and chowns it for the configured root inspector. Thus
INSPECT's -d check at account.rs252-255 observes an inspection-created directory, not the original image fact.
Choice224-232 sets prepare_home=false. The final keeper uses the same immutable image ID but overrides WORKDIR to the
actual checkout (container.rs376-377); it therefore does not create that different image home. INITIALIZE exits on
prepare_home=no at account.rs270, and container.rs627-629 confirms only running state before clearing the marker. No
HOME/skel readiness check catches the absent home. Valid image metadata can express a missing WORKDIR: retained Buildah
SetWorkDir source only assigns OCI/Docker config fields (vendor/github.com/containers/buildah/config.go355-361); no test
image, command payload or experiment was constructed. Missing HOME/skel is the established consequence, not a claimed
specific harness failure or existing-work overwrite/re-ownership. Suggested bucket other; confidence confirmed
conditional source route, no installed occurrence/frequency asserted. Coverage searches across both specs, TODO Planned,
BUGS, feedback index/files, TRIAGE_OUTCOMES and unlanded-plan index found no accepted missing-home exception or
duplicate. SPEC.md3602-3603 requires seeding a selected missing home; SPEC_impl.md5813 requires the same for existing
accounts. possible_cover: SPEC_impl.md5801 and the landed plan133-137 explicitly prescribe inspection without a workdir
override. The code follows that mechanism, but the resulting missing-home behavior conflicts with the product rule and
original-image-facts premise at5802-5804; no explicit exception accepts a home that exists only in the disposable
inspector. Broad deliberate-configuration intent does not clearly settle this phase mismatch. Proposed drop none;
surface this specification/mechanism tension at synthesis rather than treating it as covered. The pure unit test
account.rs496-509 injects home_exists directly, so it verifies decision preservation but cannot witness this
Podman-created-directory premise. Minimal correction direction: gather original-image home existence without letting
inherited WORKDIR create the paths being measured; preserve checkout exclusion and avoid changing later agent workdir
semantics. No new investigated uncertainty remains for this narrowed finding.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g125/f6`, `a050/f3`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
