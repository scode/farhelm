# Templates can delete a different template than the one shown

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Delete can remove a different template from the one shown. The old button can also suppress Undo.

## Details

The Templates footer captures the Delete label and policy, then looks up the currently active editor when the handler
runs (`crates/gui/farhelm-gui/src/templates/editor.rs:1286–1305`,
`crates/gui/farhelm-gui/src/templates/dialog.rs:480–494`). Switching from a clean editor replaces that editor before the
next frame appears (`templates/dialog.rs:529–545,609–648,769–786`). These changes let an untouched legacy template leave
immediately, widening the set of editor switches that have no intervening confirmation. A handler from the displayed
frame can therefore combine one template’s Delete policy with another template’s name. That can remove the wrong saved
template and omit Undo. Bind the displayed name with its Delete kind and reject the callback if that editor has been
replaced. The same-name, last-write-wins exception in `SPEC.md:1283–1287,5487–5493` does not authorize a different named
target.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `templates/l3`, `templateSave/l4`, `recipeCards/l3`.
