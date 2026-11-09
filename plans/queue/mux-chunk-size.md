# Smaller transfer chunks between helm and supervisor

Written against main at 9e3ab60b on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Review how the helm-to-supervisor connection multiplexes its streams.** One connection carries every terminal,
> upload, download, TCP relay and control reply as framed channels, and the maintainer wants to challenge its sizing and
> scheduling before more traffic (recipe tunnels) moves onto it. As read from the code on 2026-11-07, not measured:
> frames up to 8 MiB; uploads, downloads and relays in 256 KiB chunks with a 4 MiB credit window per stream, so one
> stream's throughput is capped near window / round-trip time; terminal output in 32 KiB chunks with no credit window
> (watermark pause and resume instead); on the helm, a single 64-slot writer queue with no priority lane, where bulk
> traffic is rationed to 4 queued frames ahead of keystrokes and acks; on the supervisor, priority, ordinary and bulk
> queues, with bulk drained only when the other two are empty, so sustained terminal output can starve relays and
> downloads; no cap on concurrent relays; no half-close on relays; and a 60-second write stall that ends the whole
> connection. Challenge the chunk sizes, window sizes, queue depths and fairness between streams, measure where it
> matters, and change what does not hold up. Since that reading, recipe tunnels moved onto this connection, which added
> a cap of 64 tunneled connections per host and half-close for every relay.

While planning, the maintainer read an explanation of the current design and decided, in their words: "just switch the
frame size to 32. no elaborate complexity/measurement needed", confirmed as 32 KiB data chunks with the window and the 8
MiB frame ceiling unchanged; and "terminals prioritized above upload/download is fine and can be made intentional in
spec". Two follow-ups went into TODO.md's Mid term bucket in the planning PR (auto-scaling the window, and capping the
Mac app's own tunnels); they are not this plan's work.

What is true today, from a read of main at 9e3ab60b (line numbers approximate):

- **Transport and framing.** Remote hosts use the system `ssh` running `farhelm internal stdio`, which copies stdio to
  the supervisor's unix socket (SPEC_impl.md around 3166-3186; `crates/farhelm/src/main.rs` around 2793 and 2816); a
  local supervisor speaks the protocol on its socket directly (`crates/farhelm-proto/src/io.rs`). A frame is a 4-byte
  length, a 1-byte kind and a 4-byte channel id, then the body (`crates/farhelm-proto/src/lib.rs` around 588-631);
  channel 0 is control. `MAX_FRAME_LEN` is 8 MiB (around 460), a decode-time sanity cap so a corrupted length cannot
  make the reader allocate gigabytes.
- **Writers.** Each side has one writer task that flushes per frame; a write making no progress for 60 s ends the
  connection (`io.rs` around 115, 224-231). The helm has one 64-slot queue (`crates/farhelm-helm/src/client.rs`
  around 230) with only the heartbeat biased; upload and relay data share a 4-frame ration held until written (around
  283-287, `client/tcp_relays.rs` around 419), so about 1 MiB of bulk can sit ahead of a keystroke. The supervisor has
  priority (32 slots, upload and relay acks), ordinary (64 slots, terminal output and replies) and bulk queues, with
  bulk served only when both others are empty (`crates/farhelm-supervisor/src/service/connection.rs` around 244-259 and
  973-988).
- **Chunks and credit.** `UPLOAD_CHUNK_BYTES` = 256 KiB (`lib.rs` around 1802-1821) and `UPLOAD_WINDOW_BYTES` = 4 MiB
  (around 1830-1837), with cumulative-byte acks, one per chunk. Uploads send full chunks; a download fills each frame
  across short file reads and sends a partial frame only at end of file (`service/downloads.rs` around 368-401); relays
  forward each socket read as it arrives, up to the chunk size, and return credit when the destination accepts the bytes
  (`crates/farhelm-proto/src/tcp_relay.rs` around 110-118). The supervisor's per-upload queue is
  `UPLOAD_WINDOW_BYTES.div_ceil(UPLOAD_CHUNK_BYTES) + 1` frames (`service/uploads.rs` around 36-56), so it scales with
  the chunk; its comment explains why a full queue fails that transfer. The same `UPLOAD_CHUNK_QUEUE` is also the
  supervisor's connection-wide bulk queue depth (`connection.rs` around 258), which grows from 17 to 129 frames, still
  about 4 MiB in bytes; that is intended. Other code uses of the chunk constant: `crates/farhelm-proto/src/tcp_relay.rs`
  (three), the supervisor's uploads, connection, downloads and tcp_relays modules, and the helm's `client.rs`,
  `client/downloads.rs`, `client/tcp_relays.rs` and `uploads.rs`. Comments with the old numbers baked in go stale:
  `client.rs` around 271, 286, 8020 and 8950, `tcp_relay.rs` around 2 and 240, and the supervisor's `core.rs` around
  279. Other 256 KiB literals in the tree are unrelated. Recheck the list against the code before changing it.
- **Mixed versions break.** Builds sharing a protocol version talk to each other, and a 32 KiB build mixed with a 256
  KiB one breaks every transfer: an old helm's 256 KiB upload chunks fail the new supervisor's size check, relay reads
  over 32 KiB end the relay on the new side, and shipped peers size their per-transfer queues at 17 frames while the new
  side may send 128 frames of 32 KiB on credit ("download receiver overflow"). So the protocol version must bump.
