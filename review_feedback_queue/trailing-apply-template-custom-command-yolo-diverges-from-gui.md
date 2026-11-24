# apply_template leaves stale non-YOLO or unset YOLO on Command custom-agent drafts and diverges from GUI hand edits

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Applying launch templates that declare a custom agent on the Command tab behaves differently in the desktop app and the
CLI. A template without an explicit YOLO field works in the desktop app but fails from the CLI, and stacking a
custom-agent template onto a non-YOLO command draft either produces an invalid non-YOLO custom draft (which the CLI
later rejects) or silently flips a later built-in command draft to YOLO in the desktop app.

## Details

**other.** Source inspection. Main reference `crates/farhelm-proto/src/launcher.rs:865-870,1081-1091`.

In crates/farhelm-proto/src/launcher.rs:865-870, apply_template sets next.command_agent = Some(agent) on
LauncherKind::Command without setting next.yolo = Some(true) when agent.uses_custom_catalog(). Then
crates/farhelm-proto/src/launcher.rs:1081-1091 only rejects fields.yolo == Some(false) rather than checking next.yolo ==
Some(false). By contrast, GUI hand edits in crates/gui/farhelm-gui/src/launcher/draft.rs:1062-1069
(LaunchDraft::choose_command_agent, called during template replay at crates/gui/farhelm-gui/src/templates/apply.rs:514)
force self.command.yolo = Some(true) whenever a Custom command agent is selected. This breaks the shared-fold contract
(crates/farhelm-proto/src/launcher.rs:5-10 and SPEC.md, Launch templates) in three concrete ways:

1. A Command template declaring Custom without fields.yolo succeeds and marks YOLO in the GUI, while the same template
   in CLI/agent create (crates/farhelm-helm/src/agent_launch.rs:307-313) fails with "a command launch needs --yolo or
   --no-yolo unless a template sets its YOLO answer".
2. Stacking T1 ({kind: command, yolo: false}) then T2 ({agent: custom}) passes apply_template with next.command_agent =
   Some(Custom) and next.yolo = Some(false): CLI/helm later rejects it in CommandLaunch::validate_stored
   (crates/farhelm-proto/src/session_launch.rs:189-191), whereas GUI replay silently flips draft.command.yolo to
   Some(true).
3. Stacking T1 ({kind: command, yolo: false}), T2 ({agent: custom}), T3 ({agent: claude}) passes apply_template with
   next.yolo = Some(false) throughout, yet GUI replay calls choose_command_agent(Some(Custom)) on T2 (mutating
   draft.command.yolo to Some(true)) and choose_command_agent(Some(Claude)) on T3 (which does not restore false),
   leaving a non-YOLO Claude command flipped to YOLO in the GUI while the CLI keeps yolo = false.

How to verify: Call apply_templates with [TemplateFields { kind: Some(Command), yolo: Some(false), .. }, TemplateFields
{ agent: Some(Custom), .. }] and observe Ok(state) with state.command_agent == Some(Custom) and state.yolo ==
Some(false), then replay the same batch through crates/gui/farhelm-gui/src/templates/apply.rs.

Fix: Align apply_template and GUI choose_command_agent on one rule: either have apply_template set next.yolo =
Some(true) when LauncherKind::Command selects a custom agent (while still refusing fields.yolo == Some(false) in the
same or later template — and confirm whether CLI --command --agent custom should still require explicit --yolo per
SPEC.md), or refuse next.yolo == Some(false) in apply_template and do not let choose_command_agent silently overwrite
yolo during template replay.

Reviewer metadata (g41b_custom_agents_proto_launch/l1): suggested bucket: other (no security bypass:
CommandLaunch::validate_stored at session_launch.rs:189-191 fails closed on custom && !yolo at both helm and supervisor
boundaries, and the GUI divergence flips toward yolo=true rather than weakening YOLO gating).; confidence: confirmed;
possible cover: none.

Additional source record at `1b3fe49af86979aebc2719965682a17f8c6366ac`,
`crates/gui/farhelm-gui/src/templates/apply.rs:514` (g49_custom_agents_gui_plumbing/f8):

At reviewed tip 1b3fe49af86979aebc2719965682a17f8c6366ac (source analysis confirming and extending cross-group note from
g41b):

- `LaunchDraft::choose_command_agent` (`crates/gui/farhelm-gui/src/launcher/draft.rs:1062-1069`) unconditionally sets
  `self.command.yolo = Some(true)` whenever `agent.is_some_and(LaunchHarness::uses_custom_catalog)`.
- During template replay, `crates/gui/farhelm-gui/src/templates/apply.rs:514` invokes
  `draft.choose_command_agent(Some(agent))`.
- By contrast, the shared fold `farhelm_proto::launcher::apply_template`
  (`crates/farhelm-proto/src/launcher.rs:865-870, 1074-1091`) does not set `next.yolo = Some(true)` when
  `fields.agent == Some(LaunchHarness::Custom)` and refuses only `fields.yolo == Some(false)` (line 1085), not a
  pre-existing `next.yolo == Some(false)`.
