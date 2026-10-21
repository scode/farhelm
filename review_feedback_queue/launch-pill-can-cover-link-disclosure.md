# Launch status may cover a link's destination at activation

Reviewed commit: 7dffe9b5c3b7bbde26889db0f2a38b5214536202

## TLDR

A pending launch sequence may hide a terminal link's real destination while the link opens, if its status pill remains
drawn through mouse release. Actual overlap and cancellation/render timing remain unverified. No exploit was reproduced.

## Details

At the reviewed commit, `crates/farhelm-ui/assets/app.css:668–685` centers and enlarges the pointer-transparent status
pill at layer 50; the destination disclosure uses layer 40 (`1328–1341`). The shared admission code supplies only
approval-card and tunnel-notice bounds as covers (`terminal.js:3618–3627`). Web/tunnel and file activation use that list
(`terminal.js:3659–3684`, `terminal-files.js:369–379`), leaving the pill unmeasured.

There is substantial counterevidence to an ordinary-click failure: pointerdown capture cancels the sequence before
mouseup (`template-shortcuts.js:171–175,255–262`). Rust then clears the sequence signal and the indicator's next render
omits the pill. A timely update removes the cover before admission. These callbacks establish cancellation dispatch and
state mutation, not a render/publication acknowledgment before release. The independent audit also notes that Enter
disarms the JavaScript owner before the Rust relay has necessarily removed an already published pill; a subsequent
pointerdown can then find nothing armed to cancel. That is another conditional interval, not a demonstrated exploit.

Later discovery found a distinct ordering that avoids that cancellation race: press the link first, start a sequence
while holding the mouse, wait for its pill to render, then release. The gesture tracker has no timeout or keyboard
reset; prefix input is consumed, and cancellation listens to pointerdown/click rather than mouseup. Pointerdown preceded
the sequence, while click cancellation can follow mouseup activation (`terminal-links.js:143–170`,
`template-shortcuts.js:226–235,255–262`, final pin). This supports another trigger for the same omitted-cover predicate,
not a second finding. Actual pill/disclosure overlap, retained xterm activation and native event behavior remain
unverified.

Establish the actual overlap and event/DOM publication schedule before designing a wider change. If reachable, counting
the pill's live bounds or refusing the gesture that dismisses it are possible remedies. A DOM already uncovered but
awaiting paint is a different stale-paint question and does not prove this missing-cover predicate caused activation.

Suggested bucket: provisionally **highest**, for the possible destination-disclosure safeguard bypass. Confidence:
**likely only conditional on actual overlap and pill persistence through release**. The omitted selector and declared
layers are confirmed statically; ancestor stacking, native IPC ordering, paint, and consequential user reliance were not
exercised. Historical landing-report assertions are allegations, not authenticated runtime evidence.

Possible cover: `SPEC.md:1857–1860,1888–1893` at the final review tip requires an uncovered disclosure. The subsecond
allowance at `3241–3244` preserves explicit safeguards, so it does not clearly accept this case.
`SPEC_impl.md:2120–2137` describes two implemented covers without accepting all other occlusion. Ledger entries
`approval-cards-can-obscure-file-disclosure.md` and `tunnel-error-notice-obscures-link-disclosure.md` concern different
surfaces. No matching Planned, BUGS, queue or filter coverage was established. The detector omission predates the range,
but the in-range placement change can enlarge overlap; pre-existence does not establish accepted behavior.

Provenance: trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`; quick-launch contracts reviewers,
terminal-link reviewers, and a fresh independent sequence/disclosure auditor. Their duplicate reports describe one
conditional concern. The independent auditor's later-turn exclusion verdict is ambiguous, so the concern is retained. No
runtime test was performed.

Original finding IDs: `terminal-links-contracts-1/f2`, `terminal-links-contracts-2/f1`,
`terminal-links-implementation-1/f2`, `terminal-links-tests-1/f2`, `quick-launch-contracts-1/f1`,
`quick-launch-contracts-2/qlc2-f1`, `quick-launch-implementation-1/f2`, `quick-launch-implementation-2/f2`,
`quick-launch-tests-1/f2`, `audit-sequence-target-disclosure/audit-disclosure-1`.
