# Rust tests on Tensorlake sandboxes

NOTE: This is not the default way to run tests. Use it only when the maintainer asks for Tensorlake in the task at hand
(for example "use tensorlake for the landing checks"). Without that, run checks locally as `.agents/validation.md`
describes.

Tensorlake sandboxes are cloud virtual machines billed per second, so a nextest archive built here can be split into
hash partitions that run on several sandboxes at once. `scripts/tensorlake/rust-tests.ts` (Deno) drives that from the
development machine, and `scripts/tensorlake/sandbox-tests.ts` builds the sandbox template it clones and holds the
ledger and sweep both share. `bootstrap.sh` and `run-rust-shard.sh` beside them run on the sandbox and are Bash, because
the stock image has no Deno. This file is what an agent needs to run them, and to rebuild the sandbox template from
scratch when the stored one is gone or stale.

## When and how much

- Only on request. The template carries rootless Podman and the exact cached production base, so the real environment
  e2e test runs there too. Builds, lints, doctests and the UI's desktop tests stay local: the sandboxes have no Rust
  toolchain and no WebKitGTK.
- The maintainer's request states a budget: how many sandboxes may exist at once. Stay at or below it (`--shards`).
- Name owned sandboxes `farhelm-sbx-<checkout>-<n>` (`--prefix farhelm-sbx-<checkout>-`), where `<checkout>` is the
  checkout directory's name lowercased with everything outside `[a-z0-9]` turned into `-`. A sandbox with that prefix
  that this session did not create belongs to another agent: leave it alone. A maintainer's request may name a different
  prefix; use it.
- The script records every sandbox it creates (name, ID, disk size) in `<scratch>/ledger.jsonl` before using it.
  Snapshots outlive their sandboxes and bill storage, and once a sandbox is gone its ID is the only way to tell which
  snapshots were yours. Keep the ledger for the whole session.
- `rust-tests.ts run` terminates its sandboxes when their shards end, including after failures, and then deletes the
  snapshots whose source is in the ledger, except the template and its own staging snapshot (below). If a run is
  interrupted, run `sandbox-tests.ts sweep` (below) before anything else. Never delete a snapshot or sandbox you cannot
  attribute from the ledger.

## Running the Rust tests on sandboxes

`scripts/tensorlake/rust-tests.ts` runs a nextest archive built here across sandboxes, one hash partition each, and
`run-rust-shard.sh` is its sandbox side. On its first use (2026-11-08), the helm, protocol, supervisor, terminal
emulator and farhelm crates' 4,660-odd tests ran on ten sandboxes in about a minute and a half each, once the archive
was there.

1. With the pinned nextest on `PATH` (`docs/test-run-evidence.md`) and an empty working copy of the revision under test,
   `cargo build` (the e2e harness needs `target/debug/farhelm-fixtures`, which nextest's setup script would otherwise
   build), then `cargo nextest archive --locked -p <package> ... --archive-file <file>`. The Rust packer also prepares
   this revision's static native-musl CLI payload through `scripts/build-podman-test-payloads.sh`. Install `musl-tools`,
   `file` and binutils locally. A matching absolute `FARHELM_HELM_PAYLOAD_DIR` can supply this ordinary installer input.
   The packer stages it once even for a narrow initial filter, because the staging snapshot can later run another
   selection from the same archive. An intentional `FARHELM_TEST_SKIP_PODMAN=1` avoids this build and is forwarded to
   every shard. Record the missing real-environment coverage.
2. `scripts/tensorlake/rust-tests.ts run --scratch <dir> --template <snapshot> --prefix <name-prefix> --shards <n> --archive <file> --out <dir> [-- <nextest filter arguments>]`.
   It uploads the archive (about a gigabyte), source, tools and static provisioning payloads once, to a staging sandbox,
   snapshots that, and clones the snapshot for every shard. The staging snapshot is kept, so rerunning the same archive
   (a narrower filter, say) passes `--staged <snapshot>` to reuse the existing uploads. `sandbox-tests.ts sweep` removes
   it with the rest. A staged snapshot freezes those inputs and the worker script. Create a fresh stage after changing
   them, or to enable Podman coverage when the stage was created with the opt-out.
3. Read each shard's `summary.txt` like a recorded run: the user manager's state, Podman and its image, self-ssh, then
   nextest's summary, failures and any runtime `SKIPPED` lines. A failed shard's run directories come back as
   `runs.tar.gz`; keep them as evidence. Shards run through the recorder's generic mode, because its nextest mode does
   not take an archive; say so when reporting, with every shard's run ID.

