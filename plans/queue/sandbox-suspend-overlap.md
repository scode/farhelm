# Sandbox Suspend and Resume no longer wait for launches on other hosts

Written against main at cb501937 on 2026-11-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Plan decisions to revisit"), verbatim:

> **Revisit keeping a sandbox session's Suspend and Resume exclusive with launches.** Decided on the maintainer's behalf
> on 2026-11-10 when landing nonblocking-session-actions stopped: that plan lets you act on other sessions while a
> launch or a session action is pending, and as delivered it also let Suspend and Resume from a sandbox session's row or
> top bar run while a launch or a change to the host list was pending, while the Hosts panel's own Suspend and Resume
> still waited. The plan was sent back to keep the session-facing Suspend and Resume waiting as before, since its goal
> file listed relaxing that exclusion as needing reassessment. The alternative was to keep the overlap, after showing
> that a suspend or resume running beside a launch on the same host cannot leave either in a wrong state.

The maintainer chose the alternative: let Suspend and Resume overlap launches, scoped to the host, and make a launch's
own wake of a host robust beside them.

### What planning found (read from the code, not run)

- The exclusion is the page's launch and host-mutation token (`crates/farhelm-ui/src/ops.rs`, `OpLock`; its module docs
  say session-facing sandbox mutations share it). A session-facing Suspend or Resume holds it until its request ends,
  allowed up to 150 s (`api.rs` around 5649-5658), so meanwhile every launch on every host, and every host change, is
  refused. It is already incomplete: the "Session unavailable" card's Resume (`session_view.rs`,
  `SessionUnavailableCard`, around 3343-3352) takes no token, and other windows and Tensorlake's own idle suspend
  overlap regardless.
- Helm: Suspend and Resume (`crates/farhelm-helm/src/tensorlake/lifecycle.rs`, `change`, around 49-164) take a try-lock
  on the host's provisioning lock and refuse with 409 "This host is busy; try again after its current operation
  finishes" when it is held; that lock serializes them against setup, update, removal and host edits. Session create
  does not take it; it needs a `Connected` client and checks the expected incarnation, so a held host refuses a create
  cleanly.
- The overlap cases: (1) a session Resume racing a launch's own wake: one gets 409 busy, and if the launch loses it is
  refused needlessly; (2) a Suspend after the wake's POST, while the launch polls for the connection: the launch polls
  to its 60 s limit and is refused with a misleading timeout; (3) a Suspend between the launch binding its connection
  and the create: the create is refused cleanly; (4) a Suspend while the create is with the supervisor: the connection
  cut gives an unknown outcome, and a retry with the same intent key returns the same session (SPEC.md around 3684,
  4804); (5) a Resume of a connected host during a create does nothing. None leaves a wrong state.
- The launcher already treats a host another surface is suspending or resuming as unavailable (`list/shared.rs`,
  `available_for_launch`, around 403-405; SPEC.md around 343).

### Where things are (as of cb501937; correct line numbers to the truth as you go)

Every place a session-facing or Hosts-panel Suspend or Resume claims or reads the page token:

- `crates/farhelm-ui/src/sandbox_sessions.rs`, `Controls` (around 171-309): `ops.claim_guard()` at the click (around
  224), joined to the session claim (`with_host_guard`, around 230); the buttons disabled through `ops.busy()` (around
  290-302).
- Both "Stop and suspend" answers: the row Stop question (`list/view.rs` around 2606) and the header Stop question
  (`session_view.rs` around 2416, with `composer_ops.busy()` around 2407), each passing the token through
  `SessionGuard::with_host_guard`. Stop-and-suspend carries it through `take_host_guard` (`list/view.rs` around 3076)
  and `HeaderAction::Sandbox` drops it after the request (around 3144-3170).
- The Hosts panel: `hosts.rs`, `run` (around 2101-2207, claim around 2112), a generic dispatcher over an opaque request
  future shared with operations that are real registry changes.
- The launch's wake: `create_form.rs` (existing folder around 6641-6685; hidden or preview path around 6050-6162) calls
  `api::resume_host_connected` (`api.rs` around 5666-5701): a resume POST with its own 150 s budget (a real resume can
  spend 120 s restoring memory), then polling the hosts listing until the host is connected, for 60 s after the POST
  returns. `api.rs` around 5684-5686 notes that a stale suspended state can stay visible right after a successful
  resume.
- The helm answers 409 for three different refusals in `lifecycle.rs` around 50-67: busy, "only for Tensorlake hosts",
  and a missing API key. The UI flattens every refusal into a string.
- Spec text: SPEC.md around 1324-1325 ("Launches and changes to the host registry still exclude each other"), around 333
  and 343; SPEC_impl.md around 219-221 (an admitted launch pins its resolved host into the host choice) and 1142-1143
  (the token "remains the launch-versus-host-mutation exclusion"), and 6887-6909 (the provisioning lock and Resume).

### Acceptance criteria

