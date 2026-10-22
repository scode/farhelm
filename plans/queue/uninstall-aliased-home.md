# Host uninstall: recognize the home folder when it is reachable at two paths

Written against main at 0fb0699a on 2026-10-21. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says; this plan changes them in the one place described below, as the maintainer decided.

This plan has no dependency on another plan.

## The goal

Uninstalling Farhelm from a remote host (the uninstall item in a host's menu) refuses on hosts where the account's home
folder is reachable at two paths, even though the installation is entirely Farhelm's own. Make uninstall work there,
without opening up anything else that its safety checks refuse today.

TODO.md entry this plan covers (Near term): "**Uninstall refuses on a host whose home folder is reachable at two
paths.**" (added by the planning PR; its full text is in TODO.md).

### The bug

On some machines (corporate development VMs, for example) the account's home folder lives at one path and is
bind-mounted at another, so every file in it has two paths. Logging in over SSH, `$HOME` and the passwd entry all give
one spelling. The host's systemd user manager can still use the other: one that started before the home was remapped
builds its unit search path from the spelling it saw then. Uninstall asks systemd where it loaded the supervisor unit
from (`FragmentPath`), compares that with the unit file it plans to remove, after `readlink -f` on both, as text, and
refuses with "... is loaded from <one spelling>, not from <the other>; uninstall only removes a unit set up from the
hosts panel". `readlink -f` resolves symlinks, not bind mounts, so the same file keeps two spellings. Farhelm had
installed that unit itself.

### A worked example

Use it in tests and in the code comments the maintainer asked for (see Decisions already made). The host account is
`alex`. Its real home folder is `/srv/accounts/a1`, which the machine also mounts at `/home/alex`. The SSH login's home
is `/home/alex`; the systemd user manager uses `/srv/accounts/a1`.

```
/home/alex/                        (same folder as /srv/accounts/a1/)
  .config/systemd/user/
    farhelm-supervisor.service       the supervisor's unit file
  .local/lib/farhelm/                Farhelm's lib directory (uninstall removes it)
    farhelm                          the supervisor binary
  .local/state/farhelm/              the state directory (uninstall keeps it)
```

Uninstall's three spelling-sensitive checks, today:

1. **Unit origin.** The unit systemd loaded must be the planned unit file. Here
   `/srv/accounts/a1/.config/systemd/user/farhelm-supervisor.service` against
   `/home/alex/.config/systemd/user/farhelm-supervisor.service`: refused. This is the bug.
2. **Binary inside the lib directory.** The supervisor binary the host runs must be inside the lib directory, so the
   removal removes the installation actually in use. Fails closed (refuses) on a spelling mismatch.
3. **State directory not inside the lib directory.** Otherwise the removal would take the data with it. Fails open on a
   spelling mismatch: a state directory registered as `/srv/accounts/a1/.local/lib/farhelm/data` is inside the lib
   directory but does not start with `/home/alex/.local/lib/farhelm`, so today uninstall would remove it. Contrived, but
   it is the one check whose failure deletes data.

### The fix the maintainer chose: only the home folder may have a second name

For each path the uninstall inspection resolves, the host checks whether a folder above it (or the path itself) is the
same folder, by identity, as the login's home. If so, that leading part is rewritten to the login's home spelling. Then
the three checks run exactly as they do today, as text.

One guard keeps the rewrite honest: it is kept only when the host confirms that the rewritten path and the original are
the same file (`[ -ef ]` on the two full paths); otherwise the path keeps today's spelling. Without it, a non-recursive
bind of the home (`mount --bind` rather than `--rbind`) could make the rewritten path name a different file: if
something inside the home is its own mount, one spelling sees that mount and the other sees the folder hidden under it.
The rewrite would then pass a check for a file uninstall does not actually remove. The guard can only reject rewrites,
so every fallback is today's behavior. In short, a path is respelled under the home only when the host confirms that
both spellings are the same file. In the example:

- Unit origin: `/srv/accounts/a1` is the same folder as `/home/alex`, so the fragment becomes
  `/home/alex/.config/systemd/user/farhelm-supervisor.service`, equal to the planned unit file. Uninstall proceeds.
- The contrived state directory becomes `/home/alex/.local/lib/farhelm/data`, which the existing text check sees inside
  the lib directory, so uninstall refuses and keeps the data.

What it must keep refusing, as today, because nothing above these paths is the home folder:

- **Shared unit folder.** An administrator mounts one folder of unit files (say `/srv/shared-units`) as every user's
  `~/.config/systemd/user`; systemd reports `/srv/shared-units/farhelm-supervisor.service`. Refused; removing it would
  break every account using it.
- **Shared install.** An administrator mounts one Farhelm install (say `/opt/farhelm`) as every user's
  `~/.local/lib/farhelm`, and the host's supervisor binary is registered as `/opt/farhelm/farhelm`. Refused; removing it
  would empty the install for everyone.
- **A hard-linked copy of the unit in another unit folder,** for example under `~/.local/share/systemd/user/`. After the
  rewrite it is still a different path from the planned unit file, so it is refused. Removing the planned file would
  leave the copy, and systemd could start the supervisor again.

The maintainer weighed and rejected comparing each folder pair by identity wherever it lives: it would accept the two
shared-folder cases above, which today's text comparison refuses.

### Acceptance criteria

- On a host whose login home is reachable at a second path, an uninstall whose files are Farhelm's own plans and runs,
  whichever spelling systemd uses for the unit.
- Every case under "What it must keep refusing" is still refused, with today's messages, and the contrived state
  directory case is now refused.
- The rewrite applies to every path the uninstall inspection resolves, so all three checks see the same spellings.
  Nothing outside uninstall changes behavior.
- A respelled path is kept only when the host confirms it is the same file as the original (the guard above).
- When the host's home cannot be resolved, uninstall behaves exactly as today.
- SPEC.md's paragraph on filesystem aliasing (around SPEC.md:3494, "Filesystem aliasing beyond symlinks is unsupported")
  gains a narrow exception: uninstall from the hosts panel recognizes the host account's home folder reached through a
  second path, such as a bind mount; any other aliasing stays unsupported, and uninstall still refuses what it cannot
  tell apart. Keep the list of exceptions a few lines above it (around SPEC.md:3481, which names "filesystem aliasing"
  among the setups that need not be refused) consistent with it. In SPEC_impl.md's UNINSTALL passage, both the
  `FragmentPath` sentence (around SPEC_impl.md:4620) and the sentence saying the paths are compared as canonical paths
  the host resolves with `readlink -f` (around SPEC_impl.md:4625) say how the home's second spelling is handled, the
  same-file guard included.
- Ample code comments, as the maintainer asked. At the rewrite, one full explanation lays out the logic and every case
  above: the bug, the shared unit folder, the shared install, the contrived state directory, the hard-linked copy, and
  the non-recursive bind the guard exists for. It also says why per-folder identity was rejected, so a later reader does
  not "simplify" the design into it. At each of the three checks, a comment says what that check guards, how the
  respelling affects it, and which of those cases concern it, and points to the full explanation rather than repeating
  it.
- Tests cover the bug case, each must-keep-refusing case, the contrived state directory, and the unresolvable-home
  fallback (see Validation).
- A `fix:` PR with a changelog fragment (root `AGENTS.md`, "Changelog fragments"), which also removes this plan's
  TODO.md entry.

## Decisions already made

The maintainer's words when asking for this plan (2026-10-21): "Let's use the planning system to plan a fix for the
following bug. Do not use any information in this screenshot verbatim, any logs, files, or anything. No hostnames, no
details, or anything like that. Just the generality of the problem." That rule binds everything this plan produces: no
host names, account names or real paths from the original report in code, tests, comments, commit messages, PR text, the
changelog fragment or the report. Use the made-up example above.

Answers from the up-front questions page:

- **Scope: uninstall works on such a host.** Chosen over keeping the refusal with better wording and over supporting a
  doubled home throughout Farhelm. The spec gets a narrow exception for uninstall only; sessions, working directories,
  managed checkouts and every other path comparison keep treating doubled paths as unsupported.
- **Mechanism: home folder only, applied to all three checks**, as described under "The fix the maintainer chose". The
  maintainer first asked whether one parent directory could be compared by identity ("Isn't there one parent directory
  we're looking at? If we compare the inode of that, we can assume that everything under it is fine"), then chose this
  design over per-folder identity once shown the shared-folder cases.
- **Comments:** "lets mandate in the plan that there be ample comments in the code explaining the logic, laying out
  these cases".
- **Reviewer:** gpt-6.1-sol at high effort (see Review gate).
- **If gpt-6.1-sol is unreachable** when a review is due (CLI missing, quota, outage): block the plan per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. No substitute
  reviewer.
- No-workhorse mode, as `plans/.agents/planning.md` requires.

## Requirement sources

**The user's request and decisions:** above.

**Binding repository constraints:**

- SPEC.md's remote uninstall passage (around SPEC.md:147-172) and its filesystem aliasing paragraph (around
  SPEC.md:3494);
- SPEC_impl.md's UNINSTALL passage (around SPEC_impl.md:4597-4625);
- root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
- `.agents/test-authoring.md` for tests; tests never change the test process's own environment variables.

**Planner proposals** (a smaller design that meets the acceptance criteria is fine, logged as a DECISION):

- Do the rewrite in the inspection's host script (`inspect_uninstall` in
  `crates/farhelm-helm/src/provisioning/backend.rs`, inside its `resolve` helper), because only the host can answer
  "same folder". Canonicalize `$HOME` once with `readlink -f`. After each `readlink -f`, walk up the result, starting
  with the path itself, until an ancestor is `[ -ef ]` the canonical home. Replace that prefix with the canonical home,
  and keep the result only if it is `[ -ef ]` the original (the same-file guard).
