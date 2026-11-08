# Tensorlake recipes: a base Farhelm always builds on

Written against main at c277fa78 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

This is the first of three chained plans toward "sandbox sessions": a session launched from a template or the launcher
that runs in a Tensorlake sandbox of its own, created from a recipe. The TODO.md entry they share is "Phase 2: a sandbox
per session" (bucket "Near term", subsection "Tensorlake"). The design records are
`lore/2026-11-03-tensorlake-sandbox-hosts-design.md` and
`lore/2026-09-29-per-session-sandboxes-as-the-remote-host-goal.md`. The later plans are `prepared-snapshots.md` (a
Farhelm-managed snapshot with the supervisor already installed) and `sandbox-sessions.md`. This plan touches neither of
those features. It only changes what a recipe's software is, so that every recipe yields a snapshot Farhelm built.

The maintainer's words while planning (2026-11-07 and 2026-11-08):

> right now we have a recipe for building a sandbox. but its super had-hoc.

> and the user should also be able to provide a template sandbox snapshot in tensorlake that satisfies criteria

> lets have us still install claude/codex by default (we can aworry about turning it off later). both using official
> installers like I think we do today already.

mise was discussed and then dropped by the maintainer: "actually just drop mise entirely for now just to keep things
simple. no mise at all in the entire plan." Nothing in this plan installs or mentions mise.

### What changes

Today a recipe's software is either **Install packages** (Farhelm builds a snapshot from `tensorlake/ubuntu-systemd`
with apt packages, Claude Code and Codex) or **Use an existing image or snapshot** as is (nothing built, no agents
installed). See SPEC.md's recipe paragraphs (around "Tensorlake recipes belong to the helm too" and "A package recipe's
hosts start from a snapshot Farhelm builds").

After this plan a recipe has a **base** and Farhelm always builds on it:

- **Base**, one of three: **Tensorlake Ubuntu** (today's `tensorlake/ubuntu-systemd` base, held in one place as now),
  **Your image** (a Tensorlake image name in the user's project), or **Your snapshot** (a Tensorlake snapshot ID).
- **Ubuntu packages**, optional, for any base.
- Then Claude Code and Codex from their official installers, exactly as today's build does, then the snapshot. There is
  no switch to leave the agents out yet; the maintainer deferred that.

Every recipe now builds, so "Use an existing image or snapshot as is" disappears. The build's existing early check
becomes the base's requirements check. Today it refuses unless systemd is PID 1 and passwordless sudo works
(`crates/farhelm-helm/src/tensorlake/build_script.rs`, `ready`). It must also check that apt is available (Ubuntu or
Debian) and that bash, curl and git exist. It stops at the first missing requirement and names it, in the recipe row's
existing failure line. This check replaces the separate "Check snapshot" action the planner first proposed.

### The UI that was agreed (mocked up on the planning page)

In the recipe editor, the **Software** block becomes:

1. Three cards in the existing `recipe-source-card` style:
   - **Tensorlake Ubuntu**: "Tensorlake's Ubuntu image with systemd. Nothing else to prepare."
   - **Your image**: "An image in your Tensorlake project, by name."
   - **Your snapshot**: "A snapshot in your Tensorlake project, by ID."
2. For the last two, the existing Image or Snapshot ID input, followed by a requirements note. It says Farhelm builds on
   the base, so the base must:
   - boot systemd as its init system;
   - let the sandbox user use sudo without a password;
   - be Ubuntu or Debian, with apt;
   - have bash, curl and git.

   It adds that the build checks these first and stops with the one that's missing. This replaces today's systemd-only
   note.
3. The existing **Ubuntu packages** textarea, now shown for every base.
4. A short help line: "Farhelm then adds Claude Code and Codex with their official installers and takes a snapshot."

The recipe list row's summary names the base, for example:

> 4 CPU · 8 GB · 40 GB · base: your image my-team/devbox-base · 4 packages · 1 tunnel

A Tensorlake Ubuntu base reads "base: Tensorlake Ubuntu". The build line keeps today's format: "Snapshot built … ·
Claude Code … · Codex …". A failed base check reads like any other failed step, for example:

> Snapshot build failed while checking the base (step 4 of 9): the sandbox user cannot use sudo without a password.

Pick the step names that match the code; adding or renumbering steps is fine.

### Acceptance criteria

- A recipe stores a base: Tensorlake Ubuntu, an image name, or a snapshot ID. It also stores optional packages and
  today's size, agents, GitHub and tunnels. Build snapshot, Rebuild snapshot and Retry work for every base. Every build
  installs the packages (if any), Claude Code and Codex, and then takes the snapshot, exactly as today's package build
  does.
- The build checks the base's requirements before installing anything (systemd as PID 1, passwordless sudo, apt, bash,
  curl, git). It stops at the first one missing with a message naming it. A Tensorlake Ubuntu base passes.
- Upgrading converts recipes saved as "reuse image" or "reuse snapshot" to that base, with their packages empty.
  - Such a recipe has no snapshot until one is built. Its row shows today's "No snapshot yet" state with Build snapshot;
    adding a host from it builds the snapshot first, as Add host already does for a package recipe with none.
  - Hosts already created from it are untouched. They keep running, reconnecting, suspending and deleting as today, and
    **Rerun** setup on such a host must not start a snapshot build: the sandbox already exists, so setup skips the
    snapshot step whenever the host's sandbox exists (`provisioning/tensorlake_add.rs`, `tensorlake_steps`). Today that
    path assumes a reuse recipe never builds; check it and cover it with a test.
  - Adding a host from it needs the built snapshot, like any recipe.
- Existing package recipes migrate to the Tensorlake Ubuntu base with their packages and their built snapshot kept, so
  their rows and hosts are unchanged.
- The rule that a recipe cannot reuse a snapshot Farhelm built for a recipe still holds for the snapshot base, with its
  refusal message. Switching base deletes nothing until the next build replaces the snapshot, as a rebuild does today.
  Check the current "switching from packages to an existing image deletes its built snapshot" rule and adapt it; log the
  DECISION.
- Add host → Tensorlake sandbox works as today for every recipe with a built snapshot. The setup paths that existed only
  for reuse recipes (no build, no agents installed) are removed. The logind and linger preparation setup step stays.
- SPEC.md's recipe paragraphs and SPEC_impl.md's matching sections describe bases, the always-build rule, the
  requirements check and the migration. Remove the "reuse … as is" text.
- Settings and the add-host dialog's inline recipe editor both show the new Software block. The add-host summary under
  the Recipe picker names the base where it used to name the software source.
- A changelog fragment (`kind: changed`) tells users that recipes now have a base that Farhelm always builds on, and
  that recipes which reused an image or snapshot need one build before adding hosts.

## Decisions already made

Settled with the maintainer on 2026-11-07 and 2026-11-08, in conversation and on a planning page with mockups:

- **Base plus layers, always built**, as above. The maintainer chose "your recommendation", which was the user's
  snapshot as a base Farhelm builds on, over "use as is".
- **Agents always installed** with the official installers. A switch to leave them out comes later.
- **No mise** anywhere.
- **Existing reuse recipes are converted** to that base with no snapshot. The alternative of keeping "as is" for hosts
  only was rejected.
- **No separate "Check snapshot" action.** The build's early check covers it.
- **Three chained plans.** This one goes first, and `prepared-snapshots.md` runs after it lands.
- **Code reviewer:** a single fresh-context Opus 5.5 agent at high effort with the general charter below.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Real sandboxes are authorized** for verifying this plan. Asked per plan, the maintainer answered "yes, up to 100 its
  fine" (2026-11-08). So:
  - create at most 100 sandboxes over this whole plan;
  - use the smallest size that works;
  - terminate each one and delete every snapshot it made as soon as its check is done;
  - keep a ledger of every sandbox and snapshot you create (name and ID) in the scratch directory, and sweep from it
    before anything else after an interruption;
  - report the count.

  Never use the maintainer's live Farhelm install or its helm to create or reach sandboxes (root `AGENTS.md`, "The live
  install is off-limits"). Run a throwaway test helm with its own temporary state; its own helm tag keeps its sandbox
  names apart from the maintainer's. Follow `.agents/tensorlake.md` for the ledger and sweep habits. Its "only on
  request" rule is satisfied by this decision, for this plan only. Register any SSH key a test needs with the signed-in
  `tl` CLI, and remove it when done. If the machine has no usable Tensorlake credentials, block with that as the
  question rather than skipping the verification.

## Requirement sources

**The user's request:** the quotes and decisions above.

**Binding repository constraints:**

- SPEC.md's Tensorlake recipe and snapshot-build paragraphs, and its Security and billing wording ("a build bills").
- SPEC_impl.md's Tensorlake sections.
- The build engine's recorded-before-acting rule for builders and snapshots
  (`crates/farhelm-helm/src/tensorlake/build.rs` module docs).
- Root `AGENTS.md`: "The live install is off-limits", "Sharing the machine with other agents", "Tests and validation",
  "Changelog fragments" and "TODO.md and FLAKES.md".
- `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`.

**Planner proposals** (change them if the code says otherwise, and log a DECISION):

- the exact UI wording above;
- the shape of the stored base: one enum, migrated in place in the recipes table's JSON;
- the requirements-check commands.

## Outline

The code today:

- Recipe types are in `crates/farhelm-proto/src/tensorlake/recipes.rs`: `RecipeSource` with `Packages` and the reuse
  variants, plus `BuildStep` and its 9-step numbering.
- The helm's recipe store and migration live in `crates/farhelm-helm/src/tensorlake/` (`recipes`, `build.rs`,
  `build_script.rs`), and the table is in `store.rs`.
