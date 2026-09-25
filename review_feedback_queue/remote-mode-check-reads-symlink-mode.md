# The remote mode check reads a symlink's own mode

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Re-running setup or Update on a host whose farhelm binary or unit file is a symlink either fails with a permissions
error, changes the linked file's permissions and reports "repaired mode" every time, or replaces the symlink with a
copy.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F13 / COR-SYMLINK-MODE`, tagged **definite**. Anchors and title: `provisioning/backend.rs:481-491`,
`provisioning/backend.rs:512-520`, `provisioning/backend.rs:593-598`, `provisioning/backend.rs:776-783` — The remote
metadata check hashes a symlink's target but reads the link's own mode

Before installing a file, provisioning asks the remote host for the existing file's hash and mode, to skip an unchanged
file and "repair" a wrong mode. The remote script (backend.rs:481-491) uses `[ -e path ]` and `sha256sum < path`, both
of which follow a symlink to its target, but `stat -c '%a' path` without `-L`, which reports the symlink's own mode —
always 777 on Linux. I reproduced this in a scratch directory with GNU coreutils 9.4: a symlink to a 0755 file prints
777 without `-L` and 755 with it. `chmod` on a symlink changes its target.

Symlinked destinations are ordinary here: the recorded binary path comes from `command -v farhelm`, which reports the
PATH entry as-is (often a symlink into a versioned directory), and dotfile repositories commonly symlink unit files. Two
outcomes follow:

- Same content: the mode "differs" (777 vs 755), so `set_target_mode` (backend.rs:593-598, 776-783) runs `chmod 755`
  through the link on the target. If the target is root-owned or read-only, that fails with EPERM and the UPDATE fails
  even though the content is already correct; otherwise it changes the target's permissions and reports "repaired mode"
  on every run.
- Different content: `mv -f tmp link` replaces the symlink itself with a regular file, so the link's original target
  keeps the old bytes and the user's link layout is rewritten.

The local transport opens the file and reads the target's mode, so the two transports disagree. Suggested change: use
`stat -L -c '%a'` and decide explicitly what to do with a symlinked destination — resolve it before installing, or
refuse with a clear message.

User-visible consequence: re-running setup or Update on a host whose farhelm binary or unit file is a symlink either
fails with a permissions error, changes the linked file's permissions and reports "repaired mode" every time, or
replaces the symlink with a copy.
