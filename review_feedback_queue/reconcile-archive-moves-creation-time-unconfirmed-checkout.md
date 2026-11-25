# reconcile_archive completes a move without the creation-time-unconfirmed refusal

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

A checkout whose filesystem records no creation time — which Farhelm is supposed to leave in place and never archive —
can still be moved to the trash when an older interrupted archive is recovered at startup or on a Delete retry. The
folder is displaced into trash (recoverable), not permanently destroyed.

## Details

**other.** Source inspection; commissioner confirmed both code paths. Established consequence: spec-deviant archival of
an unconfirmed checkout, recoverable in trash. The permanent-loss/highest route needs a legacy journal + inode reuse +
re-clone and is unestablished; would move up if shown. Not highest as established: trash never permanently deletes an
unconfirmed checkout (verify_archived_handle refuses CreationTimeUnconfirmed, used at checkout_trash.rs:241 and :890),
so the folder is displaced/recoverable, not destroyed.

`crates/farhelm-supervisor/src/working_copies.rs:1730` — `archive_move` refuses an unconfirmed row
(`if creation_time_unconfirmed(row.path_birth_ns, source_observed.birth_ns) { return Err(CreationTimeUnconfirmed) }`)
BEFORE journaling (persist_journal at 1738). `reconcile_archive_with_effects` (2143-2305) re-checks source identity at
:2213 with `same_directory(&observe(&source)?, expected, row.path_birth_ns)` but has NO creation_time_unconfirmed
refusal, then renames in the (true,None)/(true,_) arms (e.g. 2233-2245). For recorded-birth None + observed-birth None,
`same_directory` (607-615) falls back to dev+ino, which a re-clone at the same path can reuse (module docs note ext4
commonly reuses directory inodes). Such a row can be archive_pending only from a pre-2026-10-22 supervisor (current
archive_move refuses before journaling), so this needs a legacy journal + upgrade; if the folder was re-cloned with a
reused inode, the user's new folder is moved to trash.

Verify: grep reconcile_archive_with_effects for creation_time_unconfirmed (absent); compare archive_move:1730. Fix
shape: add the same unconfirmed refusal to reconcile's source-present rename branches (skip/leave-in-place rather than
rename), mirroring archive_move.

Reviewer metadata (area 10 chunk A, managed checkouts): suggested bucket: other; confidence: confirmed code gap, likely
route (premise: a pre-rule supervisor left an archive_pending journal on a None/None row); proposed_drop: none;
possible_cover: SPEC.md "Managed checkouts" Confirmed 2026-10-22 (unconfirmed rule) and SPEC_impl "Owned checkout
admission and lifetime" (interrupted-archive recovery) — neither clearly settles recovery of an unconfirmed row (the
legacy-row acceptance covers None/Some, not None/None recovery). Commissioner validation: both paths confirmed at the
cited lines; coverage searched (queue, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue) — no match.
