# The switch regression omits visible frames missing its sentinels

Reviewed commit: b2545c81d0728993a1b4b6ee2547de5aa5e434f9

## TLDR

The session-switch regression discards visible frames when their expected edge text or rule is absent. Its final
comparison can therefore miss that category of temporary visible failure. No current product rendering failure was
demonstrated.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed conditional test-oracle gap**, not a runtime false-pass
observation or a demonstrated product regression.

`e2e/tests/terminal-switch-geometry.spec.ts:67-82` records a frame only when row zero contains a span whose complete
text is `you`, row two contains a rule, and the terminal is visible. A visible frame failing either content predicate
produces no evidence of that frame or its missing sentinel. After the return switch, `158-166` waits for later good
paint and at least four retained samples, then compares only those samples. The sizing trace is attached but not
asserted.

If an early visible return frame has absent, relocated or split sentinel content and later frames return to baseline,
every selected sample can match while the earlier interval escapes the assertion. This does not need a full trace
buffer. The test's docstring (`23-26`) specifically claims transient visible-layout coverage after reveal;
`SPEC_impl.md`, "The grid client", lines 1848-1850 at this revision calls for separate fit/handoff checks. The test
authoring rule requires an observation that distinguishes the competing mechanisms.

The original review's proposed live-run-clipping proof was **refuted**: `grid-model.js:539-544` validates live rows
against committed columns and `547-563` replaces the screen on resize. `grid-terminal.js:774-787` clipping is not proof
of the claimed live route. The fixture redraws asynchronously on SIGWINCH (`11-19`), but its exact frame ordering was
not observed. Keep the finding limited to the selection/assertion blind spot.

Record missing sentinels in visible frames, or independently assert unexpected committed/fitted transitions. Name any
permitted partial-redraw interval explicitly so a stronger check does not forbid ordinary program painting. A current
readiness sample alone does not fix this content-conditioned evidence selection.

Possible coverage: no exact existing queue, ledger, accepted specification, known bug or unlanded plan covers the
oracle; Planned was empty. The separate capped-readiness item has a different cause and false-failure consequence. No
rare-glitch filter match was established for this regression-sensitivity gap.

Provenance: run `ee465fa5-0b29-4ce7-b1fe-8fabdb679cfe`, `fit/f1`. The original claim, refutation and narrower final
finding remain in its event history. No runtime test or actual flake was claimed.
