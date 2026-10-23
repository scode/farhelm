# Five helm provisioning guards: uninstall refusals and a bounded inspection read

Written against main at c3345af7 on 2026-10-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Uninstall from the hosts panel must not delete data it promises to keep or end sessions it never checked, and a stuck
shell inspection must not hold the helm's host-planning slots. Four refusals and one bound: uninstall refuses when kept
data could lie inside the removed installation through a home alias, and when a mount lies inside the installation; an
offline retry refuses while the supervisor unit is still running; uninstall planning rejects a kept-data path with
control characters before confirmation; the post-exit output read in the shared capture path gets a short bound.

Carry out these 5 triage outcomes, each recorded under its own heading in `TRIAGE_OUTCOMES.md`, in this order (ledger
order, which `review_feedback_queue/.agents/execute-outcomes.md` asks for), one PR each. Read every entry in full first.
Each has a size gate of about 100 lines; over it, block and bring it back.

1. `uninstall-two-home-aliases-hide-kept-data.md` (`fix code`)
2. `uninstall-home-bind-inside-installation.md` (`fix code`)
3. `offline-uninstall-retry-skips-session-check.md` (`fix code`)
4. `uninstall-notice-copies-raw-remote-path.md` (`fix code`)
5. `shell-inspection-drains-exhaust-planning-pool.md` (`fix code`)

## Outline

Facts from the triage session's reading of main at b97bcfe4 (verify them; line numbers drift). The uninstall code is in
`crates/farhelm-helm/src/provisioning/` (`service.rs` planning around 1245-1498, `backend.rs` `inspect_uninstall` around
2440-2539 and `remove_directory` around 2669-2693, `plan.rs` `plan_uninstall` around 780-870). Read SPEC.md "Topology"
(uninstall) and the home-alias text in "Maintainer-confirmed decisions" (around 3528-3536: uninstall "still refuses
layouts it cannot tell apart safely") first.

- PR 1: `backend.rs` (around 2490-2511) rewrites each path separately, and `plan.rs` (around 820-822) compares raw with
  raw and rewritten with rewritten, so state reached through one alias of home and an installation reached through
  another, with a nested mount visible through only one, escapes the containment check. In the inspection script, test
  each ancestor of the kept state path with `-ef` against the resolved installation folder and refuse on a match. This
  only adds refusals, consistent with the comment around 2464-2473 against accepting aliases. Test with faked inspection
  output. This fixes unreleased work: edit `releasing/changelog.d/uninstall-home-alias.md` rather than adding a
  fragment.
- PR 2: `remove_directory` runs `rm -rf` with no mount guard; `rm --one-file-system` does not help because a bind on the
  same filesystem has the same device number, and BusyBox `rm` lacks the flag. Refuse when any mount point lies inside
  the installation folder (read `/proc/self/mountinfo` or equivalent), both during inspection and again in the same
  shell as the removal, as the existing symlink recheck does. Test with faked inspection output.
- PR 3: with no connected client, uninstall planning (`service.rs` around 1302-1361) runs no session check, and the
  probe reports absence whenever a custom command name (such as `fh`) no longer resolves (`backend.rs` around
  1251-1258). Refuse an offline retry, with an explanation, while the host's supervisor unit is still running (the
  connected path already refuses a running unit around 1425). The crash route stays out of scope. Test.
- PR 4: uninstall planning never validates the kept state path; only the stop step does when it builds its script
  (`shell_path`/`path_text`, `backend.rs` around 2813-2833, delegating to `units.rs` around 471-482). Apply the same
  check while planning and refuse before confirmation. The hidden-characters copy warning in the GUI is not part of this
  outcome. Test.
- PR 5: `capture_child` (`backend.rs` around 1585-1629, `finish_child_output` around 1807-1812) awaits its output
  readers without a deadline after the child exits or is killed; Set up, Update and Remove planning each hold one of the
  four planning slots across it (`service.rs` around 51, 616, 1084, 1364, 1382). Bound the post-exit reads the way the
  probe's 2-second bound-and-abort already does (`stop_probe`, from the
  `abandoned-planners-exhaust-global-planning- slots.md` ledger entry). Because install and update share
  `capture_child`, this narrows the BUGS.md entry "A host install or update can hang until the helm restarts if a local
  ssh helper keeps its output open": rewrite it to match what remains (the file-transfer path keeps its hang; bounding
  it is not part of this outcome). This fixes unreleased work: edit `releasing/changelog.d/planning-probe-bound.md`,
  which also overstates the earlier fix, rather than adding a fragment. Test that a lingering descendant holding the
  pipe no longer holds the call.

