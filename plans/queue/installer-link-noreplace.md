# Installer: the Terminal command never overwrites a new arrival

Written against main at 6b7bbcda on 2026-10-26. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Execute two triage outcomes recorded on 2026-10-26, per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute
triage outcomes": one draft PR per outcome, in this plan's one stack. Their `TRIAGE_OUTCOMES.md` entries are the binding
statement of what to do; read each in full from `main@origin`, together with its feedback file under
`review_feedback_queue/`, before working on it:

1. `installer-terminal-link-overwrites-new-foreign-file.md` (`fix code`). The installer places the `farhelm` Terminal
   command at `~/.local/bin/farhelm`. It inspects that path and moves a foreign file there aside under a visible name,
   then publishes its own link with an overwriting rename, so a file another program writes there between the check and
   the rename is lost. During the one-time move from the old layout, a recorded old copy is hashed by path and later
   removed by path, so a replacement made in between would be deleted.
2. `older-installer-rebuild-deletes-late-foreign-app.md` (`fix spec`). Installing a release older than the handoff floor
   runs that release's own published installer, which can delete an app placed at the app location while it builds.
   Published scripts cannot change; the outcome is a SPEC.md sentence, no code.

What the code does today, from triage's read of main at 2977e6c9 (line numbers approximate):

- `crates/farhelm-supervisor/src/installation/install.rs`: `place_link` (around 990-1077), called last after the app is
  built or updated (around 333). It checks the path (around 998), hashes a recorded old-layout copy (around 1009-1013),
  renames a foreign entry to `farhelm.replaced-<stamp>` (around 1037), then publishes with
  `std::fs::rename(&stage, &link)` (around 1059). Its doc comment (around 986-989) says the public names keep path-based
  checks.
- The same file already uses a no-replace rename for the app (rustix `RenameFlags::NOREPLACE`, which maps to
  `RENAME_EXCL` through `renameatx_np` on macOS; around 905-940 and 1133-1138).
- `scripts/install.sh`: `HANDOFF_FLOOR` (around 101) and the handoff to an older release's own installer (around
  932-975). SPEC_impl.md describes the handoff around 4612-4621.

### Acceptance criteria

- Outcome 1: the final placement of the Terminal command never replaces an entry that appeared after the installer's
  check. The ledger says "a no-replace rename"; its intent is that the final step refuses an existing name. Planning
  review found a smaller shape with that property on every filesystem: once the old copy and any foreign file have left
  the name, create the symlink directly at `~/.local/bin/farhelm`, which refuses an existing name, so no rename flag,
  staging name or unsupported-volume fallback is needed. Use it unless the code shows a reason not to (log the
  DECISION); if you use a no-replace rename instead, a volume without the flag needs a fallback you choose and log. If
  the direct symlink leaves SPEC.md reserving a staging name nothing uses, leave SPEC.md alone and say so in the report.
- On a new arrival it stops and says so (decided below): the app stays installed, the Terminal command is not placed,
  the new file is left untouched and named, and the message says to run the installer again (which then moves it aside
  under a visible name, as for any foreign file found at the start). If a foreign file was already moved aside earlier
  in the same run, the refusal still reports where it was kept (SPEC.md requires that move to be reported; today that
  note is only returned on success). The refusal also reaches the app's updater; handle it the way the existing "is a
  directory" refusal is handled there.
- Both old-layout copies are in scope: the recorded `farhelm` copy the placement overwrites, and `farhelm-desktop`,
  which is removed by path after hashing. Each is moved straight to the visible `<name>.replaced-<stamp>` name, hashed
  there, and deleted only if the hash matches the recorded one; otherwise it stays under that visible name and is
  reported. A private or reserved name would let a crash lose a foreign file, which SPEC.md forbids.
- A test places a file after the check; adding the one test hook that needs (a step between the check and the final
  placement) is in scope. The doc comment and SPEC_impl.md's installer text are updated where they describe the
  placement. `fix:` with a changelog fragment.
- Outcome 2: SPEC.md's installation section states that installing a release older than the handoff floor runs that
  release's own installer, with the protections it had then, which include no protection against something put at the
  app location while it builds. No code change. `docs:`, no fragment.

## Decisions already made

Settled with the maintainer on 2026-10-26:

- **The outcomes** are as recorded in `TRIAGE_OUTCOMES.md`: close the Terminal-command race, accept the old-release
  handoff in the spec.
- **Complexity gate**, in the maintainer's words: "with complexity gate, if this turns out to be non-trivial block the
  plan." Triage estimated outcome 1 at about 20-40 lines of non-test code plus a test; the old-layout handling above is
  part of that estimate. If the non-test code turns out well beyond that (say, more than about 80 lines), block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), with the measured size and why, after pushing outcome
  2's PR if it is finished.
- **On a new arrival, stop and say so** (planning page): no retry loop that moves the arrival aside.
- **One plan for the two outcomes**, since both concern the installer's foreign-file promise in the same SPEC.md
  section.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the two `TRIAGE_OUTCOMES.md` entries named above, and the decisions above.

**Binding repository constraints:** SPEC.md's installation and ownership text (around 443-455 and 542-555: foreign files
at `~/.local/bin/farhelm` are kept under a visible name and reported) and its accident-guard carve-out (around
3320-3333); SPEC_impl.md's installer sections (around 4600-4700); root `AGENTS.md`, "Tests and validation", "Changelog
fragments", "Conventional Commits"; `.agents/test-authoring.md`; `review_feedback_queue/.agents/execute-outcomes.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- The direct symlink above; if a rename is kept, reuse the file's existing no-replace rename helper rather than adding
  another.
- Keep the refusal's wording in the style of the installer's other foreign-file messages.

## Outline

Two PRs, in ledger order:

1. `plan/installer-link-noreplace/01-terminal-link-noreplace`: outcome 1, with its test and fragment.
2. `plan/installer-link-noreplace/02-older-installer-spec`: outcome 2's SPEC.md sentence.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy on
`farhelm-supervisor`, the `installation::install` module tests through `python3 scripts/record-test-run.py` (nextest,
`--tmux none`), and `bash scripts/test-install-sh.sh` if the change reaches what it drives. PR 2 needs only
`dprint check`.

## Fallbacks

Examples of departures that need the scope reassessment: a retry loop on arrival, changing `scripts/install.sh`'s
handoff, or protecting the public link with Farhelm's installation lock instead of the rename.

Decisions to log: direct symlink versus rename (and the fallback if a rename), how the old-layout copies are moved
aside, the refusal wording, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-installer-link-noreplace-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/installer-link-noreplace/<nn>-<short-name>`, as the outline names them.
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

The plan is complete when a linear stack of draft PRs exists, one per triage outcome in the order the outline gives,
each meeting that outcome's completion criteria in `TRIAGE_OUTCOMES.md` (as refined by Decisions already made above),
each having passed its review gate, and each recording its execution in the outcome's Execution field (change id,
bookmark and PR URL) and removing its feedback file and `review_feedback_queue/INDEX.md` line, per
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes". Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
