# A model catalog refresh can move the highlighted choice

Reviewed commit: `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`

## TLDR

When the model list loads, Enter can pick a different choice than the one you highlighted. The change affects the
launcher draft.

## Details

The model list keeps a positional highlight. Catalog publication resets the separate launcher search highlight and can
insert choices before the highlighted row in the model list. Enter then resolves the same position against the new list.
Keep the highlighted choice by identity or clear it when its meaning changes. Source:
`crates/gui/farhelm-gui/src/launcher/view.rs:476` and `crates/gui/farhelm-gui/src/launcher/view/model_field.rs:149`.

The positional behavior predates this range. Restoring older model rows in `456558a85` expanded the catalog changes that
can move the selection. Source-confirmed.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g039/g039-02.
