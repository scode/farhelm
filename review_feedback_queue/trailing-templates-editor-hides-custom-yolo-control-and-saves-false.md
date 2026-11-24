# Templates editor hides Approvals control for custom command templates while allowing yolo=false to be added and saved

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

In the Templates editor, adding Approvals to a Command template that declares a custom agent (or opening a stored one
whose Approvals was set to 'Keeps safety checks') displays only the static message 'Custom agents always run in YOLO
mode.' with no toggle and lets the user save, yet actually saves YOLO as off ('Not YOLO'). Every attempt to apply that
saved template then fails, and reopening it in the editor offers no control to fix it.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/templates/editor/rows.rs:341`.

Four places interact to create this defect:

1. `crates/gui/farhelm-gui/src/templates/fields.rs:148`: `Field::Yolo.add` initializes `fields.yolo = Some(false)`
   unconditionally, even when `fields.agent` is `Some(LaunchHarness::Custom)`.
2. `crates/gui/farhelm-gui/src/templates/editor/rows.rs:341-344`: when
   `fields.agent.is_some_and(LaunchHarness::uses_custom_catalog)`, `Field::Yolo` renders only a static
   `hint("templates-fixed-yolo", "Custom agents always run in YOLO mode.", palette)` with no choice control and no
   indication that `fields.yolo` is `Some(false)`.
3. Neither `Field::refusal` (`crates/gui/farhelm-gui/src/templates/fields.rs:229-295`) nor `check_template_shape`
   (`crates/farhelm-proto/src/launcher.rs:546-643`) rejects `fields.yolo == Some(false)` when `fields.agent` is
   `LaunchHarness::Custom`. Saving succeeds and writes `"yolo": false` (which the template list summary displays as 'Not
   YOLO').
4. Applying the saved template fails unconditionally in `farhelm_proto::launcher::apply_template`
   (`crates/farhelm-proto/src/launcher.rs:1081-1091`) with `YOLO answer: custom agents always run in YOLO mode`.

Additionally, in `crates/gui/farhelm-gui/src/templates/editor/rows.rs:423-426`, choosing a custom agent in
`Field::Agent` on a Command template unconditionally sets `fields.yolo = Some(true)` even if `Field::Yolo` was not in
the template — silently inserting an unrequested Approvals field (contrary to SPEC.md's explicit add/remove field model
and `fields.rs:135`). If the user removes that unexpected Approvals field and later adds Approvals back (or switches an
Agent custom-agent template to Command and adds Approvals), `Field::Yolo.add` sets `fields.yolo = Some(false)` and
triggers the broken save.

How to verify: In the Templates editor, create a Command template, add Agent type and choose Custom (or a registered
custom agent), remove Approvals, click '+ Add field' -> 'Approvals', observe the editor says 'Custom agents always run
in YOLO mode.' with no toggle, and click Save. The saved template stores `yolo: false` and fails when applied via `tl:`
or quick launch. (Also reproducible by switching an Agent custom-agent template to Command and adding Approvals, or
opening a stored template with `kind: command, agent: custom, yolo: false`.)

Fix: (1) In `Field::add` (`fields.rs:148`), set
`fields.yolo = Some(fields.agent.is_some_and(LaunchHarness::uses_custom_catalog))` so adding Approvals for a custom
agent starts at `true` (or omit `Field::Yolo` from `offered` / set it to `true`). (2) In `Field::refusal`
(`fields.rs:229-295`), refuse `self == Field::Yolo && custom && fields.yolo == Some(false)` with an actionable message,
and in `rows.rs:341-344`, when `fields.yolo == Some(false)`, show the incompatible value / a control to change it to
YOLO instead of only claiming 'Custom agents always run in YOLO mode.'. (3) In `rows.rs:423-426`, only coerce
`fields.yolo = Some(true)` if `fields.yolo.is_some()` (or keep existing explicit field presence so selecting Agent type
does not silently add Approvals).

Reviewer metadata (g54_custom_agents_gui_templates_goldens/l2): suggested bucket: high — materially degrades the user
experience on a direct editor workflow: the editor misstates the saved YOLO state, saves an unapplicable template
without error, and gives no control to repair it. Not `highest` because `apply_template` fails closed and refuses
`yolo: false` on custom agents rather than bypassing YOLO checks.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g54_custom_agents_gui_templates_goldens/l2`.
