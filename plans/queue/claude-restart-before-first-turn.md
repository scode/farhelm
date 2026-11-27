# Goal: Restart a Claude session before its first turn

NOTE: This is a goal file for an unattended run executed through the planning system (`plans/AGENTS.md`). It is not a
design document: SPEC.md and SPEC_impl.md stay authoritative, except where this file records a decision the maintainer
made that amends them, and then the run updates them to match. The executing session has none of the planning
conversation, so everything it needs is here or in the repository files it names.

## The goal

Restarting a Claude Code session before anything was saved for its conversation must start Claude again instead of
failing. Today it fails like this. Farhelm's injected Claude `SessionStart` hook reports the conversation id as soon as
Claude starts, so Restart is offered a few seconds after launch (`AgentKind::restart_readiness` is `ShortlyAfterStart`).
Claude only writes its conversation file once the first prompt is submitted. Restart runs `claude --resume <id>`, and
Claude exits with `No conversation found with session ID: <id>`, leaving the session with an empty terminal. The
maintainer hit it on two hosts, both times on a fresh session (one right after a Replace).

What the planning session verified against Claude Code 2.1.332, started without Farhelm in a terminal host
(`scripts/agent-tui.py`), in a directory it had already trusted:

- Started and quit without a prompt: no `<id>.jsonl` existed under `~/.claude/projects/<dir>/`, and
  `claude --resume <id>` exited at once with the error above.
- After a prompt was submitted, the file existed about 1.2 seconds later, holding the user record, while Claude was
  still thinking. So the file appears long before the first turn completes.
- `claude --session-id <id>` accepted an id that had been reported but never saved.
- Not shown: resuming after a stop in the middle of a turn. The test turn finished before the stop.

Acceptance criteria:

- A Restart (plain, or Restart with without a named conversation) of a Claude session whose report named a conversation
  file that does not exist runs the session's start command instead of the resume command, and the new Claude reports
  its own conversation through the ordinary path.
- A Restart of a Claude session whose reported file exists resumes that conversation exactly as today. That includes a
  session whose first prompt has been submitted but whose first turn has not finished.
- A Claude capture stored by an older build (a bare id, no file) resumes exactly as today, unchecked.
- Restart stays available from launch for Claude sessions. Nothing polls, watches, or inotify-waits for the file; the
  only new check runs when a Restart runs.
- `/clear` before the next prompt is covered by the same check: Claude reports the new id before saving it.
- SPEC.md, SPEC_impl.md, the affected doc comments and the Claude docs page describe the new behavior, and a changelog
  fragment covers it.

## Decisions already made

These were settled by the maintainer on 2026-11-26. Do not reverse them.

- **Fix 1, not "disable Restart until the first turn".** The maintainer chose checking the file at Restart over making
  Restart unavailable until a turn exists, and pointed out that the fix only differs materially from that alternative
  because it resumes as soon as the file exists, before the first turn completes. They asked explicitly whether this
  meant polling or inotify; it does not, and must not: one check, when Restart runs.
- **This amends a confirmed SPEC rule, on purpose.** SPEC.md's Restart section (around "Confirmed 2026-10-03: Restart
  always means the conversation is preserved") says there is no fresh-restart variant and that a Restart losing the
  conversation is a bug, not a fallback. The maintainer's choice of fix 1 narrows that rule: when Claude saved no
  conversation file, there is nothing to preserve, and Restart starts fresh. Edit that paragraph to state this exception
  and that it was decided on 2026-11-26. Do not present it as a wording fix.
- **What runs when nothing was saved: the session's stored start command**, not `claude --session-id <old id>`. The
  start command works the same for agent launches, command launches and legacy launches, whose resume command need not
  use `--resume` at all. Claude picks a new id and reports it as usual. Restart with uses the replacement launch's start
  command.
- **A missing file after real turns is treated the same.** Farhelm cannot tell "never saved" from "deleted or moved
  later" by looking at the file, and the conversation is gone either way. Restart starts fresh; it does not refuse.
- **No notice.** Nothing tells the user that Restart started fresh. At most a supervisor log line.
- **Real-agent drift check: yes, add it and run it once.** Extend the real-Claude drift test so it actually presses
  Restart before the first prompt and checks the new Claude comes up. That spends no vendor turn. Run that case once on
  the executing host as evidence and report the result; nothing runs it automatically. Do not add a real-agent case that
  restarts mid-turn (timing-sensitive, and it spends a turn); cover that branch with supervisor tests.