### Validation

Each PR: the `farhelm-helm` provisioning unit tests through the recorder (`.agents/narrow-tests.md`); for the shell
script changes in PRs 1-2 a test that runs the inspection script fragment against a faked layout if the existing tests
do that, otherwise the plan-level tests with faked inspection output. `python3 releasing/check-changelog.py format`
where a fragment changes. `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all -- --check` once at the tip.
Do not run the installer, `farhelm helm setup`, or anything against the live install (root `AGENTS.md`). No full
battery.

## Fallbacks

Each outcome's gate is binding: over about 100 lines, block and bring it back. Examples of departures that need the
scope reassessment: a general filesystem-aliasing redesign (PR 1), a recursive removal reimplemented outside `rm` (PR
2), checking the helm's last-known sessions for the crash route (PR 3), or a GUI copy warning (PR 4).

Decisions to log: each PR's measured size; for PR 2 how mount points are found on the supported host systems; for PR 5
what the BUGS.md entry says afterwards.

## Gates, acceptance and requirement sources

### Size gates

Each outcome's gate, where it has one, is in its `TRIAGE_OUTCOMES.md` entry's Completion criteria and is binding: about
100 lines of diff, or the larger number the entry states (code, tests and docs, not counting `TRIAGE_OUTCOMES.md`, the
queue removal or the changelog fragment). The maintainer prefers size gates to time estimates. Over the gate, block and
bring it back to the maintainer (an entry that says "as above" means this): block per `plans/.agents/executing.md`,
"Executing one plan", step 10, after building and pushing the plan's other outcomes, with a question that describes the
larger fix, its measured size, and asks whether to build it or discard the outcome. Measure and log each outcome's size.
The entry's described fix is the triaging session's proposal; a different fix within the same gate that meets the same
completion criteria is fine, logged as a DECISION.

### Acceptance criteria

- Each outcome's Completion criteria in its `TRIAGE_OUTCOMES.md` entry, including any test it names.
- Each outcome's PR removes its feedback file and `review_feedback_queue/INDEX.md` entry and updates its Execution
  field.
- A `fix` PR follows `AGENTS.md`, "Changelog fragments". A fix to work not yet in a stable release (check whether the
  change being fixed shipped in the latest stable tag) edits that work's existing fragment under
  `releasing/changelog.d/` instead of adding a second one; this file's outline names the fragment where the planning
  session found one. Otherwise it adds a fragment (`kind: fixed`, or `kind: none` with the reason when nothing is
  user-facing). `docs`, `test`, `chore` and bookkeeping PRs carry none.
- Any PR that changes Rust or browser tests or their helpers runs `python -B scripts/check-test-sleeps.py` with the
  isolated interpreter `docs/test-sleep-check.md` prepares (root `AGENTS.md`, "Tests and validation"); a Rust change
  also runs `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` at least once at the tip.

## Requirement sources

**The user's decisions (2026-10-22):** the maintainer decided each outcome on a triage page that day, after a read-only
verification of every finding against main; each outcome's Decision field records the choice and, where they gave one,
their words. Where a principle question decided several outcomes at once, the Decision field says which. While
scheduling, the maintainer agreed to that day's 40 pending outcomes being scheduled as ten plans grouped by area (this
is `uninstall-guards-oct22.md`) and chose the review gate below: a single fresh-context Opus 5.5 agent at high effort
with the general charter, no swarm. No-workhorse mode, which `plans/.agents/planning.md` requires.

**Binding repository constraints:** the rule each entry's Assessment cites;
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes"; `AGENTS.md`, "Changelog fragments",
"Conventional Commits", "Code rules" and "Tests and validation", with `.agents/validation.md`;
`.agents/test-authoring.md` for any test change; and anything this file's outline names as binding for the plan.

**Planner proposals:** the PR order and the outline above, including the notes a fresh planning review added. A planner
proposal is not a requirement: a smaller design that meets the same completion criteria is fine, logged as a DECISION.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-uninstall-guards-oct22-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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

