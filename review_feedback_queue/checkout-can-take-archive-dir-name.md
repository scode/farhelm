# A fresh checkout can take the reserved archive directory name

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

One unlucky checkout title makes later Deletes move other repositories inside it, and its own session can never be
deleted or restarted ("Invalid argument" on every Delete).

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F1 / COR-ARCHIVE-NAME`, tagged **definite**. Anchors and title: `service/core.rs:2085-2121`,
`working_copies.rs:764-770`, `crates/farhelm-proto/src/github_checkout.rs:328-353`, `service/core.rs:4580-4618`,
`service/core.rs:7451-7518` — A fresh checkout can be created under the reserved archive name
`farhelm-archived-working-copies`

Farhelm can start a session in a fresh GitHub clone that it creates itself (a "managed checkout"). The clone goes into a
checkout root the user configured, named `<repo>-N` when the user gives no title, or `<repo>-<slug>` when they do. The
slug lowercases the title and turns every run of characters other than letters and digits into one hyphen. When the last
session using a managed checkout is deleted, Farhelm does not delete the clone. It moves it into a fixed subdirectory of
the same root, `farhelm-archived-working-copies` (`ARCHIVE_DIR_NAME`), under a timestamped name.

Nothing reserves that subdirectory name for new checkouts, and ordinary titles produce it exactly:

- repo `farhelm` with the title "archived working copies"
- repo `farhelm` with the title "farhelm-archived-working-copies" (a title that already starts with the repo name does
  not get the prefix twice)
- repo `farhelm-archived` with the title "working copies"
- repo `farhelm-archived-working` with the title "copies"

Two checks could catch this, and in the common case neither does:

- The directory scan that decides which names are taken (`occupied_related_names`) skips any entry with the archive name
  on purpose. Both the preview and the create-time recheck (`validate_destination`) use this scan.
- The nesting rule `fresh_root_constraint_error` does refuse a path inside the archive directory. However, it only
  computes that archive path from existing registry rows that are active and already have a directory. The check sits
  inside a loop over those rows, after a `continue` that skips rows without a recorded path. In a root with no such row,
  nothing refuses the name and the clone's `mkdir` succeeds. That covers the first checkout ever created under a root,
  and a root whose earlier checkouts have all been archived.

Three things go wrong once such a checkout exists:

- **Other checkouts get moved into the user's clone.** When the last session of any other managed checkout in that root
  is deleted, `ensure_archive_root` finds a real directory on the same filesystem at the archive name, and accepts it.
  The overlap guard only compares the checkout being moved against other active rows, so it sees no problem. The
  checkout is renamed into the colliding repository's working tree. It now shows up there as an untracked subdirectory
  of an unrelated repository.
- **The colliding session can never be deleted.** Deleting its last session first marks the registry row
  `archive_pending`, which records the planned move. It then tries to rename the directory into a subdirectory of
  itself. `renameat2(RENAME_NOREPLACE)` rejects that with `EINVAL`; three reviewers confirmed this with throwaway calls.
  The rename helper counts only `EEXIST`/`ENOTEMPTY` as a name collision, so `EINVAL` surfaces as a plain error. The row
  stays `archive_pending`, and every Delete retry and every startup recovery repeats the same failing rename. While the
  row is in that state, `restart_session` refuses to restart the session, and `refuse_pending_archive` refuses new
  sessions in that directory.
- **The preview can loop on a name that cannot be created.** Suppose the archive directory already exists from earlier
  deletes and every registry row in the root is retired. The preview still proposes the name: the scan skips it, and the
  nesting rule has no row to work from. Launch then fails at `mkdir` with a conflict that tells the user to get a new
  preview, and the new preview proposes the same name again.

No attacker is needed; the trigger is ordinary title text. The outcome breaks two SPEC.md promises: only the final
reference moves the recorded checkout into the archive, and an operation "must not accidentally affect the wrong
object".

The fix is to reserve the name no matter what the registry holds. Either `checkout_basename` or both the preview and
`validate_destination` should refuse a planned basename equal to `ARCHIVE_DIR_NAME`. The occupancy scan should also
report that name as taken when it is the candidate. Add a test that uses a root with no registry rows, since that is the
case the existing guard misses. F2 is the companion fix for databases that already contain such a checkout.

Restater note: The reviewer text says archived checkouts inside the colliding clone could be destroyed by
`git clean -fdx` or committed by `git add -A`. Stock git does neither for a directory that has its own `.git`:
`git clean` skips nested repositories unless `-f` is given twice, and `git add -A` records a gitlink (with a warning),
not the files. The realistic risk is that the user deletes or moves the colliding clone by hand and takes every archived
checkout inside it along. The finding's substance and severity otherwise hold against the code.
