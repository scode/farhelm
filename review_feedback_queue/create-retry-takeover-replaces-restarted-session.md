# A stale create retry can take over a session the user has since restarted

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

A create can be interrupted, for example by a supervisor crash. Its session then shows up never launched, and the
create's request stays open for a retry. If you restart that session and the restart fails ambiguously with its terminal
gone, a later retry of the original create treats the session as never launched. Farhelm then throws away the session's
current record, including a Restart with choice and any remembered conversation, and starts the original create's
command in it again. If the failed restart left an agent running in the background, that agent is duplicated.

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
  Restart with launch (store.rs 4764-4792), clears the captured conversation, and relaunches the original command. An
  agent the failed restart left running in its generation-1 scope keeps running beside it.
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
