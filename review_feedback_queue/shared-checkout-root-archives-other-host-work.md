# A checkout root shared between hosts lets one host trash a checkout another host's session is working in

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

When two hosts see the same checkout folder through a shared network home (for example an NFS home mounted on both, with
the all-hosts checkout folder pointing into it), a session on the second host can work inside a managed checkout the
first host created. The first host does not count that session as using the checkout. Deleting the first host's last
session in it moves the folder, with the second host's in-progress work, into the first host's trash, out from under the
running session. The first host's trash then offers to delete it permanently: its "a session still uses this" protection
does not see the other host's session, and the single-checkout delete button asks for no confirmation. Uncommitted work
done on the second host can be lost.

## Details

Found by the 2026-10-22 prioritized persistent review, group `044-managed-checkouts-2of2` (finding f1).

Suggested bucket: highest (possible loss of uncommitted user work). The reviewer notes that the loss needs the user's
own trash delete, which is how the spec defines the trash's permanent cleanup, and that triage should first confirm
whether a checkout root shared between hosts is meant to be supported at all; if it is not, this becomes a spec
clarification.

Confidence: likely. Unverified premises: that two supervisors sharing one checkout root on a network filesystem is a
supported configuration (the spec neither supports nor forbids it), and that no other layer (helm or supervisor) refuses
a session on host B in a folder that is host A's managed checkout; the reviewer found none in its scope. Recording
confirmed by reading that the archive move and the trash delete consult only the local supervisor's own records.

Mechanism:

- Lifetime membership, and so the "last reference" decision, is per supervisor: `working_copy_members` rows live in each
  host's own supervisor database (`crates/farhelm-supervisor/src/store.rs` 3440-3519, where Delete counts remaining
  members with `member_count`; `crates/farhelm-supervisor/src/service/teardown.rs` 1000-1128 then calls the archive
  move). A session on another host whose working directory is inside the checkout has no row there.
- `archive_move_with_effects` (`crates/farhelm-supervisor/src/working_copies.rs:1639-1699`) verifies only that the
  folder is still the one this supervisor created (inode plus creation time, `same_directory` at 591-598), which it is,
  and then renames it into `<root>/farhelm-archived-working-copies/` (1675-1698). Nothing in the primitive or its
  callers can see a session on another host.
- The trash delete's guard against an archive holding a session's working folder
  (`crates/farhelm-supervisor/src/checkout_trash.rs` 365-426) checks `member_count` and `session_directories`, both from
  the local store (`crate::store::session_directory_paths`; see the test near `checkout_trash.rs:1124`). The
  single-checkout delete removes the archive immediately, without confirmation (SPEC.md "Managed checkouts", trash
  paragraph).
- The trigger is plausible in ordinary use: the checkout folder is an all-hosts setting (SPEC.md "Managed checkouts":
  "globally with per-host overrides"), so a home shared over NFS between two hosts gives both supervisors the same
  physical root. SPEC.md names NFS as a supported filesystem (around lines 1091 and 3524). Starting an existing-folder
  session on host B inside host A's checkout, to run something on the other machine, is an ordinary action.

Scenario: host A creates managed checkout `~/checkouts/repo-1` for session S_A. The user starts session S_B on host B
with folder `~/checkouts/repo-1` (same NFS home) and works there with uncommitted changes. The user deletes S_A. A sees
zero members and archives `repo-1` into `~/checkouts/farhelm-archived-working-copies/repo-1-<time>`. S_B's folder path
is now gone (the agent may keep running in the moved directory through its open directory handle, but restarting S_B
fails or starts elsewhere). Later the user clicks delete on that archive in A's trash (no confirmation), or confirms a
bulk delete. A's guard sees no local session in it and removes it recursively, destroying S_B's uncommitted work.

Other interactions the reviewer checked and found safe: allocation on a shared root uses an exclusive mkdir, so the
hosts cannot claim the same name (`working_copies.rs` 1361-1373). Archive names collide only into a no-replace rename
with a fresh suffix (1876-1898). Recovery, archive and trash act only on this supervisor's own rows, each re-verified by
inode and creation time, so A never archives or deletes a folder B created. The one exception is the no-creation-time
fallback after a folder was removed by hand, which SPEC.md "Maintainer-confirmed decisions" (dedicated ownership;
out-of-band manipulation unsupported) covers.

How to verify: on two hosts sharing an NFS home, or in a test with two supervisor stores over one temporary root, create
a checkout through store A, add a session in store B whose cwd is inside it, delete A's session, then run A's trash
delete. The folder is removed even though B's session references it.

Fix options (a maintainer decision): state in the spec that a checkout root shared between hosts is unsupported and
refuse it where detectable (for example a marker in the root naming the owning installation, checked at allocation and
archive). Or have the helm, which knows every host's sessions, add other hosts' sessions in the same canonical path on
hosts sharing a filesystem identity to the last-reference and trash guards. At minimum, the trash dialog and the archive
notice could warn that sessions on other hosts are not considered.

Reviewer's proposed drop, kept for triage: SPEC.md "Maintainer-confirmed decisions" says Farhelm has "dedicated
ownership of the managed checkouts it creates, their checkout roots and the trash" and that out-of-band manipulation is
unsupported (around line 3513). The filesystem-aliasing paragraph just above it says a folder reached through an alias
is not recognized. Either might be read as excluding a root shared with another host, but neither names that case: the
other host is Farhelm itself, using an ordinary session, through the same path.

Possible cover, for triage to judge: SPEC.md "Maintainer-confirmed decisions", "Confirmed 2026-10-11: Farhelm has
dedicated ownership ..." (around line 3513); the filesystem-aliasing paragraph (around line 3503); SPEC.md "Managed
checkouts", the paragraph on roots other local accounts can write to (around line 987), which covers other accounts, not
the same account on another host. Nothing in TODO.md's Planned bucket, BUGS.md, the queue or TRIAGE_OUTCOMES.md was
found.
