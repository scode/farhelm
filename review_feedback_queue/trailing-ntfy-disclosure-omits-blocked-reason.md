# SPEC's network disclosure and v1 non-goals were not updated for blocked-agent ntfy alerts

Reviewed commit: 721dd96fe7315659b661faa0ede8656d5f4192a1

## TLDR

A user reading what Farhelm sends over the network is told the ntfy connection carries waiting alerts and test alerts
with session titles and host names. With the new Agent is blocked switch on, it also carries whatever reason the agent
wrote (up to 500 characters of agent-composed text, which can name files, commands, error output or anything else the
agent saw). The non-goals list and the Notifications paragraph also still say the only OS-reaching notifications are
ntfy waiting alerts.

## Details

SPEC.md:5485-5487 ("When ntfy alerts are configured, the helm also posts waiting alerts and explicit test alerts to the
saved server and topic ... This connection sends session titles and host display names") is the outbound-connections
disclosure. 721dd96fe7 added a second alert kind (crates/farhelm-helm/src/ntfy.rs:318-344, AlertKind::Blocked, body
"<title> on <host> is blocked: <reason>"; queued from crates/farhelm-helm/src/agent_requests.rs:1356) that posts the
agent's reason to the same server, and changed SPEC.md elsewhere (3443-3447, 4894-4900) but not this disclosure. The
reason is only cleaned of presentation-unsafe characters and refused above 500 characters (agent_requests.rs:1298-1318);
it is not limited to titles/host names, and the waiting paragraph's promise at SPEC.md:3450-3451 ("no screen contents,
prompts, command lines or other paths are added") does not hold for it, since the agent decides the text. SPEC.md:3525
("Other than ntfy waiting alerts, desktop or operating-system notifications are not part of v1") and SPEC.md:5567
(non-goal: "Desktop or operating-system notifications other than configured ntfy waiting alerts") are likewise stale.
SPEC.md:3444 also says the blocked alert follows the waiting alert's "character rules", whose stated cap is 200
characters per field, while the reason's limit is a 500-character refusal. Verify by reading those lines against ntfy.rs
AlertKind::body. Fix: extend the network disclosure to say blocked alerts send the agent's reason (agent-written text
that may include anything the agent chose to say), and update the two 'waiting alerts only' statements and the
character-rules sentence.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (documentation/disclosure drift; no code defect, but it is a privacy disclosure the
  user relies on when choosing a public ntfy.sh topic).
- Confidence: confirmed.
- possible_cover: none known
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g19/f1 (finding), group g19 "ntfy waiting alerts: helm delivery", findings log line 1.
