# Validation: choosing and running checks

The full "Finishing work" text from the root `AGENTS.md`, moved here with its references rewritten: how to choose
checks, the inventory of every CI-equivalent check and what it covers, reuse after a rebase, CI and the release
workflow. Read it before choosing validation, running any runtime test, creating or updating a PR, merging one, or
reporting work as done (`AGENTS.md`, "Tests and validation"). The principles that apply to every task are also in that
root section, word for word; when you change one copy, change both.

## Finishing work

For changes to tests or their fixtures, apply `.agents/test-authoring.md` during authoring and supply its full text
verbatim to the independent reviewer. Use the review process selected for the task; this checklist does not request an
additional reviewer or a review swarm. Report concrete violations of the relevant contracts, not mechanical demands for
every bullet to appear in every test.

Run `python -B scripts/check-test-sleeps.py` with the isolated interpreter prepared in `docs/test-sleep-check.md` when
changing Rust tests, their helpers, or the checker. The CI workflow's formatting job runs this source-only check and
requires zero unannotated delays. A `sleep-ok` reason explains a deliberate observation window, scheduling stimulus, or
polling interval; it does not replace a readiness oracle or excuse an unverified fixture premise.

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

Rustup reads the exact compiler version, rustfmt, clippy and wasm target from `rust-toolchain.toml` for local, CI and
release builds. Bump that file's version in a deliberate PR. Purpose-specific targets are added to that toolchain by
their scripts; explicit `cargo +nightly` and `rustup ... --toolchain nightly` still select nightly for coverage.

For Rust execution, first put the pinned nextest on PATH using the guarded setup in `docs/test-run-evidence.md`. Run
from the checkout root with Python 3.11+ (Python 3.13+ on macOS). Use owned sandboxes for expensive, slow or
memory-heavy checks; small focused checks can run locally. The recorder's nextest mode enforces four global slots, zero
retries and failure on empty selection, and retains each run's configuration and JUnit report. Runtime `SKIPPED`
messages still require reading the retained output: an early-return test is a JUnit pass, not proof that its required
systemd or SSH substrate ran.

- The Linux `containerized_sessions` e2e module covers decoding historical stored choices, refusing creation and
  terminal/lifecycle/file access, and Delete. Its fixtures use stored rows without creating a container or base image.
  Delete uses installed Podman to confirm absence, or succeeds when Podman is missing. Select it with
  `cargo nextest run -p farhelm --test e2e -E 'test(containerized_sessions::)'`. Minimal cleanup/revocation unit tests,
  including missing-engine and unconfirmed-absence cases, use
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(container::tests::)'`.
- The real Linux `podman_environments` e2e module uses a throwaway helm, the production provider and the ordinary
  in-container supervisor/installer. One recipe build covers a public GitHub managed checkout, endpoint credential
  delivery, ordinary grid/tab/file/upload/download protocols, a nonce-proven loopback HTTP relay, pause/Resume process
  survival, Stop and conversation Restart with a stand-in, helm reconnection, external-stop automatic start once, the
  user's environment Delete (unasked while paused, one that keeps the container for a session an agent inside created
  and reports the count, and the last session's, which seals and destroys the container) and nonforcing cleanup of an
  image held by another owned container. The explicitly ignored
  `real_ssh_environment_managed_checkout_and_ordinary_features` variant runs the same scenario through a real localhost
  SSH place on an owned, preconfigured machine. Select it with `--run-ignored only` and its exact test name when SSH
  acceptance is requested. It fails if strict passwordless localhost SSH is unavailable. This protocol coverage leaves
  native desktop opening and browser file-link gestures to their own checks. It requires the production rootless Podman
  probe to pass (Linux, at least 4.9.3, cgroup v2, systemd cgroup manager and usable user scopes), a working engine
  event backend, and the cached managed base
  `docker.io/library/ubuntu@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852`. The fixture
  requires that image to be present before it starts. Its production recipe build uses the network for apt and both
  official agent installers and clones the public repository from GitHub. Agent turns use a stand-in. Select it with
  `cargo nextest run -p farhelm --test e2e -E 'test(/^podman_environments::/)'`. Nextest reserves all four slots and
  allows 40 minutes for its 30-minute scenario, five-minute cleanup and teardown. The matching setup runs
  `bash scripts/build-podman-test-payloads.sh` once per selected suite. By default it adds the native musl Rust target,
  builds this source's CLI, checks that it is a static ELF and stages the ordinary release-shaped `farhelm` archive
  under `target/podman-test-payloads/`. Build tools include `musl-tools`, `file` and binutils. Only selections
  containing this module trigger that payload build. `FARHELM_HELM_PAYLOAD_DIR` can supply an absolute directory of
  matching staged payloads. Retain it intentionally with `--keep-farhelm-env FARHELM_HELM_PAYLOAD_DIR`. Automatic setup
  exposes that variable only to matching tests through nextest's setup environment. Archive workers receive prebuilt
  matching payloads from the Rust packer and retain its private `FARHELM_TEST_PODMAN_PAYLOAD_DIR` through the recorder.
  Cargo is unavailable on those workers. An intentional opt-out sets `FARHELM_TEST_SKIP_PODMAN=1` and passes
  `--keep-farhelm-env FARHELM_TEST_SKIP_PODMAN` to the recorder. Setup then skips the payload build and the test prints
  SKIPPED. Report that as missing real-environment coverage and exclude it from runtime pass evidence.
- `bash -n scripts/build-podman-test-payloads.sh` and `shellcheck scripts/build-podman-test-payloads.sh` inspect the
  focused payload producer. Its real artifact/install contract is exercised by the Podman e2e selection above.
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings` — the default members include the GPUI desktop GUI (`farhelm-gui`), so
  this lints it under the GUI bans in `crates/gui/clippy.toml` and needs the Linux packages named under the workspace
  nextest run below.
- `cargo clippy -p farhelm --bins -- -D warnings` — lints the shipped binary's configuration, in which the supervisor's
  `test-seams` feature is off; the `--all-targets` run above always has it on, because the e2e tests enable it through a
  dev-dependency.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'workspace Rust targets' --concurrency '4 nextest slots; retries 0' -- cargo nextest run --workspace --exclude farhelm-desktop`
  — executes applicable unit and integration targets, including the support crates omitted from default-members. Nextest
  starts a process per test and bounds concurrency across binaries; the e2e group has four slots, the two-host fixture
  reserves two, and the real Podman fixture reserves all four. The release gate runs this same selection, including the
  farhelm integration targets, after preparing rootless Podman, the runner's systemd user manager and bus environment, a
  verified transient user scope, and its offline Ubuntu image. It narrows every later Rust step with a filter rather
  than `-p`, so those steps share one build; ordinary CI does not run this suite. Do not also run the old full libtest
  battery. Every terminal a test's supervisor creates is a terminal host: the e2e suite's in-process harness and its
  real supervisor process fixture start hosts from the built `farhelm`, and the supervisor's own unit tests that need a
  real terminal run in terminal hosts their test binary starts from itself (`hosts::test_program`, glibc Linux only). A
  few tests still start a real tmux server: the tests of ending old terminals at startup, which build an earlier
  release's server by hand, the test-state cleanup's tests of a server a killed test left behind, and the test of a
  user's own tmux (stock `/usr/bin/tmux`) under a session's scope. They use the machine's tmux and print `SKIPPED`
  without one, which is missing coverage, not a pass; the release gate installs the distribution's tmux for them. On
  Linux the workspace run needs Python 3 at `/usr/bin/python3`: besides the e2e stand-ins named below, the supervisor's
  `managed_supervisor::` stop test runs a Python stand-in whose main thread exits before its other thread. `--workspace`
  also builds the GPUI desktop GUI (`farhelm-gui`), so on Linux this run and the doctest run below need GPUI's build
  dependencies: `pkg-config`, the fontconfig development package (`libfontconfig-dev`; fontconfig is found through
  pkg-config at build time) and the xkbcommon one (`libxkbcommon-dev`, linked directly). FreeType is used from the
  system when pkg-config finds version 24.3.18 or newer and is otherwise built from bundled sources with the C++
  compiler. Wayland and Vulkan need no development package: they are loaded at run time.
