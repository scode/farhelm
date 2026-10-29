# Folder-dropdown refusal fixture enters version-mismatch mode

Reviewed commit: da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c

## TLDR

A browser regression checks recovery from an ordinary refused launch after its mock response has also put the page into
version-mismatch mode. The later interactions therefore run with an extra notice and live updates withdrawn. No
production bug, observed test failure or demonstrated false pass is claimed.

## Details

Suggested bucket: **other**. Confidence: **confirmed by source inspection**. The affected test was changed by launcher
redesign commit `09b806f7f08f436afacf0ac1f7cb867c8a7ef126`; the missing header is adjacent pre-existing fixture code.

In `e2e/tests/sidebar.spec.ts:6564–6567`, the folder-dropdown test fulfills the session POST with status 500 and a plain
refusal body, without `x-farhelm-build`. The shared request path calls `skew::note_build` before classifying the status
(`crates/farhelm-ui/src/api.rs:2149–2155`). An absent header becomes a latched silent-build mismatch
(`src/skew.rs:183–185,207–219`), displaying the mismatch notice (`src/lib.rs:1811–1815`) and withdrawing the feed
(`src/feed.rs:506–512`). `installComposerChoices` stamps its own catalog and history routes, not this POST.

The test continues after the refusal at `sidebar.spec.ts:6614–6639`: it checks that suggestions close, changes to
command mode, checks pointer stability across blur and submits again. Those assertions no longer exercise only an
ordinary refusal. The source establishes the wrong fixture premise; its effects on timing, geometry and regression
detection were not measured. No runtime reproduction ran.

Use the observed helm build stamp on the synthetic refusal and assert that the mismatch notice remains absent before the
later interactions. `SPEC_impl.md:5450–5454` requires every helm reply to carry that stamp. The decisions at
`TRIAGE_OUTCOMES.md:13917–13938` and `10291–10307` address the same defect class in other named tests; their scoped
repairs do not cover this sidebar fixture. No matching Planned item, unlanded plan, known bug or queue item covers it.
The folder-suggestion closing decision concerns the product behavior, not this additional fixture state. Proposed drop:
none. Possible cover: those older decisions provide precedent, not exact coverage.

Provenance: trailing-review run `2b7a8eaf-c4e9-42ab-be93-1acf29106132`, discovery finding `g11/f3`, checked against the
request classifier and exact earlier triage scopes by the parent.
