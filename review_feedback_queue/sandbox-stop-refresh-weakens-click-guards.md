# Sandbox Stop can weaken its original no-tabs guard

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Stop and suspend can close a terminal tab opened during its activity check even though the initiating click showed no
tabs and no tab-closing question was shown.

## Details

Suggested bucket: **highest**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are
not runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/sandbox_sessions.rs:194-238`. Locations below use that revision unless another is named; the
assessment was checked through the reviewed tip.

Controls captures stop_asks=false for an initially tabless Stop, then awaits the independent activity read
(sandbox_sessions.rs:215-238). A normal session refresh can replace its execute callback while that wait remains
pending; execute recomputes no_tabs from the newer session (:194-207). An ordinary terminal tab opened by another client
can therefore make the sent no-tabs guard false, while idle/waiting/ended activity still skips the question. API and
helm forward that weakened guard, and the supervisor closes tabs when it is false.

The investigation established independent sidebar/activity requests, a stable mounted row/controller and the ordinary
tab producer. No runtime frequency is claimed. SPEC.md:1978-1984 explicitly protects a no-tabs click even when a prompt
is skipped; its multiple-GUI and pre-redraw allowances retain destructive preconditions. This is not the existing
combined numeric-tab question issue: no tab-closing question is drawn here. Preserve the original operation, session
identity and Stop guards across the wait. Suggested bucket highest; confirmed source route. g051-I1, g056-consent,
g057-controls-no-question and g058-s1 corroborate it.

The same unfrozen initial payload also chooses Suspend versus Resume from a refreshed callback before the activity wait
(:189,216-227,273-278). Preserve that distinct direction-reversal route within this item. The implementation binding
omission is confirmed; actual timing/platform manifestation is unmeasured. Independent consultation did not agree that
the product's as-delivered allowance accepts interpreting Suspend as Resume, so its original high consequence and
normative uncertainty remain; it is not silently lowered. A question already drawn captures its Operation and does not
merely invert at confirmation.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical
`g054/g054-stop-guards-after-activity`, exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings history, dispositions and independent consultations
preserve original confidence, suggested buckets and every revision. No fix or triage decision was executed.
