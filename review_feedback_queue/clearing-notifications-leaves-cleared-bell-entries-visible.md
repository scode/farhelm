# Clearing notifications leaves the cleared bell and entries visible under build mismatch

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Cleared notifications reappear under build mismatch.

## Details

F214 — **definite** — `crates/farhelm-ui/src/list/view.rs:2945` — Clearing notifications leaves the cleared bell and
entries visible under build mismatch

Clear succeeds on the server and closes the popup, but leaves the local listing unchanged. With unattended refresh
disabled, the bell remains and reopening it shows already-cleared entries until another explicit refresh occurs. Refresh
after successful Clear, or remove entries locally through the acknowledged boundary, so the completed action is
reflected in navigation state.

## Evidence and triage context

- crates/farhelm-ui/src/list/view.rs:2934–2947 closes the popup, sends the clear, and on success only removes an error
  entry.
- crates/farhelm-ui/src/api.rs:2250–2266 sends the cleared boundary and returns without local reconciliation.
- crates/farhelm-ui/src/list/row.rs:964–970 derives bell presence from the listing's notification vector; 1690–1700
  renders that same vector.
- crates/farhelm-ui/src/feed.rs:233–244,300–313 disables unattended refresh under mismatch.
- SPEC.md:1151–1153 requires Clear to remove the entries and bell. SPEC_impl.md:2791–2797 explains that newly built rows
  exclude cleared entries.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:3630–3634 accepts withdrawal of unattended work, not failure to reconcile a successful explicit Clear.

Caveats:

- Requires build mismatch and no unrelated explicit listing refresh.
- The server's cleared boundary is correct; the reported defect is client presentation.
- No runtime reproduction was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_12_cor:p1:F5`.

- `ui_desktop_12_cor:p1:F5`: confidence as filed: definite; suggested bucket as filed: other.