- `python3 scripts/record-test-run.py --kind development --selection 'workspace doctests' --concurrency '4 doctest threads' -- cargo test --locked --doc --workspace --exclude farhelm-desktop -- --show-output --test-threads=4`
  — preserves doctest coverage separately because nextest does not execute it.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'terminal host' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p terminal-host`
  — the terminal host crate (`crates/terminal-host/`, contract in its `SPEC.md`): real-pty tests of the built
  `terminal-host` binary (lifecycle, input, resizes, busy and takeover, shutdown, SIGTERM and SIGKILL, the exit while a
  descendant holds the pty, stuck input on the client's own connection, malformed peers, snapshots and trimming, the
  child's signals, environment and descriptors, the host's own signal mask, descriptors and command line, terminal
  settings byte-equal to a new pty's with only erase and IUTF8 changed), in-process tests of its event loop (among them
  the order of replies and the stream's first record that clients rely on), golden fixtures of every version-1 frame
  (`tests/golden/v1/`), and property tests of the codec and of the journal against a model. Seconds. Part of the
  workspace nextest run; the focused macOS CI job runs it too, in a step of its own after the uninstall acceptance.
  Nextest runs each test in a process of its own, which these fixtures need: under libtest (`cargo test`, and the
  coverage and mutation runs below) run them with one test thread (`RUST_TEST_THREADS=1` or `-- --test-threads=1`).
  Plain parallel `cargo test` is not supported for this crate: a child one test forks holds every descriptor of the test
  process until it execs, and on macOS, which cannot open a pty or socket close-on-exec in one call, it can keep another
  test's for good; that test then waits for an end of output or a close that never comes
  (`crates/terminal-host/TESTING.md`).
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'terminal host spawning, client, discovery and cleanup' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor -p farhelm-teststate -p farhelm --lib --test terminal_host_spawn --test terminal_host_client --test terminal_host_stop --test terminal_host_registry -E 'test(/^hosts::/) | binary(terminal_host_spawn) | binary(terminal_host_client) | binary(terminal_host_stop) | binary(terminal_host_registry)'`
  — the supervisor's side of terminal hosts and the harnesses' cleanup of them. The supervisor's `hosts::` unit tests
  cover the private socket directory, the socket path bound against macOS's 104-byte limit, the client tag's golden
  fixtures (`crates/farhelm-supervisor/tests/fixtures/host-tag/`, which must never change for a shipped version) and
  unreadable tags, and the spawn's start protocol against stand-in first stages (readiness from the exit status alone, a
  relative state directory, and a start that does not finish ending whatever answers at its socket or naming it), the
  client's connection outcomes against stand-in listeners (a missing socket gone without retries, refusals gone only
  after the refusal window, which any other answer restarts, busy retried and never gone, foreign versions, malformed
  and silent answers unavailable), its input window, reply matching, one `Read` per connection, the write deadline of a
  dropped half-written frame, events before an ending frame, the event queue's bound and what `shutdown` reports, and
  the verified termination against stand-in processes (a stopped match ended by SIGTERM with SIGCONT, one that does not
  act on SIGTERM killed, another socket or start identity never signalled, an exited process already gone, the serving
  process found by its command line, and under another spelling of its directory found by its socket's name but never
  signalled; process-table walks one at a time, a walk that ran out of time failing later searches at once until it
  returns and a search waiting for a healthy walk), the stop of a state directory's hosts against stand-ins (a missing
  directory with nothing to stop, only sockets directly inside counted, a symlinked or unreadable socket directory
  failing, a listener that is not a host never signalled, a `Goodbye` or a closed connection without an end unconfirmed,
  a signal candidate already gone confirming nothing while the socket is still served, a stalled search and slow hosts
  cut off by the time budget, a failed host task reported as failed, a report nobody reads given up on), and discovery
  against stand-in hosts (a directory mixing hosts serving the golden tags, a newer release's and a foreign tag, another
  protocol version, silence, a missing `Info`, busy, a stale socket and entries that are not sockets, sorted into their
  classes and looked up by session, generation and tab order; a missing socket directory, a refused one, two hosts
  claiming one agent launch, tabs in creation-stamp order against tab-id and name order with a duplicated tab id
  flagged, the caller's own sockets left uncontacted, a taken host's connection, and a gone socket removed only while it
  is still gone: never when a listener bound at its path has the inode number the scan recorded, set on purpose so that
  every filesystem checks it, nor when a live process names it on its command line); `farhelm-teststate`'s `hosts::`
  tests cover its blocking shutdown client against stand-in listeners (refusals believed only over its stale window, an
  incomplete scan kept). `crates/farhelm/tests/terminal_host_spawn.rs` starts real hosts from the built `farhelm` (never
  inside a test process, since a host forks): readiness and no zombie, a failed start leaving nothing, no `FARHELM_*`
  marker or credential on the host while its child has them, and the test-state cleanup for a normal run, a dead run,
  nested `remote/` state, and a stopped host that keeps its run (a sweep, a `TestDir` drop and `close`) for a retry,
  plus, on Linux only, a stopped host with a full backlog. `crates/farhelm/tests/terminal_host_client.rs` drives the
  supervisor's client against real hosts the same way: take-over and the peer process, a stopped host timing out and
  then ended by the verified SIGTERM, a stopped host with a full backlog found by its process rather than judged gone, a
  killed host's socket judged gone, a read decoding the snapshot record, a gap and the exit, resizes and snapshot
  replies, input acknowledged in order, Shift+Enter in one read, no resend after a takeover mid-input, and `Shutdown`.
  `crates/farhelm/tests/terminal_host_stop.rs` runs `farhelm internal stop-terminal-hosts` against real hosts, each
  success checked against the host processes when the command returns: live hosts shut down with their programs hung up,
  a killed host's socket file removed, a stopped host ended by the verified SIGTERM, a state directory given through a
  symlink stopping its live host and leaving a stopped one unconfirmed and unsignalled, a `hosts` symlink whose target's
  host is left running, a missing directory succeeding at once, a socket that cannot be confirmed failing the run while
  a host beside it still stops, a `Goodbye` without an end failing it, a relative or empty state directory refused as a
  usage error, and a full, unread stderr not holding the exit back. `crates/farhelm/tests/terminal_host_registry.rs`
  scans real hosts: the spawn's own found by their tags, a host with a foreign tag kept as unavailable, a killed host's
  socket judged gone and removed, and every other host still running after the scan. Under a minute, part of the
  workspace nextest run; the focused macOS CI job runs it too, in the terminal host step. Portable to macOS except where
  it reads `/proc`. Outside that selection, Linux only and part of the workspace nextest run:
  `cargo nextest run -p farhelm --test e2e -E 'test(terminal_host_cleanup::)'` shows that dropping the e2e suite's
  in-process harness and its real supervisor process ends a host started in their state directories.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'host terminal runtime' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test terminal_host_runtime -E 'test(/^hosts::terminal::/) | binary(terminal_host_runtime)'`
  — the supervisor's runtime for a terminal in a terminal host (`crates/farhelm-supervisor/src/hosts/terminal.rs`). The
  `hosts::terminal::` unit tests (also part of the `hosts::` selection above) drive it against stand-in hosts in the
  test process: the replay policy (a recovered adoption answering the queries no supervisor saw and only those,
  splitting a read across output an earlier supervisor was sent, output written while none was attached, and live
  output), a fresh adoption answering everything, and a "fresh" one whose host already sent output treating that output
  as seen; a replay ending inside an open synchronized update answering nothing of it when an earlier supervisor saw it,
  and once when none did, drawn without waiting for its deadline; effects following the bytes (a resize at
  `delivered_end` drawing a seen update answers nothing, an update straddling `delivered_end` answers its seen part
  never); the goal's three replay cases with a notice subscription (a query, a clipboard write and a bell in the
  snapshot, sent to an earlier supervisor, and written while none was attached: only the unseen query answered, only
  live notices reported); notices fanned out to every subscription, one that falls behind losing the oldest; checkpoints
  taken once due at a read's clean end, at its split candidate, after a synchronized update's deadline draws it when
  output already made one due, and at once after a long sequence ends, never by time alone on a manual clock, a rejected
  one retried only after as much output again, and an orderly checkpoint taken where the terminal is clean, reported
  current with nothing new, kept as not clean mid-sequence, refused while replaying, reporting a rejection, and timed
  out, with the deadline kept on a current-thread runtime while the encode is held (an orderly one, and an automatic one
  started from a read), the status and views answering meanwhile and a read of the whole emulator waiting for it to come
  back (a non-waiting read reading nothing, then again once the status changes); checkpoints due by four times the
  latest snapshot's size within their floor and ceiling, and taken where a replay ends, never at a read's split
  candidate before the threshold; an encoder panic handing the emulator back and reported to an orderly stop as a failed
  encode, a lost encode task ending the runtime as `EmulatorLost` and releasing a waiting read with an error, and a
  shutdown during an encode answering the waiting read first; notices queued for no one when nobody subscribes, and
  every subscription ending with the runtime; a gap keeping the emulator, marking the history trimmed (readable with the
  emulator) and asking for a redraw with a resize one row smaller and one back, and a recovery across a gap asking for
  it only once caught up; a snapshot record restored once (before adoption returns, so a seeded host is caught up only
  with its seed) and not again on a resumed stream, and one that does not restore making the host unavailable; a
  synchronized update drawn at its deadline on a manual clock; the child's exit ending its root for good and reaching
  the notifier, and an exit known from `Info` (at adoption or on a reconnect) withdrawing the root at once but announced
  only with its record; the adapters to the supervisor's terminal-neutral process types for every liveness, with a child
  pinned to a reused pid no root; input fates (written, discarded, unknown when the connection ends and not resent on
  the next one); callers held while the window is full, a cancelled waiting send sending nothing; nothing written after
  input lost with a connection, a send still waiting for room in the queue included, nor any later part sent through one
  input sequence; a paste going out as consecutive `Input`s with one receipt for all, and one cut short by a lost
  connection closed first on the new connection; a closing marker owed for a paste after an earlier one in the window
  was acknowledged, and owed again after a second loss before its acknowledgement; a paste waiting for the terminal's
  paste budget while an earlier one is unsent, and refused as ended once the runtime is aborted; a lost connection
  freeing no room in the input queue while busy callers wait; input cut off while written unknown and not resent, the
  epoch moving once for the loss (Linux); a reconnect answered by another process than the adopted host unavailable as
  replaced, naming the adopted one; answers to a query flood bounded by the window and the held-answer cap; a dropped
  connection resumed from the processed position, a resize lost with one sent again and one already in force not, a
  resize to the size in force never sent, and resizes taking effect at the host's record; the reconnect limit; a
  shutdown whose process stays reported unconfirmed, and one on a connection that ended unnoticed sent again on a new
  connection (Linux). The stand-in answers the adoption's first `Info` after `Read` ahead of the stream's first records,
  as a real host may. `crates/farhelm/tests/terminal_host_runtime.rs` runs it against real hosts started from the built
  `farhelm` (never inside a test process, since a host forks): output, title and the pinned child; a DA1 query answered
  end to end; input reaching the child and its exit recorded; an update left open drawn once a manual clock passes its
  deadline; a host seeded with a snapshot, and one seeded with several hundred KiB of history and a silent child caught
  up with the seed on screen when adoption returns (a seed that is not a snapshot failing the adoption); a recovered
  runtime rebuilding the screen; the replay policy across a crash (a raw-mode child reading each answer in turn: a query
  in the snapshot and one sent to the crashed supervisor not answered again, one asked while none was attached answered,
  its premise shown by an `Info`-only connection, and only the clipboard write after the recovery reported, never a
  bell); output overflowing the smallest journal while no supervisor is attached, with a shell that sets a mode in the
  lost output, marked trimmed and redrawn on `SIGWINCH` with the mode back; a sequence left open then silence refused an
  orderly checkpoint and ending in a gap; a sequence longer than the cap written while away ending in a gap; the FIFO
  helpers giving up within their bound when nobody is at the other end; a resize; and a shutdown confirmed by the host's
  process. Seconds, part of the workspace nextest run; the focused macOS CI job runs it too, in the terminal host step.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'host terminal switch points' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/core::tests::host_terminals::/) | test(/^service::session_hosts::/) | test(sweep_launch_dir_keeps_every_host_record) | test(/^host_terminals::/)'`
  — the supervisor's terminal switch points with a terminal host behind a session's agent (`Terminal` in
  `crates/farhelm-supervisor/src/service/terminals.rs`). The unit tests (`service::core::tests::host_terminals`) use a
  stand-in host in the test process (`hosts::terminal::test_host`) whose child is a `sleep` the stand-in owns: status
  and the recorded exit code, the never-started excerpt from the emulator, Stop before the host's exit record (a plain
  exit, then the launch failure once the record arrives) and Stop classifying from a record that arrives while it waits,
  the wait for that record pending until it arrives, the launch shim's own report recorded without waiting, an
  unreadable host left alone (no transition even beside a current sentinel, unknown status, a root that its earlier
  probe authorized refused, refused verbs and prompt), the ticker recording a host exit on the liveness notifier's wake
  with the tick far away, the hook anchor (and the publication gap's retry), the orderly stop's hold wait, prompt
  delivery gated on the emulator's paste mode, at one part and at the prompt cap's two, and grid attaches over the
  supervisor's own connection handling (a grid attach catching up and typing into the host, a takeover displacing a grid
  attachment, a paused grid viewer detached as stalled, an attach to a startup placeholder refused with an incumbent
  kept). Delete's handling of hosts (`service::session_hosts`) against stand-ins, some in the state directory's socket
  directory named by a launch record the test writes: an unconditional Delete of an unreadable agent host killing its
  child from the identity pinned at adoption and keeping the session when its adopted process cannot be confirmed ended
  (the stand-in's process is the test runner, which the verified signal never signals); a recorded host that answers
  only with a foreign protocol refused by the agent guard and signalled, never reconnected to, by an unconditional
  Delete (the stand-in counts two connections, one per settling); a record naming another session's host refusing and
  touching nothing; the record of the host the agent's runtime holds left to the runtime (the stand-in counts only the
  adoption's connection); a gone record's leftover socket file removed with the session deleted; and the overall
  deadline for settling and ending hosts (a gate holding each end before it starts, not a stuck process read, and a
  one-second budget) returning a retryable refusal and releasing the directory admission and the lifecycle claim. Pure
  unit tests cover the bounded join and the startup sweep keeping every launch host record. The e2e tests
  (`crates/farhelm/tests/e2e/host_terminals.rs`) give a created session's agent a real host the test started from the
  built `farhelm`, in place of the host its create started (`Supervisor::install_host_agent_for_test`), and check the
  exit stored with its code, Stop and Delete killing an unmarked process tree from the host's child (Stop leaving the
  host's process running with its final screen), Restart stopping that tree before its new host starts, the delete
  guard, and a grid attach showing the host child's output; and Delete ending hosts, every test's grandchild ignoring
  SIGHUP (it publishes its pid only after its trap; disposition and ancestry checked first) so that only a sweep rooted
  at the host's child reaches it: a host agent guarded and then ended (gone at Delete's return) beside a record naming
  the host its runtime holds, a recorded host of another generation refused by the agent guard beside an ended agent and
  then ended while another session's recorded host is never touched, a terminal-less session's recorded agent host, a
  SIGSTOPped host agent ended by the verified signal, a SIGSTOPped recorded host refused by the guard and then
  signalled, and a host renamed over the agent's socket neither shut down nor signalled while Delete ends the host the
  runtime adopted and deletes the session. In `host_creates.rs` (the selection below): the unbound recorded start an
  interrupted create leaves fenced by an unconditional Delete, and Delete of a session whose runtime is held in a
  snapshot encode ending the adopted host by its process after another host was renamed over its socket, which is left
  running. Linux only (the e2e suite), a few minutes (several tests wait out a stopped host's connect deadline); part of
  the workspace nextest run. Run it when a switch point, the host runtime's adapters, the status evidence, the grid
  attach path or Delete's handling of hosts change.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'creating agents in terminal hosts' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/core::host_launch::/) | test(/a_retry_finds_the_sessions_own_published_host/) | test(/^hosts::spawn::/) | test(/^host_creates::/)'`
  — a create starting its agent in a terminal host, where every create starts it
  (`crates/farhelm-supervisor/src/service/core/host_launch.rs`). The unit tests pin the host child's environment as a
  function of its inputs (the supervisor's environment passed through, `TERM_PROGRAM=farhelm` with the version the
  emulator's XTVERSION answer names, no `TMUX`, `TMUX_PANE` or `LISTEN_*`, where the launch environment applies, the
  default `PATH`), the launch record's encoding and its refusals, a first stage started apart from its spec (pinned,
  given nothing until finished, killed having read nothing when dropped, a taken name refused), a retry that finds the
  session's own published host with no other evidence. The e2e tests (`crates/farhelm/tests/e2e/host_creates.rs`) run
  in-process supervisors starting hosts from the built `farhelm`: the row recorded as a host row with the socket's name,
  no tmux session, Stop ending the agent and sparing the host; the host's command line and empty environment holding no
  credential or marker while its child has the markers; the agent in its launch scope with the host outside it (with a
  user manager, else `SKIPPED`); the login shell's environment (Farhelm's terminal variables, the supervisor's `PATH`,
  the launch environment's `HOME`, no `TMUX` or `TMUX_PANE`) and a startup file's tmux autostart guarded by `TMUX`
  running; a wrapper prefix running in the host; a create interrupted after its host started retried without a second
  agent; a first stage that outlived an interrupted create (holding its spec, before binding) waited for by the retry,
  which then finds its host, or ended by the verified signal when it does not finish, with one host and one agent either
  way, and fenced the same way by an unconditional Delete of that session after a restart, which then leaves no host and
  no agent; a process the interrupted first stage forked that still names the socket (a stand-in second stage) leaving
  the retry unresolved while it stays, and letting the same retry relaunch when it ends during the retry's wait; a
  launch error's cleanup after a restart keeping the host record of a host that still runs; a cleanup whose runtime is
  held by a snapshot encode (an encode gate) falling back to ending the host by its socket after the runtime's 15 s
  bound, while the encode is still held (the gate shows no encode passed or expired); a create that lost to Delete
  ending its host; a start whose first stage dies after readiness leaving nothing, and one the spawn reports may have
  left a host, which the create ends and removes the socket of; stand-in hosts that cannot be used, ended by the
  verified signal and rolled back, or kept as a launching record when the signal must not touch them; and a keyed create
  an older supervisor started in tmux and was interrupted after its launching row (made by writing the row's kind)
  refused when retried, its row rolled back, no host started, and a second retry refused the same way. Needs Python 3 at
  `/usr/bin/python3` (the stand-in hosts). Linux only, two or three minutes (several tests wait out client deadlines);
  part of the workspace nextest run. Environment supervisors use the same ordinary host launch path, exercised by the
  real Podman suite above. Run this selection when the create path, the launch spec's composition, the host spawn or the
  launch identity's rollback or evidence changes.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'restarting host agents' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/host_handover::tests::/) | test(a_terminal_less_host_row_restarts_in_a_host_never_in_tmux) | test(/a_restart_seed_|_seed_of_an?_|a_seed_whose_runtime|a_stop_during_a_restarts_seed|a_restart_without_consent_touches_no_program|a_restart_asks_again_for_consent/) | test(a_failed_host_restart_after_a_stop_began_restores_a_placeholder) | test(restart_of_a_host_agent_stops_its_tree_before_a_new_host_starts) | test(/^host_relaunches::/)'`
  — Restart of an agent in a terminal host (`crates/farhelm-supervisor/src/service/core/host_restart.rs`) and the
  startup rules for the hosts a restart left (`host_handover`, "Superseded agent hosts" and a restart that never started
  its replacement). The unit tests pin the superseded rule as a function of a host's tag and its session's row
  (superseded only by a confirmed later launch in another host: never while the row is launching, when its
  `terminal_address` names the host itself, for the current generation, a tmux row, another session, a tab, or no row),
  which hosts count as the row's own at its `terminal_address`, and the claim-window fallback choosing only from a
  launch directory listed in full; a Restart of a terminal-less host row going to a host, never tmux (with no host
  program, failing definitively with the row as it was); the runtime's Restart seed against a stand-in host (encoded on
  the blocking pool with the emulator handed back and the runtime driving on; on a runtime with one blocking thread held
  busy, a seed whose runtime is stopped while its encode is queued answering at once with the encode ending, and a
  cancelled seed of a runtime that had ended leaving the emulator readable; a seed's caller cancelled at the encode gate
  leaving the emulator to come back); an orderly stop during a Restart's seed (held at the gate) refusing the Restart
  promptly with no generation claimed; a Restart without consent of a working host agent touching neither its program
  nor that of an earlier start's host, also when the agent turned working while the Restart settled that start; and a
  host restart failing definitively after an orderly stop gathered the host terminals (held at its window) putting back
  a placeholder, the old runtime ended and its held checkpoint encode cancelled. The e2e tests run in-process
  supervisors with real hosts started from the built `farhelm`: a Restart starting a new host at the old size that keeps
  the old history (and none of the old screen) with default modes, ending the old host and removing its launch record;
  Restart stopping a running host agent's tree, with a grandchild that ignores SIGHUP, before the new host's start
  begins, as that start records (in `host_terminals.rs`); a second Restart fencing the first stage an interrupted one
  left holding its spec, so that releasing it afterwards starts no second agent; a host agent's tab, its host, shell and
  launch record surviving a Restart; an orderly stop during a Restart's window (held at its gate) closing the session's
  host tab runtime, with the Restart, succeeding or failing, bringing none back connected; a wedged host a confirmed
  restart replaced not holding a later Restart up; a replacement whose first stage refuses unwound definitively, the old
  host kept as the session's terminal for that supervisor and adopted by the next one through the row's
  `terminal_address`, and a later Restart moving on from it; an unusable replacement host ended by the verified signal
  and the restart unwound; one that cannot be confirmed ended keeping the launching record with no terminal and the old
  runtime stopped, and Delete then ending both; a failed confirmation write publishing the new host and still ending the
  old one; a grid viewer detached as "session restarted" and reattaching to the new host's screen and history; the seed
  able to hold what the old program printed as it was stopped; a crash (the fault seam) right after the generation
  claim, and one after a failed replacement was cleaned up, each leaving the old host the session's terminal at the next
  start, with its history kept by the next Restart; with the newest earlier host unreadable at that start, no older host
  taken, and the newest taken at the start after; with the newest earlier host dead, the next older one taken with its
  history; a restart interrupted after its new host started, and one after its confirmation, each converging at the next
  start (the new host adopted and confirmed, the old host ended and its record removed); an earlier host stopped with
  SIGSTOP left alone by the next start and ended by the one after; a restart whose row is deleted through the store
  mid-launch ending both hosts; and Delete after an interrupted restart ending both generations' hosts. Needs Python 3
  at `/usr/bin/python3` (the unusable host's stand-in). Linux only (the e2e suite), about three minutes; part of the
  workspace nextest run. Run it when Restart's host path, the host spawn, the runtime's seed, reload's host attribution
  or the superseded rule changes.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'host terminals across supervisor restarts' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/a_prompt_waits_for_a_recovered_host_terminal_to_catch_up|spare_claims_read_a_host_rows_own_host|a_stop_request_survives_a_restart|an_unreadable_launch_record_leaves_its_session_unavailable|an_orderly_stop_ends_its_encodes_before_its_runtime_drops|a_readoption_that_loses_its_fence|readoption_|a_scan_restricted_to_some_names|delete_settles_the_record_of_a_startup_placeholders_socket/) | test(/^host_restarts::/)'`
  — a restarted supervisor keeping its host terminals (`crates/farhelm-supervisor/src/service/core/host_handover.rs`):
  the orderly stop's checkpoints, reload finding each host row's host again, and re-adoption of hosts that were
  unavailable. The unit tests (in `service::core::tests::host_terminals`, against stand-in hosts, some bound in the
  state directory's socket directory where a reload finds them) refuse a prompt while a recovered runtime is still
  replaying, even with paste mode on, and deliver it once caught up; protect a host session's spare-checkout claim while
  its runtime runs or cannot read its host, release it once its child exited, and, with no terminal on the entry,
  release it only while no socket file is at the path its row names; keep a durable stop request through a reload whose
  host reports its child reaped before the exit record is replayed, and record the exit with its code and the stop
  annotation once it is; keep an unavailable terminal, recording and removing nothing (a launch sentinel included), for
  a session whose launch record cannot be read beside a host whose tag cannot; and, on a runtime the test owns, end an
  orderly stop's checkpoints within its budget while every encode is held, with the held encode released by its cancel,
  the runtime dropping at once and the state directory's claim free; a re-adoption of a startup placeholder whose entry
  changed, or during which an orderly stop began, installing nothing and ending the runtime it made; and re-adoption's
  backoff (a placeholder first tried a second after it is seen, the next try scheduled later before each try, none
  repeated before it is due while the session is busy); and Delete of a session holding a placeholder settling the
  launch record of its socket (its recorded first stage fenced) rather than reaching the host as a runtime's.
  `hosts::registry`'s restricted scan contacts only the names asked for. The e2e tests
  (`crates/farhelm/tests/e2e/host_restarts.rs`) run in-process supervisors with real hosts started from the built
  `farhelm`, dropped and constructed again on one state directory: an orderly stop leaving each host a snapshot where
  its terminal is and ending its runtime, after which the next supervisor publishes the same host, restored from that
  snapshot, with the same screen and a `Running` row, keeps that very runtime through `serve`'s second reload, and
  drives it; a crash (no orderly stop) recovered from a snapshot plus output seen before the crash and output written
  while no supervisor was attached; an agent that exits with code 7 while no supervisor runs, recorded with its code
  once replayed; a host killed while none runs, its session recorded as exited with no terminal; a create interrupted
  before its confirmation found by its host's tag and confirmed, and one whose login shell failed before the launch step
  ending as a launch error once its exit is replayed; a stop whose checkpoint ran out of time (its encode held) leaving
  the previous snapshot, from which the next supervisor recovers with the tail; a create published after the stop began
  gathering, on a runtime the test owns, publishing a placeholder, its runtime stopped and its held encode cancelled,
  and the runtime dropping at once; and a host stopped with SIGSTOP left alone (unavailable, listed unknown, nothing
  recorded, Stop refused for the unavailable host, the host and its agent still running) and adopted by the same
  supervisor once continued. Linux only (the e2e suite), a minute or so; part of the workspace nextest run. Run it when
  reload, the orderly stop, re-adoption, discovery or the host runtime's adoption or encodes change. The `readoption_`
  tests also cover recovery after a runtime ran: timeout recovery on the ticker's liveness wake, every unstable
  episode's full reconnect window, retry counts kept across success with no healthy-key spin, final refusal reasons, the
  stop fence, and an exited runtime replayed without recording its exit twice; tabs recovering in place with their
  ordering and scope, startup placeholders learning their tags, retry counts kept through an agent Restart, failed
  closes left to the reap, gone tabs closed with their records removed and adopted process kept, and cell/stop fences.
  The encoder's cancel itself is in the terminal emulator selection below
  (`an_encode_stops_once_its_cancel_fires_and_is_unchanged_until_then`).
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'terminal tabs in terminal hosts' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(launch_file_names_round_trip_through_the_parser) | test(sweep_launch_dir_keeps_every_host_record) | test(/core::host_tabs::tests::/) | test(closing_a_tab_whose_runtime_reads_gone_confirms_its_process_first) | test(/^host_tabs::/)'`
  — terminal tabs, each of which runs in a terminal host of its own
  (`crates/farhelm-supervisor/src/service/core/host_tabs.rs`). The unit tests pin a tab host's launch record name
  (`<id>.tab-<tab id>.host`) round-tripping through the launch-directory name parser, which accepts the tab form only
  for a `.host` record; the startup sweep keeping tab records; the tab strip's creation order against stand-in hosts (an
  exited tab hidden and picked by the reap, unreadable tabs kept and never reaped, a tab whose close failed reaped
  again, once its retry time comes, which backs off from a second to a minute) and a new tab stamped past the session's
  newest; a close whose tab runtime reads its host gone while the adopted process runs (a stand-in) keeping the tab, its
  record and its retry mark. The e2e tests (`crates/farhelm/tests/e2e/host_tabs.rs`) run in-process supervisors with
  `/bin/sh` tab shells, starting hosts from the built `farhelm`: a tab beside a host agent opened at the agent's size,
  its record naming its socket, typed into, attached as a grid, a takeover displacing the previous client's agent and
  tab attachments together, and closed leaving no host, socket, record or tab process; a tab shell dead at open (the
  settle held until its pid is gone) refused with its last words, leaving no tab host process, socket or record; opens
  failed just before publishing, and (with the user manager, else `SKIPPED`) after the host started and before its
  adoption, leaving nothing at the refusal's return, the latter killing a SIGHUP-immune job of the tab's shell and its
  tab scope; a SIGHUP-immune job with no markers under a tab's shell killed by its close and by Delete; a tab start
  interrupted with its first stage holding the spec settled by the next supervisor's reload (which waits for it) and its
  host adopted, then ended by Delete; `serve`'s reload keeping held tab runtimes without contacting their hosts while it
  scans for another session's agent; a tab host stopped across a restart kept as a listed placeholder whose close and
  guarded Delete are refused and which an unconditional Delete ends, and a tab runtime made unreadable (its reconnect
  limit) treated the same way; an exited tab, and a tab whose host was killed, reaped with a ten-minute ticker interval;
  a close whose host end fails (a seam) finished by the reap, no sooner than its one-second backoff; and an orderly
  restart keeping a tab listed, at the same socket, with its screen, attachable as a grid and typed into. Needs the
  Linux only (the e2e suite), a minute or two; part of the workspace nextest run. Run it when tab opening, closing, the
  tab strip, the tab reap, the tab records, Delete's handling of tab runtimes or the reload's tab adoption change.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'reclaiming orphaned terminal hosts' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(known_session_ids_are_every_rows_and_every_reservations) | test(/^host_reclaim::/)'`
  — the startup reclaim of orphaned terminal hosts (`crates/farhelm-supervisor/src/service/core/host_reclaim.rs`). The
  store's unit test pins which sessions the database knows (every row's and every reservation's, a reservation whose row
  was rolled back included, a session deleted with its reservations not). The e2e tests
  (`crates/farhelm/tests/e2e/host_reclaim.rs`) run in-process supervisors with real hosts started from the built
  `farhelm`, and check the state once `serve` accepts: a session missing from the database whose launch records remain
  keeping its agent and tab hosts running with their records, while its killed tab's record and socket file go and
  another session's adopted runtime is never taken over; a database removed, then a new session made in its replacement,
  then a restart, leaving the old hosts running while an unrecorded orphan nobody knows is ended at the same start;
  unrecorded hosts tagged for sessions nobody knows (agent and tab) and an unrecorded tab of a known session ended, the
  known session's agent program running on, and a killed host's socket file removed; with both sessions' launch records
  removed so that the database alone knows them, a host whose session has only a pending reservation and an unrecorded
  agent host of a session known only by its row (at a generation it never had) left running, as is a SIGSTOPped host of
  an unknown session, while a readable orphan beside them is ended; hosts whose snapshot does not restore and a host
  with a foreign raw tag left running beside an ordinary orphan that is ended; a `hosts` symlink keeping a record and
  the stale socket file it names in another directory; a reclaim whose discovery is held past a one-second budget (a
  fault seam and `SupervisorTimeouts::orphan_reclaim`) ending nothing, and the next start ending the orphan; a missing
  and then an empty database reclaiming nothing; a launch directory whose listing fails at its open, and one that fails
  partway after counting a record (a fault seam), reclaiming nothing, a session known only by its records keeping every
  host, socket and record, until a whole listing ends the unrecorded orphan beside them; a startup placeholder's socket
  never connected to by `serve` (a stand-in counting the connections it accepts); an interrupted start of a session
  missing from the database settled (its first stage released by the settle's wait hook) and the host it brings up left
  with its record, and one whose first stage never finishes fenced and its record cleared; and, with a ten-minute ticker
  interval, an existing session's tab whose shell was killed while no supervisor ran reaped, and a tab whose host was
  killed meanwhile leaving neither record nor socket file. The e2e harness's wait for a served supervisor's socket
  allows the reclaim's whole budget on top of its own 20 seconds. Linux only (the e2e suite), about a minute with four
  slots (stale sockets, a stopped host and a fenced first stage wait out their deadlines); part of the workspace nextest
  run. Run it when the reclaim, discovery, the launch records' settling, a host runtime's adoption, `serve`'s startup
  order or the store's reservations change.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'host terminals under a real supervisor process' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/^host_processes::/) | test(the_schema_33_marker_changes_no_table_and_keeps_every_row) | test(/^store::tests::schema_34_/)'`
  — terminal hosts the way a real supervisor process creates them: `farhelm supervisor run`. The unit tests check that
  schema 33, the marker that makes every build at 32 refuse a database a supervisor that may create host terminals
  opened, changes no table or row and is not taken by a process without the state directory's claim, and that schema 34,
  which drops the stored tmux session names and renames `pane` to `terminal_address`, keeps every session field for
  field (a tmux pane, an unconfirmed launch, a host socket), both kinds of reservation and the notification trigger,
  matches a fresh database, and refuses, leaving the database at 33, one whose stored tmux name of a session or pending
  reservation is not `fh-` and its session id (a settled reservation's does not stop it). The e2e tests
  (`crates/farhelm/tests/e2e/host_processes.rs`) start the built `farhelm` as processes and look only through what a
  viewer sees (a grid attachment's screen and the input it sends), the host's process by its command line, and the
  session's row: a bare `farhelm supervisor run` creates a session as a host row whose agent and a newly opened tab both
  attach as grid streams; its host agent comes through an orderly restart (SIGTERM, a clean exit) and a SIGKILL of the
  supervisor process with the same host process, its screen (the ready line and a line echoed before) and its input; a
  state directory whose `supervisor.sock` path is 103 bytes, macOS's longest, holds its host sockets (each path shorter
  than the supervisor's) and the terminal works; with a real helm on the same state directory, a session created through
  the helm's API reaches a hand-rolled browser WebSocket as grid messages showing the agent's screen, and `text` and
  `key` messages sent up the socket reach the program; and a bare supervisor given a recording `FARHELM_TMUX` running no
  tmux at all through a host session's create, attach, tab, listing, rename, several ticker intervals, Delete and
  SIGTERM, with no tmux socket left. Linux only (the e2e suite), a minute or two; part of the workspace nextest run. Run
  it when the startup, the schema, the orderly stop, reload, the host socket's path, the helm's grid relay, or when and
  whether the supervisor uses tmux changes.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'ending old terminals' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/core::old_terminals::/) | test(/^hosts::process::/) | test(the_child_gets_the_supervisors_environment_in_farhelms_terminal) | test(/^old_terminals::/) | test(a_retry_of_a_create_interrupted_in_tmux_is_refused)'`
  — the startup step that ends the old terminals, the private tmux server earlier releases ran sessions in
  (`crates/farhelm-supervisor/src/service/core/old_terminals.rs`). The unit tests pin how a pane of that server is
  attributed to a session's agent or one of its tabs by its markers, the verified signal's command-line test as a
  parameter (`hosts::process`), and `FARHELM_TMUX` kept from a terminal host's child. The e2e tests
  (`crates/farhelm/tests/e2e/old_terminals.rs`) build the earlier release's layout by hand with the `tmux` on `PATH` (a
  server on the state directory's `tmux.sock`, `fh-<id>` sessions, marked windows whose panes carry the markers, and
  old-kind rows) and start a supervisor of this release on it: a dead old agent's job with no markers, alive only in its
  launch scope, ended with the server and its files, the row interrupted with its pane id kept and one unresolved notice
  that survives another start, the old warning resolved, and Restart starting the session in a host (with a user
  manager, else `SKIPPED`); a server killed beforehand, its refusing socket, config and staged config write removed, an
  old agent's marked SIGHUP-immune daemon ended by the row's identity, and an old row that had exited left as it was
  with no notice; and an old tab of a session whose agent and other tab run in hosts ended (its SIGHUP-immune job too)
  while those hosts run on and the session gets no notice. The retry test shows that a create an older release left
  interrupted in tmux is still refused once the startup interrupts its row. Its e2e tests need a tmux on `PATH` (the
  machine's; without one they print `SKIPPED`). Linux only (the e2e suite), a minute or so; part of the workspace
  nextest run. Run it when the startup's ending of the old terminals, the reload's handling of old-kind rows or the
  verified signal changes.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'grid streams' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor --lib -E 'test(/^grid_output::/) | test(/^service::grid_stream::/)'`
  — how a host terminal reaches a grid viewer: the encoding (`crates/farhelm-supervisor/src/grid_output.rs`: cells to
  runs with styles, redefined palette colors, wide and combining characters and links, a row with more links than a
  message holds, hostile text packing into messages a client accepts, the screen diff, its shift and a shift-only
  update, a resizing frame, the cursor's default blink, history appended across reads, reads bounded by lines and by
  bytes (links counted, checked against the encoded size), eviction, erasure, relayout and palette-rewrite resets,
  history's width and the alternate screen, message packing, clipboard parts) and the per-attachment sender and input
  side (`crates/farhelm-supervisor/src/service/grid_stream.rs`) against a stand-in host in the test process: the attach
  sequence while output runs; a paused flood withheld while paused (seen through the stream's park count, so a sender
  that ignored the pause fails) and losing no history line; a writer queue shown full across several history batches;
  eviction at the cap; a resize, an erasure and a restore resetting history, and a resize drag resetting it once it
  settles; output and an exit published between a read and its bookkeeping not lost (held at a read seam), and output or
  an exit during a long catch-up not lost; a resize drag during a long catch-up ending it and resetting history once,
  after `ReplayComplete`; the stall bound, and an input failure ending a paused stream at once; clipboard writes, the
  latest kept, with neither bells nor primary-selection writes able to push one out; the exit in `state`; an unreadable
  host ending the stream; keys, text and bracketed pastes; a paste written only once its last part arrives, and nothing
  of one whose last part never does, of one past the backlog, or of one half collected at a takeover; empty paste parts
  adding nothing; query answers and another caller's input never landing inside a paste; a paste cut short by a lost
  connection closed first on the new connection, ahead of the kept answers, and the attachment's input failing; ended
  attachments leaving no pump; refused input; and input taken while the emulator is held. Every stream is applied to a
  test client that runs the format's stream check (`grid_output::test_client`). Seconds, in-process. Part of the
  workspace nextest run. Its ignored tests (one per kind of output: typical, 256-color, true color) measure the attach
  cost and a resize drag's cost at the full scrollback (bytes, history resets, and arrival times over a simulated 10
  Mbit/s link): run them by hand in release with `--run-ignored only --no-capture --test-threads 1` and
  `-E 'test(/measure_the_(attach|resize)_cost_at_the_scrollback_cap/)'`, the attach ones optionally with
  `FARHELM_GRID_MEASURE_OUT` naming a directory for their messages, each written as a 4-byte little-endian length and
  the message (passed with `--keep-farhelm-env`); they assert nothing.
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
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'terminal emulator' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-terminal-emulator`
  — the supervisor's terminal emulator (`crates/farhelm-terminal-emulator/`), run in-process with no pty or network:
  Farhelm's query-answer table, side effects by feed mode (live, replay of seen and of unseen output), the
  synchronized-output deadline on a hand-stepped clock (and its flush before a resize), the snapshot split candidate on
  agent-shaped reads, a property test that whole, byte-at-a-time, chunked and candidate-split feeding end in the same
  state once any open update is flushed, the handler's forwarding as a property test against a bare
  `alacritty_terminal`, Farhelm's screen-text format and titles (tabs, modes 47 and 1047), the mode accessors (resetting
  any mouse tracking mode ends reporting), one-column terminals with wide characters and resizes (each bounded to 10
  seconds, so that a regression of the vendored reflow fix fails after about 1 GB of allocation instead of running until
  memory runs out), a screen full of tabs captured in linear time, and hostile output through the vendored caps
  (combining floods, 8 MB OSC 52 and titles, long hyperlinks, floods of hyperlinks and combining marks against the
  per-terminal extra storage, a synchronized update past vte's 2 MiB buffer). The terminal snapshot format: the round
  trip (a recording fed in reads, snapshots where the emulator is clean after a read's split candidate or end, restore,
  then compare field by field with the uninterrupted emulator after the rest of the output in the same reads, after
  probes, and after the rest in other reads and a final flush) as a property test over generated output, over a
  one-column terminal and over two short synthetic recordings, and over 20,000 short scripts of awkward output and
  resizes (from 8x4 and two columns, each continued after the restore); the review reproducers (a one-column encoder
  panic, four round-trip losses); the version 1 golden fixtures (`tests/fixtures/snapshots/v1/`: each blob restores to
  its recorded state, each script still encodes to its blob and reaches its feature); hand-built version 1 blobs and
  refusals of malformed, truncated, oversized (before allocating) and unknown-version blobs, of streams holding
  sequences version 1 never writes or taking too much work to replay (before feeding), and of fixups no live output
  makes; history past the restore's cell bound dropped and marked trimmed; mutated and crafted blobs never panicking;
  extras shared by content through a restore, within the extra storage limit; the size ladder (titles before history);
  the trimmed mark across restores; an encode's cancel (nothing once it fires, the same bytes until then); the character
  width table's fingerprint; the Restart seeds, and a seed's encode stopping at whichever look its cancel fires on.
  About a minute. Part of the workspace nextest run. Run it when the crate or anything under `vendor/` changes. Ignored
  tests, run by hand in release: the snapshot spike's full round-trip configuration
  (`cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E 'test(/^full_size_round_trip_seed_/)'`,
  a few minutes), the review's 300,000 short scripts (`-E 'test(short_scripts_round_trip_at_review_size)'`) and 230,000
  hostile blobs (`-E 'test(hostile_blobs_never_panic_at_review_size)'`), the full round trip over private recordings of
  real sessions named in `FARHELM_SNAPSHOT_RECORDINGS` (`-E 'test(private_recordings_round_trip)'`, passed to the
  recorder with `--keep-farhelm-env`), the per-terminal memory measurement against the crate's stated budget
  (`-E 'binary(memory)'`, release, Linux; its numbers print only with `--no-capture`, which the recorder's generic mode
  accepts), and `write_missing_golden_fixtures`, which writes the fixtures of a new golden script and never overwrites
  one.
- `python3 scripts/record-test-run.py --kind development --selection 'snapshot encoder benchmark' --concurrency 'one test; fixtures sequential' -- cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E 'binary(encode_speed)' --no-capture`
  — the ignored snapshot encoder benchmark (`tests/encode_speed.rs`): typical, true-color and image-like output at 200
  columns, 60 screen rows and 12,000 history lines, measured in sequence. Each reports five encodes' median and spread,
  blob size and history dropped. Fixture shape and repeated bytes are asserted; elapsed time is not. Compare on the same
  machine with load recorded. Generic recording permits `--no-capture`; the controlled nextest mode does not. Run it
  when the snapshot encoder changes, beside correctness and memory checks; minutes, no network.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'agent screen fixtures' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor --lib -E 'test(/^agent_kind::screen_fixtures::/) | test(/^agent_kind::status_line_fixtures::/)'`
  — status detection against the captured agent screens (`crates/farhelm-supervisor/tests/fixtures/screens/` and
  `tests/fixtures/status-lines/`, `docs/agent-screen-fixtures.md`): the fixtures' shape and privacy checks, each screen
  read as the state it was captured in by its harness's reader, the permission-mode footers, and each screen painted
  into the supervisor's terminal emulator reading back as the same text, with every row on its own row, and read by its
  reader as the captured screen is. The captured text is the expected value. Seconds, in-process. Part of the workspace
  nextest run; run it when a status reader, the emulator crate, `vendor/` or the fixtures change.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'grid input encoder' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-supervisor --lib -E 'test(/^grid_input::/)'`
  — the supervisor's encoder of grid viewer input (`crates/farhelm-supervisor/src/grid_input.rs`): keys against
  Farhelm's key table `crates/farhelm-supervisor/tests/fixtures/grid-keys.jsonl` (the sequence for every key code below
  256, modifier set, platform and cursor-key mode), mouse reports against its mouse table
  `crates/farhelm-supervisor/tests/fixtures/grid-mouse.jsonl` (every grid mouse event, modifier set, reporting mode and
  encoding, at the screen's edge and the default encoding's limit), pastes (bracketed-paste markers once around a paste
  of several parts, line endings and sanitizing carried across parts, the closing marker of an interrupted paste), UTF-8
  mouse coordinates, focus reports, the wheel on the alternate screen, and `Input` framing. Seconds, in-process. Part of
  the workspace nextest run. Which keydowns become key messages at all (third-level shift, lone capitals, the key after
  a dead key) and mouse motion de-duplication are the GUI's, not this selection's. Both tables are checked-in contract
  data, first computed from the xterm.js code Farhelm's terminals used before the grid. The key table was rendered from
  the old Dioxus UI's grid client, whose generator went with that UI; both tables are now edited by hand, together with
  the encoder, and a changed row is a key a program will read differently.
