# Slow tests on boat sandboxes

NOTE: This is not the default way to test. Use it only when the maintainer asks for boats in the task at hand (the
landing monitor has standing permission, recorded in its log). Without that, run checks locally as
`.agents/validation.md` describes. The SSH test worker (`.agents/test-worker.md`) is the other remote option.

Boat sandboxes (https://docs.boat.dev) are full Ubuntu 24.04 VMs driven through the `boat` CLI, which must be installed
and signed in. `scripts/boat/boat-tests.ts` (Deno) runs two kinds of work on them from the development machine, against
binaries built there:

- hash partitions of a Rust nextest archive (the workspace run, or any selection from it), one partition per sandbox;
- the GPUI app's end-to-end suites (`scripts/gui-e2e-smoke.py` and `scripts/gui-lifecycle.py`) on one sandbox;
- the docs website's screenshot captures (`scripts/docs-screenshots.sh`), one page or the whole set, spread over the
  pool (`website/.agents/refreshing.md`, "With N boats").

Builds, lints, doctests and the GPUI goldens stay local. The sandboxes have a Rust toolchain, but a cold build there
costs more than the run it would save, and the goldens compare pixels with references fetched through the maintainer's
GitHub access, which a sandbox never gets (`scode-ssh-delegate`'s trust rules: no GitHub, cloud or registry credentials
on a worker).

## Cost rules, and why the pool exists

Boat charges for every sandbox start, not only for running time: the account's plan includes a number of starts per
rolling minute, hour and day, and starts beyond those cost money or are refused. Running time is cheap (a `large`
sandbox, 8 vCPU and 16 GB, costs about 7 cents an hour). A sandbox does not stop when idle; it runs until deleted or
until its hard deadline (its TTL, counted from creation), which stops it even in the middle of a command.

So the harness keeps a pool: `up` (or the first run that needs one) creates sandboxes and bootstraps them once, every
later run resets and reuses them, and `down` deletes them all. A create happens only when `boat limits` says the start
is free; otherwise the harness reports it and creates nothing. Each run pushes a member's deadline out to three hours
from then when less is left.

- Stay within the budget the maintainer gave (`--budget`, default 4) and the size they asked for (`--type`, default
  `large`).
- Name owned sandboxes with a prefix of your own (`--prefix`, default `farhelm-boat-<checkout>-`). Every member is
  recorded in `<scratch>/pool.json` when it is created. A sandbox the pool does not list belongs to someone else: never
  run anything on it, extend it or delete it, whatever its name.
- Delete the pool (`down`) when the work that needed it is over and nothing is expected to need it within about 25
  minutes (a `large` sandbox idling that long costs about one paid start), and always before the session ends. `down`
  prints any member that is still present; report those.
- Give concurrent runs the same `--scratch`: members are claimed through lock files there, so two runs never share a
  sandbox. A sandbox whose exec ended without boat's exit frame may still be running that command, so the harness
  deletes it instead of reusing it.

## Running

All subcommands take `--scratch <dir>` (outside the checkout, in your scratch directory) and the pool options above.

1. Rust: with the pinned nextest on `PATH` (`docs/test-run-evidence.md`) and an empty working copy of the revision under
   test, build the archive locally:
   `cargo nextest archive --locked --workspace --exclude farhelm-desktop --archive-file <file>` (or `-p` the packages
   you want). Then
   `scripts/boat/boat-tests.ts rust --scratch <dir> --shards <n> --archive <file> --out <dir> [-- <nextest filter arguments>]`.
   It packs the tracked source, a one-commit clone, the pinned nextest, `farhelm-fixtures` and this revision's static
   Podman payloads (`scripts/build-podman-test-payloads.sh`, which needs `musl-tools`, `file` and binutils locally; an
   intentional `FARHELM_TEST_SKIP_PODMAN=1` skips it and is forwarded to every shard, which you report as missing
   coverage). A rerun of the same archive on the same members (a narrower filter, say) passes `--reuse-stage` instead of
   `--archive` and uploads nothing.
2. GPUI: `scripts/boat/boat-tests.ts gui --scratch <dir> --out <dir> [--no-build] [smoke] [lifecycle]` builds what
   `scripts/gui-agent.py start` builds (with `gui-agent.py`'s own build step, unless `--no-build`), ships those four
   binaries with the source, and runs the chosen suites (both by default) one after the other through the recorder with
   `--no-build`. The paste step of the smoke is skipped on Linux, as it is locally.
3. Docs screenshots:
   `scripts/boat/boat-tests.ts docs --scratch <dir> --out <dir> [--no-build] [--only <page>]... [--members <n>]` ships
   the checkout's files (untracked ones git would not ignore included, for page modules being written) and the app
   binaries as `gui` does, runs `scripts/docs-screenshots.sh --local --no-build` for each named page, spread round-robin
   over as many free members as it can claim (up to `--members`, one per page by default, within the budget), or the
   whole capture on one member, and brings the PNGs back to `<out>/docs/shots/<page>/`. A run from shipped binaries is
   never publishable. `docs --build-there` is the publishable one: it ships the source with main recorded beside it and
   no binaries, and the member builds and captures every page in one run, which writes the capture record
   (`<out>/docs/shots/capture.json`) a publish needs. A cold build there takes most of the run. Captures run as a
   sandbox account of their own, `fhshots`, since the capture's leak check refuses any shot that draws the capturing
   account's name, and the sandbox's own account, `user`, is a word the app prints (the Add host dialog's `user@host`).
4. Read each `summary.txt` like a recorded run: the user manager's state, Podman and its image, self-ssh and tmux for a
   Rust shard; the binaries' presence and unresolved libraries for a GUI run; then the summary, failures and any runtime
   `SKIPPED` lines. A failed run's recorder directories come back as `runs.tar.gz`; keep them as evidence. Rust shards
   run through the recorder's generic mode, because its nextest mode does not take an archive; say so when reporting,
   with every run ID. That mode keeps no output of passing tests, so a shard keeps it in nextest's JUnit report instead
   and counts the runtime `SKIPPED` lines it finds there into `summary.txt`; a count of `UNKNOWN` means the shard is no
   evidence that nothing skipped.

What to expect, measured on 2026-11-17 from a cold local build of main: the workspace build took about 4.5 minutes, the
workspace archive about 10 more (1.75 GB), its upload to one sandbox about two minutes, and the whole workspace (7,909
tests) about nine and a half minutes on one `large` sandbox, with only the macro compile-contract test failing (see
below) and no runtime `SKIPPED`. The GPUI binaries took about 3.5 minutes to build; the GUI run took about seven minutes
in all, packing and a 500 MB upload included, of which the smoke's 49 steps took 160 seconds.

Other subcommands: `up --count <n>` creates and bootstraps members ahead of a run, `status` lists the pool with each
member's deadline and lock holder, `extend [--ttl <s>]` pushes every member's deadline out.

## How it works, and why

- The test binaries have absolute paths compiled in (fixture directories, `CARGO_BIN_EXE_*`, the target directory), so
  each sandbox unpacks the checkout at this checkout's own absolute path, and a shard extracts the archive over it with
  nothing remapped. The GUI run uses the same place, so `cargo metadata` there names the target directory the shipped
  binaries are in.
- Nextest's e2e setup script runs `cargo build -p farhelm-fixtures`. Shards run with a copy of `.config/nextest.toml`
  without that hook (the fixtures arrive prebuilt) but keep the Podman setup, which consumes the staged payloads and
  exposes `FARHELM_HELM_PAYLOAD_DIR` only to the matching test.
- Shards run with four nextest slots, as the recorder's nextest mode does locally. With eight (a `large` sandbox's CPU
  count) the real Podman test, which reserves all four slots of the e2e group, and 57 other e2e tests were never
  scheduled, and nextest ended with "58/1910 tests were not run" and no failure (2026-11-13).
- Boat's exec gives even the unprivileged `user` the ambient capability `CAP_DAC_READ_SEARCH` (observed 2026-11-13 and
  2026-11-17), which lets it read files whose mode forbids it, so a test that expects a permission refusal would fail.
  Both sandbox runners re-exec themselves under `setpriv` without inheritable or ambient capabilities first. If a
  permission-refusal test still fails on a boat and passes locally, the maintainer's call (2026-11-13) is to treat it as
  a boat environment failure, not a code finding; say so where the run is reported.
- The macro compile-contract test resolves crates offline from the local cargo cache and always fails on a sandbox; run
  it locally.
- A test that passes locally and fails on a boat is otherwise still a finding: the sandboxes are less loaded than the
  shared development machine, so races that the slow machine hides show up there.

## What the image gives you, and what bootstrap adds

Observed on fresh `large` sandboxes in November 2026; verify rather than assume, since boat changes the image without
notice. Ubuntu 24.04 with glibc 2.39 (the same as the development machines, so locally built debug binaries run
unchanged; if the development machine moves to a newer Ubuntu, this stops being true), systemd as PID 1 with logind and
a lingering user manager for `user` (uid 1000, passwordless `sudo`), an sshd on port 22, Python 3.12, Node 24, Rust, and
Mesa's software Vulkan driver. `scripts/boat/sandbox/bootstrap.sh` adds rootless Podman with the exact cached production
base the environment test requires, the distribution's tmux (the few Rust tests that start a real tmux server print
`SKIPPED` without it, which is missing coverage), Mutter for the GPUI runs, and passwordless self-ssh for the tests'
ssh-to-localhost second host. It takes about half a minute and runs on every claim, as a no-op once done.
