# Warning repair does not cover implicit user-base deletion

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Warning repair does not cover implicit user-base deletion

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g059/l2`.

### g059/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b. Original TLDR/consequence preserved: shared recipe Delete
also removes owned container artifacts while the confirmation at this intermediate revision names only Tensorlake
snapshots, exceeding the stated scope; original potentially highest concern was loss of user-owned work beyond consent.
The same revision's recipe_editor.rs:585 says exactly 'Delete recipe {shown_name}? Any running build is cancelled and
the snapshots Farhelm built for it on Tensorlake are deleted.' Exact later repair remains
350a4b33b6d4476f4a78e43cea124bc2b828f462 recipe_editor.rs:584: 'Delete recipe {shown_name}? Running builds are cancelled
and Farhelm's images and snapshots for this recipe are deleted. Images in use must be released first.' That repair names
owned images across places and retains captured-target/rendered-token binding (:544-554,:873-883; recipes.rs:448-460).
Both UI files are unchanged through pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, as previously checked.
proposed_drop: preserve the exact later-repair proposal ONLY for the historical omitted-owned-container-artifact
warning. It neither authorizes deleting user bases nor covers the canonical highest pruning defect g061/l8 (alias
g058/c2), and no parent drop/priority decision is assumed accepted. Correction: retract every prior l2 statement that
target ownership labels, nonforcing removal, omission of bases from retired_images, or container-use checks prove custom
user bases are protected. Own app recheck at the reviewed revision: proto recipes.rs:788-798 admits a local image ID;
pull_base build.rs:808-839 reuses an existing reference without tagging/protecting it. retire_images_unlocked :689-750
checks only the explicit owned target, then executes image rm -- ID, omitting --no-prune; clean_images :781-788 calls it
during recipe deletion. Direct primary source verification this turn: Podman v4.9.3 cmd/podman/images/rm.go:55-70
defaults NoPrune false (https://raw.githubusercontent.com/containers/podman/v4.9.3/cmd/podman/images/rm.go). Vendored
common/libimage/image.go:185-193 defines dangling by absence of names/children, and :470-518 obtains the parent, deletes
the explicit image and recursively removes a now-dangling parent unless NoPrune; implicit parents never pass Farhelm's
label predicate
(https://raw.githubusercontent.com/containers/podman/v4.9.3/vendor/github.com/containers/common/libimage/image.go). This
confirms the broad safety premise was wrong. The supplied complete g061/l8 history and g058/c2 history additionally
establish ordinary committed ancestry through Podman/Buildah and libimage oci/layer_tree; claims-037 entries g058/c2 and
g059/l2 independently preserve the same separation. A valid local untagged user-authored base selected by ID can lose
its sole retained copy when its last unprotected owned descendants retire. Tagged/named bases, other image/container
references and read-only state can protect it; no forced removal of an in-use base, universal impact or observed loss is
claimed. A saved recipe reference is not an engine reference protecting that image. Confidence: source-confirmed
conditional route, with direct own app/default-pruning checks and named owner/parent ancestry verification; no runtime
observation. Suggested bucket: retain the canonical pruning route at highest for conditional user-work loss. The earlier
other recommendation applies only to the historical omitted-owned-artifact wording and must not lower this distinct
consequence. Could-be-highest: yes, original concern retained and supplied user-base route now established.
possible_cover: exact repair for historical wording; canonical g061/l8 / alias g058/c2 for the separate retained pruning
route, not new discovery or a new local ID. Contract: SPEC.md recipe Delete confirmation; SPEC_impl.md 'Answers bind to
what was drawn' and the explicit user-base preservation rule. The admission/lifetime proof in c3 remains narrow:
serializing/owning the operation does not constrain engine pruning semantics. Original proposed-drop bases remain in
history, with only their invalid broad implication retracted. Remaining own-source checks: explicit preservation text
and committed-child/Delete caller closure; no runtime tests, builds or reproductions. Closing synthesis checks: directly
re-read the complete SPEC_impl.md Shared recipes and Podman places sections at the reviewed commit, including :6527-6529
separating user bases from committed layers and :6566-6568 prohibiting user-base deletion. This contradicts pruning; it
is no accepted specification exception. Directly re-read build.rs:137-151: the admitted literal existing base creates
the build helper; :203-224 commits/publishes the owned image, retaining the base only as provenance.
tensorlake/recipes.rs:593-615 calls provider cleanup before the final store check, so the guard cannot prevent or undo
implicit parent loss. Also directly verified primary Podman v4.9.3 ABI images.go:558-568 passes NoPrune into libimage
(https://raw.githubusercontent.com/containers/podman/v4.9.3/pkg/domain/infra/abi/images.go), and libimage oci.go:60-94
plus layer_tree.go:263-340 recognize ancestry through matching layers and one added history entry
(https://raw.githubusercontent.com/containers/podman/v4.9.3/vendor/github.com/containers/common/libimage/oci.go ;
https://raw.githubusercontent.com/containers/podman/v4.9.3/vendor/github.com/containers/common/libimage/layer_tree.go).
The ordinary commit-history preservation is independently source-verified in the complete named g061/l8 and g058/c2
histories; no separate runtime evidence is claimed. All readily checkable named app/contract/default-prune premises for
this correction are resolved; no open source lead. Final scope: historical owned-image warning omission retains exact
later repair coverage; conditional user-base pruning remains canonical g061/l8 at highest with its stated
naming/container/other-image/sole-copy caveats, proposed_drop false for that consequence. The independent check still
judges every original drop/consequence assessment. No new defect ID, no discovery pass, and no runtime/test/build/live
action this turn.

Returned proposal (claims-037, event 9): **covered**, suggested bucket **other**.

The historical warning mismatch is real at the runtime-introducing commit. A later commit in this same review range
explicitly adds images and every place to the warning, with unchanged text through review tip. This repairs omitted
owned-container-artifact wording only. The record's broader claim that custom bases are protected is invalidated by the
separate retained pruning route.

Original stronger consequence: Deletion could destroy resources beyond the user's stated consent, potentially user-owned
work. The intermediate confirmed consequence was omitted regenerable container artifacts; later wording repair was
proposed, not accepted. Its narrowed consequence assessment and every earlier caveat require independent judgment.

Unresolved premise or limitation: No ongoing omitted-owned-images wording gap remains at tip. The repaired text does not
authorize deleting a user-owned base; the independent check must not treat the wording repair as coverage for pruning or
rely on the latest record's disproved broad protection rationale.

Proposed basis: Propose repair coverage only for the omitted-container-artifact warning, retaining the historical
finding and the separate highest pruning route. Reject any broader no-user-base-loss basis.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: later-source-repaired-owned-image-warning-only-with-pruning-correction-pending. Full037
real566 warning names only Tensorlake snapshots while cleanup now includes owned container artifacts;350 explicitly
warns about Farhelm images/snapshots across places and late-finishing builds, unchanged to tip. Propose historical
repair only for that omitted owned-artifact wording. The line9 broad claim that labels/nonforcing removal protect custom
bases is false under retained058c2/061l8 default pruning. Original059 explicitly reopened turn4 with named evidence to
revise its own log; no warning-repair dismissal can erase user-base loss. Preserve original wider consent/work
possibility and gate the narrowed historical consequence.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g059 record](records/review-through-00de0b6c/groups/g059.json). Record ID: `l2`.
