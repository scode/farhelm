# Narrow-sidebar tests measure the wider outer container

Reviewed commit: a03f32543bdaed2cbfea9cf33d43335611e071ff

## TLDR

Two browser regressions can accept session content extending beyond the narrow content area they intend to protect. They
measure the wider sidebar that also contains the host rail.

## Details

Suggested bucket: **other**, for test coverage. Confidence: **confirmed static assertion regression**. No current
product clipping or complete browser mutation run is claimed.

`e2e/tests/sidebar.spec.ts:3582–3597,3693–3700,3839–3841,3891–3893` moves each 280px fixture override to
`.sidebar-content` but leaves containment measurements on `.app-sidebar`, whose default total width remains 396px. The
content begins after the 56px rail and clips horizontal overflow. A 300px row can exceed the fixture's 280px column
while satisfying the outer width and endpoint checks. The other assertions check internal proportions, alignment or
different rows; they do not restore this containment boundary. The mismatch remains at pinned tip
`9b19f8d754bf7658761ef246cfda0ea8397f4f21`.

Measure `.sidebar-content` for containment and establish its actual fixture width before measuring rows. No source
premise is missing for the mismatched bound; runtime behavior was not tested.

Possible cover: none. Existing long-path-fixture and host-status-layout decisions describe other causes. Searches of
both specifications, Planned, unlanded plans, BUGS, queue and ledger found no exact accepted exception or pending
repair. No filter applies to this missing test discrimination. Proposed drop: none.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, `g018/g018-f1`, found in discovery pass 1.