- Follow `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes": one reviewable commit, stable
  bookmark and draft PR per outcome (unless this file groups some outcomes into one PR, which the maintainer agreed for
  those cases only), in a single linear stack in the order given under The goal above. Each PR carries its outcome's
  agreed change, the removal of its feedback file and `review_feedback_queue/INDEX.md` entry, and its
  `TRIAGE_OUTCOMES.md` Execution update. Record the jj change ID and bookmark in that Execution field before creating
  the PR, then add the PR's URL (with `/changes` appended) to the same change and push it again; no separate bookkeeping
  PR. Replace the Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/uninstall-guards-oct22/<nn>-<short-name>` (for example `plan/uninstall-guards-oct22/01-short-name`).
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
demands exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high
effort, launched as a native sub-agent where your harness serves that model and otherwise through
`$scode-harness-shellout` (that skill owns the launch mechanics). The prompt carries the full charter, because the
reviewer has nothing else:

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
  `TRIAGE_OUTCOMES.md` entry, including any size gate (ask the reviewer to flag anything beyond the gate).

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
alternatives considered: in particular those named under Fallbacks above, whether each gated outcome met its gate (with
the measured diff size) and why, and every review finding you decided not to follow. The user will ask for these later.

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

The plan is complete when a linear stack of draft PRs exists, one per outcome listed under The goal unless this file
groups some into one PR (an outcome discarded under its gate still gets its bookkeeping PR; one whose gate says to bring
it back makes the plan block instead, after its other outcomes are pushed), that together meets the acceptance criteria
above, each PR passed its review gate, and each outcome's `TRIAGE_OUTCOMES.md` Execution field names its change ID,
bookmark and PR. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then
close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the
queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Decisions

### 2026-10-23: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-10-23 (claim 8c534c).
>
> ### Build the home-alias uninstall refusal at about 150 lines, or discard it?
>
> **The problem.** Uninstalling Farhelm from a host deletes its installation folder and promises to keep the host's
> Farhelm data folder. On a host where the home folder can be reached under two different paths (for example two bind
> mounts of it), with another mount visible under only one of them, the installation can be named through one path and
> the data folder through the other. Uninstall's check that the data folder is not inside the installation compares path
> spellings, so it can miss that layout and delete the data it promised to keep. Nobody has reported this happening; it
> was found by reading the code. On 2026-10-22 you chose to refuse such layouts (SPEC.md's home-alias decision already
> says uninstall "still refuses layouts it cannot tell apart safely"). The triage entry,
> `uninstall-two-home-aliases-hide-kept-data.md`, set a size gate of about 100 lines and said to bring it back if
> larger.
>
> **What I built, measured and did not push.** The fix asks the host, for each path it checks, whether any folder above
> it is the same folder as the one uninstall would delete, comparing the folders themselves rather than their names, and
> uninstall refuses when the data folder's answer is yes. It only adds a refusal. A test runs the real host-side check
> against a fake aliased home and confirms the refusal (it fails without the fix). It came to about 154 changed lines:
> roughly 80 of code and comments, 75 of tests. About 30 of those are mechanical, because the host's answer is a new
> field in a result that several fakes, a parser test and other call sites build. It does not fit in 100 without
> dropping the test or the explanation. The plan's four other uninstall fixes in this group are built as draft PRs; they
> land once you answer, either way.
>
> **Options.**
>
> - **Build it at about 150 lines.** Uninstall then refuses every layout where the data folder sits inside the
>   installation under another name. It also refuses a few layouts that would in fact be safe (one alias with a nested
>   mount where removal would leave the data alone); that is the refusal side SPEC.md already accepts. Cost: one more PR
>   of this size.
> - **Discard the outcome.** Uninstall keeps today's check by spelling. The data-loss layout stays possible on hosts set
>   up that way, which no user has reported.
>
> **Recommendation:** build it. It closes a data-loss path with a refusal only, the excess over the gate is mostly
> mechanical plumbing and a real end-to-end test, and the approach is proven; the next run would rebuild it as
> described.

The maintainer's answer:

### Answer (2026-10-23): build it as described; treat about 700 changed lines as the size cap

The maintainer's words, verbatim, given in conversation before this block was raised, about this plan's PR over its size
gate:

"when this bubbles up can you just unlbock the plan and allow more complexity, lets just say 700 lines cap."

Restatement: build the home-alias uninstall refusal as the Blocked section describes (about 154 changed lines, including
its end-to-end test), and treat about 700 changed lines as this plan's size cap in place of the triage entry's 100-line
gate. Then land it with the plan's four other uninstall fixes.
