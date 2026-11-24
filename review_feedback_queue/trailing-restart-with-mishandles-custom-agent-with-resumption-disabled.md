# Restart with mishandles custom agents whose registration disabled resumption

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

When you open Restart with on an existing custom-agent session after turning off "Supports resumption" for that custom
agent in Settings, the launcher gives contradictory or broken behavior: if the session has no captured conversation, it
tells you "Enter a conversation to restart" even though it hides the Conversation ID input and cannot resume; if the
session has a captured conversation, Restart stays enabled and submits a request that the helm/supervisor then rejects.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/request.rs:380`.

- An existing custom-agent session created while `supports_resumption` was `true` keeps `resume: Some(...)` in its
  stored `SessionLaunch::Agent`, so `RestartBaseline::of` (`crates/gui/farhelm-gui/src/launcher/draft.rs:116-127`) opens
  Restart with.
- When the registration is subsequently edited to `supports_resumption: false`, `refresh_custom_capabilities`
  (`view.rs:2262`) sets `draft.agent.custom_resumption = false`.
- `LaunchDraft::offers_conversation` (`crates/gui/farhelm-gui/src/launcher/request.rs:536-541`) returns `false` when
  `!self.agent.custom_resumption`, and `active_conversation` (`request.rs:530-533`) returns `None`.
- `LaunchDraft::named_start_error` (`request.rs:572-586`) only emits
  `"This custom agent no longer supports resumption."` when `!self.conversation.is_empty()`.
- Consequently, when `self.conversation` is empty (the initial state of Restart with):
  1. If `restart.conversation_required` is `true` (no captured conversation), `more_options.rs:33-36` hides the
     Conversation ID field (since `!offers_conversation() && conversation.is_empty()`), while `restart_refusal`
     (`request.rs:380-382`) refuses with `"Enter a conversation to restart"` — instructing the user to fill in an input
     that is not rendered (and whose input would be refused anyway).
  2. If `restart.conversation_required` is `false` (captured conversation exists), neither `agent_refusal`
     (`request.rs:336-374`) nor `restart_refusal` (`request.rs:378-398`) checks `self.agent.custom_resumption`. Editing
     model/effort enables Restart and submits `RestartSessionBody`, which the helm compiles via `compile_custom`
     (`crates/farhelm-helm/src/launches.rs:766`) with `resume: None`, and the supervisor rejects in
     `restart_session_named` (`crates/farhelm-supervisor/src/service/core.rs:12505, 12531-12535`).
- How to verify: Construct a `LaunchDraft::restart_with` for a `SessionLaunch::Agent` with `LaunchHarness::Custom`,
  `custom_agent: Some("acme-code")`, `resume: Some(...)`; keep `draft.agent.custom_resumption = false` and
  `conversation = ""`. Observe `launch_refusal`: when `conversation_required == true` it returns
  `"Enter a conversation to restart"` while `offers_conversation() == false`; when `conversation_required == false` and
  model is edited, `launch_refusal` returns `None`.
- Fix: In `agent_refusal` (or `restart_refusal`), when `matches!(self.mode, Mode::RestartWith(_))` and
  `self.tab == Tab::Agent` and `harness.uses_custom_catalog()` and `!self.agent.custom_resumption`, refuse immediately
  with `"This custom agent no longer supports resumption."` regardless of whether `self.conversation` is empty.

Reviewer metadata (g52_custom_agents_gui_launcher_a/f4): suggested bucket: other (confusing refusal / late server
rejection in Restart with after disabling resumption on a custom agent; supervisor still safely refuses without data
loss).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g52_custom_agents_gui_launcher_a/f4`.
