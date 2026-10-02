# Execute the 2026-10-02 highest-priority triage outcomes: clipboard bound, fallback queue cleanup, terminal data limit

Written against main at 97f010cd on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan. It touches neither the helm's clipboard endpoint nor its SPEC.md
best-effort sentence, which `plans/triage-restart-takeover-update.md` item 4 changes; if that plan has landed by the
time you run, read its result so your comments do not contradict it.

## The goal

Carry out the three triage outcomes recorded in root `TRIAGE_OUTCOMES.md` whose Execution field reads
`` planned in `plans/triage-clipboard-terminal-limit.md` ``, exactly as root `AGENTS.md` section "Execute triage
outcomes" prescribes: one reviewable commit, one stable bookmark and one draft PR per outcome, in a single linear stack,
in ledger order:

1. `desktop-clipboard-fetch-backlog.md` (fix code)
2. `desktop-protocol-filesystem-fallback.md` (other: queue cleanup only)
3. `terminal-output-queue-missing-byte-budget.md` (fix code)

Each outcome's `TRIAGE_OUTCOMES.md` entry (Assessment, Decision, Completion criteria) is the authoritative statement of
what that PR must do. Read the entry and its feedback file under `review_feedback_queue/` before starting the item. This
file adds the plan-time decisions and the mechanisms proposed to satisfy those entries; where the two seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- Three draft PRs exist, stacked in the order above on top of the plan stack's tip (see PR discipline).
- Each PR does what its ledger entry's Completion criteria say, as refined by the plan-time decisions below.
- Each PR removes its feedback file and its line in `review_feedback_queue/INDEX.md`.
- Each PR updates its own `TRIAGE_OUTCOMES.md` Execution field to `complete` with the jj change ID, bookmark and PR URL.
  Record the change ID and bookmark before creating the PR, then add the URL to the same change and push again; no
  separate bookkeeping PR.
- PRs 1 and 3 have passed the review gate (both reviewers). PR 2 gets no review.
- PR 3 also changes this plan's line in `plans/INDEX.md` to `[executed]`. Do not delete this plan file.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request:** "then we schedule using planning system", after triaging the highest-priority bucket of the
review feedback queue on 2026-10-02.

**The user's triage decisions (2026-10-02),** recorded in full in each ledger entry's Decision field:

- Item 1: fix code. SPEC.md already requires the bound: "Local authority and trust between hosts" calls OSC 52 writes
  "an explicitly allowed, bounded effect across the remote-host boundary", and remote malicious behavior must not
  disrupt ordinary GUI controls. No spec change.
- Item 2: first decided as fix code, then superseded the same day: the hardening is deferred to the TODO.md
  `Maybe
  later` entry "Close the desktop window's filesystem read fallback (hardening only)", which already landed
  with the triage record. What remains is removing the feedback file and its index line. No code, spec or TODO change.
- Item 3: "fix code with a simple fix and a hard limit but explain the limit's relationship to REPLAY_CHUNK clearly and
  with a test so that if REPLAY_CHUNK grows larger we can't fail to remember to update the other constant."

**The user's plan-time decisions (2026-10-02):**

- P1. Review gate: two independent fresh-context reviewers per PR, one on Opus 5.5 at high effort and one on gpt-6-astra
  at high effort, both with the general review charter. No review swarm. PR 2 (pure queue cleanup) gets no review.
- P2. Item 3's oversized frame detaches only that terminal, reusing the helm's existing queue-overflow detach and its
  "stalled" reason. In the user's words: "rare/unexpected enough that complexity/code for better error isn't worth it".
  No new detach code, no new UI text, and the connection to the host stays up.
- P3. Item 3's limit is a `farhelm-proto` constant of 64 KiB, headroom above the supervisor's 32 KiB `REPLAY_CHUNK` so
  the supervisor can grow its chunk a little without breaking older helms.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Sharing the machine with other agents; Agent scratch space; The live install is off-limits),
`plans/AGENTS.md` (Executing), `review_feedback_queue/AGENTS.md` (Lifecycle), and `.agents/test-authoring.md` for test
changes.

**Planner proposals** are the per-item mechanisms below. A fresh-context planning review checked them for unnecessary
scope and against the code; it found none, confirmed `send_bytes` is the supervisor's only data-frame sender and that a
controlled-peer harness already exists, and its correctness notes are folded into items 1 and 3.

## Per-item outline

Line numbers drift; find the code by name.

