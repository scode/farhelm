# Restart with refusal text bypasses peer-text rendering

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Restart with displays refusals without the ordinary restart protection.

## Details

F224 — **definite** — `crates/farhelm-ui/src/restart_with.rs:650` — Restart with refusal text bypasses peer-text
rendering

The separate dialog renders refusal text raw, including remote-controlled values or a missing-directory path. Hidden or
directional characters can therefore make its explanation misleading, while ordinary Restart already applies peer-text
rendering. Use the same escaped, direction-isolated representation at this independent refusal sink; fixing the ordinary
restart surface does not cover it.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:10152: restart validates the stored working directory.
- crates/farhelm-supervisor/src/service/core.rs:3399-3403: a missing directory is quoted raw in the refusal.
- crates/farhelm-helm/src/sessions.rs:2298 and 2331-2333: supervisor restart failures propagate to http_error.
- crates/farhelm-helm/src/client.rs:2527-2531 and crates/farhelm-helm/src/lib.rs:2182: supervisor error text survives
  into the HTTP body.
- crates/farhelm-ui/src/api.rs:2155-2157, 1648-1654, and 1392-1403: ActionRefusal retains the trimmed raw refusal.
- crates/farhelm-ui/src/session_view.rs:1352 and 2416: the changed-settings failure stores e.text in restart_with_error
  and passes it to the dialog.
- crates/farhelm-ui/src/restart_with.rs:649-650: the dialog interpolates the message directly.
- crates/farhelm-ui/src/session_view.rs:2515-2519: ordinary Restart renders its error through PeerLine and
  DetailPart::peer.
- crates/farhelm-helm/src/sessions.rs:2331-2333 propagates the supervisor restart result; :2298 maps an error through
  http_error.
- crates/farhelm-helm/src/client.rs:2526-2531 preserves the supervisor error message, and
  crates/farhelm-helm/src/lib.rs:2182 places it in the response body.
- crates/farhelm-ui/src/api.rs:2155-2157 constructs ActionRefusal for an unsuccessful restart; :1648-1654 obtains its
  raw text through refusal_text.
- crates/farhelm-ui/src/session_view.rs:1352 copies e.text into restart_with_error; :2416 passes that string to the
  dialog.
- crates/farhelm-ui/src/restart_with.rs:649-650 renders the message directly.
- crates/farhelm-ui/src/session_view.rs:2515-2519 renders ordinary Restart errors through PeerLine, confirming that the
  two error surfaces have different protection.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"basis": "review_feedback_queue/FILTER.md:24-42", "comparison": "A diagnostic-only interpretation resembles the
  filter, but shipped requests can encounter a stored directory containing presentation characters, and an untrusted
  supervisor can choose the error text. The complete filter match is not established."}
- SPEC_impl.md:415-438 describes Restart with's escaped command fields and modal focus protection; neither accepts raw
  diagnostic rendering.
- crates/farhelm-ui/src/session_view.rs:2515-2519 protects ordinary Restart's separate error renderer, not this one.

Caveats:

- The source report's reference to session_view.rs:1353 is one line late: the Restart with assignment is at 1352.
- A missing directory is refused before the stop; this finding does not claim that this particular refusal kills an
  agent.
- No HTML execution or additional authority is demonstrated.
- No runtime reproduction was performed.
- The character-rendering defect is definite; its practical security severity is provisional.
- No executable injection or unauthorized relaunch was demonstrated.
- The error text does not itself set YOLO authorization: crates/farhelm-ui/src/api.rs:1663-1666 reads that state from a
  dedicated header.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_14_sec:p1:F4`, `ui_desktop_14_cor:p1:F5`.

- `ui_desktop_14_sec:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.
- `ui_desktop_14_cor:p1:F5`: confidence as filed: definite; suggested bucket as filed: highest, provisionally for
  peer-controlled diagnostic spoofing.
