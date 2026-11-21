# New model-family aliases can select an undisplayed model on Enter

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Enter can fill in an agent and model from a new family alias while the launcher still says models are loading.

## Details

Launcher Enter computes results from the live query and data at the point of dispatch
(`crates/gui/farhelm-gui/src/launcher/view/search.rs:195–198`). The newer model-family aliases can become selectable
after a catalog response even while the presented result remains Loading models
(`launcher/search.rs:530–561,737–740,798–806`, `launcher/view.rs:476–494`). The repository-loading change exposes the
same gap when a live result replaces a loading view (`88127fa6e`, `launcher/search.rs:847–854`,
`launcher/view/search.rs:400–406`). A recent setup can fill several fields, so the possible draft impact is wider than a
model-only change (`launcher/view/recents.rs:169–198`). The history-triggered route predates this range. Bind Enter to
the actually presented choice or loading state and keep live checks for invalidation (`SPEC.md:1145–1147,5500–5505`).

Confidence: confirmed by inspection of the live resolution path and the alias definitions.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `launcher/l9`, `templates/l5`, `templateSave/l8`.
