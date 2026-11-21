# The recipe Delete regression checks the prompt box without checking the warning text

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The recipe Delete regression checks that the prompt box fits without verifying that its warning text remains visible.

## Details

crates/gui/farhelm-gui/src/settings/recipe_leave_tests.rs:274–296 checks the warning through the prompt's accessible
name and compares only the left and right edges of the outer prompt box to the footer. The prompt's accessible name and
rendered warning are distinct outputs: crates/gui/farhelm-gui/src/prompt.rs:1238–1258 sets the outer aria_label and
renders the warning in a separate child. The Settings wrapper supplies the prompt box width independently at
settings/recipes/editor.rs:683–694. A layout regression in the rendered child can preserve the label and all existing
outer-box assertions. The test also uses the normal Settings fixture, while SPEC.md:541–545 covers very short windows.
Keep the footer-ownership assertion and add evidence that the warning's rendered lines and answer controls remain within
the visible footer at the minimum supported window size. This is a test coverage finding. Current warning clipping was
not established.

Confidence: confirmed by inspection of the assertion and render tree.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `recipeCards/f7`.
