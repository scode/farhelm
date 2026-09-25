# A pre-mkdir create rollback leaves a phantom membership

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a crash-and-retry during checkout creation, another checkout made at the same path is never archived when its
sessions are deleted and cannot be cleaned up.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F7 / COR-ROLLBACK-PHANTOM`, tagged **possible**. Anchors and title: `store.rs:3146-3158`,
`working_copies.rs:1028-1047`, `service/core.rs:7217-7219`, `service/core.rs:8500` — A pre-mkdir create rollback leaves
a phantom membership in another checkout, so that checkout is never archived

Whether a managed checkout is archived on Delete depends on its membership rows. A membership row links a session to a
checkout it uses, and the checkout is archived only when the last one is removed. `rollback_pre_mkdir_create` (store.rs)
is the undo step for a fresh create whose `mkdir` positively failed before creating anything. It deletes the session
row, but it removes only the session's membership in its own planned checkout. The other session-deletion paths remove
all of a session's memberships.

If the session had meanwhile become a member of a different checkout, that membership survives and points at a session
that no longer exists. That checkout then never reaches zero members and is never archived. Its row stays `allocated`
forever and keeps its path reserved, and nothing in Farhelm can release it.

The reviewer's sequence:

1. Fresh create A records its session S_A (with no canonical cwd, and `cwd = <root>/bar-1`) and a `planned` registry
   row. The supervisor crashes before `mkdir`.
2. Create B previews `bar-1`. The name is free on disk, and `fresh_root_constraint_error` ignores planned rows, so B
   allocates it.
3. During B's allocation, `attach_retained_sessions` adds S_A as a member of B. That function adds every session whose
   recorded cwd lies inside a newly allocated path, and S_A's recorded cwd is `bar-1`.
4. The client retries A with its original key. Keyed retries skip the admission-time nesting recheck (core.rs:8500).
   `allocate(A)` gets `EEXIST`, which is classified as a pre-mkdir failure, so `abandon_fresh_pre_mkdir` runs the
   rollback, and the `(S_A, B)` membership survives.

Suggested change: in `rollback_pre_mkdir_create`, delete all of the session's memberships, with the same last-reference
refusal that `delete_session` applies. Optionally, have `attach_retained_sessions` skip sessions whose
`fresh_checkout_id` points at an unresolved `planned` row, because their cwd is only a candidate, not a directory they
use.

Restater note: Step 4 does not happen as described. A keyed retry of a pending fresh create goes through
`validate_retry` first. For an origin row that is still `planned`, `validate_retry` refuses with a retained error when
anything already exists at the planned path ("… is occupied but its registry row recorded no directory identity …",
core.rs ~7643-7665). Directory admission is held from that check through `allocate`, because `CreateGuards` lives for
the whole of `create_session_admitted`, so B cannot create `bar-1` in between. In the stated sequence the retry keeps
S_A as an Error session instead of rolling it back, which is F8's outcome, not a phantom.

The code fact itself holds: `rollback_pre_mkdir_create` removes only its own membership. But reaching it while S_A is a
member elsewhere requires the planned path to be absent when validation runs and present at `mkdir`. That is possible
only if B's directory was removed by hand and something outside Farhelm recreates the path in that window. Consider
downgrading this to possible/low, or folding it into F8.

Side observation (not verified end to end, not a new finding): in that hand-removed case without outside interference,
the retry's `mkdir` would succeed. That would record a second active row at the same path as B's still-`allocated` row,
because retries skip the nesting recheck and `recover_checkout_destination` does no overlap check.
