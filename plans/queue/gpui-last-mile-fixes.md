# GPUI last mile: launcher and session-pane fixes and the launcher's remaining parity tests

written against main at 4f6cd2ad on 2026-11-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The GPUI desktop app replaced the browser UI. Its migration ended with a few small behavior gaps and a list of old
launcher tests not yet ported. This plan closes them, as recorded in the "GPUI last mile" TODO.md entry that points
here. Every old test case is read from the last browser-UI commit with `git show 24bc0580:e2e/tests/<name>.spec.ts` (or
the named file there); `docs/gpui/parity/launcher.md` is the authoritative list of what is "not yet ported", and its
rows are updated as each case is ported or given a disposition.

### What the PRs do

1. **A launch onto a suspended host says so.** Today the launcher refuses a launch whose host is not connected with
   "Choose a connected host before launching" (`crates/gui/farhelm-gui/src/launcher/request.rs`, and the templates
   quick-launch path that shows "Not launched. Choose a connected host before launching"). When the chosen host is a
   sandbox or container that is suspended (or stopped and resumable), the reason instead names the host and says to
   resume it, worded through the provider wording module (`crates/gui/farhelm-gui/src/environment_ui.rs`, `ProviderUi`)
   so a container and a sandbox each read right. The reason reads "<host> is suspended. Resume it in Hosts, then
   launch." (wording chosen by the maintainer on 2026-11-17), and a quick launch shows it after "Not launched. ".
   Nothing resumes automatically. SPEC.md "Topology" currently says "Launching on a suspended host resumes it first and
   waits for a fresh connection"; add an interim sentence there saying the desktop app refuses such a launch for now and
   asks the user to resume the host, and that resuming before the create is planned. Record the deviation in
   `docs/gpui/parity/launcher.md` and give the old environment cases that depend on resume before create
   (tensorlake-lifecycle's 13 launch-after-wake cases; tensorlake-sandbox-sessions' "an existing-host launch stays
   nonmodal while its Resume is held", and "Hosts refuses a host's Resume and Settings while a launch is waking that
   host") the disposition "deferred: resume before create is a near-term TODO item", replacing their "not yet ported".
   The create half of "a held sandbox Suspend and a launch on another host proceed side by side" does not depend on
   resume before create (its launch goes to another, connected host): port it in step 6 as a test that the create is
   sent while the unrelated Suspend is still unanswered, using the fake helm's observable queued commands, and record
   any part the fake's in-order replies cannot show as that limitation. Also in
   `docs/gpui/parity/sidebar-and-session-view.md`, the deviation starting "Not yet ported, waiting for the launcher's
   environment work" (Suspend and Resume refused while this window's own launch on the host runs) gets the same deferred
   disposition. Type `fix:`. The released browser app resumed the host itself, so this is a change for users: add one
   sentence to the existing `releasing/changelog.d/gpui-desktop.md` fragment (it covers the unreleased desktop app, so
   this amends it rather than adding a fragment, per root `AGENTS.md`, "Changelog fragments") saying a launch onto a
   suspended sandbox or container now asks you to resume it first.
2. **The suspended session card names its provider.** The session pane's unavailable card for a suspended sandbox or
   container says "Asked the environment ..." where the Hosts row already says "Asked Tensorlake ..." through
   `ProviderUi::lifecycle_asked` (`crates/gui/farhelm-gui/src/environment_ui.rs`; its Hosts use is in
   `hosts/identity/phase.rs`). Make the card use the same wording and drop the deviation in
   `docs/gpui/parity/sidebar-and-session-view.md` starting "While a Suspend or Resume waits for its provider".
3. **Clone's copied folder and session name are spelled out.** In the launcher, a Clone's copied folder and session name
   holding an invisible or direction-changing character are shown as they are; the old app spelled them out as
   `<U+...>`. In `crates/gui/farhelm-gui/src/launcher/view/shown_text.rs` the copied command and resume command follow
   an opened-value rule (spelled out while the value equals the opened one, so typed text is never escaped); use that
   pattern for the folder and name fields, leave the model and Custom flags behavior as it is, test it, and drop the
   Deviations bullet in `docs/gpui/parity/launcher.md` starting "A Clone's copied folder and session name holding a
   hidden character". About 30 production lines.
4. **Checkout composer and host picker tests (old PR 6b).** github-checkout-composer 161 (the highlight stays in view
   through the list's wrap), 212's name-edit half (editing the name issues no repository read), 462 (the "No repository
   matches" row and its Enter; focusing the repository field closes the search results), 1297 (a host switch drops the
   old preview; the existing preview test covers only a reconnect); host-picker 104 (a press outside closes the list);
   launcher-enter's held Enter on the closed host button. Host-picker 163 ("retains the empty value until hosts arrive")
   is superseded, not ported: SPEC_impl.md, "The UI channel", opens the channel with a full snapshot of the pushed
   collections, hosts among them, so the launcher never opens before hosts arrive; cite
   `launcher::view_tests::the_host_button_is_empty_before_hosts_arrive`. 212's configuration-change half and 1297's
   setup check for the newly selected repository stay not yet ported unless they turn out cheap.
5. **Hover reasons, footer tiers, a refusal case (old PR 6c).** Launch's hover help for Restart with's three refusal
   reasons recorded as Deviations without a test ("settings changed; close and reopen restart with", "Wait for this
   session's other operation to finish", "Wait for the host change"; reuse `launcher::view_tests::launch_help`); the
   footer's button tiers (Launch primary while it can launch, Reset choices neutral), listed in
   `docs/gpui/parity/foundations.md` under `tests/buttons.spec.ts` as not yet ported (cite the new test there too);
   launch-refusal.spec 306 (a Save as template refusal after a refused Launch is not labelled "Not launched.").
6. **Leftover launcher cases**, each from its `docs/gpui/parity/launcher.md` entry's "not yet ported" list:
   launcher-tabs 186's other half (a folder row keeps the typed command) and 121 (a dormant invalid wrapper does not
   block Save as template on the Command tab, test in `templates::save_tests`); permission-switch 258's model-picker
   path drawn; named-conversations 15 and 84; nonblocking-launch Clone's and Replace with's halves of 345, 237 (Rename
   keeps the keyboard as a refusal arrives) and 970 (remembered selection stays the source's; the Started report holds
   no button or link); restart-with's composing Enter in the model field (superseded by the shared Enter rule if it
   cannot be driven; say so); the sidebar.spec launcher cases listed in that entry; terminal-multihost
   "create-dialog-selector-disappearance"; podman-environments "restored container placement displays the recipe and
   host it submits" (a `tl:` container template showing its recipe and host and sending them in one create; draft half
   `templates::apply_tests::a_container_template_enters_a_new_container_at_its_home`). Split this step into several PRs
   by parity entry if it grows past about 400 test lines.

