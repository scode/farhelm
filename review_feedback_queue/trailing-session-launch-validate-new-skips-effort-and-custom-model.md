# SessionLaunch::validate_new does not validate widened LaunchEffortValue or custom model strings

Reviewed commit: 7164b111339f33f9ec0b296488c38102191f6d9b

## TLDR

Widening effort on stored and transmitted launches to accept custom free-text strings allows invalid or arbitrary effort
text (including control characters or unknown words on built-in agents) to pass launch-shape validation when loading
stored sessions or validating a session launch directly.

## Details

**other.** Source inspection. Main reference `crates/farhelm-proto/src/session_launch.rs:505-560`.

Commit 7164b111339f33f9ec0b296488c38102191f6d9b changed LaunchSelection.effort from Option<LaunchEffort> (a closed
8-variant enum) to Option<LaunchEffortValue> (crates/farhelm-proto/src/launch.rs:645-650,750), which is
#[serde(untagged)] with a fallback Custom(String) variant. Previously, serde deserialization rejected any unknown effort
string at supervisor DB load (crates/farhelm-supervisor/src/store.rs:3239-3246, whose doc comment states the DB is a
trust boundary refusing anything outside this build's vocabulary via serde + SessionLaunch::validate_new), supervisor
wire admission (crates/farhelm-supervisor/src/service/handlers.rs:211, core.rs:3578), and pre-launch-kind row migration
(crates/farhelm-proto/src/session_launch.rs:323-357). Now any arbitrary string (empty, >256 bytes, spaces, terminal
escape sequences) deserializes into LaunchEffortValue::Custom(String), and SessionLaunch::validate_new
(crates/farhelm-proto/src/session_launch.rs:505-552) validates custom_agent ID, custom_reporting, and YOLO for Custom
launches, but never checks selection.effort (or selection.model for Custom). As a result:

- A built-in SessionLaunch::Agent (e.g., Codex or Claude) with selection.effort =
  Some(LaunchEffortValue::Custom("anything\x1b[31m")) passes validate_new() and from_pre_launch_kinds().
- A Custom SessionLaunch::Agent with selection.model or selection.effort violating
  custom_agents::validate_model_or_effort passes validate_new().
- GUI helpers such as launch_effort_label (crates/gui/farhelm-gui/src/permission/words.rs:31-37) pass Custom(value)
  through verbatim.

How to verify: Deserialize a SessionLaunch::Agent for harness "codex" with "effort": "bogus\u001b[31m" (or harness
"custom" with "effort": "bad effort") and call validate_new(); it returns Ok(()).

Fix: In SessionLaunch::validate_new (crates/farhelm-proto/src/session_launch.rs:513-529):

- For selection.harness.uses_custom_catalog(), validate selection.model (if Some) and selection.effort (if Some, via
  as_cli_arg()) with crate::custom_agents::validate_model_or_effort.
- For non-custom harnesses, reject selection.effort when effort.known().is_none() (matching check_template_shape at
  crates/farhelm-proto/src/launcher.rs:570-579 and launches::validate_selection at
  crates/farhelm-helm/src/launches.rs:1135-1143).

Reviewer metadata (g41b_custom_agents_proto_launch/l3): suggested bucket: other (helm's launches::compile and
launches::compile_custom validate effort/model on the normal create path and the supervisor executes start/resume argv
rather than selection.effort, so there is no established security or data-loss route from shipped helm APIs; this is a
validation-boundary regression at supervisor DB/wire/migration).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g41b_custom_agents_proto_launch/l3`.
