# Uninstall symlinked lib: refuse a symlinked program directory on a remote host

Written against main at bd9d6643 on 2026-10-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file directs a change to them.

## The goal

The hosts panel's uninstall action removes Farhelm from a remote host: it disables and removes the supervisor's unit,
removes Farhelm's private lib directory (`~/.local/lib/farhelm`, holding the binary and any private tmux) with `rm -rf`,
and forgets the host. SPEC.md describes it in the hosts section ("Removing Farhelm from a remote host is a host action
too ..."), SPEC_impl.md in the UNINSTALL paragraphs of its provisioning section.

The bug: if the lib directory is itself a symlink, uninstall's checks pass, because they compare canonical paths (the
binary the supervisor runs resolves inside the link's target). The removal step then runs `rm -rf` on the link, which
removes only the link. Uninstall reports success and forgets the host, while the target directory and the Farhelm binary
stay on the host.

After this plan, uninstall refuses that layout before changing anything on the host, with a message naming the symlink
and where it points, and the host stays listed with its data untouched. Removing the host from the list without
uninstalling (the separate Remove action, which never touches the host) keeps working for such a host exactly as it does
today.

The source TODO.md entry is "Refuse symlinked program directories during remote uninstall" (a follow-up to the remote
uninstall change in PRs #1565 and #1568). The maintainer's words, from the planning conversation: "if an UNINSTALL is
attempted we should check and refuse etc. But we should never block the user from being able to remove a host from the
list WITHOUT uninstalling if that's what they want, symlink or not."

Acceptance criteria:

- Uninstall planning refuses when the lib directory path itself is a symlink (`[ -L ]` on the host, a dangling link
  included). The refusal is the ordinary typed planning refusal the other uninstall checks use, so it applies at
  planning and again at confirmation (confirmation plans again and requires the same plan). Nothing on the host changes
  and the host stays listed.
- The refusal message names the lib directory and the symlink's target as the host reports it, says uninstall does not
  run on this layout, and points to removing the host from the list as the way to stop using it without uninstalling.
  Match the voice of the neighbouring refusals in `plan_uninstall`.
- A symlink further up the path (a symlinked home directory, or `~/.local`) is not refused: `rm -rf` of the path then
  removes the real directory, which is the intended behavior.
- The removal step itself refuses, in the same shell command as the `rm`, when the path is a symlink at the moment of
  removal (it became one after confirmation). That shows as a failed step: the host stays listed, and choosing uninstall
  again re-plans and is refused at planning. This mirrors how unit-file removal already repeats its check in the same
  shell command (SPEC_impl.md).
- Remove (forgetting the host without uninstalling) is unaffected for a host whose lib directory is a symlink. Confirm
  from the code that nothing in this change reaches it; add a test only if some shared path makes it plausible.
- SPEC.md's list of what uninstall refuses (hosts section) gains a lib directory that is itself a symlink, with the
  pointer to Remove. SPEC_impl.md's UNINSTALL paragraphs say where the check happens (inspection and planning) and that
  the removal command repeats it.
- A `fix` changelog fragment under `releasing/changelog.d/`, in the same commit.
- The last PR removes the TODO.md entry "Refuse symlinked program directories during remote uninstall".

## Decisions already made

Settled with the maintainer during planning. Do not reverse them.

1. **Refuse at planning and re-check in the removal command.** Both checks belong to uninstall only.
2. **Never block Remove.** Removing a host from the list without uninstalling must keep working for any layout.
3. **The refusal points to Remove** as the way to stop using the host without uninstalling.
4. **Review gate:** two fresh-context reviewers per PR, Claude Opus 5.5 at high effort and gpt-6-astra at high effort,
   no review swarm (How to run, Review gate).
5. **No-workhorse mode** (How to run).

## Implementation outline

Planner proposals unless marked as a decision. Grounded in main at bd9d6643.

**Today.** The service gathers facts with `ProvisioningBackend::inspect_uninstall`
(`crates/farhelm-helm/src/provisioning/service.rs`, around `uninstall_plan`), whose SSH implementation in
`crates/farhelm-helm/src/provisioning/backend.rs` runs one shell script: a `resolve` helper prints, per asked path, an
existence flag (a dangling symlink counts as existing) and `readlink -f`'s canonical path, NUL-separated, then the state
directory and the unit's systemd facts. `parse_uninstall_inspection` turns that into `UninstallInspection` (one
`HostPath` per asked path: `path`, `exists`, `canonical`). The service builds `UninstallFacts`
(`crates/farhelm-helm/src/provisioning/plan.rs`; `lib_dir` is the canonical path when it exists), and
`PlanLayout::plan_uninstall` refuses on the binary-outside-lib and state-inside-lib checks, then plans
`RemoveDirectory { path: paths.lib_dir }` with the path as named. `SystemBackend::remove_directory` runs
`d=...; if [ ! -e "$d" ] && [ ! -L "$d" ]; then printf absent; exit 0; fi; rm -rf -- "$d"`.

**Inspection.** Report whether the lib directory path is itself a symlink (`[ -L ]`). Proposal: ask it for the lib
directory only, as one extra field of `UninstallInspection`, rather than adding a third field to every asked path; take
the other shape if the code argues for it and log the DECISION. Update the parser's tests and the `cfg(test)` scripted
backend in `crates/farhelm-helm/src/provisioning.rs`. The Playwright-only backend in
`crates/farhelm-helm/src/provisioning/e2e.rs` touches no filesystem (it reports every path as present); it just reports
"not a symlink".

**Where the refusal runs.** The service's `uninstall_plan` (`provisioning/service.rs`) already refuses, before
`plan_uninstall` is reached, when the host cannot resolve an asked path ("uninstall cannot tell where ... leads on this
host ..."); `readlink -f` prints nothing for a dangling link whose target's parent is missing. So the symlink refusal
has to run before that resolution check, or a dangling link gets the older message without the pointer to Remove. Either
check the flag in the service ahead of resolution, or carry it into `UninstallFacts` and order things so the symlink
refusal wins; log which. Name the link's target from the inspection's canonical path when the host resolved one, and say
the target could not be resolved otherwise. Keep the existing canonical checks as they are.

**Removal.** In `remove_directory`'s script, refuse when `[ -L "$d" ]` before the `rm`, with output the Rust side turns
into a `BackendFailure` whose message says the directory became a symlink and uninstall stopped without removing it.
Keep `absent` handling as is (a dangling link is `-L`, so it now refuses rather than being removed; that matches the
planning rule).

**Tests.** A planning test that a symlinked lib directory refuses with the target named, through whichever layer holds
the refusal, including a dangling link. A parser test for the new field. The real-shell test
`uninstall_host_commands_refuse_setup_units_and_skip_finished_work` in `crates/farhelm-helm/src/provisioning.rs` already
runs the real inspection and removal scripts against fixture paths with a fake `systemctl`; extend it (or add a sibling
in the same style) with: a symlinked lib directory, where inspection reports the flag and `remove_directory` refuses and
leaves both the link and its target in place; and a lib directory under a symlinked parent (a symlinked `.local`, say),
where inspection reports no symlink, which is the only level at which the ancestor rule is actually tested. If an
existing service-level uninstall test (`uninstall_refuses_hosts_whose_removal_farhelm_does_not_own` and its neighbours)
can take the case cheaply, add it there to cover planning and confirmation through the service. No new harness and no
Playwright spec.

**Size.** Small. Suggested stack: one PR.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-uninstall-symlinked-lib-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
  `plan/uninstall-symlinked-lib/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits. The change is a `fix` and carries a changelog fragment under `releasing/changelog.d/` in the
  same commit, per root `AGENTS.md` (Releases and the changelog). Validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge (`plans/AGENTS.md`).
- Changes to tests or fixtures follow `.agents/test-authoring.md`; run `python -B scripts/check-test-sleeps.py` per root
  `AGENTS.md` when Rust tests change.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, a nextest selection of the helm's provisioning module (its uninstall
tests and the real-shell host-command test), and `dprint check` on changed files. If the Playwright filesystem backend
changed, consider whether a hosts-panel spec covers it. Say in the report which checks ran and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands (Decision 4): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra
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

Before implementing a substantial departure from the outline above (refusing other symlinked paths, a general
filesystem-identity check, changing Remove, reworking the inspection protocol beyond the new flag; these are examples,
not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context
review through galaxy-brain with this charter, supplying the request, the decisions above, this outline, the current
diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the inspection field's shape, the refusal's exact wording, how the removal
command reports a symlink, the tests chosen, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The planner proposals in the outline are yours to change when the code argues otherwise. Not
covered, and a reason to block per `plans/AGENTS.md` (Executing one plan, step 10) after recording the concrete
tradeoff: anything that would make Remove refuse or change for some layout, removing the symlink's target directory
instead of refusing, or refusing symlinked ancestors.

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
