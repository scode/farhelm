# Guarded answer removes focus without a handoff

Reviewed commit: f1d71445d629f8ee244a592b2173507e32b8c343

## TLDR

In an ordinary launcher, choosing Guarded removes the focused answer while waiting for a refreshed checkout preview,
without moving focus to a surviving control. A failed preview leaves the launcher open in that state.

## Details

Suggested bucket: **other**. Confidence: **confirmed focus-handoff omission**; browser fallback and the next Tab
destination were not exercised. No redirected agent input or user-work-loss consequence is established.

`crates/farhelm-ui/src/list/create_form.rs:6474–6493` takes the question and arms pending submission; `6446–6447` then
removes its controls. The pending effect at `6088–6131` supplies no waiting or failure focus handoff. The form's local
Tab trap (`1858–1890`) cannot catch Tab targeted at the document body. Provide a surviving focus target before removing
the focused answer, retaining it through the preview wait and refusal.

Exclude quick-origin launchers: their separate effect at `5068–5074` supplies composer focus. The checkout regression
checks preview and permission state but not waiting focus (`e2e/tests/github-checkout-composer.spec.ts:629–646`).
Automatic terminal focus remains vetoed while the modal exists (`assets/terminal.js:1118–1124`); an independent auditor
agreed that this refutes the proposed automatic typing-redirection mechanism, without certifying every keyboard route.

Possible cover: none established in the required coverage sources. Proposed drop: none for the focus omission.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g048/f1`, narrowed and independently checked by
`g131`.