1. **Bound the page-side clipboard writes.**
   - Today every OSC 52 write reaches `clipboardProvider.writeText` in `crates/farhelm-ui/assets/terminal.js`, which
     calls `window.__farhelmNativeClipboardWrite` (installed by `arm_native_clipboard_script` in
     `crates/farhelm-ui/src/auth.rs`; one fire-and-forget `fetch` to `POST /api/clipboard` per call) or, in a browser,
     `navigator.clipboard.writeText`. Copy-on-select's mouseup path in terminal.js picks between the same two writers.
     Nothing bounds outstanding writes.
   - Proposal: a small pure module under `crates/farhelm-ui/assets/` (following `copy-on-select.js` and
     `shift-enter-key.js`: terminal.js calls the exact function `node --test` exercises), holding one coalescing writer
     shared by both terminal.js write paths: at most one write in flight and at most one pending value, a newer value
     replacing the pending one, the pending value written when the in-flight write settles either way. The underlying
     writer is whichever route exists at call time, as today. For the coalescer to know when a native write settles, the
     bridge in `auth.rs` returns its `fetch` promise, still with its own `.catch` so it never rejects; keep the auth.rs
     unit test passing and extend it if it pins the old shape. Keep the OSC 52 handler synchronous (it must still return
     `undefined`, never a promise; terminal.js's provider header explains why) and every failure silent.
   - The coalescer treats an underlying write that returns no promise (the bridge's synchronous `catch` path, a missing
     `navigator.clipboard`, a test mock) as already settled; otherwise it would stay "in flight" forever and silently
     drop every later copy.
   - Routing copy-on-select through the same coalescer is required, not optional: otherwise a pending OSC 52 value can
     land after the user's own newer copy. One coalescer instance per page, shared by every terminal and both paths,
     created only once its asset has loaded; add its global to terminal.js's `tryMount` preconditions the way the other
     helper modules are (the asset comments in `crates/farhelm-ui/src/lib.rs` describe that list).
   - The session header's click-to-copy (`session_view.rs`) is a single user-initiated write and is not routed through
     the coalescer. It already calls `.catch(fallback)` on whatever the bridge returns, which is why the bridge's
     promise must never reject: a rejecting promise would start a `navigator.clipboard` fallback there, a behavior
     change. `e2e/tests/header.spec.ts` mocks the bridge and is the spec to run for it.
   - Register the new asset the way `copy-on-select.js` is registered in `crates/farhelm-ui/src/lib.rs`, and run
     `scripts/check-desktop-assets.sh`.
   - Tests: node tests in `crates/farhelm-ui/js-tests/` for a burst of writes against an underlying writer whose promise
     never settles (exactly one call reaches it; nothing else is retained beyond the one pending value), for the latest
     value being the one written once the in-flight write settles (resolve and reject both), and for a writer that
     returns no promise not blocking later writes.
   - Changelog fragment: `kind: fixed`.
2. **Queue cleanup.** Remove `review_feedback_queue/desktop-protocol-filesystem-fallback.md` and its index line, and
   update its Execution field. Nothing else. `docs:`. No review, no changelog fragment.
3. **Hard limit on incoming terminal data.**
   - Today the only size cap on a data frame is `MAX_FRAME_LEN` (8 MiB, `crates/farhelm-proto/src/lib.rs`). The helm's
     `dispatch` (`crates/farhelm-helm/src/client.rs`) hands every data frame's body to `route_terminal_event`, which
     queues it into a per-attachment channel bounded at `TERM_EVENT_QUEUE` (256 events) and, when full, removes the
     entry, signals a `DETACH_REASON_STALLED`/`DetachCode::Stalled` detach and releases the upstream attachment. The
     queue is registered before the attach request is sent, so frames that arrive before `Attached` are routed into it
     too. The honest supervisor sends terminal data only through the forwarder's `send_bytes`
     (`crates/farhelm-supervisor/src/service/connection.rs`), chunked at `REPLAY_CHUNK` (32 KiB); confirm that is still
     the only data-frame sender before relying on it.
   - Proposal: add a public constant to `farhelm-proto` for the largest terminal data chunk a helm accepts, 64 KiB (P3).
     In `route_terminal_event` (or `dispatch`, whichever keeps the overflow handling in one place), a data frame whose
     body exceeds it takes the same path as a full queue (P2): remove the entry, signal the stalled detach, release
     upstream. Frames for channels with no entry are ignored as today.
   - Documentation: the new constant's doc comment says what it bounds (one data frame's payload, so one attachment's
     queue holds at most `TERM_EVENT_QUEUE` × the limit, about 16 MiB), that the supervisor's `REPLAY_CHUNK` must not
     exceed it and why (a larger chunk would detach every terminal on helms that enforce the limit), and that raising
     the limit is safe for newer helms but supervisors must keep chunking to the smallest limit any supported helm
     enforces. `REPLAY_CHUNK`'s doc comment points back at the limit. Rewrite `TERM_EVENT_QUEUE`'s "Honest caveat"
     paragraph: it currently disclaims any memory bound and explains why no byte bound was added; it should now state
     the bound the limit implies.
   - Tests: a supervisor unit test that fails when `REPLAY_CHUNK` exceeds the proto limit, with a failure message that
     says to read the limit's doc comment before raising either (the user asked for a test, so this is a test; a
     compile-time assertion may be added beside it but does not replace it). A controlled-peer regression in the helm,
     using the existing fake-supervisor test harness that the client's tests already use, that sends one data frame of
     limit + 1 bytes for the attaching channel before replying `Attached` and shows the terminal ends detached as
     stalled, plus a frame of exactly the limit still being delivered. Use the real attach exchange (read the `Attach`,
     write the frame, reply `Attached`), not a helper that registers the terminal without one, because the ledger asks
     specifically for the pre-`Attached` window;
     `a_full_terminal_queue_detaches_that_terminal_without_blocking_the_others` in `client.rs` and
     `scripted_supervisor_attach` in `crates/farhelm-helm/src/terminal.rs` show the harness. Prefer putting the size
     check in `route_terminal_event`, or a helper shared with its full-queue branch, so there is one teardown path. No
     multi-megabyte fixtures beyond that.
   - Changelog fragment: the effect is only observable with a hostile or broken supervisor; choose `kind: fixed` or
     `kind: none` with a reason, and log the DECISION.
   - This PR also marks this plan's `plans/INDEX.md` line `[executed]`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-clipboard-terminal-limit-log.md` in the parent directory of the checkout you run in, derived as
