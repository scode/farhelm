# Stop losing conversation-identity reports: no lock time limit, a longer hook budget, and retries

Written against main at f9b96fc5 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a SPEC_impl.md sentence that the user decided to change.

This plan has no dependency on another plan. Two neighbors touch nearby ground; read what they landed (if they have) so
your text does not contradict it:

- The earlier `triage-confirm-ssh-identity` plan, now landed, rewrote SPEC.md "Durability and resume" toward report-only
  conversation identity (#1444) and changed Claude's resume selector handling (#1458).
- TODO.md's `Near term` entry "Remove heuristic conversation-identity fallbacks" (no plan yet) removes Claude's record
  scan. This plan does not depend on it: the scan never takes the lock this plan is about.

## The goal

When an agent starts a conversation (at launch, after `/clear`, on a resume inside the agent), Farhelm's hook tells the
session's supervisor the conversation's ID, and that ID is what Resume reopens. Today a valid report can be lost in two
ways, and Claude never reports again until its next session start, so Resume reopens the wrong conversation:

1. **The supervisor gives up on a busy lock.** Report admission waits at most `CAPTURE_CLAIM_WAIT` (1 s, in
   `crates/farhelm-supervisor/src/service/core.rs`) for the session's `capture_locks` claim and otherwise refuses with
   `Conflict` and writes nothing. Review item: `review_feedback_queue/claude-clear-report-dropped-on-claim-timeout.md`;
   its record in root `TRIAGE_OUTCOMES.md` explains the confirmed chain.
2. **The hook gives up on a restarting supervisor.** `farhelm internal hook` (`crates/farhelm/src/hook.rs`, `report`)
   makes exactly one attempt. A refused connection, a missing socket, or a connection that closes before answering (the
   supervisor exiting mid-request) ends the report.

The fix, as decided with the user:

- Remove the time limit on the supervisor's lock wait for every harness, and log when a wait is slow.
- Move Claude's sender check (the hook was run by the session's pane process or its direct child) before the lock, so it
  runs while the hook is certainly still connected.
- Raise the hook's own budget from 2 s to 30 s on every path, Goose included, and the outer kill timers Farhelm controls
  or documents for Claude, Codex and Grok to 60 s.
- Make the hook retry when no supervisor process is on the other end (a refused connection or a missing socket, for at
  most about 4 s), and wait up to the full 30 s whenever a supervisor is there; never retry a refusal.
- Leave the Pi and OMP reporter plugins unchanged.

Acceptance criteria:

- Two draft PRs exist, stacked in the order below (see PR discipline).
- No report path refuses because the lock was busy. A Claude, Goose or Pi report is applied once the lock frees however
  long that takes; a Codex, Grok or OMP report is applied as long as its hook is still there when the lock frees (about
  30 s, U3). A regression test holds the lock past the old 1 s limit and shows the report landing (Claude, and at least
  one of Codex, Grok or OMP).
- Inline and doc comments at the changed code carry the reasoning behind each decision, as the per-PR outline lists, so
  future readers do not undo it.
- The hook keeps reporting across a brief supervisor absence (about 4 s with nothing on the other end of the socket),
  gives up quickly when no supervisor comes back, waits up to 30 s for a supervisor that is there, and never resends a
  request the supervisor refused; tests cover each.
- Every place that states the old numbers or the old "the reporter retries / the refresh pass converges the row"
  rationale is corrected: code comments, SPEC_impl.md, the website docs.
- Each PR adds a changelog fragment.
- PR 2 also removes the TODO.md entry "Apply identity reports that arrive while the session's record is busy", removes
  the feedback file and its line in `review_feedback_queue/INDEX.md`, updates that item's Execution field in root
  `TRIAGE_OUTCOMES.md` to `complete` (change IDs, bookmarks, PR URLs). Record the change ID and bookmark before creating
  the PR, then add the URL to the same change and push again; no separate bookkeeping PR.
- Both PRs have passed the review gate. No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request:** "let's use planning system to queue up late identity reports", for the TODO.md `Near term` entry
"Apply identity reports that arrive while the session's record is busy".

**The user's decisions (2026-10-02),** reached after walking through why the time limit exists:

- U1. The time limit on the lock has no purpose beyond diagnostics. In the user's words: "there is no reason to even
  have a timeout on the lock acquisition besides for DIAGNOSTIC purpose (like timeout, log it, retry, so if it happens
  there is better evidence). This is just on-disk and in-memory stake keeping that is contended with; this lock is not
  held while waiting on some remote network I/O". Remove it. SPEC.md "Healthy local filesystems" (do not add complexity
  to bound local filesystem hangs) and "Slow hosts" already say so; the fix is consistent with them.
