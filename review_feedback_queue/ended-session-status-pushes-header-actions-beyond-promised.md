# Ended-session status pushes header actions beyond the promised 650px width

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An ended-session badge could clip actions at the promised pane width.

## Details

F194 — **possible** — `crates/farhelm-ui/assets/app.css:1213` — Ended-session status pushes header actions beyond the
promised 650px width

The ended-status badge consumes a fixed width alongside the header action cluster. Its width budget can push trailing
controls outside a 650px pane, where they are promised to remain visible. Exact geometry remains unverified; no
independent Chromium or native WebKit measurement ran. Make the ended status responsive to the action cluster,
preserving complete status text in a tooltip or another accessible surface.

## Evidence and triage context

- crates/farhelm-ui/src/session_view.rs:2029 renders the status before the trailing action cluster at line 2112.
- crates/farhelm-ui/src/status.rs:146 constructs ended-status text including the exit code and annotation.
- crates/farhelm-ui/assets/app.css:1215 prevents status shrinking; line 1227 prevents action-cluster shrinking; line
  3487 caps the badge at 32ch without responding to remaining header space.
- SPEC.md:1205 and SPEC_impl.md:403 promise all six actions from a 650px main pane.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- The express clipping allowance applies below 650px. No matching accepted disposition was found for clipping at 650px.

Caveats:

- The original isolated Chromium geometry was not independently reproduced.
- No native WebKit result was verified.
- This finding does not challenge the accepted behavior below 650px.
- No independent browser geometry measurement.
- Do not broaden this to all session states or native WebKit without evidence.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_01_cor:p1:F2`.

- `ui_desktop_01_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
