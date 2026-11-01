# Testing the terminal host

NOTE: This crate is held to a higher bar than the rest of the repository because running hosts outlive every release
(SPEC.md, "Change policy"). This file records how that bar is measured and what is knowingly left uncovered, so the next
change starts from the same baseline instead of rediscovering it.

## The suites

- `tests/host_lifecycle.rs` and `tests/host_process.rs`: the built `terminal-host` binary on a real pty with a real
  child, driven through its socket. These are the behavioral specification; each test's docstring says which SPEC.md
  promise it pins.
- `tests/protocol_codec.rs`: the golden fixtures in `tests/golden/v1/` (the exact bytes of every version-1 frame, never
  edited; a new version adds a directory beside it), a check that every message has a fixture and every fixture file a
  message, round trips through a byte stream, and every refusal the decoders make. (Version 1 changed once before it
  shipped: `Displaced` was removed and `InputAck` gained its `discarded` count, so `displaced.hex` was deleted and
  `input-ack.hex` written afresh.)
- `tests/journal_model.rs`: the journal against a model that keeps every event, under random operation sequences with a
  cap small enough to force trimming, and output and caps large enough that reads, trims and snapshots cross output
  blocks. The model computes for itself what the journal must retain, which reason a bad snapshot is rejected for, and
  whether a read continues where it is or rebuilds from the stored snapshot: reads come from retained positions, and
  from trimmed, between-events and past-the-end ones, and a rebuild must start with the snapshot record. (Without that
  last check, a journal that dropped its start-offset test from `Journal::read` passed: it answered a trimmed position
  with a gap from there instead of the snapshot. Checked by making that change: the test now fails within a few cases.)
  Proptest records a failing case in `tests/journal_model.proptest-regressions`; there is none now. The two kept from
  development were dropped in the third round of review: proptest replays a seed through the current strategy, and the
  strategy had changed since, so they no longer produced the cases they were kept for. A few fixed cases beside it pin
  what the model cannot see: the exact numbers `Info` reports, the rejection reasons in their order, and a trim that
  cuts no more than the cap requires.
- `src/host.rs`'s unit tests run the same event loop in-process, on a pty and child the test starts. The first few serve
  it from a thread, mainly so coverage tools can see it. The step-driven ones call `Host::step` from the test's own
  thread between their reads and writes on the socket, and look at the host's state between iterations: that is how the
  bounds a client cannot observe exactly (the read budget, the drain bounds, the outgoing high-water mark and the reply
  budget it sets, the pending-input bound, the client's receive limit, how many connections one iteration accepts, what
  poll waits for) are pinned without timing. Some put a regular file where the pty master goes, so that how much output
  is ready does not depend on the kernel's tty buffering, or (opened read-only) so that every write to it fails; one
  puts a datagram socket there, so that each write the host makes arrives as one datagram and a small `Input` can be
  seen to go out in one `write`.

Some tests are per platform, because Linux and macOS ptys differ where SPEC.md says they do (a live child that closes
its terminal ends the output on Linux, but not on macOS) or because they see a process's state differently: Linux reads
`/proc`, macOS uses `ps` and `lsof`, and the child's own report where `ps` has no keyword for it (macOS's `ps` shows a
process's blocked signals but not its ignored ones, so a Perl one-liner as the child reports both, leaving out SIGFPE,
which Perl ignores itself as it starts). The child's descriptors are its own report on both platforms: a Perl one-liner
that tries `fstat` on each number, since listing `/dev/fd` opens a descriptor of its own (and macOS's `ls` keeps `.`
open besides, which the first macOS run of that test showed). Linux runs locally; macOS runs through the focused macOS
CI job (`.agents/validation.md`), the only place the macOS-only tests run.

## Running the tests: one test thread under libtest