- **Review gate:** a fresh-context Opus 5.5 agent at high effort with the general charter (see "Review gate").
- **Release note:** see `## Release note` at the end of this file. It is the maintainer's wording.
- **Delegation mode: no-workhorse** (see "Execution mode").

## Implementation outline and requirement sources

Requirements come from three places, kept apart here so a reviewer can challenge the planner's reading rather than only
check compliance with it:

- **The maintainer's request and decisions** (above): check the reported file at Restart; resume if it exists; start
  fresh if not; no watching; Restart available from launch; start command for the fresh path; no notice; drift case.
- **Repository constraints:** SPEC.md's rule that Farhelm identifies a conversation only from the harness's own report
  and that checking the exact file a report names is verification, not identification (so the path comes from the
  report, never derived from Claude's directory layout); `AGENTS.md`, "Code rules" (per-harness decisions live in the
  places `crates/farhelm-supervisor/src/agent_kind/mod.rs`'s module docs name, with exhaustive matches, never
  `kind == X` in shared code); the convention that every locator prefix joins `RESERVED_LOCATOR_PREFIXES` in
  `crates/farhelm-proto` and is listed in the custom-agent contract page with a contract changelog entry.
- **Planner proposals**, reviewed by a fresh-context planning review and adjusted by it. Each can be revisited through
  "Scope reassessment" below if the code shows a better shape.

The outline, built on existing mechanisms rather than beside them:

1. **Keep the reported file path with the capture, as a Claude typed locator.** Claude's `SessionStart` and `Stop`
   payloads carry `transcript_path`, and the hook already forwards it for Claude (`crates/farhelm/src/hook.rs`); no hook
   or wire change is needed. Encode the locator in the supervisor at admission, in the positional report path
   (`crates/farhelm-supervisor/src/service/core/vendor/positional.rs`), following Codex's precedent of building its
   locator from `transcript_path` server-side. Reuse `SessionLocator`, `encode_locator`, `parse_locator` and
   `validate_locator` from `agent_kind/mod.rs` with a new `LocatorVendor::Claude`, rather than a new database column (id
   and path change together on every report, `/clear` and correction, and the conversation column already carries richer
   identities for Codex, Grok, Pi and OMP). Use a per-policy field on the positional policy so only Claude encodes;
   Muse, Antigravity and custom agents stay plain ids. Validate the raw id with the existing check before encoding. When
   `transcript_path` is missing or fails validation, store the bare id rather than refusing the report: the check
   improves Restart and must never cost capture.
2. **Claude's semantics differ from Pi's and OMP's.** For Pi and OMP the file is the resume selector and is required for
   an offer. For Claude the session id is the selector and the file is only the existence hint. So Claude gets its own
   arms in `IntegrationSnapshot::restart_offer` (Resume for a Claude locator with or without a file, and for a plausible
   bare id) and in `filled_resume_argv` (substitute the session id). `typed_locator_offer` does not fit Claude.
3. **A separate per-kind restart-target decision; Claude stays out of `verify_report_only_resume`.** That function and
   `verifies_resume_target` implement a refusal contract (a failed check withdraws the offer and refuses before the
   stop). Claude's check never refuses and never rewrites capture; it picks between two commands. Either add a new
   exhaustive per-kind predicate beside `verifies_resume_target`, or turn its bool into a three-way per-kind policy at
   its single call site. That choice is a close call; decide it and log a DECISION. If `locator_vendor(Claude)` comes to
   return `Some`, make sure no path sends Claude into the Pi/OMP verification (check the core test fixture keyed off
   `locator_vendor`, too).
4. **Check with the existing open helper, and only NotFound starts fresh.** Use `open_regular_file` in
   `crates/farhelm-supervisor/src/agent_kind/records.rs` as is (bounded, no-follow, distinguishes not found, a regular
   file, and every other error). Do not read or parse Claude's transcript. Not found runs the start command; a regular
   file resumes; any other error resumes as today, without refusing.
5. **Decide after the old agent has stopped.** `restart_session` in `crates/farhelm-supervisor/src/service/core.rs`
   fixes the argv before the stop today. Build and validate both candidate commands before the stop (the same pre-stop
   checks the resume argv gets: custom program expansion, command placeholder validation, `ensure_executable_argv`),
   then make the file check after the stop and reap, before the restart seed is read. Once the old process tree is gone,
   nothing can create the file, so the decision is final and a prompt saved at the last moment is resumed. Because the
   check never refuses, the rule that refusals happen before anything is stopped still holds. Fall back to checking just
   before the stop only if the code shows a concrete obstacle, and record why. Bare-id captures and named restarts skip
   the check.
