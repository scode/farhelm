# Detached recipe write crosses recreated generation

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

A detached database write can outlive its recipe generation and update a later recipe recreated with the same name.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/tensorlake/build.rs:843-847,997-1001 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g068/d2, event 31):

Discovery pass1 NEW verified source-level ordering defect. Reviewed commit 744ea4122d033ef616454fe592ecb2173593736c.
TLDR: deleting a recipe while its interrupted build is being reconciled, then recreating the same recipe name, can leave
the new recipe showing cleanup for a builder already deleted; a delayed old write can also replace its new builder
bookkeeping. Mechanism: clean_leftovers passes the entire reconcile_builder future to cancellable remote (843-847).
Reconcile awaits record_builder after finding a name match (997-1001). record_builder writes builder_sandbox_id and
clears pending_create_at unconditionally by recipe NAME (957-964); update_tensorlake_recipe_state reads the row only
when its blocking closure executes (304-338). Necessary local dependency farhelm-supervisor/src/db.rs:230-244 implements
call with spawn_blocking and std::sync::Mutex, not a FIFO database actor: dropping its JoinHandle await detaches work
and supplies no drain/ordering guarantee. Interleaving: old record closure has been dispatched but not acquired the
mutex; DELETE sets cooperative cancel; remote drops reconcile; run awaits its final build_started_at-clear (699-707),
then ends; delete's own reconciliation records/sweeps old builder, finishes row deletion; user recreates same name
through create_tensorlake_recipe (recipes.rs:323-356); the previously dispatched closure finally acquires the mutex and
applies old builder ID to the replacement row. Per-connection mutual exclusion does not prevent this ordering, and no
row generation/build operation token is checked. This does not require HTTP cancellation: owned DELETE deliberately
cancels the build. Existing pending markers prevent deletion without reconciling old resources, but do NOT fence a
detached stale write after reconciliation; ID-equality guards in sweep_builder protect clearing a different ID, but do
not constrain record_builder's unconditional assignment. Suggested bucket other for confirmed stale/new-recipe cleanup
state and broken lifecycle bookkeeping; no established permanent billing leak, security consequence, process loss or
unique-work loss is claimed. Confidence confirmed source interleaving, no runtime reproduction under source-only
charter; thread scheduling is the trigger, not an unavailable vendor premise. Checks: original build/store
implementation and Db::call; through-tip db.rs diff changes only a new separate full-sync helper and leaves call
unchanged, while assigned build diff leaves these wrappers unchanged. SPEC_impl.md Tensorlake snapshot builds and module
docs promise cancellation never cancels state writes and deleting waits for created-resource bookkeeping; the outer
wrapper violates the intended drain boundary. Fix: cancel only remote API waits/sleeps, or ensure all dispatched state
writes have completed before build task exits; alternatively generation-check writes as defense, including same-name
recreation. proposed_drop false; possible_cover unknown, parent handles coverage. Caveat: destructive/billing extensions
need their own complete route; the established defect is stale state applied to a later recipe lifecycle. Original l1
missing-row refutation remains narrow: it handles a deleted row at execution time, not a replacement row with the same
name.

Later qualification (review record g068/d2, event 32):

Evidence delta to complete d2 finding: Cargo.lock:6277-6280 pins tokio1.53.2. Read exact cached
<home>/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tokio-1.53.2/src/runtime/task/join.rs:18-23,35-36: dropping
JoinHandle detaches the task and it continues; task/blocking.rs:106-120 describes non-abortability of started blocking
work. runtime/blocking/pool.rs:408 enqueues FIFO and :517-520 pops a task but RELEASES the pool lock before task.run.
Correction/qualification: Tokio's dispatch queue itself is FIFO; the defect is NOT queue reordering. Multiple blocking
workers can dequeue in order but reach Db's connection mutex out of order, e.g. old worker descheduled after dequeue and
before lock, later workers complete cancellation/end/reconcile/delete/create operations first. Per-connection std Mutex
exclusion does not drain that earlier worker or generation-check its future write. This establishes the delayed-closure
premise with the exact dependency, without runtime/vendor evidence. Original update_tensorlake_recipe_state:309-338
reads the current same-name row when old closure finally runs, and record_builder:959-962 applies the obsolete ID
without generation. SPEC_impl.md:5795-5798 promises delete waits for build cancellation and state writes are not
cancelled; outer remote cancels the wait and detaches such a write. Through-tip Db::call is unchanged (only a separate
call_full_sync_if helper was added); build wrapper/record assignment unchanged. Suggested bucket other, confidence
confirmed source interleaving, proposed_drop false, possible_cover unknown; NEW verified discovery ID d2. Scope remains
stale cleanup/new-recipe bookkeeping; permanent billing/work-loss extension not established or claimed.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g068/d2`; supporting records
`g047/interface-g068-d2`, `g065/g068-d2`, `g063/g068:d2`, `g071/c15`. The retained private log preserves earlier wording
and its corrections. This record does not claim runtime reproduction.
