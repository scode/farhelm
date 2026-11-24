# Enter on a deleted custom-agent chip launches the previously selected agent instead of refusing

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

If a registered custom agent is deleted while the session launcher is open and the user presses Enter on that custom
agent's chip before the window redraws, the launcher fails to switch to the deleted agent but still immediately launches
using whatever agent was previously selected in the form. In Replace with mode, this can replace and retire the running
session using the wrong agent.

## Details

**highest.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/choices.rs:392-402`.

In `crates/gui/farhelm-gui/src/launcher/view/choices.rs:392-402` (`Launcher::harness_choices`), the harness `RadioGroup`
wires `.on_change` to call `launcher.choose_custom_agent(id, window, cx)` and `.on_enter` to unconditionally call
`launcher.press(window, cx)`. Under the shared choice-control Enter rule
(`crates/gui/farhelm-gui/src/components/radio_group.rs:552-560` and
`crates/gui/farhelm-gui/src/components/mod.rs:501-518`, `enter_choice`), pressing non-held Enter on a chip runs `apply`
(`on_change`) and then immediately runs `main` (`on_enter`) unconditionally.

In `crates/gui/farhelm-gui/src/launcher/view/choices.rs:119-142` (`Launcher::choose_custom_agent`), if the custom agent
ID is no longer present in `Collection::CustomAgents`, the method sets
`self.notice = Some("That custom agent is no longer registered".into())`, calls `cx.notify()`, and returns early without
modifying `self.draft`. Immediately afterwards, `enter_choice` invokes `on_enter` -> `launcher.press(window, cx)`, which
dispatches a launch using the untouched previous `self.draft` (for example, an already-selected built-in agent in
New/Clone, or the source session's agent in Replace with, where `Replace with` retires the existing session).

Notice that `permission_row` in the same file (`choices.rs:159-163`, `permission_enter`) explicitly guards `on_enter`
against launching when `on_change` did not establish a launchable choice, whereas `harness_choices` passes
`launcher.press(window, cx)` unconditionally.

How to verify: From source, trace `RadioGroup::key` -> `enter_choice` -> `on_change` (`choose_custom_agent` returning
early on missing `CustomAgent`) -> `on_enter` (`launcher.press`). In a GPUI test, open the launcher with a valid
selection (e.g., Replace with or New with Claude selected and a custom agent registered), focus the custom agent chip,
remove the custom agent from the pushed store without repainting (or delete it before Enter), send plain Enter on the
chip, and observe `press()` dispatching a create for the previous agent instead of stopping on the notice.

Fix: Track whether `choose_custom_agent` succeeded (or guard `harness_choices`'s `on_enter`, e.g., only calling
`self.press(window, cx)` when the chip's chosen value actually became the active draft harness/custom_agent, or have
`on_enter` check a flag / compare the focused choice against `self.draft.agent.custom_agent` rather than launching when
selection was rejected). Also verify whether a stale custom-agent chip in `Replace with` can retire a session without
matching the shown chip.

Reviewer metadata (g53_custom_agents_gui_launcher_b/f1): suggested bucket: highest — In Replace with mode (where the
form is prefilled with a valid host, folder, and non-YOLO agent), pressing Enter on a vanished custom-agent chip can
launch the wrong agent and retire the user's running session/process without consent to that action, violating SPEC.md's
drawn-frame rule ("launches that retire a session (Replace with) limited to the action and consent already shown").
Outside Replace with, it still launches an unintended agent/process (potentially YOLO if YOLO was selected on a host
that allows it). If Replace with's retirement or unintended agent launch does not reach `highest` under maintainer
judgment, it would be `high`; surfacing the `highest` boundary explicitly.; confidence: confirmed (source analysis of
`choices.rs:119-142`, `choices.rs:392-402`, `radio_group.rs:552-560`, and `components/mod.rs:501-518`; not executed).;
possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g53_custom_agents_gui_launcher_b/f1`.
