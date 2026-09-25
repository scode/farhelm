# A released lock flag lets the installer roll back another run

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Running the installer twice at nearly the same time can leave the farhelm command deleted or silently reverted even
though the installer reported a successful update.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F1 / COR-STALE-LOCK-FLAG-ROLLS-BACK-OTHER`, tagged **definite**. Anchors and title: `scripts/install.sh:1161`,
`scripts/install.sh:742`, `scripts/install.sh:743` — The installer keeps LOCK_ACQUIRED=1 after releasing the lock, so
its exit handler can roll back another installer's live transaction

The installer remembers that it holds the install lock in a shell variable, `LOCK_ACQUIRED`, set to 1 by `acquire_lock`.
Its exit handler `cleanup` (run on every exit, normal or signalled) checks that flag and, if set, either rolls back
whatever journal is at `$LOCK_DIR/journal` or removes the lock directory. After a successful commit the installer
releases the lock at L1161 (`remove_owned_lock`) but never sets the flag back to 0; the only `LOCK_ACQUIRED=0` in the
file is the initialisation at L932. The process then keeps running for a while: on Linux only the closing report and a
`tmux -V` probe, but on macOS the whole app-bundle build (copying two binaries, four SHA-256 passes, `rm -rf`, `mv`,
`lsregister`), which takes seconds.

If a second installer B for the same install directory starts in that window, it can take the freed lock (its downloads
happen before it asks for the lock, so it can be ready immediately). When the first installer A then exits, its
`cleanup` still believes it owns the lock, and `$LOCK_DIR/journal` is now B's journal. Two ways this goes wrong:

- A rolls back B's in-progress transaction. If B has already moved its new `farhelm` in, A's undo of `INSTALL cli`
  (`rm -f farhelm`) deletes it and A's undo of `PARK cli` moves the old binary back. B then commits, writes an ownership
  record for whatever `farhelm` is now there, and prints "Updated farhelm …". If B had committed first, A's
  `rm -f farhelm` deletes B's new binary and the `PARK` undo finds no backup, leaving no `farhelm` at all.
- If B has no journal yet, A's `remove_owned_lock` deletes B's `pid` and lock directory (it checks only that the
  directory has the lock's shape, not whose pid is in it), so a third installer can enter alongside B.

The effect is that running the installer twice in close succession can leave the command deleted or silently reverted
while the installer reports success. The fix is a one-liner: set `LOCK_ACQUIRED=0` right after the release at L1161 (and
after the stale-lock `remove_owned_lock` inside `acquire_lock`). Optionally `cleanup` could also confirm that
`$LOCK_DIR/pid` still contains `$$` before acting.

Restater note: the window is milliseconds on Linux and seconds on macOS. SPEC.md "Operator prerequisites and failure
behavior" excludes installs racing _uninstall_, but says nothing about two installs racing each other, so this is not
covered by an accepted exclusion.
