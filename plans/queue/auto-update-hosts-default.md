# Update remote hosts automatically by default

Written against main at 2c73a156 on 2026-11-19. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file tells you to change them: the maintainer's request below reverses a rule they state.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> **Auto-update hosts by default.** Existing users who have NOT checked auto-update get enrolled in auto-updates. Only
> people who explicitly turn auto-update off AFTER this change has been made get a persistent preference and are not
> auto-updated. Make sure we get this right in terms of the settings representation. UI wise, it's the same setting as
> today, it just goes to being checked by default on new installations and on upgraded installations.

Today the helm's `preferences.auto_update_remote_hosts` column is nullable, and only a stored `true` lets the helm
update remote hosts by itself; unset and `false` both mean off. After this plan, only a `false` written after the change
means off. Everything else (never chosen, a `false` stored before the change, `true`) means on.

### Acceptance criteria

1. A new helm schema step (90, or the next free number if main has moved; see "Adding a schema version" below) rewrites
   every stored `false` in `preferences.auto_update_remote_hosts` to NULL and leaves `true` and NULL alone. Nothing else
   in the preference row changes.
2. The helm's automatic-update policy reads (`crates/farhelm-helm/src/provisioning/automatic.rs`, `scan`, both checks;
   `crates/farhelm-helm/src/provisioning/service.rs`, the final policy read in `start_automatic_update`) treat anything
   other than `Some(false)` as on. A fresh helm database therefore updates remote hosts automatically, and so does an
   upgraded one whose user never turned the switch off after upgrading.
3. Turning the switch off in Settings → Hosts stores `false`, which stops new automatic starts exactly as it does today
   (planning still in flight included), and survives helm restarts and further migrations.
4. The desktop app shows the switch the way the helm reads it. Settings → Hosts' **Update remote hosts automatically**
   is on unless `false` is stored, including when the helm has pushed no preference row yet, the way **Name sessions on
   quick launch** reads (`crates/gui/farhelm-gui/src/settings/choices.rs`, `Choice::stored`). The sidebar attention
   list's **Always update** button follows the same reading (`crates/gui/farhelm-gui/src/hosts/status.rs`,
   `auto_updates_on`): it still appears only while the setting is off, which is now only after an explicit opt-out.
5. An explicit `PUT /api/preferences` with `"auto_update_remote_hosts": null` clears the stored choice, which now reads
   on. The desktop app only ever sends booleans for this field.
6. SPEC.md, SPEC_impl.md, the code comments that state the old rule, and the user docs pages all describe the new
   default (the full list is under "Planner proposals" below).
7. The release brick test passes for a candidate that carries this change: its remote-update step observes the automatic
   update instead of clicking Update (see Decisions).
8. A devbuild's helm starts with automatic remote-host updates off, or, if no simple way to do that exists, a Near term
   TODO.md entry records the follow-up and the devbuild instructions say that devbuilds update shared hosts
   automatically like production builds (see Decisions).

## Decisions already made

Settled with the maintainer on the 2026-11-19 planning page:

- **Representation:** reuse the existing column and wire field, and clear stored `false` values from before this change
  with a one-time migration (option A). The rejected alternative was a new "explicitly off" column and wire field, with
  the old one dropped. It behaves the same for users but costs more churn across the helm, the wire type, the app and
  the tests.
- **Enrolling earlier opt-outs:** taken from the TODO wording. Anyone who turned the switch off before this release is
  enrolled again; only turning it off after upgrading sticks. The switch, its place in Settings → Hosts and its wording
  stay as they are.
- **Power-loss durability of the opt-out is out of scope.** The untriaged review item
  `review_feedback_queue/automatic-update-opt-out-can-roll-back.md` (saving `false` for this field commits at
  `synchronous=NORMAL`, so a power cut can restore an earlier `true`) stays with triage. Do not change the store's
  FULL-sync condition and do not remove that item. Name it in the report's follow-ups, since an opt-out is now the only
  thing that stops automatic updates.
