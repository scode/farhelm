# Supervisor idle CPU: what was eating it, and what to do about it

NOTE: This is a point-in-time investigation, not a spec and not a plan. It records what one live supervisor was doing on
2026-10-08 (release 0.25.0), how the cost was attributed, what two independent reviewers said about it, and what the
maintainer decided to pursue. The near-term TODO entries that reference this file carry the work; this file carries the
evidence and the options as they looked that day.

## The complaint

The maintainer kept seeing the supervisor use far more CPU than first principles would suggest, on a host where none of
the agents were producing much output. The host ran one supervisor with about 10 live sessions (mostly Codex and
Claude, many idle), about 23 sessions in total counting stopped ones, and one connected helm.

## How it was measured

Everything was observation of the running process: `perf record` with DWARF call graphs at 199 Hz for a minute,
per-thread CPU ticks from `/proc`, and `bpftrace` for syscall counts, file paths, child process argv, and per-function
attribution through uprobes. Nothing was restarted or reconfigured.

The supervisor used about 44% of one core over the profiled minute. That number is inflated somewhat by the profiler
itself; the lifetime average was about 21% (2h31m of CPU over 11h49m of uptime). The supervisor's private tmux server
used another 7% or so. The process had 54 threads, 53 of them tokio workers or blocking-pool threads.

## What the supervisor was doing

