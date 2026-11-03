# Validation: choosing and running checks

The full "Finishing work" text from the root `AGENTS.md`, moved here with its references rewritten: how to choose
checks, the inventory of every CI-equivalent check and what it covers, reuse after a rebase, CI and the release
workflow, and browser end-to-end validation. Read it before choosing validation, running any runtime test, creating or
updating a PR, merging one, or reporting work as done (`AGENTS.md`, "Tests and validation"). The principles that apply
to every task are also in that root section, word for word; when you change one copy, change both.

## Finishing work

For changes to tests or their fixtures, apply `.agents/test-authoring.md` during authoring and supply its full text
verbatim to the independent reviewer. Use the review process selected for the task; this checklist does not request an
additional reviewer or a review swarm. Report concrete violations of the relevant contracts, not mechanical demands for
every bullet to appear in every test.

Run `python -B scripts/check-test-sleeps.py` with the isolated interpreter prepared in `docs/test-sleep-check.md` when
changing Rust or browser tests, their helpers, or the checker. The CI workflow's formatting job runs this source-only
check and requires zero unannotated delays. A `sleep-ok` reason explains a deliberate observation window, scheduling
stimulus, or polling interval; it does not replace a readiness oracle or excuse an unverified fixture premise.

Before creating, updating, or merging a PR, or claiming work is done, make a judgment call about what validation would
add useful evidence. Never mechanically run the entire test battery. A PR, rebase, stack completion, or merge request is
not by itself a reason to launch it. The list below is the authoritative inventory of available CI-equivalent checks and
their coverage, not a checklist to execute in full.

Start with the actual diff, the successful checks already available, and the concrete risks still uncovered. Choose the
smallest set of checks likely to expose a regression in the affected behavior; no additional runtime tests is a valid
choice when existing evidence still covers the change. Widen only when a specific material risk or unresolved failure
needs broader coverage. Cross-cutting, security, lifecycle, installer, or release changes deserve careful assessment,
not an automatic full run. Before choosing a full battery, explain what risk it covers that existing evidence and
targeted checks cannot adequately cover. Cost does not excuse ignoring a material risk, but running more tests without
identifying one is not diligence.

For a documentation-only change that cannot affect generated artifacts or executable examples — ordinary Markdown
documentation, a lore entry, agent instructions, or comments with no doctest or generation role — do NOT run Rust,
JavaScript, desktop, installer, provisioning, or browser tests. Run only formatters, linters, link checkers, or other
checks that actually inspect the changed files, if any exist. Do not run a check solely because it appears in the full
gate list.

When building a stack, use targeted checks at each review unit to catch likely regressions while they are cheap to
localize. At the tip, assess the combined diff for interactions those checks did not cover. Stack completion does not
require another battery or the highest-cost gates; run broader checks only for identified coverage gaps. Apply the same
judgment when landing the stack.

Reuse is per command. Compare the tested revision with the current head, including upstream changes and conflict
resolutions after a rebase. A new commit hash does not invalidate a successful run. If extensive validation just
finished and a minor rebase preserves the tested behavior, reuse it without more runtime tests. If the rebase changes
behavior, run targeted checks most likely to expose a problem in those changes or their interactions; widen only when
that leaves a material gap. Resolve uncertainty by inspecting the diff before defaulting to execution. Failed,
interrupted, poorly identified runs, or runs whose coverage no longer applies are not evidence of a pass. Report checks
run now; checks reused, with the covered revision and why their coverage still applies; and checks skipped, with the
reason.

For Rust execution, first put the pinned nextest and tmux on PATH using the guarded setup in
`docs/test-run-evidence.md`. Run from the checkout root with Python 3.11+ (Python 3.13+ on macOS). Use owned sandboxes
for expensive, slow or memory-heavy checks; small focused checks can run locally. The recorder's nextest mode enforces
four global slots, zero retries and failure on empty selection, and retains each run's configuration and JUnit report.
Runtime `SKIPPED` messages still require reading the retained output: an early-return test is a JUnit pass, not proof
that its required systemd or SSH substrate ran.