6. **Pass the launch phase through the relaunch.** `relaunch_into_host` (`service/core/host_restart.rs`) hardcodes the
   resume phase for its shell source and `{farhelm_args}` mode. Carry the phase in the relaunch plan so the fresh path
   uses the start phase (for a command launch that selects its start command text). An initial prompt is never stored
   and a restart already passes none, so no code is needed to avoid re-sending one; keep a test assertion for it.
   Whether the fresh path also clears the old, never-saved capture (the existing `clear_capture` mechanism used for
   named starts) is a close call: clearing avoids a stale target when the new run cannot report, at the cost of Restart
   reading "not captured" for the second before the new report lands. Decide it and log a DECISION.
7. **Readers of Claude's captured value that must keep working.** The reserved `claude:` prefix (with the custom-agent
   contract page, its contract changelog, and the test in `crates/farhelm` that checks the page lists every prefix). The
   drift-test identity helpers (`reported_runtime_identity`, and `turn_identity` in
   `crates/farhelm/tests/e2e/real_agent_capture/drift/capture_timing_hooks.rs`) must understand the Claude locator, or
   every Claude drift run fails on identity matching; that is required work, not part of the optional-looking drift
   case. Confirm that a capture changing from a bare id to a locator for the same conversation on the next report does
   not post a "conversation changed" notification. The fake agent's Claude hooks send no `transcript_path`, so existing
   end-to-end tests keep covering bare ids; new tests add a path.
8. **Specs and docs.** SPEC.md: the confirmed Restart paragraph (see Decisions), the per-session resume promise
   paragraph (its `claude --resume <conversation-id>` example), the paragraph on exact-target checks and missing files
   (Claude's missing file starts the start command, with no withdrawal and no notification; an unreadable file resumes),
   the "Claude requires no transcript check" sentence (still no ownership proof; now a Restart-time existence check;
   bare-id captures stay resumable unchecked), and what `{conversation}` means for Claude (the session id).
   SPEC_impl.md: the Claude capture and `LocatorVendor` passages. Doc comments on `LocatorVendor`, `restart_offer` and
   `relaunch_argv` (which today promises exactly one argv). The Claude page under
   `website/src/content/docs/docs/agents/` (read `website/AGENTS.md` first), including the caveat below.

Risks to state in docs or the report rather than design around:

- The check assumes the supervisor sees the same filesystem Claude does. A wrapper that runs Claude with a different
  view of its home (a bind-mounted `~/.claude`, say) while keeping its process ancestry visible would turn every Restart
  of a saved conversation into a fresh start. Document this on the Claude docs page.
- A transcript Claude's own retention cleanup removed now starts fresh silently, where today Claude prints an error.
  That matches the maintainer's decision.

Suggested stack, to adjust if the code argues otherwise (restructure rather than stack corrections):

1. Capture Claude's reported file with its id (locator, reserved prefix and contract page, offer and resume arms, drift
   identity helpers). No behavior change to what Restart runs.
2. Start fresh when Claude saved nothing (phase plumbing, the after-stop check, supervisor unit tests for present,
   absent, error and after-stop, a fake-agent end-to-end test whose Claude hook sends a path, SPEC and docs, changelog
   fragment).
3. The real-Claude drift case that restarts before the first prompt, run once. This PR, as the last code PR, removes the
   TODO.md entry "Restarting a Claude session before its first turn fails." (`plans/.agents/planning.md`: a TODO plan's
   last code PR removes the entries it covers). If the stack ends up shaped differently, the last code PR removes it.

## Scope reassessment

Before implementing a substantial departure from the outline (a new parser, a new database column, a recovery protocol,
a compatibility layer, a mechanism that does the job of an existing one, or a generalization the outline did not plan;
these are examples, not a list), run a fresh-context review at your own model and effort. Supply the request, the
decisions above, the outline, the current diff, and the proposed departure: what changed, why it is necessary, and which
simpler alternative was ruled out. Give it this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call within the agreed scope (including the two close calls named in the outline) is a
routine fork: decide it after the reassessment and log it as a DECISION. Repeated corrective reviews of the same
component also require reassessing its necessity before another repair round. The same decision boundary as "Unattended
fallback" applies. Same-model sub-agents you run get bounded units within the outline; an unapproved new mechanism found
mid-unit is left unimplemented and reported back, with its checks and incomplete work stated honestly.

## The checkout

This goal runs inside a checkout of the GitHub repository `scode/farhelm`: the nearest Git worktree or jj workspace
containing the session's working directory (a `.git` file counts as well as a `.git` directory), never an enclosing one.
Your first action, before anything else in this file, is to confirm that one of that checkout's remotes points at
`scode/farhelm` (host, owner and name; SSH or HTTPS, with or without `.git`). If it does not, stop before the resume
protocol, write nothing (no log), and say the goal has to be started from a checkout of the project. Every project path
in this file is relative to that checkout's root.