### Acceptance criteria

- The six steps above are built, each PR passing its review gate, with the parity files updated so no launcher case is
  left "not yet ported" without a reason (a case may stay not yet ported only with a one-line reason why it is not cheap
  to drive, logged as a DECISION).
- A suspended host's launch is refused with the host named and the resume advice, in both the launcher and quick launch;
  SPEC.md carries the interim sentence.
- No change makes a launch resume a host.

## Decisions already made

- **Suspended-host launches (maintainer, 2026-11-17):** the full resume before create (the launch resumes the host,
  waits, then launches) is a separate near-term TODO item, not this plan. This plan does the simplest thing: keep the
  refusal, word it to name the suspended host and say to resume it, record the deviation, and add the interim SPEC note.
- **Test style (migration orchestrator, 2026-11-16):** parity tests are table-driven where that is natural and lean;
  composing Enter is superseded by the shared Enter rule; the github-checkouts 935 and 1047 server halves are dropped as
  supervisor behavior.
- **Repository text cites no coordination ids;** attribute a decision as "decided by the migration's orchestrator on
  <date>" or "decided by the maintainer on <date>".
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate), the maintainer's standing
  choice of 2026-11-17 for this work.
- **No-workhorse mode**, the plans default.

## Requirement sources

**The maintainer's request:** the "GPUI last mile" TODO.md entry that points here, and the decisions above.

