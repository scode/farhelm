# The update popup stops following the current step after opening

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The update popup stops keeping the current step visible.

## Details

F206 — **definite** — `crates/farhelm-ui/src/hosts.rs:2443` — The update popup stops following the current step after
opening

The popup scrolls to the current installation step when opened but does not rerun that operation when progress advances.
In a short viewport the highlighted step can move outside the visible list, which the user cannot scroll to recover.
Track current-step changes reactively and scroll after their corresponding DOM update.

## Evidence and triage context

- crates/farhelm-ui/src/hosts.rs:2443 subscribes the scrolling effect only to placed; :2474 renders changing step props.
- [private local path] tracks reads performed by the effect.
- [private local path] states that callback replacement does not notify reactive readers.
- crates/farhelm-ui/assets/app.css:4161 disables pointer events and :4179 clips the step list.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC.md:108–111 explicitly promises to keep the current step visible. No matching queue coverage. The rare-glitch
  filter does not apply: a constrained viewport and hovering across progress are ordinary conditions, not a demonstrated
  rare timing window.

Caveats:

- Requires a viewport too short for the complete list and a hover spanning progress changes.
- Leaving and hovering again repairs the display.
- No browser reproduction.
- Rehovering repairs the display.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_07_sec:p1:F3`.

- `ui_desktop_07_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
