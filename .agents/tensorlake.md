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
   project of the suite. `--grep` and `--repeat` apply to every shard and are for narrow reruns of failures; add
   `--reuse-pack` to send the previous run's pack, the same build, because a Rust test build since then relinks
   `target/debug/farhelm` with test features. The script packs the tracked tree, the binaries, the web bundle, the
   pinned tmux and a one-commit Git checkout; balances the projects into shards by spec size (WebKit weighted heavier);
   clones one sandbox per shard; runs all shards at once, each in one attached exec; copies back each shard's
   `summary.txt` and `console.log`, and for a failed shard its retained run directories as `runs.tar.gz`; and terminates
   and sweeps.
3. Read the results like any recorded run. Each shard is its own recorder run, identified by the run ID in its
   `summary.txt`; its record lives on the sandbox and goes with it, except a failed shard's, which comes back as the
   tarball. A failed run's tarball is the retained evidence `.agents/test-evidence.md` requires; keep it until the
   failure is settled. In landing notes and reports, list every shard's run ID, say the runs were on Tensorlake
   sandboxes at the maintainer's request, and note the Node version difference below.

For the browser suite's host-terminal run (`.agents/validation.md`, the `FARHELM_E2E_SUITE_TERMINALS=host` entry), add
`--suite-terminals host`: every shard's script then runs with that variable set and kept through the recorder's scrub,
its `summary.txt` says so (a shard whose summary does not is failed, since it ran the ordinary suite), and with no
projects named the run takes every project of the files `e2e/host-terminal-specs.ts` lists (a project outside them does
not exist in that run).

The shard runner uses the recorder's generic mode (`--kind development` without `--runner playwright`), because the
Playwright mode accepts only spec files and grep selectors and always runs both engines; engine-specific shards need
`--project`. Say so when reporting.

## What the image gives you, and what it does not

Observed on fresh sandboxes in October 2026; verify rather than assume, since Tensorlake changes it without notice.

- The stock image is Ubuntu 24.04 (`tensorlake/ubuntu-minimal`). The user is `tl-user` (uid 1000) with passwordless
  `sudo`. Node 24 (24.16 when observed, against 24.20 on the development machine) and Python 3.12 are installed; Rust,
  tmux, Playwright browsers and an SSH server are not.
- On a sandbox created from the stock image, PID 1 is Tensorlake's own init, not systemd, and nothing boots a systemd
  user manager. The template's packages pull in `systemd-sysv`, which installs `/sbin/init`, and clones of the template
  boot it (observed November 2026), so on a clone PID 1 is a real systemd with logind. The supervisor puts sessions in
  transient systemd user scopes; without a user manager it runs them unscoped and scope-dependent tests return early,
  which the evidence rules do not accept as a pass (`.agents/validation.md`), so the shard runner makes sure a user
  manager runs (below).
- Tensorlake's in-sandbox daemon runs its own `sshd` on port 22, for its ssh gateway, with a host key and an
  `authorized_keys` file under `/run/tl/ssh/` (seen on clones in November 2026; it was not there when the template was
  built in October). On a clone it takes the port before the image's `ssh.socket` can, which then stays failed. I have
  not checked whether a fresh stock-image sandbox has it too; the shard runner handles either (below).
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
   leaving dpkg half-configured. On a sandbox without systemd as PID 1 (the template sandbox), the shard runner creates
   that directory, so one that has run a shard needs it removed before any later apt run. On a clone, where PID 1 is
   systemd, the directory is systemd's own: leave it, and package scripts reach the running manager.
