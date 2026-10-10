# Ordinary creation never refreshes the sidebar under build mismatch

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Creation under build mismatch could leave the new session absent from navigation.

## Details

F211 — **possible** — `crates/farhelm-ui/src/list/view.rs:3496`; `crates/farhelm-ui/src/list/view.rs:3488–3497` —
Ordinary creation never refreshes the sidebar under build mismatch

New and Clone open their result but rely on unattended listing refresh to add its sidebar row and count. Build mismatch
disables that refresh, so without another explicit refresh the session can become unavailable through its expected row
after selecting elsewhere. The session itself is not lost, and no runtime sequence was reproduced. Explicitly refresh
the listing after every successful creation.

## Evidence and triage context

- crates/farhelm-ui/src/list/create_form.rs:4465–4483 delivers the successful create through on_created.
- crates/farhelm-ui/src/list/view.rs:3447–3450 identifies Replace with; 3486 opens the result; 3496–3498 requests a
  listing only for that replacement case.
- crates/farhelm-ui/src/lib.rs:1587 only updates the selected session.
- crates/farhelm-ui/src/feed.rs:233–244 disables fallback polling under skew; 300–313 suppresses feed-triggered rereads.
- crates/farhelm-ui/src/reader.rs:476–483 still permits attended reads.
- SPEC_impl.md:3630–3634 withdraws unattended behavior while requiring explicit actions to keep working.
- crates/farhelm-ui/src/list/view.rs:3486–3497: creation opens the returned session but requests an explicit listing
  read only for Replace with.
- crates/farhelm-ui/src/list/view.rs:1575–1588: under latched build mismatch, feed and fallback reads stand down while
  explicit reads remain permitted.
- SPEC_impl.md:3630–3634 withdraws unattended behavior while preserving explicit user operations; it does not explicitly
  accept failing to refresh the result of plain creation.
- review_feedback_queue/FILTER.md:28–34 requires both a rare trigger and a qualifying recoverable consequence. Recovery
  by explicit refresh fits, but rarity is not established.
- review_feedback_queue/header-actions-skip-listing-read.md:16–25 concerns header Replace and Restart, not ordinary
  creation. It is possible related coverage, not an exact trigger/caller match.
- No process loss or security consequence was established, and no runtime reproduction was performed.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/header-actions-skip-listing-read.md:16–26 covers header Replace and Restart. The build-mismatch
  symptom overlaps, but ordinary New/Clone use the separately editable creation callback at view.rs:3496.

Caveats:

- Requires a latched build mismatch and no unrelated explicit listing refresh.
- The session exists and initially opens successfully; this is missing navigation state, not session loss.
- No runtime reproduction was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_12_cor:p1:F2`, `ui_desktop_12_sec:p1:C4`.

- `ui_desktop_12_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `ui_desktop_12_sec:p1:C4`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed:
  not separately tagged in candidate list.
