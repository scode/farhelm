## What this was about

An agent could start another session with an initial message, but could not send a follow-up to a session already
running. `farhelm agent prompt` now sends a message to another live Claude or Codex session, including one on another
host. It asks through the normal command approval card unless the asking host is trusted to run Farhelm commands without
asking.

## Things you should know

The card shows the full message as labelled data, the target's permission shield and whether someone has the session
open. Blank lines remain blank; unsafe characters are shown escaped. The open-client notice is a snapshot. Delivery is
allowed while someone is typing there, so the message can mix with their draft. Self-targets, unsupported agents, agents
waiting on a question, unknown status and agents that are not live are refused. The owning supervisor checks again after
approval; a replaced target connection requires a fresh request.

A busy Claude or Codex queues the message after its current turn. Success means tmux accepted the paste and submit key,
without waiting for the agent to start or finish. A failed delivery may have sent some or all of the input; a retry
sends it again. Agent instructions explain this prompt-injection boundary and advise against sending secrets through it.

Manual checks used Claude Code 2.1.302 and Codex 0.163.0 in private test terminals. Both had bracketed paste enabled
while idle and working. Claude Enter and Codex Tab submitted an idle message and queued a busy one for the next turn;
Codex Enter instead steered the current turn. Immediate paste and submit needed no screen delay. These observations
establish the behavior of those versions, not every future vendor version. Claude's editor expanded a literal tab to
four spaces, while Codex retained tabs and newlines in the recorded user message. Farhelm sends the original bytes to
the terminal; the vendor editor can change them.

Main gained managed-checkout trust settings while this stack was being built. The rebase kept its trust-question status
detection and moved this stack's protocol increments to 56 in the supervisor PR and 57 in the command PR. The upgrade
path remains supported under the existing protocol rules; going back to an older Farhelm after installing this version
is not supported, as agreed for the plan.

## Open questions and possible follow-ups

There is no blocking decision. A UI button, delivery to other agents and a native vendor delivery route remain separate
work. The per-agent delivery seam allows a later route without changing the other agents. The existing warning for a
missing agent reply starts when a user submits text with Enter; this command does not start that warning's clock. A
command can therefore report delivery without Farhelm warning if the target never replies. The plan deliberately leaves
that warning's scope for later work. Ordinary conversation reporting remains in place.

## The PRs

- [#2257 — supervisor prompt delivery](https://github.com/scode/farhelm/pull/2257/changes), draft.
- [#2265 — command, approval and documentation](https://github.com/scode/farhelm/pull/2265/changes), draft on #2257.

## Checks run, reused and skipped

- Run on the rebased stack:
  `cargo nextest run -p farhelm -p farhelm-proto -p farhelm-supervisor -p farhelm-helm -p farhelm-ui --lib --bins -E 'test(prompt) | test(agent_instructions) | test(session_permission)'`
  passed 57/57 selected tests across six binaries, recorder `7f448663-9f09-47ab-9b6e-37fe62b081a6`. Its source snapshot
  was PR1 `cf4d3d5f` plus the PR2 working diff (recorder source fingerprint
  `8cf2aed29d5765be83106a5bafe1556a7b2382ecc9f006a9638ace5f19621e30`). It covers behavior before the numeric protocol
  correction; no behavior changed afterward.
  `cargo nextest run -p farhelm-proto --lib -E 'test(protocol_version_is_pinned_at_57) | test(prompt_delivery_wire_shapes_and_directions_are_pinned) | test(prompt_target_reply_json_shape_is_pinned) | test(agent_prompt_wire_shape_retains_fencing_and_approval) | test(unknown_control_message_tag_fails_decode)'`
  passed 5/5, recorder `0b8df2cb-eff1-4c0e-982d-bf58012530ba`, on final PR1 `314148e0` plus the final PR2 working diff
  (fingerprint `0a50d1ae9c46ce553306b095189316320360eba80560078bc0870fd46cbbfb64`). That working diff became PR2
  `14fcfd22`. Both runs used pinned nextest 0.9.148 and tmux 3.7c with four slots and no retries; reports and console
  output are complete, and neither emitted a runtime `SKIPPED` marker.
- Run on the rebased stack: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, changed-Markdown dprint, changelog format, instruction-link check,
  and the isolated test-sleep parser (304 delays inspected, none missing a reason) passed. Formatting was checked again
  after the protocol correction; Clippy results remain applicable because the correction changed only protocol version
  literals and their test names.
- Reused from PR1 `af8dc86e` plus the PR2 working diff (source fingerprint
  `55bc0af6c0a5a51cfb803ef46556e6756d89bf343dce677f064683297417b001`): `cargo nextest run -p farhelm --test agent_cli`
  passed 42/42 (`1969fb8a-ce4f-41d5-86d9-4f2b3e933eb6`); the e2e selection
  `test(prompt_running_agent::) | test(agent_listing_real_stack::)` passed 7/7 (`e576383e-fc37-4dd4-a4cf-d8fa9754ec9d`);
  and
  `playwright test tests/approval-layout.spec.ts -g 'a running prompt card keeps its full message and target facts readable'`
  passed 2/2 in Chromium and WebKit (`e24fd6c7-9021-4e0b-aada-e6f75a7376d6`). The later trust-settings rebase and
  numeric protocol correction did not change the tested prompt behavior or card layout. The Dioxus web bundle passed at
  that same pre-trust-rebase source stage. `bun install --frozen-lockfile && bun run build` passed for the website on
  earlier PR1 `9c9f476d` plus PR2 working copy `90e94fc0` with the updated docs. Neither the subsequent trust changes
  nor the version correction changed the affected page or card, so those builds still cover them.
- Earlier development runs `7278a2de-4d66-47e4-97c5-174e62784a67`, `7d266fa0-21dc-44fb-a808-48be2b9f8a0d`,
  `ce257364-8fd1-46b3-bcaf-4016a7558488` and `9a59d822-f836-4d01-a709-9d4db29b1555` remain retained as failures. Their
  test fixtures or expected escaped-tab text were corrected in this run; the later passes above are separate
  observations. No latent flake was established.
- Skipped the full Rust and browser batteries, desktop runtime, installer and provisioning checks because the focused
  runs cover the changed delivery, approval and wire contracts without exercising unrelated surfaces.

## Review gate's outcome

The required fresh-context gpt-6.1-sol reviews at high effort covered both PRs and their changed tests. The final PR2
review found one rebase error: PR1 had lost its independent protocol bump. The author corrected it to 56 in PR1 and 57
in PR2; the final wire tests passed. The same reviewer verified both final PR tips and found the correction complete,
with no other actionable issue. Fresh cold readers also checked the commit and PR wording. Neither PR was marked ready
or merged by the executor.
