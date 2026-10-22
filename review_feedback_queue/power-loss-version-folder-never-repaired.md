# A version folder damaged by a power loss right after an update is never repaired

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

If the Mac loses power or panics shortly after Farhelm updates itself (the desktop app does this in the background about
daily) or after a fresh install, the new version's `farhelm` program inside Farhelm.app can come back empty or garbled,
because nothing is flushed to disk before it is put in place. Farhelm then cannot start its local supervisor, and the
`farhelm` command in Terminal fails. The documented fix, running the installer again, does not help for this one file:
the installer sees that the version's folder already holds a program, keeps it, reports success, and leaves the app
broken. The app's automatic updater also considers that version installed and does nothing. The breakage lasts until the
user deletes a folder inside the app bundle by hand, moves the app aside, or a newer release arrives.

## Details

Found by the 2026-10-22 prioritized persistent review, groups `048-installer-1of2` (finding f1) and `049-installer-2of2`
(finding f1). Both reviewers found the same defect independently; this file merges them.

Suggested bucket (both reviewers): high. A bricked installation that the documented remedy, re-running the installer,
does not repair; no user data is lost, so not highest.

Confidence: likely. Unverified premise (both reviewers): that APFS on current macOS can, after a power loss or panic,
expose a file renamed into place without `fsync`/`F_FULLFSYNC` as empty or with unwritten contents while the rename and
later small writes survive. Confirmed by reading: the keep-forever rule on rerun and the updater's same-version skip.
Recording re-read `scripts/install.sh` 432-453 and 1259-1293 at the reviewed commit and confirmed the presence-only
test.

Mechanism:

- Writes: the version program is copied with plain `cp` into a private work folder and renamed into
  `Contents/Versions/<v>/` (`stage_version_dir`, `scripts/install.sh:434-438`, then 1265-1276 in the update path; the
  fresh and rebuild path copies into a staging bundle at 1299-1300 and swaps it in with one rename at 1310-1313).
  `replace_file` (446-453) copies and renames the other app files the same way. Nothing flushes file data before these
  renames or before the Installed record (1283-1284) is pointed at the new version, and macOS `fsync` does not flush the
  drive cache either. After a power loss or panic within the writeback window, the rename and the Installed record can
  be durable while `Versions/<v>/farhelm` is zero-length or partly written.
- The kept-folder rule: `scripts/install.sh:1259-1265` keeps any `Versions/<v>` folder whose `farhelm` is a regular,
  non-symlink file, regardless of its contents, on the stated assumption that "a release's bytes do not change". Line
  1265 tests only presence. A rerun of the same version therefore skips `stage_version_dir`, renames everything else
  into place again, and prints success (around 1455-1457).
- Every other bundle file self-heals on a rerun: `replace_file` always replaces `farhelm-desktop`, the icon, the
  Installed record, Info.plist and the ownership record, and the forwarder is replaced whenever its bytes differ (1280).
  Only the version program does not. (If the ownership record itself was lost, the rerun instead refuses the bundle as
  foreign at around 1210-1214, which at least says so.)
- Who uses the damaged file: the desktop app starts its supervisor only from
  `Contents/Versions/<its compiled version>/farhelm` (SPEC_impl.md "Side-by-side versions inside Farhelm.app", around
  4417-4420), and the forwarder execs the Installed version (`scripts/install.sh:317-321`) for Terminal use and for
  every session hook.
- Updater side: `crates/farhelm-ui/src/desktop/updater.rs:1040-1043` treats Installed == latest as up to date and
  installs nothing, and 1069-1071 decides an install succeeded from the Installed record alone, so neither an automatic
  nor an on-demand check repairs the folder until a newer release exists.
- Trigger: a crash or power loss in the seconds after an install or update completes. The background updater (SPEC.md
  "Security", around 2954-2956) makes updates frequent and unattended, so the user never chose the moment. Rare, but the
  damage persists and no documented command fixes it. SPEC.md "Concurrent and interrupted runs" counts a crash or reboot
  as an interruption and forbids "leaving a state that no documented command recovers from". SPEC.md "Power-loss
  durability of terminal file downloads and checkouts" names "the installation the installer writes" as critical state
  that "may be held to such a requirement, under its existing contracts" and says it does not weaken the prohibition on
  "bricking an installation". The installer's own header promises that "running the installer again finishes the job"
  (`scripts/install.sh:51-57`, repeated in cleanup's docstring at 703-709).

How to verify: by reading, line 1265 tests only presence. On a Mac, replace `Contents/Versions/<installed>/farhelm` with
an empty file, re-run the installer pinned to that version, and observe that the empty file is kept and that
`farhelm --version` through `~/.local/bin/farhelm` fails. On a macOS VM, a power cut right after an update reports
success would show whether the premise holds in practice.

Fix sketch: at the kept-folder test, keep an existing version folder only when its `farhelm` has the same bytes as the
staged one (`same_bytes "$STAGING_DIR/farhelm" "$new_version_dir/farhelm"`; the staged copy is already checksum-verified
at around 1040-1046), and otherwise treat it like an incomplete folder: move it aside and replace it, as the existing
branch does (a running Farhelm keeps its open inode). Optionally also flush the staged program and the Installed record
before the renames that publish them, as the TODO.md race-free design proposes.

Reviewer's proposed drop and possible cover, kept for triage: TODO.md Near term "Make the Mac installer race-free" says
"A power cut mid-update has no guaranteed outcome" and plans "flushes before renames". It is not in the Planned bucket,
and it does not mention that a rerun cannot repair the damaged version folder, so it is not full coverage. The second
reviewer adds that SPEC.md says the installation "may" be held to power-loss requirements but adds none, so triage may
judge that the installer has no power-loss contract beyond its own header comment. SPEC.md "Healthy local filesystems"
concerns I/O errors and hangs, not power loss. Nothing in BUGS.md, the queue or TRIAGE_OUTCOMES.md was found.
