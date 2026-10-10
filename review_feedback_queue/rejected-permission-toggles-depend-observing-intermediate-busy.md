# Rejected permission toggles depend on observing an intermediate busy effect

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A refused permission revocation could leave a misleading unchecked checkbox.

## Details

F29 — **possible** — `crates/farhelm-ui/src/hosts/settings_dialog.rs:382` — Rejected permission toggles depend on
observing an intermediate busy effect

The checkbox is restored after refusal only if a rendering effect observes busy changing from true to false. If the
actual request fails before that effect sees true, the native checkbox could stay unchecked while command permission
remains enabled. That scheduling order has not been demonstrated in the shipped transport. The refusal text remains
visible and no new permission is granted. Reset the control from operation completion or an outcome generation,
independently of intermediate busy observation.

## Evidence and triage context

- crates/farhelm-ui/src/hosts/settings_dialog.rs:374 stores the previously effect-observed busy value; :385 repairs
  controls only after an observed true-to-false transition.
- crates/farhelm-ui/src/hosts.rs:1164 sets busy before spawning the request; :1187 clears it after refusal.
- [private local path] processes runnable tasks before effects.
- crates/farhelm-ui/src/api.rs:1199 awaits the real HTTP client, so a mock immediately-ready future alone would not
  prove a shipped trigger.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- No exact acceptance or existing item found. Do not apply a cosmetic-glitch filter while the possible consequence is a
  misleading revocation control.

Caveats:

- No reproduction proving that a shipped request fails before the intermediate effect.
- The error and explanatory text remain visible.
- This does not grant permission; it may falsely display a successful revocation.
- The security-sensitive misleading state is not safely disposable under the rare-display-glitch filter.
- Actual fast-refusal ordering is the material unverified premise.
- The refusal message and permission help text remain visible.
- No permission is newly granted, and no successful server revocation is undone.
- Keep possible; do not promote to definite without exercising the shipped scheduler and transport.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_08_cor:p1:F2`.

- `ui_desktop_08_cor:p1:F2`: confidence as filed: possible; suggested bucket as filed: highest.
