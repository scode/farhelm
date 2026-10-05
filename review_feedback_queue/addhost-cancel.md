# Cancelled Add host can still accept a queued setup answer

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Cancelled Add host can still accept a queued setup answer.

## Details

`F24 / COR-ADDHOST-CANCEL` — **definite** — `crates/farhelm-ui/src/hosts.rs:3721` — Cancelled Add host can still accept
a queued setup answer

Cancelling Add host closes the dialog in its parent, but does not immediately clear the child's retained setup offer.
Until the next render unmounts the child, a queued affirmative answer can still read that offer, validate the unchanged
SSH destination, claim the operation lock, and hand the setup request to the parent. The parent starts the request
without checking whether Add host is still open. Unlike the existing-host setup case, this path can perform the remote
install after cancellation.

Either affirmative answer can install the remote supervisor and its service. The permanent answer also saves the global
preference before the parent admits the submission, so it can change future setup behavior as well. Cancel should
synchronously retire both the offer and any automatic-submit authority, and the parent should validate that the
submission belongs to the live Add host opening. Cover Cancel followed by each affirmative answer in a controlled
queued-event fixture.

Suggested bucket: high

Possible cover: The earlier confirmation migration in `TRIAGE_OUTCOMES.md:2229–2270` covers other independently editable
dialogs; it does not establish that this Add host path was fixed.

Caveats: No browser reproduction was performed, and the size or frequency of the queued-event window was not measured.
This may warrant highest priority because it can write to a remote host after cancellation; the supplied high bucket and
conservative audit caveat are preserved.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **high**.

Originating reviewers and passes: `ui_lifecycle p1`, `ui_data p2`.

Possible cover recorded during collection: Earlier confirmation migration TRIAGE_OUTCOMES.md:2229–2270 concerns other
independently editable dialogs..

Collection caveats: No browser repro; eventburst window unmeasured. Potentially highest remote-write consent;
conservative audit rules apply.

## Filed reviewer metadata

- `ui_data p2`: confidence as filed: **definite / confirmed**. Premise: cancel and answer events can reach
  already-rendered handlers before rerender removes the dialog; `ops.rs:483–510`,
  `e2e/tests/terminal.spec.ts:3378–3435`, and TRIAGE_OUTCOMES.md:2229–2270 establish this event-burst premise. Suggested
  bucket as filed: **highest**.
- `ui_lifecycle p1`: confidence as filed: definite; confirmed by local offer and parent admission. Caveat: the
  event-burst exposure has not been runtime measured. Suggested bucket as filed: high; potentially highest if classified
  as a remote-write consent boundary.
