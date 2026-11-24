# check_template_shape allows saving a Custom agent template with yolo=false, producing permanently unapplicable templates

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Both the CLI and the desktop Templates editor allow saving a Command launch template that declares a custom agent with
YOLO turned off, creating a saved template that Farhelm always refuses when you try to apply it (and in the desktop
editor the Approvals toggle is hidden behind a static YOLO message, so the user cannot even see or fix the saved
non-YOLO flag without removing the field).

## Details

**other.** Source inspection. Main reference `crates/farhelm-proto/src/launcher.rs:546-581`.

check_template_shape (crates/farhelm-proto/src/launcher.rs:546-643) validates custom_agent IDs (lines 560-569) and
custom effort strings (lines 570-581) against template.fields.agent, but never refuses
template.fields.agent.is_some_and(LaunchHarness::uses_custom_catalog) combined with template.fields.yolo == Some(false),
even though apply_template (crates/farhelm-proto/src/launcher.rs:1081-1091) and CommandLaunch::validate_stored
(crates/farhelm-proto/src/session_launch.rs:189-191) unconditionally reject that combination. This is reachable from two
shipped authoring paths:

1. CLI/agent template create:
   `farhelm agent template create foo --command 'acme {farhelm_args}' --no-yolo --agent custom` (or `--agent <id>`)
   builds TemplateFields with kind=Some(Command), agent=Some(Custom), yolo=Some(false)
   (crates/farhelm/src/main.rs:1107-1135). crates/farhelm-helm/src/agent_requests.rs:3351 runs check_template_shape,
   which returns Ok(()), so the template is approved and saved, then fails every time it is applied.
2. Desktop Templates editor: After choosing Command + Custom agent, if the user removes and re-adds the Approvals
   (Field::Yolo) field, Field::Yolo::add sets fields.yolo = Some(false)
   (crates/gui/farhelm-gui/src/templates/fields.rs:148). Neither check_template_shape nor Field::refusal
   (fields.rs:229-295) refuses it, and templates/editor/rows.rs:341-344 replaces the radio buttons with the static hint
   "Custom agents always run in YOLO mode." — hiding the false value while saving yolo: false.

How to verify: Call check_template_shape on LaunchTemplate { name: "bad".into(), fields: TemplateFields { kind:
Some(LauncherKind::Command), agent: Some(LaunchHarness::Custom), yolo: Some(false), ..Default::default() } }; it returns
Ok(()), whereas apply_template refuses it.

Fix: In check_template_shape (crates/farhelm-proto/src/launcher.rs:560-570), refuse
template.fields.agent.is_some_and(LaunchHarness::uses_custom_catalog) && template.fields.yolo == Some(false) with
"custom agents always run in YOLO mode" (and in the GUI editor, initialize Field::Yolo::add to Some(true) when
agent.uses_custom_catalog(), or refuse yolo == Some(false) in Field::refusal).

Reviewer metadata (g41b_custom_agents_proto_launch/l4): suggested bucket: other (no YOLO bypass because apply_template
and CommandLaunch::validate_stored refuse the template at application/launch; usability bug that lets users save
unapplicable templates).; confidence: confirmed; possible cover: none.

Additional source record at `1b3fe49af86979aebc2719965682a17f8c6366ac`, `crates/farhelm-helm/src/agent_requests.rs:3351`
(g46_custom_agents_helm_requests/f3):

- Checked during Turn 2 cross-group reconciliation with `g41b/l4`.
- `SPEC.md` ("Topology", lines 494-496) specifies: "The Command tab can declare a registered agent or generic
  **Custom**. Either declaration counts as YOLO and carries its own command, optional resume command and reporting
  behavior."
- In `crates/farhelm-helm/src/agent_requests.rs:3315-3360` (`write_template_for_agent`), validation checks only
  `fields.command.is_some() && fields.yolo.is_none()`, `mixes_launch_kinds`, and
  `farhelm_proto::launcher::check_template_shape(&template)` (`agent_requests.rs:3351`). Neither
  `write_template_for_agent` nor `check_template_shape` (`crates/farhelm-proto/src/launcher.rs:546-660`) rejects
  `agent.is_some_and(LaunchHarness::uses_custom_catalog) && yolo == Some(false)`.
- Consequently, `farhelm agent template create foo --command "my-tool {farhelm_args}" --agent custom --no-yolo` (or
  `--agent <registered-id> --no-yolo`, or editing a `--no-yolo` command template to set `--agent custom`) passes
  validation, shows a misleading approval card with `agent: custom` and `asserted YOLO: no`
  (`crates/gui/farhelm-gui/src/approvals/model.rs:815-824,877-882`), and persists a broken template.
- Applying that template always fails in `apply_template` (`crates/farhelm-proto/src/launcher.rs:1079-1088`:
  `"YOLO answer: custom agents always run in YOLO mode"`), and launching it is also rejected by
  `CommandLaunch::validate_stored` (`crates/farhelm-proto/src/session_launch.rs:189-191`). When opened in the GUI
  template editor, `crates/gui/farhelm-gui/src/templates/editor/rows.rs:341-344` hides the YOLO toggle for custom agents
  and displays only the static hint `"Custom agents always run in YOLO mode."`, while
  `crates/gui/farhelm-gui/src/templates/fields.rs:235-290` flags no error on `Field::Yolo`.
