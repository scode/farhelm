# Trim the add-host dialog: remove its two optional fields

Written against main at 5baee365 on 2026-10-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The hosts panel's add host dialog asks for an ssh destination and two more fields, **remote farhelm (optional)** and
**remote state dir (optional)**. Those two go: the dialog asks only for the ssh destination. Nobody adding a host knows
what to put in them, and adding a host already works without them: the probe finds the installed supervisor where setup
puts it, and setup records the binary and state directory it installed.

Only the dialog changes. The helm's HTTP API (`POST /api/hosts`, `POST /api/hosts/probe`), the `--ensure-hosts` JSON5
file, the registry columns, and the hosts panel's other uses of a row's stored paths keep the two fields. The browser
test harness and developer setups rely on them.

Acceptance criteria:

- The add host dialog shows the ssh destination field, the add button and cancel, and nothing for a remote binary or
  state directory. A probe sent from the dialog carries `null` for both fields.
- `api::probe_ssh_host` in `crates/farhelm-ui/src/api.rs` keeps its two path parameters and `install_field`: the host
  row's set-up/rerun action in `crates/farhelm-ui/src/provisioning.rs` (around `prepare`, `ProvisioningOperation::Add`)
  still sends the row's stored paths through it. The dialog passes empty strings, or its binding collapses to the
  destination alone; either way, do not remove the parameters.
- The two browser tests in `e2e/tests/terminal-multihost.spec.ts` that type into the fields keep working with those
  `.fill(...)` lines deleted (see Decisions, P1), and their comments stop saying the harness makes the fields mandatory.
- The docs screenshot spec `e2e/docs-shots/add-a-remote-host.spec.ts` no longer puts a callout on `.add-host-farhelm`.
- The Near term TODO.md entry "Remove the add-host dialog's two optional fields" is removed, and a changelog fragment
  exists.

## Decisions already made

Requirement sources are kept apart: the maintainer's words, binding repository rules, and planner proposals.

The maintainer's request and answers:

- M1. The TODO.md entry (Near term), verbatim: "The add host dialog's **remote farhelm (optional)** and **remote state
  dir (optional)** fields go. Nobody adding a host will know what to put in them, even the maintainer is unsure what
  they are for, and they are not a tested part of the UI."
- M2. Remove them from the dialog only. The API, the `--ensure-hosts` file and the stored host rows keep the fields.
- M3. Review gate: two fresh-context reviewers per PR, a Claude Opus 5.5 agent at high effort and a gpt-6-astra agent at
  high effort, both with the general review charter (below). No review swarm.
- M4. No-workhorse mode (below).

Binding repository rules:

- Root `AGENTS.md`: Conventional Commits; a `feat` PR carries a changelog fragment; the TODO entry is removed in the PR
  that addresses it; "Finishing work" for validation; `.agents/test-authoring.md` for test changes;
  `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` when browser tests change; browser specs run
  on Chromium and WebKit through the recorder.
- Root `AGENTS.md` "Docs screenshots": screenshots are regenerated in bulk by the maintainer's "refresh the docs
  screenshots" flow. Do not run `scripts/docs-screenshots.sh` or `scripts/publish-docs-shots.sh` here, and never commit
  a screenshot.

Planner proposals (with reasons; change them if the code says otherwise, and log a DECISION):

- P1. In `terminal-multihost.spec.ts`, the test "add-host-discovers: a form-registered ssh host progresses to connected"
  and the test that removes the remote host and re-adds it through the form both fill `.add-host-farhelm` and
  `.add-host-state-dir` with the harness's isolated binary and state directory. Delete only those `.fill(...)` lines.
  The tests still reach the harness's remote because `e2e/start-stack.sh` starts the helm with
  `FARHELM_E2E_PROVISIONING_BACKEND_DIR`, the file's `beforeAll` calls
  `configureDiscoveredProbe(info.remote_ssh, info.farhelm, info.remote_state)`, so the injected probe
  (`crates/farhelm-helm/src/provisioning/e2e.rs`) answers with those dial coordinates whatever the request carried, and
  registration in `crates/farhelm-helm/src/provisioning/service.rs` prefers the probe's coordinates over the request's.
  The `readded.remote_farhelm`/`remote_state_dir` assertions still hold; their meaning becomes "the discovered
  coordinates reached the row", and the comments should say that. Do not convert either test to an API registration or
  delete it: that would give up form coverage for nothing. Verify this premise by running the two tests; if it turns out
  wrong, that is a block, not a reason to delete coverage.
