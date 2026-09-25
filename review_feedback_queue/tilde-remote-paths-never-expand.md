# Tilde remote paths are quoted and never expand

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Entering `~/.local/bin/farhelm` as a host's binary path makes the host look like it has no supervisor even when one is
running.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F23 / COR-TILDE-PATHS`, tagged **possible**. Anchors and title: `ssh.rs:129`, `ssh.rs:135`,
`provisioning/backend.rs:927`, `provisioning/backend.rs:938` — Tilde paths in remote_farhelm/remote_state_dir are quoted
and never expand

The add-host form's binary-path and state-directory fields are free text, and the store accepts a value starting with
`~`. Both the steady-state dial (`ssh_stdio_args`, ssh.rs:129 and 135) and the probe script (backend.rs:927, 938) wrap
the value in single quotes for the remote shell, and the shell does not expand `~` inside quotes. So
`~/.local/bin/farhelm` is looked up as a relative path literally named `~/.local/bin/farhelm` in the home directory,
which does not exist. The probe reports "no supervisor", and the steady-state dial fails with the generic "either no
supervisor is running on the host, or the ssh connection itself failed" message. A state directory `~/x` becomes
`$HOME/~/x`.

Typing a home-relative path with `~` is the most natural way to enter one, and it produces a host that never connects
with a misleading reason. Suggested change: reject a leading `~` with a clear message, or deliberately expand `~/` as
`"$HOME"/` followed by the quoted remainder.

User-visible consequence: entering `~/.local/bin/farhelm` as a host's binary path makes the host look like it has no
supervisor even when one is running.
