# Provisioning ignores supervisor unit drop-ins

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Setup or Update can report success while a leftover override on the host keeps the supervisor running a different
binary, tmux or state directory than the one shown.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F15 / COR-DROPINS-IGNORED`, tagged **possible**. Anchors and title: `provisioning/backend.rs:1665`,
`provisioning/plan.rs:409`, `provisioning/plan.rs:501` — Provisioning never looks at unit drop-ins, so the unit it
writes may not be what runs

systemd lets a user override parts of a unit with drop-in files in `farhelm-supervisor.service.d/*.conf` (for example
from `systemctl --user edit`, or a manual `FARHELM_TMUX` pin). Settings in a drop-in override the main unit file.
Provisioning writes only the main file (plan.rs:409). Its reach check (backend.rs:1665) only works out which directory
units live in; it never asks systemd what the _effective_ unit is (`systemctl --user cat`, or
`show -p DropInPaths,ExecStart,Environment`).

So a leftover drop-in that changes `ExecStart`, `Environment` or `ExecStartPre` silently wins. Every step reports
Completed while systemd runs the drop-in's binary, tmux or state directory — an older binary (so the update did
nothing), a tmux below the version floor, or a different state directory. That is the same "the plan named one tmux, the
host ran another" problem the unit's `FARHELM_TMUX` pin exists to prevent, per the `supervisor_unit` docstring
(plan.rs:501).

Suggested change: have the reach check report `DropInPaths` and `FragmentPath`, and have the plan refuse, or name in its
text, any drop-in that overrides `ExecStart`, `Environment` or `ExecStartPre`.

User-visible consequence: setup or Update can report success while a leftover override on the host keeps the supervisor
running a different binary, tmux or state directory than the one shown.