- Three concrete GUI impacts follow:
  1. Stacked template batch
     `[T1(kind: Command, yolo: false, command: "claude {farhelm_args}"), T2(agent: Custom), T3(agent: Claude)]`:
     `apply_template` (and CLI) finishes with `command_agent = Some(Claude)` and `yolo = Some(false)`, whereas GUI
     `replay` flips `draft.command.yolo` to `Some(true)` on T2 and keeps `Some(true)` after T3, silently marking the
     Claude command as YOLO.
  2. Single `yolo`-less custom command template `{ kind: Command, agent: Custom, command: "acme {farhelm_args}" }`:
     `apply_template` + CLI (`crates/farhelm-helm/src/agent_launch.rs:307-313`) leaves `yolo = None` and refuses to
     launch until `--yolo`/`--no-yolo` is given, whereas GUI `replay` silently populates
     `draft.command.yolo = Some(true)` (allowing unseen quick launch or silently overwriting an existing
     `yolo = Some(false)` draft instead of refusing).
  3. Hand-picking Custom and then switching back to Claude in the Command tab dropdown
     (`crates/gui/farhelm-gui/src/launcher/view.rs:2161-2167`) likewise leaves `command.yolo` flipped to `Some(true)`.
- How to verify: Unit-test `templates::apply::apply` with
  `[T1(kind: Command, yolo: false, command: "claude {farhelm_args}"), T2(agent: Custom), T3(agent: Claude)]` and compare
  `draft.command.yolo` (`Some(true)`) against `apply_templates` (`Some(false)`).
- Fix: Keep `replay` faithful to the fold (`applied.yolo`) instead of letting `choose_command_agent` silently mutate
  `command.yolo` during replay; decide consistently in `apply_template` whether a custom command template without `yolo`
  implies `Some(true)` or requires an explicit `yolo: true` (and whether a prior `next.yolo == Some(false)` must be
  refused rather than only `fields.yolo == Some(false)`).

Reviewer metadata (g49_custom_agents_gui_plumbing/f8): suggested bucket: other (cannot weaken YOLO safety or lose data
because flipping `yolo` to `true` on a Command launch adds no CLI flags and only triggers stricter YOLO confirmation; it
breaks GUI/CLI template parity and misclassifies non-YOLO built-in commands as YOLO).; confidence: confirmed; possible
cover: none.

Additional source record at `df0dff4c9b925534b0aae763c325155279a58829`,
`crates/gui/farhelm-gui/src/launcher/draft.rs:1066` (g52_custom_agents_gui_launcher_a/f8):

- In `crates/gui/farhelm-gui/src/launcher/draft.rs:1062-1069`, `LaunchDraft::choose_command_agent` sets
  `self.command.yolo = Some(true)` whenever `agent.is_some_and(LaunchHarness::uses_custom_catalog)`.
- By contrast, the shared template fold `farhelm_proto::launcher::apply_template`
  (`crates/farhelm-proto/src/launcher.rs:865-870, 1081-1091`) does NOT mutate `next.yolo` when applying `agent: Custom`
  on `LauncherKind::Command`; it only refuses `fields.yolo == Some(false)` while the active command agent uses the
  custom catalog.
- Because `templates::apply::replay` (`crates/gui/farhelm-gui/src/templates/apply.rs:514`) calls
  `draft.choose_command_agent(Some(agent))` for each template in a batch:
  1. Stacking `[ { kind: Command, yolo: false }, { agent: Custom }, { agent: Claude } ]` folds in `apply_template` (and
     on CLI/helm) to `{ command_agent: Some(Claude), yolo: Some(false) }`, while the GUI draft ends up with
     `{ command_agent: Some(Claude), yolo: Some(true) }`.
  2. Stacking `[ { kind: Command, agent: Custom }, { agent: Claude } ]` (no `yolo` set) folds in `apply_template` to
     `yolo: None` (requiring an explicit answer), while the GUI pre-fills `yolo: Some(true)`.
  3. Stacking `[ { kind: Command, yolo: false }, { agent: Custom } ]` folds in `apply_template` to `yolo: Some(false)`
     (refused by `CommandLaunch::validate()` on CLI/helm), while the GUI silently rewrites `yolo` to `Some(true)`.
  4. Independently of templates, choosing Custom in the Command tab's Agent type dropdown and then switching to None or
     a built-in agent leaves `self.command.yolo` stuck at `Some(true)` even if it was previously `None` or
     `Some(false)`.
- Note on severity: The flip is always toward `yolo = Some(true)` (never `Some(false)`), so on hosts that ask before
  YOLO launches it triggers an extra confirmation rather than bypassing one; it cannot skip a YOLO gate or lose user
  work.
- How to verify: In `draft_tests.rs`, set
  `draft.choose_tab(Tab::Command); draft.choose_command_yolo(false); draft.choose_command_agent(Some(LaunchHarness::Custom)); draft.choose_command_agent(Some(LaunchHarness::Claude));`
  and observe `draft.command.yolo == Some(true)`. In `templates/apply_tests.rs`, apply the 3-template batch
  `[ { kind: Command, yolo: false }, { agent: Custom }, { agent: Claude } ]` and compare `draft.command.yolo`
  (`Some(true)`) with `farhelm_proto::launcher::apply_template` (`Some(false)`).
- Fix: Align `apply_template` and `LaunchDraft` on one rule. Either have `choose_command_agent` not overwrite
  `self.command.yolo` (or keep the pre-custom value / compute effective YOLO as `true` only while
  `command.agent.is_some_and(LaunchHarness::uses_custom_catalog)` in `command_launch()` and the Command tab's UI), or if
  `agent: Custom` is meant to force `yolo: true` in templates too, mirror that rule in
  `farhelm_proto::launcher::apply_template` and replay `applied.yolo` consistently.

Reviewer metadata (g52_custom_agents_gui_launcher_a/f8): suggested bucket: other (template-replay divergence from
`farhelm_proto::launcher::apply_template` and sticky unexpected `yolo = Some(true)` state when switching Command agent
type away from Custom; fails safe on YOLO confirmation).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g41b_custom_agents_proto_launch/l1`,
`g49_custom_agents_gui_plumbing/f8`, `g52_custom_agents_gui_launcher_a/f8`.