## Resume protocol

The working log is the plan's record, `plan/claude-restart-before-first-turn`: its `log.md` lives in the directory
`cargo xtask records path plan/claude-restart-before-first-turn` prints, run from the checkout root after the claim
(`plans/.agents/outside-files.md`, "Files outside the repository"). Resolve that to an absolute path, then invoke the
`agent-resumeable` skill with it. If the log exists, read all of it and resume where the previous holder left off,
cross-checking it against GitHub (open `plan/claude-restart-before-first-turn/*` PRs) and the record's `notes/`, and
going by what GitHub shows where they disagree. If it does not exist, this is a fresh start. Nothing else is a log for
this plan; in particular, no log is ever created beside the checkout.

## Execution mode

No-workhorse mode. No unit of your own decomposition of this work goes to a weaker model or a lower effort than your
own. The review gate keeps the model and effort demanded below; scope reassessment runs at your own model and effort. Do
not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point. The planning system requires
no-workhorse mode for its plans (`plans/.agents/planning.md`), so if galaxy-brain is already active when the run starts,
this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering where they would finish the work sooner without
costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict can run
in separate jj workspaces whose results you merge back; you own the stack and every PR. A sub-agent whose model is not
available as a native sub-agent type, the reviewer included, is launched through `scode-harness-shellout`. The
sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", lists (the resume check and the cold reads of a Blocked
question and of a report) are exempt from this paragraph and run as that section says.

## PR discipline

Build a linear stack of bite-sized PRs with the `jjstack` skill, without churn: code one PR adds and a later PR deletes
means the stack should be shaped differently. Set up per `plans/.agents/executing.md`, "Executing one plan": the stack's
base is `main@origin`, or this plan's own open PRs when it resumes, never another plan's PRs. Bookmarks are
`plan/claude-restart-before-first-turn/<nn>-<short-name>`. PRs stay drafts; never mark one ready and never merge. A PR
that needs correcting is restructured rather than corrected on top, including PRs an earlier run built. Before every
push of a plan bookmark, follow `plans/.agents/executing.md`, "Executing one plan", step 9.

This plan never edits anything under `plans/`; its state moves only through the queue script, as
`plans/.agents/executing.md` describes.

Commit messages and PR titles follow `AGENTS.md`, "Conventional Commits", and the maintainer's commit and PR text rules
in their standing instructions. Changelog fragments follow `AGENTS.md`, "Changelog fragments" (the behavior change is a
`fix`, so it needs one; where what got built departs from the release note below, its context paragraphs say how, per
`plans/.agents/executing.md`, "Executing one plan", step 8). Tests follow `.agents/test-authoring.md`, and validation
follows `AGENTS.md`, "Tests and validation", `.agents/validation.md` and `.agents/test-evidence.md`. Running the real
Claude drift case follows `docs/harness-drift-tests.md`. Before relying on a claim about how Claude behaves in a
terminal, read `.agents/real-agent-tui.md`. Before touching harness-specific behavior, read the module docs of
`crates/farhelm-supervisor/src/agent_kind/mod.rs`.

## Review gate

Before finishing any PR, have its changes reviewed by a fresh-context agent (one that starts with none of your
conversation) on Opus 5.5 at high effort. This is a demand for that model and effort, not a unit to pick a model for.
Launch it natively, or through `scode-harness-shellout` when it cannot be reached natively. Its prompt names the repo
root (the checkout you run in), the bookmark or commit range to review, the file the findings go to (in your scratch
directory), and this charter in full, because the reviewer has nothing else; for a PR that changes tests or fixtures,
also include the full text of `.agents/test-authoring.md`, as `AGENTS.md`, "Tests and validation", requires:

> Review this PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved. Report findings to the named file, most severe
> first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

Give it this file's "The goal", "Decisions already made" and "Implementation outline and requirement sources" sections
as the acceptance criteria. Address what it finds before moving on.

## Resource watchdog

Right after the resume protocol, and before any delegation, start the watchdog from the `scode-build-goal` skill: the
Deno script `scripts/watchdog` in that skill's directory, with its path resolved through symlinks (`realpath`) at run
time. Its module docs describe each subcommand's output. It is mandatory: a full disk or an OOM kill ends an unattended
run mid-implementation with no signal beyond a dead delegate, and a Rust workspace's build output is large.