- The Linux `containerized_sessions` e2e tests require rootless Podman and the offline image
  `docker.io/library/ubuntu:24.04`. They never pull images and fail if either prerequisite is missing. An intentional
  run without this coverage sets `FARHELM_TEST_SKIP_PODMAN=1` and passes `--keep-farhelm-env FARHELM_TEST_SKIP_PODMAN`
  to the recorder. The explicit SKIPPED output is not isolation evidence. A focused selection is
  `cargo nextest run -p farhelm --test e2e -E 'test(containerized_sessions::)'`; creator-fence, timeout and
  token-privacy unit tests are selected by
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(container::tests::)'`.
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo clippy -p farhelm --bins -- -D warnings` — lints the shipped binary's configuration, in which the supervisor's
  `test-seams` feature is off; the `--all-targets` run above always has it on, because the e2e tests enable it through a
  dev-dependency.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'workspace Rust targets' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run --workspace --exclude farhelm-desktop`
  — executes applicable unit and integration targets, including the support crates omitted from default-members. Nextest
  starts a process per test and bounds concurrency across binaries; the e2e group has four slots, supervisor tmux
  modules share two, the two-host fixture reserves two, and RSS reserves all four. The release gate retains its narrower
  package/target selections; ordinary CI does not run this suite. Do not also run the old full libtest battery.
- `python3 scripts/record-test-run.py --kind development --selection 'workspace doctests' --concurrency '4 doctest threads' --tmux required -- cargo test --locked --doc --workspace --exclude farhelm-desktop -- --show-output --test-threads=4`
  — preserves doctest coverage separately because nextest does not execute it.
- `scripts/test-tmux-pinned-shutdown.sh` — the x86_64 Linux release gate builds the exact checksummed release named by
  `.github/release/source-pins.env` on cache miss, then runs every focused output-client teardown regression in its own
  test process with its own recorded nextest report. Keep this explicit release subset while the full e2e target remains
  excluded there. A successful local workspace nextest run already covers these tests on the same pinned substrate; do
  not duplicate them merely because this script is listed. A distro tmux loses the affected-version coverage.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'terminal host' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p terminal-host`
  — the terminal host crate (`crates/terminal-host/`, contract in its `SPEC.md`): real-pty tests of the built
  `terminal-host` binary (lifecycle, input, resizes, busy and takeover, shutdown, SIGTERM and SIGKILL, the exit while a
  descendant holds the pty, stuck input on the client's own connection, malformed peers, snapshots and trimming, the
  child's signals, environment and descriptors, the host's own signal mask, descriptors and command line, terminal
  settings byte-equal to a tmux 3.7c pane's when that tmux is first on `PATH`, else said as `SKIPPED`), in-process tests
  of its event loop, golden fixtures of every version-1 frame (`tests/golden/v1/`), and property tests of the codec and
  of the journal against a model. Seconds. Part of the workspace nextest run; the focused macOS CI job runs it too, in a
  step of its own after the uninstall acceptance. Nextest runs each test in a process of its own, which these fixtures
  need: under libtest (`cargo test`, and the coverage and mutation runs below) run them with one test thread
  (`RUST_TEST_THREADS=1` or `-- --test-threads=1`). Plain parallel `cargo test` is not supported for this crate: a child
  one test forks holds every descriptor of the test process until it execs, and on macOS, which cannot open a pty or
  socket close-on-exec in one call, it can keep another test's for good; that test then waits for an end of output or a
  close that never comes (`crates/terminal-host/TESTING.md`).
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'terminal host spawning and cleanup' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-supervisor -p farhelm-teststate -p farhelm --lib --test terminal_host_spawn -E 'test(/^hosts::/) | binary(terminal_host_spawn)'`
  — the supervisor's side of terminal hosts and the harnesses' cleanup of them. The supervisor's `hosts::` unit tests
  cover the private socket directory, the socket path bound against macOS's 104-byte limit, the client tag's golden
  fixtures (`crates/farhelm-supervisor/tests/fixtures/host-tag/`, which must never change for a shipped version) and
  unreadable tags, and the spawn's start protocol against stand-in first stages (readiness from the exit status alone, a
  relative state directory, and a start that does not finish ending whatever answers at its socket or naming it);
  `farhelm-teststate`'s `hosts::` tests cover its blocking shutdown client against stand-in listeners (refusals believed
  only over its stale window, an incomplete scan kept). `crates/farhelm/tests/terminal_host_spawn.rs` starts real hosts
  from the built `farhelm` (never inside a test process, since a host forks): readiness and no zombie, a failed start
  leaving nothing, no `FARHELM_*` marker or credential on the host while its child has them, and the test-state cleanup
  for a normal run, a dead run, nested `remote/` state, and a stopped host that keeps its run (a sweep, a `TestDir` drop
  and `close`) for a retry, plus, on Linux only, a stopped host with a full backlog. Under a minute, part of the
  workspace nextest run; the focused macOS CI job runs it too, in the terminal host step. Portable to macOS except where
  it reads `/proc`.
