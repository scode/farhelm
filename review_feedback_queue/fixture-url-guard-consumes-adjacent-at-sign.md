# Rust fixture guard mistakes adjacent prose for URL userinfo

Reviewed commit: 8f2efb5fa5f4111f9855c13f63690e5ec83588fe

## TLDR

The independent fixture privacy check can miss a local proxy URL followed by comma-separated text containing an at sign.
Automatic capture scrubs this exact input, but manually authored fixtures can pass the weakened guard.

## Details

Suggested bucket: **other**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

The queued private-IPv6 guard omission is a different recognizer case. The ordinary-userinfo ledger fix does not
explicitly accept consuming neighboring prose as authority. Independent consultation agrees that only the
automatic-capture leak claim is refuted, retains this guard remainder, and agrees OTHER because no actual publication or
catastrophic effect is established.

## Recorded source assessment

Reviewed commit 8f2efb5fa5f4111f9855c13f63690e5ec83588fe. Reopened after independent check: the previous record refuted
automatic capture leakage but incorrectly treated that as disposing of the independent guard defect. Exact remainder:
contains_local_url at :217-221 takes http://localhost:8080,user@example.com through the comma and applies rsplit('@'),
leaving example.com; :225 never sees localhost. The old host scan stopped at comma, so the assigned patch introduces
this false negative. screen_fixtures_carry_no_personal_data at :299 exempts user@example.com from email detection and
:306 passes the original text to the URL guard, so this precise text passes both privacy assertions. TLDR: a manually
authored or manually captured fixture can retain a local proxy URL without the independent privacy test flagging it. The
fixture contract includes manual capture (module docs :1-3) and expressly permits hand-made derived screens
(docs/agent-screen-fixtures.md:38-42); module docs :21-23 and screen_fixtures.rs:288-294 promise an independent
identifying-data backstop. Python does scrub this exact URL at capture-agent-screens.py:100-102,150, so no automatic
capture bypass or actual publication is claimed. Suggested bucket other: concrete regression confined to a maintainer
test guard, without catastrophic consequence. Confidence confirmed by source for this input; actual occurrence in
hand-authored fixture content is not established or required to demonstrate the detector contract failure. No fixture
was written and no runtime was run. Fix must recognize punctuation separating displayed URLs from adjacent prose without
breaking legitimate userinfo punctuation; add the exact guard regression case. proposed_drop: false for this narrow
independent-guard remainder; prior automatic-capture-route refutation remains valid. possible_cover: none searched.
Parent reports independent agreement on this narrow detector behavior. Preserve all earlier c3 records.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g036/c3`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
