# Keep one copy of the rule for a template's launcher tab

Written against main at a38ea524 on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

A launch template that has no launch kind (stored before templates recorded which launcher tab they switch to) gets one
inferred from its fields. That rule, and the two field groups it is built from, are written out in three places. Move
them into the shared protocol crate so each field list exists once, with no change in behavior.

Acceptance criteria:

- `farhelm-proto`'s launcher module (`crates/farhelm-proto/src/launcher.rs`) holds the inference as a method on
  `TemplateFields` (something like `implied_kind()`), built on two named field-group helpers (something like "sets a
  command-launch field" and "sets an agent-launch-only field"). Each field list is written exactly once.
- The helm's `with_launch_kind` and `mixes_launch_kinds` (`crates/farhelm-helm/src/agent_requests.rs`) and the Templates
  dialog's `Draft::from_template` (`crates/farhelm-ui/src/list/templates.rs`) use them, and no longer list fields
  themselves.
- Behavior is unchanged: the existing helm and UI tests pass without edits, and a new table test in the proto crate pins
  the inference field by field.
- The PR removes the TODO.md entry "Keep one copy of the rule for a template's launcher tab."
- One draft PR, which passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words):** "Keep one copy of the rule for a template's launcher tab. A
template stored before templates chose their launcher tab has no tab, so Farhelm infers one from its fields: any command
field (`command`, `yolo`, `resume_command`) means command, otherwise any agent field (`agent`, `model`, `effort`,
`permissions`, `workspace_trust`) means agent, otherwise it switches nothing. That rule is written twice, in the helm
(`with_launch_kind` in `crates/farhelm-helm/src/agent_requests.rs`, used when an agent creates a template) and in the
Templates dialog (`Draft::from_template` in `crates/farhelm-ui/src/list/templates.rs`, used when an old template is
opened), and the helm's `mixes_launch_kinds` repeats the same two field lists. The copies agree today, but nothing keeps
them in step: a field added to one list and not the other would make the same template switch to different tabs
depending on who saved it, with no test to notice. Move the rule into the shared protocol crate
(`farhelm-proto::launcher`, which already holds the template types and `apply_template` and which the web UI can use,
unlike the helm), as something like `TemplateFields::implied_kind()`, and have all three call sites use it. No reason
for the duplication is recorded."

**The user's decisions (2026-10-08):**

- D1. The maintainer saw the planner's correction below (`mixes_launch_kinds` does not repeat the same lists, and the
  plan keeps its difference) and raised no objection.
- D2. Review gate: "gpt-6.1-sol high effort, no swarm for reviewers."
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**What the planner verified (at a38ea524):**

- `with_launch_kind` and `Draft::from_template` implement the same rule: command if `command`, `yolo` or
  `resume_command` is set; otherwise agent if `agent`, `model`, `effort`, `permissions` or `workspace_trust` is set;
  otherwise no kind. Both apply it only when `kind` is `None`.
- `mixes_launch_kinds` is different, on purpose. Its agent side is `kind == Agent` or `model`, `effort`, `permissions`,
  `workspace_trust`: it leaves out `agent`, because an agent type beside command fields is the command launch's declared
  agent type and is allowed. Its command side is `kind == Command` or `command`, `yolo`, `resume_command`. Folding
  `agent` into a shared agent group used by this check would start refusing agent-plus-command templates that are valid
  today. That is why the plan needs two field groups rather than only `implied_kind()`: a template with an agent-only
  choice plus a command field and one with an agent type plus a command field both infer the command kind, but only the
  first mixes kinds.
- Several fields are `Option<Option<_>>`, where an explicit `null` (a reset, such as `resume_command: null`) is still a
  choice. The helpers must test the outer `is_some()`, as the current code does.
- `farhelm-proto`'s launcher module is not behind the `io` feature, so the web UI can call it.
- `apply_template` in the same module carries its own per-field checks with labels for refusals. It is out of scope;
  leave it alone (mention it in the PR description only if it helps a reader).

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Releases and the changelog;
Harness-specific code; Sharing the machine with other agents; Agent scratch space; The live install is off-limits),
`plans/AGENTS.md` (Executing), and `.agents/test-authoring.md` for the new test.

## Outline

Line numbers drift; find the code by name.

### PR 1: one copy of the launch-kind rule

`refactor:`, no changelog fragment (no behavior change).

- In `crates/farhelm-proto/src/launcher.rs`, on `TemplateFields`: two public field-group helpers (names are yours; log
  them), one for the command-launch fields (`command`, `yolo`, `resume_command`) and one for the agent-launch fields
  other than the agent type (`model`, `effort`, `permissions`, `workspace_trust`), and `implied_kind()`, which is pure
  inference that ignores `kind`: command if the command group is set, otherwise agent if the agent type or the agent
  group is set, otherwise `None`. Document the asymmetry (why the agent type is not in the agent group) on the helpers,
  since it is exactly what a future reader would "fix".
- Callers keep "an explicit kind stays": `with_launch_kind` and `Draft::from_template` become
  `fields.kind = fields.kind.or(fields.implied_kind())` or equivalent. `mixes_launch_kinds` stays in the helm and is
  rebuilt from the two groups plus the explicit kind.
- A table test in the proto crate: each field alone implies the expected kind (including `model`, `effort`,
  `permissions` and `workspace_trust` alone, which the helm's test does not cover, and a `null` reset counting as set),
  the agent type alone implies agent, the agent type plus any command field implies command, and placement-only fields
  (`host`, `destination`, `name`) imply none.
- Leave the existing helm test (`template_create_infers_kind_without_reinterpreting_command_agents`) and the UI tests
  (`legacy_kind_is_inferred…`) unchanged; they are the evidence that behavior did not move.
- Remove the TODO.md entry.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, and through `scripts/record-test-run.py` (with the pinned nextest and
tmux setup from `docs/test-run-evidence.md`) a nextest selection covering `farhelm-proto`'s launcher tests, the helm's
agent template tests and the UI crate's template tests. Run `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md`, since Rust tests change. No browser tests: nothing user-visible changes.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-template-launch-kind-rule-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
  `plan/template-launch-kind-rule/<nn>-<short-name>`.
- The stack follows Outline. Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack
  deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top; that
  applies to all of this plan's own open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entry this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code or tests, use the active galaxy-brain skill to delegate a review of that PR's
changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review swarm: a
fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing
one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

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

Before implementing a substantial departure from the outline above (changing how apply_template checks fields, moving
mixes_launch_kinds out of the helm, changing the template wire format, migrating stored templates; these are examples,
not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context
review through galaxy-brain with this charter, supplying the user's request and decisions above, this outline, the
current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled
out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the helpers' names and where the asymmetry is documented, and every review
finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code or tests passed the review gate. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
