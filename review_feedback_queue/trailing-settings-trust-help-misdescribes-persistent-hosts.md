# Settings trust help says sandbox and container sessions trust automatically

Reviewed commit: cc9bec6cf70ab293d6f8b2c2115bc170f69c8d01

## TLDR

Every agent's trust help in Settings now says "Sandbox and container sessions apply workspace trust automatically" and
"On installed hosts ... use this setting". A user with a persistent Tensorlake sandbox or Podman container host made
with Add host reads that as covering their sessions there. In fact those sessions follow the off-by-default switch, so
the agent still stops at its trust question. The user is told the switch does not matter there when it is the only thing
that does.

## Details

At cc9bec6cf7 (unchanged at the tip), `crates/gui/farhelm-gui/src/settings/agent_trust.rs:235-236` (Claude) and
`:267-268` (Codex, Muse, Pi, Antigravity) add the two sentences above. The policy in
`crates/farhelm-helm/src/workspace_trust.rs:29-37` (`TrustDestination::for_host`) makes a destination disposable only
when the host row is session-owned. SPEC.md "Managed checkouts" (~1853) says "Structured launches in a session's own
sandbox or container automatically trust ... Local, SSH and persistent recipe hosts follow Settings." The website page
(`start-a-session.mdx:383-385`) draws the line correctly ("On installed hosts and hosts you create with Add host, trust
follows Settings → Agents"). The module doc at `agent_trust.rs:1-2` says "installed and persistent hosts", so the intent
is known; only the user-facing paragraph is wrong.

Fix: say "in a session's own sandbox or container" and name hosts made with Add host beside installed hosts, as the
website does; update the goldens in `crates/gui/farhelm-gui/tests/goldens.txt` that capture this text.

Suggested bucket: `other` (misleading help text; the consequence is the agent asking its usual question). Confidence:
confirmed from source. Related queue item editing the same strings:
`trailing-settings-trust-help-omits-agent-requested-launches.md`, planned in `plans/queue/triage-settings-nov24.md`.
That item is about agent-requested launches, not persistent hosts, so it does not cover this, but the two fixes should
be made together.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g04-disposable-trust-impl` record `f1`.
