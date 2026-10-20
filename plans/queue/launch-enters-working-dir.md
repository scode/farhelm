# A launch starts in its session folder or fails clearly

Written against main at 319b286a on 2026-10-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

When a session's folder exists but cannot be entered (a recursive permission change, a folder owned by someone else,
ACLs, a network or FUSE mount that answers stat but refuses entry), tmux silently starts the agent or terminal tab in
HOME instead, and Farhelm reports success and keeps showing the selected folder, so an agent can act on HOME unattended.
The maintainer's principle, which SPEC.md gains: a launch (agent, Restart, Replace, terminal tab) either starts in the
session's working directory or fails with a clear error naming it; it never silently starts somewhere else. Resolving a
symlink to its target is not "somewhere else". In the maintainer's words: "This is NOT an acceptable silent failure."

Carry out this triage outcome, recorded under its own heading in `TRIAGE_OUTCOMES.md`. Read the entry in full first: its
Assessment says what is wrong and which rule settles it, its Decision holds the maintainer's words, the planned fix and
any complexity gate, and its Completion criteria are the acceptance criteria for that outcome.

1. `unusable-directory-launches-in-home.md` (`fix spec+code`): An existing but unsearchable folder can make a session
   launch in the home directory instead.

### Acceptance criteria

- Each outcome's Completion criteria in its `TRIAGE_OUTCOMES.md` entry, including its regression test where it names
  one.
- Each outcome's PR removes its feedback file and `review_feedback_queue/INDEX.md` entry and updates its Execution
  field.
- A `fix` PR carries a changelog fragment per `AGENTS.md`, "Changelog fragments" (`kind: fixed` unless this file says
  otherwise); test-only, docs-only and bookkeeping PRs do not.

## Requirement sources

**The user's decisions (2026-10-18):** each outcome's Decision field records the maintainer's own words and the agreed
scope. Scheduling the 25 outcomes triaged that day as twelve plans, and which outcomes share a plan, was agreed with the
maintainer. Review gate: a single fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm.
No-workhorse mode, which `plans/.agents/planning.md` requires.

**Plan-specific answer (2026-10-18):** the launch helper always enters the session folder (and sets `PWD` to match),
after the login shell's startup files have run. That overrides a `cd` in a user's `.zshrc` or `.bashrc`, which today
moves the agent elsewhere; the maintainer chose that, as the principle requires. Terminal tabs enter the folder before
their shell starts, so a startup-file `cd` still applies to a tab, which is ordinary shell behavior.

**Binding repository constraints:** the rule each entry's Assessment cites as settling it;
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes"; `AGENTS.md`, "Changelog fragments",
"Conventional Commits" and "Tests and validation", with `.agents/validation.md`; `.agents/test-authoring.md` for any
test change. SPEC.md's requirement that "a missing or unusable directory still produces a clear error", and SPEC.md's
launch and terminal-tab sections.

**Planner proposals:** each entry's planned fix, the PR order above, and the outline below. A planner proposal is not a
requirement: a smaller design that meets the same completion criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main (verify them): `ensure_cwd_usable` in
`crates/farhelm-supervisor/src/service/core.rs` checks only that the path is absolute and a directory. The launch spec
(`launch.rs`) has no working-directory field; tmux starts the login shell in the folder, which runs
`exec farhelm internal launch <spec>`, and the helper runs checkout preparation, then execs the agent in whatever
directory it inherited. Terminal tabs run a bare `env -u ... <shell> -l -i`. On Linux a systemd scope wrapper wraps the
helper for agents and the shell for tabs, and execs in place either way. A helper failure is already recorded as a
launch failure, and a tab whose shell dies at open already has its last output quoted in the refusal, so an
enter-or-fail step that prints the folder reaches the user with no new plumbing.

Proposed shape (planner proposal):

