# Tunnel helper exit may block flushing stdout

Reviewed commit: 8a7f66ede90651d47d1facc1e877ce81f842af1a

## TLDR

A tunnel helper may remain alive after its silence deadline if exit cleanup flushes buffered output into a full pipe.
The exact ordering and shipped-library premise remain unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Recording judgment: The pending macOS tunnel-lifetime plan addresses helm exit/heartbeat through the SSH gateway. Its
stated work does not expressly remove this helper's Rust stdout exit-flush mechanism; retain this distinct investigated
uncertainty. No later plan experiment is borrowed as proof of it.

Source location: `crates/farhelm/src/main.rs:2750-2753,2784-2808 at 8a7f66ede90651d47d1facc1e877ce81f842af1a`.

Source assessment (read with the later corrections below) (review record g040/l8, event 21):

Reviewed commit 8a7f66ede90651d47d1facc1e877ce81f842af1a; assigned 036-021 introduces the watchdog exit branch,
unchanged at pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: a helper whose caller stops reading stdout may
remain alive after its input-silence deadline if process-exit cleanup flushes pending stdout bytes. This could defeat
the cleanup intended to stop leaked Tensorlake sessions from keeping a sandbox running. Unverified after investigation;
this is a source-backed race concern, not a reproduced hang or vendor observation. Checks/results: exact cached Tokio
1.53.2 stdout.rs:170-178,217-227 wraps std stdout in Blocking; blocking.rs:116-132 queues an ordinary write and reports
accepted bytes before the worker completes, and :146-174 separately queues flush work. Its io/util/copy.rs:45-200
requests flush after pending reads/EOF, but that request is asynchronous and can await worker completion. Installed
standard-library process.rs:2571-2573 calls rt::cleanup; rt.rs:139-146 calls io::cleanup. std io/stdio.rs:729-744 uses
try_lock, so an ALREADY LOCKED stdout is correctly skipped (the initial narrow deadlock hypothesis is refuted). However
when the lock is free, :742 replaces the previous LineWriter; alloc/io/buffered/linewriter.rs:1-18 says a partial line
flushes on drop, and bufwriter.rs:710-715 calls flush_buf on drop. LineWriterShim::write in linewritershim.rs:55-180
buffers a small newline-free tail; BufWriter::write at :553-567 can accept it without writing to the OS. Concerned
interleaving: an underlying stdout pipe is full; a short newline-free socket chunk is buffered by the stdout worker,
which releases the lock; deadline handling wins before a queued/as-yet-unpolled flush worker owns that lock; process
exit then obtains the free lock and synchronously drops/flushed the tail into the full pipe. Missing premise: no
retained execution/fixture establishes this exact buffered-tail/worker-ordering state for the shipped helper at
deadline, nor a version-matched shipped Rust standard-library build. The readable source exposes the race and supplies
no ordering exclusion; source inspection alone cannot establish its actual incidence or independently observe that
completion never occurs. A source-matched process fixture filling the helper stdout pipe, controlling the final short
buffered write/flush scheduling, keeping stdin silent/open, and asserting bounded process exit would settle it; such
runtime experiments are forbidden here. SPEC_impl.md Tensorlake hosts:5973-5976 explicitly requires stdout backpressure
not defeat expiry. Fix direction if confirmed: avoid flushing Rust stdout on forced deadline termination, e.g. raw
unbuffered OS pipe writes or a deliberate immediate exit primitive after bounded diagnostic handling; preserve the
separate clean-EOF drain contract. Suggested bucket other for the established shutdown-contract concern; continuing
cloud billing is plausible but not independently demonstrated here. Confidence likely, with exact
interleaving/shipped-stdlib premise above; no security or user-work-loss route. proposed_drop false; possible_cover none
identified. Distinct from l2: that refutes ordinary Tokio Runtime drop, while this concerns std process-exit cleanup
after the watchdog fires. Distinct from l7: no oversized argument is needed; nominal 120-second limit can reach this
branch. Do not extend the try_lock refutation to the unlocked buffered-tail case.

Later qualification (review record g040/l8, event 22):

Supplement to l8, retaining its complete preceding mechanism, source-only likely confidence, other bucket, proposed_drop
false and exact missing interleaving/shipped-stdlib premises. Further source checks: LineWriterShim write at installed
alloc/src/io/buffered/linewritershim.rs:142-150 sends newline-free small writes to BufWriter::write, whose :553-567 can
buffer without an OS write. std/src/io/stdio.rs:719-723 initializes that LineWriter; cleanup :741-742 acquires try_lock
then replaces/drops it. BufWriter Drop :710-715 synchronously flushes the old buffer. The checked assigned process test
unread_supervisor_socket_cannot_bypass_the_idle_deadline (stdio_exit.rs:258-323) exercises upstream backpressure and
calls collect_stdio at :303; collect_stdio at :41-50 actively drains stdout. It therefore does not establish the
separate unread-stdout/buffered-tail premise and cannot refute l8. This is supporting test provenance for the same
concern, not a separate test-only defect. A read-only diff again found no main.rs/stdio_exit.rs changes to pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b. No runtime fixture, hang, exact production scheduling, billing observation or
version-matched release standard library was obtained; those qualifications remain. Initial held-lock concern is refuted
by try_lock; the unlocked buffered-tail path remains unverified after investigation. No substantive lead remains
unclassified.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g040/l8`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
