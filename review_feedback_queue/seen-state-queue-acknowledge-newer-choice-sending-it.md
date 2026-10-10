# Seen-state queue can acknowledge a newer choice without sending it

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The read-state queue can report success without sending the user's latest choice.

## Details

F197 — **definite** — `crates/farhelm-ui/src/api.rs:2413` — Seen-state queue can acknowledge a newer choice without
sending it

One client's first read mark can commit while its response remains pending, another client can mark unread, and the
first client can observe that change and mark read again. Equality with the first request's timestamp makes the queue
discard the newer choice, leaving unread stored despite reporting success. Track choices recorded after sending
independently of value equality and send the latest before completing.

## Evidence and triage context

- crates/farhelm-ui/src/api.rs:2359 overwrites the queued choice during an outstanding write; line 2413 considers value
  equality sufficient to finish.
- crates/farhelm-ui/src/api.rs:2508 forwards the older write result to the newer waiting callback without another
  request.
- crates/farhelm-ui/src/list/row.rs:991 derives another mark-read action from the currently displayed shared unread
  state.
- crates/farhelm-helm/src/sessions.rs:2594 commits the value and announces its change before sending the response.
- crates/farhelm-ui/src/api.rs:581 documents and handles the same equal-valued newer-choice distinction in the
  preference queue.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- Concurrent GUIs are best effort, but SPEC.md:2237 expressly calls for easy, low-complexity race fixes; the neighboring
  preference queue supplies a directly applicable mechanism. Neither existing seen-state ledger item covers this
  coalescing error. No proven sub-second user-action limit.

Caveats:

- Inspection only; no runtime reproduction.
- The reported trigger requires another client to change the seen state while the first client's response remains
  outstanding.
- No security or user-work-loss consequence is established.
- The outstanding response need not be confined to a sub-second interval, so the person-race filter is not established.
- TODO.md Planned, BUGS.md and the queue index contain no matching item.
- The demonstrated sequence requires another client.
- No data-loss or security consequence.
- No runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_04_cor:p1:F2`.

- `ui_desktop_04_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
