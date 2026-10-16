# Sharing the machine, scratch space and Claude Code shell commands

The root `AGENTS.md` sections on sharing the machine, agent scratch space, and Claude Code shell commands, moved here
unchanged. The root keeps the rules every task needs, word for word; this file has the rest: the isolation mechanisms,
the full list of what stays where it already goes, docker naming, and the history behind each rule. Read it before
creating anything outside the checkout, installing a tool environment, allocating a build directory, using docker, or
cleaning up. When you change a sentence that also appears in the root, change both.

## Sharing the machine with other agents

Several agents work on this project at once, on this machine, as the same unix user, each in its own checkout or jj
workspace. The harnesses are built so that concurrent runs do not collide: every test picks its ports from the kernel
(the Rust e2e harness binds port 0, the Playwright config and the desktop smoke ask for a free port per run, the CentOS
script lets docker choose), state lives in per-run random directories under `/tmp` addressed by private tmux and
supervisor sockets, systemd scopes and units carry a UUID, and the CentOS script's ssh alias is suffixed per run. Do not
reintroduce a fixed port, a fixed path under `/tmp` or `$HOME`, a fixed unit or container name, or a fixed ssh alias in
a harness; when a harness genuinely needs a stable name, derive it from the run's own random directory.

What is NOT isolated, and what follows from it:

- One agent per checkout or workspace. Playwright's stack-info and auth files, `.ci-tmux/`, `.ci-nextest/`, and the dx
  output under `target/` are per checkout, and `scripts/build-tmux-assets.sh` deletes and rebuilds its python
  environment under `target/` without a lock. Two agents in one tree race on all of them. Sibling checkouts and
  `jj workspace` directories are the supported shape; a shared `CARGO_TARGET_DIR` across workspaces is fine for cargo
  (it locks) but not for the dx and tmux-asset builds: the desktop smoke serializes its own cargo and dx builds behind
  `/tmp/fh-build.lock`, and nothing serializes the rest. (Fixed lock files like that one, and the CentOS script's lock
  next to the ssh config, are the exception the fixed-path rule allows: a lock exists to be shared, and it names no
  state.)
- Processes you did not start are someone else's. A helm or supervisor answering on a port you did not choose, a
  `farhelm-<uuid>-*.scope` unit whose UUID no run of yours minted, an `fh-it.*` or `fh-e2e.*` directory holding a live
  flock, or a docker container from the CentOS script: leave them alone. The teststate sweep already reaps dead runs
  from any checkout, guarded by the flock, so nothing needs killing by hand.
- The `systemd --user` manager and the CPU are shared. Timing budgets, the transient-scope availability probe, and the
  RSS measurement in `terminal_backpressure` assume a quiet machine, and another agent's battery is exactly the load
  FLAKES.md records as tripping them. A failure under that load is an environment event: keep the retained run, re-run
  narrowly when the machine is quieter, and do not log it as a flake or a regression on one observation.

## Agent scratch space

Anything you create outside the checkout at your own discretion goes under `/tmp/farhelm-tmp/<id>/`, where `<id>` is the
output of one `uuidgen` you run the first time you need scratch space and reuse for the rest of the session. If you lose
track of it (after compaction, say), mint a new one rather than guessing which existing directory is yours, unless your
working log records it. The point is that all agent scratch on this machine is in one place, so finding it never
requires digging through `/tmp` or asking what some directory is.

