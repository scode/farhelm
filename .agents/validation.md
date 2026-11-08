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
  modules share two, the two-host fixture reserves two, and RSS reserves all four. The release gate runs this same
  selection, including the farhelm integration targets, after preparing rootless Podman and its offline Ubuntu image. It
  narrows every later Rust step with a filter rather than `-p`, so those steps share one build; ordinary CI does not run
  this suite. Do not also run the old full libtest battery. The e2e suite's in-process harness and its real supervisor
  process fixture create new terminals unless a test asks for old ones by name (`harness_tmux`,
  `supervisor_process_tmux`, `SupervisorSeams::tmux_terminals()`), as the supervisor's own unit tests that need a real
  terminal do; those, and the tests of tmux itself, are what the pinned tmux is for. On Linux it needs Python 3 at
  `/usr/bin/python3`: besides the e2e stand-ins named below, the supervisor's `managed_supervisor::` stop test runs a
  Python stand-in whose main thread exits before its other thread.
- `python3 scripts/record-test-run.py --kind development --selection 'workspace doctests' --concurrency '4 doctest threads' --tmux required -- cargo test --locked --doc --workspace --exclude farhelm-desktop -- --show-output --test-threads=4`
  — preserves doctest coverage separately because nextest does not execute it.
- `scripts/test-tmux-pinned-shutdown.sh` — the x86_64 Linux release gate builds the exact checksummed release named by
  `.github/release/source-pins.env` on cache miss, then runs every focused output-client teardown regression in its own
  test process with its own recorded nextest report. This explicit release subset remains beside the full workspace
  selection. A successful local workspace nextest run already covers these tests on the same pinned substrate; do not
  duplicate them merely because this script is listed. A distro tmux loses the affected-version coverage.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'terminal host' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p terminal-host`
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
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'terminal host spawning, client, discovery and cleanup' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-supervisor -p farhelm-teststate -p farhelm --lib --test terminal_host_spawn --test terminal_host_client --test terminal_host_stop --test terminal_host_registry -E 'test(/^hosts::/) | binary(terminal_host_spawn) | binary(terminal_host_client) | binary(terminal_host_stop) | binary(terminal_host_registry)'`
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
  in-process harness and its real supervisor process ends a host started in their state directories. The browser stack's
  cleanup test, `scripts/test-start-stack-cleanup.sh` (by hand, after `cargo build` and the web build, a few minutes;
  run it when `e2e/start-stack.sh`'s cleanup changes), plants a real host in both state directories of every stack it
  boots and requires the stack's cleanup to end them, while a host in another stack's live run directory (in the sweep's
  family, its lock held and backdated past the sweep's grace) survives every phase and a directly observed sweep pass,
  which must reap a dead canary run beside it.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'host terminal runtime' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test terminal_host_runtime -E 'test(/^hosts::terminal::/) | binary(terminal_host_runtime)'`
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
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'host terminal switch points' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/core::tests::host_terminals::/) | test(/a_failed_tmux_probe_still_samples_and_observes_host_terminals/) | test(/^service::session_hosts::/) | test(sweep_launch_dir_keeps_every_host_record) | test(/^host_terminals::/)'`
  — the supervisor's terminal switch points with a terminal host behind a session's agent (`Terminal::Host` in
  `crates/farhelm-supervisor/src/service/terminals.rs`). The unit tests (`service::core::tests::host_terminals`, and one
  in `service::ticker::tests`) use a stand-in host in the test process (`hosts::terminal::test_host`) whose child is a
  `sleep` the stand-in owns: status and the recorded exit code, the never-started excerpt from the emulator, Stop before
  the host's exit record (a plain exit, then the launch failure once the record arrives) and Stop classifying from a
  record that arrives while it waits, the wait for that record pending until it arrives, the launch shim's own report
  recorded without waiting, an unreadable host left alone (no transition even beside a current sentinel, unknown status,
  a root that its earlier probe authorized refused, refused verbs and prompt), the ticker recording a host exit on the
  liveness notifier's wake with the tick far away, a failed tmux probe that still samples a live host and records a gone
  one's exit, the hook anchor (and the publication gap's retry), the orderly stop's hold wait, prompt delivery gated on
  the emulator's paste mode, at one part and at the prompt cap's two, and grid attaches over the supervisor's own
  connection handling (a bytes-only attach refused, a grid attach catching up and typing into the host, a takeover
  displacing a grid attachment, a paused grid viewer detached as stalled, an attach to a startup placeholder refused
  with an incumbent kept). Delete's handling of hosts (`service::session_hosts`) against stand-ins, some in the state
  directory's socket directory named by a launch record the test writes: an unconditional Delete of an unreadable agent
  host killing its child from the identity pinned at adoption and keeping the session when its adopted process cannot be
  confirmed ended (the stand-in's process is the test runner, which the verified signal never signals); a recorded host
  that answers only with a foreign protocol refused by the agent guard and signalled, never reconnected to, by an
  unconditional Delete (the stand-in counts two connections, one per settling); a record naming another session's host
  refusing and touching nothing; the record of the host the agent's runtime holds left to the runtime (the stand-in
  counts only the adoption's connection); a gone record's leftover socket file removed with the session deleted; and the
  overall deadline for settling and ending hosts (a gate holding each end before it starts, not a stuck process read,
  and a one-second budget) returning a retryable refusal and releasing the directory admission and the lifecycle claim.
  Pure unit tests cover the bounded join and the startup sweep keeping every launch host record. The e2e tests
  (`crates/farhelm/tests/e2e/host_terminals.rs`) give a created session's agent a real host the test started from the
  built `farhelm`, in place of the host its create started (`Supervisor::install_host_agent_for_test`), and check the
  exit stored with its code, Stop and Delete killing an unmarked process tree from the host's child (Stop leaving the
  host's process running with its final screen), Restart stopping that tree before its new host starts, the delete
  guard, a bytes-only attach refused and a grid attach showing the host child's output; and Delete ending hosts, every
  test's grandchild ignoring SIGHUP (it publishes its pid only after its trap; disposition and ancestry checked first)
  so that only a sweep rooted at the host's child reaches it: a host agent guarded and then ended (gone at Delete's
  return) beside a record naming the host its runtime holds, a recorded host of another generation refused by the agent
  guard beside an ended agent and then ended while another session's recorded host is never touched, a terminal-less
  session's recorded agent host, a SIGSTOPped host agent ended by the verified signal, a SIGSTOPped recorded host
  refused by the guard and then signalled, and a host renamed over the agent's socket neither shut down nor signalled
  while Delete ends the host the runtime adopted and deletes the session. In `host_creates.rs` (the selection below):
  the unbound recorded start an interrupted create leaves fenced by an unconditional Delete, and Delete of a session
  whose runtime is held in a snapshot encode ending the adopted host by its process after another host was renamed over
  its socket, which is left running. Linux only (the e2e suite), a few minutes (several tests wait out a stopped host's
  connect deadline); part of the workspace nextest run. Run it when a switch point, the host runtime's adapters, the
  status evidence, the grid attach path or Delete's handling of hosts change.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'creating agents in terminal hosts' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/core::host_launch::/) | test(/a_retry_finds_the_sessions_own_published_host/) | test(/^hosts::spawn::/) | test(/^host_creates::/)'`
  — a create starting its agent in a terminal host under the supervisor's switch (`SupervisorSeams::new_terminals`,
  `crates/farhelm-supervisor/src/service/core/host_launch.rs`; on in every shipped launch through
  `farhelm supervisor run`'s default, and in the in-process harnesses unless a test asks for old terminals). The unit
  tests pin the host child's environment as a function of its inputs (the supervisor's environment passed through,
  `TERM_PROGRAM=farhelm` with the version the emulator's XTVERSION answer names, no `TMUX`, `TMUX_PANE` or `LISTEN_*`,
  where the launch environment applies, the default `PATH`), the launch record's encoding and its refusals, a first
  stage started apart from its spec (pinned, given nothing until finished, killed having read nothing when dropped, a
  taken name refused), a retry that finds the session's own published host with no other evidence. The e2e tests
  (`crates/farhelm/tests/e2e/host_creates.rs`) run in-process supervisors with the switch on, starting hosts from the
  built `farhelm`: the row recorded as a host row with the socket's name, no tmux session, Stop ending the agent and
  sparing the host; the create reply, its keyed replay and the listing naming the agent's terminal as a host and
  reporting the switch on (what the client's migration band and session details read); the host's command line and empty
  environment holding no credential or marker while its child has the markers; the agent in its launch scope with the
  host outside it (with a user manager, else `SKIPPED`); the login shell's environment (Farhelm's terminal variables,
  the supervisor's `PATH`, the launch environment's `HOME`, no `TMUX` or `TMUX_PANE`) and a startup file's tmux
  autostart guarded by `TMUX` running; a wrapper prefix running in the host; a create interrupted after its host started
  retried without a second agent; a first stage that outlived an interrupted create (holding its spec, before binding)
  waited for by the retry, which then finds its host, or ended by the verified signal when it does not finish, with one
  host and one agent either way, and fenced the same way by an unconditional Delete of that session after a restart,
  which then leaves no host and no agent; a process the interrupted first stage forked that still names the socket (a
  stand-in second stage) leaving the retry unresolved while it stays, and letting the same retry relaunch when it ends
  during the retry's wait; a launch error's cleanup after a restart keeping the host record of a host that still runs; a
  cleanup whose runtime is held by a snapshot encode (an encode gate) falling back to ending the host by its socket
  after the runtime's 15 s bound, while the encode is still held (the gate shows no encode passed or expired); a create
  that lost to Delete ending its host; a start whose first stage dies after readiness leaving nothing, and one the spawn
  reports may have left a host, which the create ends and removes the socket of; stand-in hosts that cannot be used,
  ended by the verified signal and rolled back, or kept as a launching record when the signal must not touch them; and a
  retry keeping its row's kind with the switch flipped either way across a restart. Needs the pinned tmux first on
  `PATH` and Python 3 at `/usr/bin/python3` (the stand-in hosts). Linux only, two or three minutes (several tests wait
  out client deadlines); part of the workspace nextest run. The container launch kind's test is in the containerized
  suite above (`containerized_sessions::podman_agent_runs_from_a_terminal_host`). Run it when the create path, the
  launch spec's composition, the host spawn or the launch identity's rollback or evidence changes.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'restarting host agents' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/host_handover::tests::/) | test(a_terminal_less_host_row_restarts_in_a_host_never_in_tmux) | test(/a_restart_seed_|_seed_of_an?_|a_seed_whose_runtime|a_stop_during_a_restarts_seed|a_restart_without_consent_touches_no_program|a_restart_asks_again_for_consent/) | test(a_failed_host_restart_after_a_stop_began_restores_a_placeholder) | test(restart_of_a_host_agent_stops_its_tree_before_a_new_host_starts) | test(/^host_relaunches::/)'`
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
  supervisors with the switch on and real hosts started from the built `farhelm`: a Restart starting a new host at the
  old size that keeps the old history (and none of the old screen) with default modes, ending the old host and removing
  its launch record; Restart stopping a running host agent's tree, with a grandchild that ignores SIGHUP, before the new
  host's start begins, as that start records (in `host_terminals.rs`); a second Restart fencing the first stage an
  interrupted one left holding its spec, so that releasing it afterwards starts no second agent; a host agent's tab, its
  host, shell and launch record surviving a Restart; an orderly stop during a Restart's window (held at its gate)
  closing the session's host tab runtime, with the Restart, succeeding or failing, bringing none back connected; a
  wedged host a confirmed restart replaced not holding a later Restart up; a replacement whose first stage refuses
  unwound definitively, the old host kept as the session's terminal for that supervisor and adopted by the next one
  through the row's `terminal_address`, and a later Restart moving on from it; an unusable replacement host ended by the
  verified signal and the restart unwound; one that cannot be confirmed ended keeping the launching record with no
  terminal and the old runtime stopped, and Delete then ending both; a failed confirmation write publishing the new host
  and still ending the old one; a grid viewer detached as "session restarted" and reattaching to the new host's screen
  and history; the seed able to hold what the old program printed as it was stopped; a crash (the fault seam) right
  after the generation claim, and one after a failed replacement was cleaned up, each leaving the old host the session's
  terminal at the next start, with its history kept by the next Restart; with the newest earlier host unreadable at that
  start, no older host taken, and the newest taken at the start after; with the newest earlier host dead, the next older
  one taken with its history; a restart interrupted after its new host started, and one after its confirmation, each
  converging at the next start (the new host adopted and confirmed, the old host ended and its record removed); an
  earlier host stopped with SIGSTOP left alone by the next start and ended by the one after; a restart whose row is
  deleted through the store mid-launch ending both hosts; and Delete after an interrupted restart ending both
  generations' hosts. Needs the pinned tmux first on `PATH` and Python 3 at `/usr/bin/python3` (the unusable host's
  stand-in). Linux only (the e2e suite), about three minutes; part of the workspace nextest run. Run it when Restart's
  host path, the host spawn, the runtime's seed, reload's host attribution or the superseded rule changes.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'moving old sessions into terminal hosts' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(a_relaunch_into_a_host_writes_the_kind_and_an_abort_restores_tmux) | test(/^tmux::tests::pane_history_/) | test(/^host_migrations::/)'`
  — a Restart that moves a tmux session's agent into a terminal host under the internal switch
  (`crates/farhelm-supervisor/src/service/core/host_restart.rs`, "From a tmux pane"). The unit tests pin the store's
  relaunch writing the new launch's kind with its generation and an abort restoring the tmux kind with the pane, and,
  against a private tmux server, the history capture the seed is made from: the rows above the screen alone (never the
  visible screen), styled, wrapped rows joined, at the pane's size; an empty history giving none, a capture over its
  byte bound keeping the newest whole lines and saying it was cut, and a pane named under another session told apart
  from a failed read and a capture out of time; a pane resized between its two reads read again at its new size, and one
  resized on every read failing after three; only the newest rows asked for taken, starting from the default rendition
  (a style set before them written again); a 5,000-column pane's history of short lines taken only as far as a seed's
  restore budget at that width; and a capture seeded and restored as the move does it, keeping line drawing across a
  line end, a wide character wrapped at the last column, a hyperlink on its first row only, and only the history part of
  a row that wraps into the screen. The e2e tests (`crates/farhelm/tests/e2e/host_migrations.rs`) run in-process
  supervisors on one state directory, the first with the switch off making the old session in the real private tmux
  server and the next with it on, starting hosts from the built `farhelm` (an agent with distinct start and resume
  commands that records each launch and the conversation it resumed): the move seeding the new host with the tmux pane's
  history and nothing of its last screen, resuming the recorded conversation exactly once, the row becoming a running
  host row, the old agent window removed while a tmux tab stays alive, listed first and typed into through a client's
  attachment, a new tab opening as a host, and a takeover displacing a client's host agent, tmux tab and host tab
  attachments together; a Restart with the switch off staying in tmux; a refused new host leaving the tmux session as it
  was (row, pane, window and history), retried successfully; a history read that fails refusing the move with the tmux
  session kept and listed as stopped, and the next supervisor moving it; a capture out of time tried again for fewer
  rows, refused after a second timeout with the way out named, and moving the session when the retry finishes; a pane
  resized past what a seed can hold moved without a seed and its old window kept; the old window's removal failing with
  the move still succeeding; crashes right after the generation claim (no terminal, a later Restart starting empty, the
  dead old window left to Delete), after the new host started and before the old window's removal (the new host adopted
  and confirmed at the next start); a move whose row is deleted mid-launch ending its new host; an orderly stop during
  the seed (held at a gate) refusing the move at once with the tmux row unchanged; a moved agent outliving its tmux
  session when the last tmux tab closes (observed once the session's output client is reaped), through an observation
  pass, a supervisor restart, Stop and Restart; and Clone and Replace of an old session, created from its own launch,
  making host sessions. Needs the pinned tmux first on `PATH`. Linux only (the e2e suite), a few minutes; part of the
  workspace nextest run. Run it when Restart's target choice, the tmux history capture, the store's relaunch kind or the
  restart's host path changes.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'host terminals across supervisor restarts' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/a_prompt_waits_for_a_recovered_host_terminal_to_catch_up|spare_claims_read_a_host_rows_own_host|a_stop_request_survives_a_restart|an_unreadable_launch_record_leaves_its_session_unavailable|an_orderly_stop_ends_its_encodes_before_its_runtime_drops|a_readoption_that_loses_its_fence|readoption_|a_scan_restricted_to_some_names|delete_settles_the_record_of_a_startup_placeholders_socket/) | test(/^host_restarts::/)'`
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
  (`crates/farhelm/tests/e2e/host_restarts.rs`) run in-process supervisors with the switch on and real hosts started
  from the built `farhelm`, dropped and constructed again on one state directory: an orderly stop leaving each host a
  snapshot where its terminal is and ending its runtime, after which the next supervisor publishes the same host,
  restored from that snapshot, with the same screen and a `Running` row, keeps that very runtime through `serve`'s
  second reload, and drives it; a crash (no orderly stop) recovered from a snapshot plus output seen before the crash
  and output written while no supervisor was attached; an agent that exits with code 7 while no supervisor runs,
  recorded with its code once replayed; a host killed while none runs, its session recorded as exited with no terminal;
  a create interrupted before its confirmation found by its host's tag and confirmed, and one whose login shell failed
  before the launch step ending as a launch error once its exit is replayed; a stop whose checkpoint ran out of time
  (its encode held) leaving the previous snapshot, from which the next supervisor recovers with the tail; a create
  published after the stop began gathering, on a runtime the test owns, publishing a placeholder, its runtime stopped
  and its held encode cancelled, and the runtime dropping at once; and a host stopped with SIGSTOP left alone
  (unavailable, listed unknown, nothing recorded, Stop refused for the unavailable host, the host and its agent still
  running) and adopted by the same supervisor once continued. Needs the pinned tmux first on `PATH`. Linux only (the e2e
  suite), a minute or so; part of the workspace nextest run. Run it when reload, the orderly stop, re-adoption,
  discovery or the host runtime's adoption or encodes change. The `readoption_` tests also cover recovery after a
  runtime ran: timeout recovery on the ticker's liveness wake, every unstable episode's full reconnect window, retry
  counts kept across success with no healthy-key spin, final refusal reasons, the stop fence, and an exited runtime
  replayed without recording its exit twice; tabs recovering in place with their ordering and scope, startup
  placeholders learning their tags, retry counts kept through an agent Restart, failed closes left to the reap, gone
  tabs closed with their records removed and adopted process kept, and cell/stop fences. The encoder's cancel itself is
  in the terminal emulator selection below (`an_encode_stops_once_its_cancel_fires_and_is_unchanged_until_then`).
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'terminal tabs in terminal hosts' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(launch_file_names_round_trip_through_the_parser) | test(sweep_launch_dir_keeps_every_host_record) | test(/core::host_tabs::tests::/) | test(/tmux::control_codec::tests::/) | test(closing_a_tab_whose_runtime_reads_gone_confirms_its_process_first) | test(/^host_tabs::/)'`
  — terminal tabs in terminal hosts under the supervisor's switch (`--new-terminals host`)
  (`crates/farhelm-supervisor/src/service/core/host_tabs.rs`). The unit tests pin a tab host's launch record name
  (`<id>.tab-<tab id>.host`) round-tripping through the launch-directory name parser, which accepts the tab form only
  for a `.host` record; the startup sweep keeping tab records; the tab strip's creation order against stand-in hosts (an
  exited tab hidden and picked by the reap, unreadable tabs kept and never reaped, a tab whose close failed reaped
  again, once its retry time comes, which backs off from a second to a minute) and a new tab stamped past the session's
  newest; a close whose tab runtime reads its host gone while the adopted process runs (a stand-in) keeping the tab, its
  record and its retry mark; and the pane-fact query's tab flag, which makes tmux's marker query run whenever a window
  carries a tab marker, a lone one at index 0 included, and only then for single-window sessions. The e2e tests
  (`crates/farhelm/tests/e2e/host_tabs.rs`) run in-process supervisors with the switch on and `/bin/sh` tab shells,
  starting hosts from the built `farhelm`: a tab beside a host agent opened at the agent's size, its record naming its
  socket, typed into, attached as a grid, a takeover displacing the previous client's agent and tab attachments
  together, and closed leaving no host, socket, record or tab process; a tmux agent's session (made with the switch off,
  served after a restart with it on) listing its tmux tab before its host tabs in opening order, each host tab at the
  tmux window's size, a takeover across the tmux agent and a host tab, and its Delete refused for three open tabs by a
  guard, then ending both tab hosts, their sockets and records and the tmux session while another session's tab host
  runs on; a host agent whose last old tmux tab closes (tmux ends its tmux session) with a host tab already open, both
  still running and a further host tab opening; a lone tmux tab at index 0 listed and closed; a tab shell dead at open
  (the settle held until its pid is gone) refused with its last words, leaving no tab host process, socket or record;
  opens failed just before publishing, and (with the user manager, else `SKIPPED`) after the host started and before its
  adoption, leaving nothing at the refusal's return, the latter killing a SIGHUP-immune job of the tab's shell and its
  tab scope; a SIGHUP-immune job with no markers under a tab's shell killed by its close and by Delete; a tab start
  interrupted with its first stage holding the spec settled by the next supervisor's reload (which waits for it) and its
  host adopted, then ended by Delete; `serve`'s reload keeping held tab runtimes without contacting their hosts while it
  scans for another session's agent; a tab host stopped across a restart kept as a listed placeholder whose close and
  guarded Delete are refused and which an unconditional Delete ends, and a tab runtime made unreadable (its reconnect
  limit) treated the same way; an exited tab, and a tab whose host was killed, reaped with a ten-minute ticker interval;
  a close whose host end fails (a seam) finished by the reap, no sooner than its one-second backoff; and an orderly
  restart keeping a tab listed, at the same socket, with its screen, attachable as a grid and typed into. Needs the
  pinned tmux first on `PATH`. Linux only (the e2e suite), a minute or two; part of the workspace nextest run. Run it
  when tab opening, closing, the tab strip, the tab reap, the tab records, Delete's handling of tab runtimes, the
  reload's tab adoption or tmux's pane listing change.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'reclaiming orphaned terminal hosts' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(known_session_ids_are_every_rows_and_every_reservations) | test(/^host_reclaim::/)'`
  — the startup reclaim of orphaned terminal hosts (`crates/farhelm-supervisor/src/service/core/host_reclaim.rs`). The
  store's unit test pins which sessions the database knows (every row's and every reservation's, a reservation whose row
  was rolled back included, a session deleted with its reservations not). The e2e tests
  (`crates/farhelm/tests/e2e/host_reclaim.rs`) run in-process supervisors with the switch on and real hosts started from
  the built `farhelm`, and check the state once `serve` accepts: a session missing from the database whose launch
  records remain keeping its agent and tab hosts running with their records, while its killed tab's record and socket
  file go and another session's adopted runtime is never taken over; a database removed, then a new session made in its
  replacement, then a restart, leaving the old hosts running while an unrecorded orphan nobody knows is ended at the
  same start; unrecorded hosts tagged for sessions nobody knows (agent and tab) and an unrecorded tab of a known session
  ended, the known session's agent program running on, and a killed host's socket file removed; with both sessions'
  launch records removed so that the database alone knows them, a host whose session has only a pending reservation and
  an unrecorded agent host of a session known only by its row (at a generation it never had) left running, as is a
  SIGSTOPped host of an unknown session, while a readable orphan beside them is ended; hosts whose snapshot does not
  restore and a host with a foreign raw tag left running beside an ordinary orphan that is ended; a `hosts` symlink
  keeping a record and the stale socket file it names in another directory; a reclaim whose discovery is held past a
  one-second budget (a fault seam and `SupervisorTimeouts::orphan_reclaim`) ending nothing, and the next start ending
  the orphan; a missing and then an empty database reclaiming nothing; a launch directory whose listing fails at its
  open, and one that fails partway after counting a record (a fault seam), reclaiming nothing, a session known only by
  its records keeping every host, socket and record, until a whole listing ends the unrecorded orphan beside them; a
  startup placeholder's socket never connected to by `serve` (a stand-in counting the connections it accepts); an
  interrupted start of a session missing from the database settled (its first stage released by the settle's wait hook)
  and the host it brings up left with its record, and one whose first stage never finishes fenced and its record
  cleared; and, with a ten-minute ticker interval, an existing session's tab whose shell was killed while no supervisor
  ran reaped, and a tab whose host was killed meanwhile leaving neither record nor socket file. The e2e harness's wait
  for a served supervisor's socket allows the reclaim's whole budget on top of its own 20 seconds. Needs the pinned tmux
  first on `PATH`. Linux only (the e2e suite), about a minute with four slots (stale sockets, a stopped host and a
  fenced first stage wait out their deadlines); part of the workspace nextest run. Run it when the reclaim, discovery,
  the launch records' settling, a host runtime's adoption, `serve`'s startup order or the store's reservations change.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'host terminals under a real supervisor process' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/^host_processes::/) | test(a_startup_terminal_kind_is_the_switch_the_supervisor_runs_with) | test(the_schema_33_marker_changes_no_table_and_keeps_every_row) | test(/^store::tests::schema_34_/)'`
  — terminal hosts the way a real supervisor process creates them: `farhelm supervisor run`, whose hidden
  `--new-terminals` defaults to `host` (`SupervisorStartup::new_terminals`). The unit tests check that the startup value
  is the switch the supervisor runs with (`Supervisor::new_terminals`), that schema 33, the marker that makes every
  build at 32 refuse a database such a supervisor opened, changes no table or row and is not taken by a process without
  the state directory's claim, and that schema 34, which drops the stored tmux session names and renames `pane` to
  `terminal_address`, keeps every session field for field (a tmux pane, an unconfirmed launch, a host socket), both
  kinds of reservation and the notification trigger, matches a fresh database, and refuses, leaving the database at 33,
  one whose stored tmux name of a session or pending reservation is not `fh-` and its session id (a settled
  reservation's does not stop it); the `farhelm` binary's own unit test
  (`supervisor_run_defaults_to_terminal_hosts_and_can_still_ask_for_tmux`, part of the workspace nextest run, or alone
  as
  `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'supervisor run default' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm --bin farhelm -E 'test(supervisor_run_defaults)'`)
  pins the flag's default and its `tmux` spelling. The e2e tests (`crates/farhelm/tests/e2e/host_processes.rs`) start
  the built `farhelm` as processes and look only through what a viewer sees (a grid attachment's screen and the input it
  sends), the host's process by its command line, and the session's row: a bare `farhelm supervisor run` creates a
  session as a host row whose agent and a newly opened tab both attach as grid streams; the suite's process fixture for
  tests written against tmux (`harness::supervisor_process_tmux`, which passes `--new-terminals tmux`; plain
  `supervisor_process` runs the bare command) still creates a tmux row whose agent is ready in the private tmux server
  and which answers a grid-capable viewer with bytes; a session created in tmux is, after an orderly stop and a bare
  start on the same state directory, still served from tmux (a byte stream) and listed as an old terminal on a
  supervisor that creates new ones, while a session created then is a host; a session created with
  `--new-terminals host` is a host row; its host agent comes through an orderly restart (SIGTERM, a clean exit) and a
  SIGKILL of the supervisor process with the same host process, its screen (the ready line and a line echoed before) and
  its input; a state directory whose `supervisor.sock` path is 103 bytes, macOS's longest, holds its host sockets (each
  path shorter than the supervisor's) and the terminal works; and, with a real helm on the same state directory, a
  session created through the helm's API reaches a hand-rolled browser WebSocket with `?grid=1` as a `grid` stream
  notice and then grid messages showing the agent's screen, and `text` and `key` messages sent up the socket reach the
  program; a bare supervisor given a recording `--tmux` running no tmux at all through a host session's create, attach,
  tab, listing, rename, several ticker intervals, Delete and SIGTERM, with no tmux socket left; an old session's private
  server adopted by a bare supervisor, still serving it, and ending when it is deleted; and an old session listed as
  exited with no terminal by a supervisor whose `--tmux` does not exist, which still creates host sessions and leaves
  the server running. Needs the pinned tmux first on `PATH`. Linux only (the e2e suite), a minute or two; part of the
  workspace nextest run. Run it when the flag, the startup's wiring of it, the schema, the orderly stop, reload, the
  host socket's path, the helm's grid relay, or when and whether the supervisor uses tmux changes.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'grid streams' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-supervisor --lib -E 'test(/^grid_output::/) | test(/^service::grid_stream::/)'`
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
  `FARHELM_GRID_MEASURE_OUT` naming a directory for their messages (passed with `--keep-farhelm-env`); they assert
  nothing.
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
  network: Farhelm's query-answer table, side effects by feed mode (live, replay of seen and of unseen output), the
  synchronized-output deadline on a hand-stepped clock (and its flush before a resize), the snapshot split candidate on
  agent-shaped reads, a property test that whole, byte-at-a-time, chunked and candidate-split feeding end in the same
  state once any open update is flushed, the handler's forwarding as a property test against a bare
  `alacritty_terminal`, Farhelm's screen-text format and titles (tabs, modes 47 and 1047), the mode accessors (resetting
  any mouse tracking mode ends reporting), the browser theme's colors, one-column terminals with wide characters and
  resizes (each bounded to 10 seconds, so that a regression of the vendored reflow fix fails after about 1 GB of
  allocation instead of running until memory runs out), a screen full of tabs captured in linear time, and hostile
  output through the vendored caps (combining floods, 8 MB OSC 52 and titles, long hyperlinks, floods of hyperlinks and
  combining marks against the per-terminal extra storage, a synchronized update past vte's 2 MiB buffer). The terminal
  snapshot format: the round trip (a recording fed in reads, snapshots where the emulator is clean after a read's split
  candidate or end, restore, then compare field by field with the uninterrupted emulator after the rest of the output in
  the same reads, after probes, and after the rest in other reads and a final flush) as a property test over generated
  output, over a one-column terminal and over two short synthetic recordings, and over 20,000 short scripts of awkward
  output and resizes (from 8x4 and two columns, each continued after the restore); the review reproducers (a one-column
  encoder panic, four round-trip losses); the version 1 golden fixtures (`tests/fixtures/snapshots/v1/`: each blob
  restores to its recorded state, each script still encodes to its blob and reaches its feature); hand-built version 1
  blobs and refusals of malformed, truncated, oversized (before allocating) and unknown-version blobs, of streams
  holding sequences version 1 never writes or taking too much work to replay (before feeding), and of fixups no live
  output makes; history past the restore's cell bound dropped and marked trimmed; mutated and crafted blobs never
  panicking; extras shared by content through a restore, within the extra storage limit; the size ladder (titles before
  history); the trimmed mark across restores; an encode's cancel (nothing once it fires, the same bytes until then); the
  character width table's fingerprint; the Restart seeds, and a seed's encode stopping at whichever look its cancel
  fires on, and a seed from a tmux pane's capture cut short marked trimmed, its styles carried across line ends without
  a background filling the next row, its line drawing restored, its history bounded by its restore budget (none at
  10,000 by 400), and the history lines a seed holds reported. About a minute. Part of the workspace nextest run. Run it
  when the crate or anything under `vendor/` changes. Ignored tests, run by hand in release: the snapshot spike's full
  round-trip configuration
  (`cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E 'test(/^full_size_round_trip_seed_/)'`,
  a few minutes), the review's 300,000 short scripts (`-E 'test(short_scripts_round_trip_at_review_size)'`) and 230,000
  hostile blobs (`-E 'test(hostile_blobs_never_panic_at_review_size)'`), the full round trip over private recordings of
  real sessions named in `FARHELM_SNAPSHOT_RECORDINGS` (`-E 'test(private_recordings_round_trip)'`, passed to the
  recorder with `--keep-farhelm-env`), the per-terminal memory measurement against the crate's stated budget
  (`-E 'binary(memory)'`, release, Linux; its numbers print only with `--no-capture`, which the recorder's generic mode
  accepts), and `write_missing_golden_fixtures`, which writes the fixtures of a new golden script and never overwrites
  one.
- `python3 scripts/record-test-run.py --kind development --selection 'snapshot encoder benchmark' --concurrency 'one test; fixtures sequential' --tmux none -- cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E 'binary(encode_speed)' --no-capture`
  — the ignored snapshot encoder benchmark (`tests/encode_speed.rs`): typical, true-color and image-like output at 200
  columns, 60 screen rows and 12,000 history lines, measured in sequence. Each reports five encodes' median and spread,
  blob size and history dropped. Fixture shape and repeated bytes are asserted; elapsed time is not. Compare on the same
  machine with load recorded. Generic recording permits `--no-capture`; the controlled nextest mode does not. Run it
  when the snapshot encoder changes, beside correctness and memory checks; minutes, no tmux or network.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'agent screen fixtures' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-supervisor --lib -E 'test(/^agent_kind::screen_fixtures::/) | test(/^agent_kind::status_line_fixtures::/)'`
  — status detection against the captured agent screens (`crates/farhelm-supervisor/tests/fixtures/screens/` and
  `tests/fixtures/status-lines/`, `docs/agent-screen-fixtures.md`): the fixtures' shape and privacy checks, each screen
  read as the state it was captured in by its harness's reader, the permission-mode footers, and each screen painted
  into the supervisor's terminal emulator reading back as the same text, with every row on its own row, and read by its
  reader as the captured screen is. No tmux: the captured text is the expected value. Seconds, in-process. Part of the
  workspace nextest run; run it when a status reader, the emulator crate, `vendor/` or the fixtures change.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'grid input encoder' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-supervisor --lib -E 'test(/^grid_input::/)'`
  — the supervisor's encoder of grid viewer input (`crates/farhelm-supervisor/src/grid_input.rs`): keys against
  Farhelm's key table `crates/farhelm-supervisor/tests/fixtures/grid-keys.jsonl` (the sequence for every key code below
  256, modifier set, platform and cursor-key mode), mouse reports against its mouse table
  `crates/farhelm-supervisor/tests/fixtures/grid-mouse.jsonl` (every grid mouse event, modifier set, reporting mode and
  encoding, at the screen's edge and the default encoding's limit), pastes (bracketed-paste markers once around a paste
  of several parts, line endings and sanitizing carried across parts, the closing marker of an interrupted paste), UTF-8
  mouse coordinates, focus reports, the wheel on the alternate screen, and `Input` framing. Seconds, in-process. Part of
  the workspace nextest run. Which keydowns become key messages at all (third-level shift, lone capitals, the key after
  a dead key) and mouse motion de-duplication are the grid client's, pinned by its own tests, not this selection. Both
  tables are checked-in contract data, first computed by the vendored xterm.js and read by no test from it now. The key
  table is rendered from the grid client's keyboard rules: after a deliberate change to them (`grid-input.js`, or
  `shift-enter-key.js`), run `node crates/farhelm-ui/js-tests/grid-input-tables.js --write` from the repository root,
  review the diff (a changed row is a key a program will read differently), and run this selection; the JS harness's
  `grid-input-tables.test.js` fails while a checked-in table differs from what the client computes, and it also holds
  the client's keydown and Alt-click tables (`crates/farhelm-ui/js-tests/fixtures/input/`). The mouse table is edited by
  hand, together with the encoder.
- `python3 scripts/record-test-run.py --kind development --selection 'vendored vte' --concurrency 'libtest default threads' --tmux none -- cargo test --manifest-path vendor/vte/Cargo.toml --features ansi`,
  `python3 scripts/record-test-run.py --kind development --selection 'vendored alacritty_terminal' --concurrency 'libtest default threads' --tmux none -- cargo test --locked --lib --manifest-path vendor/alacritty_terminal/Cargo.toml`
  and
  `python3 scripts/record-test-run.py --kind development --selection 'vendored crates paired' --concurrency 'libtest default threads' --tmux none -- cargo test --locked --manifest-path vendor/pair-tests/Cargo.toml`
  — the vendored, patched terminal crates (`vendor/README.md`): upstream's unit tests plus the tests of Farhelm's
  patches (the parser's ground state, the OSC payload and its overflow notice, zero-width, hyperlink and title caps,
  hyperlink reuse, the per-terminal extra storage, the history counters, XTVERSION, the pending-wrap flag only under
  autowrap, the erase and cursor movement fixes, clearing a row after it grew), and in `vendor/pair-tests/` the tests
  that need both patched crates together (an OSC 8 the parser drops ending the terminal's hyperlink). These are excluded
  from the workspace, so no workspace test run, clippy, `cargo fmt` or dprint (beyond `vendor/README.md` and
  `vendor/pair-tests/Cargo.toml`) reaches them, and CI does not run them. Run all three when anything under `vendor/`
  changes; seconds, plus a first download of the crates' own locked dependencies.
- `cd crates/farhelm-ui/js-tests && node --test` — the JS unit harness for the asset-JS layer's pure functions
  (PLAN_M6_5.md item 1); node is already a CI requirement for Playwright below, so this adds no dependency. Run from
  inside the directory rather than as a glob from the repo root: node's no-argument default discovery (every `*.test.js`
  in cwd) is the oldest, most version-portable form the test runner has, whereas quoted-glob CLI arguments are newer and
  CI pins no node version. Run it also when `crates/farhelm-proto/grid-fixtures/` changes: `grid-proto-fixtures.test.js`
  holds the grid client to those files and fails on a fixture it has no entry for, which `farhelm-proto`'s own tests
  cannot show.
- `node --test e2e/tests/helpers/program-input.test.ts` — the decoding the shortcut and dialog specs use to say what a
  program read from a page's terminal input frames (`e2e/tests/helpers/program-input.ts`): a byte stream's frames as
  their bytes even when JSON-shaped, grid keys as the supervisor encodes them by their numeric codes (Shift kept on
  Enter and Tab; a missing code, or one that disagrees with the key, a token), anything else as a token that fails an
  exact comparison, and the frames tagged with their socket's stream notice. Seconds, no browser. Run it when that
  module changes.
- `cd e2e && npx playwright test --config grid-client/playwright.config.ts` — the grid terminal component
  (`crates/farhelm-ui/assets/grid-*.js`) on a static page fed recorded grid streams (display, input, scrollback,
  selection and copy, links, paste, history applied in slices), its fit held to the documented formula over the measured
  character across text sizes and pixel ratios (and, for old terminals only, xterm.js's fit held to the same formula,
  the compatibility swap's premise), its copy across a wide character's wrap leaving out the empty edge cell, and
  `terminal.js`'s plumbing for grid streams against a fake socket (a new terminal's grid mounted with no xterm.js built
  or waited for, a byte stream on its attachment refused, a pending kind held until named, the swap from xterm.js for an
  old or unknown kind, attach, paste and drop interception, link opening, the hyperlink mismatch warning, file-path
  hovers and downloads, tunnels, reconnect and takeover snapshots, a catch-up ended by a takeover or a close before its
  marker, a stalled detach that does not reconnect, and typing and pasting clearing the selection), in Chromium and
  WebKit. No stack and no build: the page and the shipped assets are served through request routing. Needs `npm install`
  and the browsers from the browser suite's setup below; a few minutes. Not in CI. Run it when the grid assets,
  `terminal.js`'s grid handling or the fixtures under `crates/farhelm-ui/js-tests/fixtures/grid/` change.
- `cd e2e && FARHELM_E2E_SUITE_TERMINALS=tmux python3 ../scripts/record-test-run.py --runner playwright --kind development --tmux required --keep-farhelm-env FARHELM_E2E_SUITE_TERMINALS --selection 'old-terminal set, both engines' --concurrency '3 browser workers, one stack each; retries 0' -- npx playwright test`
  — the browser suite's old-terminal set: the tests whose subject exists only for old terminals (tmux, drawn by
  xterm.js), against stacks whose two supervisors start with `--new-terminals tmux`, with the pinned tmux first on
  `PATH` as for the browser suite. The ordinary run (the variable unset) creates every terminal in a terminal host and
  leaves these tests out; this run has nothing else. The set (`e2e/suite-terminals.ts`) is the whole spec files
  `OLD_TERMINAL_FILES` lists (xterm.js's link-gesture bookkeeping and its stale-row painting while scrolled back) and
  the tests tagged `@old-terminal` (`OLD_TERMINAL_TAG`) in the files `OLD_TERMINAL_TAGGED` lists: xterm.js's catch-up
  buffering, write-callback flow control and stall, its byte-stream input bound, its own query answers, selection,
  parser and DOM renderer, its OSC 52 read answer, and a supervisor still on old terminals showing no migration band.
  Each tagged test says in place where new terminals' counterpart is pinned. The `--keep-farhelm-env` is required: the
  recorder scrubs the variable otherwise, and the run is then the ordinary one under this label (global setup prints
  `suite terminals: tmux`, the config's metadata says `suiteTerminals`, and every test carries a `suite terminals`
  annotation, so a report shows which run it was). Global setup refuses a stack whose startup session is not a tmux
  terminal on a supervisor with the switch off. The config refuses either run when the set's account is incomplete (a
  listed file missing, a file listed as both whole and tagged, a spec using the tag unlisted or a listed one using it
  nowhere). A selection of its projects uses the recorder's generic mode (`--kind development` without
  `--runner playwright`, then `npx playwright test --project=<project> ... --retries=0`), since the Playwright mode
  refuses `--project`. On Tensorlake, `sandbox-tests.ts run --suite-terminals tmux` (`.agents/tensorlake.md`). Needs
  what the browser suite needs; about a hundred tests over twelve spec files. Not in CI and not part of the deflake
  sweep. Run a changed tagged test's or whole file's projects when it changes, and the set when terminal.js's xterm.js
  path, the helm's byte-stream relay or the supervisor's tmux path change in a way the Rust tests do not reach.
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'helm terminal relay' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-helm --lib -E 'test(/^terminal::tests::/) | test(/^client::tests::/)'`
  — the helm's terminal WebSocket (`crates/farhelm-helm/src/terminal.rs`) and its supervisor client
  (`crates/farhelm-helm/src/client.rs`), in-process against stand-in supervisors: attach, takeover, refusals, the stream
  notice, replay-complete ordering, heartbeat, pause and resume, frame and queue bounds, the stall detach, teardown, and
  the client's requests, uploads and relays, and the attachment trace (the `farhelm_helm::terminal_attachments` debug
  events the desktop smoke reads) reporting each attachment's start and end once. For grid streams: only `grid=1` asking
  for them on both routes, a grid stream relayed unread both ways (a 200 KiB frame sent before the reply, a browser
  message of exactly the 64 KiB input bound arriving as one frame, resize, pause and resume after it), a message over
  that bound ending the attachment with nothing forwarded after it (on the socket, and in the client whoever sends
  next), a slow browser pausing its grid stream and resuming it once it catches up with no frame lost, at 32 KiB
  messages with a writer queue's worth arriving after the pause and at 1 MiB rows with 8 arriving after it, the queue's
  pause and resume marks, event and byte bounds and frame bounds, an ended attachment never paused again, a grid-capable
  attach answered with bytes keeping a byte stream's bounds and never paused by the helm, and a wedged browser's handler
  ended by the stall detach for a bytes-only and a grid attach (premise: parked mid-send with the attachment live;
  oracle: the handler's own teardown `Detach` and the socket's close). Under a minute. Part of the workspace nextest
  run. Run it when the helm's terminal relay, its client's terminal queue or the protocol's attach fields change.
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
  in the job's shared target tree with `DESKTOP_SMOKE_WEB_DIST` naming the release's own web bundle, so it embeds the
  bundle that ships instead of building one; before any of it needs Xvfb, four pre-display process legs drive
  `farhelm-desktop`'s own entry point directly: a missing tmux and a below-floor tmux each refuse with one exact plain
  stderr line and exit status 1 (the app's own preflight, run before it would spawn its managed supervisor), and a
  non-tmux bootstrap failure (an unusable state directory) exits 1 through the ordinary fallback path with no panic. The
  non-pixel Xvfb integration gate that follows covers the embedded helm, managed supervisor, desktop authentication, the
  tmux override reaching the managed supervisor (read from its environment) while it runs no tmux and starts no tmux
  server, since its sessions are all new terminals (the sentinel tmux logs only the preflight's `-V`), the supervisor
  outliving a hard exit and an ordinary quit (reused, then replaced by a launch whose program file changed, with a
  session surviving the replacement), a second launch with another program refusing beside the open app without stopping
  its supervisor, a replacement that would fail its tmux preflight stopping nothing, a frozen (SIGSTOP) supervisor being
  replaced with a kill after the grace, restart persistence, that every asset the window requested went through the
  desktop asset handler, and that the teardown's terminal-host stop ends a real host planted in the answering-supervisor
  leg's state directory. The app's supervisor creates its sessions in terminal hosts, as every shipped one does, and the
  smoke requires every session it creates to be a host row; a session's terminal counts as alive by its database row
  (`terminal_kind` and `terminal_address`: a listening host socket in the kernel's socket table, or a tmux session for a
  tmux row, named from the session's id). Which session the relaunched window holds open, and that a token rotation left
  that attachment open, are read from the embedded helm's attachment trace (the `farhelm_helm::terminal_attachments`
  debug events, turned on through the launches' `RUST_LOG`), and after the supervisor replacement the kept session is
  attached as a grid viewer through the replacement and a typed command must write its file. API checks retain
  host-qualified session references; row and scope checks use their local session IDs. The optional coordinate-driven
  leg is not part of CI; it checks typed input through a file the typed command writes.
- `DESKTOP_SMOKE_APPEARANCE=1 scripts/desktop-smoke.sh` — optional Linux native appearance integration using the
  ordinary owned Xvfb/openbox setup. Four fresh/stored dark/light cases check native media-query premises, palette
  before renderer content, real Settings choices, private WebKit profile persistence, Inter, embedded assets and the
  same live terminal attachment's input and ordered output. The session's terminal is a grid terminal (every session the
  app creates runs in a terminal host), and the in-page probe reads it as the browser suite's terminal helpers do: the
  grid's painted background, its input field for focus, its own paste and grid text messages for typing, and its model
  for the 512-line burst; a terminal that is not a grid, a stream the helm did not announce as grid, or any xterm.js
  surface added to the page during the case (recorded from the page's start) fails it. Retains bounded JSON and one
  final client screenshot per case. DOM assertions do not prove transient paint correctness; normal appshots provide
  visual review. No live native system-appearance event, physical keyboard dispatch, throughput or macOS claim. Run
  through the recorder; the script supplies its own private profile and closed scenario inputs.
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
  and stop tests the same way, with the stop's check that a process has finished exiting, which on macOS rests on the
  zombie state alone:
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
- `python3 -B scripts/test-install-pinned-nextest.py` and `python3 -B scripts/test-install-pinned-dx.py` — the
  installers that put the pinned nextest and dx binaries on PATH in the release build jobs (and nextest locally), and
  the checksum verification and atomic publication they share (`scripts/pinned_tool.py`): archive and executable
  digests, archive shape, cache reuse, curl's size limit, each tool's platform mapping, and dx's refusal of a pin that
  is not the workspace's `dioxus` version. Run them when an installer, the shared module or a pins file changes;
  seconds, no network.
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
- `python3 -B scripts/plans-landing/test-landing.py`, `deno check scripts/plans-landing/idle-watch.ts`,
  `deno test -A scripts/plans-landing/idle-watch.ts` and `shellcheck scripts/plans-landing/notify.sh` — the landing
  monitor's round tooling (`scripts/plans-landing/README.md`): the monitor log edits a resume relies on (Status and
  entry sharing one stamp, a stopped monitor's Status refused, the claim line replaced once or refused), the round
  files, the claim check before every push and merge (exact slug and claim id), the landing notes' placeholder and
  line-length rules, the review page's escaping, and the idle watcher's reading of `queue status` and of the origin
  remote. The jj and gh sequences themselves are exercised only by real landings. Run them when those files change;
  seconds, no network beyond Deno's first fetch of its `jsr:` imports.
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
- `deno test -A scripts/tensorlake/sandbox-tests.ts` and
  `shellcheck scripts/tensorlake/bootstrap.sh scripts/tensorlake/run-shard.sh` — the Tensorlake browser-test harness's
  own checks: how projects are balanced into shards, how the snapshot listing is read, which projects each run of the
  suite has (the ordinary one, or the old-terminal set's), loading the set's account from the tested checkout, and how
  an old-terminal run's shard summary is checked. Run them, with `deno check`, when the harness changes; seconds, no
  sandbox. Using the harness itself is described in `.agents/tensorlake.md`.
- `python3 scripts/record-test-run.py --kind development --selection 'portable brick-test harness' --concurrency 'one unittest process' --tmux none -- python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`
  — validates owned Tart lifecycle, signed-input planning, guest execution, fixed command operations, durable handoffs,
  compiled checkpoint contracts, fresh-versus-update installer entrypoints, fixed command-only execution and provenance,
  bounded pre-action provider recovery with retained failures and no uncertain-action replay, structured native actions,
  durable completed-update evidence without transient progress images, exact native update-channel readouts without
  mandatory tooltip images, normal Finder readiness with verified menu dismissal, conditional product confirmations,
  remote menu Remove/re-add and Uninstall sequencing, independent resource-preservation/removal receipts, restart
  process handoffs, calibrated window measurements, gesture-trace and bonus gesture-readiness gates, failure gates and
  evidence import with fictional fixtures. Use a substrate that permits process-group inspection; Linux continuity cases
  skip on macOS. These tests prove harness contracts, not native GUI behavior or a release pass. Actual native fixture
  probes and release checks retain their own private evidence. The guest-only Swift geometry helper needs separate SDK
  type-check and read-only runtime validation in an owned disposable guest; the portable suite does not exercise that OS
  API.
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
runs in parallel. Every stack's two supervisors create new terminals (terminal hosts, drawn by the grid component), and
an automatic per-test check (`e2e/tests/helpers/host-premise.ts`, through `helpers/evidence.ts`) fails a test any of
whose terminal sockets, in a browser context the test observes (the default one, every context `observeContext`
observes, and any registered with `watchHostPremise`; tabs and remounts included), got a byte-stream notice for a real
session; a test that serves a session the helm does not list declares it with `fabricatedTerminals`. Global setup
refuses a stack whose startup session is not a terminal host, and the readiness waits run the per-socket check early.
Tests about old terminals alone are the old-terminal set above, which this run leaves out. The full-suite child command
is `cd e2e && npx playwright test`. It needs `cargo build` and
`cd crates/farhelm-ui && dx build --package farhelm-ui --platform web --release` first, because it drives the built web
UI against a real helm and supervisor, plus a one-time
`cd e2e && npm install && npx playwright install chromium webkit`. Reuse matching builds and successful test results
when the intervening diff leaves their coverage intact; see `docs/test-run-evidence.md` for recorded selections.

When the maintainer asks for browser specs to run on Tensorlake sandboxes, read `.agents/tensorlake.md` in full first.
It splits a selection across several cloud machines at once; it is never the default.