- Edge cases for that walk:
  - skip the rewrite when `$HOME` is unset, when `readlink -f "$HOME"` fails, when the canonical value is empty, or when
    the canonical home is `/`;
  - stop a `${p%/*}`-style walk when it runs out of components;
  - pick a walk direction and document it (it only matters for a folder inside the home that is a bind of the home
    itself, and the guard makes either direction safe).
- Shell portability: `test -ef` is not POSIX, but dash, bash, BusyBox `sh`, mksh and zsh all have it (the planner
  checked dash). The script runs under `sh -c`. Keep each `-ef` test inside an `if` or `&&`, so that a shell lacking it
  (status 2) falls back to today's behavior instead of failing the script.
- Why `$HOME` is the right home: the planned paths come from the reach script's raw `$HOME` over the same SSH login (or
  from the manager's `XDG_CONFIG_HOME`, which the respelling treats the same way on both sides). Say so in a comment.
- Feed the rewritten value through the existing `HostPath.canonical` field, so the three checks in
  `crates/farhelm-helm/src/provisioning/service.rs` and `plan.rs` (`plan_uninstall`) stay as they are; a separate field
  would touch every consumer and test for no behavior change.
- Update the docs that describe `canonical` as the host's plain `readlink -f` answer:
  - the `UninstallInspection` doc;
  - `HostPath::canonical`;
  - the output description on `inspect_uninstall`;
  - `plan_uninstall`'s doc;
  - the spec docstring of the real-script test.
