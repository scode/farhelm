# Queued Settings saves can commit a later version of an approved draft

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A queued Settings save can commit later edits to an approved form, including changes that clear a saved alert token.

## Details

A Settings save batch checks which live form entities remain present, then samples their contents when each form reaches
the front (`crates/gui/farhelm-gui/src/settings/editing.rs:214–226,342–374,453–512`). It carries no version of the draft
the user authorized. This also affects the second field in a Checkout save and a recipe waiting behind another form. For
Notifications, a later server edit can replace an already-approved draft and clear its stored token under the normal
origin-change rule (`settings/ntfy.rs:115–141,481–490`, `SPEC.md:2554–2557`). Bind each queued write to the draft it was
authorized to save, and keep later edits available for a fresh action. Freezing which forms appear in Save and Close
addresses a separate entry and does not by itself fix this problem.

The independently reviewed upgrade Settings path reaches the same queued revision outcome for Checkout fields
(`settings/checkout_field.rs:425–473`). An already approved recipe shows the same problem with a later edit before its
own Save starts (`settings/recipes/editor.rs:505–529`).

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footer/l4`, `launcher/l6`, `footerTests/l2`,
`footerEditors/l7`.
