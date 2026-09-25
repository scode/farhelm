# The launch shim prepends Farhelm's whole binary directory to PATH

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

An agent started through Farhelm can run a different `claude` or `tmux` than the user gets by typing the same command
over SSH.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F19 / COR-PATH-BINARY-FOLDER`, tagged **definite**. Anchors and title: `launch.rs:137-144`,
`service/core.rs:12266-12270` — The launch shim prepends Farhelm's whole binary directory to the agent's PATH

Agents running inside a session can call `farhelm` by name, for example to spawn a child session. The supervisor wants
that name to reach the exact build that launched the agent rather than some other installed copy. It records the
directory of its own executable in the launch spec as `farhelm_bin_dir`: the field is documented at `launch.rs:137-144`
and filled at `service/core.rs:12266-12270` as `farhelm_exe.parent()`. The shim's `launch_child_command`
(`launch.rs:~935-941`) puts that directory in front of the PATH the login shell produced. The same function builds the
environment for the agent and for the git clone and post-clone hook of a fresh-checkout launch.

The problem is that the directory usually contains more than `farhelm`. On the helm machine, `install.sh` installs into
`${FARHELM_INSTALL_DIR:-$HOME/.local/bin}`. That directory also commonly holds Claude Code's native `claude`, pip
`--user` scripts and other user tools. On hosts the helm provisions, the binary lives in `~/.local/lib/farhelm/`, next
to Farhelm's private pinned `tmux` (`farhelm-helm/src/provisioning/plan.rs`, `lib_dir.join("tmux")`). Every program in
that directory now takes precedence over the user's own PATH order. That applies to the agent's own `argv[0]` and to
everything the agent runs. If the user's PATH puts a different `claude` (say an npm global) ahead of `~/.local/bin`, a
Farhelm session runs the other one. An agent that runs `tmux` on a provisioned host gets Farhelm's pinned build, which
may not speak the same protocol as the user's own tmux server.

This contradicts SPEC.md's environment contract. A session process must behave as if the user had SSHed in and typed the
command, and "a bare `claude` in a profile must work exactly as it does from the user's own shell". SPEC_impl only asks
that the supervisor put "its own binary on the session PATH", not its neighbours. The suggested fix is to give the shim
a directory that contains only `farhelm`, for example `<state_dir>/bin/farhelm` as a symlink to `farhelm_exe` refreshed
at supervisor startup, and pass that directory as `farhelm_bin_dir`. The `farhelm` override then keeps working without
reordering any other program.