- U2. "sender check first makes sense": Claude's foreground attribution moves before the lock.
- U3. Codex, Grok and OMP keep their under-lock order unchanged; only the time limit goes. Their admission reloads the
  saved binding under the lock, may carry part of it forward (Codex's persistent thread), verifies the vendor record
  between two foreground attributions, and commits only if the binding is unchanged. Moving those checks out of the lock
  would turn contention into a refused commit, which is the bug again. With the hook's 30 s budget, the attributions
  only fail if the lock is held about 30 s, which the healthy-filesystem rule accepts. No extra early check for them: it
  would change nothing.
- U4. "Change the 5 second claude level timeout to 60 seconds. Change the timeout in the hook itself to 30 seconds. This
  should be sufficient so it basically never hits, and not _completely_ insane when it does." Applied to "all harnesses.
  and goose too."
- U5. "make sure while we're here that the hook doesn't just silently fail if the supervisor happens to restart (race)":
  add a basic retry.
- U7. "decision 2: leave alone": the Pi and OMP reporter plugins (`crates/farhelm-supervisor/assets/*.ts`) keep their 2
  s kill timer. Changing their bytes forces a new published file name, and OMP's admission refuses reports from a
  session launched with the previous plugin, so every running OMP session would lose conversation tracking until
  relaunched. Both report at the end of every turn, so a lost report heals on the next turn anyway. They still get the
  hook's retry, cut short by their own timer.
- U8. The retry window, settled over several exchanges. On a Unix socket a successful connect means a live process holds
  the listening socket; a refused connect or a missing socket file means nothing is there. In the user's words: "if
  thigns are not BROKEN we do want to wait. only if the kernel basically tells us there is no process on the other end
  should we timeout more aggressively." So: a refused connect or a missing socket is retried for at most about 4 s from
  the first such failure, then the hook gives up quietly; once a connect succeeds, the hook waits up to the full 30 s; a
  connection that drops after connecting goes back to the reconnect rule; nothing goes past 30 s in total. The 4 s cap
  exists because the hook runs inside a session whose supervisor can be down for a long time (the Mac app quit while
  sessions keep running, per SPEC.md's Supervisor definition), and unattended turns still trigger hooks: Grok's on every
  prompt and reply, Claude's on compaction. It also keeps sessions launched before the upgrade, which keep their old 5 s
  Claude and Codex kill timers, from being killed mid-retry when no supervisor is there. Removing the socket file on
  clean shutdown to tell "down" from "restarting" was considered and rejected: a restart is a clean shutdown too, so it
  would make every restart look like "down".
- U6. Review gate: one fresh-context Opus 5.5 reviewer at high effort, no swarm. Two PRs. No-workhorse mode (which
  `plans/AGENTS.md` requires for every plan anyway).

What was considered and dropped:

- A retry on the supervisor's "not recording right now" answer. That answer does not mean "shutting down": it comes from
  a supervisor that lacks its state directory's claim or could not read the boot id (`may_record`, set by
  `reload_sessions`), which does not clear within seconds. A restarting supervisor shows up as a refused or dropped
  connection, which U8 already covers.
- The TODO entry's earlier design (reply to the hook first, commit later on a supervisor-owned task) solved a problem
  that does not exist. Its premise, that a longer wait shows the user a hook error, is wrong: the hook exits 0 silently
  when its own budget runs out, well inside the vendor's kill timer. The real constraint was only that Claude's sender
  check needs the hook process alive, which U2 and U4 address.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Harness-specific code, which
puts per-harness behavior where the module docs of `crates/farhelm-supervisor/src/agent_kind/mod.rs` say and forbids
`kind == X` branches in shared code; Releases and the changelog; TODO.md; review_feedback_queue/; Sharing the machine
with other agents; Agent scratch space; The live install is off-limits), `plans/AGENTS.md` (Executing),
`review_feedback_queue/AGENTS.md`, `.agents/test-authoring.md` for test changes, and `docs/test-sleep-check.md` for any
delay a test needs.

**Planner proposals** are the mechanisms below that the decisions do not name. A fresh-context planning review checked
them for unnecessary scope and against the code; its notes are folded in.

## Per-PR outline

Line numbers drift; find the code by name.

### PR 1: the supervisor applies reports however long the lock is busy (`fix:`)

