# Remote Mac uninstall's mount check is best effort and never wedges

Written against main at 8e876c14 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Let a remote Mac's uninstall work when a volume name contains " (" or " on ".** Before removing Farhelm's
> directories from a remote Mac, uninstall reads the Mac's mount table to make sure nothing is mounted inside them, both
> when planning and again at the final directory removal, after the supervisor's job is already gone. The parser refuses
> the whole table when any line is ambiguous, so one unrelated volume whose name contains " (" or " on " (say
> `/Volumes/Backup (old)`) makes uninstall refuse: before anything changes, or partway through. It fails safe, but the
> user cannot uninstall while that volume is mounted. Only an ambiguous line that could name a path at or below the
> directory being removed should refuse. Found by the landing review of the macos-remote-hosts plan on 2026-11-08.

Why the check exists: uninstall ends with `rm -rf` of the install (lib) directory, and a filesystem mounted on or inside
it would lose the files the mount shows (a user's home reached by a second path, a shared install, a network volume).
`rm`'s one-file-system option is no substitute: a same-filesystem bind keeps its device number, and BusyBox `rm` lacks
it (`crates/farhelm-helm/src/provisioning/plan.rs` around 1056-1072). Linux reads `/proc/self/mountinfo`, which is
unambiguous; a Mac's base tools offer only `mount`'s human-readable "source on path (options)" lines, with no
machine-readable mode (the doc comment on `MAC_MOUNT_POINTS_SH` in `macos.rs` around 266-270; SPEC_impl.md around
7577-7579 records the platform split). SPEC.md's uninstall text does not mention mounts.

What is true today, from a read of main at 8e876c14 (line numbers approximate):

- The Mac parser is the shell function `mount_points DIR SEP` in the constant `MAC_MOUNT_POINTS_SH`
  (`crates/farhelm-helm/src/provisioning/macos.rs` around 264-306), run as `LC_ALL=C mount`. Before comparing a line
  with `$1`, it refuses the whole call when: a line lacks the `* on *(*)` shape ("cannot parse"); the text after the
  first " on " contains another " on ", or the source or options contain " (" ("ambiguous"); or the mount point is not
  absolute. An empty table also fails.
- Planning: `inspect_uninstall` (`crates/farhelm-helm/src/provisioning/backend.rs` around 3010-3150) runs `mount_points`
  for every spelling of each asked path (the unit plist, the lib dir, the state dir, the dialed binary; `service.rs`
  around 1762-1777) and fails as a whole on a refusal; `plan_uninstall` (`plan.rs` around 1060-1072) refuses only for
  mounts under the lib directory's spellings.
- Final removal: `remove_directory` (`backend.rs` around 3343-3389) picks the Mac or Linux function by `uname -s`, runs
  `mount_points` on the resolved directory, and `rm -rf`s only when nothing is inside. On a Mac this runs after the
  launchd job is removed and stopped (`plan.rs` around 1073-1109), so a refusal here leaves the job gone and the folder
  in place; unmounting and running uninstall again finishes it. The function must keep working when the farhelm binary
  is already gone (a retry after a partial removal), which is why it is shell.
- Tests: `macos.rs` around 650-672 (`mount_guard_preserves_spaces_and_rejects_ambiguous_tables`, a `ShellFixture` with a
  fake `mount`); every ambiguous fixture there puts the bad line under the owned directory, so the "unrelated ambiguous
  line passes" case is untested. The real-Mac test `crates/farhelm-helm/src/provisioning/tests/macos_real_tests.rs`
  around 590-770 is `#[ignore]` and needs an SSH Mac.

### Acceptance criteria

- The Mac check becomes best-effort protection that never wedges uninstall (the maintainer's decision below, which
  replaces the TODO entry's narrower "only an ambiguous line that could name a path at or below the directory should
  refuse"). `mount_points` checks only lines it reads one way: the "source on path (options)" shape, exactly one way to
  split it under today's rules, and an absolute mount point. Every other line is skipped (today's `return 1` refusals
  become `continue`), and an empty table or a failing `mount` yields no mount points. Skipped lines print nothing:
  callers paste stderr into the error the user sees, and output past the backend's stream cap (64 KiB) would itself fail
  the command, which is the very wedge being removed.
- The function ends with an explicit `return 0` after its loop, since a shell loop otherwise returns its last command's
  status and could still fail the callers' `|| exit 1` (those stay, because the Linux function still fails on an
  unreadable `/proc/self/mountinfo`).
