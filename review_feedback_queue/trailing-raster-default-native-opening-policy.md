# Raster default and warning exemption are the dated product policy

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: original consequence preserved: native modifier-click can open agent-controlled raster files in
the user's default application, so an unauthorized expansion of that cross-host effect could be security-impacting.
Synthesis confirms only the narrow dated policy, not safety of arbitrary image bytes or handlers.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g052/c1`.

### g052/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (candidate, open):

Reviewed commit: 6d610d435dd12bbe8458b1a31b0988d95ff7338c. TLDR: original consequence preserved: native modifier-click
can open agent-controlled raster files in the user's default application, so an unauthorized expansion of that
cross-host effect could be security-impacting. Synthesis confirms only the narrow dated policy, not safety of arbitrary
image bytes or handlers. Details: DEFAULT adds exactly png, jpg, jpeg, gif, webp, bmp and avif; warning_kind marks those
exact extensions Safe and keeps svg under Html. The module's first paragraph explicitly says 'This is an allowlist of
final extensions, not a claim that a listed file is safe.' SPEC.md:4881-4883 explicitly amends the default: '(amended
2026-11-12 at the maintainer's request: the default also lists .png, .jpg, .jpeg, .gif, .webp, .bmp and .avif).' The
same Security passage says 'The click authorizes this transfer across the host boundary, including for agent-controlled
files.' Its settings-warning rule excludes 'the default's image types'. Source changes agree with those exact sets and
retain SVG's warning. Suggested bucket: highest if unsafe native opening were unauthorized; could-be-highest: yes.
Confidence: confirmed source/explicit contract acceptance only; no executable-image or handler-exploitation route
established or tested. proposed_drop: the exact dated default and warning policy accepts this specific extension
expansion; no broader security drop, and no independent drop accepted. possible_cover: none. Caveats: source-only, no
runtime/reproduction. SPEC_impl.md's older md/txt/json default wording is stale against the explicit dated SPEC.md
amendment and is not treated as a competing safety guarantee. No unresolved premise remains for this narrow policy
check.

Returned proposal (claims-033, event 10): **covered**, suggested bucket **other**.

The exact extension expansion and warning exemption are explicitly accepted. This does not establish that arbitrary
image bytes or native handlers are safe. No exploit or unconsented native-open route was established; the earlier
possible-highest consequence must still receive an independent drop check.

Original stronger consequence: An unauthorized expansion of agent-controlled files opened by native handlers across
hosts could be security-impacting (original possible highest). No such exploit was reproduced or established.

Unresolved premise or limitation: A concrete executable-image/handler exploit or a route opening without the specified
modifier-click was never established. The policy check has no unresolved extension-set premise.

Proposed basis: Drop only the assertion that adding these exact default extensions and exempting their settings warnings
is unauthorized. Do not generalize this acceptance to arbitrary image safety.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-accepted-raster-opening-defaults-and-warning-exemption. Parent reread033 in two
bounded complete outputs after the first clipped output earned no credit. Exact dated product amendment adds
png/jpg/jpeg/gif/webp/bmp/avif and grants modifier-click cross-host transfer, including agent-controlled files; source
adds exactly those formats and keeps SVG HTML warning. Proposed acceptance only of that extension/warning policy, not
arbitrary native image-handler safety or opening without the required gesture. No exploit or additional input route was
established; preserve the original security possibility for the fresh gate.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g052 record](records/review-through-00de0b6c/groups/g052.json). Record ID: `c1`.
