# Resume navigation check ends before reply settlement

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

The Resume regression can miss a completion that wrongly switches back to the original session: the test starts that
navigation itself before proving Resume finished processing.

## Details

Suggested and recorder bucket: **other**. Confidence: **verified missing test witness**. No production navigation bug or
actual false pass was observed. Reviewed change: `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7`; the test remains at
`e2e/tests/tensorlake-sandbox-sessions.spec.ts:481-490` at the reviewed tip (originally `385-394`).

The test releases Resume, observes the fixture call and the source row losing stale state, asserts B selected, then
clicks A before waiting for A's controls to enable. The fixture publishes its call/suspended state independently of page
reply processing (`56-64`, listing rewrite `44-54`). Independent feed/list refreshes can therefore satisfy the early
checks. The persistent operation owner awaits execution and only then clears pending/its claim
(`crates/farhelm-ui/src/list/view.rs:3049-3065` at the introducing revision). The enabled-control checks require that
later boundary, but happen after the test changed the selection it was supposed to protect.

Keep B selected through a positive reply-settlement witness, then assert it remains selected. A's row controls can be
checked without selecting A. SPEC.md:1840-1849 and SPEC_impl.md:1147-1158 require completion to preserve the user's
other selection. Existing terminal-host Info reply-correlation and Resume preview-readiness items concern different
tests and phases. No exact acceptance, Planned item, unlanded plan, BUGS, queue, ledger or filter cover was found.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g015/l2`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Source establishes the missing causal witness, not the frequency of a bad
schedule. Proposed drop: none.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): tensorlake-sandbox-sessions.spec.ts:590-599
retains the old :481-490 boundary: release Resume, observe fixture calls and the nonstale source, assert B selected,
then explicitly select A before checking Suspend enabled. Added fixture support does not turn fixture publication into a
client-settlement witness. The missing causal witness remains source-confirmed; false-pass scheduling was not measured.
