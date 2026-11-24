# All custom agents share one remembered wrapper prefix instead of keeping one per registered agent

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

Remembered wrapper prefixes are stored per agent type, but every registered custom agent shares a single bucket.
Launching one custom agent with a wrapper prefix causes Farhelm to pre-fill that same shell wrapper when starting a
completely different registered custom agent, and launching a second custom agent without a wrapper erases the first
agent's remembered wrapper.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/store.rs:263`.

Commit 7164b111339f33f9ec0b296488c38102191f6d9b introduced `LaunchHarness::Custom` plus
`LaunchSelection.custom_agent: Option<String>` so multiple distinct registered custom agents share one
`LaunchHarness::Custom` variant, but `crates/farhelm-helm/src/store.rs:263-274` (`record_launch_history`) still keys
`preferences.remembered_wrapper_prefixes` solely by `BTreeMap<farhelm_proto::LaunchHarness, Vec<String>>`
(`wrappers.insert(selection.harness, ...)` / `wrappers.remove(&selection.harness)`). Correspondingly, the desktop
launcher looks up `self.remembered_wrapper(LaunchHarness::Custom, cx)` in
`crates/gui/farhelm-gui/src/launcher/view/choices.rs:136` when picking any registered custom agent, and
`crates/gui/farhelm-gui/src/launcher/draft.rs:747-810` (`choose_custom_agent` -> `switch_agent`) only updates
`self.agent.wrapper` when `before.agent.harness != Some(harness)`, so switching between two custom agents in the open
launcher also leaves the previous custom agent's wrapper in place. SPEC.md (Sessions -> Creation) specifies that the
last wrapper prefix from a successful user agent launch is remembered per agent type and that switching agent types
loads the new type's remembered prefix, and SPEC.md (Topology) treats each registered custom agent as its own selectable
agent in the launcher's Custom group. Because a wrapper prefix is raw shell syntax executed via `bash -c` before the
agent executable, carrying one custom agent's wrapper (for example, an environment/endpoint/credential wrapper) onto
another custom agent can execute the wrong wrapper against the wrong program, while launching another custom agent
without a wrapper silently wipes the first agent's remembered prefix. Verify by registering two custom agents `a` and
`b`, launching `a` with a wrapper prefix, then opening New and selecting `b` (or launching `b` without a wrapper and
reopening `a`): `store.preferences().remembered_wrapper_prefixes` holds only one `"custom"` entry. Fix by keying
custom-agent remembered wrapper prefixes by custom agent ID (for example, a separate per-custom-agent map in preferences
or string keys distinguishing custom IDs from built-in harnesses) and updating `choose_custom_agent` in the launcher to
load that ID's remembered wrapper on intra-custom switches too.

Reviewer metadata (g44_custom_agents_helm_launches/f1): suggested bucket: high (can silently pre-fill and run the wrong
shell wrapper prefix — potentially including environment/endpoint overrides meant for a different custom agent — on
another registered custom agent, and silently clears a custom agent's remembered wrapper whenever a different custom
agent launches without one; could arguably be `other` if treated purely as convenience state, or worth checking whether
a hidden/collapsed wrapper carrying credentials/env onto a different program crosses into `highest`, though the user
still launches the session themselves so `high` is the established bucket).; confidence: confirmed (source analysis
across store.rs:263-282, choices.rs:136, and draft.rs:747-810 at 1b3fe49af86979aebc2719965682a17f8c6366ac).; possible
cover: none.

Additional source record at `df0dff4c9b925534b0aae763c325155279a58829`,
`crates/gui/farhelm-gui/src/launcher/draft.rs:759` (g52_custom_agents_gui_launcher_a/f6):

- `LaunchDraft::choose_custom_agent` (`crates/gui/farhelm-gui/src/launcher/draft.rs:747-772`) delegates first to
  `self.choose_harness(LaunchHarness::Custom, catalog, remembered_wrapper)` (line 759), which calls `switch_agent`
  (`draft.rs:807-843`).
- When switching from custom agent A (`acme-code`) to custom agent B (`other-code`), `before.agent.harness` is already
  `Some(LaunchHarness::Custom)`, so `switched_agent` at `draft.rs:825` evaluates to `false`.
- Because `switched_agent` is `false` inside `switch_agent`, lines 826-836 do not run:
  1. `self.agent.wrapper` is NOT replaced with `joined(remembered_wrapper)` and `self.agent.wrapper_explicit` is NOT
     reset to `false`. An explicit wrapper prefix typed for `acme-code` (or restored from a Clone/recent setup of
     `acme-code`) silently remains attached to `other-code`, even though `choose_custom_agent` treats switching between
     registrations as switching agents (`edit.switched_agent = true`, clearing `model`, `custom_effort`, and
     `conversation`). Note SPEC.md ("Creation"): "Switching agent types loads the new type's remembered prefix."
  2. In `choose_custom_agent` lines 760-767, clearing `self.agent.model` and `self.agent.custom_effort` produces no
     `edit.notice` (unlike switching between built-in agents, which reports cleared model/effort choices via
     `reconciliation_notice`).
- How to verify: In `draft_tests.rs`, select custom agent A via `choose_custom_agent(&agent_a, None, None)`, call
  `draft.edit_wrapper("env ACME=1".into())`, then call `draft.choose_custom_agent(&agent_b, None, None)`. Observe
  `draft.agent.wrapper` is still `"env ACME=1"` and `draft.agent.wrapper_explicit` is still `true`.
- Fix: Inside the `if self.agent.custom_agent.as_deref() != Some(agent.id.as_str())` block of `choose_custom_agent`
  (`draft.rs:760-767`), reload `self.agent.wrapper = joined(remembered_wrapper);` and
  `self.agent.wrapper_explicit = false;` (and populate `edit.notice` if `self.agent.model.is_some()` or
  `!self.agent.custom_effort.is_empty()` was cleared).

Reviewer metadata (g52_custom_agents_gui_launcher_a/f6): suggested bucket: other (stale wrapper prefix carried between
custom agents; visible in More options, though easily missed when More options is collapsed; could execute an unintended
wrapper command if the user assumes switching custom agents resets the wrapper like built-in agents do).; confidence:
confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g44_custom_agents_helm_launches/f1`,
`g52_custom_agents_gui_launcher_a/f6`.
