# Upgrade epochs, as an upgrade floor

Written against main at e7b47815 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's goal is to change them, as stated below.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Upgrade epochs.** Give every release an upgrade epoch, and only promise clean upgrades across one epoch: an
> installation on epoch N updates normally while the latest release is on N+1, and one whose latest release is N+2 is
> told to uninstall and reinstall. A change that needs new logic in the upgrade path itself ships that logic in a
> release that starts a new epoch; once that release has been out long enough (a week, a month), the next epoch may rely
> on it. Covers the desktop app, the helm, and supervisors. Until this lands, SPEC.md's "Upgrade compatibility and
> client scale" forbids upgrade-breaking changes outright; replace that rule with the epoch rule here. The overall plan
> is `lore/2026-10-05-release-brick-protection-plan.md`.

The maintainer chose, while planning, to build epochs as an **upgrade floor**: the oldest version a release agrees to
update from. Starting a new epoch means raising the floor to the first release of the previous epoch; the period between
two floor raises is an epoch. The maintainer asked for this to stay small ("how much complexity is required to gate
epoch 0 apps, i'm worried this spirals out of control"), and accepted this design over a first proposal with epoch
numbers in signed release metadata, an app-side pre-download check and a new host state.

The floor governs the **update mechanics** (the installer, the app's updater, the helm's Update of a host), not the
format of stored state. Uninstalling keeps the user's data, so after "uninstall and reinstall" the new release opens the
old state as usual; whether a feature's data survives stays decided per feature, as SPEC.md already says.

What is true today, from a read of main at e7b47815 (line numbers approximate; correct them to the truth as you go):

- **The rule to replace:** SPEC.md, "Upgrade compatibility and client scale" (around 4759), the paragraph around
  4817-4822: "Until upgrade epochs exist (TODO.md), no change may break the upgrade path … An installation of any stable
  release from v0.23.0 on … must update to the new release through its normal path and come up on its existing state,
  and the new helm must be able to update the supervisors that release installed." Echoes: SPEC.md around 2988-2993
  (Durability) and 3779-3787, `releasing/mac-vm-test/RECIPE.md` around 33, `releasing/.agents/stable.md` around 207-212
  (the brick-test step). Data compatibility is decided per feature (around 4772-4777).
- **The Mac app's update:** `crates/farhelm-ui/src/desktop/updater.rs` fetches the target release's `install.sh`, checks
  it against that release's signed `SHA256SUMS`, and runs it with `FARHELM_VERSION`, `FARHELM_INSTALL_SUMS_FILE` (the
  permanent two-variable contract, SPEC_impl.md around 7277-7282) and optionally `FARHELM_INSTALL_EXPECT`. It treats
  statuses 74 (busy) and 75 (installation changed) as "try again later" (around 1572-1575), judges success by the
  installed record, and otherwise shows "installing Farhelm vX failed: … (the app's log has the details)"
  (`install_failure_message`, around 1270). A lasting reinstall warning exists (`set_needs_reinstall`; its text points
  at `curl -fsSL https://get.farhelm.io/install.sh | sh`, SPEC.md around 602-605).
