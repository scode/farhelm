# Tensorlake recipes: prepared snapshots with Farhelm already installed

Written against main at c277fa78 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

This is the second of three chained plans toward "sandbox sessions": a session that runs in a Tensorlake sandbox of its
own, created from a recipe when the session launches. The TODO.md entry they share is "Phase 2: a sandbox per session"
(bucket "Near term", subsection "Tensorlake"). The design records are
`lore/2026-11-03-tensorlake-sandbox-hosts-design.md` and
`lore/2026-09-29-per-session-sandboxes-as-the-remote-host-goal.md`. `recipe-bases.md` runs before this plan and makes
every recipe yield a snapshot Farhelm built (base, packages, Claude Code, Codex). `sandbox-sessions.md` runs after and
uses what this plan builds.

The maintainer's words while planning (2026-11-08):

> regarding session start - we can't be installing farhelm every time it'll be too slow. we need some concept of a
> "prepared snapshot" that has already gone through those steps and can be re-used (and perhaps for now we have soethign
> simple like "expires after 24 hours").
>
> (regarding your "version mismatch after farhelm updates" that would just become another reason to just expire the
> prepared snapshot)

On how it is shown while preparing: "the messaging the user sees while it's preparing needs to make it clear this only
happens the first time and then only once per 24 hours".

### What a prepared snapshot is

It is a second, Farhelm-managed snapshot per recipe, taken on top of the recipe's built snapshot after Farhelm's own
per-host preparation:

- the logind and linger preparation that setup runs today;
- Farhelm's supervisor installed with its user unit enabled, exactly as setup installs it today (payload upload,
  install, unit, enable, linger).

What it must not contain:

- **No supervisor identity or state.** The supervisor's identity lives only in `<state>/supervisor.db`
  (`crates/farhelm-supervisor/src/store.rs`, `ensure_host_identity`). The preparer stops the supervisor after the
  install and deletes its whole state directory. It then verifies that no `supervisor.db` (or any other per-install
  state the backend map below lists) remains before the checkpoint, so each sandbox started from the snapshot mints its
  own identity on first start. SPEC.md's rule that copied supervisor state is refused stays as it is. Prepared snapshots
  satisfy it by never carrying state.
- **No credentials.** The preparer delivers no endpoint tokens, logins or GitHub token and starts no tunnels. Those are
  delivered per host on connect, as today.
- **A non-empty `/etc/machine-id`.** It is emptied before the checkpoint, as the recipe build already does.

A prepared snapshot is **stale** once any of these is true:

- it is more than 24 hours old;
- the helm's Farhelm version differs from the one that prepared it;
- the recipe's built snapshot has changed since it was prepared.

It is rebuilt **lazily**: the first use that needs a fresh one prepares it, and concurrent uses join that preparation,
the way `RecipeBuilds::start_or_join` / `ensure_snapshot` work for recipe builds. Preparing bills a short-lived preparer
sandbox. Create it with the recipe's CPUs and memory and the recipe snapshot's own disk size (a clone cannot be smaller
than its snapshot's disk, and using the snapshot's size keeps a later recipe disk change from making the prepared
snapshot unusable), and the builder's short idle timeout. The preparer's name carries a reserved prefix that "Sandboxes
not on this helm" already recognizes, or that list learns the new one. It is recorded before it is created, like a
builder. When preparing finishes, the preparer is terminated and its other snapshots deleted, and the previous prepared
snapshot is deleted right after the new one is recorded, the way Rebuild snapshot already retires the old recipe
snapshot (`build.rs`). Deleting a recipe deletes its prepared snapshot too.

**A naming collision to resolve first.** The code and TODO.md already call the recipe's built snapshot "prepared" in
places (for example around `RecipeBuilds::ensure_snapshot` in `crates/farhelm-helm/src/tensorlake/build.rs`). Rename
those uses so "prepared snapshot" means only the new Farhelm-managed one, in code, docs and UI. The UI never says
"template"; it says "snapshot".

### Who uses it in this plan

**Add host → Tensorlake sandbox** creates the sandbox from the prepared snapshot instead of running the supervisor
install. Setup then:

