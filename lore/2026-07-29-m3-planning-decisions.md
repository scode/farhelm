# M3 planning: the calls that could have gone the other way

Decisions made 2026-07-29 while planning M3 in detail (PLAN_M3.md), recorded here because each had a defensible
alternative and the plan text alone doesn't show the fork.

**Retained exit codes beat SPEC.md's letter.** SPEC said an exit during supervisor downtime "shows as exited with
unknown code". Review pushed to enforce that literally — always unknown, even when the surviving dead pane still holds
the true code. Decided the other way: the sentence assumed unknowability rather than mandating ignorance, and the
no-guessing rule distinguishes retained knowledge from invention. SPEC.md was sharpened (durability and status
sections) instead of the implementation dumbing down what tmux genuinely knows.

**Fresh launch, not a dressed-up "fallback resume".** With profiles deferred to M5, an uncaptured session has no
user-authored resume invocation to fall back on. The tempting move was to offer the launch invocation labeled as the
fallback resume — it even reads like SPEC's "may land in the agent's own picker" language. Rejected: relabeling a
launch as a resume is exactly the kind of comfortable lie the status rules forbid elsewhere. Until M5 gives users the
field, the honest offer is a fresh launch, labeled as such.

**Idempotency outcomes tombstone; they don't die with the session.** Deleting the key with its session (one reviewer's
preference, arguably SPEC's spawn-key lifetime rule) reopens a duplicate window: retry racing delete re-creates. A
replay for a deleted session instead returns an explicit gone-error — never a live-looking success with a dead id,
never a duplicate. Interactive-create keys are their own namespace; M7's spawn keys will define their own lifetime.

**Interim agent-kind recognition is deliberately dumb.** Basename-of-first-token recognition misses `env claude` and
wrapper scripts. The alternatives — wrapper-aware heuristics, or a full early profile system — were rejected as
guessing and scope creep respectively. Instead: dumb defaults plus optional explicit overrides on the wire, no UI
surface, with M5's profiles as the real fix. One invariant guards the seam: an integrated kind must carry
`{conversation}` in its template, so capture can never silently discard a captured identity.

**Stop annotations landed in M3, not M5.** They render like status (M5's ground) but their defining property is
durability — surviving restart and reboot on the session's own record — and M3 is where the durable last-known-outcome
record gets built. Adding them later would have meant reopening that record's schema one milestone after building it.
