# Replace cleanup captures owned IDs before draining replies

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

If a Replace test fails while its real request is pending, cleanup can miss the replacement created while it drains that
request. The extra test session can remain until a later stack reset or shutdown.

## Details

Suggested and recorder bucket: **other**. Confidence: **verified conditional source path**, not a runtime observation.
Reviewed change: `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7`; affected source is unchanged at the reviewed tip.

In `e2e/tests/replace.spec.ts:633-638,699-704`, the array passed to `cleanupOwnedSteps` contains a route-draining step
followed by `...owned.map(...)`. That mapping runs before the first step. The real `route.fetch` callbacks at
`568-572,669-673` can still append a successful replacement ID during the drain, after its deletion closures were
constructed. A body assertion can fail before the reply-held witness, making this sequence reachable. The normal
successful path already records the ID and is unaffected.

`helpers/drained-route.ts:19-42` drains entered callbacks correctly. `helpers/cleanup.ts:6-14` executes the supplied
array and cannot recover a later-added ID. Route drain before cleanup execution therefore does not prove drain before
owned-ID enumeration. Defer enumeration until after drain, or finish drain before constructing deletion steps.

The terminal-suite afterEach drains routes without deleting sessions; a later reset lists/deletes leftovers and normal
stack shutdown stops its terminal hosts (`helpers/terminal-suite.ts:154-213`, `start-stack.sh:375-447`). No persistence
beyond those outer boundaries, user-install effect, security consequence or user-work loss is claimed. Existing catalog
cleanup concerns work initiated after observation ends; this is an already-entered callback and early array snapshot. No
exact specification acceptance, Planned item, pending repair, known bug, queue, ledger or filter cover was established.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g012/c2`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. The log preserves the original incorrect code-refutation and its reversal.
Actual failure/reply scheduling and incidence were not measured. Proposed drop: none.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): replace.spec.ts:633-638,699-704 still
evaluates ...owned.map when constructing cleanup steps, before the held-route drain runs and can add its replacement ID.
Later edits affect terminal-instance checks from line 860 onward. Catalog cleanup's pre-POST lifetime and
route-retirement rejection remain different causes. The conditional array-construction ordering is source-confirmed;
allocation and leak frequency were not measured.
