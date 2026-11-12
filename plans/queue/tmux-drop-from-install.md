# Stop requiring, installing and shipping tmux

Written against main at 683a9a72 on 2026-11-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and this plan changes them where its behavior changes.

Its TODO.md source is the Near term entry "Finish removing tmux and xterm.js after old terminals end". This is the
second of four chained plans that together remove tmux and xterm.js from Farhelm ("phase 2"). It runs after
`tmux-end-old-terminals.md` has landed, which made a new supervisor turn off the private tmux server it finds without
any tmux binary, removed the tmux driver, byte streams and xterm.js, and left the tmux version-policy module in
`crates/farhelm-supervisor/src/tmux.rs` (the floor, probe and program resolution) for this plan's users. After this plan
come `tmux-drop-from-tooling.md` (test tooling, CI, brick test, agent instructions) and `tmux-drop-from-docs.md` (specs
and docs not already updated, a comment sweep and a final audit). Read the landed first plan's file history and report
(`plans/REPORTS.md`, or `git log` for its PRs) to see what it actually did before starting.

## The goal

### The maintainer's request, in their words

> plan the work to completely nuke tmux/xterm.js support.

> besides certain narrow helpers that are copied from xterm.js (should be with attribution), the intent is to have no
> trace of them. unless you find something that seems like it's worthy of my attention and is somehow needed. we want to
> not find in 2 months there's some crazy complexity somewhere due to tmux/xterm.js still

> it's not JUST about deleting references to those things, also understanding how the code works today and if we have
> complexity that is only there either because (a) of tmux specific concerns or xterm.js specific concerns or (b)
> complexity just because we had two different types for a while.

> this phase should remove all the tmux installation stuff, bundling of tmux in the installation/distribution stuff, all
> the release stuff having to do with it, musl binaries, all the complexity every where except the bare minimum to
> handle safely and cleanly turning off the tmux the supervisor created.

("musl binaries" means the bundled static musl build of tmux. The static musl builds of `farhelm` itself stay: they
exist so one Linux build runs on every distribution and doubles as the payload pushed to hosts, not for tmux.)

### Acceptance criteria

1. **Nothing requires or looks for tmux.** `farhelm helm setup` no longer chooses, probes, refuses over or records a
   tmux (`--tmux` removed; the `TmuxDecision` and `choose_tmux` block, the dry-run's `<tmux>` placeholder and the tests
   built on a fake `tmux -V` go). The supervisor unit template has no `FARHELM_TMUX` and no tmux directory on `PATH`
   (`crates/farhelm-helm/units/farhelm-supervisor.service.in`, `units.rs`). The desktop app has no tmux preflight, probe
   or `FARHELM_TMUX` handoff (`crates/farhelm-ui/src/desktop/tmux_preflight.rs`, `crates/farhelm-ui/src/tmux_probe.rs`,
   their uses in `desktop.rs`), and `scripts/desktop-smoke.sh` loses its preflight legs and sentinel tmux with them,
   together with the release workflow's collection of those legs' `tmux-preflight-*.stderr` evidence
   (`.github/dist-build-setup.yml`). Units already installed keep their `FARHELM_TMUX` line until their next setup or
   Update; the first plan already scrubs it from what sessions inherit.