- Terminal host coverage, when the crate changes:
  `cargo +nightly llvm-cov -p terminal-host --branch --summary-only -- --test-threads=1`, with cargo-llvm-cov 0.9.1
  installed in your scratch directory (`cargo install --locked --root <scratch>/tools cargo-llvm-cov --version 0.9.1`,
  then that `bin` on `PATH`) and `rustup component add llvm-tools-preview --toolchain nightly`. The bar is 95% of lines;
  branches are reported, and the uncovered ones are system-call failure paths the maintainer accepted leaving uncovered
  rather than adding fault injection (2026-10-31). Code between a fork and its exec is excluded with `coverage(off)`.
  `crates/terminal-host/TESTING.md` has the current numbers and what is uncovered.
- Terminal host mutation testing, when the crate changes:
  `RUST_TEST_THREADS=1 cargo mutants -p terminal-host --timeout 120 --jobs 2` with cargo-mutants 27.1.0 installed the
  same way (about two hours per 300 mutants with two jobs, since most of the event loop's mutants hang a test until the
  timeout, and the crate has about 650; restrict a rerun to the functions a change touched with `--file` and `--re`),
  Linux only, run inside its own CPU-capped `systemd-run --user --scope` as `crates/terminal-host/TESTING.md`, "Running
  it without disturbing a shared machine", shows: loop-breaking mutants spin every test thread, and the hosts their
  tests start outlive the killed tests. Every surviving mutant is killed by a test or listed with its reason in that
  file.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'terminal emulator' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-terminal-emulator`
  — the supervisor's terminal emulator (`crates/farhelm-terminal-emulator/`), run in-process with no pty, tmux or
  network: the query-answer table against tmux 3.7c's recorded answers, side effects by feed mode (live, replay of seen
  and of unseen output), the synchronized-output deadline on a hand-stepped clock (and its flush before a resize), the
  snapshot split candidate on agent-shaped reads, a property test that whole, byte-at-a-time, chunked and
  candidate-split feeding end in the same state once any open update is flushed, the handler's forwarding as a property
  test against a bare `alacritty_terminal`, screen text and titles shaped like tmux's capture (tabs, modes 47 and 1047),
  the mode accessors (mouse tracking resets as in tmux), the browser theme's colors, one-column terminals with wide
  characters and resizes (each bounded to 10 seconds, so that a regression of the vendored reflow fix fails after about
  1 GB of allocation instead of running until memory runs out), a screen full of tabs captured in linear time, and
  hostile output through the vendored caps (combining floods, 8 MB OSC 52 and titles, long hyperlinks, a synchronized
  update past vte's 2 MiB buffer). Seconds. Part of the workspace nextest run. Run it when the crate or anything under
  `vendor/` changes.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'emulator parity with tmux' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-supervisor --lib -E 'test(emulator_parity::)'`
  — the terminal emulator against the pinned tmux 3.7c, run with Farhelm's server configuration in a private server per
  test (`crates/farhelm-supervisor/src/emulator_parity/`): screen text, cursor, title and query answers compared at
  every cut of hand-picked and seeded random output (wide, combining and emoji characters, wraps, trailing blanks,
  colors, the alternate screen, scroll regions, origin and insert modes, erases, saved cursors, titles, synchronized
  updates, resizes), with every update closed and the uncompared cuts inside updates bounded, the harness's own
  comparator and dead-pane checks, the frozen agent screens repainted with their status readers run on the emulator's
  text, recorded agents' startup probes, and the emulator's whole answer table, with the known screen and answer
  differences pinned on both sides. Without that tmux first on `PATH` each test says `SKIPPED`. Under a minute on a
  quiet machine, a few on a loaded one (the generated streams). Part of the workspace nextest run, in the supervisor's
  two-slot tmux group; run it when the emulator crate, `vendor/`, the supervisor's tmux configuration or a status reader
  changes. Its one `#[ignore]`d test replays real agent sessions, which stay outside the repository because they hold
  conversation text: set `FARHELM_TEST_AGENT_RECORDINGS` to `<harness>=<directory>` entries separated by `:` (each
  directory holding the session's `raw.bin` and `events.jsonl`, `recordings.rs` gives the format) and run the same
  command with `--keep-farhelm-env FARHELM_TEST_AGENT_RECORDINGS` before the `--` and `--run-ignored only` after
  `--lib`, selecting `test(emulator_parity::recordings::)`.
- `python3 scripts/record-test-run.py --kind development --selection 'vendored vte' --concurrency 'libtest default threads' --tmux none -- cargo test --manifest-path vendor/vte/Cargo.toml --features ansi`,
  `python3 scripts/record-test-run.py --kind development --selection 'vendored alacritty_terminal' --concurrency 'libtest default threads' --tmux none -- cargo test --locked --lib --manifest-path vendor/alacritty_terminal/Cargo.toml`
  and
  `python3 scripts/record-test-run.py --kind development --selection 'vendored crates paired' --concurrency 'libtest default threads' --tmux none -- cargo test --locked --manifest-path vendor/pair-tests/Cargo.toml`
  — the vendored, patched terminal crates (`vendor/README.md`): upstream's unit tests plus the tests of Farhelm's
  patches (the parser's ground state, the OSC payload and its overflow notice, zero-width, hyperlink and title caps,
  hyperlink reuse, the history counters, XTVERSION, the erase and cursor movement fixes), and in `vendor/pair-tests/`
  the tests that need both patched crates together (an OSC 8 the parser drops ending the terminal's hyperlink). These
  are excluded from the workspace, so no workspace test run, clippy, `cargo fmt` or dprint (beyond `vendor/README.md`
  and `vendor/pair-tests/Cargo.toml`) reaches them, and CI does not run them. Run all three when anything under
  `vendor/` changes; seconds, plus a first download of the crates' own locked dependencies.
- `cd crates/farhelm-ui/js-tests && node --test` — the JS unit harness for the asset-JS layer's pure functions
  (PLAN_M6_5.md item 1); node is already a CI requirement for Playwright below, so this adds no dependency. Run from
  inside the directory rather than as a glob from the repo root: node's no-argument default discovery (every `*.test.js`
  in cwd) is the oldest, most version-portable form the test runner has, whereas quoted-glob CLI arguments are newer and
  CI pins no node version.
- `cd crates/farhelm-supervisor/asset-js-tests && node --test` — the OMP conversation-reporter asset's own execution
  proof: dispatch ordering against a controlled subprocess mock (including a deliberately nonserialized variant the
  proof must reject), stale-id cancellation, execution-time file recheck, the subscribed transition events, and the
  silent-failure boundary, run against the real shipped asset. Same node terms as the UI harness above; the release gate
  runs it through the recorder with the `OMP reporter asset scenarios` selection.
- `cd website/feedback-tests && node --test` — the feedback endpoint's handler (`website/feedback/handler.js`, deployed
  as `website/api/feedback.js`): method, configuration and size refusals, field caps counted in code points, fences that
  user text cannot close, and that no response or log line carries the token, the inbox's name, the message or the
  contact. Configuration and the GitHub client are injected; nothing touches the network or `process.env`. Run it when
  the handler or its tests change; CI's website job runs it beside the website build.
