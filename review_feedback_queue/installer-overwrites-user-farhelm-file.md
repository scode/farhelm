# The installer destroys a pre-existing user file named farhelm

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Installing into a directory that already holds the user's own file named farhelm (or farhelm-desktop on macOS) silently
and irrecoverably replaces it while the installer reports an update.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F8 / COR-INSTALLER-OVERWRITES-USER-FARHELM`, tagged **possible**. Anchors and title: `scripts/install.sh:1094`,
`scripts/install.sh:1116`, `scripts/install.sh:1153` — The installer replaces and then deletes any pre-existing regular
file named farhelm or farhelm-desktop in the install directory

Before replacing binaries, the installer checks each destination with `refuse_unless_absent_or_regular` (L1094), which
accepts any regular file. The replace loop then moves an existing file aside to `.farhelm.old` (L1116 onward), and after
the commit `rm -f`s the backup (L1153). Nothing compares that existing file with the ownership record
`.farhelm-installation` that describes what the installer last put there. A hand-written `farhelm` wrapper script in a
custom `FARHELM_INSTALL_DIR` such as `~/bin`, or an unrelated `farhelm-desktop` on macOS, is therefore destroyed without
a backup, and the run reports "Updated farhelm …".

The uninstaller takes the opposite stance on the same kind of collision: on Linux it keeps and reports a
`farhelm-desktop` that its record does not claim (ownership.rs:214–225), and it refuses a payload whose digest does not
match. So install and uninstall disagree about whether a same-named file is Farhelm's. Overwriting a file with the
program's own name in its install directory is a common convention for installers, so this may be an accepted tradeoff,
but neither SPEC.md nor `docs/install_uninstall.md` says so.

Suggested change: when an existing destination is not accounted for by a valid ownership record, either refuse, or keep
the moved-aside file under a non-reserved name and print where it went. Otherwise, document the overwrite in
`docs/install_uninstall.md`.

Restater note: on Linux the installer never touches `farhelm-desktop` (it only replaces `farhelm`); the
`farhelm-desktop` half applies to macOS only.
