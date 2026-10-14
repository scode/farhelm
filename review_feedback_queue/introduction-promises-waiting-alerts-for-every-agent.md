# Introduction promises waiting alerts for agents without detection

Reviewed commit: 9f759eb6a62ee475e8a83563e0b6aa7c1e6c00a1

## TLDR

Users are told blocked agents turn red and ring a bell, but several explicitly supported agents silently appear idle
instead.

## Details

Suggested bucket: `other`. Confidence: confirmed. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed
drop: none.

website/scripts/render-svgs.mjs:217–221 introduces this claim, shipped in both pillars SVGs:41–42 and displayed by both
introductions. Only Claude and Codex have waiting readers; Goose, Pi, OMP, Grok and Generic use a reader that can return
only Working or Idle (crates/farhelm-supervisor/src/agent_kind/screen_reader.rs:204–229). Waiting sounds require a
transition to Waiting (crates/farhelm-ui/assets/sounds.js:27–38). Thus an ordinary approval prompt from those other
agents cannot produce the advertised red status or waiting bell, even with sounds enabled and the session in view. Scope
the claim to Claude and Codex; distinguish optional sounds from guaranteed alerts.

Possible cover: SPEC.md, Status (:1140–1147, :1210–1226), explicitly limits detection. The adjacent statement that
support varies is less specific than this unconditional alert promise. Ledger notification-bell entries concern tracking
notifications and do not match this trigger.

Current-main status: Persists at pinned main: render-svgs.mjs:217–221 and screen_reader.rs:204–229 remain unchanged;
current SPEC.md:1233–1240 still limits Waiting to Claude and Codex. No fixing commit identified.

Review coverage: full commit diff and relevant code paths read. Runtime behavior and harmful reliance were not
reproduced; the note preserves the reviewer's stated unverified premises.