How it works, and why: the test binaries have absolute paths compiled in (fixture directories, `CARGO_BIN_EXE_*`, the
target directory), so the staging sandbox unpacks the checkout at this checkout's own absolute path and each shard
extracts the archive over it, with nothing remapped. Nextest's e2e setup script runs `cargo build -p farhelm-fixtures`.
Cargo is unavailable on clones, so a shard uses a copy of `.config/nextest.toml` with that fixture-build hook removed.
The retained Podman setup consumes prebuilt payloads. The worker retains `FARHELM_TEST_PODMAN_PAYLOAD_DIR` through the
recorder, and setup exposes the normal `FARHELM_HELM_PAYLOAD_DIR` only to matching tests through nextest's setup
environment. The real Podman fixture reserves all four nextest slots and has a 40-minute limit. A test that passes
locally and fails there is still a finding: the sandboxes are less loaded than the shared development machine, so races
that the slow machine hides show up (on first use, three e2e tests that stopped a session soon after creating it).

Give concurrent runs separate `--scratch` directories and name prefixes: each run's sweep terminates every live sandbox
in its own ledger.

## What the image gives you, and what it does not

Observed on fresh sandboxes in October 2026; verify rather than assume, since Tensorlake changes it without notice.

- The stock image is Ubuntu 24.04 (`tensorlake/ubuntu-minimal`). The user is `tl-user` (uid 1000) with passwordless
  `sudo`. Node 24 and Python 3.12 are installed; Rust, tmux and an SSH server are not.
- On a sandbox created from the stock image, PID 1 is Tensorlake's own init, not systemd, and nothing boots a systemd
  user manager. The template's packages pull in `systemd-sysv`, which installs `/sbin/init`, and clones of the template
  boot it (observed November 2026), so on a clone PID 1 is a real systemd with logind. The supervisor puts sessions in
  transient systemd user scopes; without a user manager it runs them unscoped and scope-dependent tests return early,
  which the evidence rules do not accept as a pass (`.agents/validation.md`), so the shard runner runs only on clones
  and makes sure a user manager runs (below).
- The glibc matches Ubuntu 24.04 development machines (2.39), so the locally built debug `farhelm` and
  `farhelm-fixtures` run there unchanged. Pushing them saves a Rust toolchain and a half-hour build on every sandbox. If
  the development machine moves to a newer Ubuntu, this stops being true: build on the sandbox instead, or pick a
  matching image.
- The stock image lacks rootless Podman. The template installs it with the production base the environment test uses
  (`docker.io/library/ubuntu@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852`). The production
  readiness probe requires Podman at least 4.9.3, cgroup v2, the systemd cgroup manager and usable user scopes. The
  managed-checkout/lifecycle fixture also needs engine events. Its ignored SSH acceptance variant requires explicit
  selection and the strict passwordless localhost SSH setup described below. Both variants exercise ordinary tab, file,
  transfer and TCP-relay protocols with a stand-in agent. The shard preflight requires the exact cached base unless the
  explicit Podman opt-out is set. On clones, `/dev/net/tun` comes up readable by root only and no udevd runs to apply a
  rule, so `run-rust-shard.sh` opens it up before the tests (without that, Podman's networking fails with
  "open(/dev/net/tun): Permission denied").
- On clones, systemd-resolved owns `/etc/resolv.conf` and was seen with no upstream server, so nothing resolved; the
  gateway does not answer DNS either. The template gives resolved public servers. The real environment recipe uses the
  network for apt and both official agent installers, and its managed checkout clones a public GitHub repository, so DNS
  and outbound access are test prerequisites. Agent turns use a stand-in.
- Sizes: create with `-c 4 -m 8192`. Disk is chosen at creation and a clone cannot be smaller than its snapshot, so the
  template stays at 10 GiB, the platform minimum. The Rust runner gives its staging sandbox and clones 60 GiB, because
  the staged checkout plus the extracted archive take about 25.

## Building the template by hand

`scripts/tensorlake/sandbox-tests.ts make-template --scratch <dir> --name <sandbox>` does this and prints the snapshot
ID; it needs no local build. If it fails or the platform has changed, these are the steps and why each exists:

1. Create a named sandbox (`tl sbx create --timeout 60 -c 4 -m 8192 --disk_mb 10240 <name>`). Named, because an unnamed
   one is destroyed at its idle timeout. The 60-second idle timeout cannot be changed later.
2. Install packages:
   `build-essential ca-certificates curl git rsync systemd dbus dbus-user-session openssh-server python3`, plus
   `podman uidmap passt slirp4netns fuse-overlayfs tmux` for the Rust tests (tmux for the few that start a real tmux
   server; a template built before it was on this list has none, so rebuild it), and a resolved drop-in naming public
   DNS servers (see above). Before running apt, make sure `/run/systemd/system` does not exist and run
   `dpkg --configure -a`: when that directory exists, libc6's install script believes systemd is running, calls it, and
   fails with "Failed to connect to bus", leaving dpkg half-configured. On a clone, where PID 1 is systemd, the
   directory is systemd's own: leave it, and package scripts reach the running manager.
