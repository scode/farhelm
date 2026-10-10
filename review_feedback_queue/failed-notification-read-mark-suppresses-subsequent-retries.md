# A failed notification read-mark suppresses subsequent retries

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

One failed notification read mark prevents ordinary retries.

## Details

F212 — **definite** — `crates/farhelm-ui/src/list/view.rs:2915` — A failed notification read-mark suppresses subsequent
retries

The local queue retains an attempted read mark as its suppression boundary even when the request fails. Later
open-and-close cycles for the same sequence do not resend it, so the bell remains unread while entries appear read.
Separate pending optimistic marks from acknowledged marks and make failures retryable without overwriting newer
acknowledgements, restoring the promised next-close recovery.

## Evidence and triage context

- crates/farhelm-ui/src/list/view.rs:863 stores attempted marks in a component-lifetime map.
- crates/farhelm-ui/src/list/view.rs:2915 inserts through before the request; 2918–2925 only logs failure and never
  removes or rolls back that entry.
- crates/farhelm-ui/src/list/view.rs:2866–2870 raises the next opening's read_through to the attempted mark.
- crates/farhelm-ui/src/list/view.rs:2911–2912 then skips the write when the same notifications close again.
- SPEC.md:1808–1812 accepts silent read-mark failure on the premise that closing the list again repairs it.
- crates/farhelm-ui/src/list/view.rs:863: bell_read_sent is a component-lifetime map.
- crates/farhelm-ui/src/list/view.rs:2866–2870: reopening uses max(server read mark, cached sent mark).
- crates/farhelm-ui/src/list/view.rs:2911–2925: closing skips an already-covered sequence, inserts the attempted mark
  before awaiting the write, and only logs failure.
- crates/farhelm-ui/src/api.rs:2250–2266: mark_notifications sends the request and returns failure without maintaining a
  retry queue.
- crates/farhelm-ui/src/lib.rs:533–537: the bell's unread count still depends on the session's server-derived
  notifications_read_through.
- crates/farhelm-ui/src/list/row.rs:1691–1700: the row supplies that unread count and routes notification close actions
  to the list owner.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC.md:1810–1811 accepts a lost read mark, but only describes a bell remaining loud until the next close. It does not
  cover suppressing that retry.
- review_feedback_queue/FILTER.md:24–42 does not establish coverage for a state that survives ordinary refresh and
  retry.
- SPEC.md:1803–1812 accepts silent notification-read failures only with the stated next-close recovery.
- review_feedback_queue/FILTER.md:24–42 does not fully cover a cached state that survives reopening, listing refresh and
  reconnect and prevents the ordinary retry.
- TRIAGE_OUTCOMES.md:6204–6212 concerns cancellation between a server-side seen write and its event announcement, not
  this client-side notification retry latch.

Caveats:

- Requires a failure that did not commit the mark; a lost response after a successful write alone does not establish the
  defect.
- A reload, newer notification, successful clearing, or another client's mark can provide recovery.
- No runtime reproduction was performed.
- Remounting the list, successfully reading a newer notification, or clearing notifications can recover.
- If the server committed despite a lost response, its later read mark can mask the problem; the defect requires an
  actual unsuccessful mark.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_12_cor:p1:F3`, `ui_desktop_12_sec:p1:F1`.

- `ui_desktop_12_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
- `ui_desktop_12_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
