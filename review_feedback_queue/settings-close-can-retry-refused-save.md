# Save and Close can automatically retry a pending save that the helm refused

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Save and Close can miss one pending refusal and retry the rejected draft without a new decision.

## Details

A page footer save tracks only its page's forms (crates/gui/farhelm-gui/src/settings/editing.rs:342–370). An open recipe
editor can Save concurrently: its button only checks the recipe's own flags and its request uses the recipe key
(settings/recipes/editor.rs:523–528, :557–564, :731–753), independently of the preferences key used by a page saver such
as File types (settings/file_types.rs:223–229). With both requests pending, a close question lists both, but the
existing-batch branch installs only Batch.after and skips watch_close_question for the other pending save
(settings/editing.rs:576–589, :617–635). If the user chooses Save and the mounted recipe's request then refuses before
the page save settles, the recipe keeps the same draft and records its refusal (settings/recipes/editor.rs:565–566,
:581–597). Its unobserved false receipt cannot stop the page batch. When that page completes, AfterBatch::SaveAndClose
starts a fresh close batch. It sees the now-dirty recipe and invokes Save again (settings/editing.rs:355–372, :438–459,
:471–512). If the retry succeeds, the normal close can hide the refusal. SPEC.md:2454–2457 and SPEC_impl.md:6459–6463
require observing pending saves and stopping on the first failure. SPEC.md:5495–5498 forbids automatic retry. Attach the
close decision to the receipts of all pending explicit saves present when it is made, and stop on any failure before
dispatching remaining work. This occurs with the same mounted target and unchanged draft, so target and version checks
(l1/l4) and retaining a departed editor's receipt (l10) do not fix it.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footer/l13`.
