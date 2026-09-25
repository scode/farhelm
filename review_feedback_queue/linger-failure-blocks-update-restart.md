# A linger failure stops UPDATE before restart

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a host where loginctl fails for any reason other than a recognised permission refusal, Update always fails at "enable
linger" and the host stays on its old version even though the new files were installed.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F12 / COR-LINGER-BLOCKS-RESTART`, tagged **possible**. Anchors and title: `provisioning/plan.rs:408-433`,
`provisioning/backend.rs:1848-1870`, `provisioning/backend.rs:2047-2067` — The optional linger step runs before restart
and attach, and any unrecognized loginctl failure fails the run

The step order is `WriteUnit`, `DaemonReload`, `EnableSupervisor`, `EnableLinger`, then `RestartSupervisor` (UPDATE
only) and `AttachSupervisor` (plan.rs:408-433). Linger is documented as optional: SPEC.md Topology says that when an
optional step cannot be done, "provisioning says so and continues without it", and SPEC_impl.md calls linger the
optional step. But `enable_linger` (backend.rs:1848-1870) turns a failure into the non-fatal `Degraded` outcome only
when `linger_was_refused` (backend.rs:2047-2067) matches: an exit code other than 0 or 255 _and_ stderr containing both
a refusal phrase ("permission denied", "not authorized", …) and the word "loginctl" or "linger". Every other failure is
an error that stops the run: `loginctl` not installed (exit 127, "not found"), logind or the system bus unreachable
("Failed to connect to bus"), `id -un` printing nothing, or a polkit denial worded differently.

For UPDATE that is the worst place to stop. The new binary, tmux and unit are already on disk, but the supervisor is
never restarted onto them, and because the failure is a fixed property of the host, every retry fails the same way — the
host can never be updated through the panel. For ADD the supervisor has already been started by `enable --now` when the
run is marked failed. Suggested change: move `EnableLinger` after restart and attach, and treat every linger failure in
which the remote command actually ran (exit ≠ 255) as `Degraded` with its stderr, keeping hard failure only for
transport errors.

User-visible consequence: on a host where loginctl fails for any reason other than a recognised permission refusal,
Update always fails at "enable linger" and the host stays on its old version even though the new files were installed.