- **Who the strict priority starves.** On the supervisor, upload and relay acks ride the priority queue, ahead of
  terminal output; data the host sends (downloads, and tunnel traffic from the host to the helm) rides the bulk queue,
  behind it. So sustained terminal output can starve downloads and the host-to-helm direction of tunnels, but not
  uploads or the helm-to-host direction.
- **Existing SPEC_impl.md coverage** to link rather than restate: terminal pause and resume (around 1611 and 3283-3306),
  relay credit, half-close and the 64-connection cap (around 3345-3370), and framing (around 3253). Missing today: the
  writers and their queues, the chunk, window and ack contract, the 60-second stall rule, and the frame ceiling.
- **Terminals.** Byte streams go in 32 KiB pieces (`REPLAY_CHUNK`, `connection.rs` around 37); the helm rejects terminal
  frames over 64 KiB (`lib.rs` around 475); grid messages target 32 KiB. Terminals use pause and resume marks rather
  than credit.
- **Recorded rationale.** Only qualitative: chunking so "no bulk payload may monopolize the framing layer", the window
  so "a large file costs time, never memory", the 4-frame ration capping bulk ahead of input at "about a megabyte".
  Nothing records why 256 KiB or 4 MiB in particular; both arrived in one commit on 2026-08-02.
- **OpenSSH** packs session-channel data into packets of at most 32 KiB whatever the frame size, so 256 KiB frames
  bought little on remote hosts (they did mean 8× fewer frames, acks and flushes, and a local supervisor uses no ssh at
  all).

### Acceptance criteria

- `UPLOAD_CHUNK_BYTES` (and anything else that sizes upload, download or relay data frames) is 32 KiB. The 4 MiB window,
  the 8 MiB frame ceiling, the terminal sizes, the queue depths that are not derived from the chunk, and the scheduling
  stay as they are; queue depths derived from the chunk scale with it. Uploads, downloads and relays work as before,
  including a download's partial final frame and transfers larger than one window. Stale comments naming the old numbers
  are corrected.
- The protocol version bumps per `crates/farhelm-proto/src/lib.rs`'s rules, with the reason (mixed chunk sizes break
  transfers) in the commit and in SPEC_impl.md.
- Comments beside the constants say why 32 KiB: it matches OpenSSH's session-channel packet size, so larger frames
  bought little on remote hosts, and it cuts the bulk that can wait in the helm's writer queue ahead of a keystroke from
  about 1 MiB to about 128 KiB. They do not promise a latency, since bytes already in the pipe and in ssh and TCP
  buffers are not bounded by this.
- SPEC_impl.md gets one subsection explaining how the connection multiplexes its streams: an overview that links the
  existing passages listed above and adds what is missing (the writers and their queues, the chunk, window and ack
  contract, the stall rule, the frame ceiling), accurate to the code after this change, without duplicating text that
  already exists.
- SPEC.md (or SPEC_impl.md, wherever the product promise belongs) states as intentional that terminal output from a host
  takes precedence over data the host sends (downloads, and tunnel traffic toward the helm), so those can slow, or stall
  while terminal output stays heavy; uploads and traffic toward the host are not held behind terminal output. Word it to
  match the code.
- Existing tests that assume 256 KiB chunks (frame counts, sizes, window arithmetic) are updated; no new measurement
  work.
- A `perf` PR with a changelog fragment (if the change has no user-facing effect worth an entry, a `kind: none` fragment
  with the reason). The PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08:

- **32 KiB data chunks, nothing else retuned**, and no measurement work. The alternatives were a measured review with
  fairness and priority fixes, or a report only.
- **The window and the 8 MiB frame ceiling stay.** Auto-scaling the window is a Mid term TODO entry, not this plan.
- **Terminal priority over transfers is intentional** and written into the spec as such; no fairness change.
- **The Mac app's uncapped tunnels** are a Mid term TODO entry, not this plan.
- **The explainer lives in SPEC_impl.md**, kept current. The alternatives were a dated lore record or the report only.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, narrowed by the maintainer's decisions above.

**Binding repository constraints:** `crates/farhelm-proto/src/lib.rs`'s protocol version rules; SPEC_impl.md's
connection sections; root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md
and FLAKES.md"; `.agents/test-authoring.md` if tests change; `.agents/validation.md`.

**Planner proposals:** the comment wording above, and putting the priority promise in SPEC.md if it already describes
transfers and tunnels from the user's side.

## Outline

One PR, `plan/mux-chunk-size/01-32k-chunks`, typed `perf`: the constant and every use of it, the comments, the
SPEC_impl.md explainer, the priority promise, the test updates, the changelog fragment, and the TODO.md removal.

### Validation

`cargo nextest` for `farhelm-proto`, `farhelm-helm` (uploads, downloads, relays) and `farhelm-supervisor` (uploads,
downloads, relays, connection), and the `farhelm` e2e tests `tcp_relays`, `file_downloads` and `attachment_uploads`, per
the pinned setup in `.agents/validation.md` and through `scripts/record-test-run.py`; `cargo clippy` and
`cargo fmt --check`; `dprint check` on changed Markdown; `python3 releasing/check-changelog.py format`.

## Fallbacks

Examples of departures that need the scope reassessment: changing the window, the frame ceiling, queue depths or
scheduling; adding measurement harnesses; capping tunnels.

Decisions to log: every use of the chunk constant you changed or deliberately left, the protocol version you bumped to
(other queued plans, such as `grid-binary-stream`, also bump it; on a rebase conflict over the number, take the next
free one), where the priority promise went, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-mux-chunk-size-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/mux-chunk-size/<nn>-<short-name>`, as the
  outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