- **Release brick test:** adapt it in this plan. Step 5 of `releasing/mac-vm-test/RECIPE.md` (the remote supervisor
  upgrade) observes the automatic update the candidate starts on its own, the way step 4 already observes the local
  supervisor upgrade. The "before" continuity sample moves to before the candidate relaunch, and the same checks follow.
  The maintainer accepted that a manual Update click is no longer exercised there; the automatic update runs the same
  authorized Update flow. A pre-change opt-out in the old app cannot keep the host manual, because the migration clears
  it by design. Do not try that route.
- **Devbuilds:** in the maintainer's words, "(b) except if no simple mechanism can be found make it a near term todo
  item to follow-up on it, and proceed with devbuilds auto-updating like the production builds". Option (b) was: "make a
  devbuild's helm start with automatic host updates off, by the smallest means the executor finds and with no new
  user-facing setting". The reason is that a devbuild shares remote hosts with the installed app. A development build
  counts as unknown age, so it updates only hosts on a lower protocol. A devbuild of a change that bumps the
  helm–supervisor protocol would put that build on every shared host the moment it connects, and the installed app could
  not update them back. See "Devbuild: what counts as simple" below for the boundary.
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **Release note:** the maintainer's own text, in the `## Release note` section below.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`: "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md",
  "Frozen material" (CHANGELOG.md's published sections and `website/src/release-notes/*.json` mention the old default;
  leave them).
- `.agents/validation.md` and `.agents/test-evidence.md` for builds, tests and goldens; `.agents/test-authoring.md` for
  every test you add or change.
- `releasing/AGENTS.md`, "Mac VM test edits": before editing anything under `releasing/mac-vm-test/`, read
  `releasing/mac-vm-test/BRICK-TEST.md` in full, including "Validate changes to this coordinator" and "Requests to
  change the process". This plan is the maintainer's request to change the process.
- `.agents/devbuild.md` before editing it or `scripts/devbuild-mac.sh` / `scripts/devbuild-github.sh`.
- `website/AGENTS.md` before editing anything under `website/`. This is an unattended run with incidental docs edits:
  start no preview server, and name the changed pages in the report.
- `.agents/careful-rebase.md` for the schema-number collision rule.

**Planner proposals** (verified against the code and checked by an independent planning reviewer):

- **Migration shape.** Follow the 75→76 step in `crates/farhelm-helm/src/store.rs` (a single `UPDATE` plus the version
  stamp in one batch, with a short comment saying why) and the 63→64 step, which rewrites a preference value. The
  fresh-schema table definitions do not change, but the fresh schema's bare version literal must still equal
  `SCHEMA_VERSION`. The migration comment says plainly that the step cannot tell a pre-change `false` from any other
  `false` seen below the new version, and that this is the point. Rewind tests elsewhere plant only `true` for this
  field, so they keep passing.
- **Migration test shape.** Follow `schema_75_drops_embedded_tokens_and_preserves_recipe_and_build_state`
  (`crates/farhelm-helm/src/tensorlake/recipes_tests.rs`) and the existing
  `schema_68_automatic_host_updates_start_off_and_persist` (`store.rs`). Plant a row in a current store, stamp the
  database back to the previous version, reopen it, and check the result. Loop over planted `false`, `true` and NULL the
  way `schema_64_...` does, and assert NULL, `true` and NULL afterwards, with an unrelated preference kept. The repo has
  no per-schema fixture files for the helm store; do not invent one. `schema_68_...`'s name and assertions state the old
  rule, so reword it so it no longer claims the setting starts off.
- **One shared reading or five inline checks.** The helm and the app read the same `Preferences` type. A method on it
  would make helm behavior and the app's display agree by construction. Against that, the type has no methods today, the
  quick-launch naming precedent keeps its check inline, and `Choice::stored` classifies fields in a match. Either is
  defensible. Inline checks matching the naming precedent are the default; log a DECISION whichever you pick.