**Binding repository constraints:** SPEC.md "Topology" (suspended hosts), "Sandbox sessions"; SPEC_impl.md "The UI
channel"; root `AGENTS.md`, "Code rules" (per-harness and per-provider decisions live in their named modules; no
`matches!` over providers in shared code), "Tests and validation", "Changelog fragments", "Conventional Commits";
`.agents/test-authoring.md`; `docs/gpui/parity/README.md` (status words).

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): the suspended reason as one
exhaustive `ProviderUi` method used by both refusal sites; the card wording through the existing `lifecycle_asked`; the
shared launcher test helpers
(`launch_tests::{launched, launched_with, launched_over, press, sent_to, creates, created, created_as, times_out_after_moving_on}`,
`view_tests::{launcher_on, launcher_over, launch_help, fixture_hosts, name_of, has_node, draft, read}`,
`pending_tests::fleet`). The fake helm (`farhelm_helm_client::fake::FakeHelm`) answers commands in order; a command sent
while an earlier one is out can be observed but not answered first.

## Outline

1. `plan/gpui-last-mile-fixes/01-suspended-launch-refusal`: step 1.
2. `plan/gpui-last-mile-fixes/02-suspended-card-provider`: step 2.
3. `plan/gpui-last-mile-fixes/03-clone-copied-escape`: step 3.
4. `plan/gpui-last-mile-fixes/04-checkout-host-tests`: step 4.
5. `plan/gpui-last-mile-fixes/05-hover-footer-refusal-tests`: step 5.
6. `plan/gpui-last-mile-fixes/06-leftover-launcher-tests` (split as step 6 says): step 6, and removing the TODO entry.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: for each PR, `cargo fmt --check`, clippy on
`farhelm-gui` with `--all-targets`, and the `farhelm-gui` tests the PR touches through
`python3 scripts/record-test-run.py`, all on the build workers; `python -B scripts/check-test-sleeps.py` when Rust tests
change; `dprint check` on changed Markdown. No Mac run is needed.

## Fallbacks

If the code has moved since this was written, follow the current code and record the corrected names in the working log
as a DECISION; never edit this plan file. Departures that need the scope reassessment: making a launch resume a host,
adding a Resume button to the refusal, or any new production state for steps 4 to 6 (they are tests and parity text,
plus a fix only where a ported case finds a real bug, which is logged and kept small). Decisions to log: the exact
refusal wording, each case left not yet ported with its reason, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-gpui-last-mile-fixes-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Build and test workers

Run every cargo build, test, clippy and GUI bridge run on the shared remote build workers, not on the machine you run
on, which many agents share. How to use them (the runner, which workers executors may use, how to run tests through the
recorder there, and the rules that keep worker names out of the repository) is in the file `boatrunner-info.md` in the
home directory of the user running you. Read it in full before your first build. Never copy a worker's name, account or
any path from that file into the repository, a PR, a Blocked question or your report. Formatters and small checks that
need no build (`dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`) may
run locally.

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
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (an opus
consult or the reviewer below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/gpui-last-mile-fixes/<nn>-<short-name>`,
  as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last code PR of the stack removes this plan's TODO.md entry (the "GPUI last mile" bucket).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

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

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

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
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (merging or
splitting steps where the work showed a better cut, logged as a DECISION), together meeting the acceptance criteria
above, each having passed its review gate, and the last code PR removing this plan's TODO.md entry. Open, not merged. If
a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