- The add-host path is `crates/farhelm-helm/src/provisioning/tensorlake_add.rs` (`steps`, `tensorlake_steps`).
- The UI is `crates/farhelm-ui/src/tensorlake/recipe_editor.rs` and `recipes.rs`.
- The browser fake is `crates/farhelm-helm/src/tensorlake/e2e.rs`, and the browser spec is
  `e2e/tests/tensorlake-recipes.spec.ts`.

A likely stack:

1. `docs:` spec changes for bases, if you prefer them separate. Otherwise fold them into the behavior PR.
2. `feat:` recipe bases. This PR carries:
   - the proto type, the store migration and the helm build engine starting builders from the base;
   - the requirements check;
   - removal of the reuse-only setup paths;
   - the UI's Software block and row summary;
   - unit tests on the fake model, and browser spec updates;
   - SPEC.md and SPEC_impl.md, and the changelog fragment.

   Split helm from UI only if review size demands it, and never leave main with a UI that cannot save what the helm
   expects.
3. Real-sandbox verification (see the decision), with the results in the report.

Do not remove the Phase 2 TODO.md entry. The last plan, `sandbox-sessions.md`, owns that.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect these:

- `cargo fmt`, plus clippy on the touched crates;
- the helm crate's Tensorlake and provisioning tests, run through `python3 scripts/record-test-run.py`;
- the Tensorlake browser specs on both engines;
- `dprint check` on changed Markdown;
- `python3 releasing/check-changelog.py format`;
- `python -B scripts/check-test-sleeps.py` when tests change;
- the real-sandbox verification.

## Fallbacks

These departures need the scope reassessment:

- a "use as is" path for any base;
- a switch to skip agent installs;
- any new action button on the recipe row;
- changing how hosts already created from a recipe behave.

Block, rather than work around, if the real gateway or API cannot start a builder from an image name or a user snapshot,
or if verification would need more sandboxes than the decision allows.

Log these as DECISIONs: the base type's shape, the migration, the step names and numbering, the requirement commands,
what switching base does to an existing snapshot, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-recipe-bases-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type,
`$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when a watched filesystem drops
under 10% or under 5 GB free or available memory drops under 10% (whichever comes first), when the number keeps falling,
when the condition recovers, and when the monitor itself fails. In Claude Code a notification is a stdout line of a
monitor started with the Monitor tool; elsewhere, the harness's equivalent. A file update alone is not a notification.

Before relying on the watchdog, verify delivery with a harmless synthetic alert, and verify failure detection by killing
a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a throwaway monitor
without killing it and confirm you are notified within two sample intervals. If any of these notifications is
unavailable or unverified, say so in the log and check the status file at least once a minute. On every path, check
heartbeat freshness before each new build, test run, sub-agent or review launch; a dead monitor, or a heartbeat stale
for two sample intervals, pauses new launches until monitoring is restored.

An alert is an instruction to act: stop launching work, remove jj workspaces, build output and scratch you own that are
no longer needed, wait for or stop the job most likely responsible, and resume only when the watchdog reports headroom.
Record the watchdog's handle, watched paths, status path and delivery mechanism in the log, include its state in every
handoff note, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/recipe-bases/<nn>-<short-name>`.
- Avoid churn; within this run, a PR that needs correcting is restructured rather than corrected on top, including PRs
  an earlier run built.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's
  cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`. PRs stay drafts: never mark one ready and never merge; landing is the monitor's job.

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 at high effort, launched
natively or through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be
reached when a review is due and is still unreachable after a reasonable retry, block the plan per
`plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute another
reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the merits and log the
DECISION.

### Scope reassessment

Before implementing a substantial departure from the outline (examples under Fallbacks; not a blacklist), or when the
same component has needed repeated corrective review rounds, run a fresh-context review at your own model and effort.
Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered, including every review finding you decided not to follow.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Wording in this file that turns
out inaccurate (a line number, a file name, a class name) is corrected to the truth. A material scope expansion, a
weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or
a log entry is not authorization. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you and logged as a DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should
know and possible follow-ups go into the report's "things you should know" and "open questions and possible follow-ups"
sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its code review gate, and the real-sandbox verification is recorded in the report. This plan does not change
TODO.md. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12). Never edit `plans/` yourself.
