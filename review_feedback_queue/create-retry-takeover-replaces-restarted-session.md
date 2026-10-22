# A stale create retry can take over a session the user has since restarted

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

A retry of an interrupted create may take over a session that has since been restarted, potentially duplicating a
surviving agent. Losing a newer Restart with choice requires a narrower overlap and failure sequence: the retry must
read the old choice before an already-started restart saves the new one, and the session's durable record must still
look unlaunched. That complete sequence has not been established in practice.

For a managed-checkout retry, the reverse mismatch is also possible under additional failure conditions: recovery may
show or reuse approval for the original launch while executing the newer launch now stored on the session. Practical
reachability and security impact remain unverified.

## Details

Originally found by the 2026-10-22 prioritized persistent review, group `064-supervisor-db-5of6` (finding f1), against
`08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7`. Reassessed by `A23-fingerprint`, `A23-reconciliation` (f1),
`A23-pending-retry` (f1), the recording agent and an independent disposition reviewer against the reviewed commit above.

Suggested bucket: highest candidate (a possible duplicate agent, and loss of the session's Restart with launch). The
reviewer expected triage might discard it or treat it as other given the cover below.

Confidence: likely mechanism and conditional impact. Same-key production retry, adverse ordering, missing launch
evidence and survival of later work remain unverified. No runtime reproduction was performed in this reassessment.

Mechanism:

- `SessionStore::restart_pending_launch` (`crates/farhelm-supervisor/src/store.rs:3749-3990`) is documented as stricter
  than the service probe (3794-3810). In its transaction it refuses only for a non-empty pane, an outcome outside
  `launching`/`interrupted`/`error`, or a non-NULL `conversation_source` (3858-3863; the reviewer cited 3866-3871). It
  reads `generation` and carries it into the reinserted row as a "launch fence" (3833-3837, 3946-3950), but never treats
  `generation > 0` as evidence that the row is no longer the create's untouched attempt, although only `begin_relaunch`
  advances it (store.rs:4141), on a user restart. That call writes exactly the accepted shape: `launching`, pane `''`
  (4150-4166), and `conversation_source` NULL under `clear_capture` (4167-4169).
- A successful durable confirmation normally records its pane and prevents takeover. An in-flight restart holds the
  lifecycle claim, and takeover waits (`service/core.rs:9642-9662`). Ambiguous failures or failed restoration can leave
  an eligible row; a fresh-terminal confirmation write failure can also publish a live run as Launching without saving
  its pane (`12021-12090`). These paths have different implications for whether a newer launch choice was saved.
- The service probe `reserved_launch_evidence` (`service/core.rs:8962-9030`) checks only the generation-0 scope and
  sentinel and whether the tmux session exists now; its comment states the assumption outright ("a reservation's session
  is one this create is still trying to launch for the FIRST time"). If those probes report absence, takeover can
  acquire and replace an advanced-generation row. The replacement uses the retry's validated snapshot, not necessarily
  the original create command. A later agent surviving outside the probed evidence could be duplicated.
- Precondition: a Pending reservation on a session the user can restart. TRIAGE_OUTCOMES.md
  "archived-retry-resurrects-session.md" confirms that an interrupted keyed create can leave exactly that.

The earlier report overstated which launch the retry restores. `validate_retry` reads the **current** row
(`service/core.rs:8494-8505`) and builds its launch from that row (`8638-8699`); agent retries additionally compare
newly approved inputs (`8520-8537`). A Restart with choice saved before validation can become the retry's launch. The
original request fingerprint does not imply execution of the original command.

Overwriting a newer choice requires it to change after that snapshot. Create holds directory admission through
settlement (`service/core.rs:1974-1983,7249-7286`). Restart takes directory admission before lifecycle, then releases
directory admission while retaining lifecycle (`11016-11080`). An already-admitted Restart can overlap retry validation;
a fresh Restart cannot enter after retry has taken directory admission. The retry waits for lifecycle only at takeover
and does not repeat the earlier physical probes or launch snapshot there.

Restart with saves replacement settings only after publication (`service/core.rs:11595-11644`; `store.rs:4760-4792`). An
ordinary ambiguous pre-publication failure does not save them. A narrower source-visible sequence is a fresh-terminal
confirmation write failing, publication of the live terminal as Launching (`service/core.rs:12021-12090`), then a
successful replacement-launch write. If retry's probes and snapshot preceded publication, takeover can later see the
durable empty-pane row while carrying the older launch. This needs transient confirmation failure, successful later
launch persistence, absent conversation evidence and no intervening durable confirmation. It remains an unverified
sequence. Probes before spawn do not imply tmux remains absent after spawn; a retry beginning after publication would
instead need physical evidence to be absent then.

Managed-checkout reconciliation enters the same recovery path (`service/core.rs:7662-7694,7731-7757,8806-8881`), but
Restart requires allocated provenance, the accepted directory identity and Ready preparation (`11037-11075`). An
ordinary pre-allocation interrupted create cannot establish this overlap. A reservation must remain Pending despite
reaching those prerequisites before the later restart and retry. That managed-origin sequence remains unverified.

The starting coexistence of Ready preparation and a Pending reservation has a source-supported path without an initial
crash: preparation writes Ready after clone/hook (`launch.rs:1854-1867`), while a successful create treats final
reservation-settlement failure as nonfatal (`service/core.rs:9389-9405`). This establishes a possible starting state,
not the complete later takeover sequence. Evidence-bearing reload or retry ordinarily settles that reservation.

An additional consent consequence was found by `A24-restart` (f1), independently checked, and consolidated here. Managed
recovery inspection returns the fingerprint's original launch (`service/core.rs:7599-7623`). The helm checks that
launch, uses it in any required wrapper card, and checks it again after approval
(`crates/farhelm-helm/src/agent_requests.rs:1802-1937`). A new card is conditional; already retained approval can be
reused. Recovery verifies the exact inspected fingerprint (`service/core.rs:7572-7579`) and supplies its launch to
create recovery (`7662-7694`). However, `validate_retry` selects the current row's launch, and its comparison with
approved inputs applies only to ordinary agent-request fingerprints (`8521-8539`), not this managed fingerprint.
Fingerprint equality therefore does not prove equality with a changed session launch.

This direction differs from overwriting a newer choice: the changed launch can already exist before validation. It still
needs a Pending reservation, allocated identity-matching Ready preparation, a takeover-eligible later row, and absent
evidence at both inspection and launch-capable recovery. Ordinary successful publication blocks takeover; ordinary
pre-publication failure does not save new settings. A later ambiguous relaunch or failed restoration after settings
changed is a proposed route, not a demonstrated production sequence. Pre-spawn probes from the earlier overlap variant
must not silently substitute for this serial scenario's missing physical-evidence premise.

Suggested bucket for this extension: highest candidate, conditional on executing a launch different from the approved
one. Confidence: the two launch authorities are established by source; full supported execution and practical harm are
unverified. SPEC_impl.md:5446-5461 and 5626-5641 require original recovery and approval meaning. SPEC.md:1333-1345
accepts specific compounded-fault process and conversation effects but does not expressly accept a different approved
launch. Coverage remains for triage. The independent reviewer agreed to retain this substantive extension, not discard
it as an exact duplicate. No additional queue file or runtime reproduction was requested.

Relation to the refused-retry fix: TRIAGE_OUTCOMES.md "refused-create-retry-can-delete-restarted-session.md" (fix code,
executed in PR #2570, the `refused-retry-rollback-fence` plan) is the same class (a stale create retry acting on a
session the user has since restarted, with no generation fence) through a different mechanism, the refusal rollback
deleting the row. Its fence (`delete_untouched_retry` in the current main's `store.rs`, checking the reservation is
still pending and the row is at generation zero) covers only that rollback. Its assessment says "the successful retry
path rechecks in a transaction", which this finding contradicts for the generation. Recording checked the current main's
checkout: the takeover's refusal condition in `restart_pending_launch` is unchanged (still pane, outcome and
`conversation_source` only). The maintainer's choice to fix the sibling may bear on whether this one is accepted.

A synthetic advanced-generation row accepted by the store would demonstrate the missing fence, not the complete
production consequence. No tests or reproductions are requested by this review.

Fix sketch: in the takeover transaction, also return `Launched` when `generation != 0`, matching the fence the refused
rollback now uses. Optionally have the service probe do the same.

Reviewer's proposed drop, kept for triage: probably covered by SPEC.md "Lifecycle operations", the paragraph confirmed
2026-10-10 (around SPEC.md:1333-1345). The chain needs two independent rare faults: the interrupted create (a crash at a
specific moment) and an ambiguous restart failure. For coinciding faults like these, the spec accepts a process that
"keeps running beside a relaunch or a new launch" and "that a session's recorded conversation is wrong until the agent
next reports one". What the paragraph does not name is the durable replacement of the session's record: a reverted
Restart with launch and a cleared remembered conversation. That is a lasting wrong record, though not database
corruption.

The independent reviewer agreed to consolidate the A23 candidates only with the current-row, publication, locking and
managed-origin qualifications above. The existing potential-highest finding is retained. SPEC_impl.md:5425-5434's
stored-row retry rule supports the correction; it does not settle generation-blind takeover. No Planned item or known
accepted bug covers the unresolved consequence, and the rare-failure filters exclude loss of user-owned work.

Possible cover, for triage to judge: SPEC.md "Lifecycle operations", 2026-10-10 confirmation; TRIAGE_OUTCOMES.md
"refused-create-retry-can-delete-restarted-session.md" (sibling, different mechanism, see above); TRIAGE_OUTCOMES.md
"archived-retry-resurrects-session.md" (the same takeover class for archived sessions, closed by removing Archive; does
not cover restarted sessions).
