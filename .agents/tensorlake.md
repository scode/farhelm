# Browser tests on Tensorlake sandboxes

NOTE: This is not the default way to run tests. Use it only when the maintainer asks for Tensorlake in the task at hand
(for example "use tensorlake for the landing checks"). Without that, run checks locally as `.agents/validation.md`
describes.

The whole browser suite takes two to four hours on the shared development machine, because every run boots exactly three
stacks (`STACK_COUNT` in `e2e/stack-slots.ts`) and WebKit is slow. Tensorlake sandboxes are cloud virtual machines
billed per second; each one runs its own three stacks, so the suite's (engine, spec file) projects can be split across
several sandboxes and run at once. On its first use, 42 projects ran across ten sandboxes in about ten minutes, against
about 45 locally. `scripts/tensorlake/sandbox-tests.ts` (Deno) drives it from the development machine; `bootstrap.sh`
and `run-shard.sh` beside it run on the sandbox and are Bash, because the stock image has no Deno. This file is what an
agent needs to run them, and to rebuild the sandbox template from scratch when the stored one is gone or stale.

## When and how much

- Only on request, and only for browser specs. The Rust workspace stays local: its container tests need rootless Podman,
  which the sandboxes do not have, and it takes ten to twenty minutes anyway.
- The maintainer's request states a budget: how many sandboxes may exist at once. Stay at or below it (`--shards`).
- Name owned sandboxes `farhelm-sbx-<checkout>-<n>` (`--prefix farhelm-sbx-<checkout>-`), where `<checkout>` is the
  checkout directory's name lowercased with everything outside `[a-z0-9]` turned into `-`. A sandbox with that prefix
  that this session did not create belongs to another agent: leave it alone. A maintainer's request may name a different
  prefix; use it.
- The script records every sandbox it creates (name, ID, disk size) in `<scratch>/ledger.jsonl` before using it.
  Snapshots outlive their sandboxes and bill storage, and once a sandbox is gone its ID is the only way to tell which
  snapshots were yours. Keep the ledger for the whole session.
- `run` terminates its sandboxes when their shards end, including after failures, and then deletes the snapshots whose
  source is in the ledger, except the template. If a run is interrupted, run `sweep` (below) before anything else. Never
  delete a snapshot or sandbox you cannot attribute from the ledger.

## Running a sharded browser run

1. Build locally as for any browser run (`cargo build`, the release `dx build`), on the revision you are testing, with
   an empty working copy. The script packs tracked files only, but the binaries and web bundle are whatever the build
   left, so they must come from that revision.
2. `scripts/tensorlake/sandbox-tests.ts run --scratch <dir> --template <snapshot> --prefix <name-prefix> --shards <n> --out <dir> [--grep <title>] [--repeat <n>] [project ...]`.
   Projects are named as in `e2e/playwright.config.ts` (`chromium-settings`, `webkit-settings`); with none it runs every
   project of the suite. `--grep` and `--repeat` apply to every shard and are for narrow reruns of failures. The script
   packs the tracked tree, the binaries, the web bundle, the pinned tmux and a one-commit Git checkout; balances the
   projects into shards by spec size (WebKit weighted heavier); clones one sandbox per shard; runs all shards at once,
   each in one attached exec; copies back each shard's `summary.txt` and `console.log`, and for a failed shard its
   retained run directories as `runs.tar.gz`; and terminates and sweeps.
3. Read the results like any recorded run. Each shard is its own recorder run, identified by the run ID in its
   `summary.txt`; its record lives on the sandbox and goes with it, except a failed shard's, which comes back as the
   tarball. A failed run's tarball is the retained evidence `.agents/test-evidence.md` requires; keep it until the
   failure is settled. In landing notes and reports, list every shard's run ID, say the runs were on Tensorlake
   sandboxes at the maintainer's request, and note the Node version difference below.

The shard runner uses the recorder's generic mode (`--kind development` without `--runner playwright`), because the
Playwright mode accepts only spec files and grep selectors and always runs both engines; engine-specific shards need
`--project`. Say so when reporting.

## What the image gives you, and what it does not

Observed on fresh sandboxes in October 2026; verify rather than assume, since Tensorlake changes it without notice.

- The stock image is Ubuntu 24.04 (`tensorlake/ubuntu-minimal`). The user is `tl-user` (uid 1000) with passwordless
  `sudo`. Node 24 (24.16 when observed, against 24.20 on the development machine) and Python 3.12 are installed; Rust,
  tmux, Playwright browsers and an SSH server are not.
- PID 1 is Tensorlake's own init, not systemd, and nothing boots a systemd user manager. The supervisor puts sessions in
  transient systemd user scopes and falls back to an early-return mode without them, which the evidence rules do not
  accept as a pass (`.agents/validation.md`), so the shard runner starts one itself (below).
