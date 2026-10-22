# Conformance test never admits its management candidates

Reviewed commit: 1876b3bdc4196529538a3420ba084ae5dd4ae16d

## TLDR

The generic responsiveness test can pass without proving that typing remains responsive after a management operation is
admitted. Its candidates are refused as busy before reaching the waits it intends to exercise.

## Details

Suggested bucket: **other**, for a regression-test gap. Confidence: **confirmed source mechanism**. No production
responsiveness failure is established.

`crates/farhelm-supervisor/src/service/handlers.rs:9545` requires zero management permits; `9575` creates that saturated
fixture, and `9848–9875` sends the candidate and observes progress before any gates are released. The admitted-task,
owned-mutation and rename-handoff representatives busy-refuse before entering their shared waits (`2170–2172,2234–2243`;
`service/connection.rs:1144–1150`). Slot release at `9391` is reached only in cleanup at `9425`. The later generic
observation at `10396–10423` remains saturated too.

An additional admitted observation should retain the independent wait gates and prove the candidate actually reached its
intended wait before measuring typing. Do not replace deliberately unsettled boundaries with general readiness.

Partial mitigation: `f3097ca1af8c4d61278d7e3e1e95b94717d14f90` separately tests admitted creation's real intent and
directory waits at `handlers.rs:4916–5057`, and waiter cancellation at `5124–5196`. The finding is about generic
conformance coverage, not an absence of all admitted-create tests.

Possible cover: none found. The dedicated tests cover only part of the generic representatives, and no filter fully
matches the gap. Proposed drop: none.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g052/g052-f1`.