2. **Provisioning carries one payload.** Linux provisioning has no tmux payload, reach probe fields, version floor or
   "Upload tmux"/"Install tmux" steps (`crates/farhelm-helm/src/provisioning/`: `PayloadKind`, the plan's tmux branch,
   `Reach.needs_tmux`/`host_tmux`, `tmux_meets_floor`, `StopSupervisor`'s tmux fields, the unarchived-payload path in
   `payloads.rs` and `release_payloads.rs`, the per-asset locks if only two payloads needed them). Update removes the
   bundled tmux at `~/.local/lib/farhelm/tmux` as a step named in the plan the user authorizes, and so does adding a
   host again over an existing install. The changelog fragment says that on hosts where Farhelm had installed its own
   tmux, `tmux` typed inside a session was that copy (its directory comes first on sessions' `PATH`), and after the
   update it is the system's tmux or none. The browser e2e provisioning fixture field `needs_tmux` and its users go
   (`e2e/tests/provisioning.spec.ts`, `e2e/readme-hero/scenario.ts`, `docs/docs-shots/scenario.json5`).
3. **Uninstall has no tmux step.** The remote stop drops `tmux -S <socket> kill-server` and its tmux fields entirely.
   Stopping the unit already ends a server in its control group; any private server that outlived an already-stopped
   unit survives uninstall (a host never updated to this release, or one whose startup shutdown failed and whose
   supervisor was later stopped), which the maintainer accepted. SPEC.md's uninstall promise ("the private tmux server
   is ended too, and any private tmux binary goes with the lib directory") is rewritten to match.
4. **The installer gives no tmux advice.** `scripts/install.sh` loses its tmux probe, floor and Homebrew advice, the
   `brew.sh` OSC 8 link and colour used only by it, and the launch-advice ordering that existed for it;
   `scripts/test-install-sh.sh` loses the matching cases. `scripts/fixtures/install-v0.27.0.sh` is a frozen historical
   installer and stays.
5. **Releases publish no tmux.** The dist extra-artifacts in `crates/farhelm/Cargo.toml`,
   `scripts/build-tmux-assets.sh`, the global job's tmux work and its comments (`.github/dist-build-setup.yml`,
   `dist-workspace.toml`; regenerate `release.yml` per `releasing/AGENTS.md`, never by hand),
   `.github/workflows/sign-sums.yml`'s two tmux members, header and static-check copy, and
   `crates/farhelm-helm/src/provisioning/assets.rs`'s tmux name and its `sums_members` parity, all go together. The
   signed release fixtures (`crates/farhelm-helm/tests/fixtures/release/`, signed by a throwaway key whose secret is
   gone) are regenerated without tmux per their README, with the `without-tmux` variant replaced by another missing
   asset. Text promising "six payloads" (`releasing/AGENTS.md`, SPEC_impl.md "Provisioning") is corrected.
   `scripts/build-private-tmux.sh`, the zig pin and `.github/release/source-pins.env` stay: the pinned test tmux build
   still uses them, and `tmux-drop-from-tooling.md` removes them.
6. **Developer and test scripts that staged the tmux payload** stop: `scripts/devbuild-mac.sh`'s tmux section and its
   Homebrew tmux prerequisite (and `farhelm internal verify-release-sums` if that leaves it with no production use; keep
   it if a fixture recipe or test still needs it, and say which), `scripts/build-podman-test-payloads.sh`'s tmux,
   `scripts/test-provision-centos.sh`'s tmux payload and `FARHELM_TEST_TMUX` with its consumer in
   `crates/farhelm-helm/src/provisioning.rs`.
7. **The version-policy module goes** with its last users: `TMUX_FLOOR`, `TmuxVersion`, `probe_tmux`, the program
   resolution and `TMUX_PROGRAM_ENV` (the first plan's scrub of `FARHELM_TMUX` from session environments keeps its own
   literal), and the `floor_and_release_pin_cannot_drift` test. If the first plan left anything else in
   `crates/farhelm-supervisor/src/tmux.rs` that is not part of the startup shutdown, it goes too.
8. **Docs state the new requirement.** SPEC.md (the Mac's Homebrew tmux requirement, supported tmux programs and
   `--tmux`/`FARHELM_TMUX`, the uninstall promise), SPEC_impl.md (setup, the desktop preflight, provisioning payloads,
   uninstall), the docs website (`get-started/install.md` "If the installer asks for tmux", `add-a-remote-host.mdx`'s
   tmux lines, `using/manage-hosts.mdx`, `using/update-and-uninstall.md`, and the "tmux is still needed" section of
   `using/new-terminals.md` with the anchors pointing at it), and `docs/install_uninstall.md`.
9. **Bookkeeping**, each removal in the PR that deletes its code: review-queue items
   `concurrent-devbuild-copy-outlives-hash-check`, `devbuild-run-only-skips-old-payload-authentication`,
   `no-tmux-lifecycle-witness` and `offline-uninstall-live-tmux-history-unresolved` (the accepted residual in criterion
   3 settles it), re-checked against your diff first, per `review_feedback_queue/AGENTS.md`.
