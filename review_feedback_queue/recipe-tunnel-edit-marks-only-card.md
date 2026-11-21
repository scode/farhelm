# Recipe tunnel edits mark the card without identifying the edited field

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A recipe with multiple tunnels marks only the card as edited, leaving the changed row and field unidentified.

## Details

SPEC.md:2450–2451 says a dot marks each edited field and section.
crates/gui/farhelm-gui/src/settings/recipes/editor.rs:1093–1159 draws independent sandbox port, host and destination
port text controls. Their renderer receives no baseline or edited state. The only tunnel marker is at :1229–1233, where
any array difference lights the card's heading. The code already behaved this way in
a518858b709e264d369c901fe89bb3186c056b6d^ at editor.rs:1175–1179. The card change carries the behavior forward, and the
pinned tip does not correct it. The new assertion in settings/recipe_editor_tests.rs:222–226 checks the footer's
form-wide marker after changing the name. Its tunnel assertions at :649–665 check adding and removing rows without
checking per-field markers. Use the existing field marker in each edited tunnel control and compare using the row's
stable key, so removing one row does not mark all later rows as edited.

Confidence: confirmed by inspection. The tunnel controls contain no field-level marker and the only edit comparison is
the heading's aggregate.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `recipeCards/l6`.