- **Tests that would now race an automatic update.** Every helm test harness starts the automatic-update listener with
  production provisioning. With default-on, a test whose harness host is one protocol version behind would have its own
  listener start a real update. `updating_a_skewed_host_is_not_stopped_by_its_old_refusal` in
  `crates/farhelm-helm/src/provisioning.rs` is one; search for other harness hosts on `PROTOCOL_VERSION - 1` or with a
  parseable older build. Fix them with the idiom several `provisioning.rs` tests already use,
  `harness.state._automatic_updates.stop()`. Do not add a harness-wide default-off option, and do not turn the tests
  into a stored-`false` setup unless the test is about the preference itself. The e2e harness and the GUI smoke and
  lifecycle scripts run supervisors from the same build, so no host there qualifies.
- **Tests whose premise flips.** Besides `schema_68_...`: the preference tests in
  `crates/farhelm-helm/src/preferences.rs` (`automatic_host_update_preference_is_sparse_and_announced` and others), the
  automatic-update tests in `provisioning.rs`, and in the app `session_list/attention/tests.rs`, `hosts/status/tests.rs`
  and `settings/tests.rs`. Update assertions that encode "unset is off". Add a test that unset reads on and an explicit
  `false` reads off, at the helm's policy and in `Choice::stored`.
- **The sweep of text that states the old rule.**
  - SPEC.md: the provisioning paragraph's "explicit automatic-update opt-in is the authorization" (around line 141); the
    paragraph opening "**Update remote hosts automatically** in Settings is off by default for new and existing
    installations"; Settings → Hosts' "off unless explicitly enabled"; the Security list's "Updates are never forced"
    item ("the separate, off-by-default automatic remote-host update setting"); and any other "off-by-default" mention
    (one near line 824).
  - SPEC_impl.md: the "Helm schema 68" paragraph states the old rule in the present tense. Add a paragraph for the new
    schema step that supersedes it, and amend the schema-68 one so the document does not contradict itself.
  - Code comments: `crates/farhelm-helm/src/preferences.rs` module docs, `crates/farhelm-proto/src/ui_channel.rs` (the
    field's doc), `choices.rs` (the module docs and `Choice::stored`'s doc), and `hosts/status.rs` (`auto_updates_on`'s
    doc). Leave the schema-67 migration comment in `store.rs` alone; it is history.
  - Website: `website/src/content/docs/docs/using/manage-hosts.mdx`, `using/settings.md`,
    `using/update-and-uninstall.md` ("Your remote hosts are not updated automatically unless you turn on ..."), and
    `how-it-works/security-model.md` ("the automatic updates you can turn on").
- **GUI goldens.** Settings Hosts goldens that draw the switch with no stored choice now show it on. Regenerate and
  publish them per `.agents/validation.md`, and look at each changed picture.

### Adding a schema version

If main gained another helm schema step while this plan ran, take the next free number at rebase.
`.agents/careful-rebase.md` covers this collision, and a schema step never counts as a conflict between plans.

### Devbuild: what counts as simple

A simple mechanism is a change confined to the devbuild scripts and instructions, or a product hook of a few lines that
only a devbuild uses, is tested, and needs no new preference, wire field or schema step. Two directions to evaluate
first:

- Can the devbuild script store `false` through the helm's ordinary preference path before the first automatic scan
  runs?
- Does the helm already take a startup flag or configuration that a small, devbuild-only addition could build on?

