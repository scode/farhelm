# Late reporter preparation failure has conflicting capture rules

Reviewed commit: 081e2a348ddf3c1f9263a7c671675f8848e1f089

## TLDR

If a named restart passes its first reporter check but reporter preparation fails during launch, it can keep the
previous conversation as the later Restart target. Broad tracking-failure acceptance and the specific promise to clear
capture when preparation fails leave this case ambiguously covered.

## Details

Suggested bucket: **other** for the established source-contract conflict. Confidence: **confirmed conditional
mechanism**, without an established healthy ordinary producer. Late-only I/O occurrence was not measured. Consequential
wrong-conversation continuation could require **highest** treatment; no observed continuation or user-work loss is
claimed.

`crates/farhelm-supervisor/src/service/core.rs:11605–11629` decides capture retention from successful preflight;
`11713–11770` settles that choice transactionally. Launch independently prepares the reporter at `14617–14637` and
continues without it on failure. Publication retains the prior capture (`12725–12730`). The tests fail preparation
before both checks (`21052–21089`), so they do not establish differing-result coverage. The mechanism remains at review
tip `98bb15850f5f5ba1df6ed66873338261c45ff948`.

Healthy concurrent publication does not supply the suspected trigger: `src/files.rs:188–214,398–438,474–503` publishes
complete bytes without replacing an existing destination, and `pi_extension.rs:89–114` verifies existing bytes. State
ownership prevents a second ordinary serving supervisor. Deliberate reporter modification belongs to the separately
accepted same-account interference case, not an uncovered healthy-concurrency defect.

Possible cover: tip `SPEC.md:2233–2240` accepts continued resumption of an already-recorded conversation after tracking
stops; `TRIAGE_OUTCOMES.md:13200–13213` applies it to transcript-read I/O failure. `SPEC.md:3187–3202` accepts
host-local filesystem failures. However, the later preparation-specific rule at `SPEC.md:2277–2282` requires clearing
when the extension cannot be prepared, and ledger `18896–18917` does not limit that requirement to the first check.
`SPEC_impl.md:5611–5619` describes both checks without expressly accepting differing results. No exact Planned, BUGS or
existing queue coverage settles that conflict. Proposed drop: none for this bounded ambiguity; the separate healthy
concurrent-publication allegation is source-refuted. Resolve the intended contract before adding recovery machinery.

Provenance: trailing-review run `47edc805-f43c-4ef2-abb1-0bba27a41c2f/review-98bb1585`, canonical `g046/f1`, with
duplicate evidence from `g045/f1` and `g007` prose. Independent auditor `a011/f1` agreed to the healthy-producer
exclusion but found whole-finding late-I/O exclusion ambiguous. `a004` separately verified the preflight repair and
fixed-harness admission; those verdicts do not settle this later failure.