- `cargo check -p farhelm-ui --features desktop` — the desktop renderer compiles nowhere else; needs the webkit2gtk/gtk
  dev packages (see the CI job for the apt list).
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'desktop Rust targets' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-ui --features desktop`
  — exercises desktop-only persistence and IPC seams with the same system dependencies as the compile check.
- `python3 scripts/record-test-run.py --kind development --selection 'desktop doctests' --concurrency '4 doctest threads' --tmux none -- cargo test --locked --doc -p farhelm-ui --features desktop -- --show-output --test-threads=4`
  — preserves the desktop feature's separate doctest coverage. Desktop execution remains release-only in hosted CI.
- `cargo check -p farhelm-desktop` — the shipped desktop binary sits outside `default-members` (so ordinary builds never
  compile WebKit), which means `-p` is the only thing that ever compiles it.
- `scripts/check-desktop-assets.sh` — holds the desktop build's `asset!()` set and the web bundle's files to the same
  set, in both directions. The shipped desktop-artifact builder calls `--compare` on its actual embedded bundle, so
  release validation preserves parity without a duplicate CI rebuild. `--self-test` also proves head CSS/classic JS
  remain product assets while only the body renderer module and WASM are excluded.
- `PATH="$(scripts/build-pinned-tmux-ci.sh):$PATH" scripts/desktop-smoke.sh` — the x86_64 Linux release gate runs this
  with an isolated `CARGO_TARGET_DIR`; before any of it needs Xvfb, four pre-display process legs drive
  `farhelm-desktop`'s own entry point directly: a missing tmux and a below-floor tmux each refuse with one exact plain
  stderr line and exit status 1 (the app's own preflight, run before it would spawn its managed supervisor), and a
  non-tmux bootstrap failure (an unusable state directory) exits 1 through the ordinary fallback path with no panic. The
  non-pixel Xvfb integration gate that follows covers the embedded helm, managed supervisor, desktop authentication, the
  tmux override reaching the managed supervisor, the supervisor outliving a hard exit and an ordinary quit (reused, then
  replaced by a launch whose program file changed, with a session surviving the replacement), a second launch with
  another program refusing beside the open app without stopping its supervisor, a replacement that would fail its tmux
  preflight stopping nothing, a frozen (SIGSTOP) supervisor being replaced with a kill after the grace, restart
  persistence, and that every asset the window requested went through the desktop asset handler. API checks retain
  host-qualified session references; tmux and scope checks use their local session IDs. The optional coordinate-driven
  leg is not part of CI.
- `DESKTOP_SMOKE_APPEARANCE=1 scripts/desktop-smoke.sh` — optional Linux native appearance integration using the
  ordinary owned Xvfb/openbox setup. Four fresh/stored dark/light cases check native media-query premises, palette
  before renderer content, real Settings choices, private WebKit profile persistence, Inter, embedded assets and the
  same live terminal attachment's input and ordered output. Retains bounded JSON and one final client screenshot per
  case. DOM assertions do not prove transient paint correctness; normal appshots provide visual review. No live native
  system-appearance event, physical keyboard dispatch, throughput or macOS claim. Run through the recorder; the script
  supplies its own private profile and closed scenario inputs.
- `DESKTOP_SMOKE_APPEARANCE=no-initialize scripts/desktop-smoke.sh` — one optional Linux missing-initialize leg using
  the same builds and owned display. A fixed valid rootless index exercises the creation-time startup deadline before
  App renders. It requires actual hidden native creation, natural refusal with exit 1, the owned supervisor's birth, its
  detached setup (own session, `/dev/null` stdin, its log file as stderr) and the app's record of it, and that it keeps
  running after the refusal; teardown stops it by that record. Private profiles and bounded process/window observations
  are retained. X11 polling supplies sampled no-observed-map evidence; an absent hidden XID is recorded as unknown
  realization, never manufactured by showing the window. Requires Linux pidfd support in the existing Python runner.
- `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh` — the curl-installer is POSIX
  `sh` run unread on a fresh machine (`curl | sh`), so a syntax slip or a shellcheck-catchable bug in it gets caught
  before the slower Rust-side asset-table parity test in `assets.rs` would notice. `test-install-sh.sh` is bash, not
  POSIX `sh` (it is never piped into a stranger's shell, so nothing forces the same portability constraint), which is
  why only `install.sh` goes through `sh -n`.
- `cargo build -p farhelm && bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` — drives the macOS-only
  installer as a real child against a fixture HTTP server, using a Darwin/arm64 `uname` shim on Linux and BusyBox. The
  installer hands its locked work to the release's own `farhelm`, so every fake release's `farhelm` is a wrapper that
  runs this tree's real binary (required; CI builds it static with musl so the Alpine/BusyBox run can use it too).
  Covers fresh install, updates side by side and interrupted updates finished by a Terminal run or an older app's
  updater (the write order stopped after each step is covered by the Rust module tests of
  `farhelm_supervisor::installation::install`), the forwarder, 404, checksums, malformed archives, version
  normalization, missing prerequisites, exact closing messages and tmux advice, terminal styling/progress and plain
  redirected output, app layout and ownership/recovery. Linux refuses before downloading or writing; old releases
  (without app resources, or from before the side-by-side layout) refuse before anything changes. Nothing outside
  `~/.local/bin` and `~/Applications/Farhelm.app` changes, and no service manager runs. Every invocation uses `env -i`
  with an explicit environment, never the harness process's own. Refused `/latest` cases attribute requests by their
  scenario's origin path and report unexpected request lines, so delayed requests from another fixture cannot silently
  become this install's request count.
- `python3 scripts/record-test-run.py --kind development --selection 'installed uninstall acceptance' --concurrency 'one fixture at a time' --tmux none -- python3 scripts/test-uninstall.py --binary target/debug/farhelm --installer scripts/install.sh`
  — macOS-only, after building the CLI: runs the actual installer and installed uninstaller against private fixture
  homes and release archives. Covers updates, confirmation, dry-run preservation, foreign files and native bundles.
  Other platforms report a skip; Linux no longer has end-to-end coverage of installer-made installations because the
  installer refuses Linux. The focused macOS CI job and release gate also run
  `cargo nextest run -p farhelm --bin farhelm -E 'test(uninstall::)'`, the version-lock, installation-lock and session
  keeper tests
  (`cargo nextest run -p farhelm-supervisor --lib -E 'test(/installation::|version_lock::|session_hold::/)'` and
  `cargo nextest run -p farhelm --test session_hold --test version_lock_forwarder`), each through the recorder with four
  slots and `--tmux none`. The focused macOS CI job (not the release gate) also runs the desktop app's supervisor record
  and stop tests the same way: `cargo nextest run -p farhelm-supervisor --lib -E 'test(managed_supervisor::)'` (the
  uninstall stop's own tests are part of `uninstall::` above). After the uninstall acceptance, in a step of its own that
  runs whatever the steps before it did (so that a host failure cannot hide that gate), it runs the terminal host's
  whole test suite, `cargo nextest run -p terminal-host`, the host's entry test in the CLI,
  `cargo nextest run -p farhelm --test terminal_host_entry`, and the supervisor's spawning and the test-state cleanup
  (the `terminal host spawning and cleanup` selection above).
- `scripts/test-provision-centos.sh` — the x86_64 Linux release gate boots a systemd CentOS Stream 9 container and makes
  the helm provision it over ssh, which is the only coverage of a helm installing onto a distribution other than its
  own. Needs docker and `musl-tools`: the payloads it pushes are the release's musl-static `farhelm` and static tmux,
  because the workspace's glibc debug binary cannot exec on CentOS 9. A few minutes, most of it the musl build; the
  container image and the tmux build are both cached after the first run.
- `dprint check`
- `python3 scripts/check-agents-md.py` and `python3 scripts/check-agents-md.py --self-test` — keeps the agent
  instructions loadable: the root `AGENTS.md` within the 28 KiB budget (Codex injects only its first 32 KiB and drops
  the rest silently) and ending with its truncation sentinel, every path an instruction file names and every
  `` `file`, "Heading" `` citation resolvable (including mixed-case specification filenames), every `.agents/` file
  reachable from the root, and no inventoried `CLAUDE.md` or `CLAUDE.local.md` (either disables default native
  `AGENTS.md` loading). Run it whenever an instruction file changes or a Markdown heading is renamed or a file moved;
  the self-test when the script changes. Seconds, no network. CI runs it in the dprint job, which, unlike the formatting
  job, also runs for docs-only changes.
- `cd website && bun install --frozen-lockfile && bun run build` — the docs website (Astro + Starlight) must build; this
  is what catches a page with missing frontmatter, a sidebar slug that names no page, or a broken internal link or
  anchor. Bun is the package manager the Vercel project is configured with, so the lockfile is `bun.lock`; commit it
  with any dependency change. Vercel itself never builds on a push; see `.agents/vercel.md`. The build itself runs under
  Node (the `astro` binary's shebang), and Astro refuses Node older than 22.12; the CI job pins Node for that reason.
- `dist generate --check` — `release.yml` is generated from `dist-workspace.toml` plus `.github/dist-build-setup.yml`,
  and the release `plan` job refuses a stale one; this asks the same question before a tag has to. Needs the pinned
  cargo-dist (`cargo install --locked cargo-dist --version 0.32.0`, the version `dist-workspace.toml` names).
- `scripts/publish-docs-shots.sh --self-test` — the docs screenshot publisher's whole validation: against a bare
  repository standing in for GitHub, it checks that refused publishes push nothing, that only `refs/docs-assets/keep` is
  ever created (never a branch), that a snapshot holds exactly the shots the docs pages reference, that partial,
  incomplete, or since-modified captures are refused, that the manifest records the capture's main commit, and that a
  snapshot stays until six weeks after it was replaced (the one main pins, always). Run it, with `shellcheck`, when the
  script changes; seconds, no network, needs ImageMagick's `convert`.
- `bash scripts/test-plans-watch.sh` — drives the plans monitor's watcher (`scripts/plans-watch.sh`) against a strict
  stub `gh` that runs the watcher's own `jq` filter over GitHub-shaped tree listings: change detection, baselines read
  from a commit, the wake check that keeps claims from waking idle executors, the idle cap, retried and persistent
  failures, malformed and truncated responses, hung requests, argument errors, and prompt exit on SIGTERM without
  leaving children behind. Run it, with `shellcheck` on both scripts, when either changes; under a minute, no network,
  needs `jq`.
- `python3 scripts/test-plans-queue.py` — the plans queue script (`scripts/plans-queue.py`), which is the lock that
  keeps concurrent plan executors off each other's plans: its line grammar, every state transition and refusal, the
  invariants between a state and its files, the rules a planning PR must keep, the wake check, and the compare-and-swap
  loop against an in-memory GitHub that injects racing writers and lost responses. One case runs the real `dprint` when
  it is on PATH, which downloads dprint's plugins if its cache is cold. Run it when the script changes; seconds, no
  other network.
- `python3 scripts/test-todo-picker.py` — TODO extraction and the legacy standalone checklist
  (`scripts/todo-picker.py`): how TODO.md buckets and entries are read, that entries with a plan are left out, that a
  summaries file written against an older TODO.md is refused, and that summaries are escaped into the page. Run it when
  the script changes; seconds, no network.
- `deno test -A scripts/serve-page.ts` — the legacy standalone static server (agent pages use the `interactive-artifact`
  skill): that it answers with `Cache-Control: no-store` and moves past a taken port. Run it, with `deno check` on the
  same file, when the script changes; seconds. Needs Deno, and fetches `jsr:@std/http` the first time.
- `deno test -A scripts/tensorlake/sandbox-tests.ts` and
  `shellcheck scripts/tensorlake/bootstrap.sh scripts/tensorlake/run-shard.sh` — the Tensorlake browser-test harness's
  own checks: how projects are balanced into shards and how the snapshot listing is read. Run them, with `deno check`,
  when the harness changes; seconds, no sandbox. Using the harness itself is described in `.agents/tensorlake.md`.
- `python3 scripts/record-test-run.py --kind development --selection 'portable brick-test harness' --concurrency 'one unittest process' --tmux none -- python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`
  — validates owned Tart lifecycle, signed-input planning, guest execution, fixed command operations, durable handoffs,
  compiled checkpoint contracts, structured native actions, restart process handoffs, calibrated window measurements,
  gesture-trace and bonus gesture-readiness gates, failure gates and evidence import with fictional fixtures. Use a
  substrate that permits process-group inspection; Linux continuity cases skip on macOS. These tests prove harness
  contracts, not native GUI behavior or a release pass. Actual native fixture probes and release checks retain their own
  private evidence. The guest-only Swift geometry helper needs separate SDK type-check and read-only runtime validation
  in an owned disposable guest; the portable suite does not exercise that OS API.
- `python3 releasing/check-changelog.py format` and `python3 releasing/check-changelog.py --self-test` — the changelog
  layout lint the release gate runs on every tag, and the checker's own fixtures (format variants, a synthetic git
  history for the fragment sweep, synthetic dist manifests for the announcement comparison). Run the lint when
  `CHANGELOG.md` or a fragment changes, the self-test when the checker does.
- `python3 releasing/finalize-release-notes.py --self-test` — the body swap that replaces a stable release's draft notes
  on its GitHub release with the approved text, against bodies shaped like dist's, and the refusals that need no
  repository or release. Run it when that script or the checker's parser changes; seconds, no network.
- `python3 releasing/notes-review-page.py --self-test` — the legacy standalone notes page (stable releases use the
  `interactive-artifact` skill): entry ids, attribute escaping, HTTP(S)/relative-only link targets and PR links; under
  node, it checks that the copied feedback block carries each note and a regenerated page drops notes typed on its
  previous revision. Without node it says `SKIPPED` for the page script part. Run it when that script changes; seconds,
  no network.

This inventory names the relevant local checks. The CI workflow (`ci.yml`) covers formatter, Clippy, desktop
compilation, the JS harness, the website build and its feedback endpoint tests, installer validation, generated-workflow
validation, and the agent-instructions check; costly Rust, pinned-tmux, desktop runtime, and CentOS gates run in the
x86_64 Linux release artifact job. If either workflow changes, update this list in the same change.

The CI workflow runs ONLY on demand: it has no push or pull-request trigger (removed 2026-09-12), so neither a PR, a
`gh pr ready`, nor a merge to main starts a run. The release build gate is the validation that decides whether a build
ships; the local checks above are what validates a change before it lands. The focused headless macOS uninstall suite
runs with `gh workflow run ci.yml --ref <branch> -f suite=uninstall-macos`. `gh workflow run ci.yml --ref <branch>` runs
the whole baseline for any ref when a hosted verdict is wanted. PRs are still opened as drafts and marked ready when the
user asks to publish them, or as part of landing them, never on the agent's own initiative.

The RELEASE workflow is not on that list: it is generated, runs on tag pushes only, and carries its own gate, which is
not something to run locally. `releasing/.agents/workflow.md` describes it, the parts of it that CAN be checked locally
(the changelog checks above among them), and the unsigned-release recovery procedure.

Browser end-to-end validation follows the same judgment and reuse rules, including at merge time. Its CI job is disabled
(`if: false` in ci.yml), so CI green does not include browser coverage. Assess whether the change leaves a concrete
browser integration risk untested; choose relevant specs when they cover it, and run the full suite only when the risk
needs that breadth. Do not start the full suite merely because a PR is about to merge. Documentation-only changes need
no browser tests, and a minor rebase does not discard existing browser evidence.

When browser validation is warranted, run the selected specs through the recorder on Chromium and WebKit (WebKit stands
in for the desktop app's engine family). Every run, full or selected, boots three stacks and spreads its (engine, spec
file) projects over three workers, one stack each (`e2e/stack-slots.ts`), so a selection of a few spec files already
runs in parallel. The full-suite child command is `cd e2e && npx playwright test`. It needs `cargo build` and
`cd crates/farhelm-ui && dx build --package farhelm-ui --platform web --release` first, because it drives the built web
UI against a real helm and supervisor, plus a one-time
`cd e2e && npm install && npx playwright install chromium webkit`. Reuse matching builds and successful test results
when the intervening diff leaves their coverage intact; see `docs/test-run-evidence.md` for recorded selections.

When the maintainer asks for browser specs to run on Tensorlake sandboxes, read `.agents/tensorlake.md` in full first.
It splits a selection across several cloud machines at once; it is never the default.
