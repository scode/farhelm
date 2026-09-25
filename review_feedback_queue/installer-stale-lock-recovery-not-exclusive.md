# Stale-lock recovery is not exclusive

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After an interrupted install, starting two installer runs at once can leave no farhelm binary while both runs report
that the previous installation was restored.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F2 / COR-STALE-LOCK-RECOVERY-NOT-EXCLUSIVE`, tagged **definite**. Anchors and title: `scripts/install.sh:699`,
`scripts/install.sh:700`, `scripts/install.sh:715`, `scripts/install.sh:716`, `scripts/install.sh:446` — Stale-lock
recovery is not exclusive, so two runs can both roll back the same journal or reclaim each other's lock

When `acquire_lock` finds an existing lock whose pid is no longer running, it treats the lock as wreckage from a crash.
If a journal exists it calls `rollback_from_journal` (L700) and exits; otherwise it removes the stale lock with
`remove_owned_lock` (L715) and re-creates it with `mkdir` (L716). None of these steps happens under anything exclusive:
two installers started after the same crash both see the dead pid and both proceed.

With a journal, both processes read it before either has appended its `UNDONE` markers. The first restores the old
binary (`.farhelm.old` → `farhelm`); the second, working from its earlier read, replays the `INSTALL cli` undo
(`rm -f farhelm`) and deletes the binary the first just restored, then treats `PARK cli` as already done because the
backup is gone. Both print "restored the previous installation", and there is no `farhelm` left. The correctness-edge-
inputs reviewer reproduced this in a `mktemp -d` by extracting the journal functions (L473–648) and running two rollback
passes, the second against a copy of the journal taken before the first pass's markers: the first pass restored the old
binary, the second deleted it and returned success.

Without a journal, the race is on the lock itself. A removes the stale lock and creates its own; B, which already
decided the lock was stale, calls `remove_owned_lock`, which checks only the directory's shape (`is_our_lock`), not
whose pid it holds, so it deletes A's fresh live lock and creates its own. Both then enter the replacement phase. If
instead A's `mkdir` wins after B's removal, B's unguarded `mkdir` at L716 fails and, under `set -e`, the installer exits
with only mkdir's own "File exists" message.

The `UNDONE` markers make replay safe only when replays happen one at a time. The suggested fix is to claim a stale lock
atomically, for example `mv "$LOCK_DIR" "$LOCK_DIR.stale.$$"`: exactly one process wins the rename, re-checks and rolls
back from the renamed directory, deletes it, and creates a fresh lock; the loser restarts `acquire_lock`. The L716
`mkdir` should also get its own failure message.

Restater note: the trigger needs an earlier interrupted install plus two installer runs started close together; it is
narrow but the mechanism is confirmed in the code.