- The glibc matches Ubuntu 24.04 development machines (2.39), so the locally built debug `farhelm` and
  `farhelm-fixtures`, and the pinned tmux in `.ci-tmux/`, run there unchanged. Pushing them saves a Rust toolchain and a
  half-hour build on every sandbox. If the development machine moves to a newer Ubuntu, this stops being true: build on
  the sandbox instead, or pick a matching image.
- Sizes: create with `-c 4 -m 8192`; three stacks fit in 8 GiB. Disk is chosen at creation and a clone cannot be smaller
  than its snapshot, so the template stays at 10 GiB (it uses about 4) and clones get 30 GiB for retained traces and
  screenshots.

## Building the template by hand

`scripts/tensorlake/sandbox-tests.ts make-template --scratch <dir> --name <sandbox>` does this and prints the snapshot
ID; it needs the same local build as a run. If it fails or the platform has changed, these are the steps and why each
exists:

1. Create a named sandbox (`tl sbx create --timeout 60 -c 4 -m 8192 --disk_mb 10240 <name>`). Named, because an unnamed
   one is destroyed at its idle timeout. The 60-second idle timeout cannot be changed later.
2. Install packages:
   `build-essential ca-certificates curl git rsync systemd dbus dbus-user-session openssh-server python3`. Before
   running apt, make sure `/run/systemd/system` does not exist and run `dpkg --configure -a`: when that directory
   exists, libc6's install script believes systemd is running, calls it, and fails with "Failed to connect to bus",
   leaving dpkg half-configured. The shard runner creates that directory, so a sandbox that has run a shard needs it
   removed before any later apt run.
3. Push and unpack the source tree, binaries and web bundle, and a one-commit Git checkout (the script's `pack`). The
   recorder identifies the tested commit through Git and finds the pinned tmux through the checkout; without `.git` it
   refuses with "Git checkout unavailable" and "repository pinned tmux path is unavailable". The checkout is a shallow
   clone of the local checkout's HEAD, so the sandbox never contacts the repository's remote.
4. In `e2e/`, `npm ci`, then `sudo -n env PATH=<path> npx playwright install-deps chromium webkit` and
   `npx playwright install chromium webkit`. `install-deps` needs root and the caller's PATH to find npx.
5. Give `tl-user` passwordless SSH to itself: an ed25519 key, the public key in `authorized_keys`, and `ssh-keygen -A`
   for host keys. The harness registers its second supervisor as an ssh-to-localhost host; without self-ssh the
   multi-host specs skip.
6. Run one small shard as a probe (a couple of quick specs on both engines) and check that `summary.txt` says the user
   manager is `running` and self-ssh is `ok`.
7. Remove the probe's output, `tl sbx checkpoint <name>`, wait until `tl sbx checkpoint ls` shows the snapshot
   completed, then terminate the sandbox and delete its automatic suspend snapshot. Record the template's snapshot ID.

## What every shard does first

Processes do not survive from one exec to the next, but their files do, and services must live inside the exec that runs
the tests. `run-shard.sh` therefore, every time:

- Removes leftovers from an earlier exec: a stale session-bus socket (it refuses connections) and the previous user
  manager's cgroup tree. Cgroup v2 refuses to move a process into a group that has child groups with controllers enabled
  ("Device or resource busy"), so the tree is removed children first.
- Starts a systemd user manager: creates `/run/systemd/system` (all `sd_booted()` checks for), `/run/user/1000`, a
  delegated cgroup `user.slice/user-1000.slice/user@1000.service` with `+cpu +memory +pids` enabled in its parents and
  owned by `tl-user`, moves its own shell into it, starts a session bus, then `/usr/lib/systemd/systemd --user`, and
  waits for `systemctl --user is-system-running` to say `running`.
- Starts `sshd` (it needs `/run/sshd`) and adds localhost's host keys to `known_hosts`.

## Tensorlake CLI traps

- `tl sbx cp` copies one file; directories fail with a misleading "file not found". Move trees as tarballs.
- `tl sbx exec` forwards no stdin, and its own options (`-t`, `-e`, `-w`) go before the sandbox name; everything after
  the name is the command. `tl` also parses options after the name as its own, so `tl sbx exec NAME df -h` prints `tl`'s
  help: wrap such commands in `sh -c '...'`.
- Keep a whole shard in one attached exec. Anything backgrounded is frozen when the sandbox idles and gone by the next
  exec.
- Snapshots survive termination, including the automatic suspend snapshots the platform makes when a named sandbox
  idles. Sweep them by the IDs in your ledger (`sandbox-tests.ts sweep --scratch <dir> --keep <template>`).
