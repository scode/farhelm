# Trust hint in a disposable session points to a switch that does not apply there

Reviewed commit: cc9bec6cf70ab293d6f8b2c2115bc170f69c8d01

## TLDR

When an agent in a session's own sandbox or container still asks its folder-trust question, the hint above the terminal
says Farhelm's answer is "off by default" and offers Settings. Since this commit those sessions get trust automatically
and the Settings switch covers only installed hosts. A user who follows the hint and turns the switch on sees no change
in that sandbox and gets no explanation of why the agent asked.

## Details

cc9bec6cf7 made new launches on session-owned hosts trust regardless of Settings
(`crates/farhelm-helm/src/workspace_trust.rs:142-152`) and rescoped every switch to installed hosts (SPEC.md "Managed
checkouts", ~1853 and ~1894-1903). It did not touch the band's show rule. At the tip, `Facts::shows`
(`crates/gui/farhelm-gui/src/session_pane/trust_band.rs:90-96`) shows the band whenever the question is up, the switch
reads off and the hint is not dismissed, with no host condition. The words come from `TrustAgent::band_words`
(`settings/agent_trust.rs:348-384`): Codex/Muse "It's off by default because repository hooks, settings and commands are
then trusted", Antigravity "Farhelm can skip this question ... It's off by default", Claude "Farhelm can answer it for
you, but it's off by default".

The question can still appear in a disposable session: a plain Restart of a session whose stored launch has no trust (it
keeps the stored choice, `sessions.rs:4829`), Antigravity with Custom `--settings` (which takes precedence), or Claude
in the sandbox home when the recipe selects a Claude config file setup did not seed (setup seeds only `~/.claude.json`,
`credential_delivery.rs:911-938`, while the launch check reads the selected config). In each case the switch cannot
change the outcome but the band says it would.

Fix: hide the band, or use different words, when the session's host is session-owned; the host listing already carries
`is_session_owned` (`hosts.rs:278`). SPEC.md "Managed checkouts" (~1904) states the show rule without a host condition,
so the spec needs the same update.

Suggested bucket: `other` (misleading guidance; the agent asks its usual question). Confidence: confirmed that the band
has no host condition and that its words contradict the new policy; the routes to the question in a disposable session
come from source, not observation. Possible cover: SPEC.md "Managed checkouts" specifies when Claude's hint is offered
("for a structured Claude launch or a session created from a fresh managed checkout") but not that its words fit
disposable hosts, and the Codex/Muse/Antigravity paragraph (~1904) has no host condition. Reviewer `g01` raised the
Claude part separately (its `c6`); after reconciliation both reviewers agree it is this defect and that the sentence
covers only when the hint is offered.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g04-disposable-trust-impl` record `f7` (duplicate: `g01-hint-bands-hooks` `c6`).
