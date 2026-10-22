# An update briefly leaves new launch files group-writable inside Farhelm.app under a permissive umask

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

When you update Farhelm by running the installer from a shell whose umask lets the group write new files (umask 002 or
000), the installer puts the new launch settings file, and the new Terminal forwarder script when that changes, inside
the app as group-writable for a moment before tightening them. On a Mac every account is in the shared staff group, so
another account on the machine can open the file in that moment and keep write access after it is tightened and put in
place. Through the launch settings it can make its own code run as you the next time Farhelm opens; through the
forwarder, every time the `farhelm` command runs, including from agents inside sessions. This is the remaining gap of an
already-decided fix (the launch settings file must not take its mode from the caller's umask).

## Details

Found by the 2026-10-22 prioritized persistent review, group `049-installer-2of2` (finding f3).

Suggested bucket: highest (another local account gains code execution as the user).

Confidence: likely. Confirmed by reading (reviewer and recording) that the copy is group-writable between lines 449 and
450 under umask 002, and that the update path sets no umask around these writes (the script's only `umask` subshells are
at 372, 435, 974, 1146, 1148, 1249 and 1299). Unverified premise: that an attacker account can reliably open the file
inside that window of a few milliseconds. It is a separate `cp` and `chmod` process, and the name is predictable.

Mechanism (line numbers re-pinned by recording; the reviewer's update-path numbers were about five lines high):

- `replace_file` (`scripts/install.sh:446-453`) does `cp "$1" "$REPLACE_TMP"` (449) into the destination's own folder,
  then `chmod "$3"` (450), then renames (451). `cp` without `-p` creates the destination with the source's mode masked
  by the umask, so the copy exists at the predictable name `<dir>/.farhelm-new.<pid>.<name>` with the source's
  group-write bit until the separate `chmod` runs.
- Sources with group write under umask 002: the update path writes `$UPDATE_WORK/Info.plist` with `cat >`
  (`write_info_plist`, 331-332, called at 1285) and `$UPDATE_WORK/forwarder` the same way (`write_forwarder`, 276-277,
  called at 1279), and the Installed record with `printf` (1283). None is chmodded before `replace_file` copies it
  (1286, 1281, 1284). They are created 0664 (0666 minus 002) inside the private work folder, which is safe there, but
  the copy in `Contents/` or `Contents/MacOS/` (folders 0755, group staff by BSD inheritance) is 0664 group staff until
  the `chmod`. Info.plist is replaced on every update; the forwarder whenever its text changes (1280).
- Attack: another local account in staff watches `Contents/` (kqueue on a readable folder, or polling the predictable
  name; the installer's pid is visible in `ps`) and opens the new file for writing during the cp-to-chmod window.
  Permissions are checked only at open, so the descriptor keeps working after `chmod 0644` and the rename. It then
  writes an `LSEnvironment` key (for example `DYLD_INSERT_LIBRARIES`) into the live Info.plist, or shell commands into
  the live forwarder. Nothing re-signs or re-verifies these files (SPEC_impl.md "Side-by-side versions inside
  Farhelm.app", around 4398-4403), so the app or the forwarder runs the injected code as the user.
- Not affected: the fresh-install and rebuild path chmods everything inside the private `mktemp -d` build folder before
  the bundle is renamed into place (1297-1313). The ownership record (`write_current_record`, umask 077 at 372) and the
  binaries and icon (extracted with an explicit final mode, see 463-466) are not affected either.
- Prior decision: TRIAGE_OUTCOMES.md "app-info-plist-uses-caller-umask.md" judged exactly this threat (umask 002, a
  staff-group account, `LSEnvironment`) worth a code fix. The executed fix (PR #1148) sets the mode after writing, which
  closes the steady state but not this window in the update path. The comment at `scripts/install.sh:327-330` states the
  intent ("The callers give it an explicit owner-only-writable mode for that reason").

How to verify: under `umask 002`, run an update in the macOS-shaped harness (`scripts/test-install-sh.sh`, whose case P1
already installs under umask 002) with a `cp` double that pauses, or `ls -l` from a second process, and observe
`.farhelm-new.<pid>.Info.plist` at mode 0664 in `Contents/`.

Fix sketch: give the copy its final mode before it becomes reachable. For example chmod the source in `$UPDATE_WORK`
before copying, or run the copy under `(umask 022; cp ...)` or `(umask 077; ...)` so the file beside the destination is
never group- or world-writable. `write_info_plist` and `write_forwarder` could also write under umask 022 themselves.

Reviewer's proposed drop, kept for triage: the trigger needs both a user who runs the installer manually under a
permissive umask (the desktop updater normally inherits 022) and a hostile second local account on the Mac, plus winning
a short race. SPEC.md says the browser UI is not recommended on machines with untrusted local users, but that concerns
the browser, not the installer. Recording notes this is a rarity argument, not a cited cover.

Possible cover, for triage to judge: TRIAGE_OUTCOMES.md "app-info-plist-uses-caller-umask.md" (same threat, fixed only
for the steady state). `FILTER.md` "Races a person would have to win inside a sub-second window" does not apply: the
attacker is a program, and the consequence is a security one.