- An optional working-directory field in the launch spec, with a serde default so a spec without it still parses (the
  spec does not reject unknown fields), filled in where Create, Restart and Replace build the spec (one shared place in
  `core.rs`). The helper enters it before exec, sets `PWD`, and fails naming the folder if it cannot, through the
  launch-failure record every early exit of the helper already writes. Whether the check goes before or after checkout
  preparation is yours to settle (preparation may be what creates a managed checkout's folder); log it.
- Terminal tabs go through a small `farhelm internal` step that shares the helper's enter-or-fail code and then execs
  the shell, inside the scope wrapper on Linux. Avoid `env -C` (GNU only). A fixed `/bin/sh -c` with the folder as an
  argument is an acceptable alternative if the subcommand turns out awkward; log the choice.
- Create, create retry, Restart, Replace and OpenTab all reach the folder through these two paths; an optional earlier
  pre-check is allowed but not in place of the in-launch check.
- A tab's failure becomes visible through the tab-open liveness check, which already quotes the dying shell's last
  output; make sure the message names the folder.
- Tests use a folder the user cannot enter, skipped when running as root. Since Create, Restart and Replace share the
  spec builder, a unit test of the helper's enter-or-fail step, a runtime test for Create, a runtime test for a tab, and
  a check that the shared builder fills in the folder cover the ledger; add Restart and Replace runtime tests only if
  they report the failure differently from Create.
- SPEC.md states the principle, with the symlink note, near the existing "clear error" requirement.
- macOS and Linux use the same mechanism. Whether macOS privacy protection for Documents and Desktop routinely triggers
  this is unverified; say so in the report rather than testing it.

### Validation

Per `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression in what each PR changes, and report checks run, reused and skipped. `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`; the supervisor's launch
and tmux tests this touches through the recorder with the pinned nextest and tmux setup (`--tmux required`), narrow
first; the test-sleep checker when a test with delays changes; `python3 releasing/check-changelog.py format`. Read the
retained output of the new tests: a root-skip or other early return is a JUnit pass, not proof the unenterable-folder
case ran.

## Fallbacks

No complexity gate. Examples of departures that need the scope reassessment: changing tmux's invocation instead of
entering the folder in the launched process, a new protocol message, or a pre-check offered in place of the in-launch
check.

Decisions to log: the field's name and default, the tab step's form, the error wording, and how each launch kind reaches
the check.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-launch-enters-working-dir-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Follow `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes": one linear stack for this
  outcome; split it into bite-sized PRs if that reads better (for example the helper and agent launches, then terminal
  tabs), each removing nothing until the last, which removes the feedback file and index line and completes the
  Execution field. Each PR carries its outcome's agreed change, the removal of its feedback file and
  `review_feedback_queue/INDEX.md` entry, and its `TRIAGE_OUTCOMES.md` Execution update. Record the jj change ID and
  bookmark in that Execution field before creating the PR, then add the PR's URL (with `/changes` appended) to the same
  change and push it again; no separate bookkeeping PR. Replace the Execution field's current `planned in ...` text
  rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/launch-enters-working-dir/<nn>-<short-name>` (for example
  `plan/launch-enters-working-dir/01-launch-enter-folder`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `fix` for a change users of Farhelm can observe, with a changelog fragment in the same commit; `test`
  for a change only to tests or their fixtures; `docs` for documentation and specification text only; `chore` for
  maintainer tooling and queue bookkeeping. Only a fragment-requiring type adds a fragment.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TRIAGE_OUTCOMES.md`, `review_feedback_queue/`, `TODO.md` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, the outline, and the outcome's full
  `TRIAGE_OUTCOMES.md` entry, including any complexity gate (ask the reviewer to flag anything beyond the gate).

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above. If current code or specs invalidate a recorded triage decision (for example,
the finding turns out already fixed or wrong), that is a block: `review_feedback_queue/.agents/execute-outcomes.md`,
"Execute triage outcomes", returns such an item to the user rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every outcome that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that meets the acceptance criteria above, each PR passed
its review gate, and the outcome's `TRIAGE_OUTCOMES.md` Execution field names its change IDs, bookmarks and PRs. Open,
not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Blocked

Blocked while landing on 2026-10-20 (claim 89f53d).

### What this plan does

When a session's folder exists but cannot actually be entered (a permissions problem, say), tmux used to start the agent
or terminal tab in the home folder instead, while Farhelm still showed the chosen folder. This plan makes every launch
enter its folder or fail with an error naming it: in its PR, agents enter it just before the agent program starts, and
terminal tabs start through a small Farhelm helper that enters the folder before starting the user's shell.

### What the landing found

Nothing of this plan has merged; its one PR (#2363) is still open, and main has none of its changes.

1. The landing's run of the whole supervisor test library failed seven tests that this plan breaks (run
   `acae51f5-18dc-4dc0-af71-f9bda72e0a19`). Those tests open terminal tabs on a test supervisor whose Farhelm program
   path deliberately points at nothing, so that agent launches fail on purpose. Tabs now need that program to run the
   new helper, so every tab in those tests dies at once and the open is refused. The tests cover tab scoping, deleting a
   session with tabs, Restart and Replace with tabs, and forged tab markers. The executor ran only the plan's own launch
   tests, not these. In this finding nothing outside the tests is wrong, but fixing them means giving those tests a
   stand-in program that can run the tab helper, without changing the placeholder the agent-launch tests rely on. That
   is new test machinery, not a landing-sized fix.
2. By reading the code (not run): every newly started agent now records its folder, and when the agent fails to start
   for any reason, the error starts with "could not launch in working directory <folder>". The most common failure, an
   agent program that is not installed, would then point the user at the folder instead of the missing program.
3. Minor, by reading: in the Mac app, the tab helper runs through the app's small forwarder program (the stable entry
   point that hands off to the current version) rather than the supervisor's own versioned copy, which adds one extra
   process level to every terminal tab there. SPEC_impl.md's section on side-by-side versions reserves that forwarder
   for short-lived launch helpers. Switching to the supervisor's own copy is a one-line change.

The other seven plans in this landing round do not depend on this one and are landing without it.

### Question

How should this plan continue?

- (a) Send it back to an executor to fix the seven tests and make the error name the folder only when entering the
  folder is what failed, and to switch the Mac tab helper to the supervisor's own program (or document why not). This
  keeps the plan's design and costs one more execution.
- (b) Send it back for the tests only, and accept the error wording and the Mac forwarder as they are. Faster, but a
  missing agent program would be reported as a folder problem.
- (c) Abandon the plan and leave the original problem open.

Recommendation: (a). The test fix is needed either way, and the misleading error would affect the most common launch
failure.
