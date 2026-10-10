# Manual read/unread toggles cannot visibly toggle back under build mismatch

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A manual read toggle cannot visibly reverse under build mismatch.

## Details

F215 — **definite** — `crates/farhelm-ui/src/list/view.rs:2979` — Manual read/unread toggles cannot visibly toggle back
under build mismatch

The settled manual write changes neither the row's seen state nor the target of its next toggle. Without unattended
refresh, the old label and dot remain, and another click submits the same value instead of its inverse. Reconcile the
acknowledged manual value locally or explicitly refresh from completion; ordering the write queue alone cannot repair
this stale control.

## Evidence and triage context

- crates/farhelm-ui/src/list/view.rs:2973–2984 queues the write with seen_toggle_report.
- crates/farhelm-ui/src/list/view.rs:3885–3900 reports success only by removing an error entry.
- crates/farhelm-ui/src/api.rs:2498–2516 sends the queued mark and calls its report without updating the listing.
- crates/farhelm-ui/src/list/row.rs:908,991–1013 derives the label and next submitted value from the unchanged session's
  unseen state.
- crates/farhelm-ui/src/feed.rs:233–244,300–313 suppresses unattended refresh under mismatch.
- SPEC.md:1075–1083 defines a working manual read/unread toggle; SPEC_impl.md:3630–3634 preserves explicit actions under
  mismatch.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:4364–4375, seen-toggle-report-panics-after-unmount.md, addresses accessing a dropped signal. Its
  try_write fix does not refresh or reconcile the row after a successful write.

Caveats:

- Requires build mismatch and no unrelated listing refresh.
- Automatic mark-on-open is a separate path; use a nonselected session to avoid conflating its behavior with this
  defect.
- No runtime reproduction was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_12_cor:p1:F6`.

- `ui_desktop_12_cor:p1:F6`: confidence as filed: definite; suggested bucket as filed: other.
