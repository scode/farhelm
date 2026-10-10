# Feed retained after browser sign-in interruption falsely represents an available GUI

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Lost browser storage could leave an invisible feed counted as an available GUI.

## Details

F299 — **possible** — `crates/farhelm-ui/src/feed.rs:391` — Feed retained after browser sign-in interruption falsely
represents an available GUI

Clearing browser credentials can make later requests show sign-in and unmount approval cards while the existing feed's
server credential remains valid. That subscription has no unmount cleanup, and automatic pongs may keep it counted as
GUI presence, leaving later requests waiting for unavailable cards. Exact socket survival is unverified. Release the
feed on unmount or sign-in interruption so availability reflects a mounted approval surface.

## Evidence and triage context

- auth.rs:364-371 rereads browser storage for requests; api.rs:1235-1245 can therefore send an unauthenticated request
  after storage loss, and :1250-1265 raises the token prompt. lib.rs:1502-1529 then unmounts the feed and approval
  cards. feed.rs:391-471 installs a parked JavaScript subscription and releases it only on a recv error, with no unmount
  cleanup in :484-515. dioxus-web-0.7.10/src/document.rs:208-254 gives JavaScript ownership of the evaluator lifetime.
  helm/events.rs:227 closes revoked credentials, which storage removal does not itself revoke; :263-268 accepts
  automatic browser pongs. helm/approvals.rs:270-271 uses subscriber_count as GUI presence. Thus an orphaned socket may
  make requests wait for unavailable approval cards. SPEC.md:2225-2229 and TRIAGE_OUTCOMES.md:6313-6330 accept lost
  browser action reports, not false GUI availability for subsequent requests. Runtime survival of this exact socket
  remains unverified.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_07_cor:p1:C3`.

- `ui_desktop_07_cor:p1:C3`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed:
  not separately tagged in candidate list.
