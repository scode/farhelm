# Partial templates cannot retain non-YOLO approval choices without an agent type

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Approval-only templates cannot preserve supported partial settings.

## Details

F225 — **definite** — `crates/farhelm-ui/src/list/templates.rs:286` — Partial templates cannot retain non-YOLO approval
choices without an agent type

When no agent type is specified, the editor offers an incomplete permission vocabulary and rejects otherwise applicable
non-YOLO approval choices. Users cannot save an approval-only partial template even though templates are intended to
support stacking incomplete settings. Offer the full vocabulary while the agent is unspecified, restrict it after an
agent is chosen, and keep application-time validation.

## Evidence and triage context

- crates/farhelm-ui/src/list/templates.rs:218-220 permits the permissions field without an agent, but lines 281-288
  restrict its choices to YOLO when agent is absent.
- crates/farhelm-ui/src/list/templates.rs:242-250 uses the same restricted options to refuse existing permission values.
- crates/farhelm-ui/src/list/templates.rs:357-360 and 441-449 propagate that refusal to editor and launcher saves.
- crates/farhelm-ui/src/list/save_template.rs:98-105 can remove Agent while retaining Permissions; lines 165-171 then
  refuse the save.
- crates/farhelm-proto/src/launch.rs:265-280 allows Approve for OMP and every explicit permission for Goose.
- crates/farhelm-proto/src/launcher.rs:506-511 and 579-588 validate against the current launcher's agent when applying a
  template, so an earlier stacked template can supply the compatible agent.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- The explicit non-YOLO modes currently concern OMP and Goose.
- This does not silently change a launch to YOLO; it refuses saving.
- FILTER.md:96-114 does not cover ordinary template-authoring refusal or this consequence.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_11_cor:p1:F4`.

- `ui_desktop_11_cor:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.