- **The install itself:** `scripts/install.sh` hands the work to the new release's own `farhelm internal install`
  (`crates/farhelm-supervisor/src/installation/install.rs`; `crates/farhelm/src/main.rs` around 1603-1611 writes the
  `install-failure` message and exits with the refusal's status, which `install.sh` around 1207-1216 passes through).
  `Context::check_update_only` (around 526-546) already reads `Contents/Versions/installed` and refuses a newer
  installation; `STATE_CHANGED = 75` and `BUSY = 74` are defined around 111-117. The `Versions/` layout arrived in
  v0.22.0, so every installation from v0.23.0 on has the record; older layouts (the unversioned layout with an ownership
  record, and bundles from before records, `--replace-legacy-bundle`) are rebuilt whole today.
- **Remote supervisors:** the helm updates a host by installing its own version over SSH
  (`crates/farhelm-helm/src/provisioning.rs`, `provisioning/service.rs`). Update's probe returns the host's version for
  connected and protocol-skewed hosts alike (`provisioning/backend.rs` around 66-104).
  `downgrade_refusal(..) .or_else(schema_refusal)` runs at four places in `service.rs` (planning for connected and
  skewed hosts around 1235 and 1280, confirmation for both around 2144 and 2187); tests
  `update_refuses_a_host_running_a_newer_farhelm` and `update_refuses_a_host_whose_database_schema_is_newer` in
  `provisioning.rs` around 2945-3041. Automatic updates reuse the plan and record a failure once per host incarnation
  (`automatic.rs` around 99-130). The panel's Uninstall needs a connected host and keeps the data directory; Add finds a
  live old supervisor and registers it as-is (`service.rs` around 1467-1530 and 2091-2117; SPEC.md around 179-195).
- **Development builds:** main's version is `0.0.0-unreleased` (root `Cargo.toml`), reported by test-harness supervisors
  and by hosts a devbuild updated (`.agents/devbuild.md` around 31-32: the installed app offers to update such a host
  back). `farhelm_proto::is_development_build` identifies them; `build_is_newer` and the installer's `newer_than`
  already treat unknown order specially. The `build_is_older` doc comment in `crates/farhelm-helm/src/hosts.rs` around
  426-429 records why tests must not compare against the real build stamp.
- **Release procedure:** `releasing/AGENTS.md` (including "PR merges these procedures authorize" and "Mac VM test
  edits"), `releasing/.agents/stable.md`, `releasing/mac-vm-test/BRICK-TEST.md`, `RECIPE.md`, and
  `releasing/mac-vm-test/brick_test.py`, whose baseline v0.23.0 is hardcoded around 184-188 (with `test_brick_test.py`).
  `CHANGELOG.md`'s dated stable headings record promoted releases (a tag without a section was never promoted).

### Acceptance criteria

- **One floor constant** in `farhelm-proto` (which both the installer and the helm depend on), initially **v0.23.0**,
  the version SPEC.md already promises updates from. Checks take the floor as a parameter so tests use fixed literals,
  not the real constant.
- **Development builds and unparseable versions are never below the floor**, on both the installer and the helm side.
- **Installer gate**, in `farhelm internal install` (`Context::run`, after the expectation check and before update or
  rebuild, for update-only and Terminal runs alike), so `install.sh` needs no change and holds no copy of the floor:
  updating an existing installation below the floor stops before anything changes, with a message that the installation
  is too old to update, that its data stays on disk, and that the user should run `farhelm uninstall` and then the
  installer. It exits with a new dedicated status distinct from 0, 1, 2, 74 and 75 and below 126. A fresh install is
  never refused. Installations from before v0.22.0's records are known to be below the floor and are refused; an
  installation with a `Versions/` folder but a missing or unreadable record is decided conservatively (log the
  DECISION). Leave the legacy-bundle and rebuild code in place even where it becomes unreachable for updates; removing
  it is a follow-up for the report.
- **The app:** the updater recognizes the dedicated status and shows the lasting reinstall warning (the existing
  `set_needs_reinstall` mechanism) with text that says to run `farhelm uninstall` first, then the installer. Apps
  released before this change keep showing the generic failure with details in their log; that is accepted.
- **Remote hosts:** one helper, used at all four Update entry points beside the downgrade and schema refusals, refuses a
  host whose supervisor version is below the helm's floor. Its message gives a remedy that works by hand: stop the
  host's Farhelm supervisor service, delete Farhelm's directory on the host (the helm knows its path from the probe),
  and the data directory too if the user accepts losing it, then remove the host from Farhelm and set it up again. No
  protocol change, no new host state, no change to automatic updates.
- **SPEC.md** replaces the freeze with the floor rule: updating from any release at or above the floor must work through
  the normal path and come up on its existing state; installations and hosts below it are refused with the remedies
  above. It defines epochs as the periods between floor raises, says the floor is raised only when a release needs
  upgrade logic an earlier release shipped, states that the floor covers update mechanics while data compatibility stays
  per feature, and states what each component shows. The echoes listed above are brought in line. SPEC_impl.md says
  where the floor lives and how each gate reads it.
- **How a raise happens:** the agent whose change needs a floor raise raises the constant in its own PR, and says so in
  the PR and its changelog fragment. The release procedure, when the floor differs between the previous stable and the
  proposed base, shows the maintainer the raise before the release goes ahead: the new floor release, when it was
  promoted (from `CHANGELOG.md`'s dated stable headings), how long ago, and how many stable releases have shipped since.
  A manual step, no script, no fixed threshold. The maintainer accepts it or picks an earlier base.
- **Brick test:** the baseline range starts at the floor. Keep the literal in `brick_test.py` and add a unit test that
  parses the Rust constant and checks they match (read `releasing/mac-vm-test/BRICK-TEST.md` first, per
  `releasing/AGENTS.md`).
- **Tests:** the installer gate in `crates/farhelm-supervisor/src/installation/install/tests.rs` beside the existing
  refusal tests; the updater's mapping of the status in the desktop updater's tests; the helm refusal (including a
  development build never refused) beside the existing refusal tests; the brick-test agreement test.
- Typed `feat` for the gates, with a changelog fragment that says installations older than v0.23.0 can no longer update
  in place and must uninstall and reinstall. Spec and procedure PRs are `docs`; the brick-test tooling part is `chore`
  or `test` as fits.
- The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08:

- **The upgrade-floor design.** The alternatives were the first proposal (epoch numbers published in signed metadata, an
  app-side check of the target's epoch before downloading, a new host state) and the floor design without the app
  change.
- **No waiting threshold.** In the maintainer's words: "there is no need to have special thresholds. epoch bumps are not
  common. just make it very clearly explicit and obvious to the maintainer during the release process." The release flow
  shows the history; the maintainer makes the call.
- **The maintainer decides a floor raise** when a release is requested, alongside its base and version; agents flag
  changes that need one. The planner turned "flag" into "raise the constant in the PR that needs it, and the release
  procedure surfaces the raise"; the maintainer sees this in the planning PR.
- **No state promise after a reinstall**; the message says the data stays and may not carry over cleanly.
- **Dev releases and RCs get no special handling.** (A floor release's own RC sorts below it under SemVer and is
  refused; log it as a DECISION.)
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Planned on hold**, at the maintainer's request, so it runs only once they unhold it.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's "Upgrade compatibility and client scale" (changed by this plan as stated);
the permanent contracts in SPEC_impl.md (the updater's installer interface, the site layout), which nothing here
changes; `releasing/AGENTS.md` (read it before touching anything under `releasing/`; no release tags, no publication);
root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md",
"Instructions by path".

**Planner proposals, several from the planning review:** the initial floor of v0.23.0; the gate in
`farhelm internal install`; the dedicated status; development builds exempt; the manual remote remedy; raising the floor
in the PR that needs it; the brick-test agreement test.

## Outline

An estimate; merge or split PRs where that makes review easier, and log the DECISION.

1. `plan/upgrade-epochs/01-floor-spec`, typed `docs`: SPEC.md and SPEC_impl.md, the floor rule replacing the freeze.
2. `plan/upgrade-epochs/02-installer-gate`, typed `feat`: the floor constant, the gate in `farhelm internal install`
   with its dedicated status, the updater's reinstall warning for it, tests, changelog fragment.
3. `plan/upgrade-epochs/03-helm-refusal`, typed `feat`: Update refuses hosts below the floor, with tests.
4. `plan/upgrade-epochs/04-release-procedure`: the release flow's floor-raise step, the brick-test baseline and its
   agreement test, and the TODO.md removal.

### Validation

The installation tests in `farhelm-supervisor`, the desktop updater's Rust tests (`--features desktop`), the helm
provisioning tests, and the brick-test harness unit tests (they run on Linux), through `scripts/record-test-run.py`.
`test-install-sh.sh` only if you add an end-to-end status case. `cargo fmt --check`, clippy, `dprint check` on changed
Markdown, and `python3 scripts/check-agents-md.py` if instruction files change. Do not cut, tag or publish any release
to test this.

## Fallbacks

Examples of departures that need the scope reassessment: publishing epoch metadata on get.farhelm.io, adding an epoch
field to the handshake (a protocol bump), a new host state, making the panel's Uninstall work on a skewed host, an
automatic uninstall, a startup check of how old the stored state is, enforcing waiting time in code, or a shell copy of
the floor in `install.sh`.

Decisions to log: the dedicated status value, how an unreadable record is treated, the exact user-facing messages, how a
floor release's RC is handled, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-upgrade-epochs-log.md` in the parent directory of the checkout you run in, derived as that section says.
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/upgrade-epochs/<nn>-<short-name>`, as the
  outline names them.
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
