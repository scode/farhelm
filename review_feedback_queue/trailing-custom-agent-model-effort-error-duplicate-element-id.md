# Custom agent Model and Effort error lines share duplicate GPUI element ID 'launcher-custom-value-error'

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When a user enters invalid values in both the Model and Effort fields of a custom agent in the session launcher at the
same time, both validation status lines mount under the same UI element ID, which collides in the accessibility tree and
triggers a debug panic when assistive technology is active.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/choices.rs:265`.

At reviewed tip 1b3fe49af86979aebc2719965682a17f8c6366ac (in
`crates/gui/farhelm-gui/src/launcher/view/choices.rs:241-311`, added in commit df0dff4c9b925534b0aae763c325155279a58829
atop our scope file `launcher/view/choices.rs`):

- In `Launcher::custom_agent_fields` (`choices.rs:243-284`), the `input` closure creates an un-ID'd `div()` wrapping
  `row(row_id, ...)` and an optional `status_line("launcher-custom-value-error", capitalized(error), palette)`
  (`choices.rs:265`).
- `status_line` (`choices.rs:611-624`) creates a `Stateful<Div>` with `.id(id).role(Role::Status).aria_label(...)`.
- Both `model` (`choices.rs:269-276`) and `effort` (`choices.rs:277-284`) are placed as children of the same un-ID'd
  `div().flex().gap(px(12.))` (`choices.rs:297-302`).
- When both `model` and `effort` fail `validate_model_or_effort` simultaneously (e.g. typing an uppercase letter or
  space into both), two sibling `Stateful<Div>` nodes mount in the same frame with the identical GPUI `ElementId`
  `"launcher-custom-value-error"`.
- Per `SPEC_impl.md` ("GPUI cookbook: traps", lines 1602-1603), duplicate element IDs are a debug panic while assistive
  technology is active and collide in accessibility/state tracking.
- How to verify: Render `Launcher` with a registered custom agent selected and set both `custom_model` and
  `custom_effort` to invalid strings (e.g. `"Bad Model"` and `"Bad Effort"`) with assistive technology / accessibility
  debug checks active.
- Fix: Pass distinct static error IDs (e.g. `"launcher-custom-model-error"` and `"launcher-custom-effort-error"`) as a
  parameter to the `input` closure in `custom_agent_fields`.

Reviewer metadata (g49_custom_agents_gui_plumbing/l4): suggested bucket: other (narrow UI/accessibility bug: only
triggers when both custom Model and custom Effort are simultaneously invalid; debug panic with AT active, no security or
data-loss impact).; confidence: confirmed; possible cover: none.

Additional source record at `df0dff4c9b925534b0aae763c325155279a58829`,
`crates/gui/farhelm-gui/src/launcher/view/choices.rs:262-286` (g53_custom_agents_gui_launcher_b/l1):

In `crates/gui/farhelm-gui/src/launcher/view/choices.rs:241-286` (`Launcher::custom_agent_fields`), the local `input`
closure builds both the Model column and the Effort column inside anonymous `div()` wrappers and renders each field's
inline error via `status_line("launcher-custom-value-error", capitalized(error), palette)` (`choices.rs:262-264`).
`status_line` (`choices.rs:607-619`) assigns `.id(id)` and `.role(Role::Status)` directly on the returned
`Stateful<Div>`.

Because neither the Model wrapper `div()` nor the Effort wrapper `div()` assigns an `.id(...)`, both status elements
receive the exact same element ID path (`"launcher-custom-value-error"`) whenever both fields fail
`validate_model_or_effort` simultaneously (for example, typing uppercase or whitespace in both fields). Per
`SPEC_impl.md:1602-1603`, duplicate GPUI element IDs trigger a debug panic when assistive technology is active and
collide in element/accessibility identity.

How to verify: Inspect `choices.rs:243-286` and `choices.rs:607-619`. In a GPUI test with a registered custom agent
selected, type invalid text (e.g. `Bad`) into both `launcher-custom-model` and `launcher-custom-effort` and render a
frame with accessibility/AT active (or inspect the drawn element IDs).

Fix: Pass distinct error element IDs (e.g., `"launcher-custom-model-error"` and `"launcher-custom-effort-error"`) into
the `input` closure in `Launcher::custom_agent_fields`.

Reviewer metadata (g53_custom_agents_gui_launcher_b/l1): suggested bucket: other — Correctness and accessibility bug
(colliding status element IDs and debug panic under assistive technology when both fields are simultaneously invalid);
does not lose user data or compromise security.; confidence: confirmed (source analysis of `choices.rs:241-286`,
`choices.rs:607-619`, and `SPEC_impl.md:1602-1603`; not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g49_custom_agents_gui_plumbing/l4`,
`g53_custom_agents_gui_launcher_b/l1`.
