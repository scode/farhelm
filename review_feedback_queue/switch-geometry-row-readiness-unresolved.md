# Paint readiness omits asynchronously committed row dimensions

Reviewed commit: b2545c81d0728993a1b4b6ee2547de5aa5e434f9

## TLDR

The session-switch test might accept an old-height screen as ready and later compare a correct screen with that stale
baseline. The fixture's required row change and timing have not been established; this is an investigated uncertainty.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. No product defect, observed false failure,
security or user-work-loss route is claimed.

`e2e/tests/terminal-switch-geometry.spec.ts:123-129` obtains fitted columns/rows but checks only sampled columns, screen
width and `GEOMETRY READY <cols>`. The fixture reports columns only (`11-19`). `resizeAndPaint` (`137-142`) witnesses a
fit callback and that column-only predicate before the baseline read at `147`. Fitting proposes/sends a size
(`grid-terminal.js:486-512`); model painting is scheduled separately with requestAnimationFrame (`623-646`) and screen
height comes from committed rows. A fit callback alone does not witness new row commitment or paint.

The exact unresolved premises are important: this fixed fixture uses heights 898 and 900 (`145-146`), a two-pixel change
that need not cross a row boundary. A supported engine/font/pixel-ratio/effective-room combination must produce equal
columns but differing rows, and the baseline read must precede the later row paint. Source inspection of the complete
test, CSS height chain, measured-font formula and paint scheduling did not settle those run-specific metrics or event
ordering. No runtime geometry experiment was performed.

Evidence that would settle it is per-resize fitted and committed row counts plus fresh timestamped screen-height
samples, showing different row counts and a successful readiness/baseline read before the new paint. Without that
evidence, do not describe the conditional sequence as an actual failing run. The column-only witness is the established
source fact. `SPEC_impl.md`, "The grid client", lines 1840-1850 specifies deterministic fitting and separate browser
checks, but does not supply this fixture's missing metrics.

This is distinct from capped trace retention: it could occur with unfilled traces. The reviewer initially proposed
withdrawing the concern for lack of a proved fixture trigger. The commissioner retained it under the persistent-review
rule for concrete concerns that remain unverified after accessible investigation; the original proposal remains in the
drop register.

Possible coverage: no exact existing queue/ledger item, accepted rule, known bug or unlanded plan found; Planned was
empty. No filter clearly covers the conditional test-oracle issue. Preserve the uncertainty rather than inferring that
every height change or engine exhibits it.

Provenance: run `ee465fa5-0b29-4ce7-b1fe-8fabdb679cfe`, `fit/f7`, with investigation, proposed withdrawal and retained
revision under the same identity. No actual flake or runtime validation is claimed.
