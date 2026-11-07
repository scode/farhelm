# Reject uncertain old-terminal consent before Delete sweeps processes

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

When tmux cannot be used, a Delete presented as stopping nothing alive can still kill either an old agent or live
old-tab work beside a migrated, exited host agent. The old-agent variant later refuses; the migrated-host variant can
complete.

## Details

Source coordinates from the original records:
`crates/farhelm-supervisor/src/service/core.rs:14778-14886; crates/farhelm-supervisor/src/service/teardown.rs:478-582,748-795`;
`crates/farhelm-supervisor/src/service/teardown.rs:199-201,754`;
`crates/farhelm-supervisor/src/service/teardown.rs:570-582,782-795`;
`crates/farhelm-supervisor/src/service/teardown.rs:570`;
`crates/farhelm-supervisor/src/service/core.rs:14778-14895;crates/farhelm-supervisor/src/service/teardown.rs:478-582`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **highest**. Source-confirmed conditional behavior does not imply
an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the
finding.

Local synthesis checks confirm the migrated-host route. listing.rs:177-195 supplies an empty tmux map when gates
suppress old queries. An exited usable host agent with no host tabs is shown ended with zero tabs, so the ordinary
sidebar NothingAlive action is the same as f4's old-agent route (GUI references retained in prior f4).
still_alive_for_delete probes the host (core.rs:14796-14823), settles its known hosts (14824-14838), and for
NothingAlive counts old tabs via pane_states_with_markers (14850-14886). Unusable synthesizes NoServer, so that count is
zero. AgentEnded returns earlier at14839-14840 because that guard already authorizes stopping tabs. teardown still
collects tab-unit globs independently at506-524 and calls WholeSession reap at570-582. The portable sweep also claims
every process carrying the session marker (sweep.rs:252-259); tab_environment_with_browser sets the session marker for
tabs (core.rs:15621-15624). After the sweep, a Host runtime skips its unusable tmux session at teardown.rs:754. In the
original f4 old-agent case, the terminal-less branch instead reaches has_session refusal at782-795. The difference is
final outcome (success versus late refusal), not the source of the unconsented kill. Linux tab-scope enumeration alone
suffices to establish the migrated variant without relying on platform-readable marker inheritance.

One centralized early Delete admission policy can address both while needing two explicit cases: (1) refuse a tmux-agent
session on Unusable before any destructive work, using durable terminal kind if the runtime is missing; (2) for a
host-agent session, do not admit NothingAlive while old-tab liveness is unknown on that unusable server, unless absence
can be established independently. Merely adding Restart's !host_agent unusable check to Delete fixes f4/l6 but misses
l4. A blanket refusal of every host-agent Delete is stronger than needed: Unconditional already authorizes all
processes, and AgentEnded authorizes tabs after separately checking the host agent. A richer per-session absence fact or
conservative uncertainty guard can preserve genuinely known host-only cases. These are branches of one early admission
fix and should be reviewed together.

Exact accepted residual remains distinct: SPEC_impl.md:2839-2840 says 'Old tabs of a session whose agent runs in a
terminal host are not looked for in an unusable server, so that session’s Delete does not end them.' That accepts
inability to discover/end those old terminals; it does not override SPEC.md:1672-1677 requiring NothingAlive to refuse
if any tab runs and a refusal to stop nothing. Killing a hidden tab program through its scope exceeds consent even if
its tmux window survives. Do not propose dropping the consent violation as covered by the orphaned-terminal residual.

Suggested bucket: highest, unchanged, because both source-established routes kill user-owned live processes beyond the
guard. Confidence: confirmed conditional source/GUI routes; no measured incident or runtime experiment. proposed_drop:
false. possible_cover: direct duplicate g025/l6; related same-admission variant g025/l4. Preserve f4,l6,l4 IDs for
commissioner consolidation; no log history erased or consequence lowered. f1 stays materially separate: its server is
incorrectly Absent, so an early check only for Unusable cannot protect it.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g031/f4`, `g025/l4`,
`g025/l6`, `g026/l2`, `g032/l1`, `g009/g009-i5`. The retained append-only histories preserve all earlier claims and
corrections. Reviewed source references name their own revisions; the inclusive reviewed tip is the hash above. This
review used source inspection only, with no product runtime experiments or live-install changes.
