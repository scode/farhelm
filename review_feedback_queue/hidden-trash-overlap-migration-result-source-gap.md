# Hidden-trash overlap handling has an unresolved result contract

Reviewed commit: a48bf50d94c87765d0b7cbc327bc1df5e275a302

## TLDR

The planned trash migration must skip active checkouts without making existing archives disappear from tracking. No
prescribed or implemented skip result establishing the suspected loss was found.

## Details

`plans/queue/hidden-trash-folder.md:49–50,136–145` requires skipping migration around an active checkout. Existing
`crates/farhelm-supervisor/src/checkout_trash.rs:110–124,324–325,348–353` prunes a missing selected path but preserves
its record on lookup error. A future helper returning successful skip followed by new-name-only lookup could lose
tracking, but the plan supplies no such helper or prescribed result. Returning an error preserves records. Tests at
`working_copies.rs:3565–3602` model older archives inside active checkouts; they do not establish the proposed new
behavior.

Suggested bucket: `other`. Confidence: incomplete planning/source lead, not a current product regression. Possible
cover: trailing-tip `SPEC_impl.md:2672–2677` requires uncertain failures to retain records. Whether that contract
sufficiently settles the missing helper behavior remains for triage; no work-loss route is established. Provenance:
`a008/f6`. No implementation or runtime pass is inferred from the plan.