- `python3 scripts/record-test-run.py --kind development --selection 'vendored vte' --concurrency 'libtest default threads' -- cargo test --manifest-path vendor/vte/Cargo.toml --features ansi`,
  `python3 scripts/record-test-run.py --kind development --selection 'vendored alacritty_terminal' --concurrency 'libtest default threads' -- cargo test --locked --lib --manifest-path vendor/alacritty_terminal/Cargo.toml`
  and
  `python3 scripts/record-test-run.py --kind development --selection 'vendored crates paired' --concurrency 'libtest default threads' -- cargo test --locked --manifest-path vendor/pair-tests/Cargo.toml`
  — the vendored, patched terminal crates (`vendor/README.md`): upstream's unit tests plus the tests of Farhelm's
  patches (the parser's ground state, the OSC payload and its overflow notice, zero-width, hyperlink and title caps,
  hyperlink reuse, the per-terminal extra storage, the history counters, XTVERSION, the pending-wrap flag only under
  autowrap, the erase and cursor movement fixes, clearing a row after it grew), and in `vendor/pair-tests/` the tests
  that need both patched crates together (an OSC 8 the parser drops ending the terminal's hyperlink). These are excluded
  from the workspace, so no workspace test run, clippy, `cargo fmt` or dprint (beyond `vendor/README.md` and
  `vendor/pair-tests/Cargo.toml`) reaches them, and CI does not run them. Run all three when anything under `vendor/`
  changes; seconds, plus a first download of the crates' own locked dependencies.
- `python3 scripts/record-test-run.py --kind development --selection 'vendored gpui-pre native keys and driver record' --concurrency 'libtest default threads' -- cargo test --locked --manifest-path vendor/gpui-pre/Cargo.toml --features test-support --test native_key_metadata --test driver_record`
  — the vendored GPUI core's tests of Farhelm's native-key and bridge layers (`vendor/README.md`, "GPUI"): original
  repeat, Down and Up propagation, consumption before logical key matching, unchanged binding selection, and no native
  identity on synthetic keys or replayed prefixes; and a list item's selected, not-selected and unmarked states in the
  bridge's driver record. Like the terminal crates, the GPUI trees are excluded from the workspace, so nothing else runs
  it. Run it when anything under `vendor/gpui-pre*` or `vendor/patches/gpui/` changes; minutes, since it builds GPUI on
  its own.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'helm terminal relay' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-helm --lib -E 'test(/^terminal::tests::/) | test(/^client::tests::/)'`
  — the helm's terminal WebSocket (`crates/farhelm-helm/src/terminal.rs`) and its supervisor client
  (`crates/farhelm-helm/src/client.rs`), in-process against stand-in supervisors: attach, takeover, refusals, data that
  beat the attach reply reaching the browser first, replay-complete ordering, heartbeat, frame and queue bounds, the
  stall detach, teardown, and the client's requests, uploads and relays, and the attachment trace (the
  `farhelm_helm::terminal_attachments` debug events) reporting each attachment's start and end once. The grid stream:
  relayed unread both ways (a 200 KiB frame sent before the reply, a browser message of exactly the 64 KiB input bound
  arriving as one frame, resize, and a browser's pause and resume dropped after it), a message over that bound ending
  the attachment with nothing forwarded after it (on the socket, and in the client whoever sends next), a slow browser
  paused by the helm and resumed once it catches up with no frame lost, at 32 KiB messages with a writer queue's worth
  arriving after the pause and at 1 MiB rows with 8 arriving after it, a browser's own pause and resume never touching
  that flow, the queue's pause and resume marks, event and byte bounds and frame bounds, an ended attachment never
  paused again, and a wedged browser's handler ended by the stall detach (premise: parked mid-send with the attachment
  live; oracle: the handler's own teardown `Detach` and the socket's close). Under a minute. Part of the workspace
  nextest run. Run it when the helm's terminal relay, its client's terminal queue or the protocol's attach messages
  change.
- `cd crates/farhelm-supervisor/asset-js-tests && node --test` — the OMP conversation-reporter asset's own execution
  proof: dispatch ordering against a controlled subprocess mock (including a deliberately nonserialized variant the
  proof must reject), stale-id cancellation, execution-time file recheck, the subscribed transition events, and the
  silent-failure boundary, run against the real shipped asset. Run it from inside the directory rather than as a glob
  from the repo root: node's no-argument default discovery (every `*.test.js` in cwd) is the oldest, most
  version-portable form the test runner has, and CI pins no node version. The release gate runs it through the recorder
  with the `OMP reporter asset scenarios` selection.
- `cd website/feedback-tests && node --test` — the feedback endpoint's handler (`website/feedback/handler.js`, deployed
  as `website/api/feedback.js`): method, configuration and size refusals, field caps counted in code points, fences that
  user text cannot close, and that no response or log line carries the token, the inbox's name, the message or the
  contact. Configuration and the GitHub client are injected; nothing touches the network or `process.env`. Run it when
  the handler or its tests change; CI's website job runs it beside the website build.
- `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings` — the desktop app sits outside
  `default-members` (unlike the GPUI GUI crates, which are default members), so the default-member clippy run never
  lints it; the second run lints it with the test-automation bridge compiled in (`docs/gui-agent.md`). It runs the GPUI
  app, so on Linux it needs GPUI's build packages (named under the workspace nextest run) and the GLib development
  package (`libglib2.0-dev`) for GIO. CI's `desktop` job runs exactly these.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'desktop app' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-desktop`
  — the desktop app's process (`crates/farhelm-desktop/`): the bootstrap's supervisor discovery, record and replacement
  decisions, the pre-window refusal surface and its native helper commands, the host listing it reads at startup, the
  app's state files, the window frame's format and restore arithmetic, bundle resolution, the updater against injected
  dependencies (no network or real bundle), and the update readout's wording. Excluded from the workspace nextest run
  above, so this is the only run of these tests.
- `python3 -B scripts/check-gui-lints.py` and `python3 -B scripts/test-check-gui-lints.py` — proves every ban in
  `crates/gui/clippy.toml` still fires: it lints `crates/gui/lint-demo` (a non-default member that violates each ban on
  purpose behind its `violations` feature) and fails when a demo's expected diagnostic is missing or a ban has no demo.
  Clippy matches banned paths by name, so a GPUI or GPUI Kit upgrade that moves an item would otherwise end its ban
  silently. Run it when `crates/gui/clippy.toml`, the demo crate, or the GPUI or GPUI Kit versions change. Reuses the
  default-member clippy build; needs the same Linux packages. CI's `clippy` job runs both.
- `python3 -B scripts/check-crate-layering.py` and `python3 -B scripts/test-check-crate-layering.py` — from
  `cargo metadata --all-features`, fails when a crate under `crates/gui/` reaches `farhelm-helm` or `farhelm-supervisor`
  through normal or build dependencies, when one other than the GPUI app crates the script names reaches GPUI or GPUI
  Kit, or when the manifest of a GUI crate, `farhelm-desktop` or `farhelm-helm-client` does not deny clippy's
  `unwrap_used` and `expect_used`. Seconds, nothing compiled. Run it when a manifest under `crates/gui/`, either of
  those two crates' manifests, or `Cargo.lock` changes. CI's `deny` job runs both.
- `git show <base>:scripts/check-foundation-paths.py | python3 -I - --repo <checkout> --base <base> --head <PR commit>`
  and `python3 -B scripts/test-check-foundation-paths.py` — refuses a GPUI area PR, rebased onto `<base>`, that changes
  a foundation's owned path in a way no extension point admits, by the rules in the base revision's
  `docs/gpui/foundation-ownership.md`; the base's copy of the script runs, so the PR cannot change the check that judges
  it. Changes to the shared wiring (the table's `reported` rule) pass and are printed as `reported`, for the review gate
  to judge. `--owner` reports without refusing. It runs in the GPUI migration's landing step from milestone M1 on, not
  in CI. The tests build temporary git repositories. Seconds, no network. Run them when the script or the table changes.
- `python3 scripts/record-test-run.py --kind development --selection 'GPUI app goldens' --concurrency 'libtest default threads' -- python3 scripts/check-gui-goldens.py`
  — the GPUI app's screenshot goldens (`crates/gui/farhelm-gui/src/goldens.rs`): every component, the dialog frame and
  the layer manager's overlays (a tooltip, a popover, the prompt over its veil), the keyboard focus ring, host-supplied
  text over its spoofing corpus, the terminal's styles and cursor shapes, and an attached terminal's status line and
  exit line, light and dark, compared pixel for pixel with the PNGs under `crates/gui/farhelm-gui/tests/goldens/<os>/`.
  The test is ignored in workspace runs because it needs the software renderer, which the script selects (Mesa's
  lavapipe on Linux: the `mesa-vulkan-drivers` and `libegl-mesa0` packages). After an intended visual change, `--update`
  rewrites the references; look at each changed PNG before keeping it. A failure leaves the new captures in
  `gui-goldens/` beside the test binary. Run it when anything under `crates/gui/farhelm-gui/` that draws, its fonts, or
  GPUI or GPUI Kit change; CI does not run it yet.
- `python3 -B scripts/test-gui-agent.py` — the harness that drives the GPUI app (`scripts/gui-agent.py`,
  `docs/gui-agent.md`; headless on Linux, in the GUI session on a Mac): the run directory's checks (absolute, and
  shallow enough for the deepest socket's path limit), the app's environment (nothing of the caller's display, bus,
  bridge or `FARHELM_*` variables; on Linux the run's own compositor and lavapipe; the run's state directory and
  supervisor program, and the window size and appearance a run asks for), the seeding requests (a `--fleet` kind's
  replay shape, its transcript fitting a narrow terminal, and only the waiting kind declaring an agent type), the wait
  for a fleet's statuses, `terminal-text --wait` asking again until its text shows, that a recorded pid whose start time
  no longer matches is never signalled (from /proc on Linux, `ps` on macOS, in the C locale and UTC), and the `--lock`
  rules: held until the holder is stopped, kept by `stop` while part of the run may still be running, given up by a
  holder whose `start` was killed while it waited, and a lock path that cannot be opened failing at once. Seconds;
  starts and builds nothing; runs on Linux and macOS. The harness itself is a tool, not a gate: an end-to-end run
  (`start`, then `stop`) builds the app and needs `mutter`, `dbus-run-session` and lavapipe on Linux, or a logged-in GUI
  session of the same user on a Mac. Run the tests when the script changes, and an end-to-end run when the script, the
  bridge's wiring in `farhelm-gui` or `farhelm-desktop`, or the app's startup changes.
- `python3 -B scripts/test-gui-screenshot-batch.py` — the page `scripts/gui-screenshot-batch.py` builds to compare the
  GPUI app with the old app's reference pictures (`docs/gpui/screenshot-batch.md`): unported reference scenes say "not
  yet ported" and pictures the batch did not take say "not captured in this batch", mockups are labelled, a capture sits
  next to its reference scene with its variant and real size, touched scenes come first and once, unknown touched ids
  and a failed capture, start or stop are shown, each golden labelled with its platform, a reference scene missing from
  the committed scene list (`docs/gpui/screenshot-scenes.json`) is flagged, the list's validation (and the committed
  list passing it), and the output being outside the checkout and new. Reading PNGs is checked against every row filter,
  and cropping a Linux capture's window frame against a framed picture and pictures without that frame (left uncropped
  and reported). The capture step runs against a fake `gui-agent.py`: one app run per variant with its theme, size (on
  Linux, larger by the frame, cropped back to the variant's size) and lock, `stop` after every `start` that made a run
  directory (also a failed or interrupted one), a failed start (also with no app scenes), a failed or timed-out step, a
  failed `stop` or a golden no platform has failing the batch, every platform's goldens copied wherever the batch runs,
  and run directories removed when clean and kept when not, and scenes with their own `start` options getting an app run
  of their own while the rest share the list's `app_start`. Seconds; needs neither the app nor the reference set. The
  batch itself is a tool, not a gate. Run the tests when the script or the scene list changes.
- `python3 scripts/record-test-run.py --kind development --selection 'GPUI app end-to-end smoke' --concurrency 'one app run, steps in order' --termination-grace 30 -- python3 -B scripts/gui-e2e-smoke.py`
  — the GPUI app end to end (`scripts/gui-e2e-smoke.py`): the real app, its embedded helm and a real supervisor with
  three basic fake agents, driven through the bridge by `scripts/gui-agent.py`. In one app run, in order: the list shows
  every seeded session; a click on a row opens its terminal, which shows the fake agent's ready line and echoes a
  stamped line typed into it; a row's actions button opens its menu and Escape closes it; the menu's Delete asks, and
  Confirm delete removes the session from the helm's listing and its row from the window; the menu's Rename opens the
  rename dialog, and a saved title reaches the helm's listing and the row; on a session of its own, a paste of over a
  megabyte that the receiving program put on the clipboard (an OSC 52 write) reaches it whole between one pair of
  bracketed-paste markers, with an end marker embedded in it removed rather than ending the paste early (on a Mac only:
  the step is skipped on Linux, where headless Mutter never lets the app own the clipboard, so a Linux run is no
  evidence for paste); and on another session of its own, text the bridge commits the way an input method does
  (non-ASCII) reaches the program and comes back echoed. A failed run keeps its run directory under `/tmp` for its logs.
  An interrupted one (Ctrl-C, or SIGTERM from the recorder) still stops its app run, for which the recorder's
  `--termination-grace` leaves it time. Under 20 seconds on a build worker with the app already built (4 and 18 s
  measured); it builds the app first unless given `--no-build`. Needs what `gui-agent.py start` needs (`mutter`,
  `dbus-run-session` and lavapipe on Linux, or a logged-in GUI session of the same user on a Mac). CI does not run it.
  Run it when the app's session list, row menu, Delete, Rename, terminal attach, paste or text input, the bridge's
  wiring, the app's startup or `scripts/gui-agent.py` change.
- `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh` — the curl-installer is POSIX
  `sh` run unread on a fresh machine (`curl | sh`), so a syntax slip or a shellcheck-catchable bug in it gets caught
  before the slower Rust-side asset-table parity test in `assets.rs` would notice. `test-install-sh.sh` is bash, not
  POSIX `sh` (it is never piped into a stranger's shell, so nothing forces the same portability constraint), which is
  why only `install.sh` goes through `sh -n`.
- `cargo build -p farhelm && bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` — drives the macOS-only
  installer as a real child against a fixture HTTP server, using a Darwin/arm64 `uname` shim on Linux and BusyBox. The
  installer hands its locked work to the release's own `farhelm`, so every fake release's `farhelm` is a wrapper that
  runs this tree's real binary (required; CI builds it static with musl so the Alpine/BusyBox run can use it too).
  Covers fresh install, upgrade-floor refusal of legacy layouts with their files preserved and the dedicated status
  passed through the shell, updates side by side and interrupted updates finished by a Terminal run or an older app's
  updater (the write order stopped after each step is covered by the Rust module tests of
  `farhelm_supervisor::installation::install`), the forwarder, 404, checksums, malformed archives, version
  normalization, missing prerequisites, exact closing messages, terminal styling/progress and plain redirected output,
  app layout and ownership/recovery. Linux refuses before downloading or writing; old releases (without app resources,
  or from before the side-by-side layout) refuse before anything changes. Nothing outside `~/.local/bin` and
  `~/Applications/Farhelm.app` changes, and no service manager runs. Every invocation uses `env -i` with an explicit
  environment, never the harness process's own. Refused `/latest` cases attribute requests by their scenario's origin
  path and report unexpected request lines, so delayed requests from another fixture cannot silently become this
  install's request count.
- `python3 scripts/record-test-run.py --kind development --selection 'installed uninstall acceptance' --concurrency 'one fixture at a time' -- python3 scripts/test-uninstall.py --binary target/debug/farhelm --installer scripts/install.sh`
  — macOS-only, after building the CLI: runs the actual installer and installed uninstaller against private fixture
  homes and release archives. Covers updates, legacy upgrade-floor refusal with the old installation preserved,
  confirmation, dry-run preservation, foreign files, native bundles and uninstall through an indirect Terminal link.
  Other platforms report a skip; Linux no longer has end-to-end coverage of installer-made installations because the
  installer refuses Linux. The focused macOS CI job and release gate also run
  `cargo nextest run -p farhelm --bin farhelm -E 'test(uninstall::)'`, the version-lock, installation-lock and session
  keeper tests
  (`cargo nextest run -p farhelm-supervisor --lib -E 'test(/installation::|version_lock::|session_hold::/)'` and
  `cargo nextest run -p farhelm --test session_hold --test version_lock_forwarder`), each through the recorder with four
  slots. The focused macOS CI job (not the release gate) also runs the desktop app's supervisor record and stop tests
  the same way, with the stop's check that a process has finished exiting, which on macOS rests on the zombie state
  alone:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/managed_supervisor::|procs::tests::finished_exiting/)'` (the
  uninstall stop's own tests are part of `uninstall::` above). After the uninstall acceptance, in a step of its own that
  runs whatever the steps before it did (so that a host failure cannot hide that gate), it runs the terminal host's
  whole test suite, `cargo nextest run -p terminal-host`, the host's entry test in the CLI,
  `cargo nextest run -p farhelm --test terminal_host_entry`, and the supervisor's spawning, its client, its host
  terminal runtime, discovery, the test-state cleanup and the command that stops a state directory's hosts (the
  `terminal host spawning, client, discovery and cleanup` selection above with `--test terminal_host_runtime` and
  `| binary(terminal_host_runtime)` added).
