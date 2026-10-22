# A Goose run nested inside a Goose session can replace that session's Resume target

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

In a Goose session started by Farhelm, another Goose conversation running underneath the session's own agent can report
itself as the session's conversation. That happens when Goose starts one of its own sub-agents, or when the agent (or a
script it runs) resumes another Goose conversation Farhelm created. Farhelm then saves that other conversation as the
session's Resume target, so a later Restart reopens the wrong conversation and the session's own conversation is no
longer offered. The original conversation still exists in Goose's own store, but the session has lost track of it.

## Details

Found by the 2026-10-22 prioritized persistent review, groups `144-cli-main-2of3` (finding f1) and `145-cli-main-3of3`
(finding f1), merged here as one defect. Group `205-harness-non-first-class-1of2` (finding f2) reported the same defect
from the supervisor side; what it added is under "Added from group 205" below.

Mechanism:

- `farhelm internal goose-hook` (`crates/farhelm/src/main.rs:1477-1505`) reports whenever
  `FARHELM_GOOSE_REPORTER_ENABLED` is `1` in its own environment. It takes the Farhelm session credential from
  `SessionEnv::from_env()` and the conversation id from `AGENT_SESSION_ID`; the payload is built from that id alone
  (`crates/farhelm/src/goose_hook.rs:20-26`). Everything it relies on is inherited environment, and nothing ties the
  report to the Goose process Farhelm launched.
- The supervisor sets the gate and credential on the launched Goose (`crates/farhelm-supervisor/src/agent_kind/goose.rs`
  around 57-70 and 111-126), so every descendant inherits them. Terminal tabs scrub the gate
  (`crates/farhelm-supervisor/src/launch.rs` around 1345-1346); the agent pane's descendants do not.