Whatever you choose must store an explicit off only when no choice is stored. A maintainer who turns the switch on in a
devbuild keeps it on, and a fresh per-target devbuild state starts off too. If the only workable route is a new
mechanism, a race-prone ordering trick, or something the scope reassessment below judges non-trivial, take the fallback.
That means a Near term TODO.md entry describing the risk and the options you found, a paragraph in `.agents/devbuild.md`
saying that devbuilds update shared remote hosts automatically like production builds (and how to turn it off in the
devbuild's Settings), and a logged DECISION. The fallback is not a block.

## Outline

1. `plan/auto-update-hosts-default/01-default-on` (`feat:`): acceptance criteria 1–6. Migration and policy reads, the
   app's reading, the tests above, SPEC.md and SPEC_impl.md, code comments, the four website pages, GUI goldens, and a
   changelog fragment (`kind: changed`). The helm and the app change together: a helm-only PR would leave main showing
   the switch off while hosts update.
2. `plan/auto-update-hosts-default/02-brick-test-automatic-remote-update` (`test:` or `chore:`, whichever fits the
   change): acceptance criterion 7, in `releasing/mac-vm-test/` (RECIPE.md step 5 and whatever the coordinator, guest
   workflow and checkpoint contract need). Keep the old-release side of the fixture unchanged. Step 3's "automatic
   updates disabled" is the desktop app's own updater, not this setting; do not conflate them. Validate per
   `releasing/mac-vm-test/BRICK-TEST.md`, "Validate changes to this coordinator": the unit tests through the test
   recorder. The fixture smoke needs the private Mac machine, so run it only if this machine is that substrate;
   otherwise skip it and say in the report that the next release's brick test is the first end-to-end proof.
3. `plan/auto-update-hosts-default/03-devbuild-starts-off` (`chore:`; rename the bookmark's short name if you take the
   fallback, before its first push): acceptance criterion 8, either way. This PR also removes this plan's TODO.md entry.

## Fallbacks

- Publishing updated GUI goldens can be blocked if this machine lacks push access to the private golden storage
  repository (`scripts/gui-goldens.py` says where). Then keep the updated captures verified locally and block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), rather than landing failing goldens.
- If the brick test cannot be adapted without changing what the old-release side of the fixture does, or without
  dropping a check other than the manual Update click itself, record the tradeoff and block on it after pushing PRs 1
  and 3.
- The devbuild fallback is described under "Devbuild: what counts as simple".

Decisions to log: the schema number used, inline checks or a shared method, every test given the listener stop and why,
every golden changed, how the brick test's continuity sample was moved, the devbuild route taken or rejected, and every
review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-auto-update-hosts-default-log.md` in the parent directory of the checkout you run in, derived as that
section says. The watchdog's status file is `farhelm-plan-auto-update-hosts-default-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`. Report the checks you ran, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and when Rust tests change, run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it.

### Resume protocol

First confirm that the checkout you run in is a checkout of `scode/farhelm`: the nearest Git worktree or jj workspace
containing your working directory, with a remote pointing at that GitHub repository. If it is not, stop, write nothing,
and say so. Then invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read it and
resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open PRs)
rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on, or
that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. `plans/.agents/planning.md` sets no-workhorse for
plans, and the maintainer saw it stated while planning, so it is an explicit choice: if galaxy-brain is already active
when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine. They are worth considering when they are likely to finish the work
sooner without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows. When the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
`plans/AGENTS.md`, "Sub-agents this file requires", names three sub-agents every plan needs: the resume check, and the
cold reads of a blocked question and of the report. They are exempt from the rules above and run as that file says:
native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory.
Resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Pass each of these as a `--path` of `sample`:

- the root of the checkout you run in
- the home directory
- `/tmp`
- the directory holding the status file
- your scratch directory
- any jj workspace or build directory you later create on another filesystem (restart the sampler with it added)

Sample every 60 seconds, at the script's default alert levels.

- In Claude Code, start the watchdog in this order:
  - run `watchdog observe --status <file>` under the Monitor tool;
  - then run `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout;
  - then run `watchdog check --status <file>`.

  Every line either monitor prints is a notification. Whenever you re-arm either one, stop the other and arm both again,
  observer first, then run `check`. An expiry notice is routine; a monitor exit is a monitoring failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer. After a `STALE`, also stop the hung sampler through its
  handle and arm a new one. Then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`:
  - a sampler run with `--synthetic-alert` delivers an `ALERT`;
  - a sampler killed through the `pid` in its status file is reported;
  - a sampler run with `--stall-after 2`, watched by its own observer armed first, produces `STALE` within two sample
    intervals.

  Stop the throwaway runs afterwards. If the mechanism is unavailable or fails this, say so in the log, run the sampler
  as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act:
  - stop launching new sub-agents and work;
  - remove jj workspaces, build output and scratch you own that are no longer needed;
  - wait for or stop the job most likely responsible;
  - resume launches only once `check` passes.

  A monitoring failure (`STALE`, `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until
  monitoring is restored. Stop a hung sampler before starting another, so that two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log, and carry them in every handoff. A
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- Use the `jjstack` skill to build a single linear stack with one reviewable commit, stable bookmark and draft PR per
  outline step. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/auto-update-hosts-default/<nn>-<short-name>`, as the outline names them.
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, restructure a PR that needs correcting rather than stacking a correction on top. That applies to PRs
  an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you. Each PR still leaves the fragment its type calls for, describing in user-facing words what that PR
  built. Where what got built departs from the release note entry (a renamed control, a part left out, a limitation it
  does not mention), the fragment's context paragraphs say how (`plans/.agents/executing.md`, "Executing one plan", step
  8). Never edit the `## Release note` section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- GUI drawing changes update the screenshot goldens in the same PR, as `.agents/validation.md` describes:
  - run `scripts/check-gui-goldens.py --update`;
  - look at every changed or added picture its report names before committing;
  - then run `python3 scripts/gui-goldens.py publish` and commit the rewritten manifest lines.

  A golden change you did not expect is a finding to explain or fix, not a picture to accept.