- Today: `report_conversation_legacy` (Claude, Goose, Pi) and `report_codex_conversation`, `report_grok_conversation`,
  `report_omp_conversation` (`crates/farhelm-supervisor/src/service/core/vendor/`) all take the claim through
  `KeyedLocks::claim_before` with `CAPTURE_CLAIM_WAIT`. Other holders of the same per-session claim are the background
  `refresh_report_only_captures` pass (`service/capture.rs`) and `refresh_reported_capture` (Codex and Grok resume
  offers). Nothing under the claim waits on the network.
- One small helper for the report paths (for example `claim_capture_for_report`): the unbounded `claim`, then a warning
  naming the session and the wait when it took more than a second. Measure after `claim` returns; no timer task. Use it
  at all four sites and delete `CAPTURE_CLAIM_WAIT`. Keep `KeyedLocks::claim_before`: the agent-request fence in
  `service/handlers.rs` still uses it. Rewrite its doc comment for that remaining caller, since it currently justifies
  itself in report-path terms.
- Claude: run `attribute_claude_report` before taking the claim, while the hook is certainly still connected. The entry
  snapshot has no pane or tmux name, so read the row before the claim and refuse early if its generation or kind differs
  from the fenced one. Move the cheap shape check (`accepts_reported_conversation`) ahead of the attribution so a
  malformed report still answers `InvalidRequest` without any tmux or process inspection, as the legacy docs promise.
  Keep the reload and generation/kind comparison under the claim, and the generation-fenced write
  (`record_reported_conversation`), which is what makes a check done before a relaunch harmless. Keep the per-kind
  decision an exhaustive `match kind`, never `kind == Claude`. Note in a comment the small new race this opens: two
  Claude reports milliseconds apart now attribute concurrently before reaching the first-come lock, so a slower older
  one could commit second. Two session starts that close together are not a realistic sequence; no mechanism for it.
- Codex, Grok, OMP: unchanged order (U3), only the helper.
- Correct the text that justified the limit or promised a retry, where it is about the report path:
  `CAPTURE_CLAIM_WAIT`'s docs (deleted with it), `claim_before`'s docs, `report_conversation`'s five-step list ("the
  bounded capture claim"), the vendor paths' "Step 2: the bounded capture claim" comments, and the passages that say a
  lost report is retried or converged (core.rs around the publication-gap discussion: "no retry queue ... another
  lifecycle event may send a fresh report"). The lifecycle-claim rationale (a report must not queue behind the restart
  that caused it) stays. Comments that only state the hook's 2 s budget belong to PR 2.
- SPEC_impl.md, "Shared attribution framework and the five-step admission": replace "at most a second of waiting on the
  report path" and the closing "The timeout bounds lock acquisition, not the entire admission operation." with the new
  rule (the claim wait has no time limit and a slow wait is logged), and make the Claude legacy-admission note in the
  same section say the positional check runs before the capture claim. Nothing else in SPEC_impl.md changes.
- Test: hold a session's capture claim, send a report with a new ID, wait until the report is parked on the claim with
  `KeyedLocks::claims_reached_for_test` (no sleep), advance paused tokio time past 1 s or otherwise show the old limit
  would have fired, release, and assert the row and the in-memory capture name the new ID. One Claude case and one
  Codex, Grok or OMP case. The planning review found no existing test that pins the timeout `Conflict`; if you find one,
  rewrite it and say so in the log.
- The user requires inline comments and doc comments that carry the reasoning, so a future reader of the admission code
  does not reintroduce a limit or move the check back: at the report-claim helper, why the wait has no time limit
  (everything under the claim is local; SPEC.md "Healthy local filesystems"; a limit only ever lost valid reports, since
  nothing resends them) and why the slow wait is logged (diagnostic evidence); at Claude's admission, why attribution
  runs before the claim (it needs the reporting hook process alive, and the hook may have left by the time a long wait
  ends) and why that is safe (the generation-fenced write); at the Codex, Grok and OMP paths, why their checks stay
  under the claim (U3).
- The PR description says that until PR 2, the hook still gives up at 2 s, so Codex, Grok and OMP can still lose a
  report held that long, while Claude's now lands after the hook has gone.
- Changelog fragment `kind: fixed`: after `/clear` (or another conversation switch) on a busy machine, Resume could
  reopen the previous conversation.

### PR 2: the hook waits longer and retries (`fix:`)

