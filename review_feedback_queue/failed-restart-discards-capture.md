# A failed restart permanently discards the session's conversation-capture state

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If a restart fails partway (for example tmux is wedged), the session can permanently lose the ability to identify its
conversation — resume is never offered for it again, even though the user did everything right and the restart itself
reported a clean failure.

## Details

Source: pre-pr-review-swarm, area supervisor-lifecycle, 2026-09-17. Confidence: definite. Two lenses found the same
region (correctness-data-flow p1 as definite, correctness-general p1 as possible with a narrower consequence analysis);
coordinator verified the full chain and raised the merged finding to definite. Extended 2026-09-17 with a second trigger
from area stores (four lenses filed the same structural gap; coordinator rejected their stated scenario and verified the
variant below).

A non-`Resume` restart opens its new generation in `begin_relaunch`, which clears the five capture columns
(`first_input_at`, `captured_conversation`, `captured_record`, `capture_ambiguous`, `conversation_source`) in the same
statement (`crates/farhelm-supervisor/src/store.rs:3084-3107`). On a "definitive" failure (nothing was spawned),
`relaunch` calls `abort_relaunch` — but that restores only outcome/exit/annotation/detail/pane/scoped/ archived
(`crates/farhelm-supervisor/src/store.rs:3166-3169`), never the capture columns. The recovered in-memory entry is
likewise rebuilt with `reset_capture=true`, so `relaunched_entry`
(`crates/farhelm-supervisor/src/service/core.rs:2648-2655`) mints `FirstInput { at: None }` + `Unclaimed`, discarding
the pre-restart values.

Why permanent: `first_input_at` is the anchor scan-based capture keys its window on — anchorless sessions are skipped
outright, and capture.rs:43 states "losing the first-input anchor costs capture across a restart". The agent is dead
(the restart's own pre-stop reaped it before the generation was claimed, core.rs:8046-8050), so no future input
re-anchors it. The captured identity itself is safe when the offer is `Resume` (it forces the `Resume` path, which never
clears, and `begin_relaunch`'s atomic basis check closes the race) — but `first_input_at` is not part of that basis
check (store.rs:3066-3069), so a session whose user typed but whose capture was still pending loses its anchor. The
durable ambiguity verdict is lost the same way, against the documented rule that it must survive so a restart cannot
re-decide on thinner evidence. Trigger: tmux wedged (tabs read fails) plus any restart of an uncaptured session.

Second trigger (stores variant): the captured identity is NOT safe when capture is present but the offer is
`FreshOnly`/`FallbackTemplate` — a Pi locator that will not parse, an implausible id, or a missing/placeholder template
(`IntegrationSnapshot::restart_offer`, `crates/farhelm-supervisor/src/agent_kind/mod.rs:1990-2017`). A matching
`Fresh`/`FallbackTemplate` restart clears all five capture columns (`reset_capture=true` for every non-`Resume` mode),
and a definitive failure's abort restores everything except them. Lost: the captured identity value, the ambiguity
verdict, the source, and the first-input anchor. Note what this is NOT: picking `Fresh` against a `Resume` offer is
refused with `Conflict` by `relaunch_argv`'s mode-vs-offer check
(`crates/farhelm-supervisor/src/service/core.rs:2847-2862`, enforced at :7720 before `begin_relaunch` at :7950), so
there is no `Resume`→`Fresh` demotion — the loss is confined to already-unresumable sessions and their capture evidence.

Suggested fix: on the definitive-failure path only, carry the five capture columns through `PriorRun` (extend the
struct; the values are readable where `begin_relaunch` already selects), restore them in `abort_relaunch`'s `UPDATE`
alongside the outcome, and build the recovered entry from those restored values instead of minting a fresh window.
