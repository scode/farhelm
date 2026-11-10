# The progress-hover test may dismiss its own entry

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

The browser test may scroll an ancestor after entering the hover target and dismiss the card it is waiting for. The
fixture geometry and native ordering remain unverified after investigation.

## Details

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. Source-confirmed interleavings are
not runtime observations.

Reviewed change: `4754d4e5f4edc8929ce77f79071fbe724d09f164`. Original source location:
`e2e/tests/provisioning.spec.ts:1098-1101`. Locations below use that revision unless another is named; the assessment
was checked through the reviewed tip.

The test can enter a progress hover and then ask Playwright to reveal a descendant. An ancestor scroll during that
reveal would dismiss the hover because the moved ancestor contains its original target. Scrolling the cloned card's own
list does not satisfy that dismissal predicate.

The exact cached Playwright 1.64.0 package was integrity-matched and its locator/actionability source inspected. It
already waits for stability before reveal, and the folded three-host fixture may need no ancestor scroll. CSS/source
checks locate the count in the card-bearing label within the scrolling sidebar but do not establish the required fixture
geometry or physical scroll/event ordering. Thus the earlier simple stability-race theory is not confirmed. A retained
supported-engine geometry/event trace would settle it; no browser test ran. Arrange target visibility before entering
the hover if needed. Suggested bucket other; unverified after investigation, with no observed test failure.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g047/g047-scroll-entry`, exclusive
base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private
findings history, dispositions and independent consultations preserve original confidence, suggested buckets and every
revision. No fix or triage decision was executed.