- Accepted consequence, stated here so you do not reintroduce detection for it: a mount inside the directory whose own
  path contains " (" or " on " is no longer detected (today it refuses as ambiguous). That layout is already outside
  SPEC.md's supported aliasing (around 4825-4829), and the maintainer chose best effort. Do not enumerate the possible
  splits of an ambiguous line.
- So the only uninstall refusal left from the mount check is a mount positively detected at or inside the directory,
  with today's message (`plan.rs` around 1060-1072, and the final check in `remove_directory`).
- Both callers keep their current behavior for every table with no skipped line, the Linux function is untouched, and
  the function still needs nothing beyond the base tools it uses today.
- Tests, extending the fixture test in `macos.rs` (around 650-672) and turning its whole-table-refusal cases into passes
  (the newline-in-name nested mount there flips to "not detected" too): an unrelated ambiguous volume
  (`/Volumes/Backup (old)`, and a name containing " on ") passes; an unparsable line, an empty table and a failing
  `mount` pass; a table whose last line is skipped returns 0; an unambiguous mount inside the directory is still
  reported. One `remove_directory` test with the existing fixture (a fake `uname` printing Darwin and a fake `mount`)
  shows the final removal goes ahead with an unrelated ambiguous line present, which is the partway-through wedge the
  TODO entry describes. No planning-level Mac test: no harness exists for it, and the function compares strings only.
- No run on a real Mac is required.
- SPEC_impl.md around 7577-7579 states that the Mac check is best effort: it refuses only a mount it reads unambiguously
  at or inside the directory and skips everything else. SPEC.md needs no change.
- A `fix` PR with a changelog fragment. The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08, on the planning page:

- **Best effort, never wedge**, in the maintainer's words: "if we can successfully parse and detect a mount within the
  directory, refuse. do not fail to delete because of some random parse of some text fails due to some obscure edge
  case. the user having a freaking mount here is an insane edge case to begin with, we don't want anything wedged Never
  "wedge" on any parse failure or ambiguity. This is best effort protection." This was their answer to whether lines
  without the expected shape should still refuse; it covers ambiguity as well.
- **Validation by shell fixture tests** with fake mount tables; a run on a real Mac was the alternative and was not
  chosen.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Planned on hold** at the maintainer's request; that changes nothing about how it runs.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md around 4825-4832 (filesystem aliasing beyond symlinks is unsupported) and
SPEC_impl.md around 7577-7579; the remote shell rules the provisioning backend already follows (POSIX `sh`, base tools
only, no farhelm binary at removal time); root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog
fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`.

**Planner proposals:** skipping every line not read one way, rather than enumerating its possible splits (the smallest
change that meets the maintainer's words) or switching to another mount source (none exists in a Mac's base tools, and a
farhelm subcommand would fail on retries after the binary is removed and on older installs); printing nothing for
skipped lines; the test set above.

## Outline

One PR, `plan/mac-uninstall-volume-names/01-ambiguous-mount-lines`, typed `fix`: the shell function, its tests,
SPEC_impl.md, the changelog fragment, and the TODO.md removal.

### Validation

The helm crate's provisioning tests that cover `mount_points`, `inspect_uninstall`, `plan_uninstall` and
`remove_directory` (narrowly, per `.agents/narrow-tests.md`), through `scripts/record-test-run.py`. The ignored real-Mac
test is not required.

## Fallbacks

Examples of departures that need the scope reassessment: a new mount source on the Mac, a farhelm subcommand at removal
time, changing which paths uninstall inspects, or changing the Linux function.

Decisions to log: anything in the skip rule that differs from the above, and every review finding you decided not to
follow. Making the check stricter again for any case is not yours to decide: the maintainer chose best effort.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-mac-uninstall-volume-names-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/mac-uninstall-volume-names/<nn>-<short-name>`, as the outline names them.
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