3. As `tl-user`, run
   `podman pull docker.io/library/ubuntu@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852`. The
   environment fixture requires that prerequisite to be present before it starts. Its recipe build then uses the
   network.
4. Give `tl-user` passwordless SSH to itself: an ed25519 key, the public key in `authorized_keys`, and `ssh-keygen -A`
   for host keys. The e2e tests that register a second supervisor as an ssh-to-localhost host print `SKIPPED` without
   self-ssh. Also create the empty file `/var/lib/systemd/linger/tl-user` (what `loginctl enable-linger` records), so
   that logind on a clone starts `tl-user`'s user manager at boot (`bootstrap.sh` does it right after apt).
5. `tl sbx checkpoint <name>`, wait until `tl sbx checkpoint ls` shows the snapshot completed, then terminate the
   sandbox and delete its automatic suspend snapshot. Record the template's snapshot ID.

Nothing proves the template before its first use: the template sandbox boots Tensorlake's own init, not systemd, so it
cannot run a shard. The first Rust run on its clones is the proof. Check that every shard's `summary.txt` says the user
manager is `running` or `degraded`, the Podman image is `present`, self-ssh is `ok` and tmux has a version.

## What every shard does first

`run-rust-shard.sh` runs only on clones, where PID 1 is systemd. It waits for logind and runs `loginctl enable-linger`,
so logind keeps the user manager running without a login session. Lingering matters: without it every ssh-to-localhost
login starts a user manager that logind stops ten seconds after the session ends, taking every session scope in it
along.

The shard fails before running any test, with `setup failed:` in `summary.txt`, when PID 1 is not systemd, lingering
cannot be enabled, the user manager is not `running` or `degraded`, `/dev/net/tun` cannot be opened up, or the cached
Podman base is missing without the explicit opt-out: the supervisor would otherwise run sessions without scopes and the
tests could still pass. `summary.txt` records PID 1, the lingering result, the manager's state and Podman's.

Self-ssh then uses an `sshd` of the shard's own on port 2222, never port 22. On clones that boot systemd (observed
November 2026), something other than the image's `sshd` already listens on port 22: `ssh.socket` fails to start, and
what answers presents a host key of its own and refuses `tl-user`'s key ("Permission denied (publickey)"). The tests'
second host is plain `ssh localhost`, which reads `~/.ssh/config`, so `run-rust-shard.sh` starts the image's `sshd` with
`-p 2222` unless something already listens there, adds a `Host localhost 127.0.0.1` block with `Port 2222` to
`~/.ssh/config` once, drops earlier `known_hosts` entries for those names (a template's file can hold keys from an
earlier boot, and ssh refuses on a stale entry rather than trying an appended one), and records the current keys with
`ssh-keyscan -4 -p 2222` (without `-4`, `ssh-keyscan localhost` tries ::1 alone). `summary.txt` says `self-ssh: ok` or
`FAILED`; a failed one leaves the tests that use the second host printing `SKIPPED` (and the explicitly selected SSH
variant of the Podman test failing), so a run with it is not evidence for those tests.

## Tensorlake CLI traps

- `tl sbx cp` copies one file; directories fail with a misleading "file not found". Move trees as tarballs.
- One `tl sbx cp` moved about 0.7 MiB/s from the development machine however fast its link was (2026-11-08), and eleven
  at once about 8 MiB/s in total: the cap is per transfer. `uploadChunked` in `sandbox-tests.ts` sends large files as
  parallel 100 MiB chunks, which took a 1.1 GB archive from about 25 minutes to under three.
- `tl sbx exec` forwards no stdin, and its own options (`-t`, `-e`, `-w`) go before the sandbox name; everything after
  the name is the command. `tl` also parses options after the name as its own, so `tl sbx exec NAME df -h` prints `tl`'s
  help: wrap such commands in `sh -c '...'`.
- Keep a whole shard in one attached exec. What an exec leaves running is frozen when the sandbox idles and cannot be
  relied on afterwards. Do not rely on it being gone either: an `sshd` started by one exec was still running in the next
  one on the template sandbox (November 2026), and on a clone the lingering user manager and any user scopes belong to
  systemd and outlive the exec.
- Snapshots survive termination, including the automatic suspend snapshots the platform makes when a named sandbox
  idles. Sweep them by the IDs in your ledger (`sandbox-tests.ts sweep --scratch <dir> --keep <template>`).
