# A stale permanent-delete button can remove a newly repaired template

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A stale Delete permanently button can remove a repaired revision of the same template without offering Undo. The
specification’s acceptance of that exact case is disputed.

## Details

The current Templates footer captures the displayed Delete kind, while the helm deletes by name without a revision
precondition (`crates/gui/farhelm-gui/src/templates/editor.rs:423–438,782–810,1286–1306`,
`crates/farhelm-helm/src/templates.rs:158–165`). Placeholder validation makes a stored legacy-invalid template eligible
for the permanent action. A later supported repair of that same name does not change the already drawn policy. The
action can remove the repaired revision and omit Undo. `SPEC.md:1283–1287` explicitly binds the Undo policy to the drawn
label, and `SPEC.md:5487–5493` accepts last-write-wins for template saves and recognizes that Undo can restore a
different revision. The independent check found that these clauses do not expressly establish whether stale permanent
deletion of a repaired same-name revision is authorized under `SPEC.md:5500–5513`. Carry a revision precondition into
permanent Delete, or clarify the intended exception before changing behavior. This is separate from an action reaching a
different named template.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `launcher/l3`, `templateSave/l11`.