that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and `TRIAGE_OUTCOMES.md` Execution fields on each PR) rather than starting over. If it does not exist, this is a fresh
start.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is not main but the tip of the plan stack, set up per `plans/AGENTS.md`
  (Executing, step 4). PRs already in the plan stack, from earlier plans or an earlier blocked run of this one, are the
  base and are not rewritten.
- One outcome per commit, bookmark and draft PR, in the order listed in The goal. Never combine outcomes. Within this
  run, if a PR needs correcting, restructure it rather than stacking a correction on top.
- Commit messages and PR titles use Conventional Commits. The type reflects the user-visible effect: `fix:` for PRs 1
  and 3, `docs:` for PR 2. Every `fix:` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit,
  per root `AGENTS.md` (Releases and the changelog); validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing the plan stack is the user's job.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for Rust test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- PR 1: `cd crates/farhelm-ui/js-tests && node --test`, `scripts/check-desktop-assets.sh`, `cargo fmt --all -- --check`,
  clippy and the unit tests for `farhelm-ui` if `auth.rs` changed (the bridge is `cfg(native_desktop)`, so
  `cargo check -p farhelm-ui --features desktop` and the desktop nextest selection for `auth` when the webkit packages
  are available; say so if they are not), and `e2e/tests/terminal-clipboard.spec.ts` plus `e2e/tests/header.spec.ts`
  (because the bridge's return value changes) on Chromium and WebKit through the recorder, after the builds root
  `AGENTS.md` names.
- PR 2: `dprint check` on the changed files, nothing else.
- PR 3: `cargo fmt --all -- --check`, clippy on `farhelm-proto`, `farhelm-helm` and `farhelm-supervisor`, and focused
  nextest selections for the new tests and the helm client's existing terminal routing tests.
- Any PR that changes Rust or browser tests or their helpers: run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.

### Review gate

Before finishing PRs 1 and 3, use the active galaxy-brain skill to delegate two independent reviews of that PR's
changes. The user demands both reviewers (P1): a fresh-context agent on Opus 5.5 at high effort, and a fresh-context
agent on gpt-6-astra at high effort, shelled out to the other harness when the executing one cannot reach that model
natively. No review swarm. Each prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

Each prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory; one per reviewer), and the acceptance criteria for that PR: its `TRIAGE_OUTCOMES.md`
entry, its item above, and the plan-time decisions that apply to it. For a PR that changes tests or fixtures, include
the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. The two reviews may run
concurrently. Address what both reviewers find before moving on; where they disagree, decide on the merits and log the
DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new subsystem, a new detach code or wire field, a
byte-counting queue, a change to the helm's clipboard endpoint, or anything else the ledger entry did not imply; these
are examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the ledger entry, the user decisions above, this
outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative
was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular where the coalescer lives and how it learns a write settled (item 1), the name
and placement of the proto constant and where the size check sits (item 3), whether `send_bytes` is still the only
data-frame sender, the changelog kind for item 3, and every disagreement between the two reviewers. The user will ask
for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. If an item needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md`
(Executing, step 7). Because the PRs form one linear stack, later items sit on top of the blocked one: finish the PRs
before it, record the question, and close the plan as blocked rather than building past it. If current code or specs
have moved so that a recorded decision no longer applies, that is a question for the user too, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself.

## Done criterion

The plan is complete when all three draft PRs exist as one linear stack on the plan stack's tip, each satisfies its
ledger entry's Completion criteria as refined by the plan-time decisions, PRs 1 and 3 have passed both reviews, each has
updated its own `TRIAGE_OUTCOMES.md` Execution field and removed its queue item, and PR 3 has marked this plan's
`plans/INDEX.md` line `[executed]`. Open, not merged: merging is the user's job. Then close the plan per
`plans/AGENTS.md` (Executing, step 8): write its report, write a closing entry in its log, and stop the watchdog.
