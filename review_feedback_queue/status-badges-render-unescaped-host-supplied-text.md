# Status badges render unescaped host-supplied text

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Remote diagnostic text can visually distort session status badges.

## Details

F51 — **definite** — `crates/farhelm-ui/src/status.rs:256` — Status badges render unescaped host-supplied text

Supervisor-supplied error details and annotations reach badges and their tooltips without escaping invisible or
directional characters. That text can conceal or reorder parts of the displayed diagnostic alongside the application's
own status wording. Text interpolation does not execute HTML, and wrong-target actions or credential exposure are not
established. Escape the peer-supplied portions at rendering and isolate them from local wording, including tooltip text.
The already-escaped compact ended-status tooltip is outside this finding.

## Evidence and triage context

- crates/farhelm-supervisor/src/launch.rs:949-955 interpolates argv[0] directly into the exec-failure report;
  launch.rs:795-818 returns valid nonempty sentinel contents without presentation escaping.
- crates/farhelm-supervisor/src/service/status.rs:679-727 preserves the sentinel detail; status.rs:459-476 publishes
  error details and annotations in SessionInfo.
- crates/farhelm-helm/src/client.rs:2723-2736 returns the supervisor's session list. manager.rs:730-771 validates count,
  session IDs and duplicates, but does not escape diagnostic fields.
- crates/farhelm-helm/src/aggregate.rs:163-165 flattens SessionInfo into the response; aggregate.rs:306-325 retains that
  information. crates/farhelm-ui/src/api.rs:1473-1483 and 2119-2122 deserialize sessions, while lib.rs:327-337 retains
  status and annotation.
- crates/farhelm-ui/src/status.rs:158-160 appends the annotation verbatim; status.rs:174 interpolates error detail
  verbatim; status.rs:256-258 renders that text and puts the same raw string in data-tooltip.
- crates/farhelm-ui/src/session_view.rs:2033 and 2622 render the shared badge in the header and stale band.
  list/row.rs:1630-1638 uses it for noncompact ended-session detail.
- crates/farhelm-ui/assets/tooltip.js:248-250 assigns the string to textContent. app.css:1162 isolates the complete
  tooltip but does not expose invisible characters or isolate its peer portion from locally authored wording.
- crates/farhelm-ui/src/peer.rs:63-85 provides the missing presentation escaping. session_view.rs:2057-2084 already
  applies it to the neighboring directory and command.
- crates/farhelm-helm/src/manager.rs:730-771: drain_sessions checks list length, session IDs and duplicate IDs, then
  returns the listing unchanged; annotations and error details are not sanitized.
- crates/farhelm-helm/src/sessions.rs:2197-2212: the session-detail handler selects the supervisor's SessionInfo and
  serializes it directly into the response.
- crates/farhelm-ui/src/api.rs:2110-2122 and crates/farhelm-ui/src/lib.rs:302-337: the response deserializes into
  Session, retaining status and annotation.
- crates/farhelm-ui/src/session_view.rs:3045-3055 and crates/farhelm-ui/src/list/row.rs:909: both session-view and
  sidebar callers pass those fields to status_badge.
- crates/farhelm-ui/src/status.rs:158-174: annotation is appended verbatim and Error.detail is interpolated verbatim.
  Lines 256-258 place the resulting text in both data-tooltip and visible text.
- crates/farhelm-ui/assets/tooltip.js:248-250: tooltip text is assigned through textContent, preventing HTML
  interpretation but preserving Unicode presentation controls.
- crates/farhelm-ui/assets/app.css:3472-3491: badge styling clips long text but supplies no peer-value direction
  isolation.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:2290-2312, session-header-raw-peer-text.md, covers title, directory and invocation
  rendering/copying, not error details or exit annotations in StatusBadgeView.
- review_feedback_queue/FILTER.md, Rare, self-correcting glitches and imprecise diagnostics: possibly relevant to an
  accidental unusual filename, but not sufficient to reject the untrusted-peer spoofing scope. The filter explicitly
  excludes security or trust-boundary consequences.
- {"basis": "TRIAGE_OUTCOMES.md:2290-2312, session-header-raw-peer-text.md", "comparison": "Related trust boundary, but
  the recorded trigger and fix concern title, working directory, invocation and copying those values. Status annotations
  and error details are separate inputs rendered at a separately editable shared component."}
- {"basis": "TRIAGE_OUTCOMES.md:2333-2357, display-peer-misses-invisible-chars.md", "comparison": "Expands the shared
  escaper and hardens host-identity presentation. This badge never calls that escaper, so the decision does not resolve
  its bypass."}

Caveats:

- No browser reproduction or runtime test was performed.
- Dioxus text interpolation and tooltip textContent do not execute HTML; this finding is about Unicode presentation.
- The compact sidebar's visible ended-status tooltip is already escaped at list/row.rs:1502. The finding does not claim
  that tooltip is vulnerable.
- Ordinary stop annotations are locally authored fixed wording; arbitrary annotation payloads require a faulty or
  hostile supervisor.
- The highest bucket is conservative handling of possible security impact, not a claim of demonstrated exploitation.
- The demonstrated consequence is visual manipulation of diagnostics. Script execution, credential disclosure and
  unauthorized cross-host operations are not established.
- No browser reproduction ran.
- TODO.md:33-43 contains no matching Planned item; BUGS.md and the current queue index provide no matching accepted or
  queued defect.
- The rare-diagnostic filter does not justify dropping an untrusted presentation-boundary defect;
  review_feedback_queue/FILTER.md:36-42 excludes security consequences.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_15_cor:p1:F1`, `ui_desktop_15_sec:p1:F1`.

- `ui_desktop_15_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `ui_desktop_15_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