1. ensures a fresh prepared snapshot;
2. creates the sandbox from it;
3. waits for the supervisor to answer (a lingering user unit starts on the clone's boot);
4. connects, letting the helm record the new identity at first contact as usual;
5. delivers credentials.

The host row records the install facts setup would otherwise learn by inspecting: where the supervisor lives, and its
version. Take them from the prepared snapshot's record. **Rerun** and **Update** on an existing Tensorlake host are
unchanged and still run the ordinary install path.

`sandbox-sessions.md` adds session launch as the second user. Expose the "ensure prepared" step so that plan can call it
and report its progress.

### The UI that was agreed (mocked up on the planning page)

The recipe list row gets one more status line about the prepared snapshot, under the snapshot line. Example wording:

- "Ready for new hosts · prepared 2 h ago for Farhelm 0.38.0 · renews after 24 h" (in the `--ok` tone). The planning
  mockup said "sessions"; sessions do not exist until `sandbox-sessions.md`, which changes it to "new hosts and
  sessions".
- While preparing: "Preparing for sessions: installing Farhelm (step 3 of 5) · 41 s", with the existing pulse dot.
- When stale: "Not prepared since Farhelm updated; the next new host prepares it (about a minute)". The other reasons
  read "…since it expired" and "…since the snapshot was rebuilt". Use "new host or session" once `sandbox-sessions.md`
  lands. Here, say "new host".
- On failure: "Preparing failed while installing Farhelm (step 3 of 5): <reason>. The next new host tries again."

There is no "Prepare now" button. The maintainer chose lazy-only.

Wherever the user waits on preparation, the wording must say this happens only the first time, and then at most once
every 24 hours or after Farhelm updates. Today that is the add-host setup view's step list; `sandbox-sessions.md` adds
the launch-progress pill. For example, a setup step titled "Prepare snapshot" with the explanation "Only the first time,
and then at most once every 24 hours or after Farhelm updates." Omit the step when the prepared snapshot is fresh.

### Acceptance criteria

- Each recipe with a built snapshot has at most one current prepared snapshot.
  - It is recorded with: its snapshot ID, when it was prepared, the Farhelm version, the recipe snapshot it was built
    from, and the install facts hosts need.
  - It is stale under exactly the three rules above.
  - It is prepared lazily, one preparation per recipe at a time; concurrent users join.
- A sandbox created from a prepared snapshot gets a supervisor with a fresh identity, distinct across two sandboxes from
  the same snapshot. The helm shows both as ordinary connected hosts, never Duplicate.
  - Unit-level: a check that the preparer removes the state directory and refuses to checkpoint if `supervisor.db` (or
    any listed state) exists.
  - Real-sandbox: two hosts from one prepared snapshot (see the decision).
- No credential, GitHub token, gh git configuration, agent login or tunnel exists in a prepared snapshot. Unit-level:
  the preparer never calls credential delivery.
- Add host → Tensorlake sandbox uses the prepared snapshot. Its setup view shows "Prepare snapshot" only when preparing
  is needed, with the first-time / once-per-24-hours wording. A host so created connects, receives its credentials and
  tunnels, and works as before.
- Billed resources are recorded before they are created and cleaned up the way builds are, including after a helm stop
  mid-preparation. "Sandboxes not on this helm" sees a leftover preparer. Deleting a recipe deletes its prepared
  snapshot and any preparer.
- The recipe row's prepared line covers every state above.
- The report gives the measured breakdown of creating a sandbox from a prepared snapshot until its supervisor answers,
  on real sandboxes. There is no hard target; the maintainer asked for measure and report.
- SPEC.md and SPEC_impl.md describe prepared snapshots:
  - staleness, lazy preparation and billing;
  - identity, made fresh because state is never captured (amend the "no identity reset" text accordingly);
  - the add-host path and the recipe row;
  - SPEC.md's paragraph on preparing a Tensorlake sandbox before inspection (around "Tensorlake sandboxes get a
    preparation step"), which moves into the prepared snapshot.
- A changelog fragment (`kind: changed`) tells users that new Tensorlake hosts start from a prepared snapshot, much
  faster after the first one, which is prepared at most once a day or after an update.

## Decisions already made

Settled with the maintainer on 2026-11-08, in conversation and on a planning page with mockups:

- **Prepared snapshots expire after 24 hours, or when Farhelm's version changes**, as quoted. The recipe snapshot
  changing also makes one stale: the planner added this, since the old one no longer matches the recipe.
- **Lazy**, with no "Prepare now" button and no background renewal.
- **Add host uses it too**: one path for hosts and sessions.
- **Measure, don't target**: report the spawn-time breakdown, with no pass/fail threshold.
- **The preparing wording says it is first-time and at most once per 24 hours**, as quoted.
- **No mise** anywhere.
- **Three chained plans**, this one after `recipe-bases.md`.
- **Code reviewer:** a single fresh-context Opus 5.5 agent at high effort with the general charter below.
- **No-workhorse mode**, the plans default.
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

- SPEC.md's host identity rules ("Host identity", "never connect two entries to one machine", copied supervisor state
  refused) and its Tensorlake host setup paragraphs.