- **What it watches.** Status file: `farhelm-plan-claude-restart-before-first-turn-watchdog.json` in the parent
  directory of the executing checkout (derived the way that skill's "Placing the files" section derives it), never in
  the record's directory. Watched paths, each passed to `sample` as `--path`: the checkout root, the home directory,
  `/tmp`, the directory holding the status file, your scratch directory, and any build directory you later create on
  another filesystem (restart the sampler with it added). Interval 60 seconds. The script's default alert levels.
- **Claude Code.** Run `watchdog observe --status <file>` under the Monitor tool and
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout. Every line
  either prints is a notification. The observer reports `STALE` once the status file is two intervals old. Whenever
  either is re-armed or restarted, stop the other and arm both again, observer first, then run `check`. An expiry notice
  is routine; a monitor exit is a monitoring failure.
- **Codex.** Arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through
  the `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On
  the observer's callback (exit 10), acknowledge and arm a fresh observer; after a `STALE`, also stop the hung sampler
  with `wake-on-exit stop`, act on any alert in its `stdout.log`, and arm a new one; then `check`.
- **Acting on it.** An `ALERT` or `WORSE` line, or `check` reporting a resource short, is an instruction to act: stop
  launching new work, remove build caches and workspaces this run owns, wait for or kill the delegate most likely
  responsible, and resume only once `check` passes. If nothing the run owns frees enough, log the shortage as a blocker
  with launches paused. A monitoring failure (`STALE`, `ERROR`, a monitor exit, `check` failing on heartbeat age or an
  unreadable source) pauses launches until monitoring is restored and `check` passes; stop a hung sampler before
  starting another, so two never write one status file. A sampler that dies again before its first sample, or a source
  still unreadable after a restart, is logged as a blocker and not retried before the next natural boundary. Run
  `watchdog check --status <file>` before every new workload launch and at natural boundaries.
- **Verification.** In each session, before the real monitors, prove the mechanism on throwaway status files in your
  scratch directory with `--interval 5`: a sampler with `--synthetic-alert` delivers an `ALERT`; a sampler killed
  through the `pid` in its status file is reported as an exit; a sampler with `--stall-after 2`, watched by its own
  observer armed first, produces `STALE` within two intervals of the missed sample. Stop the throwaway runs afterwards.
- **Fallback.** Without either mechanism, or if it fails verification, say so in the log, run the sampler as a plain
  background process, and rely on `check` before launches and at boundaries. Do not poll the status file on a timer or
  lengthen the interval. Without `deno` there is no watchdog: launches pause until it is installed.
- **Lifecycle.** Record the monitor handles, status and watched paths, and the delivery mechanism in the log, and carry
  the watchdog's state in every handoff note. If the status file's heartbeat is fresh when a session starts, an earlier
  session's monitors are still running: stop them through the logged handles, or kill the `pid` the status file names,
  before starting your own. Ignore callbacks from runs this session did not arm. A resumed session runs `check` before
  its first launch. Stop the owned monitors when the goal is done or the plan closes.

## Decision logging

Log each major design decision in the working log as it happens, under a scannable `DECISION:` label with the
alternatives considered, especially the two close calls in the outline and anything that could reasonably have gone
another way. The maintainer reviews these later; an unlogged decision is a hidden one.

## Unattended fallback

Nobody watches this run. Never ask the maintainer anything in conversation: no direct question, no harness question
tool, no approval prompt, no offer of more work, no "should I ...?" ending a turn, whatever a skill or the harness
encourages. Every fork is either decided and logged, or a block.

Resolve routine implementation forks within the agreed scope yourself and log the consequential ones as DECISION. Remove
planner-proposed machinery that turns out unnecessary while the decided behavior and repository constraints still hold.
A material scope expansion, a weakened guarantee, or an omitted required behavior needs a decision this file already
records; when none covers it, the remaining work that depends on it waits on the maintainer, which means blocking per
`plans/.agents/executing.md`, "Executing one plan", step 10 (Blocking), after pushing everything that is consistent. A
review finding or a log entry is not authorization. Things the maintainer should know and possible follow-ups are not
questions: they go into the report's "things you should know" and "open questions and possible follow-ups" sections.

## Done criterion

The goal is done when every PR of the stack exists as an open draft, has passed its review gate, and the stack together
meets the acceptance criteria, and the latest `## Decisions` entry, if any, is satisfied. Delivering the report is the
executor's closing step, not part of the goal; merging is the landing monitor's job.

## Release note

Restarting a Claude Code session prior to initiating the first turn no longer fails in in a confusion way (e.g. "No
conversation found").
