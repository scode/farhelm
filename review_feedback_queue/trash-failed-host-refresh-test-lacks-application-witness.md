# Trash failure regression does not witness the failed host's refresh

Reviewed commit: 7d6e00e861573660c1fb9f0661a31b37861ce61a

## TLDR

The multi-host trash-failure test can miss a regression that loses the failed host's remaining entries or warning when
its final refresh arrives.

## Details

`e2e/tests/trash-dialog.spec.ts:555–564` checks the retained entries and warning, then waits only for both listing
requests to arrive. The request counter is at `109–118`. The healthy host's disappearance witnesses its completed
refresh, while the failed host's independent reply can remain pending. Old state stays visible until that response
applies (`crates/farhelm-ui/src/list/trash.rs:576–599,993–998`). Assert preservation after an observable application
boundary for that host, rather than request arrival alone.

Suggested bucket: `other`. Confidence: confirmed test-oracle gap; no production regression or adverse response schedule
was observed. The earlier sequential-one-host allegation is withdrawn because its non-partial total supplies a loading
oracle (`trash.rs:929–938,956`). Possible cover: none exact found. Provenance: `g088/f1`, preserving its narrowing
history. Static inspection only; no tests ran.