- One visible side effect to note in a comment: the lib-folder symlink refusal prints `lib_dir.canonical` as the link's
  target, which may now appear in the home spelling. It is the same place, though not what `ls -l` shows. That refusal
  still runs first, because its symlink flag comes from `[ -L ]` on the asked spelling.

## Outline

One `fix:` PR: the rewrite, the comments, the SPEC.md and SPEC_impl.md changes, tests, the changelog fragment, and the
TODO.md entry removal. Split it only if a part turns out to stand on its own (log why).

The changelog fragment is written for someone running Farhelm, in plain terms: uninstall from the hosts panel now works
on hosts whose home folder is reachable at two paths (for example through a bind mount), where it used to refuse saying
the service was loaded from somewhere else.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect:

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`;
- the `farhelm-helm` provisioning unit tests that cover uninstall planning and the inspection, through
  `python3 scripts/record-test-run.py`;
- `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown.

The new behavior is the host script's rewrite, so test it there, through the real script. The checks themselves do not
change, and re-testing them over hand-made, already-respelled inspections adds nothing.

`uninstall_host_commands_refuse_setup_units_and_skip_finished_work` in `crates/farhelm-helm/src/provisioning.rs` already
runs the real inspection script under `sh -c`, through `FixtureHomeLauncher`. That gives it a fixture `HOME` and a
fixture `bin` at the front of `PATH`, holding a fake `systemctl`. Extend that pattern; never use the local transport,
which would read the test process's own `HOME` and call the real `systemctl --user`.

A real bind mount needs privileges. On the planning machine unprivileged user namespaces are blocked (`unshare -rm`
fails), and no existing harness has a privileged mount namespace, so there will be no real bind-mount test. A symlink
alone cannot stand in for one, because `readlink -f` resolves it to the home spelling before the rewrite sees it.

The suggested stand-in is two fakes in the fixture `bin`:

- a `readlink` that echoes its operand, together with a symlink `alias -> home`, which gives a second spelling that
  `readlink -f` does not collapse but `[ -ef ]` identifies, as a bind mount looks to the script;
- a `systemctl` that reports a `FragmentPath` under the alias.

That drives the production script end to end and covers:

- the bug case;
- the contrived state directory;
- a fragment in a folder outside the home (shared unit folder);
- a binary under a stand-in for a shared install;
- a same-named unit file elsewhere inside the home;
- an unresolvable home.

The guard against a non-recursive bind cannot be exercised without a real mount; cover it as directly as the script's
shape allows, or say it is untested. The report says exactly what was and was not exercised against a real second path.

## Fallbacks

Examples of departures that need the scope reassessment: comparing folders by identity outside the home folder, changing
which checks uninstall makes, touching path comparisons outside uninstall, or a new probe round trip.

Decisions to log: where the rewrite lives, which field carries it, how a missing or unresolvable home is handled, the
test stand-in chosen for a second path, and every review finding you decided not to follow.

Things to note in the report rather than fix: other places with the same blind spot that you notice (for example
`farhelm helm setup`'s check that the running user manager reads units from the expected directory).

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-uninstall-aliased-home-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

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
sub-agent type, the reviewers included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/uninstall-aliased-home/<nn>-<short-name>`
  (for example `plan/uninstall-aliased-home/01-home-alias`).
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`); this fix is `fix`, with its changelog fragment in the same commit.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
mechanics). If gpt-6.1-sol cannot be reached, block as Decisions already made says; never substitute another model.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range of this one PR;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal (the bug, the worked example, the chosen fix and what it must keep
  refusing), its acceptance criteria and Decisions already made.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim in the review's
prompt, as root `AGENTS.md` requires.

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
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
