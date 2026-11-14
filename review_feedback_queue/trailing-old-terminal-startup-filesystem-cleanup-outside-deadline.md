# Old-terminal file cleanup runs outside its startup deadline

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Farhelm's old-terminal cleanup does not enforce its promised startup budget for
filesystem cleanup, so this phase can hold a host's return after Update beyond that budget.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g149/l1`.

### g149/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4; unchanged cleanup verified at pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: Farhelm's old-terminal cleanup does not enforce its promised startup
budget for filesystem cleanup, so this phase can hold a host's return after Update beyond that budget. Details:
end_server receives one deadline, but calls remove_server_files directly at lines 182, 244 and 250. That helper at
423-449 awaits read_dir, iterates until end with no entry/time limit, collects matching temporary names, then awaits
every unlink; none is inside timeout_at. The elapsed time is recorded only after end_server returns and subtracted from
the later sweep's budget, so that accounting cannot cap the cleanup that already ran. Exact contract,
SPEC_impl.md:3000-3002: 'the step's own work, the server's end and the sweeps (not the reload's host discovery between
them), has one 15-second budget (`OLD_TERMINALS_DEADLINE`), half the 30 seconds the helm allows a supervisor it
restarted during Update to come back'. This enforcement gap is source-confirmed; no particular count, healthy-directory
delay or measured timeout is claimed. Healthy normal writes are temporary and ordinarily cleaned: files.rs:147-166 says
staging cleanup can be skipped by a hard crash, names use fresh UUIDs, and a backstop sweep handles debris; the root
otherwise stores fixed metadata plus subdirectories for per-session state. Therefore ordinary session count must not be
presented as an unbounded root-file workload. The helper itself imposes no cardinality bound on directory entries or
retained crash/failed-cleanup debris, and no bound on asynchronous filesystem completion. No source-backed healthy
numeric shape/time bound has been established. The claim is the missing enforcement of an explicit budget, not proof
that a typical healthy directory exceeds it. Socket part of original lead is refuted: pinned Cargo.lock has tokio 1.53.2
and mio 1.2.4; tokio src/net/unix/stream.rs:116-128 calls mio connect, waits for writable readiness, then checks
SO_ERROR without an application reply. mio src/sys/unix/uds/stream.rs:8-20 accepts only immediate success/EINPROGRESS
and returns other errors (including EAGAIN); src/sys/unix/net.rs:15-35 uses SOCK_NONBLOCK on Linux and :58-89 sets
O_NONBLOCK with fcntl on macOS. hosts/process.rs:206-210 and SPEC_impl.md:2970-2971 document full-backlog EAGAIN/Linux
and refusal/macOS, both promptly logged by end_server. Thus a wedged tmux application alone does not show a
hanging-connect route. Suggested bucket: other (deadline contract/enforcement), with high impact requiring an
established supported situation causing material Update delay; could-be-highest: no established or suspected
security/loss-of-work route from this issue. Confidence: confirmed source-level deadline gap; runtime occurrence and
frequency unverified, no execution or reproduction. proposed_drop: false; replaces prior proposed drop because absence
of a measured workload does not answer enforcement of the finite declared budget. possible_cover: none known. A remedy
would put the entire cleanup await under the existing deadline and log/retain any uncompleted debris for a later start;
physical completion of a filesystem syscall may still outlive cancellation, but startup need not wait for it. Further
synthesis checks: core.rs:4238-4286 explicitly identifies the root metadata/subdirectory shape and treats removal
failures as retained debris; its backstop is called in serve at 8406-8407, after constructor old-terminal cleanup, so it
does not pre-bound this helper. tmux.rs:3226-3265 stages config before a server start, not per live session. No
normal-session-count scaling route was found. At pinned tip SPEC_impl.md:2821-2823 retains the same 15-second contract,
and the complete old_terminals.rs diff since introduction changes only its documentation link; the helper/deadline gap
has no later repair.

