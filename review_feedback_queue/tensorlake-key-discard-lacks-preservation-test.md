# The new key-form Discard lacks a test that preserves the saved Tensorlake key

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The new Tensorlake Discard coverage does not prove that abandoning a replacement keeps the saved key.

## Details

5c68c1c4386801657504080c65bb3b5eb13c1119 introduced a shared Discard with per-form dispatch
(crates/gui/farhelm-gui/src/settings/editing.rs:202–209, :282–321). Its key branch drops the typed value without
touching account state (settings/tensorlake_key.rs:410–415); Clear is the separate wire mutation in :453–495. Both can
be available while a saved key has a typed replacement. The changed credential test seeds exactly that state but only
presses Clear, then asserts the mutation and loss of the saved-key indicator (settings/credential_tests.rs:895–906,
:922–939). The shared Discard regression asserts no write for ntfy only (settings/tests.rs:3235–3255). The key tests'
shared account and disagreement paths cover Save, not Discard (settings/tensorlake_tests.rs:193–300). No key-form
Discard assertion was found in the changed Settings tests at introduction or tip. SPEC.md:2450–2453, :2490–2496 and
:2520–2525 distinguish per-form edits from the one stored key and make Clear the removal action. Add a key-form
regression assertion that Discard issues no mutation, restores the empty replacement field, leaves Test available for
the saved key, and preserves the other page's replacement draft.

Confidence: confirmed by static inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footerTests/d2`.
