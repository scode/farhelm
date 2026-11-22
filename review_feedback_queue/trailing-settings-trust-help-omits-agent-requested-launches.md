# Settings workspace-trust help for Codex, Muse, Pi, and Antigravity omits agent-requested launches

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

The Settings pages for Codex, Muse, Pi, and Antigravity tell the user their workspace-trust switch applies in folders
they choose and in managed checkouts for New, Clone, Restart with, and Replace with, without mentioning that it also
applies to agent-requested launches. On a host where agent requests are auto-approved, turning the switch on lets an
agent start trusted sessions in any directory without the user seeing that stated in Settings.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/agent_trust.rs:208-246`.

Bucket note: placed in `high`; reviewer surfaced that omitting agent-requested launches from the Settings opt-in copy
could cross into `highest` on hosts with Always-allow enabled where no approval card is shown.

Before commit 4aff3f1b1e0b778d4f6265b6a0660ebfc7ac4e1e, `TrustAgent::mechanism` in
`crates/gui/farhelm-gui/src/settings/agent_trust.rs` explicitly stated "including agent-requested checkouts" for Codex,
Muse, and Pi. Commit 4aff3f1b1e0b778d4f6265b6a0660ebfc7ac4e1e rewrote `mechanism()` (lines 208-224) to "In folders you
choose and in managed checkouts, ..." and `help()` (lines 234-239) appends "New, Clone, Restart with and Replace with
use this setting. Restart and Replace keep the session's saved launch.", dropping any mention of agent-requested
launches (whereas commit b3a14cd25cc48869e18bc6be867d47822251a15c preserved "including launches an agent requests" in
Claude's `mechanism()` at line 204). By contrast, `SPEC.md` lines 1796-1798 state that "New, Clone, Restart with,
Replace with and agent-requested launches apply the current setting in any directory. An inherited spawn reads Settings
under the same rule as Clone." (and Antigravity's switch applies to newly composed Antigravity agent launches in
ordinary directories, managed checkouts, and disposable environments, `SPEC.md` lines 1805-1806). Interactive approval
cards do display a "Trusted workspace" chip (`crates/gui/farhelm-gui/src/approvals/card.rs:583-599`), which mitigates
the omission when cards require manual approval, but hosts granted "Always allow" skip the card. Fix direction: restore
"including launches an agent requests" (or add "agent-requested launches" to the verb list in `help()`) for Codex, Muse,
Pi, and Antigravity in `crates/gui/farhelm-gui/src/settings/agent_trust.rs:208-246` and refresh the affected Settings
goldens.

Reviewer metadata (g17-trust-gui-and-goldens/f1): suggested bucket: high (could be highest if Settings opt-in copy
omitting agent-requested launches is treated as exceeding the stated consent scope on hosts with Always-Allow enabled,
where no approval card is shown; surface uncertainty for bucket finalization); confidence: confirmed; possible cover:
none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g17-trust-gui-and-goldens/f1`.