Returned proposal (claims-096, event 11): **retain-verified**, suggested bucket **other**.

The cleanup awaits are outside the declared startup deadline. Retain the narrow enforcement discrepancy; ordinary
session count is not a demonstrated large-directory trigger, and no measured Update delay is established.

Original stronger consequence: Original suspicion: an unresponsive old tmux server could hang connect and delay Update;
the socket route was source-refuted. Latest consequence is a structural filesystem-deadline gap with conditional
host-return delay.

Unresolved premise or limitation: A supported healthy state-directory workload that materially exceeds the startup
budget is unestablished. No typical delay, filesystem completion bound, or session-count scaling is claimed.

Proposed basis: Record only the missing enforcement of the explicit finite startup budget, preserving the
healthy-filesystem acceptance and absent runtime premise. Do not assert a reproduced hang or count ordinary sessions as
root-directory debris.

Existing parent disposition (event 11): verified. Parent explicitly accepted named retain-verified proposal from
claims-096 after inspecting it. The cleanup awaits are outside the declared startup deadline. Retain the narrow
enforcement discrepancy; ordinary session count is not a demonstrated large-directory trigger, and no measured Update
delay is established. Record only the missing enforcement of the explicit finite startup budget, preserving the
healthy-filesystem acceptance and absent runtime premise. Do not assert a reproduced hang or count ordinary sessions as
root-directory debris. Checked source/results:
["00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:crates/farhelm-supervisor/src/service/core/old_terminals.rs:160-183,244-250,350-356,423-449:
end_server directly awaits remove_server_files, whose directory enumeration and unlink loop have no deadline. spent is
accounted only after it returns.","git diff
dbfc69751215dbf15e1d31d5bed73c3754c4ebc4..00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db for old_terminals.rs changes only a
specification-heading link; the introduced implementation survives.","Initial socket-hang suspicion was withdrawn after
the reviewer checked pinned nonblocking Tokio/mio behavior. This consultation did not independently reread registry
sources; the retained finding concerns filesystem cleanup only."] Current coverage/results: [{"source":"SPEC.md and
SPEC_impl.md","result":"SPEC_impl.md:2781-2823 declares one 15-second budget. SPEC.md:4658-4675 explicitly accepts
host-local filesystem errors/hangs, so that subset cannot establish an uncovered runtime bug. It does not expressly
accept healthy directory enumeration exceeding the declared budget."},{"source":"TODO.md:25-26","result":"Planned bucket
is empty. The phase-3 removal idea at 225-232 is in Mid term, not Planned, and does not cover these
findings."},{"source":"plans/queue","result":"Searched named behavior and symbols. No matching unlanded corrective plan.
Relevant tmux-end-old-terminals, tmux-drop-from-docs and environment-agent-operations plans are already landed per
plans/REPORTS.md:28-32, despite retained queue files."},{"source":"BUGS.md","result":"Read fully; sole accepted
ssh-output-pipe hang is unrelated."},{"source":"review_feedback_queue/INDEX.md and matching files","result":"Searched
actual mechanisms, symbols and consequences. No exact existing item or useful same-root extension found.
tab-reap-deadline-excludes-listing-request.md was read; its test polling helper is distinct from production startup
cleanup."},{"source":"TRIAGE_OUTCOMES.md","result":"Searched actual mechanisms and consequences; no matching active
decision. Archive not read."},{"source":"review_feedback_queue/FILTER.md","result":"Read fully. No whole-boundary filter
match used."}] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Original
suspicion: an unresponsive old tmux server could hang connect and delay Update; the socket route was source-refuted.
Latest consequence is a structural filesystem-deadline gap with conditional host-return delay."; "A supported healthy
state-directory workload that materially exceeds the startup budget is unestablished. No typical delay, filesystem
completion bound, or session-count scaling is claimed.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g149 record](records/review-through-00de0b6c/groups/g149.json). Record ID: `l1`.
