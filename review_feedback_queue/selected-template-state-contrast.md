# Selected template has a weak persistent state indicator

Reviewed commit: 3bc9f87d59ba9b9c44ad2987e3aa16bbb365a852

## TLDR

In Dark appearance, the selected template's fill is difficult to distinguish from its surroundings. No separate
persistent mark identifies the template being edited.

## Details

Suggested bucket: **other**. Confidence: **confirmed source accessibility deficiency**; practical visual severity was
not measured. No wrong launch or loss of user work is established.

`crates/farhelm-ui/assets/app.css:173,239,2999–3008,3101–3104` gives the selected row and its matching border 2.76:1
contrast against the dialog; selected hover is 2.43:1. `src/list/templates.rs:2233–2236,2248–2261` supplies no
selected-only checkmark, wording, weight or geometry. Readable labels identify the controls, but do not distinguish
their selected state. Focus is temporary, the bolt marks an independent quick-launch preference, and the editor name is
editable draft text.

The [W3C explanation of non-text contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)
distinguishes control presence from the visual information identifying its state. This supports the accessibility
concern; it does not establish a legal obligation or a new Farhelm conformance promise. Preserve the approved solid
selection treatment; a stripe is not required.

Still present at review tip `98bb15850f5f5ba1df6ed66873338261c45ff948`: CSS `3087–3096,3175–3201,8474–8511`, template
component `2299–2327`. Light appearance is outside this particular deficit.

Possible cover: that tip's `SPEC_impl.md:498–502` and `plans/queue/steel-foundation.md:48–61,219–223` approve the
palette and solid fill, without explicitly accepting insufficient state contrast. That ambiguity remains for triage. No
exact Planned, BUGS, queue, ledger or FILTER coverage was found. Proposed drop: none.

Provenance: trailing-review run `47edc805-f43c-4ef2-abb1-0bba27a41c2f/review-98bb1585`, `g022/f1`; later synthesis
checked the external criterion and separate discovery found no additional issue.