More broadly, anything bulky that agent work leaves behind belongs in one of two places: the checkout, where the
standard tools reclaim it (`cargo clean` for `target/`, deleting `node_modules`), or `/tmp/farhelm-tmp/`, which can be
deleted wholesale and which the machine's temporary-file cleanup reclaims anyway. Nothing bulky goes in `$HOME`
(`~/.cache`, `~/.local`, the checkout's parent directory) or loose in `/tmp`. Before this was enforced, test evidence,
deflake worktrees, tool environments and compiler caches had piled up to about 15 GB under `~/.local` and `~/.cache`,
with nothing that would ever remove them. When a procedure needs a tool environment that its own documentation leaves
the location of to you, the location is your scratch directory.

That covers scratch clones and worktrees or jj workspaces made for a review or a reproduction, a separate
`CARGO_TARGET_DIR`, Python venvs and other tool installs needed only for the task (the test-sleep checker's, for
example), a deflake eval clone, downloaded or extracted archives, saved logs and diffs, draft commit messages, delegate
prompt and result files, and experiment builds. Size does not matter: small files go there too. A `mktemp -d` that a
skill asks for goes inside your directory, not beside it. When you hand work to a delegate or subagent, give it a
subdirectory of yours and tell it to use that instead of minting its own.

When the task that needed something bulky is over, delete it: a scratch jj workspace's `target/` is routinely tens of
gigabytes. Keep what the task still cites, such as a working log or a run you are about to record, until it no longer
does. Never delete another agent's directory.

Two harness-owned trees live beside the agents' directories: the test recorder's evidence in
`/tmp/farhelm-tmp/test-runs/` (`docs/test-run-evidence.md`) and the deflake driver's state in
`/tmp/farhelm-tmp/deflake/` (`deflake/bin/deflake`). Both follow an absolute `TMPDIR` instead of `/tmp`. They belong to
their tools, not to any one agent, and like everything under `/tmp` they do not outlive the machine's temporary-file
cleanup: record what a failure showed, in a fix PR or a FLAKES.md entry, while its run directory still exists.

Docker cannot put images and containers anywhere but its own storage. Give anything you build or start an image tag or
container name that contains your `<id>`, and remove it (`docker rm`, `docker rmi`) when the task is over. The CentOS
gate's `farhelm-centos-ci` image is a cache that script maintains; leave it.

Leave these where they already go:

- The checkout's own build output (`target/`, `e2e/node_modules`, the dx output), which stays wherever the build puts
  it.
- Shared caches of standard tools: cargo's registry, rustup toolchains, npm and bun caches, Playwright's browsers in
  `~/.cache/ms-playwright`, dprint's plugins. Use them the way the tools do by default; do not redirect them.
- State created by the repo's harnesses and scripts: the `fh-it.*` and `fh-e2e.*` test directories, and the desktop
  smoke, CentOS, and installer-test directories. Supervisor and tmux sockets live inside the test directories, unix
  socket paths are limited to about 108 bytes, and the teststate sweep only looks directly under `/tmp`. For the same
  reason, do not point `TMPDIR` at your scratch directory when running tests or repo scripts: plain
  `tempfile::tempdir()` tests honor it, and some of them bind sockets.
- Anything of your own that binds a unix socket, such as a hand-started supervisor or a `tmux -S` server. Put that
  directly under `/tmp` with a random name, because `/tmp/farhelm-tmp/<uuid>/` already spends over 50 of the 108 bytes.
- Anything another rule tells you to keep somewhere specific, such as goal files and working logs.

This rule governs agents, not harness code: a harness still must not use a fixed path, per "Sharing the machine with
other agents".

## Claude Code shell commands: no globs, no expansions

This section applies only to Claude Code sessions and their subagents; other harnesses do not have the problem it works
around. If you are Claude Code, a shell command you compose and run (through your shell tool, or in a prompt to a Claude
Code subagent that will run it) contains no glob and no `$` expansion of any kind: no `*`, `?` or `[...]` patterns, no
variables (`$HOME`, `$d`), no `$(...)` or backticks, no `${...}`, no `$((...))`. Anything that needs one of those is
written as a Deno TypeScript script, in a file, and run as that file. Put it in your scratch directory (see "Agent
scratch space") unless it belongs in the repository. Plain commands with literal arguments (`git status`,
`gh pr view 123`, `cargo nextest run -p farhelm-helm`) and pipelines of them stay fine in the shell.

The reason is that Claude Code can still ask for approval of a shell command when the session runs with every permission
granted, and the maintainer attributes those prompts to shell constructs it treats as unsafe, of which globs and
expansions are the usual ones. The exact rules are Claude Code's and are not documented here. A delegate cannot show
that prompt to anyone, so it sits invisibly until someone happens to look. On 2026-10-11 the v0.27.0 release's
screenshot subagent waited 70 minutes on one `montage $(cat $g)` contact-sheet command that way, and the release notes
waited with it.

The exception is a command this repository's instructions give word for word (the pinned nextest and tmux `PATH` setup,
the release procedure's version checks, the plans flow's ntfy send): run it as written, with only its placeholders
filled in. Those predate this rule and are converted to scripts as they get touched; do not use the exception to compose
new commands in their style.
