# Slow tests on an SSH test worker

NOTE: This is not the default way to test. Use it only when the maintainer asks for the test worker in the task at hand
(the landing monitor records a standing request in its log). Without that, run checks locally as `.agents/validation.md`
describes. Boat sandboxes (`.agents/boat.md`) are the other remote option; which one a flow uses is the maintainer's
choice, and both stay supported.

The test worker is a Linux machine the maintainer runs, reached over plain ssh or `tailscale ssh`, that builds and tests
the working copy itself. Unlike a boat sandbox, it does not run binaries built on the development machine, so it takes
the builds off that machine too, and its architecture can differ from it (the first one is an arm64 Ubuntu VM).
`scripts/test-worker/test-worker.sh` drives it; its header is the command reference.

## The worker's name stays out of the repository

The worker is named in `~/.farhelm-test-worker` on the development machine, never in a file in the repository, a log
that lands, a report or a PR: host names are private (`AGENTS.md`). The file holds `key=value` lines:

```
destination=user@host
transport=tailscale
```

`transport` is `tailscale` (the wrapper verifies the host key against the tailnet's) or `ssh` (the default, with the
ordinary `known_hosts` check). A missing file means there is no worker; say so rather than guessing one.

## What it may do on the worker

The worker follows the `scode-ssh-delegate` skill's trust rules: source is pushed from the local checkout (the working
copy's tracked files only), nothing on the worker gets GitHub, cloud or registry credentials or contacts a private
remote, and only summaries, console logs and a failed run's retained recorder directories come back. It keeps one tree
per checkout name under `~/farhelm-test-worker/<checkout>/`, with its own `target/` so builds stay incremental. Each
invocation stages its tools and tracked files in a private directory. A detached controller takes the checkout's lock
before it installs the source and takes the run through completion. The install preserves file mtimes. A contender that
reaches a held lock fails and names the holder, leaving the running checkout and tools intact.

Assume the worker can be reset to a fresh image at any time. Nothing on it is precious: every run syncs the working copy
again, and a lost `target/` only makes the next build cold. Before each run the worker checks, under its setup lock, a
marker the bootstrap writes last, holding a hash of `scripts/test-worker/remote/bootstrap.sh`; when it is missing (a
fresh worker) or names another version of the script, the run bootstraps first, which takes a few minutes. `bootstrap`
also runs on its own. Setup calls queue behind each other. Setup needs exclusive use of the worker and immediately names
the active runs if tests are already running. A run arriving during setup waits up to five minutes before reporting that
setup is busy. The detached controller closes its lock descriptors in the test command, so a surviving test subprocess
cannot keep other checkouts from bootstrapping. Bootstrap installs the packages the Rust suite and the GPUI app need,
rootless Podman with the exact production base image, tmux, Mutter, Mesa's software renderer, Bun, a static BusyBox, an
OpenSSH server and passwordless self-ssh for the second-host tests, lingering for the account's systemd user manager,
and rustup (the toolchain follows `rust-toolchain.toml` on first use). It needs Ubuntu 24.04 or newer and passwordless
sudo.

## Running

- The Rust workspace suite: `scripts/test-worker/test-worker.sh rust --out <dir> [-- <nextest arguments>]`. It syncs the
  working copy and runs `cargo nextest run --workspace --exclude farhelm-desktop` there through the recorder's nextest
  mode, with four slots, so the selection and evidence match a local run.
- The GPUI app's end-to-end suites: `test-worker.sh gui --out <dir> [smoke] [lifecycle]`, which builds the app there.
- Anything else from the synced tree: `test-worker.sh run --out <dir> -- <command...>`, for a clippy run, a narrow test
  selection through the recorder, or the vendored crates' tests.
- `test-worker.sh stop` ends the run holding the lock. Interrupting the script locally does not end the remote run,
  which keeps building and holding the lock until it finishes. During preparation, Stop records a cancellation so tests
  never start. In a running suite the recorder gets time to stop its command and save evidence. The worker keeps both
  locks until the process group has stopped, including when its foreground shell exits first. After a forced stop the
  controller retains whatever the workload wrote before it was killed.
- `test-worker.sh status` shows the checkout's lock holder and its recent summaries.
- `test-worker.sh collect --out <dir> <id>` retrieves a finished run after a client disconnect or copy failure. Use the
  invocation id printed at startup. Collection reports when the run is still active or its evidence is gone.

A full build of the workspace with the GUI and its tests takes about 55 GB of `target/` on the worker, and incremental
builds only add to it, so every run (`rust`, `gui` and `run` alike) starts by holding its checkout to its own share of
the disk: its build output (`target/` and the vendored crates' own `target/` directories) is at most 60 GB, and the run
starts with 55 GB free, room for a cold build. When either is missed the run cleans up after itself, removing rustc's
incremental caches first and all of its build output only if that is not enough, and its summary's `disk:` line says
what it did. A checkout's build output can therefore not grow round after round until the disk fills; the check runs
only at a run's start, though, so two checkouts building at once can still need more room than each saw. If the room is
still missing once the checkout's output is gone, the run stops at once with status 75 and a
`disk: FULL BY OTHER WORKLOADS` line naming how much the rest of the disk uses; it never touches another checkout's
tree.

Read each `summary.txt` like a recorded run: the substrate lines (architecture, the systemd user manager, Podman, tmux,
self-ssh), the summary and failures, and the runtime `SKIPPED` count from the retained output. A failed run's recorder
directories come back as `runs.tar.gz`; keep them as evidence. Report the worker's runs by their recorder run ids and
say they ran on the test worker, with its architecture.

The command prints the summary from its worker. Each invocation has its own evidence. The driver clears the harness's
prior archive, summary and logs from a reused output directory and writes `invocation.txt` with the source commit and
result. A refused request leaves its own stamp and copies no evidence. If the copy fails, the driver reports it and the
status returned by the worker. The worker retains the newest 20 invocation directories per checkout under its tree lock.
That lock also guards sweeping day-old abandoned stages, and each upload holds the stage lock while it writes.

Only one client can own a local output directory at a time. A concurrent request for that same directory fails before
changing its files, so its stamp and artifacts always come from one invocation.

## When the worker cannot be used

An unattended flow never waits on the worker. When it is unreachable, its lock is held by something else, bootstrap
fails, or its setup is broken in a way the maintainer can likely fix quickly (a stopped VM, a disk that a run reports
`FULL BY OTHER WORKLOADS`, a lost tailnet login), notify the maintainer with the problem in one line (`plans/AGENTS.md`,
"Notifications"). If nothing changes within 10 minutes, run that round's suites the other way (boats, when the
maintainer has allowed them, or locally) and say so in the round's notes; this is the maintainer's choice of 2026-11-19,
so that a flow is never stuck on the worker. A test that fails on the worker is a finding like any other, not a reason
to switch.

## What it does not cover

- The GPUI goldens stay local: they compare pixels with references fetched through the maintainer's GitHub access, which
  the worker never gets, and the references were rendered on x86_64.
- A worker on another architecture than the release's x86_64 Linux gate tests that architecture only. x86_64-only
  breakage then shows first at the release gate; say so when it matters to the change.
- The macro compile-contract test resolves crates from the worker's own cargo cache, so it can run there.
- Anything on a Mac.