10. Each PR leaves main coherent: nothing references what it deleted (workflow steps, scripts, fixture fields, CLI flags
    in harnesses and docs), and nothing is added that a later PR of this plan deletes.

## Decisions already made

Settled by the maintainer while planning on 2026-11-11:

- **Uninstall:** drop tmux from it entirely and accept the edge case of a never-updated host whose supervisor was
  already stopped.
- **Update removes the bundled tmux** as a named plan step. The maintainer confirmed this knowing that on most remote
  Linux hosts (those whose own tmux is older than 3.7c, Ubuntu 24.04 and Debian 13 included) `tmux` run inside a session
  has been Farhelm's bundled copy, because its directory comes first on sessions' `PATH` (`LaunchSpec.farhelm_bin_dir`
  in `crates/farhelm-supervisor/src/launch.rs`, and the unit's `PATH`): after the update, `tmux` there is the system's
  or missing, and a personal tmux server started from the bundled copy keeps running from the deleted file. That is
  Farhelm's private tmux leaking into users' sessions, which is what this work removes; say it in the changelog fragment
  and in the report.
- **`--tmux` on `helm setup` is removed** (breaking for scripts that pass it; a `feat!` entry). The supervisor's own
  `--tmux` went in the first plan.
- **Media:** the maintainer approved moving the media harnesses to the post-tmux product; removing `needs_tmux` from the
  readme-hero and docs-shots scenarios is part of that. No re-capture or publish.
- **Reviewer:** a single fresh-context agent on Opus 5.5 at high effort (see Review gate). If it cannot be reached when
  a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default.
- **No line-count limit** (the maintainer, 2026-11-12, overriding the size gate this plan was written with): do not
  block or bring the plan back because of how many lines it adds. Lines the work legitimately needs are fine; scope
  still follows the outline, the Fallbacks and the scope reassessment.
- **The maintainer's off-repo signing and publishing tooling** may expect six payloads per release; this plan makes a
  release publish four archives and `install.sh`. Agents never author that tooling. The maintainer was told while
  planning. Also state it prominently in the payload PR's description and in the report's "things you should know", with
  the exact new checksum-list membership, and make the release procedure's own statement of the signed membership
  (`releasing/AGENTS.md` and the release flow files it names) list it exactly, so whoever signs the next release reads
  it there. It is not a block.

## Requirement sources

**The user's request:** the quotes and decisions above.

**Binding repository constraints:** SPEC.md and SPEC_impl.md; root `AGENTS.md` ("Frozen material", "Changelog
fragments", "Conventional Commits", "Tests and validation", the Claude Code shell rule, and "Instructions by path" for
every path you touch: `releasing/AGENTS.md` for `releasing/`, `dist-workspace.toml`, `.github/dist-build-setup.yml` and
the generated `release.yml`; `website/AGENTS.md` for the docs website; `.agents/docs-media.md` for `docs/docs-shots/`
and `e2e/readme-hero/`; `.agents/validation.md` for tests, scripts and workflows); `.agents/test-authoring.md` for every
test change; `review_feedback_queue/AGENTS.md` for queue removals. When you change a workflow or a check or test script,
update `.agents/validation.md`'s inventory and `.agents/run-all-tests.md` in the same change; when you change an
instruction file or rename a heading, run `python3 scripts/check-agents-md.py`. The upgrade floor (0.23.0) still obliges
Update to bring hosts as old as that up to this release; the first plan's startup shutdown handles their old terminals.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- The unit's `PATH` becomes the farhelm directory plus the base `PATH`. On a host whose accepted tmux lived in, say,
  Linuxbrew's `bin`, sessions' inherited `PATH` loses that directory; agents get their `PATH` from the login shell as
  over SSH, so this should not matter, but say it in the report.
- The helm's cached tmux payloads (`<state>/payloads/v…/tmux-*.bin`) are pruned by the existing housekeeping once the
  version changes; nothing to add.
- Removing the reach probe's tmux fields changes the reach record the helm parses from its own script; both sides change
  together, so no compatibility code.
- The first plan already corrected the install script's "Your sessions keep running." line on update for old-terminal
  sessions; leave it as that plan left it unless this plan's change makes it wrong again.

## Outline

Bookmarks are `plan/tmux-drop-from-install/<nn>-<name>`. Split further if a step is too large to review.

1. `01-desktop-and-setup`: criteria 1 and 7's local half: setup, the unit template, the desktop preflight and handoff,
   desktop-smoke's legs, `no-tmux-lifecycle-witness`, SPEC text for them. `feat!:` with a fragment (tmux no longer
   needed on the Mac or a Linux helm machine; `--tmux` removed).
