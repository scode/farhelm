# Provisioning chmods existing shared directories

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Adding or updating a host silently loosens permissions on the user's `~/.config/systemd/user` and, on update, on the
directory holding farhelm (possibly their home directory), exposing files there to other accounts on the host.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F28 / SEC-CHMOD-SHARED-DIRS`, tagged **definite**. Anchors and title: `provisioning/backend.rs:1717-1730`,
`provisioning/backend.rs:1704-1715`, `provisioning/plan.rs:337-352`, `provisioning/plan.rs:466-469`,
`crates/farhelm-ui/src/provisioning.rs:170` — Every plan chmods existing shared directories: the systemd user-unit dir
always, and on UPDATE the registered binary's directory (possibly $HOME)

Every plan begins with an `EnsureDirectories` step (plan.rs:337-352) for three directories: the lib directory at 0755,
the supervisor state directory at 0700, and the systemd user-unit directory at 0755. On a remote host this runs
`install -d -m <mode> -- <dir>` for each (backend.rs:1717-1730). GNU `install -d -m` does not only create missing
directories; it also chmods an existing one to the given mode. Five reviewers checked this with coreutils 9.4 in scratch
directories (700→755, 750→755, 775→755, 2775→2755, 1777→755), and I reproduced 700→755 and 1777→755 myself. Reviewers
also report that missing parent directories are created 755 regardless of umask. The local branch (backend.rs:1704-1715)
calls `set_mode` unconditionally, but no production path reaches it today.

Which directories get chmodded:

- The unit directory, normally `~/.config/systemd/user`, on every ADD and UPDATE. It is shared with every other user
  unit.
- On UPDATE, the lib directory is `Path::parent()` of the registered or probe-resolved farhelm binary (plan.rs:466-469):
  `~/.local/bin`, `~/bin`, `~/.cargo/bin`, `$HOME` itself (for a binary at `~/farhelm`), `/root`, `/usr/local/bin` when
  running as root, or even `/tmp` (for `/tmp/farhelm`).
- The registered `remote_state_dir` is forced to 0700 the same way.

If one of these directories is owned by root with a different mode, UPDATE fails at its very first step with EPERM. The
web UI submits remote UPDATE without showing the plan (`automatic_update`, `crates/farhelm-ui/src/provisioning.rs:170`),
and the ADD confirmation says "create or reuse … (mode 0755)" without saying an existing mode will change.

SPEC.md "Ownership during cleanup and provisioning" and SPEC_impl.md "Provisioning" name exactly the shared executable
directory and the user-unit directory as directories whose permissions must be preserved, with an obstacle reported
instead. The security effect: a 0700 or 0750 home or unit directory becomes 0755, so any 0644 files inside it — private
scripts, tokens, other units' `Environment=` secrets — become readable by other accounts on the host; a setgid shared
directory loses group write; and a root supervisor registered at `/tmp/farhelm` would strip `/tmp`'s sticky and
world-write bits.

A follow-on from one reviewer: once shared-directory modes are preserved, the lib directory may be group- or
world-writable. The remote install command (backend.rs:797) checks the temporary's digest and then runs `chmod && mv` as
separate commands in the same shell script, so another account that can write that directory could swap the temporary
between the check and the rename. Such directories should be refused, or the check and rename done on the same open
file.

Suggested change: mark each `DirectorySpec` as Farhelm-dedicated or shared. Enforce modes only on dedicated directories
(`~/.local/lib/farhelm`, the state directory); create shared ones only when missing
(`[ -d d ] || install -d -m 755 -- d`) and never chmod them; report permissions that block installation; say "reuse" in
the confirmation; and consider refusing UPDATE when the binary's parent is `$HOME`, `/` or `/tmp`.

User-visible consequence: adding or updating a host silently loosens permissions on the user's `~/.config/systemd/user`
and, on update, on the directory holding farhelm (possibly their home directory), exposing files there to other accounts
on the host.
