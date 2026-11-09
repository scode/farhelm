# Bounded geometry traces are also used as current readiness state

Reviewed commit: b2545c81d0728993a1b4b6ee2547de5aa5e434f9

## TLDR

The session-switch test can keep waiting on frozen historical geometry after a trace fills, even if the terminal later
paints correctly. This is a conditional source-level test defect; no actual failing run was observed.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed retention/readiness coupling**, with runtime occurrence
conditional on unmeasured timing and geometry.

`e2e/tests/terminal-switch-geometry.spec.ts:67-83` samples animation frames but stops recording geometry at 200 entries.
`paintedAtFit` at `123-129` keeps comparing the frozen last entry with the current fitted proposal. The history is
cleared at `113` and `158`, but remains across the setup waits and intermediate resize actions (`145-146`). If it fills
before a size-changing paint, later correct geometry cannot update that readiness witness.

The sizing trace has the same underlying mistake: `47-59` caps all fit/receive/flush events at 200, while
`resizeAndPaint` at `137-141` requires its recorded fit count to increase. Once full, a real later fit cannot satisfy
that count assertion, regardless of whether two pixels change columns. These are two consumers of one bounded-history
versus current-state mistake, not two queue items.

A possible healthy-run sequence is old intact geometry continuing to be sampled while a host resize/roundtrip is
delayed, followed by the correct new paint after the trace has filled. Source does not establish actual cap exhaustion,
refresh rate, host delay or exact changed column count. The two later width changes are only two pixels and need not
change columns; the earlier claim that filling before either necessarily causes a timeout was too strong. At 60
qualifying frames per second the cap would take about 3.3 seconds; that is arithmetic, not an observed rate.

Keep current readiness state independent of diagnostic retention. A ring buffer must also preserve any evidence needed
to detect transient bad return frames; that separate content-filtering blind spot is recorded independently.
`SPEC_impl.md`, "The grid client", lines 1848-1850 and the test-authoring checklist require distinct fit/handoff
evidence and readiness at the intended boundary, alongside bounded retention.

Possible coverage: no exact existing queue, ledger, accepted rule, known bug or unlanded plan covers this defect;
Planned was empty. Standard test execution and delayed work can reach the cap without a specific crash or unusual client
payload; no full rare-glitch filter match was established. This is not a latent-flake report.

Provenance: run `ee465fa5-0b29-4ce7-b1fe-8fabdb679cfe`, `fit/f4`; all stronger earlier claims and their corrections are
retained. Source-only review, no runtime test or timing experiment.
