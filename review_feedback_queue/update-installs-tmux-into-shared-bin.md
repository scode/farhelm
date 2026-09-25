# UPDATE installs the private tmux into the registered binary's directory

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Clicking Update on a host whose farhelm lives in `~/.local/bin` can silently overwrite the user's own `tmux` there, or
put Farhelm's tmux first on their shell PATH.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F1 / COR-TMUX-INTO-SHARED-BIN`, tagged **definite**. Anchors and title: `provisioning/plan.rs:372-400`,
`provisioning/plan.rs:466-469`, `provisioning/backend.rs:797-806` — UPDATE installs Farhelm's private tmux into the
registered binary's directory, overwriting or shadowing the user's own tmux

When the reach check finds no acceptable tmux on the host, the plan installs Farhelm's own static tmux build at
`lib_dir/tmux` (plan.rs:373). For ADD, `lib_dir` is the Farhelm-dedicated `~/.local/lib/farhelm`. For UPDATE,
`plan_for_row` sets `lib_dir` to the _parent directory of the registered farhelm binary_ (plan.rs:468). If the row says
the binary is `~/.local/bin/farhelm` (where `install.sh`, the README's install method, puts it), or `~/bin/farhelm`,
`~/.cargo/bin/farhelm`, or `/usr/local/bin/farhelm` when running as root, then the tmux payload is written to
`~/.local/bin/tmux` and so on.

The install step (backend.rs:797-806) hashes the existing destination and, if the bytes differ, does
`chmod 755 tmp && mv -f tmp ~/.local/bin/tmux`. There is no check of whether a `tmux` already exists there or who put it
there. The "needs tmux" decision comes from `command -v tmux` on the non-interactive ssh PATH, which on stock Ubuntu
does not include `~/.local/bin`, so a user's own `~/.local/bin/tmux` is invisible to that check and is simply replaced.
This trigger is common: the floor is tmux 3.7c, stock distributions ship older versions (Ubuntu 24.04 has 3.4), and
Farhelm's own private tmux in `~/.local/lib/farhelm` is never on PATH either, so the reach check reports "needs tmux" on
almost every host. Where no `~/.local/bin/tmux` existed before, one now does, and because Ubuntu's login profile puts
`~/.local/bin` ahead of `/usr/bin`, the user's interactive `tmux` command now runs Farhelm's build instead of the
distribution's (which may also stop it from talking to tmux servers the distribution's tmux already started). UPDATE
shows no plan, so the user is never told.

This conflicts with SPEC.md "Ownership during cleanup and provisioning" (no incidental changes to host configuration)
and SPEC_impl.md "Provisioning", which places the private tmux under Farhelm's own lib directory. Nothing needs the tmux
on PATH: the unit pins the exact executable through `FARHELM_TMUX`. The fix is to install the private tmux in a
Farhelm-dedicated directory (`~/.local/lib/farhelm/tmux`) regardless of where the farhelm binary lives, and to refuse,
or at least surface, replacing a `tmux` that Farhelm did not install.

User-visible consequence: clicking Update on a host whose farhelm lives in `~/.local/bin` can silently overwrite the
user's own `tmux` there, or put Farhelm's tmux first on their shell PATH.