- The last PR of the stack removes this plan's TODO.md entry. If you take the devbuild fallback, the same PR adds the
  new Near term entry; the maintainer authorized that entry.
- Changes to `scripts/` or a check update the inventory in `.agents/validation.md` when root `AGENTS.md` says they do.
  Any change to an instruction file runs `python3 scripts/check-agents-md.py`.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and a reasonable retry fails too, push what is consistent and block the plan per
`plans/.agents/executing.md`, "Executing one plan" (step 10). Never substitute another reviewer.

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
- the acceptance criteria (this file's goal, acceptance criteria, decisions and outline).

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim in the prompt,
as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17) is that its findings are inputs, not orders. Take a
correctness finding (a bug, a missed case, a wrong claim) after checking it yourself. Decline by default a finding that
would add code, a new mechanism, a new test layer or another round of hardening, unless you judge it plainly necessary.
Never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort in two cases:

- before implementing a substantial departure from the outline above, including a new mechanism that does the job of an
  existing one or a generalization the outline did not plan. The devbuild route is the likeliest case, and the
  plan-specific examples under Fallbacks are examples, not a blacklist.
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. Give a sub-agent a bounded unit within the outline. An unapproved new mechanism is outside its task: if it
finds one mid-unit, it leaves it unimplemented and reports back, stating its checks and incomplete work honestly. Apply
the same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. That includes the ones listed after Fallbacks above, and every review finding you decided not
to follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. If wording in this file turns out inaccurate (a line number, a file name, a test name), record the true value in
the working log; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step
10), only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
either gets decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it becomes a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into
the report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.
Three belong there in any case:

- the power-loss review item named under Decisions;
- `review_feedback_queue/staged-payloads-and-automatic-host-updates.md`, which asks whether automatic updates can push
  an operator-staged payload directory and is now less hypothetical (mention it, do not resolve it);
- that the next release's brick test is the first end-to-end run of the adapted step 5.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outline step in the order the outline gives. The
stack must:

- together meet the acceptance criteria above;
- have each PR passed through its review gate;
- remove this plan's TODO.md entry in its last PR.

The PRs stay open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

Farhelm now updates your remote hosts automatically instead of waiting for you to click; this applies to existing
installations as well as new remote hosts. If you would rather update hosts yourself, turn off Update remote hosts
automatically in Settings → Hosts after upgrading.
