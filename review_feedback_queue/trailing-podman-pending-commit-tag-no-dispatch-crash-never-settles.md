# Interrupted image capture can permanently block recipe cleanup

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: a helm crash during an image build can leave the recipe permanently undeletable even
though the recorded image never existed, and block removal or retargeting of its container host. The decisive
no-dispatch window is after Podman::commit's awaited durable retired_images.push(tag) completes (podman.rs:505-511) and
before the following command future is polled/dispatched (:512-525); stopping the helm there destroys the stack frame,
so its later refusal/success settlement cannot run and no image can later appear from that invocation. Enumerated every
production retirement-tag settlement at the pinned tip: (1) commit's own definite refusal plus absent tag clears it
(:526-534); (2) successful commit parses an image ID then replaces the tag (:542-550); (3) retire_images_unlocked sees
an existing tagged image, verifies ownership, persists its canonical ID, checks all containers and removes without force
(build.rs:690-747); (4) absent canonical IDs can be cleared (:748-757). None settles an absent lookup tag after the
no-dispatch crash. Software/prepared publication removes canonical IDs, not earlier random tags (:219,:387). The sole
place-map removal is clean_images :788, reached only after tag cleanup succeeds (:785-787). Full searches for
retired_images, state.podman, whole-state reset and recipe state SQL found no other production reset: definition saves
and historical migrations change spec_json only; recipe deletion refuses needs_cleanup (tensorlake/recipes.rs:414-420).
Leftover deletion removes containers, not recipe image records (podman.rs:725-763). Host removal/retarget c

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g061/l4`.

### g061/l4

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b; settlement paths checked through pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: a helm crash during an image build can leave the recipe permanently
undeletable even though the recorded image never existed, and block removal or retargeting of its container host. The
decisive no-dispatch window is after Podman::commit's awaited durable retired_images.push(tag) completes
(podman.rs:505-511) and before the following command future is polled/dispatched (:512-525); stopping the helm there
destroys the stack frame, so its later refusal/success settlement cannot run and no image can later appear from that
invocation. Enumerated every production retirement-tag settlement at the pinned tip: (1) commit's own definite refusal
plus absent tag clears it (:526-534); (2) successful commit parses an image ID then replaces the tag (:542-550); (3)
retire_images_unlocked sees an existing tagged image, verifies ownership, persists its canonical ID, checks all
containers and removes without force (build.rs:690-747); (4) absent canonical IDs can be cleared (:748-757). None
settles an absent lookup tag after the no-dispatch crash. Software/prepared publication removes canonical IDs, not
earlier random tags (:219,:387). The sole place-map removal is clean_images :788, reached only after tag cleanup
succeeds (:785-787). Full searches for retired_images, state.podman, whole-state reset and recipe state SQL found no
other production reset: definition saves and historical migrations change spec_json only; recipe deletion refuses
needs_cleanup (tensorlake/recipes.rs:414-420). Leftover deletion removes containers, not recipe image records
(podman.rs:725-763). Host removal/retarget checks retained PodmanRecipeState::needs_cleanup in store/podman.rs:213-249.
git log/diff across the remainder of the range shows no later change to these paths; the only podman.rs follow-up
changes an unrelated lifecycle lock. SPEC_impl.md:6521 requires failed cleanup records retained and :6570 requires
retryable explicit cleanup; retaining ambiguity immediately is necessary, but this crash window has no recovery through
the product. Keep original high/material UX bucket and blocked-cleanup/host-administration consequence. Confidence:
confirmed source crash-window and enumerated production settlement analysis; no execution or reproduction.
proposed_drop: false. possible_cover: none. Could-be-highest: no established security/user-work-loss route. Fix caveat:
absence alone is NOT proof a remotely dispatched commit has stopped. Do not clear tags immediately or apply a guessed
age bound without establishing remote process completion. A durable remotely owned operation/outcome or another
certified completion mechanism is needed; the finding does not authorize unsafe deletion. Existing tests cover absence
followed by positive image appearance, not this never-dispatched case. No priority lowering.

Returned proposal (claims-039, event 14): **retain-verified**, suggested bucket **high**.

A crash after persisting the fresh lookup tag but before dispatch leaves an image that can never appear and a record no
ordinary cleanup path can clear. Recipe deletion and container-host removal/retargeting remain blocked.

Original stronger consequence: An interrupted capture can permanently block recipe deletion and its place host's
removal/retargeting. Conservative immediate retention is necessary; the defect is absence of eventual certified
settlement. Do not fix with immediate absence acceptance or an arbitrary age timeout.

Unresolved premise or limitation: None for the no-dispatch crash window. Other transport-failure routes need actual
remote outcome knowledge and are not necessary to this proof.

Proposed basis: Retain high for permanent blocked cleanup and host administration. Preserve the remote-ambiguity safety
constraint and source-only confidence.

Existing parent disposition (event 14): verified. Parent explicitly accepted named retain-verified proposal from
claims-039 after inspecting it. A crash after persisting the fresh lookup tag but before dispatch leaves an image that
can never appear and a record no ordinary cleanup path can clear. Recipe deletion and container-host removal/retargeting
remain blocked. Retain high for permanent blocked cleanup and host administration. Preserve the remote-ambiguity safety
constraint and source-only confidence. Checked source/results:
["56659839785691f76da3af5d8f51b511cff4ba8b:environment/podman.rs:505-525 awaits the persistent retired_images tag write
before constructing/polling command dispatch. A process crash in between loses the future's later settlement.","Same
revision: podman.rs:526-550 clears a definitely refused absent tag or replaces a successful tag with canonical ID.
build.rs:690-719 converts a tag only after positive image observation; 748-757 refuses absent non-ID tags.
clean_images:779-788 removes place state only after successful retirement.","At reviewed tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, git grep enumerates production retired_images mutations only in podman.rs,
podman/build.rs and the state definition. No alternate absent-tag settlement appears in that complete named-field writer
set.","5665983:tensorlake/recipes.rs:610-614 returns provider cleanup failure before recipe deletion; 414-420 refuses
residual cleanup state. store/podman.rs:213-249 refuses removing or retargeting a place with needs_cleanup recipe state.
Relevant build implementation remains unchanged to tip.","This is a source crash-window argument, not an executed crash
or transport-loss reproduction. Absence alone cannot safely settle a genuinely dispatched remote commit."] Current
coverage/results: ["SPEC.md:4870-4874 keeps place routes until cleanup; SPEC_impl.md:6039-6044 requires retained failure
records, and 6087-6091 promises retryable explicit cleanup. Neither accepts permanent blockage after a never-dispatched
capture.","TODO Planned empty; BUGS.md concerns provisioning pipes only; active TRIAGE_OUTCOMES.md has no
commit-tag/image-cleanup decision. podman-environments.md is already landed; its definite-create-refusal follow-up does
not settle never-dispatched image commits. No unlanded plan fixes the tag state machine.","Full INDEX.md and
image/capture/cleanup searches found no exact queue item. The older
interrupted-image-inspection-leaves-unnamed-allocation item concerns another inspector's container allocation, not
durable commit lookup tags.","FILTER.md does not cover durable administration blockage: no ordinary retry can clear it.
The hung-manager and session-history boundaries do not match."] Exact cited cover: [] Original stronger consequence and
missing premise remain preserved: "An interrupted capture can permanently block recipe deletion and its place host's
removal/retargeting. Conservative immediate retention is necessary; the defect is absence of eventual certified
settlement. Do not fix with immediate absence acceptance or an arbitrary age timeout."; "None for the no-dispatch crash
window. Other transport-failure routes need actual remote outcome knowledge and are not necessary to this proof.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g061 record](records/review-through-00de0b6c/groups/g061.json). Record ID: `l4`.
