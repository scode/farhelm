# Writable snapshot may pass replacement checkout ownership

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Writable snapshot may pass replacement checkout ownership.

## Details

`F6 / COR-SNAPSHOT-ROOT` — **possible** — `crates/farhelm-supervisor/src/working_copies.rs:558` — Writable snapshot may
pass replacement checkout ownership

Deleting the last session using a managed checkout could move a replacement checkout into the archive if a writable
filesystem snapshot preserves the identity values Farhelm checks. Farhelm deliberately accepts a directory whose inode
number and creation time match its recorded values even when its device number has changed. This lets ordinary reboots
and remounts keep working on filesystems such as btrfs. The same comparison protects both the checkout and its parent
root.

The proposed failure requires replacing the mounted root as well as the checkout with a snapshot that preserves both
directories' recorded inode numbers and creation times. If that premise holds, both ownership checks pass. The archive's
additional device check compares the source and destination as they exist now, so it also passes when both are inside
the replacement filesystem. Delete can then archive the replacement folder, including newer work, despite SPEC.md:627
requiring a foreign replacement to remain untouched. This is a recoverable move within the root, not recursive deletion.

First reproduce the identity checks and actual archive move on btrfs with that exact root-replacement arrangement. If
confirmed, distinguish a remount from a different snapshot using stable filesystem or subvolume identity while retaining
ordinary remount support.

Suggested bucket: high

Possible cover: SPEC.md:614–622 and TRIAGE_OUTCOMES.md:4079–4102 explicitly accept inode-and-creation-time matching
across device changes after a remount; they do not explicitly address replacing the root with a snapshot.

Caveats: Snapshot preservation of both the root's and checkout's inode numbers and creation times is unverified, and
replacing only the checkout is insufficient if the root check refuses. TRIAGE_OUTCOMES.md:1580–1584 specifically
classifies a recoverable wrong archive move as high. The merged review requests a fresh independent D22 audit before
exclusion.

Restater note: The source confirms the conditional acceptance path, including the comparison against the archive's
current device. It does not establish the required btrfs snapshot identities or a successful move; this remains a
reproduction question.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **high**.

Originating reviewers and passes: `runtime_general p1`.

Possible cover recorded during collection: SPEC614–622/ledger4079–4102 remount matching inode/btime acceptance, snapshot
not explicit..

Collection caveats: Unverified btrfs preserves inode+btime for both root and checkout; root replacement required; move
not recursive delete. Ledger1580–1584 specifically classifies recoverable wrong archive move high; fresh independent
audit D22 still needed before exclusion.

## Filed reviewer metadata

- `runtime_systems p1`: confidence as filed: possible, contingent on an unrelated directory presenting both the recorded
  inode and identical birth timestamp after a filesystem replacement/remount. No collision was established. Suggested
  bucket as filed: highest.
- `runtime_security p1`: confidence as filed: **possible**, low evidentiary confidence. A supported filesystem operation
  yielding a distinct checkout at the same path with matching inode and birth time has not been reproduced. Suggested
  bucket as filed: `highest` only if the collision premise is established.
- `runtime_trust p1`: confidence as filed: possible; confirmed comparison rule, open premise of mounting a distinct
  filesystem object at the recorded path with the same inode and birth timestamp. No ordinary supported sequence
  establishing that premise was found. Suggested bucket as filed: `highest` conservatively.
- `runtime_general p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable
- `runtime_data p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable
