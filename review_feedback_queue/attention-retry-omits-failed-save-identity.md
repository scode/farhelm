# Automatic-update Retry omits its failed-save identity

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

An old Try again activation could restore automatic updates after a newer opt-out, but no ordinary shipped ordering
outside accepted input races was established. This remains unverified after investigation.

## Details

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. Source-confirmed interleavings are
not runtime observations.

Reviewed change: `c74be1ab43dd97fbdc4287ea5e9b2a37729d87c4`. Original source location:
`crates/farhelm-ui/src/hosts/attention_list.rs:255`. Locations below use that revision unless another is named; the
assessment was checked through the reviewed tip.

Try again carries no identity for the failed save (attention_list.rs:250-255); its callback records a fresh true
preference (list/view.rs:1052-1059). Completion supersession in settings.rs:183-208 and api.rs:1464-1488 does not reject
a stale activation before recording that new choice. Fixed true and absence of repeated manual host requests refute
different-answer/target substitution, not every dispatch order.

Investigation checked matching Dioxus 0.7.10 current-element/listener dispatch, callback replacement, reclaimed/reused
element IDs, synchronous desktop normal-event XHR and runtime handling, generated native JS, and the absence of a
first-party Retry replay producer. Those results weaken simple cached-closure/delayed-IPC theories. No ordinary shipped
activation order was established in which an old Retry reaches the current opt-in callback after a newer false choice,
outside the limited pre-redraw/covered-dialog or multiple-GUI acceptance. Source inspection cannot establish every
native input/render timing; a concrete event/target/field-identity trace through an ordinary supported input route would
settle the premise. No event or payload was constructed.

The initial independent exclusion check was ambiguous, so the concern is retained. After the additional checks, its
second and final clarification agreed narrowly with other and unverified after investigation because no
unauthorized-update, security or loss route is established. This is not acceptance of arbitrary stale dispatch. The
older unknown-policy-write-overtaking item needs an already issued request delayed before the server lock; this concern
instead needs a stale activation starting a new write, a different cause and missing premise. Preserve all historical
potentially-highest wording and reconsider priority if the route is established.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g042/g042-retry-choice-binding`,
exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`.
Private findings history, dispositions and independent consultations preserve original confidence, suggested buckets and
every revision. No fix or triage decision was executed.