2. `02-provisioning`: criteria 2, 3 and 6's provisioning half, the e2e and scenario fixture field, the docs pages.
   `feat:` or `feat!:` with a fragment (remote Linux hosts no longer get or need tmux; Update removes Farhelm's copy).
3. `03-installer`: criterion 4. `feat:` with a fragment, or fold into 01's.
4. `04-release-payloads`: criteria 5 and 6's devbuild half, the signed fixtures, release docs. `chore:` or `ci:` as fits
   (no user-visible effect beyond fewer release files; say so in a `kind: none` fragment only if the type requires one).
5. Whichever PR deletes the last user of the version-policy module deletes it (criterion 7).

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression. Expect: `cargo fmt` check, clippy (`--all-targets`, `-p farhelm --bins`, and
`cargo check -p farhelm-ui --features desktop` with `cargo check -p farhelm-desktop`), the setup and provisioning unit
tests through `python3 scripts/record-test-run.py`, `scripts/desktop-smoke.sh` for the desktop change, the browser
provisioning spec on Chromium and WebKit, `sh -n` and `shellcheck` on `install.sh` and
`bash scripts/test-install-sh.sh`, `scripts/test-provision-centos.sh` if docker is available (otherwise say so),
`dist generate --check` after the workflow change, the website build, `dprint check`,
`python3 scripts/check-agents-md.py` when instruction files change, and `python3 releasing/check-changelog.py format`.
Report checks run, reused and skipped.

## Fallbacks

Examples of departures that need the scope reassessment: keeping any tmux detection "for diagnostics", a compatibility
layer for old reach records or old units, an uninstall step for leftover servers, or keeping the tmux payload in
releases for older helms (older helms download only their own version's payloads, so none is needed).

If something tmux-shaped turns out to be genuinely needed, keep it, state why in its docs, and list it in the report's
"things you should know". Do not block for it unless keeping it contradicts a decision above.

Decisions to log: what each removed check or step was protecting and why its absence is fine, the `PATH` change, the
fixture regeneration, what happened to `verify-release-sums`, each review-queue item kept rather than removed and why,
and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tmux-drop-from-install-log.md` in the parent directory of the checkout you run in, derived as that section
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
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type, `$scode-harness-shellout` supplies the launch mechanics.

Slow tests on Tensorlake, and parallel work (the maintainer, 2026-11-12, for every tmux-* plan): when you run slow
end-to-end tests, especially the browser (Playwright) specs, use the `scode-ssh-delegate` skill and run them on
Tensorlake sandboxes in parallel, up to 10 sandboxes at once, per `.agents/tensorlake.md`. This is the maintainer's
request for Tensorlake that file asks for, and 10 is the budget. Also run independent work in parallel wherever that
causes no conflicts and loses no quality.

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
  `plan/tmux-drop-from-install/<nn>-<short-name>`, as the outline names them.
- Keep each PR reviewable. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too. Splitting a large outline step into several PRs is fine.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high effort,
launched natively when your harness serves that model, otherwise through `$scode-harness-shellout`. If it cannot be
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires. For a mostly-deleting PR, also ask the
reviewer to look for what the deletion left behind: dangling references, comments and docs describing removed behavior,
and code whose only reason was the removed path.

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
passed its review gate, and the PRs make the TODO.md and review-queue changes the decisions name. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
