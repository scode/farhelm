# Custom agent YOLO permission control omits the red escalation outline on permission escalation

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

When switching from a safer agent (such as Guarded Claude or Codex) to a registered custom agent (which always counts as
YOLO), the red permission-escalation banner appears at the top of the launcher, but the custom agent's YOLO permission
control does not show the required thick red escalation outline.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/choices.rs:285-297`.

`SPEC.md` ("Sessions" → "Creation", lines 1166-1171) specifies: "When an agent switch automatically makes the shown
permission more dangerous, a red banner slides down across the top of the launcher, naming both modes and explaining
that the new agent does not support the previous mode. ... The permission control gets a thick red outline that fades
after about two and a half seconds. Reduced motion replaces slide and fade with a static outline while the banner is
shown."

For built-in harnesses, `Launcher::permission_row` (`crates/gui/farhelm-gui/src/launcher/view/choices.rs:559-560`) wraps
the permission control in `self.with_escalation_outline(group, palette, cx)`. For custom agents,
`Launcher::agent_fields` (`view.rs:1871-1877`) renders `self.custom_agent_fields(palette)` instead of
`model_choices`/`permission_row`. Inside `Launcher::custom_agent_fields`
(`crates/gui/farhelm-gui/src/launcher/view/choices.rs:285-297`), the static `"YOLO"` pill is drawn directly as a plain
`div()` without calling `self.with_escalation_outline(..., palette, cx)`. Because `LaunchHarness::Custom` has
`omitted_permission = Some(Yolo)`, switching from a Guarded/Approve/Chat built-in agent (or from an explicit Guarded
pre-selection) to a custom agent raises the escalation banner (`draft.rs:759` -> `switch_agent` -> `escalation_since`),
yet the permission control gets no outline.

How to verify: Inspect `crates/gui/farhelm-gui/src/launcher/view/choices.rs:285-297` alongside `choices.rs:559-560` and
`SPEC.md:1166-1171`. Select Claude (Guarded) in the launcher, then select a registered custom agent chip; the top
escalation banner renders, while the YOLO pill has no escalation outline.

Fix: Pass `cx: &Context<Self>` to `custom_agent_fields` and wrap the YOLO permission badge (or permission control
container) in `self.with_escalation_outline(..., palette, cx)`.

Reviewer metadata (g53_custom_agents_gui_launcher_b/f5): suggested bucket: other — Visual safety-cue omission against
SPEC.md when escalating to a custom agent; the top red escalation banner with Undo and the YOLO pill itself are still
shown, and host YOLO confirmation still applies.; confidence: confirmed (source analysis of `choices.rs:285-297`,
`choices.rs:559-560`, `view.rs:1871-1877`, and `SPEC.md:1166-1171`; not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g53_custom_agents_gui_launcher_b/f5`.
