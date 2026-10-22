# A stale create retry can take over a session the user has since restarted

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

After an interrupted create and a later ambiguous Restart failure, a stale create retry may treat the session as never
launched and replace its record with the original launch. The transaction permits the advanced generation, but a
complete supported sequence losing a newly stored Restart-with choice or duplicating a surviving agent remains
unestablished. An in-flight Restart is protected by its lifecycle claim.

## Details

Found by the 2026-10-22 prioritized persistent review, group `064-supervisor-db-5of6` (finding f1).

Suggested bucket: highest candidate (a possible duplicate agent, and loss of the session's Restart with launch). The
reviewer expected triage might discard it or treat it as other given the cover below.

Confidence: likely. The code path is verified by reading (reviewer and recording). Not verified: that a same-key retry
arrives after a user restart in practice, and that an ambiguous restart failure can leave its agent alive with no tmux
session.

Mechanism:

- `SessionStore::restart_pending_launch` (`crates/farhelm-supervisor/src/store.rs:3749-3990`) is documented as stricter
  than the service probe (3794-3810). In its transaction it refuses only for a non-empty pane, an outcome outside
  `launching`/`interrupted`/`error`, or a non-NULL `conversation_source` (3858-3863; the reviewer cited 3866-3871). It
  reads `generation` and carries it into the reinserted row as a "launch fence" (3833-3837, 3946-3950), but never treats
  `generation > 0` as evidence that the row is no longer the create's untouched attempt, although only `begin_relaunch`
  advances it (store.rs:4141), on a user restart. That call writes exactly the accepted shape: `launching`, pane `''`
  (4150-4166), and `conversation_source` NULL under `clear_capture` (4167-4169).
- Which restart outcomes leave that shape: a successful restart records its pane (`service/core.rs` around 12012-12017);
  a definitive failure restores the prior pane (store.rs 4238-4273); an in-flight restart holds the lifecycle claim and
  the takeover waits (core.rs 9633-9639). Only an ambiguous restart failure (core.rs 11715-11719, 12036-12038), or an
  abort whose restore write failed (core.rs 11700-11713), leaves the row Launching with an empty pane at
  generation >= 1. `begin_relaunch`'s own doc says ambiguous failures "stay `Launching` for reload to reconcile"
  (store.rs around 4183-4195).
- The service probe `reserved_launch_evidence` (`service/core.rs:8962-9030`) checks only the generation-0 scope and
  sentinel and whether the tmux session exists now; its comment states the assumption outright ("a reservation's session
  is one this create is still trying to launch for the FIRST time"). A same-key retry therefore reads Absent, and the
  takeover returns Acquired. It deletes the row and reinserts the create-time row (core.rs 9573-9605), which reverts a
  stored launch if that differs (store.rs 4764-4792), clears the captured conversation, and relaunches the original
  command. A surviving generation-1 agent would then run beside it; that survivor shape is unestablished.
- Precondition: a Pending reservation on a session the user can restart. TRIAGE_OUTCOMES.md
  "archived-retry-resurrects-session.md" confirms that an interrupted keyed create can leave exactly that.

Relation to the refused-retry fix: TRIAGE_OUTCOMES.md "refused-create-retry-can-delete-restarted-session.md" (fix code,
executed in PR #2570, the `refused-retry-rollback-fence` plan) is the same class (a stale create retry acting on a
session the user has since restarted, with no generation fence) through a different mechanism, the refusal rollback
deleting the row. Its fence (`delete_untouched_retry` in the current main's `store.rs`, checking the reservation is
still pending and the row is at generation zero) covers only that rollback. Its assessment says "the successful retry
path rechecks in a transaction", which this finding contradicts for the generation. Recording checked the current main's
checkout: the takeover's refusal condition in `restart_pending_launch` is unchanged (still pane, outcome and
`conversation_source` only). The maintainer's choice to fix the sibling may bear on whether this one is accepted.

How to verify: in a store test, insert a session under a Pending reservation, call `begin_relaunch`, leave the row
Launching with an empty pane, then call `restart_pending_launch`. It returns Acquired.

Fix sketch: in the takeover transaction, also return `Launched` when `generation != 0`, matching the fence the refused
rollback now uses. Optionally have the service probe do the same.

Reviewer's proposed drop, kept for triage: probably covered by SPEC.md "Lifecycle operations", the paragraph confirmed
2026-10-10 (around SPEC.md:1333-1345). The chain needs two independent rare faults: the interrupted create (a crash at a
specific moment) and an ambiguous restart failure. For coinciding faults like these, the spec accepts a process that
"keeps running beside a relaunch or a new launch" and "that a session's recorded conversation is wrong until the agent
next reports one". What the paragraph does not name is the durable replacement of the session's record: a reverted
Restart with launch and a cleared remembered conversation. That is a lasting wrong record, though not database
corruption.

Possible cover, for triage to judge: SPEC.md "Lifecycle operations", 2026-10-10 confirmation; TRIAGE_OUTCOMES.md
"refused-create-retry-can-delete-restarted-session.md" (sibling, different mechanism, see above); TRIAGE_OUTCOMES.md
"archived-retry-resurrects-session.md" (the same takeover class for archived sessions, closed by removing Archive; does
not cover restarted sessions).

### Later review correction

Additional reviewed commit: `22059381631d91901a44fb70c49dab2f9ab24223`,
`plans/reports/refused-retry-rollback-fence.report.md`, and product source rechecked at
`b228a4e5b850c4dde87f1e661494984a9bb33d7f`. The report's active-Restart overlap is refuted: the lifecycle claim was
moved into the owned task, so takeover waits for Restart settlement. The remaining concern starts after an ambiguous
failure, not while that claim is held.

Restart with records the new stored launch only after successful publication (`store.rs:4765–4787`). An unpublished
failure alone therefore does not prove that takeover reverts a newly chosen stored launch. The original record's
definite claim was too strong. The generation-admitting takeover transaction remains, but its complete
survivor/record-loss chain and exact coincident-fault acceptance remain unresolved. Fresh independent review agreed with
this narrowing, not a whole-item exclusion. Suggested bucket remains provisionally highest; no loss was reproduced.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, run
`4fae501c-3686-4822-aa6c-037266637632`, `g007/f1`, `a006/f1`, `g074` synthesis. No runtime reproduction.
