# Restart and prompt cards credit Settings for stored workspace trust

Reviewed commit: 74d862ec7467aece5c9bf88657e77cbe59982b5a

## TLDR

When an agent asks to restart a session or to send it a prompt, the approval card shows the trust that session was
started with. If the user had the agent's workspace trust switch on when the session started and has since turned it
off, the card still says the workspace is trusted, "Enabled on the agent's Settings page, or automatically in a
session's own sandbox or container". The user checks Settings, finds the switch off, sees the session is not in a
sandbox, and cannot tell why the session they are approving work on is trusted. This is the confusion #4958 set out to
remove, on routes it did not cover.

## Details

#4958 changed `TRUSTED_WORKSPACE` (`crates/gui/farhelm-gui/src/approvals/model.rs:975`) to name both sources, and
SPEC.md "Agent-spawned sessions" (~4711) says "The trusted chip names both sources". But the chip is also drawn from a
stored launch. Restart: `crates/farhelm-helm/src/agent_requests.rs:725-775` builds the card from the session's own
stored launch, and SPEC.md (~1855) says "Plain Restart and the Replace answers that reuse the saved command keep the
stored choice"; `model_tests.rs` `launch_and_restart_cards_explain_stored_workspace_trust` (~470-505) shows Restart
cards use the same chip text. Prompt: `model.rs:516-526` builds the Permissions fact from the target's stored `launch`.
So a stored `workspace_trust: Some(true)` recorded while the switch was on is explained by the current switch, which may
be off. The card still says Trusted, so the trust is disclosed; only the explanation is wrong.

Route: switch on, create a Claude/Codex/Muse/Pi/Antigravity session on a local or SSH host, turn the switch off, an
agent runs `farhelm agent restart` (or sends a prompt) on that session.

Fix: give stored-launch cards wording that names the session's saved launch as the source ("Saved with this session when
it started"), or choose the text per action, and update the SPEC sentence to match.

Suggested bucket: `other` (misleading explanation on a card that still discloses the trust). Confidence: confirmed from
source. Possible cover: SPEC.md "Agent-spawned sessions" says only that the chip names both sources, which triage may
read as accepting this.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g02-trust-a11y-settings-approvals` record `f2`.