- SPEC_impl.md's Tensorlake add and identity sections.
- The build engine's recorded-before-acting rule (`crates/farhelm-helm/src/tensorlake/build.rs` module docs).
- `crates/terminal-host/SPEC.md`'s change policy, if anything in the supervisor's state handling touches terminal hosts.
- Root `AGENTS.md`: "The live install is off-limits", "Sharing the machine with other agents", "Tests and validation",
  "Changelog fragments".
- `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`.

**Planner proposals** (change them if the code says otherwise, and log a DECISION):

- reusing the build engine's machinery (a "prepare" job beside "build");
- the preparer's name and prefix;
- how the install facts are carried to hosts;
- the step count and names;
- wiping the whole state directory rather than chosen files.

## Outline

Where things are today:

- **Add host:** `crates/farhelm-helm/src/provisioning/tensorlake_add.rs`.
  - `tensorlake_steps` ensures the snapshot, `create_host_sandbox` creates the sandbox, then come
    `prepare_tensorlake_sandbox`, `inspect`, `refuse_copied_supervisor`, `admit_tensorlake_install`, `delivery.setup`,
    and the install actions.
  - The `PREPARE` script is in `provisioning/tensorlake_backend.rs`.
  - Install actions are `ProvisioningAction` in `provisioning/plan.rs`.
- **Recipe builds:** `crates/farhelm-helm/src/tensorlake/build.rs`, with `RecipeState` in
  `tensorlake_recipes.state_json` and the scripts in `build_script.rs`.
- **Supervisor identity and state:** `crates/farhelm-supervisor/src/store.rs` (`ensure_host_identity`). Per-install
  state under the state directory includes:
  - `supervisor.db`, tmux sockets and config;
  - `hosts/`, `launch/`, `checkout-preparation/`, `repo-cache/`, `spare-builds/`, `attachments/`, reports;
  - `github-token` and the managed-supervisor record.

  Verify this list in the code.
- **Orphans:** `crates/farhelm-helm/src/tensorlake/orphans.rs`. The recipe row UI is
  `crates/farhelm-ui/src/tensorlake/recipes.rs`.

A likely stack:

1. `docs:` spec changes, if separate.
2. `feat:` prepared snapshots in the build engine. This PR carries:
   - preparation, staleness, recording and cleanup;
   - the identity and secret guards;
   - the recipe row's prepared line;
   - unit tests on the fake model.
3. `feat:` Tensorlake hosts start from the prepared snapshot. This PR carries:
   - the add-host path;
   - the setup view's step;
   - the browser spec updates using the e2e fake;
   - the changelog fragment.
4. Real-sandbox verification and timing measurement, with the results in the report (and a lore entry with the
   measurements, which are worth keeping).

Do not remove the Phase 2 TODO.md entry; `sandbox-sessions.md` owns that.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect these:

- `cargo fmt`, plus clippy on the touched crates;
- the helm crate's Tensorlake and provisioning tests;
- any supervisor tests you touch, through `python3 scripts/record-test-run.py`;
- the Tensorlake browser specs on both engines;
- `dprint check` on changed Markdown;
- the changelog check;
- `python -B scripts/check-test-sleeps.py` when tests change;
- the real-sandbox verification.

## Fallbacks

These departures need the scope reassessment:

- an identity-reset mechanism in the supervisor, instead of never capturing state;
- background or eager preparation;
- a user-facing button;
- changing Rerun or Update for existing hosts;
- baking credentials or tunnels into the snapshot.

Block, rather than work around, in any of these cases:

- a clone's supervisor does not start on boot;
- two clones end up sharing an identity;
- the gateway or API cannot do what the design needs;
- verification would need more sandboxes than the decision allows.

Log these as DECISIONs:

- the preparer's design and name;
- the state that is wiped and how it is verified;
- the staleness bookkeeping;
- the install facts carried to hosts;
- the step wording;
- the measured timings;
- every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-prepared-snapshots-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/prepared-snapshots/<nn>-<short-name>`.
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
passed its code review gate, and the real-sandbox verification and the spawn-time measurements are recorded in the
report. This plan does not change TODO.md. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12). Never
edit `plans/` yourself.