`cargo nextest run -p terminal-host` runs each test in a process of its own, which is what these fixtures need. Under
libtest (`cargo test`, `cargo llvm-cov`, `cargo mutants`) they need one test thread: `RUST_TEST_THREADS=1`, or
`-- --test-threads=1`. Plain parallel `cargo test` is not supported. The tests start children (a host's, or the
in-process tests' own), and a child one test thread forks holds a copy of every descriptor of the test process until it
execs. On macOS, which has no atomic close-on-exec form of `openpty`, `socket`, `accept` or `pipe`, it can also keep
another test's pty end or connection open past its exec, for as long as it runs. Either way the other test waits for an
end of output, a close or an error pipe's end that comes late or never, and fails or hangs for a reason that has nothing
to do with the code under test. Linux now opens every one of those close-on-exec in one call (`posix_openpt` and
`TIOCGPTPEER` for the pty, since the third round of review), but the copy a forked child holds until its exec remains.
The host itself is single-threaded and is not affected.

`.agents/validation.md` at the repository root has the commands, the tool versions and how to install them.

## Coverage

Measured with `cargo +nightly llvm-cov -p terminal-host --branch --summary-only` on Linux, 2026-11-01, after the second
round of review (with libtest's default threads, which the third round found unsafe for these fixtures; see "Running the
tests" above, and run it with `-- --test-threads=1` from now on) (the in-crate unit tests count as part of the file they
live in; two runs differ by a few tenths, since some paths depend on timing, and this is the lower of them):

| File                       | Lines  | Branches |
| -------------------------- | ------ | -------- |
| `src/bin/terminal-host.rs` | 100.0% | n/a      |
| `src/host.rs`              | 96.7%  | 82.0%    |
| `src/journal.rs`           | 99.7%  | 95.0%    |
| `src/protocol.rs`          | 100.0% | 100.0%   |
| `src/spawn.rs`             | 88.4%  | 75.0%    |
| total                      | 96.6%  | 84.7%    |

The goal set 95% for lines and branches alike. The branch figure stays below that, and the maintainer accepted it
(2026-10-31, `.agents/validation.md`): what it misses is mostly the system-call failure arms listed below.

The daemon's code is measured because the host proper ends with an ordinary exit, which writes its coverage profile.
Excluded with `coverage(off)` (it runs between a fork and an exec, where no profile can be written): `child_after_fork`
and its `fail`, `signal_count` and `errno_location` in `src/spawn.rs`.

What remains uncovered is, with few exceptions, a system-call failure path: `fork`, `pipe`, `socket`, `bind`, `listen`,
`openpty`, `tcgetattr`/`tcsetattr`, `fcntl`, `setsid`, `dup2` or `sigprocmask` failing, `poll` failing with something
other than EINTR, `accept` running out of descriptors, and the peer-credential and descriptor-setting calls in `accept`.
(A write to the pty failing is covered: a read-only file stands in for the master. So are the pty and the child's error
pipe failing after the socket is bound, through a descriptor limit in the starter. The signal pipe is made before the
socket, and a limit low enough to fail it fails the first stage first.) None of the rest can be made to fail from a test
without a fault-injection layer, and adding one would put permanent complexity into a crate whose point is not to
change; the maintainer chose to leave them uncovered (2026-10-31). The few others:

- A second stage that dies before reporting readiness: needs a crash between fork and readiness.
- A connection from another user: needs a second account.
- The journal's "nothing retained but a cost above the cap" arm, which cannot happen (it is `unreachable!`).

## Mutation testing

NOTE: This is the list of mutants that survive, and why each is acceptable. Line numbers drift with every edit, so each
mutant is named by its function and its mutation, in cargo-mutants' own words ("replace X with Y in F"). That is what a
fresh run's `missed.txt` can be matched against. The list is only as current as the runs below: code changed since a run
covered it has to be mutated again (restricted to the changed functions with `--file` and `--re` is enough) before the
list can be trusted for it.

The first full run (`cargo mutants -p terminal-host --timeout 120`, Linux, 2026-10-31) missed 117 mutants in
`src/host.rs` and `src/journal.rs`, and 47 in `src/protocol.rs` and `src/spawn.rs`. The step-driven tests in
`src/host.rs` and the fixed cases in `tests/journal_model.rs` were written against that list, and a rerun restricted to
those mutants (plus two an interrupted journal run turned up) left 47, 44 of them in `src/host.rs`. Review of that
change rewrote much of `src/host.rs` and parts of `src/spawn.rs`, and a rerun on 2026-11-01 covered every function in
`src/host.rs` and the changed ones of `src/spawn.rs`.

The second round of review (2026-11-01) changed the journal's storage (output packed into blocks), input acknowledgement
and its cost, the close path, takeover, the start's order and the program lookup. Three reruns, one at a time, covered
what that changed:

- all of `src/journal.rs`: 158 mutants, 121 caught, 20 timed out, 13 unviable, 4 missed;
- the changed functions of `src/host.rs` (`Host::step`, `poll_timeout`, `discard_input`, `write_input`, `close_client`,
  `handle`, `refuse_client`, `accept`, `read_pending`, `act_on_buffered` and `new`, and `queue`, `resolve`,
  `acknowledge`, `drop_unstarted`, `accept_connection` and `listen`): 160 mutants, 79 caught, 43 timed out, 12 unviable,
  26 missed;
- the changed functions of `src/spawn.rs` (`start`, `second_stage`, `after_bind`, `is_executable`, `pipe`) and of
  `src/protocol.rs` (`encode_host`, `decode_client`, `decode_host` and their version-1 halves): 64 mutants, 46 caught, 8
  timed out, 7 unviable, 3 missed.

Two of the 33 missed were real gaps, killed by tests written for them and confirmed by a rerun of just those two:
`replace || with && in Journal::next` (a stream overtaken by a newer snapshot with nothing trimmed;
`a_stream_overtaken_by_a_newer_snapshot_sends_a_gap`) and `replace match guard client.gone with false in Host::handle`
(a closed client's `Input` queued after all; `a_closed_clients_input_is_dropped_as_it_is_decoded`, whose first version
checked only after a `Shutdown` that clears pending input anyway). The other 31 are listed below. A TIMEOUT counts as
caught: if a mutant makes a test hang, a test noticed it. Entries for code that these reruns did not cover stand from
the earlier runs.

NOTE: The runs above used two test threads, which the third round of review showed unsafe for these fixtures (see
"Running the tests" above): a test can hang because another test's child holds its pty, whatever the mutant. So a
TIMEOUT from those runs, in a test that waits for an end of output or a close, is weaker evidence than it looks. Runs
from the third round on use one test thread, where that cannot happen.

The third round of review (2026-11-01) changed how a connection ends (`Welcome` kept, a refusal's input dropped and its
`Error` flushed for at most `GOODBYE_FLUSH`, which `Shutdown` now shares), the socket file's removal (by identity), the
signal handler (`errno` kept), the start (a SIGTERM checked before the child and before readiness), and the pty's
opening on Linux (close-on-exec in one call). One rerun, with one test thread, covered the changed functions of
`src/host.rs` (`on_signal`, `terminated`, `Host::run`, the changed lines of `Host::step`, `poll_timeout`, `handle`,
`refuse_client`, `accept`, `read_pending`, `queue`, `end_connection`, `drop_unstarted`, `SocketFile::at`,
`remove_socket`, `listen`), of `src/spawn.rs` (`second_stage`, `after_bind`, `open_pty`, the Linux `open_pair`,
`spawn_child`) and `Journal::read`, whose model check is new: 105 mutants, 60 caught, 16 timed out, 13 unviable, 16
missed. Of the missed, seven are the listed `listen`, `Host::accept`, `second_stage` and `spawn_child` entries below,
unchanged; the other nine are in the new Linux `open_pair` and are listed below too. Every `Journal::read` mutant was
caught.

Writing the first round's tests turned up one real bug, fixed in the same change. When the pending-input bound was
reached partway through what the host had read from its client, the frames after that point stayed in the host's buffer
until the client sent something more. They had already been read, so no socket event would announce them again, and a
client waiting for their replies waited forever. `Host::step` now acts on held-back frames at the end of every
iteration, after its last write to the pty and to the client, so a drain anywhere in the iteration releases them
(`frames_held_back_by_the_input_bound_are_handled_once_it_drains`).

### Running it without disturbing a shared machine

Run it on Linux, contained like this:

```sh
systemd-run --user --scope --unit=fh-mutants-<uuid>.scope -p CPUQuota=400% \
  nice -n 19 env RUST_TEST_THREADS=1 cargo mutants -p terminal-host --timeout 120 --jobs 2
systemctl --user stop fh-mutants-<uuid>.scope
```

with a fresh UUID for the unit name, and `TMPDIR` pointing at a scratch directory of yours (cargo-mutants builds every
mutant in a copy of the workspace there). Each part is there because of something that went wrong on 2026-11-01: a
mutant that breaks the event loop makes every in-process host test spin a core until the timeout, and the test harness
runs one test per CPU by default, so one test binary took more than eight cores (one test thread, which the fixtures
need anyway, also keeps that down); and a host started by a test that cargo-mutants kills on timeout keeps running,
because it daemonizes out of the test's process group, so dozens of spinning hosts outlived their mutants. The scope
caps the CPU, and stopping it at the end ends every host the run started (they leave the process group, not the cgroup)
and nothing else. The rerun of 289 mutants took two hours this way; the event loop's mutants are slow, because most of
them hang a test until the timeout.

### Equivalent mutants

These compute the same result as the original in every state the program can reach. I checked each one against the code,
not just the test results.

- `replace | with ^ in Host::step`, at both operators of `POLLIN | POLLHUP | POLLERR` in the pty read check, and at
  `POLLHUP | POLLERR` in the client hangup check (three mutants). The flags are distinct bits, so `^` and `|` give the
  same mask.
- `replace > with >= in Host::drain_signal_pipe`. The two differ only when `read` returns 0, which means end of file on
  the pipe. The write end is never closed (`install_signal_handlers` forgets it), so that never happens.
- `delete -` on `SIGNAL_PIPE`'s initial value (-1 becomes 1). Nothing reads the initial value: the handler that reads it
  is installed only after `install_signal_handlers` has stored the real descriptor.
- `replace < with <= in accept_connection` and `replace < with <= in listen`, on `fd < 0` after `accept4` and after
  `socket`. The two differ only for descriptor 0, which is always taken: the second stage puts stdin on `/dev/null`
  (`null_stdio` in `src/spawn.rs`) before it listens, and the test process has a stdin of its own.
- `replace | with ^ in accept_connection` (on `SOCK_CLOEXEC | SOCK_NONBLOCK`) and both `replace | with ^ in listen` (on
  `SOCK_STREAM | SOCK_CLOEXEC | SOCK_NONBLOCK`). The flags are distinct bits.
- `replace < with <= in above_stdio`, on `moved < 0` after `fcntl(F_DUPFD_CLOEXEC, 3)`, which returns 3 or more, or -1.
- `replace | with ^ in open_pair`, at both operators of `O_RDWR | O_NOCTTY | O_CLOEXEC` (two mutants): distinct bits.
- `replace < with <= in open_pair` on `master < 0` after `posix_openpt`, and `replace < with <= in open_pair` on
  `slave < 0` after `TIOCGPTPEER`: they differ only for descriptor 0, which is always taken, as for `listen` above.
- `replace < with <= in Host::write_input`, on `n < 0` after `write`, and `replace < with <= in send`, on `n < 0` after
  `send`. The two differ only when the call returns 0. The host never writes an empty buffer (the protocol refuses an
  empty `Input`, and a frame is never empty), and a non-blocking write of a non-empty buffer to a pty or a stream socket
  either writes something or fails with EAGAIN.
- `replace > with >= in Host::expire_pending`, on `deadline > now`. The two differ only when the deadline is the very
  instant `Instant::now()` returns, to the nanosecond. A test cannot arrange that, and the outcome then (expiring one
  nanosecond early or late) is the same either way.
- `replace > with >= in Journal::accept_snapshot`, on `cut > 0`. With a cut of 0 the guarded code drains nothing and
  adds and subtracts 0.
- `replace > with >= in Journal::next_in_tail`, on `at.offset > cursor.position.offset`, the check that stops an
  `Output` at an unsent exit. They differ only when the unsent exit lies at the cursor's offset, and the code never gets
  there with it: an exit at or before the cursor's position has been sent just above, and one at the cursor's offset but
  a higher resize index has a resize at or before that offset still to be sent first, which returns earlier too (or, if
  that resize was trimmed, the cursor jumps over it with a gap). (The entry this list once had for
  `replace - with + in Journal::next_in_tail` is gone: blocks can now span a resize or the exit, so that mutant is no
  longer equivalent, and the model test kills it.)

### Calls that find nothing to do

These make the loop make a call that the original skips (or the reverse), when the call has no effect. The only cost is
a system call that returns EAGAIN, or the same work done later in the same iteration.

- `replace & with |` and `replace & with ^ in Host::step` in the pty read check
  (`revents & (POLLIN | POLLHUP | POLLERR)`). `|` reads the pty whenever it reported anything (a read that finds nothing
  gets EAGAIN). `^` reads whenever revents is anything but exactly those three flags, so it mostly adds such reads; it
  skips one only when the pty reports POLLIN, POLLHUP and POLLERR together, and as far as I can tell a Linux pty master
  never reports POLLERR. I have not checked macOS.
- `replace & with |`, `replace & with ^` and `replace != with ==` in `Host::step`'s pty write check
  (`revents & POLLOUT != 0`). Every iteration ends by calling `write_input` again whenever input is queued, so the write
  in this branch only happens earlier in the same iteration.
- `replace & with | in Host::step` in the client write check (`events & POLLOUT`) and in the listener check
  (`revents(listener) & POLLIN`). `write_client` with nothing writable gets EAGAIN or has nothing queued, and `accept`
  gets EAGAIN.
- `replace & with | in Host::step` in the client read check (`events & POLLIN`): `read_client` runs whenever the client
  reported anything. With its buffer full it reads nothing; otherwise it gets EAGAIN, or reads what it would have read
  in the next iteration. The one difference: a client that is closing (after `Shutdown` or a refusal) is not polled for
  reading, and if its peer shut down its writing side, the mutant notices that at once and drops the client, perhaps
  before what is queued for it is written; the original drops it when the write fails or it is written. Either way the
  connection ends.

### Performance only

These change how much work one iteration or one call does, with no difference a client can observe. A test could pin
them only by measuring time.

- `replace < with <= in Host::refill_outgoing` (`unsent < OUTGOING_HIGH_WATER`). Differs only when exactly
  `OUTGOING_HIGH_WATER` bytes are unsent, and then queues one more frame. The bound tests allow up to one frame over the
  mark, as the original does.
- `replace > with >= in Host::act_on_buffered` (`unsent > OUTGOING_HIGH_WATER`). Differs only when exactly
  `OUTGOING_HIGH_WATER` bytes are unsent, and then holds the next frame until the client takes one more byte. The budget
  tests allow up to one reply over the mark either way.
- `replace * with + in receive` on the chunk size (`64 * 1024` becomes 1088 bytes). This means more `recv` calls per
  call, but the same bytes: how much a call reads is set by its limit, which
  `one_receive_call_reads_up_to_its_limit_and_no_further` pins.
- `replace |= with &= in install_signal_handlers` (on `sa_flags |= SA_NOCLDSTOP`). This leaves SIGCHLD's flags at 0, so
  the handler also runs when the child stops or continues, and system calls are not restarted. A spurious wakeup reaps
  nothing (`waitpid` without `WUNTRACED` ignores a stop), and every call the loop makes is non-blocking or is `poll`,
  which is never restarted anyway. That gives extra iterations and no change in behavior. Pinning it needs the daemon's
  real handlers and a stopped child.
- `replace - with / in Journal::trim`, on `excess = cost - cap`. Each pass cuts at most `cost / cap` bytes from the
  front block (also clamped to the block and to the next retained resize) instead of at most `cost - cap`. For any cap
  of 2 or more (a host's is at least 64 KiB), `cost / cap` is at least 1 and never more than `cost - cap`, and a resize
  costs its whole `RECORD_COST` either way, so the loop never drops past the cap and ends in exactly the same state; it
  only takes more passes, which is why this is here and not under "Equivalent mutants".

### Races a test cannot arrange

- `replace != with == in Host::step` (`kind() != Interrupted` after `poll` fails). The mutant ends an interrupted
  iteration early and carries on after a real failure. After EINTR, the next iteration checks `TERMINATED` first, and
  the signal pipe (written by the handler) wakes its `poll` at once, so a signal is still acted on one iteration later.
  `poll` on valid descriptors does not fail otherwise. It needs the daemon's handlers and a signal during `poll`.
- `replace && with || in Host::pty_has_more` and `replace & with | in Host::pty_has_more` (in
  `poll(...) == 1 && revents & POLLIN != 0`). These differ only when the pty reports a hangup without data while it is
  still open to the host, which means the slave closed after the read just before returned EAGAIN. Then the exit is
  journaled one iteration later, after that iteration's read sees EIO.

### System-call failure arms

The maintainer declined a fault-injection layer (2026-10-31; see "Coverage" above), and these need one.

- `delete match arm Some(libc::EINTR) in Host::read_pty` and `delete match arm io::ErrorKind::Interrupted in receive`.
  Both are an EINTR from a non-blocking read, which returns at once and so practically never sees a signal.
- `replace < with == in listen` (on `fd < 0` after `socket`) and `replace < with == in above_stdio` (on `moved < 0`
  after `fcntl`): each misses its call failing.
- In the Linux `open_pair`: `replace < with == in open_pair` on `slave < 0` after `TIOCGPTPEER` (it misses the ioctl
  failing), and the fallback that the failure leads to, for kernels before 4.13, which lack `TIOCGPTPEER`:
  `replace != with == in open_pair` on `ptsname_r`'s result, and `replace < with ==`, `replace < with >` and
  `replace < with <=` in `open_pair` on the `open` after it. No kernel a test runs on takes that path.
- `replace + with - in Host::accept`, on `Instant::now() + ACCEPT_PAUSE`: the pause after `accept` runs out of
  descriptors, which needs a descriptor limit on the test process itself. (That a pause, once set, keeps the listener
  out of poll and ends on time is tested, by setting it directly:
  `a_paused_listener_is_left_alone_until_the_pause_ends`.)

### Peer credentials

These need a connection from a second user account (see "Coverage" above).

- `replace same_user -> bool with true` and `replace && with || in same_user` (Linux: `result == 0 && cred.uid == me`).
  (`Host::accept` now calls `same_user` alone, since `accept_connection` sets the descriptor flags, so the old
  `replace || with && in Host::accept` is gone with that condition.)
- `replace && with || in same_user` and both `replace == with != in same_user` in the macOS block (`getpeereid`, three
  mutants). That code is not compiled on Linux, so a Linux run cannot fail on it.

### Not compiled on Linux

A Linux run cannot fail on code that only macOS compiles. These are the macOS halves of functions that use an atomic
call on Linux (`accept4`, `pipe2`, `posix_openpt` and `TIOCGPTPEER`) and two steps on macOS:

- `replace < with ==`, `replace < with >` and `replace < with <=` in `accept_connection`, on `fd < 0` after `accept`,
  and `replace || with && in accept_connection`, on whether setting either flag failed.
- `replace != with == in pipe`, on `pipe(...) != 0`.
- The macOS `open_pair` (`openpty`, then the two flags), whose mutants the third round's rerun left out for this reason.

The macOS halves of `same_user` are listed under "Peer credentials" above.

### Killed since the reviews of the first round

Mutants the first round listed as acceptable, with a justification the reviews showed wrong, now each killed by a test:

- `replace match guard error.kind() == WouldBlock with true in Host::write_client` (now the guard on
  `WouldBlock | Interrupted`) was called harmless because a failing peer always reports a hangup. That holds for a peer
  that closed, on Linux; a peer that shut down only its reading side fails every write and reports no hangup, and macOS
  reports nothing for a socket polled without POLLIN. `a_failed_write_closes_the_client_and_still_acts_on_what_it_sent`.
- `receive`'s per-call bound (then `buf.len() > MAX_FRAME + 4 + 64 * 1024`, nine mutants) was called a performance
  matter. It bounds what one call keeps from a peer that writes as fast as the host reads, which is a memory bound; `==`
  could step over it. Now a limit argument, pinned by `one_receive_call_reads_up_to_its_limit_and_no_further` and the
  exact `MAX_FRAME + 4` the buffer-limit tests assert.
- `replace >= with < in on_signal` was called a race. The handler is a plain function:
  `the_signal_handler_wakes_the_loop_and_keeps_errno` calls it directly.
- `replace > with >= in close_inherited` was called harmless because `null_stdio` replaced 0 to 2 afterwards, which was
  wrong (with fd 2 closed, `null_stdio` opened `/dev/null` as 2 and then closed it). `null_stdio` now runs first and
  never leaves its own descriptor on 0 to 2, and the mutant closes stderr:
  `descriptors_are_not_inherited_and_the_host_does_not_pin_a_directory` checks each standard descriptor.

From the rerun's 46, these were killed by tests written for them: `replace | with & in Host::step` on the hangup check
(`a_close_is_acted_on_beyond_the_buffer_limit`), `replace == with != in Host::read_pending` on whether the old client's
front frame was unstarted (a check that went with `Displaced` in the second round), and
`replace - with + in Host::refuse_client` on the recount of unsent bytes (now in `drop_unstarted`)
(`a_refusal_finishes_a_partly_written_record_first`). Three went with the code they mutated: the match guard
`client.gone` in `Host::handle` and an `at += consumed` after a refusal in `Host::act_on_buffered` both guarded state
that is dropped right after, and `Host::close_client`'s running count of what it read is gone with the loop it bounded
(the last reading is now one call). The second review showed the first of those wrong: the state is dropped, but only
after a closed client's whole buffer of `Input`s had been queued, far past the pending-input bound. The guard is back,
on a match arm that drops a closed client's `Input` (`replace match guard client.gone with false in Host::handle`), and
`a_closed_clients_input_is_dropped_as_it_is_decoded` kills it.

From the second round: `replace > with >= in Host::refuse_client`, on keeping the frames not started before an `Error`,
was listed as behavior the spec left open. SPEC.md ("Ending" and "Host messages") now says that `Error` and `Goodbye`
both go out next, after at most a `Welcome` not written yet and the rest of a frame partly written; the code is shared
(`end_connection` and `drop_unstarted`), and `shutdown_drops_what_was_not_started` kills the mutant there.

### protocol.rs and spawn.rs

The first full run's 47 survivors here were handled directly, and a targeted rerun of these functions and constants (98
mutants, 2026-11-01) left nine; the functions of `src/spawn.rs` and `src/protocol.rs` changed since were in the reruns
above, which left only the `start`, `second_stage` and `pipe` entries below. Killed by tests added for them:

- The arithmetic in the resource-bound constants (`MAX_FRAME`, `MAX_SPEC`, `MAX_SNAPSHOT`, `MAX_TAG`, `MIN_JOURNAL_CAP`,
  `MAX_PENDING_INPUT`, `OUTGOING_HIGH_WATER`, `MAX_OUTPUT_CHUNK`, `EXIT_DRAIN_BYTES`): `tests/spec_bounds.rs`, which
  reads SPEC.md's table.
- `replace <impl fmt::Display for Malformed>::fmt -> fmt::Result with Ok(Default::default())`:
  `a_refusal_displays_its_reason`.
- `replace > with >= in split_frame`: the exact-limit case in `malformed_frames_are_refused`.
- `replace / with * in Reader<'a>::list`: `a_list_count_the_body_cannot_hold_is_refused_from_the_count`.
- `replace += with *= in Reader<'a>::list` and `replace > with ==` / `>=` on its running total: gone with the code. The
  list lies inside one frame that `split_frame` has already bounded, so the total check could not fail and was removed.
- The match guard of `main_entry` (now `flag == "--socket" && Path::new(path).is_absolute()`) and its `&&`:
  `the_command_line_takes_only_a_socket_path`.
- `replace > with >= in start` (`input.len() > limit`): the exact-limit case in `an_oversized_child_spec_is_refused`.
- `is_executable`'s mutants (now of `faccessat(..., X_OK, AT_EACCESS) == 0 && is_file`):
  `the_path_lookup_skips_what_cannot_be_run`, whose 0641 entry is what an any-execute-bit check would take.
- `replace | with ^ in set_cloexec` and `replace | with ^ in set_nonblocking`:
  `the_descriptor_flag_helpers_set_and_never_toggle`. Not equivalent: on macOS an accepted connection inherits
  `O_NONBLOCK` from the listener, and toggling would turn it off.

Left alive, each checked against the code:

- `replace > with >= in Reader<'a>::list` (`count > rest / 4`). At the boundary every item would be empty and use up the
  whole rest of the body, leaving nothing for the fields after the list, so both versions refuse the frame; only the
  message differs.
- `replace * with + in start` (the readiness report's `64 * 1024` bound becomes 1088 bytes). The second stage reports
  `ok` or a one-line error. The longest is `starting <program>: <error>` with a program path near `PATH_MAX`, which
  could pass 1088 bytes; the mutant would then cut that message short, and nothing else.
- `delete -` where a `-1` sentinel or comparison can never meet 1: `open_pty`'s initial `master` and `slave` (both
  written by `openpty`, and unused when it fails), `second_stage`'s `setsid() == -1` (setsid returns the new session id,
  which is the host's own pid and never 1), and `spawn_child`'s `fork` result (a child's pid is never 1).
- `delete -` in `set_cloexec` and `set_nonblocking`, on the `== -1` checks of `fcntl`'s results: the failure arm, which
  needs fault injection.

One defensive change has no test that could fail. `above_stdio` moves the readiness pipe and the temporary `/dev/null`
descriptor above 2 for a starter that left stdout or stderr closed, but both ways of starting a host are Rust programs,
and Rust's runtime reopens closed standard descriptors on `/dev/null` before `main`, so no test can hand the first stage
a closed one. Apart from its error check (listed above), it gives cargo-mutants nothing to change.
