# On a filesystem without creation times, a re-cloned folder passes as Farhelm's checkout and is trashed

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

If you delete a checkout folder Farhelm made by hand and clone your own copy at the same path, deleting the last session
that used the old checkout can move your new folder into the trash as if Farhelm owned it. A later trash delete then
removes it for good. Farhelm normally tells the two folders apart by their creation time, so this happens only on a host
whose filesystem does not report creation times. The same mix-up also lets a later session in your new folder get the
automatic agent trust that Farhelm gives only to its own checkouts.

## Details

Found by the 2026-10-22 prioritized persistent review, group `043-managed-checkouts-1of2` (finding f1).

What this item covers, and what it does not: the reviewer's finding covered two kinds of row without a recorded birth
time, (a) rows recorded before birth times were kept and (b) rows recorded now on a filesystem that reports none. Case
(a) is the residual the user accepted in TRIAGE_OUTCOMES.md "inode-reuse-defeats-ownership-check.md" ("Decision: the
user agreed with `fix code` using birth time as the reuse-proof discriminator, with the lenient rule for rows recorded
before the change"; completion criteria: "Rows recorded before the change keep today's `(dev, ino)` comparison,
documented as the accepted residual"). That part was sent to the independent reviewer as a proposed drop, not recorded
here. This item is case (b): that decision names only rows recorded before the change, and allocation today still
records no birth time without refusing or warning when the filesystem has none.

Mechanism (line numbers rechecked at the reviewed commit; the reviewer's were about 12 lines early):

- `same_directory` (`crates/farhelm-supervisor/src/working_copies.rs:591-598`, doc 566-590) is the one ownership rule.
  With a recorded birth time it compares inode plus birth time; with none it compares `(dev, ino)` only. Its doc says a
  removed-and-recreated directory "can carry the old `(dev, ino)`" and calls a row without a birth time, "recorded
  before birth times were kept, or on a filesystem without them", "the accepted residual for those rows". That is code
  documentation, not a spec or ledger acceptance, and it is wider than the ledger decision.
- On Linux, `observe` (447-547) leaves `birth_ns` None when `statx` does not set `STATX_BTIME` in its mask (534-541) or
  returns `ENOSYS` (518-531). Allocation stores whatever it observed (`path_birth = target_observed.birth_ns`, 1382;
  root at 1407) with no refusal when it is None.
- That fallback reaches destructive and trust-granting acts: the archive move (`archive_move_with_effects`, 1639; check
  at 1667), the trash's permanent delete through `verify_archived_handle` (701-716; also
  `crates/farhelm-supervisor/src/checkout_trash.rs` around 110-144, per the reviewer), and `cwd_is_managed` (1157 on,
  through `verify_identity_observed`, 1542-1563), which decides that a folder is a managed checkout for workspace trust.
  SPEC.md "Managed checkouts" says the Claude trust choice "never applies to a directory the user selects themselves"
  (around line 940).

Trigger: a managed checkout at `<root>/repo-1` on a filesystem that reports no birth time. The user runs
`rm -rf repo-1 && git clone ... repo-1`; the new directory gets the freed inode number on the same device (the ledger's
assessment saw ext4 do this every time). Deleting the last session that references the row then archives the user's
folder and reports the normal "moved to the trash" result. The trash then shows it as an archived checkout, and its
single-checkout delete removes it without confirmation.

Spec text it conflicts with: SPEC.md "Managed checkouts", "A foreign object replacing the recorded path must remain
untouched" (around line 1103), and "Maintainer-confirmed decisions", "The foreign-replacement guard protects ordinary
replacement, such as a folder removed and recreated or a re-clone" (around line 3517). SPEC.md around 1090 says the
device number is used "only where no creation time is available" without saying replacement detection may fail there.

Which filesystems: unverified. The reviewer's examples are ext4 formatted with 128-byte inodes and NFS where the client
or server does not supply a creation time; whether shipped kernels report birth times over NFS was not checked. SPEC.md
names NFS as supported (around lines 1091 and 3524) and calls only "uncommon filesystems such as FAT" out of band
(around line 3516).

How to verify: on an ext4 filesystem made with `mkfs.ext4 -I 128`, or with `path_birth_ns` set to NULL on a test row:
allocate, remove the directory, recreate it at the same path, confirm with `stat` that the inode was reused, then call
`archive_move` and watch it move the new directory.

Fix options, from the reviewer:

- Treat a row without a birth time as unconfirmed for archiving and use SPEC.md's "release without archiving, with a
  notice" path ("Confirmed 2026-09-28: archiving a checkout never blocks deleting its session", around line 1130), which
  never moves the folder. Do the same for trash deletion and for `cwd_is_managed`.
- Or record extra evidence at allocation when no birth time exists, such as a Farhelm-private marker (an xattr, or a
  file under `.git`) checked before archiving. Directory ctime or mtime would be weak evidence.
- The ledger entry "unstable-device-number-blocks-delete.md" set a simplicity gate for a related case: "where a platform
  cannot supply a usable creation time, refuse with a clear message instead of building further machinery."

Suggested bucket: highest (a user's own folder moved into the trash, where a delete makes the loss permanent). NOTE: for
the same consequence on legacy rows, the user judged the move `high` ("a surprising move of user work, recoverable") in
"inode-reuse-defeats-ownership-check.md"; the reviewer adds that the trash's delete can make it permanent. Triage should
settle the bucket.

Confidence: likely. The code path is confirmed by reading. Unverified: which supported setups report no birth time on
the shipped kernels, and how often inode numbers are reused there.

Reviewer's proposed drop, kept for triage: the code's own doc calls rows without a birth time "the accepted residual";
FILTER.md "Upgrades from older releases combined with another rare condition" may cover legacy rows only; SPEC.md
excludes "uncommon filesystems such as FAT" but names NFS as supported.

Possible cover, for triage to judge: TRIAGE_OUTCOMES.md "inode-reuse-defeats-ownership-check.md" (legacy rows only);
SPEC.md "Managed checkouts" around 1090-1096 (device number only without a creation time, silent on the residual);
SPEC.md "Maintainer-confirmed decisions", "Confirmed 2026-10-11: Farhelm has dedicated ownership ..." (around
3513-3525). The queue item `shared-checkout-root-archives-other-host-work.md` mentions this fallback in passing as
covered by that dedicated-ownership paragraph, but that paragraph names a removed-and-recreated folder as ordinary
replacement the guard protects.