3. Push and unpack the source tree, binaries and web bundle, and a one-commit Git checkout (the script's `pack`). The
   recorder identifies the tested commit through Git and finds the pinned tmux through the checkout; without `.git` it
   refuses with "Git checkout unavailable" and "repository pinned tmux path is unavailable". The checkout is a shallow
   clone of the local checkout's HEAD, so the sandbox never contacts the repository's remote.
4. In `e2e/`, `npm ci`, then `sudo -n env PATH=<path> npx playwright install-deps chromium webkit` and
   `npx playwright install chromium webkit`. `install-deps` needs root and the caller's PATH to find npx.
5. Give `tl-user` passwordless SSH to itself: an ed25519 key, the public key in `authorized_keys`, and `ssh-keygen -A`
   for host keys. The harness registers its second supervisor as an ssh-to-localhost host; without self-ssh the
   multi-host specs fail or skip, so a shard refuses to run without it. That key in `~/.ssh/authorized_keys` serves the
   image's own `sshd`; where Tensorlake's `sshd` holds port 22, each shard authorizes the key there itself ("What every
   shard does first"). Also create the empty file `/var/lib/systemd/linger/tl-user` (what `loginctl enable-linger`
   records), so that logind on a clone starts `tl-user`'s user manager at boot (`bootstrap.sh` does it right after apt).
6. Run one small shard as a probe (a couple of quick specs on both engines) and check that `summary.txt` says the user
   manager is `running` and self-ssh is `ok`.
7. Remove the probe's output, `tl sbx checkpoint <name>`, wait until `tl sbx checkpoint ls` shows the snapshot
   completed, then terminate the sandbox and delete its automatic suspend snapshot. Record the template's snapshot ID.

## What every shard does first

When PID 1 is systemd (a clone), `run-shard.sh` waits for logind and runs `loginctl enable-linger`, so logind keeps the
user manager running without a login session, in place of the first two steps below. It starts no `sshd` there, since
port 22 is already served (by Tensorlake's `sshd` now, by `ssh.socket` before it); it still sets up and checks self-ssh
as described below. Lingering matters: without it every ssh-to-localhost login starts a user manager that logind stops
ten seconds after the session ends, taking every session scope in it along. The hand-built manager is fatal on a clone:
the self-ssh login makes logind start the real `user@1000.service` in the cgroup the script took over, and when logind
stops it, systemd kills every process in it, the test run included.

On either kind of sandbox, the shard fails before running any test, with `setup failed:` in `summary.txt`, when
lingering cannot be enabled or the user manager is not `running` or `degraded`: the supervisor would otherwise run
sessions without scopes and the specs could still pass. `summary.txt` records PID 1, the lingering result and the
manager's state.

The shard also fails before any test when self-ssh to localhost does not work, and `summary.txt` says `self-ssh: FAILED`
with ssh's last error line. This used to be only a summary line, and in November 2026 every shard ran for a while with
self-ssh broken: the multi-host specs in `settings.spec.ts` failed on a fixture host stuck connecting, and specs that
skip without a second host would have passed. Here's what the script does, on either kind of sandbox, whichever server
answers on port 22:

- Removes every `localhost` and `127.0.0.1` entry from `tl-user`'s `known_hosts`. The template's file holds the key of
  the server that answered when it was built; replacing it leaves exactly the key this sandbox serves, and keeps
  repeated shards on one sandbox from piling up entries.
- Scans `127.0.0.1` itself and records the keys under both `localhost` and `127.0.0.1`. `ssh-keyscan localhost` tries
  only the first address `localhost` resolves to, `::1`, where Tensorlake's `sshd` does not listen, and then records
  nothing; `ssh localhost` falls back to `127.0.0.1` and checks its key under both names.
- When `/run/tl/ssh/authorized_keys` exists, appends `tl-user`'s public key to it. Tensorlake's `sshd` reads only that
  file, for every user, so the key bootstrap put in `~/.ssh/authorized_keys` is not enough. Whether the platform ever
  rewrites the file while a sandbox runs is unknown; it did not during the runs observed.
- Logs in with `ssh -o BatchMode=yes localhost true`, and records which server it reached as `self-ssh: ok (...)`.

When PID 1 is Tensorlake's init (the template sandbox during its probe), an earlier exec's files stay, its processes
cannot be relied on, and services must live inside the exec that runs the tests. `run-shard.sh` therefore, every time:

- Removes leftovers from an earlier exec: a stale session-bus socket (it refuses connections) and the previous user
  manager's cgroup tree. Cgroup v2 refuses to move a process into a group that has child groups with controllers enabled
  ("Device or resource busy"), so the tree is removed children first.
- Starts a systemd user manager: creates `/run/systemd/system` (all `sd_booted()` checks for), `/run/user/1000`, a
  delegated cgroup `user.slice/user-1000.slice/user@1000.service` with `+cpu +memory +pids` enabled in its parents and
  owned by `tl-user`, moves its own shell into it, starts a session bus, then `/usr/lib/systemd/systemd --user`, and
  waits for `systemctl --user is-system-running` to say `running` or `degraded`.
- Starts `sshd` (it needs `/run/sshd`) unless an `sshd` already runs, then sets up self-ssh as above.

## Tensorlake CLI traps

- `tl sbx cp` copies one file; directories fail with a misleading "file not found". Move trees as tarballs.
- `tl sbx exec` forwards no stdin, and its own options (`-t`, `-e`, `-w`) go before the sandbox name; everything after
  the name is the command. `tl` also parses options after the name as its own, so `tl sbx exec NAME df -h` prints `tl`'s
  help: wrap such commands in `sh -c '...'`.
- Keep a whole shard in one attached exec. What an exec leaves running is frozen when the sandbox idles and cannot be
  relied on afterwards. Do not rely on it being gone either: an `sshd` started by one exec was still running in the next
  one on the template sandbox (November 2026), and on a clone the lingering user manager and any user scopes belong to
  systemd and outlive the exec.
- Snapshots survive termination, including the automatic suspend snapshots the platform makes when a named sandbox
  idles. Sweep them by the IDs in your ledger (`sandbox-tests.ts sweep --scratch <dir> --keep <template>`).
