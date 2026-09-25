# Untitled checkout naming ignores registry-claimed folders

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a checkout folder is removed by hand or a Delete failed partway, untitled creates of that repository fail with a
confusing "nested managed checkout" error.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F9 / COR-UNTITLED-IGNORES-REGISTRY`, tagged **definite**. Anchors and title:
`service/core.rs:4580-4618`, `service/core.rs:7451-7520`, `service/core.rs:2085-2105`, `working_copies.rs:731` —
Untitled checkout naming ignores folders the registry still claims, so preview proposes a name the nested-checkout rule
rejects

When a fresh clone has no title, it is named with the lowest `repo-N` whose folder does not exist on disk right now. The
name is chosen from a directory scan only, without looking at the registry. The nesting rule
`fresh_root_constraint_error` then checks the chosen path against the registry, and refuses any path equal to (or
inside) the path of an active managed checkout.

The two disagree whenever the registry still claims a folder that is no longer on disk:

- The user removed `bar-1` by hand while its session is still listed, so the row is still `allocated`.
- A Delete moved `bar-1` into the archive but failed at a later step, so the row is `archive_pending` and still records
  `bar-1`.

In either case every untitled preview for that repository proposes `bar-1`. Both the preview and `validate_destination`
then refuse it: "…/bar-1 sits inside the managed checkout …/bar-1; nested managed checkouts are not supported". Naming
is deterministic, so this repeats on every attempt until the stale session is deleted.

SPEC.md says an unnamed checkout uses "the lowest available positive `repo-N`". Instead the default flow is blocked, and
the message describes a nesting problem that does not exist. A titled checkout is a workaround, but nothing tells the
user that.

Suggested change: add the basenames of non-retired registry rows under the same root to the set of occupied names that
both the preview and `validate_destination` pass to `checkout_basename`. Keep the nesting rule as a backstop. At
minimum, reword the refusal so it names the session holding the path and suggests using a title.