- Security check: cannot bypass the YOLO guard because both `apply_template` (`launcher.rs:1079-1088`) and
  `CommandLaunch::validate_stored` (`session_launch.rs:189-191`) refuse `custom && !yolo`, and
  `yolo_guard::check_agent_wrapper` refuses all command launches on guarded hosts regardless of `yolo`.
- How to verify: inspect `crates/farhelm-helm/src/agent_requests.rs:3315-3360`,
  `crates/farhelm-proto/src/launcher.rs:546-660,1079-1088`, and
  `crates/gui/farhelm-gui/src/templates/editor/rows.rs:341-344`.
- Fix: reject
  `template.fields.agent.is_some_and(LaunchHarness::uses_custom_catalog) && template.fields.yolo == Some(false)` in
  `check_template_shape` (`crates/farhelm-proto/src/launcher.rs`, covering both `write_template_for_agent` and
  `PUT /api/templates/{name}`), or explicitly in `write_template_for_agent` before showing the approval card.

Reviewer metadata (g46_custom_agents_helm_requests/f3): suggested bucket: other (template validation/UX bug and
misleading template-write card; cannot bypass runtime YOLO guards, so no security or data-loss impact).; confidence:
confirmed; possible cover: none.

Additional source record at `1b3fe49af86979aebc2719965682a17f8c6366ac`,
`crates/gui/farhelm-gui/src/templates/editor/rows.rs:341` (g49_custom_agents_gui_plumbing/l1):

At reviewed tip 1b3fe49af86979aebc2719965682a17f8c6366ac (introduced across commits
7164b111339f33f9ec0b296488c38102191f6d9b and df0dff4c9b925534b0aae763c325155279a58829):

- `crates/gui/farhelm-gui/src/templates/fields.rs:148` (`Field::add`) initializes `Field::Yolo` to
  `fields.yolo = Some(false)` unconditionally, even when `fields.agent` is already `LaunchHarness::Custom`.
- `crates/gui/farhelm-gui/src/templates/editor/rows.rs:341-344` renders `Field::Yolo` as only a static hint
  (`"Custom agents always run in YOLO mode."`) whenever `fields.agent.is_some_and(LaunchHarness::uses_custom_catalog)`,
  hiding the `Keeps safety checks` / `YOLO` radio buttons regardless of whether `fields.yolo` is `Some(true)` or
  `Some(false)`.
- Neither `Field::refusal` / `Draft::refusal` (`crates/gui/farhelm-gui/src/templates/fields.rs:229-295, 450-505`) nor
  `check_template_shape` (`crates/farhelm-proto/src/launcher.rs:546-643`, called by `store_template` in
  `crates/farhelm-helm/src/templates.rs:105`) rejects `kind: Some(Command), agent: Some(Custom), yolo: Some(false)`.
- Consequently, saving stores `yolo: false` (contradicting the editor text). Applying that template in the launcher is
  then refused by `farhelm_proto::launcher::apply_template` (`crates/farhelm-proto/src/launcher.rs:1081-1091`) with
  `"YOLO answer: custom agents always run in YOLO mode"`, and reopening the template offers no control to change `yolo`
  to `true` (only removing Approvals or switching Agent type away and back to Custom triggers `rows.rs:423-426`).
- How to verify: Source analysis shows that in the Templates editor, creating a Command template with Agent type =
  Custom, removing Approvals, and clicking `+ Add field -> Approvals` (or switching an Agent custom-agent template to
  Command and adding Approvals) sets `draft.fields.yolo = Some(false)` while showing only
  `"Custom agents always run in YOLO mode."`, passes `Editor::refusal` and `check_template_shape`, and fails
  `apply_template`.
- Fix: Initialize `Field::Yolo` to `Some(fields.agent.is_some_and(LaunchHarness::uses_custom_catalog))` in `Field::add`,
  coerce `fields.yolo = Some(true)` if present when switching kind to `Command` with a custom agent (or refuse
  `yolo == Some(false)` with a custom agent in `Field::refusal` and `check_template_shape`), and if
  `fields.yolo == Some(false)` is loaded from an existing template, either show the refusal/radio so it can be repaired
  or explicitly show a repair action instead of claiming it is already YOLO.

Reviewer metadata (g49_custom_agents_gui_plumbing/l1): suggested bucket: other (cannot weaken YOLO safety or lose data
because `apply_template` and `CommandLaunch::validate_stored` both refuse non-YOLO custom commands; however it creates a
contradictory UI dead-end that saves an unapplicable template with no visible control to repair it in the editor).;
confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g41b_custom_agents_proto_launch/l4`,
`g46_custom_agents_helm_requests/f3`, `g49_custom_agents_gui_plumbing/l1`.
