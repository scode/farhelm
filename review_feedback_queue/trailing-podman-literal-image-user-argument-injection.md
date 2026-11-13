# Literal input boundary refutation persists through the review tip

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Literal input boundary refutation persists through the review tip

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g059/l1`.

### g059/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (candidate, open):

Reviewed commit: 56040bb964270bb40d191902173dbb84b3ba78fc, with runtime callers introduced by 56659839785691f76da3af5d8f51b511cff4ba8b and checked through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. Original TLDR/consequence preserved: accepting leading '-' or shell syntax in a saved image reference/account could permit option or command injection on the container host. Could-be-highest: yes for that original security consequence; independent judgment of the proposed refutation remains required. Confidence: confirmed source refutation of the literal input route, not execution. proposed_drop: '--' precedes image data in image-exists (environment/podman.rs:206-215), pull/inspect (environment/podman/build.rs:808-839), and run (environment/podman.rs:421-453); account lookup shell-quotes the requested account after 'getent passwd --' (build.rs:59-73). SystemPlaceCommands::process preserves positional argv through the common route (podman_probe.rs:142-158). New later-range check: git diff from the runtime-introducing commit to the review tip shows no changes in build.rs; podman.rs changes only the lifecycle provision-lock helper at :646; podman_probe.rs changes comments only. Tip source still has the quoted getent and pull_base boundaries. Hence absence of the executor in the earlier model-only commit is not the basis for rejecting the whole-range concern: the implemented runtime consumers provide the substantive refutation, and these boundaries persist. possible_cover: corroborates g060/c4; the parent may relay this caller evidence to g058/l1. Contract: the catalog intentionally requires nonempty NUL-free literal fields and delegates reference/account syntax to Podman/image inspection. No specification-only exception was used. Suggested bucket: no active finding proposed, with no drop treated as accepted. Caveats: does not certify arbitrary image contents or every unrelated provider command; no runtime test or reproduction run.

Returned proposal (claims-037, event 8): **duplicate**, suggested bucket **other**.

Exact same literal image/account injection hypothesis as g058/l1. Both the reference terminators and shell/account lowering were checked directly through the executable consumer revision and later tip. Consolidation does not accept a security drop.

Original stronger consequence: Leading '-' or shell syntax in saved image/account inputs could alter commands on the container host. All initial and reopened potentially highest wording remains relevant to the independent check.

Unresolved premise or limitation: No literal-input injection step remains in the traced consumers. This does not certify arbitrary image contents or unrelated engine behavior.

Proposed basis: Consolidate with g058/l1 and source-refute only that common route, subject to independent drop agreement.

Existing parent disposition (event 8): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: same-root-literal-input-source-refutation. Full037 exactly duplicates058l1 image operand terminators, quoted account lookup and layered argv/SSH preservation at actual runtime566 through tip. Keep original foundation-only proposal and reopened later consumer checks, not a blanket absence-of-executor rationale. No arbitrary image content or engine behavior guarantee. Fresh gate must agree to both named histories and the narrow security refutation.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g059 record](records/review-through-00de0b6c/groups/g059.json). Record ID: `l1`.

