# Rename refusal text bypasses peer-text rendering

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Rename refusal text can visually conceal peer-controlled content.

## Details

F223 — **definite** — `crates/farhelm-ui/src/rename.rs:268` — Rename refusal text bypasses peer-text rendering

A supervisor-authored refusal reaches the rename dialog without presentation escaping. Invisible or directional
characters can hide or reorder the explanation, even though text rendering does not establish a further unauthorized
action. Render this error through the existing peer-text line and detail representation, exposing unsafe characters and
preserving the intended direction boundary.

## Evidence and triage context

- crates/farhelm-helm/src/sessions.rs:2377-2378 and 2403-2406: rename forwards to the owning supervisor and sends
  failures through http_error.
- crates/farhelm-helm/src/client.rs:2527-2531: ControlMsg::Error.message is retained unchanged in SupervisorError.
- crates/farhelm-helm/src/lib.rs:2182: http_error places the error's formatted text directly in the HTTP body.
- crates/farhelm-ui/src/api.rs:2206-2211: rename_session returns refusal_text for a non-success response.
- crates/farhelm-ui/src/api.rs:1392-1403: refusal_text only trims surrounding whitespace; embedded directional and
  invisible characters survive.
- crates/farhelm-ui/src/list/view.rs:2590-2593 and 3816-3819: the same error is stored for the owning editor and passed
  into RenameDialog.
- crates/farhelm-ui/src/rename.rs:267-268: the error is interpolated directly into a paragraph.
- crates/farhelm-helm/src/client.rs:2526-2531 converts a correlated ControlMsg::Error into SupervisorError while
  preserving message.
- crates/farhelm-helm/src/sessions.rs:2377-2378 propagates rename errors; :2404-2406 sends them through http_error.
- crates/farhelm-helm/src/lib.rs:2182 writes the formatted error chain directly into the HTTP response body.
- crates/farhelm-ui/src/api.rs:1392-1403 preserves refusal-body characters apart from surrounding whitespace; :2206-2211
  returns that text from rename_session.
- crates/farhelm-ui/src/list/view.rs:2590-2593 stores the error unchanged; :3816-3819 passes the matching generation's
  error to RenameDialog.
- crates/farhelm-ui/src/rename.rs:267-268 renders the error directly as paragraph text.
- crates/farhelm-ui/src/api.rs:1378-1387 explicitly assigns escaping and direction isolation of raw refusal text to
  display surfaces.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"basis": "review_feedback_queue/FILTER.md:24-42", "comparison": "The consequence includes a misleading diagnostic,
  but no proof establishes the full rare-trigger condition. A remote supervisor can deliberately supply presentation
  controls, and the filter excludes trust-boundary consequences. Coverage is therefore insufficient to drop."}

Caveats:

- Ordinary title-validation refusal wording is fixed; an adversarial supervisor reply provides a concrete
  controllable-message path.
- No script execution, credential disclosure, or authorization bypass is demonstrated.
- No browser reproduction was performed.
- This sink is independent of the title displays in F2.
- A malicious or malformed supervisor refusal is the relevant peer-input trigger; normal fixed validation messages need
  not contain these characters.
- No markup execution, credential disclosure, or unauthorized mutation was demonstrated.
- Security severity beyond misleading diagnostic presentation is provisional.
- This renderer is independent of the source-title renderer in F3.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_14_sec:p1:F3`, `ui_desktop_14_cor:p1:F4`.

- `ui_desktop_14_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
- `ui_desktop_14_cor:p1:F4`: confidence as filed: definite; suggested bucket as filed: highest, provisionally for
  peer-controlled diagnostic spoofing.
