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

### Landing

Landed on its own in round 91, as #2257 and #2265, merged bottom-up, with one wording fix to SPEC.md made during the
landing (below).

#### What landed since the stack was based, and whether it interacted

Only plan-queue bookkeeping reached main between this stack's base and its landing, and no other plan landed in this
round; the landing rebase was clean. The executor had already rebased onto the managed-agent trust settings (#2255,
#2256), which touched the same agent-request and approval-card code, and onto the move of session details below the
terminal (#2253), which moved the approval cards. The landing monitor read the stack against both, and a fresh-context
reviewer did the same independently. Neither found a conflict: the new prompt card follows the cards' current
conventions, its "open in a client" notice comes from the supervisor rather than anything that moved in the session
view, and its permission shield is the one the session list shows, including an observed switch to a no-checks mode.

The reviewer also checked the new channel's safety boundary and found it holds: a message containing the sequence that
ends a terminal paste cannot break out of it, because control characters other than newline and tab are refused at the
command, the helm, the asking host's supervisor and, last, the owning supervisor just before anything is sent. Only the
helm can deliver directly; a session can only ask through the approval card, and both the asking host's supervisor and
the helm refuse a session prompting itself. The asking and target connections are checked again after approval, and a
trusted host's commands-without-asking setting and "always allow" behave exactly as for the other agent commands.

#### Upgrade

#2257 raises the protocol from 55 to 56 and #2265 to 57; neither has shipped (v0.29.0 carries protocol 51), so the next
release already requires remote hosts to be updated. The helm database schema is unchanged (57). Two plans still in
flight will need care when they land: cheaper-cache-clones (now blocked) may also change the protocol and would then
need the next free number, and both it and host-terminal-colors plan to take helm schema 58, which collides between them
rather than with this stack.

#### Fixes made during the landing

SPEC.md, in two places. The paragraph on reading the agent's mode from its status line said approvals use the stored
launch alone, which the new prompt card, showing the observed shield, contradicted; it now says approval decisions do.
And the prompt paragraph now says that the owning supervisor's "idle or running" check is its latest status sample, so a
question that appears within about one sampling interval (about two seconds) before delivery can still receive the
message.

#### Possible follow-ups

- **A question that appears just before delivery.** Because of that sampling gap, a Claude permission or trust question
  that pops up between samples can receive the pasted message and its Enter, which likely answers the question with its
  highlighted choice. That undoes the reason waiting agents are refused. The same gap exists right after a launch,
  before the first sample, when a trust question may be on screen. Taking a fresh screen reading just before pasting, or
  checking that the agent's input box is on screen before sending the submit key, would close it. Not tested against a
  real agent.
- **Approval is not tied to the target's current run.** If the user restarts or replaces the target while the card
  waits, the approved message goes to the new run, which can have a different permission than the shield the card showed
  (a Replace into YOLO, say). Delete and Restart already guard against this; prompting could carry the expected launch
  the same way. That takes another protocol change, which costs users nothing extra if it lands before the next release,
  since that release already requires updating remote hosts. Otherwise SPEC.md should say the shield, like the "open in
  a client" notice, is a snapshot.
- **An empty message submits the user's draft.** SPEC.md allows an empty message, which sends only the submit key and so
  submits whatever the user has typed in the target's input box, while the card shows an empty block. Refusing empty
  messages for this command, or labelling them on the card, would make that visible.
- **Decided in the plan, worth keeping in mind.** A host trusted to run Farhelm commands without asking can now drive an
  existing YOLO session on any host with no card, since the YOLO launch rule does not apply to a command that starts no
  agent. Two sessions can still prompt each other back and forth; only prompting oneself is refused.
- **Wording.** The command's errors say "an initial prompt" because they come from the check shared with launches. The
  changelog entry says agents waiting on a question "refuse" the message, though Farhelm refuses it, and leaves out that
  the message can mix with what the user is typing; the release curation can fix both.

#### Checks

- Run now, on the stack with the landing fix (the landed diffs are identical to it): `cargo fmt --all -- --check`,
  `dprint check SPEC.md`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`; then, through the test recorder with pinned nextest and tmux, four
  slots and no retries, the whole agent command-line test file, the prompt end-to-end and agent-listing selections, and
  the executor's prompt, agent-instruction and session-permission unit selection: 106 passed, no runtime skips, run
  `bfbb16fa-4b9a-467b-848b-fb6e9226cfb7`; then fresh `cargo build` and web builds and
  `npx playwright test approval-layout.spec.ts` on both engines, one worker, no retries: 24 passed, run
  `e1acdbcf-9565-4dd2-8099-a241ce775788`. These replace the executor's runs that predated the trust settings and the
  moved approval cards.
- Reused: the executor's protocol wire-shape run `0b8df2cb-eff1-4c0e-982d-bf58012530ba` on the final code, and its
  website build, whose pages nothing since has changed.
- Skipped: everything the report skipped, for the reasons it gives.

Nothing in the executor's report became untrue at landing.
