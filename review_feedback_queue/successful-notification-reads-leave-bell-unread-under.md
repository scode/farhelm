# Successful notification reads leave the bell unread under build mismatch

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A successful notification read leaves the bell stale under build mismatch.

## Details

F213 — **definite** — `crates/farhelm-ui/src/list/view.rs:2918` — Successful notification reads leave the bell unread
under build mismatch

The server acknowledges the read boundary, but the client updates neither the bell's listing nor an explicit refresh.
Popup highlighting uses optimistic state while the bell keeps its old unread count when unattended refresh is disabled.
Apply the acknowledged boundary locally or explicitly refresh after success. This success-path reconciliation defect is
separate from failed-write retry suppression.

## Evidence and triage context

- crates/farhelm-ui/src/list/view.rs:2918–2926 handles only the error result and performs no success reconciliation.
- crates/farhelm-ui/src/api.rs:2250–2266 returns success without updating a client listing.
- crates/farhelm-ui/src/list/row.rs:1691–1697 passes session.unread_notifications() to the bell.
- crates/farhelm-ui/src/lib.rs:533–537 calculates that count from Session::notifications_read_through.
- crates/farhelm-ui/src/list/view.rs:2866–2870 uses bell_read_sent only for the opened list's read boundary.
- crates/farhelm-ui/src/feed.rs:233–244,300–313 withdraws both ordinary refresh sources under mismatch.
- SPEC.md:1143–1150 requires the bell to become grey when its notifications are read.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:3630–3634 withdraws unattended refresh, but preserves explicit interaction; it does not accept stale
  results for the user's notification interaction.

Caveats:

- Requires build mismatch, a successful read-mark, and no other explicit listing refresh.
- Server-side read state is correct.
- No runtime reproduction was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_12_cor:p1:F4`.

- `ui_desktop_12_cor:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.