- Budget: the `Hook` arm's `BUDGET` in `crates/farhelm/src/main.rs` and the `GooseHook` arm's literal both become one
  shared 30 s constant. Keep the hook's other contracts in `hook.rs` intact: silent, exit 0 always, one budget covering
  stdin and the round trip, exactly one hook-log line per run carrying the final outcome. Decide which word an exhausted
  retry logs (`connect-failed` with the last detail is the smallest change) and whether the line notes the attempt
  count.
- Outer kill timers to 60 s, for the harnesses whose timer Farhelm sets or documents:
  - Claude: `hook_argv` in `crates/farhelm-supervisor/src/agent_kind/mod.rs` (`"timeout": 5`) and its doc comment.
  - Codex: the injected `hooks.SessionStart=[...timeout=5...]` in the same file.
  - Grok: users configure its hooks by hand. `website/src/content/docs/docs/agents/grok.md` shows `"timeout": 3` three
    times; change them to 60 and add a sentence telling existing users to raise theirs.
  - Goose: Farhelm sets no timer on its `--with-extension` reporter, so only the hook budget changes.
  - Pi and OMP: unchanged (U7).
  - The tests that pin the old values (`claude_hook["timeout"]`, the Codex TOML check) move with them.
- Retry, per U8, in or around `report` in `hook.rs`:
  - A connect that fails because nothing is listening (refused, or no socket file): retry with a short pause (about 100
    ms, growing to about 1 s) for at most about 4 s from the first such failure, then give up.
  - Once a connect succeeds, the remaining 30 s budget applies to the handshake, send and reply.
  - A connection that drops after connecting (EOF, reset or broken pipe during handshake, send or reply, or closed
    before answering): reconnect under the connect rule, never past the overall budget.
  - Never retry an `Error` reply of any kind, `Unauthorized` included, and never retry a handshake that failed because
    the peer refused it (a protocol-version mismatch, which is exactly what an old hook meets after an upgrade). The
    handshake's `Outcome::io` mixes the two today; classify by `io::ErrorKind`.
  - `report` takes the credential by value on the assumption of a single round trip; a retry needs it cloned. Update its
    doc comment.
  - Resending after an ambiguous failure is safe: Claude, Goose and Pi rewrite the same ID, and Codex, Grok and OMP
    rerun admission against the stored binding, which a repeat preserves. Confirm this for Grok's ordered selection
    before relying on it.
