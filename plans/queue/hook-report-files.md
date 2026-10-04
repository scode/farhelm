# Hook report files: conversation hooks write a file instead of calling the supervisor

Written against main at 255145a0 on 2026-10-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file directs a change to them.

This plan runs after `plans/queue/launch-representation.md` (its `INDEX.md` line says so). The launch redesign rewrites
hook injection (to placeholder expansion with `{farhelm_args}`) and moves reporter settings into the launch
specification, in code this plan touches. Names below come from main at 255145a0 and some will have moved; read what
that plan landed before starting. `plans/queue/session-notifications.md` is meant to land after this plan, because it
turns some report refusals into user-visible notifications at the refusal sites this plan moves; it carries no formal
dependency (it was planned first), so picking's conflict check is what keeps the two from running at once. Its file
already describes what this plan changes.

## The goal

Farhelm learns which agent conversation belongs to a session from the agent itself: a per-launch hook,
`farhelm internal hook --vendor <adapter>`, receives the agent's payload on stdin and reports it. Goose's MCP reporter
and the Pi and OMP extension assets deliver their reports through the same hook. Today the hook reports over the
supervisor's socket (`supervisor.sock`) and waits for the reply, which drags in a pile of timing machinery: a 30 s
budget, about 4 s of retries against an absent supervisor, reconnect windows, a reply probe, and outer timeouts the
vendors' hook runners impose (60 s now, 5 s on sessions launched by older builds). And when the supervisor is not
running, reports are lost. On the Mac that is the normal case whenever the desktop app is closed, because the app hosts
the local supervisor: quitting the app stops it while sessions keep running, so every report made while the app is
closed (Claude's `SessionStart` on compaction, Grok's per-turn reports, Pi's per-turn reports) costs the agent a few
seconds of retries and is then dropped.

After this plan, the hook writes its report as a file into a per-session drop directory under the supervisor's state
directory and exits. It never touches the socket. The supervisor picks the files up on its existing periodic
reconciliation pass and runs them through the same acceptance logic as today. Reports made while the supervisor is down
wait on disk and are applied when it comes back.

The maintainer's words, from the planning conversation: "what if we instead have the hook simply deposit the latest
session in some well known state location on disk that the supervisor reads. this way ALL of the timeout complexity goes
away, AND things keep working regardless of whether the supervisor runs. no retry loop in the hooks, no slowness issues,
etc."

Acceptance criteria:

- `farhelm internal hook` (every vendor, including the Goose, Pi and OMP paths that call it) reads its payload, writes
  its report atomically (written under a temporary name, then renamed into place) into a fixed slot file in the
  session's drop directory, and exits. No socket connection, no retry or reconnect loop, no reply wait. The bounded
  stdin read, the always-exit-0 and panic-silent behavior, the hook log line, and the `--announce` pointer stay as they
  are (SPEC.md "Anything farhelm attaches to an agent launch must be invisible...").
- The supervisor applies dropped reports on its existing reconciliation pass (`capture_now` on the ticker in
  `crates/farhelm-supervisor/src/service/ticker.rs`, every 2 s) and on the reply paths that already run that pass,
  including supervisor startup and Restart, which already run it before choosing a conversation. A processed file is
  deleted after a definitive acceptance or refusal, and kept for a later pass after a transient failure. Every
  acceptance rule that applies today still applies: kind-versus-vendor, sub-agent refusal, foreground attribution for
  Claude, Codex, Grok and OMP, the Codex, Grok and OMP record verification and compare-and-swap admission, Grok's
  selection-then-enrichment ordering, OMP's asset check, Pi's locator handling, size caps, and generation checks.
  Refusals are logged by the supervisor with the reason the old reply carried.
- A report written while the supervisor is not running is applied when a supervisor next runs that session's
  reconciliation, provided the launch it reports about is still the session's current launch. A report about an earlier
  launch is discarded.
- The socket report path is removed: the `ReportConversation` control message and its handler, the hook's socket client,
  and the code and tests that existed only for the round trip. Already-running sessions keep working because their
  injected command lines are unchanged and run the new binary (SPEC_impl.md "What running sessions hold across
  versions"); update that section for what newer binaries now keep accepting.
- SPEC_impl.md's per-launch identity hook section, and every other place in SPEC.md or SPEC_impl.md that describes the
  socket report, its budget or its retries, describe the file drop instead.
- SPEC.md states, as expected behavior for now, that while a host's supervisor is not running (on the Mac, whenever the
  desktop app is closed) the agents' `farhelm` commands that need it (`farhelm spawn` and the `farhelm agent` verbs)
  fail, while sessions keep running and conversation tracking keeps working.
- User docs match: `website/src/content/docs/docs/agents/agent-hook-injection.md` (its retry, socket, outcome and
  hook-log descriptions), `website/src/content/docs/docs/agents/grok.md`, `agent-wrappers.md`, any other page the change
  makes wrong, and the "Close Farhelm and come back" section of
  `website/src/content/docs/docs/get-started/first-session.mdx`, which gains the caveat about agents' `farhelm`
  commands. Follow `website/AGENTS.md` and `website/EDITORIAL_RULES.md`.
- The last PR removes the TODO.md entry "Verify that agents keep running while Farhelm is closed on the Mac".

## Decisions already made

Settled with the maintainer during planning. Do not reverse them.

1. **File drop replaces the socket entirely.** No fallback to the socket, in the hook or the supervisor: a fallback
   would bring back the timing machinery this plan exists to remove.
2. **Upgrade gap accepted.** A supervisor older than the hook binary (a remote host mid-update, the Mac between install
   and relaunch) may briefly miss reports. Accepted by the maintainer; build nothing for it.
3. **Not a security boundary; keep honest-mistake protection.** Attribution stays what SPEC_impl.md already calls it,
   "attribution under inherited credentials, not a security boundary against the same Unix user". It still has to stop
   honest mistakes: a native sub-agent, a nested `claude -p`, or a shelled-out child replacing the session's
   conversation. So the protection survives, but the trust anchor changes (see the outline). On the per-session
   credential the maintainer said: "if it doesnt add complexity lets do it because it nicely avoids mistakes too but
   dont spend complexity on it (besides tiny amounts)". The planning review found that carrying it in the file would
   prevent none of those mistakes, since sub-agents and nested agents inherit the same credential, while adding an
   on-disk copy of it. So the report file does not carry the credential; the hook keeps today's rule that a launch
   environment without the credential reports nothing.
4. **Pickup is the existing ticker; no file watcher.** The supervisor reads drop directories on its existing 2 s
   reconciliation pass. Faster, inotify-style pickup is a separate Near term TODO entry, not part of this plan.
5. **The agents' `farhelm` commands not working while the supervisor is down is accepted for now** and written into
   SPEC.md as expected behavior. A supervisor that outlives the Mac app is a separate Maybe later TODO entry.
6. **Mac verification goes to the maintainer.** Executors run on Linux and cannot check the Mac. The report includes a
   short manual check, designed by you, for the maintainer to run on a Mac during review. At minimum it shows that
   sessions keep running across quitting and reopening the app, and that a conversation report made while the app was
   closed is applied after it reopens, so Restart resumes the right conversation. With the app closed the user cannot
   type into a session, so work out a practical way to make a report fire then, and say how in the steps.
7. **Review gate:** two fresh-context reviewers per PR, Claude Opus 5.5 at high effort and gpt-6-astra at high effort,
   no review swarm (How to run, Review gate).
8. **No-workhorse mode** (How to run).

## Implementation outline

Planner proposals unless marked as a decision. Grounded in main at 255145a0; verify against what the launch redesign
landed.

**Today.** The hook client is `crates/farhelm/src/hook.rs` (`run_with` → `run_inner` → `read_payload`, per-vendor
`parse_payload`/`parse_grok_payload`, then `report` with `report_once` retries), dispatched from `InternalCmd::Hook` and
`InternalCmd::GooseHook` in `crates/farhelm/src/main.rs`; its environment comes from `SessionEnv` in
`crates/farhelm/src/agent_client.rs` (`FARHELM_SESSION_ID`, `FARHELM_SESSION_TOKEN`, `FARHELM_SUPERVISOR_SOCK`; the
state directory is the socket's parent, as `hook_log_path` already derives it). The supervisor side is the
`ReportConversation` arm in `crates/farhelm-supervisor/src/service/handlers.rs` and `Supervisor::report_conversation` in
`service/core.rs`, with `report_conversation_legacy` (Claude, Goose, Pi) and the ownership-proof paths in
`service/core/vendor/{codex,grok,omp}.rs`, and attribution in `procs.rs` and `procs/*.rs` (`walk_to_pane`,
`is_hook_invocation_argv`, `is_hook_trampoline`, the per-vendor corridors). The accept loop captures the peer's pid and
start token (`ProcessIdentity::read`) for that attribution. Reconciliation is `capture_now` →
`refresh_report_only_captures` in `service/capture.rs`, run by the ticker and by reply paths.

**Report files (reviewed proposal).** Fixed slot files per session, each replaced by an atomic rename, in the
maintainer's sense of "deposit the latest session". Every vendor but Grok has one `latest` slot: each report carries a
complete identity and is checked against the session's stored binding (Codex's thread-id carry-over reads the previous
binding from the store), so skipping intermediate reports gives the same result. Grok is the one vendor that depends on
order: `GrokLocator::merge_report` refuses a `UserPromptSubmit`/`Stop` enrichment that arrives before a `SessionStart`
selection. So Grok has a `selection` slot and an `enrichment` slot, applied selection first; a stale enrichment for an
older conversation is already refused harmlessly. That bounds a session's drop directory at two files with no cap,
eviction or ordering scheme; do not replace it with a queue of files, because a capped queue that drops the oldest
entries would evict Grok's selection during a long supervisor outage and lose the conversation. Location: a per-session
directory under the state directory, next to `hook-log/`; the hook creates only that directory (private permissions),
never missing parents of the state directory. The temporary file lives in the same directory under a prefix the
supervisor ignores. The file holds the parsed report (vendor and the payload fields the supervisor uses today) and the
attribution evidence below.

**Attribution (proposal).** The live peer is gone by the time the supervisor reads the file, so the hook records its own
ancestry at report time: for each edge up to the existing 64-edge bound, the pid, start token, executable and bounded
argv that `walk_to_pane` reads today. The supervisor runs the existing classifiers (`is_hook_invocation_argv`,
`is_hook_trampoline`, the Claude, Codex, Grok and OMP corridors) over that recorded chain instead of over live
processes, re-applying the per-process and per-walk argv budgets on read since the file is now input to the supervisor.
The chain must contain the session's current tmux pane process (`pane_process`): by pid and start token while that
process is alive, by pid alone when the pane is dead but still listed (`remain-on-exit`). That check also ties the
report to its launch: every launch has a new pane process, so a report from an earlier launch no longer matches and is
discarded, with no new environment variable and no new stored state. Do not add a column recording the pane's identity;
pid reuse is an honest-mistake risk at the level the repository already accepts. A report whose pane is gone entirely
(tmux killed, for example by a Mac reboot) cannot be matched and is discarded; that is accepted. Split `procs.rs`'s walk
into "collect the chain" (used by the hook) and "classify a chain" (used by the supervisor) rather than writing a second
classifier. The double walk around `verify()` existed to catch the emitter changing during verification of a live
report; with recorded evidence it is no longer meaningful, so drop it unless something else depends on it. A report from
an agent process that has since exited is accepted when its chain matches as above, since it was true when made;
`owned_pane_pid` refuses a dead pane today, so the read path needs a variant that allows it. The `farhelm` crate already
depends on `farhelm-supervisor`, so the hook can call the collect half directly.

**Supervisor read path (reviewed proposal).** Extend `capture_now`, which the ticker, the reply paths, supervisor
construction, the reload path and `restart` already run, so waiting reports are applied before Restart picks a
conversation without new plumbing. For each session with a reporting integration, drain its drop directory BEFORE
`refresh_report_only_captures`' filter that skips rows whose `conversation_source` is not `hook`: a session's first
report has no hook source yet, so a drain placed after that filter would never apply it. To process a slot, first take
ownership by renaming it to a private name in the same directory, so a hook replacing the slot meanwhile does not have
its newer report deleted; then apply it through the existing per-vendor acceptance code (fed the recorded evidence
instead of a live peer). Do not drain at all while the supervisor is not recording (`may_record()` false: replaced or
shutting down). Delete the file after acceptance or a definitive refusal; on a transient failure (a store error, a pane
that could not be inspected) put it back for a later pass unless the slot has been refilled meanwhile, in which case the
newer report wins. Reports for a session that is not yet published (the publication gap) simply wait. The same pass
removes stale temporary files and drop directories whose session no longer exists (a hook racing session deletion can
recreate one), and session deletion removes the drop directory alongside `hook-log/<id>.log` and the launch artifacts
(`service/teardown.rs`, `launch_artifacts.rs`). The 65 s "no conversation identity" tripwire
(`report_liveness_tripwire`) keeps working with up to one tick of extra latency.

**Removal.** The `ReportConversation`/`ConversationReported` protocol pair, its handler arm, the hook's socket client
and retry machinery (`report`, `report_once`, `RetryConfig`, `CONNECT_RETRY_CAP`, the reply probe), the accept loop's
peer capture if nothing else uses it, and the tests that existed only for the round trip. Check whether removing the
message needs a protocol version bump under the repository's protocol rules and log the DECISION. Rewrite attribution
tests against recorded chains (simpler than today's live-process fixtures where the property allows) and keep e2e
coverage of a real hook reporting through the fake agent (`crates/farhelm-fixtures/src/fake_agent.rs`), including a
report written while no supervisor is running that is applied once one starts, and Grok's selection surviving later
enrichments made while the supervisor is down.

**Size.** Medium-large: the bulk is the collect/classify split and the test rewrites; the hook client shrinks a lot.

**Suggested stack** (adjust as the code dictates, without churn): spec and SPEC_impl changes with the code they describe
rather than a separate spec-first PR unless a spec PR reads better on its own; collect/classify split in `procs` as a
refactor with no behavior change; the file drop (hook writes, supervisor reads, socket path removed) as one PR, since
splitting it would either ship a window where reports go nowhere or build a fallback Decision 1 forbids; docs; TODO
removal with the last PR.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-hook-report-files-log.md`
in the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before
you start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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
  `plan/hook-report-files/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits. The user-visible change (reports made while the supervisor is down are no longer lost, and hooks
  no longer stall the agent for seconds when it is down) is a `fix` or `feat` and carries a changelog fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).
- Changes to tests or fixtures follow `.agents/test-authoring.md`; run `python -B scripts/check-test-sleeps.py` per root
  `AGENTS.md` when Rust tests change.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, nextest selections of the hook module, the supervisor's capture,
handlers, core report and procs modules, and the e2e hook and identity files (`hook_identity.rs`, `codex_identity.rs`,
`wrapper_launch.rs`, and the files whose helpers use hooks), the Pi/OMP asset harness
(`cd crates/farhelm-supervisor/asset-js-tests && node --test`) since those assets drive the hook, `dprint check` on
changed files, and the website build when docs change. Say in the report which checks ran and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands (Decision 7): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra
agent at high effort (shelled out to the other harness if the executing one cannot reach that model natively). No review
swarm. Both get the same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim. Address what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a file watcher, a socket fallback, a new launch
environment variable, a report journal, queue or database table, a second attribution classifier, a protocol for the
hook to learn the outcome of its report; these are examples, not a blacklist), and whenever the same component has
needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with this charter, supplying
the request, the decisions above, this outline, the current diff and the proposed departure (what changed, why it is
necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the report file's location, slot names and format; how recorded evidence is tied
to the launch; which refusals count as transient (file kept) and which as definitive (file deleted); what replaced the
double walk around verification; protocol version handling; which tests were deleted rather than rewritten and why; and
every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The planner proposals in the outline are yours to change when the code argues otherwise,
except that the slot design must not become a capped queue (the outline says why). A change that would lose today's
protection against sub-agents and nested agents replacing the session's conversation, or that would need the socket
after all, is not covered: record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md entry. Open, not merged: merging happens only after the maintainer has reviewed this
plan's report. The report includes the manual Mac check (Decision 6). If a `## Decisions` section exists, its latest
entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver
its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/`
yourself.