- Goose persists the `farhelm-reporter` declaration in each Farhelm-created conversation (SPEC.md "Durability and
  resume", around line 2198; SPEC_impl.md "Goose and Pi reporters", around 3042-3049), so a nested resume of such a
  conversation starts the reporter with the outer session's credential and the nested conversation's id.
- Admission has no Goose ownership check: Goose reports use `Slot::Latest`
  (`crates/farhelm-supervisor/src/agent_kind/mod.rs:2108-2110`) and have no foreground-transition vocabulary
  (`foreground_source_refusal`, same file, around 2133). The nested report replaces the stored conversation.
- lore/2026-09-23-goose-session-tracking.md, "The problem", records source research confirming the native sub-agent
  case: "Goose native subagents can inherit that extension and run in the same operating-system process with a different
  session ID. ... A child report can therefore replace the saved restart target unless admission has an independent
  ownership check." So the trigger is not limited to a shelled-out nested resume.

Contrast: Codex (SPEC.md around 2205-2214), Claude (around 2235-2243) and Grok (around 2245-2247) each have a rule that
keeps a nested run from replacing the foreground conversation. Goose has none in SPEC.md, and SPEC.md "Creation" (around
673-676) states why integration is otherwise never switched through the environment: a hook carried there "would reach
runs that are not the session's."

How to verify: start two Goose sessions S1 and S2 through Farhelm. In S1's agent, run
`goose session --resume --session-id <S2's conversation>` (or `goose run --resume`), then check S1's stored conversation
id or its Restart target. For the sub-agent case, have S1's agent use a Goose sub-agent and check the same.

Fix sketch (the reviewers'): pin Goose reports to the session's foreground Goose process, as Codex reports are pinned;
or have the supervisor mint a per-launch nonce in the gate that the report must match; or have the reporter refuse when
it is not under the session's own agent process. The lore note's preserved strict design (tag
`goose-capture-complex-2026-09-23`) also checked Goose's own store for a root session with no parent.

Suggested bucket: highest (resuming the wrong conversation and losing the valid Resume offer). The 144 reviewer leaned
`high` because the original conversation is not deleted; the 145 reviewer said highest.

Confidence: confirmed for the code path by reading, plus the lore note's recorded research for the sub-agent case. Not
reproduced at runtime. Unverified: that Goose starts the persisted reporter on a nested resume with the inherited
environment and `AGENT_SESSION_ID` set to the nested conversation (consistent with how the outer integration relies on
it).

Reviewers' proposed drop, kept for triage: this is a known, deliberately deferred limitation.
lore/2026-09-23-goose-session-tracking.md "Decision": "Keep basic Goose support on `main`, document its child-session
limitation, and defer stronger ownership checks to the per-harness TODO." TODO.md "Maybe later", "Goose foreground
ownership": "Keep the basic Goose reporter for now; it does not distinguish a native or shelled-out child that inherits
the reporter from the foreground conversation. ... Revisit only if reliable child isolation becomes a product
requirement or Goose exposes a direct root/subagent role signal." SPEC_impl.md around 5216-5218 accepts that Goose's
integration is switched through the environment.

Why recording did not drop it: lore and the Maybe later bucket are not coverage sources; SPEC.md never states the
child-session limitation the lore decision said would be documented; SPEC_impl.md accepts the mechanism, not this
consequence. SPEC.md "First-class harnesses" (Confirmed 2026-10-01, after the lore decision) says the allowance for
partial non-first-class integrations "does not cover lost user work (resuming the wrong conversation included)", and
FILTER.md "Rare edge cases in harnesses without first-class support" likewise does not match "resuming the wrong
conversation". If triage confirms the lore decision, what remains is at most stating the limitation in SPEC.md or the
docs and reconciling it with "First-class harnesses".

Possible cover, for triage to judge: lore/2026-09-23-goose-session-tracking.md "Decision"; TODO.md "Goose foreground
ownership" (Maybe later); SPEC_impl.md "Goose and Pi reporters" and around 5216-5218.

Added from group 205 (finding f2), confidence likely, suggested bucket highest:

- The launch controls that enable the reporter (`FARHELM_GOOSE_REPORTER_ENABLED=1` plus the session credential) are in
  the launched Goose's environment (`crates/farhelm-supervisor/src/agent_kind/goose.rs:57-69`), and the reporter is
  injected as a named stdio extension (`goose.rs:51-55`, extension string at 131-132, command-launch path at 100-105).
  Goose reports carry no child marker, so the shared sub-agent refusal (SPEC.md "Durability and resume", around 2242:
  "any report naming a sub-agent is refused for every agent type") never fires for them.
- The trigger is ordinary use of Goose's native sub-agents with hooks on (the default), not only a shelled-out nested
  resume. A plain Restart then composes `goose session --resume --session-id <sub-agent id>` and appends to the
  sub-agent's conversation.
- A further possible cover the earlier reviewers did not cite: SPEC.md "Durability and resume" (around 2278-2279):
  "Goose and Pi retain their existing admission and resume rules; for them the discriminator alone adds no foreground
  protection." Read with SPEC.md "First-class harnesses" ("The conversation-tracking limits under Durability and resume
  apply to every harness, first-class or not; a resume they accept is not the lost user work this allowance excludes"),
  the item turns on whether that sentence accepts this consequence. It does not name it, and it sits in tension with
  SPEC.md "Durability and resume" (around 2172-2175): "restart resumes exactly that conversation ... Claude Code, Codex,
  Goose, Pi, and Grok integrations at this level are required." If triage reads the sentence as accepting the
  consequence, a spec clarification may still be wanted.
- Fix direction from this reviewer, beyond the sketch above: at minimum accept a Goose report only for the first id
  reported in a launch generation plus distinguishable `/new`-style transitions, or withdraw Resume rather than switch
  it when a second id arrives within one launch, so the failure mode is no offer instead of the wrong conversation.

Related: `pi-child-report-replaces-resume-target.md` records the same missing ownership check for Pi, under the same
SPEC.md sentence; triage will probably want to decide the two together.
