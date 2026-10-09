# The docs preview stops only the server its lock names

Written against main at adf78aba on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan.

## The goal

`website/scripts/preview.sh` serves the docs site for the maintainer and stops earlier previews through the lock Astro
leaves in a checkout's `website/.astro/dev.json`. Before stopping the process the lock names, it only checks that the
process runs from that checkout's website directory. A stale lock (left by a server that died without cleaning up, a
reboot being the realistic case) whose process number now belongs to a later Astro command in the same directory, such
as a build, `astro preview` or `astro check`, passes that check, and the script stops that command. Make the script
leave such a process alone.

Acceptance criteria:

- Before stopping the process a background lock names, the script also requires that the process started no later than
  the start time recorded in the lock (`startedAt`), allowing about 2 seconds of slack for rounding and the separate
  clock reads. Otherwise it treats the lock as stale (removes it) and leaves the process alone (D1).
- It keeps stopping a real background server through Astro's own stop command (D2).
- Every path that stops a server through the lock (the plain stop and the port takeover from another checkout's preview)
  gets the check.
- The script's comments describe the real case (a stale background lock whose number a later Astro command reuses), not
  the TODO's `bun run dev` example, and note that a backwards clock step larger than the slack fails safe (lock removed,
  server untouched).
- `sh -n` and `shellcheck` pass, and the manual exercise in Outline shows a later-started process left alone.
- The PR removes the TODO.md entry "Make the docs preview check that a lock's process is the one that wrote it."
- A single draft PR, having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words):** "Make the docs preview check that a lock's process is the one
that wrote it. Before asking Astro to stop the background server named in a checkout's `.astro/dev.json`,
`website/scripts/preview.sh` only checks that the process runs from that checkout's website directory. A stale lock
whose number now belongs to a later Astro server in the same checkout, such as your own `bun run dev`, would pass, and
the script would stop that server. Also require that the process started no later than the lock file was last written
(`ps -o etimes=` against the lock's mtime), and leave it alone otherwise. Two premises are unverified, so check them
against Astro's source when fixing: that Astro writes the lock after its server process exists, and whether a foreground
server rewrites the lock as non-background, which would make the scenario unreachable. From review feedback
`preview-lock-identity.md`, triaged 2026-10-05."

**What the planner found (at adf78aba, against Astro 7.3.5 as pinned in `website/package.json` and `website/bun.lock`;
re-check if the pin changed).**

- Both premises hold. The serving process itself writes the lock after its server is listening, with `pid`, `port`,
  `url`, `urls`, `background` and `startedAt` (an ISO timestamp, millisecond precision); a `--background` launcher waits
  until the lock names its child. A foreground `astro dev` deletes a lock whose process is gone and writes its own with
  `background: false`, which the script ignores; so the TODO's exact example (a later `bun run dev` inheriting the stale
  lock) is unreachable apart from a sub-second startup window. Astro removes the lock only on a clean stop, so a killed
  server or a reboot leaves it behind.
- The reachable case: a stale background lock whose process number is reused by another Astro command run from the same
  website directory (`astro build`, `astro preview`, `astro check`, `astro sync`), none of which touch the lock. The
  directory check passes, and `astro dev stop` kills any live process whose command line looks like Astro.
- In `preview.sh`, `stop_server_in` reads the pid through `lock_pid` (a `node -e` that returns the pid only for a
  background lock), checks it with `runs_in` (the process's working directory against the website directory), then calls
  `astro dev stop`, or removes the lock if the check fails. `take_port` reaches it too. The script is Linux-only by its
  own header (`/proc`, `ss`, GNU `stat`), so `ps -o etimes=` is available.
- There are no tests for the script; no CI job checks it. Earlier changes to it used the `docs:` commit type.
- `preview.sh start` takes the fixed preview port from another checkout's running preview without asking, so running it
  unattended can stop a preview the maintainer or another session is reading.

**The user's decisions (2026-10-08):**

- D1. Compare the process's start time with the lock's recorded `startedAt` (better than the lock file's modification
  time), with about 2 seconds of slack; if the process is newer, treat the lock as stale and leave the process alone.
- D2. Keep calling Astro's own stop command rather than replacing it with the script's own kill.
- D3. Review gate: gpt-6.1-sol high, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** `website/AGENTS.md` (the preview script and when it runs); root `AGENTS.md`
(Conventional Commits; Sharing the machine with other agents, in particular processes you did not start; Agent scratch
space; Docs website). SPEC_impl.md accepts process-number reuse inside short windows; this change is about the long
window after a stale lock.

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

## Outline

### PR 1: check the lock's start time before stopping

`docs:` (the script is maintainer tooling for the docs site, as its earlier changes were typed), so no changelog
fragment.

- `lock_pid`'s `node -e` also prints `startedAt` as epoch seconds (or the script reads it in a second small call); a
  lock without a parseable `startedAt` counts as stale.
- `stop_server_in` computes the process's start as now minus `ps -o etimes= -p <pid>` and requires it to be no later
  than `startedAt` plus the slack, in addition to `runs_in`; otherwise it takes the existing remove-the-lock branch.
- Comments: the real case, the slack, and the fail-safe on a clock step.
- Remove the TODO.md entry.

### Validation

`sh -n website/scripts/preview.sh` and `shellcheck website/scripts/preview.sh`. Then a manual exercise in your own
checkout only:

- Negative case, always: start a harmless long-running process whose working directory is your checkout's `website/`
  (for example `sleep`), write a background lock in your checkout's `website/.astro/dev.json` naming its pid with a
  `startedAt` earlier than its start, run `preview.sh stop`, and confirm the lock is gone and the process still runs.
  Then stop that process yourself. Never point a lock at a process you did not start.
- Positive case, only if `preview.sh status` reports the preview port free: `preview.sh start`, then `preview.sh stop`,
  and confirm the server is gone. If the port is in use, skip this and say so in the report; do not take the port from
  another checkout's preview.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-preview-lock-identity-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

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
  `plan/preview-lock-identity/<nn>-<short-name>`.
- The stack is the single PR in Outline. Keep PRs bite-sized without churn: never add code in one PR that a later PR of
  this stack deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on
  top; that applies to all of this plan's own open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entries this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
that PR's changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review
swarm: a fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the
executing one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (replacing Astro's stop command with the script's own
kill, a test harness for the script, or a check based on the lock file's modification time instead of `startedAt`; these
are examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions above, this
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
alternatives considered: in particular the slack chosen, how a lock without a usable `startedAt` is treated, and whether
the manual positive case ran or was skipped, and every review finding you decided not to follow. The user will ask for
these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