Nearly all of it was polling, and almost all of the polling was one per-session sweep (`capture_now` →
`capture_pass` in the supervisor's `service/capture.rs`) re-deriving state that had not changed.

The sweep had two drivers: the supervisor's own 2 s ticker, and the start of every `ListSessions` request. The helm
lists every 3 s as a backstop and again on every `SessionsChanged` hint (at most every 200 ms), and the ticker emits
that hint whenever a session's status flips, which Codex sessions do constantly between working and idle. The
per-session info request runs it too. Net effect: a full sweep roughly every 1.3 s, with ticker and list sweeps
overlapping and contending for the same per-session claims, the database mutex, and the blocking pool. Panes were
being screen-sampled every 3 s rather than every 2 s even though 10 panes is well under the ticker's 16-per-tick
budget, which says about half the ticks were overrunning.

Per sweep, among other things:

- **Codex transcript re-verification.** For every hook-reported Codex session, the supervisor read the session row
  twice, then opened the exact transcript file (`~/.codex/sessions/.../rollout-*.jsonl`, 3 to 70 MB each), read its
  first 64 KiB through tokio's file API (about a dozen blocking-pool hops per read), made a second 64 KiB copy with
  `String::from_utf8_lossy`, and parsed the first line into a generic `serde_json::Value`. Nothing was remembered
  between sweeps. Across 9 Codex sessions that was about 400 KB/s of reads. The first line is Codex's `session_meta`
  header: about 23 KB, of which 22,026 bytes is Codex's entire system prompt (`payload.base_instructions.text`).
  Farhelm uses a handful of small fields from it. Grok has an equivalent per-sweep re-check.
- **Hook-report drain.** For every session's report folder (stopped sessions included), a `read_dir` and then three
  unconditional renames, one per report slot (`latest`, `enrichment`, `selection`), each to a freshly generated
  `.taken-<slot>-<uuid>` name. This happened even when the listing had just shown the folder empty, and even for slots
  the session's agent can never write (only Grok writes `selection` and `enrichment`, and Grok never writes `latest`).
  About 58 failed renames per second.
- **Stopped sessions.** For sessions whose agent exited on its own or was interrupted, a read of the launch status
  sentinel (usually absent), two or three database queries for checkout provenance, and a full read and parse of the
  checkout-preparation JSON (up to about 50 KB), synchronously on an async worker. A finished preparation yields no
  answer and records nothing, so the same work repeats forever. Both the ticker and every list paid this.
- **Database.** About 150 SQLite lock acquisitions per second (visible as `stat` calls on `supervisor.db-journal` and
  `supervisor.db-wal` at that rate). The crate has no `prepare_cached` anywhere, every store call is a `spawn_blocking`
  hop plus a mutex, the session-row read also re-parses and re-validates the stored launch, and
  `resolve_resumable_notifications` opens a write transaction per eligible session per sweep even when the update can
  match nothing.
- **tmux.** About 5 to 7 short-lived tmux client processes per second: a `capture-pane` per live pane every 3 s, a
  `display-message '#{pane_title}'` per sampled Codex pane, and a `list-panes -a` per tick, per list, and per pane-death
  wake. Each capture allocates a zeroed 64 KiB read buffer.

## Where the CPU actually went

The flat profile, by mechanism: about 26% in `munmap` (almost entirely `smp_call_function_many_cond`, the TLB-shootdown
IPIs the kernel sends to every CPU that ran one of the process's threads), about 13% futex wake and wait, about 10%
SQLite (dominated by statement parsing), about 9% `epoll_wait` (much of it spinlock contention), about 6% `mmap` and
page faults, about 4% each in file syscalls and user-space malloc/free.

The release binary is static musl with mallocng and no global allocator, and mallocng hands freed groups back to the
OS eagerly. That made it tempting to call the allocator the biggest problem. It is not: the frees exist because the
polling allocates. User-space unwinding stops at musl's `__munmap`, so a stack profile cannot say who freed what. A
uprobe experiment instead tagged the thread state at each `munmap` over 30 s:

| Context at `munmap`                                                    | Count         | Share |
| ---------------------------------------------------------------------- | ------------- | ----- |
| All                                                                    | 3694 (~123/s) | 100%  |
| Inside the sweep (`capture_pass`)                                      | 2041          | 55%   |
| Inside the Codex header read (`records::read_prefix`), within the above | 1067          | 29%   |
| None of the tagged code (mostly blocking-pool threads doing its I/O)  | 1575          | 43%   |
| Inside a tmux capture read                                             | 0             | 0%    |

The Codex share is a floor, because its file reads run on blocking-pool threads that land in the untagged row. Measured
as time inside the polls, the Codex header read alone was about 8% of a core and the whole sweep about 15%.

## How it got this way

The hook-only identity change (`090a4766`, #1540, first shipped in 0.22.0, planned in `caa3bd7b`) removed the
heuristic record scans, which was the point. It also removed the sweep coalescing that let the ticker skip its sweep
right after a list had run one, and its plan costed the remaining per-sweep work ("a row read per integrated session
and a bounded header read per Codex or Grok exact file") as negligible at the scale SPEC.md targets. The plan also
kept the per-sweep exact-file check on purpose: "Checking the exact file a report names is verification of that report,
not identification, and stays." The per-folder hook-report drain (`cb340fb0`, #1594) landed in the same release.

The 64 KiB prefix bound (`RECORD_PREFIX_BYTES`) dates from the first conversation capture (`f4c2d70d`), where it
bounded a one-off scan "to learn something the first kilobyte already said". It is a ceiling, not a buffer size; the
Codex header has since grown to about 23 KB, so shrinking it is not the fix. Reading the whole ceiling instead of
stopping at the first newline, and doing so every sweep, is.

## Why read a Codex transcript at all

Codex's `SessionStart` hook payload carries `session_id`, `transcript_path`, `cwd`, and `source` (startup, resume, or
clear). Nothing in it says whether the report is about the root conversation or one of Codex's subagent threads, which
share the process. On this host, 189 of the 321 most recent transcripts were subagent threads, whose header `source` is
`{"subagent": {"thread_spawn": {...}}}` and whose persistent thread id differs from their `session_id`; every root
conversation had `source` of `cli`, `exec`, or `vscode` and the two ids equal. The header is the only place that fact
is written, so one read at admission has a functional reason. Whether Codex actually fires `SessionStart` for subagent
threads was not verified; the spec assumes it can.

After a `/clear`, the hook can fire before the new transcript exists, so something has to look again once the file
appears. Today that is the sweep; a later Codex hook event would also do it.

Everything after that is about withdrawing Resume early if the file is deleted or replaced. Restart can check at click
time instead, the way Pi already does; Codex's Restart currently relies on the sweep having just checked, so that check
would have to move.

## What the reviewers said

Two independent reviewers (Claude Opus 5.5 and GPT-6.1-sol at high effort) read the measurements, the code, and both
specs. They agreed on the substance and on most of the ranking:

- Both wanted the Codex/Grok re-check made cheap rather than kept as is. Opus proposed gating it on a `stat` (inode and
  size) with a full header read only on change, on Restart, and for a pending `/clear`, and noted that the purer
  hooks-only model (check only at Resume) is viable with spec edits. GPT argued for a more conservative per-sweep header
  comparison and flagged that metadata-only gating assumes the header is never rewritten in place.
- Opus ranked decoupling the sweep from list requests second; GPT ranked it fifth as the larger contract change. Both
  agreed it is a multiplier on everything else.
- Both called out the stopped-session re-observation, the empty-folder renames, the duplicate row read, uncached
  statements, and the no-op notification write transactions. The last one is explicitly accepted in SPEC_impl.md
  ("No per-entry latch suppresses these calls"), so changing it is a spec edit.
- Opus ranked an allocator swap third on its own; GPT ranked it below the top five. Both agreed it treats the symptom.

## What the maintainer decided

- **Read transcripts as rarely as possible.** Only when a decision functionally needs content no hook provides, never
  as a defensive fallback, and not because the spec currently says to. Where the spec demands more, the spec changes.
- **Sweep from the timer only, and stop re-deriving what is already known.** List and info requests stop triggering
  sweeps, and the sweep itself stops re-reading things that cannot have changed. Stopped sessions whose outcome is
  settled fall under this.
- **Cut the per-sweep overhead that remains:** skip report takes for empty folders, drop the duplicate row read, cache
  hot statements, and stop opening write transactions that cannot do anything.
- **Treat an allocator swap as an experiment only,** to be done after the work above and kept only if a before/after
  measurement shows it helps.

Not pursued for now: skipping screen captures for panes with no new output, and switching SQLite to WAL.
