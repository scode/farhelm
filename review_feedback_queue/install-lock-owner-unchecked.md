# The install lock and journal are trusted without an owner check

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In a shared install directory, another user can arrange for the user's next Farhelm update to replace their farhelm
command with a program of that user's choosing.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F26 / SEC-LOCK-OWNER-UNCHECKED`, tagged **possible**. Anchors and title: `scripts/install.sh:404-431`,
`scripts/install.sh:682-708`, `scripts/install.sh:616-631` — The install lock and journal are trusted without checking
who owns them

`is_our_lock` (L404–431) decides whether an existing lock directory is the installer's own. It checks the entry names,
that they are regular files, and that the directory is not group- or world-writable, and its comment says the point of
the last check is to stop another account redirecting recovery. It never checks that the lock directory, its `pid` or
its `journal` belong to the current user. With a dead recorded pid, `acquire_lock` (L682–708) replays the journal, and a
`PARK cli` record makes rollback run `mv .farhelm.old farhelm` (L616–631).

Scenario: a shared install directory that is group-writable with the sticky bit set and owned by the victim, which
`FARHELM_INSTALL_DIR` allows. The sticky bit normally stops other group members from replacing the victim's `farhelm`.
But another group member can create a 0755 lock directory of their own containing a `pid` for a dead process, a journal
reading `PARK cli`, and their own file as `.farhelm.old`. On the victim's next update, stale-lock recovery renames the
attacker's file over the victim's `farhelm`. The victim owns the directory, so the sticky restriction does not stop the
victim's own process from doing that rename. An `INSTALL cli` record would delete the victim's `farhelm` instead.

Suggested change: require the lock directory, `pid` and `journal` to be owned by the effective user, and the journal to
be neither group- nor world-writable; refuse otherwise.

Restater note: I traced the attack against the code. The rename happens before rollback tries to append its `UNDONE`
marker to the attacker-owned journal. If that append fails, the run reports that rollback "could not fully complete",
but the victim's binary has already been replaced. The precondition (a group-writable, sticky install directory owned by
the victim) is unusual. Without the sticky bit, the attacker could replace `farhelm` directly and the lock would add
nothing, so the finding matters only in the sticky case.