1. A sandbox session's Suspend and Resume (row and top bar), Stop and suspend (row and header), and the Hosts panel's
   Suspend and Resume no longer claim or wait for the page-wide token. Each is refused only while this page's own launch
   targets or is waking that same host, and its button reflects exactly that.
2. While one of them is pending, launches on other hosts and host changes elsewhere proceed. A launch on the same host
   is not refused by the page because of it; the launcher's existing rule (a suspending or resuming host is unavailable)
   and the helm's lock decide, as today.
3. The Hosts panel's other operations (setup, update, removal, edits) keep their exclusion with launches unchanged.
4. A launch's own wake waits out a helm "busy" refusal within its existing time budget instead of failing, without
   waiting on any other refusal (wrong host kind, missing API key), and without cutting off an accepted slow resume. If
   the host is suspended again while the launch waits for it to connect, the launch fails at once with a message saying
   so, and a stale suspended state right after a successful resume does not trigger that.
5. SPEC.md and SPEC_impl.md say that Suspend and Resume are not host registry changes and state the host-scoped rule;
   `ops.rs`'s module docs match.
6. The last code PR removes the TODO entry.

## Decisions already made

Settled by the maintainer on the 2026-11-11 planning page:

- **Overlap:** allowed. These refuse only while this page's own launch targets or is waking that same host. Rejected:
  keeping the exclusion, and keeping it while also putting the "Session unavailable" card's Resume under it.
- **Hosts panel:** its Suspend and Resume get the same host-scoped rule.
- **Hardening:** yes. A "busy" refusal during a launch's wake is waited out, and a host suspended mid-wake fails the
  launch at once with a clear message.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Size gate:** about 400 lines of diff across both PRs (code, tests and docs, not counting changelog fragments or the
  TODO.md removal). Over it, block and bring it back.

## Requirement sources

**The user's request:** the entry above and the decisions just listed.

**Binding repository constraints:** SPEC.md and SPEC_impl.md as cited (the launch-versus-registry exclusion stays for
real registry changes); root `AGENTS.md`, "Tests and validation", "Changelog fragments", "Conventional Commits",
"TODO.md and FLAKES.md", and its "Code rules" (no per-harness branching in shared code; not expected to come up);
`.agents/test-authoring.md`. No real Tensorlake calls: the browser specs use the existing stand-in API.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- "This page's launch targets or is waking that host" reads state the launcher already has: whether a launch is in
  flight and the host it pinned (SPEC_impl.md around 220-221), or the host it is resuming. No per-host launch registry
  and no new lock type; if a new value is unavoidable, one `Option<HostId>` beside the token.
- `run` in `hosts.rs` is told which operation it runs (a parameter or a separate entry point for Suspend and Resume), so
  only those two skip the token; its existing per-host busy checks stay.
- Telling "busy" apart: prefer a machine-readable marker from the helm on that refusal (a distinct error code in the
  JSON body, say) over matching the message text. The retries spend the existing 60 s connection budget, never the
  POST's own 150 s budget.
- "Suspended again" means evidence of a new suspension: a newer `observed_at` or `suspended_by_farhelm_at` than the wake
  started from (`crates/farhelm-helm/src/lib.rs` around 1197-1200), or Suspended after a non-suspended state was seen.
  The stand-in test includes the stale case.

## Outline

1. `plan/sandbox-suspend-overlap/01-host-scoped`: acceptance criteria 1-3 and 5. Browser tests (Chromium and WebKit) in
   `e2e/tests/tensorlake-sandbox-sessions.spec.ts` or its neighbours: a launch on another host proceeds while a session
   Suspend is held; a session Suspend is refused (and its button disabled) while a launch targets that host; the Hosts
   panel's Suspend likewise; a Hosts panel update still waits for a launch. `fix:` with a changelog fragment.
2. `plan/sandbox-suspend-overlap/02-wake-hardening`: acceptance criterion 4, the helm marker if used, unit tests for the
   wake's decisions and a browser test with the stand-in (busy then accepted; suspended mid-wake; stale suspended after
   a successful resume). Removes the TODO entry. `fix:`, amending PR 01's fragment or adding one.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
the touched crates, the touched unit tests through `python3 scripts/record-test-run.py`, the touched browser specs on
Chromium and WebKit, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, and
`python3 releasing/check-changelog.py format`. No cloud calls.

## Fallbacks

Examples of departures that need the scope reassessment: a symmetric mechanism refusing launches while this page's
Suspend or Resume is pending, a per-host launch registry or new lock, changes to the helm's provisioning lock or to
session create's preconditions, or changing the "Session unavailable" card's Resume.

If the analysis above turns out wrong (an overlap that can leave a session, the host row or the launcher in a wrong
state), do not ship the overlap: finish what is consistent and block with the failing case.

Decisions to log: how the host-scoped check reads the launcher's state, how `run` is split, how busy is recognized, what
counts as a new suspension, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-sandbox-suspend-overlap-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/sandbox-suspend-overlap/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
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