- The user requires inline comments explaining the retry logic's reasoning where it lives, so it is not lost to future
  readers of the hook: why a refused connect or a missing socket means no supervisor process is there while a successful
  connect means one is (Unix socket semantics, and the supervisor leaving its socket file behind when it exits); why
  that case gets only about 4 s and a live supervisor the full 30 s (U8: unattended turns in a session whose supervisor
  is down, and old sessions' 5 s kill timers); why "down" and "restarting" cannot be told apart and why removing the
  socket on shutdown would not help; why a refusal and a handshake version mismatch are never retried; why resending is
  safe; and where the 30 s and 60 s numbers come from (U4: rarely hit, bounded when hit, the outer timer comfortably
  above the hook's own). At the Pi and OMP reporter plugins' `timeout: 2000`, or at their asset constants in
  `pi_extension.rs` if comments in the assets would change their bytes, explain why they deliberately keep 2 s (U7).
  Comments must not change the asset files' bytes.
- Comments and docs that state the 2 s budget or "no retry": `hook.rs`'s module docs (contract item 3, the outcome
  table, `report`'s docs), main.rs's `BUDGET` comment, `agent_kind/mod.rs`'s Claude timeout doc, `service/handlers.rs`
  ("a hook that has a 2 s budget and no retry", and the test docs saying "blow the hook's 2 s budget"),
  `service/capture.rs` ("a hook with a 2 s budget"), and `report_conversation`'s lifecycle-claim passage in core.rs
  ("blow the hook's own 2 s budget"). On the website: `docs/agents/agent-hook-injection.md` (the "two seconds" passages,
  the OMP paragraph's "two seconds", the `connect:` and `timeout` hook-log entries and "nothing retries it") and
  `docs/agents/grok.md` ("gives up its Farhelm round trip after two seconds"). Follow `website/AGENTS.md` and
  `website/EDITORIAL_RULES.md`. Search for anything else naming these numbers.
- Tests that run the real hook binary assume the 2 s budget and will break or take 30 s:
  - `crates/farhelm/tests/e2e/hook_identity.rs` has a 4 s `SILENCE_DEADLINE` ("past the 2 s budget, under the 5 s vendor
    timeout"). Its no-supervisor tests now take up to the ~4 s connect cap, and its silent-supervisor test would wait 30
    s. Give the real binary a test seam for the budget passed on the child's own command line or environment (never the
    test process's environment), or rewrite the deadlines; log the DECISION.
  - `crates/farhelm-fixtures/src/fake_agent.rs`'s `HOOK_CHILD_DEADLINE` (10 s, documented as "generously past the hook
    binary's own 2 s internal budget") now sits under the hook's budget; fix it and its doc.
- New tests in `hook.rs`'s fake-supervisor harness, deterministic, no real sleeps (any deliberate delay needs a
  `sleep-ok` reason per `docs/test-sleep-check.md`): a connection accepted and closed without answering, then answered
  on the next attempt; a socket with nothing listening that gives up after the connect cap; a refusal answered exactly
  once with no second connection; a handshake version refusal not retried.
- Changelog fragment `kind: fixed`: conversation reports now survive a supervisor restart and a slow moment. Say that
  Grok users should raise their configured hook timeout to 60, and that sessions started before the upgrade keep their
  old Claude and Codex hook timers until relaunched.
- Bookkeeping, in this PR: remove the TODO.md entry, the feedback file and its line in `review_feedback_queue/INDEX.md`;
  in root `TRIAGE_OUTCOMES.md`, add a Decision line to the item's record saying the 2026-10-02 decisions above supersede
  the plan-time deferral (unbounded claim, Claude's check first, 30 s and 60 s timers, retry per U8, Pi and OMP plugins
  unchanged), and set its Execution to `complete` with both PRs' change IDs, bookmarks and URLs.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-identity-report-wait-retry-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution field) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md`
(Executing one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/identity-report-wait-retry/<nn>-<short-name>`.
- PR 1 then PR 2, one commit, bookmark and draft PR each. Within this run, if a PR needs correcting, restructure it
  rather than stacking a correction on top, and do not add code in PR 1 that PR 2 deletes.
- Commit messages and PR titles use Conventional Commits; both PRs are `fix:`. Each adds its changelog fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for Rust test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- PR 1: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D
  warnings`, and focused nextest selections for the new tests plus the existing
  report-admission and capture tests in `farhelm-supervisor` (unit tests in `service/core.rs` and the vendor modules,
  and the e2e suites that exercise Claude, Codex, Grok and OMP reports; find them by name).
- PR 2: the same formatters and lints, `hook.rs`'s unit tests, the `agent_kind` unit tests, `hook_identity.rs` and the
  other e2e tests that launch a hooked agent through the real `farhelm internal hook`, `dprint check` on changed
  Markdown, `python3 releasing/check-changelog.py format`, and the website build
  (`cd website && bun install
  --frozen-lockfile && bun run build`) because pages changed.
- Any PR that changes Rust tests or their helpers: run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate one review of that PR's changes. The user
demands a fresh-context agent on Opus 5.5 at high effort (U6), shelled out to the other harness if the executing one
cannot reach that model natively. No review swarm. The prompt carries the full charter, because the reviewer has nothing
else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria for that PR: its section above, The goal, and decisions U1-U8.
For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Address what the reviewer finds before moving on, and log the DECISION where you decline a
finding. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a background commit task, a queue of pending
reports, a new protocol message or error kind, restructuring the Codex, Grok or OMP proofs, a marker telling the hook a
supervisor stopped for good, any change to the Pi or OMP plugins; these are examples, not a blacklist), and whenever the
same component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with this
charter, supplying the request, the user decisions above, this outline, the current diff and the proposed departure
(what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the report-claim helper's shape, how the hook classifies each failure as
retryable or final, the retry pacing and the exact connect cap, the hook-log word for an exhausted retry, how the e2e
tests control the real binary's budget, and every reviewer finding you declined. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. Agreed
fallback: if the real binary's budget cannot get a clean test seam, rewrite the affected e2e deadlines around the real
budget and connect cap instead, keeping the suites' runtime reasonable. Anything else that needs a decision: record the
concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10). Because the PRs form one linear stack,
if PR 1 blocks, do not build PR 2 on top of it.

## Done criterion

The plan is complete when both draft PRs exist as one linear stack, each satisfies its section above and the acceptance
criteria, both have passed the review gate, and PR 2 has done the TODO, review queue and triage ledger bookkeeping.
Open, not merged: merging happens only after the maintainer has reviewed this plan's report. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
