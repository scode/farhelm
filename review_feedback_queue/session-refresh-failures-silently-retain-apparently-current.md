# Session refresh failures silently retain apparently current metadata

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failed detail refreshes leave old session information looking current.

## Details

F227 — **definite** — `crates/farhelm-ui/src/session_view.rs:942` — Session refresh failures silently retain apparently
current metadata

The detail reader discards fetch errors while retaining its last successful snapshot. Repeated failures provide no
warning that status, Resume availability, and tab metadata may be stale. Keep the snapshot and retries, but retain an
applicable refresh error for display. Apply the same ordering checks that reject obsolete replies and pre-restart
snapshots before publishing or clearing that error.

## Evidence and triage context

- crates/farhelm-ui/src/api.rs:2110-2122: detail fetching returns errors for request failures, unsuccessful non-404
  responses, and invalid response JSON.
- crates/farhelm-ui/src/session_view.rs:931-949: every fetch error returns false before the commit path; the error value
  is discarded.
- crates/farhelm-ui/src/session_view.rs:879-903: refresh_stale becomes true only for an accepted None/404 result and
  false for an accepted session result.
- crates/farhelm-ui/src/session_view.rs:2598-2603: the visible refresh warning is conditional on refresh_stale and
  specifically describes the helm no longer listing the session.
- crates/farhelm-ui/src/session_view.rs:1236-1240: a healthy event feed requests another detail read; it does not itself
  replace session metadata or expose the failed read.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/session-detail-drains-full-list.md:19-47 covers request amplification from detail reads, not
  discarded errors or misleading freshness.
- review_feedback_queue/FILTER.md:24-42 covers rare diagnostic failures or brief self-correcting displays. Sustained
  endpoint failure is not established to meet its rare-trigger requirement.
- SPEC.md:1803-1812 requires visible actionable errors and names exceptions; detail-refresh failures are not among them.

Caveats:

- A later successful detail read repairs the snapshot automatically.
- Host-wide outages or authentication failures may have other visible surfaces; the demonstrated gap includes failures
  isolated to detail fetching.
- No process loss or unsafe destructive action is established by this finding.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_14_cor:p1:F2`.

- `ui_desktop_14_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