- P2. Update the `AddHostForm` docstring in `crates/farhelm-ui/src/hosts.rs` and the comment above `add-host-discovers`
  where they describe the two fields. The comment in the form about opting out of browser text mangling says "all three
  of these" fields; it now covers one.
- P3. `e2e/tests/provisioning.spec.ts` "blank optional fields and same-task double submit produce one real probe"
  already sends a destination only and asserts null fields. It keeps working; rename it so its title no longer refers to
  optional fields the user cannot see.
- P4. In the docs page `website/src/content/docs/docs/get-started/add-a-remote-host.mdx`, adjust the add-dialog
  screenshot's alt text only if it no longer reads right. The published image keeps showing the old fields until the
  next screenshot refresh; say so in the report so the maintainer can run that refresh.
- P5. Leave `docs/old_readme.md` alone: it is a parked old README, not current documentation.
- P6. Commit type `feat` (a user-visible change to the dialog), changelog fragment of kind `removed`.

## Implementation outline

UI and tests only: `crates/farhelm-ui/src/hosts.rs` (the add host form: its two `label`/`input` blocks, their signals,
the binding it builds, any Rust tests of the form), `e2e/tests/terminal-multihost.spec.ts`,
`e2e/tests/provisioning.spec.ts` (a rename), `e2e/docs-shots/add-a-remote-host.spec.ts`, possibly the docs page's alt
text, TODO.md, and a fragment under `releasing/changelog.d/`. No helm, supervisor, protocol, SPEC.md or SPEC_impl.md
change: the specs describe the registry fields, not the dialog. One PR.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-trim-add-host-dialog-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/trim-add-host-dialog/01-<short-name>`.
- One PR. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits; the `feat` PR carries its changelog fragment under `releasing/changelog.d/` in the same commit,
  per root `AGENTS.md` (Releases and the changelog); validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The PR stays a draft. Never mark it ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).
- Remove the Near term TODO.md entry "Remove the add-host dialog's two optional fields" in the PR.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, a nextest selection of `farhelm-ui` covering the hosts module,
`python -B scripts/check-test-sleeps.py`, `dprint check` on changed files, and the two changed `terminal-multihost`
tests plus the renamed `provisioning.spec.ts` test on Chromium and WebKit through the recorder. Browser evidence is
required here: P1's premise is what keeps the form coverage. Say in the report which specs ran and why.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes to TWO fresh-context
reviewers, as the user demands (M3): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra agent at high effort
(shelled out to the other harness if the executing one cannot reach that model natively). No review swarm. Both get the
same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. Because the PR changes tests, include the full text of `.agents/test-authoring.md` verbatim. Address
what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not write a launch
command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (any helm, API or spec change, removing the fields
from the API or the registry, rewriting or deleting the multihost tests, a new fixture; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the request, the decisions above, this outline, the current diff and
the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how the form's binding changes, the test rename, any alt text change, and every
reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. There is no agreed fallback beyond the planner proposals above. If P1's premise fails (the
tests cannot reach the harness remote without the fields), record what you found and block per `plans/AGENTS.md`
(Executing one plan, step 10) rather than deleting or rewriting the tests.

## Done criterion

The plan is complete when its draft PR exists, satisfies the acceptance criteria, has passed the review gate, and has
removed the TODO.md entry. Open, not merged: merging happens only after the maintainer has reviewed this plan's report.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