- `scripts/test-provision-centos.sh` — the x86_64 Linux release gate boots a systemd CentOS Stream 9 container and makes
  the helm provision it over ssh, which is the only coverage of a helm installing onto a distribution other than its
  own. Needs docker and `musl-tools`: the payload it pushes is the release's musl-static `farhelm`, because the
  workspace's glibc debug binary cannot exec on CentOS 9. A few minutes, most of it the musl build; the container image
  is cached after the first run.
- `bash -n scripts/devbuild-mac.sh && bash -n scripts/test-provision-centos.sh` and
  `shellcheck scripts/devbuild-mac.sh scripts/test-provision-centos.sh` — syntax and shell checks for the build scripts
  that add purpose-specific Rust targets. Target selection uses the checkout's pin even when invoked from elsewhere;
  devbuild's printed install commands name that toolchain too. Run when either script changes.
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
- `python3 scripts/install-pinned-cargo-deny.py`, then `<printed directory>/cargo-deny --locked check licenses sources`
  from the checkout root — the dependency policy in `deny.toml`: every crate in the graph (every workspace feature and
  dev-dependencies included) has a license expression its allowlist satisfies, so no GPL, AGPL or LGPL-only crate gets
  in, and every crate comes from crates.io, never a git repository or another registry. It reads crates' declared
  licenses only; files a crate bundles under other terms need their own look. CI's `deny` job runs the same command with
  the directory on PATH. Seconds plus `cargo metadata`'s first download of the crates, nothing compiled. Run it when a
  manifest, `Cargo.lock` or `deny.toml` changes. The advisories and bans checks are deliberately not part of it
  (`deny.toml`'s header says why), so do not run a bare `cargo deny check` as this gate.
- `python3 -B scripts/dependency-notices.py --check` and `python3 -B scripts/test-dependency-notices.py` — the
  dependency license notices `farhelm licenses` prints (`crates/farhelm/notices/dependencies.txt`; the maintainer
  decided on 2026-11-13 that every shipped binary carries them) still match the locked graph: the pinned cargo-about
  (installed by `scripts/install-pinned-cargo-about.py`) runs offline for `farhelm` and `farhelm-desktop` over every
  release target, and any difference from the committed file fails, naming `python3 scripts/dependency-notices.py` as
  the regenerate command. It also fails when an MIT, BSD, ISC, Zlib or NCSA text has no real copyright line (cargo-about
  silently prints the SPDX template, `<year> <copyright holders>`, when it cannot match a crate's own file;
  `about.toml`'s `clarify` entries and `upstream.toml`'s checked-in upstream texts cover those crates, and
  `upstream.toml` acknowledges the few whose upstream publishes no copyright line; its `[[extra]]` entries add notices
  for code a crate compiles in under a license it does not declare, such as FreeType's FTL credit in `freetype-sys`),
  and when an `upstream.toml` entry no longer matches the graph. The self-test covers the merge, filter, render, check
  and copyright-guard logic offline, and that `about.toml`'s targets and the script's binaries match
  `dist-workspace.toml`. About 45 seconds and three minutes of CPU per run (measured 2026-11-13 on an 8-core worker: one
  cargo-about run per binary, each capped at four threads through `RAYON_NUM_THREADS`), plus one `cargo fetch --locked`,
  nothing compiled. CI's `deny` job runs both, ungated, whenever CI runs. Locally and in a landing step, run the check
  gated, `python3 -B scripts/dependency-notices.py --check --only-if-inputs-changed-since origin/main`, which skips
  cargo-about unless `Cargo.lock`, a `Cargo.toml`, `dist-workspace.toml`, `deny.toml` or a file under
  `crates/farhelm/notices/` changed; and on a machine shared with other agents, run generation and the full check on a
  separate build worker where one is available, not alongside their builds.
- `python3 -B scripts/test-install-pinned-nextest.py`, `python3 -B scripts/test-install-pinned-cargo-deny.py` and
  `python3 -B scripts/test-install-pinned-cargo-about.py` — the installers that put the pinned nextest, cargo-deny and
  cargo-about binaries in place for the release build jobs, CI's `deny` job and local runs, and the checksum
  verification and atomic publication they share (`scripts/pinned_tool.py`): archive and executable digests, archive
  shape (a sole executable, or for cargo-deny an executable at a named path among plain files), cache reuse, curl's size
  limit, each tool's platform mapping. Run them when an installer, the shared module or a pins file changes; seconds, no
  network.
- `scripts/publish-docs-shots.sh --self-test` — the docs screenshot publisher's whole validation: against a bare
  repository standing in for GitHub, it checks that refused publishes push nothing, that only `refs/docs-assets/keep` is
  ever created (never a branch), that a snapshot holds exactly the shots the docs pages reference, that partial,
  incomplete, or since-modified captures are refused, that the manifest records the capture's main commit, and that a
  snapshot stays until six weeks after it was replaced (the one main pins, always). Run it, with `shellcheck`, when the
  script changes; seconds, no network, needs ImageMagick's `convert`.
- `bash scripts/test-plans-watch.sh` — drives the plans monitor's watcher (`scripts/plans-watch.sh`) against a strict
  stub `gh` that runs the watcher's own `jq` filter over GitHub-shaped tree listings: change detection, baselines read
  from a commit, the wake check that keeps claims from waking idle executors and the role it is passed (`executor`,
  `lander`, or the landing monitor's mid-round `lander-blocks`), the idle cap, retried and persistent failures,
  malformed and truncated responses, hung requests, argument errors, and prompt exit on SIGTERM without leaving children
  behind. Run it, with `shellcheck` on both scripts, when either changes; under a minute, no network, needs `jq`.
- `python3 scripts/test-plans-queue.py` — the plans queue script (`scripts/plans-queue.py`), which is the lock that
  keeps concurrent plan executors off each other's plans: its line grammar, every state transition and refusal, the
  invariants between a state and its files, the rules a planning PR must keep, the wake check for each role (the landing
  monitor's mid-round role waking for new blocks only), the pass count that bounds how long picking can skip a plan for
  conflicts, and the compare-and-swap loop against an in-memory GitHub that injects racing writers and lost responses.
  One case runs the real `dprint` when it is on PATH, which downloads dprint's plugins if its cache is cold. Run it when
  the script changes; seconds, no other network.
- `python3 -B scripts/plans-landing/test-landing.py`, `deno check scripts/plans-landing/idle-watch.ts`,
  `deno test -A scripts/plans-landing/idle-watch.ts` and `shellcheck scripts/plans-landing/notify.sh` — the landing
  monitor's round tooling (`scripts/plans-landing/README.md`): the monitor log edits a resume relies on (Status and
  entry sharing one stamp, a stopped monitor's Status refused, the claim line replaced once or refused), the round
  files, the claim check before every push and merge (exact slug and claim id), the landing notes' placeholder and
  line-length rules, the review page's escaping, and the idle watcher's reading of `queue status` (between rounds, and
  in its mid-round `--blocks-only` mode, blocks alone) and of the origin remote. The jj and gh sequences themselves are
  exercised only by real landings. Run them when those files change; seconds, no network beyond Deno's first fetch of
  its `jsr:` imports.
- `python3 scripts/test-todo-picker.py` — TODO extraction and the legacy standalone checklist
  (`scripts/todo-picker.py`): how TODO.md buckets and entries are read, that entries with a plan are left out, that a
  summaries file written against an older TODO.md is refused, and that summaries are escaped into the page. Run it when
  the script changes; seconds, no network.
- `python3 -B scripts/test-capture-agent-screens.py` — the fixture scrubber's replacement and refusal of local proxy
  URLs with userinfo, and public links containing `@` in a path or query. Run it when the scrubber changes; seconds, no
  network or vendor turns. The recorded agent screen fixtures selection independently checks the Rust guard.
- `deno test -A scripts/serve-page.ts` — the legacy standalone static server (agent pages use the `interactive-artifact`
  skill): that it answers with `Cache-Control: no-store` and moves past a taken port. Run it, with `deno check` on the
  same file, when the script changes; seconds. Needs Deno, and fetches `jsr:@std/http` the first time.
- `deno test -A scripts/tensorlake/sandbox-tests.ts`, `deno test -A scripts/tensorlake/rust-tests.ts`,
  `deno check scripts/tensorlake/rust-tests.ts` and
  `shellcheck scripts/tensorlake/bootstrap.sh scripts/tensorlake/run-rust-shard.sh` — the Tensorlake harness's own
  checks (the Rust runner and its template builder): how the snapshot listing is read, and archive configuration
  retaining the scoped prebuilt-Podman setup while removing only the local fixture build. Run them, with `deno check`,
  when the harness changes; seconds, no sandbox. Using the harness itself is described in `.agents/tensorlake.md`.
- `python3 scripts/record-test-run.py --kind development --selection 'portable brick-test harness' --concurrency 'one unittest process' -- python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`
  — validates owned Tart lifecycle, signed-input planning, Rust/Python upgrade-floor agreement and fixed-floor baseline
  selection, guest execution, fixed command operations, durable handoffs, compiled checkpoint contracts,
  fresh-versus-update installer entrypoints, fixed command-only execution and provenance, bounded pre-action provider
  recovery with retained failures and no uncertain-action replay, structured native actions, durable completed-update
  evidence without transient progress images, exact native update-channel readouts without mandatory tooltip images,
  normal Finder readiness with verified menu dismissal, conditional product confirmations, remote menu Remove/re-add and
  Uninstall sequencing, independent resource-preservation/removal receipts, restart process handoffs, calibrated window
  measurements, gesture-trace and bonus gesture-readiness gates, failure gates and evidence import with fictional
  fixtures. Use a substrate that permits process-group inspection; Linux continuity cases skip on macOS. These tests
  prove harness contracts, not native GUI behavior or a release pass. Actual native fixture probes and release checks
  retain their own private evidence. The guest-only Swift geometry helper needs separate SDK type-check and read-only
  runtime validation in an owned disposable guest; the portable suite does not exercise that OS API.
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

This inventory names the relevant local checks. The CI workflow (`ci.yml`) covers formatter, Clippy, the desktop app's
lints, the OMP reporter asset scenarios, the website build and its feedback endpoint tests, installer validation, the
dependency license and source policy, generated-workflow validation, and the agent-instructions check; costly Rust and
CentOS gates run in the x86_64 Linux release artifact job. If either workflow changes, update this list in the same
change.

The CI workflow runs ONLY on demand: it has no push or pull-request trigger (removed 2026-09-12), so neither a PR, a
`gh pr ready`, nor a merge to main starts a run. The release build gate is the validation that decides whether a build
ships; the local checks above are what validates a change before it lands. The focused headless macOS uninstall suite
runs with `gh workflow run ci.yml --ref <branch> -f suite=uninstall-macos`. `gh workflow run ci.yml --ref <branch>` runs
the whole baseline for any ref when a hosted verdict is wanted. PRs are still opened as drafts and marked ready when the
user asks to publish them, or as part of landing them, never on the agent's own initiative.

The RELEASE workflow is not on that list: it is generated, runs on tag pushes only, and carries its own gate, which is
not something to run locally. `releasing/.agents/workflow.md` describes it, the parts of it that CAN be checked locally
(the changelog checks above among them), and the unsigned-release recovery procedure.
