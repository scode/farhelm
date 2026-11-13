# Failed old-terminal termination still permits Restart

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: after a failure to end an old agent, Farhelm treats it as interrupted and Restart can start another beside it.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g149/c1`.

### g149/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. TLDR: after a failure to end an old agent, Farhelm treats it as interrupted and Restart can start another beside it. Details: end_server logs termination failures and sweep times out without preventing conversion; module lines 61-76 describe this deliberately. Suggested bucket: high for duplicate active agents; could-be-highest: yes, if concurrent agents overwrite user-owned work; that additional loss route is not established here and must remain explicit. Confidence: confirmed source behavior, no execution. proposed_drop: explicit specification acceptance only; no parent drop has been accepted, and an independent check is required before any drop. Exact SPEC.md, Terminal experience, lines 3014-3016: 'A failure to end the server or to stop what ran there is logged and never stops the supervisor from starting; those sessions are treated as ended all the same, accepting that a Restart could then start a second agent beside one still running.' SPEC_impl.md:2999-3001 likewise accepts startup continuation and duplicate agents. possible_cover: parent reports g109/g112 overlap this issue; no coverage sources searched. The strongest original consequence remains duplicate active agents with possible loss of work; rarity is not a lowering basis. This acceptance does not settle a later Delete incorrectly reporting that a surviving old agent was ended; parent identifies that as distinct g128/l1. Synthesis revision changes kind to finding because a could-be-highest drop resting only on specification acceptance needs independent review. Later-revision check: pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the acceptance in SPEC.md:3008-3010 and SPEC_impl.md:2819-2823. old_terminals.rs has only a documentation-link change after introduction; there is no later repair to this startup continuation policy. No evidence here establishes that a subsequent Delete may skip confirmation of surviving-process teardown; do not use this proposed drop to dispose of g128/l1. Discovery-turn interface synthesis before new search: supplied g005/f1, g007/f1, g013/f1 and g109/c1 establish the limiting fresh Restart cleanup at original core.rs:12985-13010: recorded scope, AgentOnly and ScopeKillFailure::Refuse run before relaunch. Current agent and legacy session-only markers are selected. Therefore failed startup cleanup alone does not establish an ordinary duplicate-agent execution; the strongest original concurrent-agent/work-loss concern remains conditional on evading that fresh sweep, with no such ordinary agent route established. This is a confidence refinement, not an accepted lowering/drop. g128/l1 and g007/c4 establish separate subsequent Delete-consent routes, respectively a surviving marked old agent and a mixed host-agent/old-tab session. g111/l1 concerns private-spare traversal reaching a retained borrower after subtree movement. g144/c3 concerns the consumed test Harness directory dying before its outer server guard. g097/l4 concerns stale child PID-to-current-environment ownership reaching another live host tab through marker/scope selectors. None is excused by the duplicate-Restart acceptance and none is treated as new discovery in this group. Their exact supplied records were read; g097/l4 was read from its named log because its neighbour snapshot entry was empty. All no-runtime caveats remain.

Returned proposal (claims-096, event 15): **covered**, suggested bucket **high**.

Both specifications expressly accept startup continuing after failed old-terminal termination, including the possibility of Restart beside a surviving agent. Any drop or consequence lowering needs a separate independent check.

Original stronger consequence: Duplicate active agents could overwrite user-owned work. Later synthesis found the fresh Restart sweep and made that route conditional; this is not an accepted lowering.

Unresolved premise or limitation: Ordinary duplicate execution still requires survivors to evade the fresh refusing Restart sweep; subsequent overwrite/loss of user work is not established.

Proposed basis: Propose coverage only for startup's documented continuation and residual agents. Send the original possible work-loss consequence and refinement through the independent drop/lowering check. Supplied histories mention g109/g112 overlap but exact keys/claims are unavailable here, so no external alias is asserted.

Existing parent disposition (event 15): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g149 record](records/review-through-00de0b6c/groups/g149.json). Record ID: `c1`.

