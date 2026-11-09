# "Always update" in the sidebar's host update prompt

Written against main at 8e876c14 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **One-click "update and always auto-update" in the sidebar's host update prompt.** The prompt at the top of the
> sidebar offers Update, and Update all when several hosts qualify, while making future updates automatic lives only in
> Settings, as the "Update remote hosts automatically" checkbox. Add an action beside them that updates the listed
> update hosts now and turns that preference on, so the user does not have to find the setting afterwards. The label
> must be short, with the full meaning in its tooltip; the preference save needs the checkbox's failure handling so a
> write that fails is visible rather than leaving the user believing the hosts will keep themselves updated.

What is true today, from a read of main at 8e876c14 (line numbers approximate):

- The prompt is `HostAttentionList` in `crates/farhelm-ui/src/hosts/attention_list.rs` (around 195-245; rows in
  `AttentionHostRow` around 249-324), mounted in `crates/farhelm-ui/src/list/view.rs` (around 4771) only while the Hosts
  panel is closed. Its header has the heading ("N hosts need attention"), **Update all** only when more than one row
  offers Update (around 223), and a ✕ dismissal when an optional update row is present. Update all calls
  `on_update(updates)`, which reaches `request_shown_updates` (`list/view.rs` around 1033) and
  `updates::requests_for_shown` (`hosts/updates.rs` around 106): it re-checks eligibility, drops stale hosts, and queues
  one Update action request per host, which the always-mounted Hosts panel rows carry out (plan, then confirm with the
  probe id, `api.rs` around 4985-5001).
- The preference is `auto_update_remote_hosts`, saved with `PUT /api/preferences` as a single-field patch. The Settings
  checkbox (`crates/farhelm-ui/src/settings.rs` around 534-559) saves through `save_settings_preference` (around
  173-207, private to that module) with `api::observe_preference`: optimistic apply to the shared preferences, rollback
  on failure if this write is still the latest, an adjacent alert while the dialog is mounted ("Could not save whether
  to update remote hosts automatically. Try again."), then one reconciling read. SPEC.md around 2114-2119 states that
  promise for Settings.
- The helm acts on the preference immediately: saving `true` bumps the fleet revision, and the automatic updater
  (`crates/farhelm-helm/src/provisioning/automatic.rs`) scans and starts an Update for every eligible waiting host, one
  attempt per connection incarnation, never retried. A manual Update and an automatic one for the same host share the
  busy claim: the second is refused as busy (`service.rs` around 1993), and the losing row already says another run
  started first. Queuing the manual updates before saving does not decide who wins: the queued requests go out from the
  Hosts panel rows a moment later (two round trips each), while the save goes out at once. That race exists today
  (setting on, plus a manual Update) and is harmless; this plan does not try to settle it.
- Hosts the helm refuses at planning time (a remote Mac with the desktop app, a host below the upgrade floor, a newer
  schema) stay listed after either kind of update; that is existing behavior.
- SPEC.md: around 147-163 (Update all; "Update remote hosts automatically", including that enabling it covers hosts
  already waiting) and around 2376-2403 (the attention list, its Update all and dismissal rules).
- Tests: `e2e/tests/host-updates.spec.ts` fakes hosts with updates through `prepare()` (around 88-240) and covers the
  prompt (Update all around 585, single Update around 606, dismissal around 617); `e2e/tests/settings.spec.ts` covers
  the checkbox, including a refused save (around 142-200) and persistence (around 1102-1170). Rust tests for the rows
  are in `attention_list.rs` (from around 326).

### Acceptance criteria

- The prompt's header gets an **Always update** button whenever at least one listed row offers Update, including a
  single one (where Update all stays hidden), and only while `auto_update_remote_hosts` is off. Its tooltip says what it
  does: update the listed hosts now and turn on "Update remote hosts automatically", so later updates install without
  asking, changeable in Settings → Hosts (wording yours, keeping that meaning).
- Clicking it first requests updates for exactly the hosts Update all would (the same `requests_for_shown` path, so
  required and optional update rows alike), then saves `auto_update_remote_hosts = true` with the same optimistic apply,
  rollback and reconciling read as the Settings checkbox, sharing that code rather than copying it.
- If the save fails, the updates still go ahead and the prompt shows an error in place, under the heading (red text,
  `role="alert"`): the setting was not turned on, so later updates will still wait for you. It has **Try again**, which
  retries only the save. Its lifetime follows the Settings alert's rule (shown only while that failed write is still the
  latest choice for the setting), so it goes once Try again or any other write succeeds. It shows while the prompt shows
  (a failed save comes back within seconds, while the updated rows still show their progress); its state lives outside
  the prompt, so closing and reopening the Hosts panel (which hides the prompt) does not lose it. The prompt does not
  outlive its last row for it.
- A save that succeeds hides the button (the setting is now on) and Settings shows the checkbox ticked.
- The new button and the error line change the prompt's height; include them in what the sidebar tracks for attention
  list size changes (the layout signal that closes session menus below it), alongside today's row changes.
- SPEC.md's attention-list passage (around 2376-2403) and, if it needs it, the automatic-updates passage (around
  154-163) describe the new action and its failure surface.
- Browser tests extending `e2e/tests/host-updates.spec.ts`: the button appears with one and with several update rows and
  not when the setting is on; a click sends one update request per listed host plus one preference write (in any order:
  the race above makes the order on the wire unreliable); a refused write leaves the updates sent and shows the error,
  and Try again resends only the write and clears the error when answered. These tests answer the preference write in
  the browser and never let it reach the test helm: the hosts there are faked only in the page, and a real write would
  turn on automatic updates for the helm's real test hosts.
- A `feat` PR with a changelog fragment. The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08, on the planning page:

- **What the button does:** start updates for the listed hosts, then save the setting; a failed save shows in the
  prompt. The alternative, saving only the setting and letting the helm's automatic updater start the updates, was not
  taken.
- **Label: "Always update".** "Update all, always" and "Auto-update" were the alternatives. The page's mockup showed it
  as a neutral button after Update all and before the ✕, with the header wrapping onto a second line at the default
  sidebar width; that wrap is accepted.
- **Shown for a single update host too**, not only with two or more.
- **The mockup of the failure state** showed the heading, then the red error text with Try again beside it, then the
  rows below as working rows with their progress bars.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Planned on hold** at the maintainer's request; that changes nothing about how it runs.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's passages named above and the Settings save promise (around 2114-2119);
root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
`.agents/test-authoring.md`.

**Planner proposals:** hiding the button while the setting is on (Update and Update all already offer the manual retry
for hosts the automatic updater refused or already tried); reusing the Settings save code and its alert-lifetime rule
rather than writing a second save path; the error's wording. The error lives in this window only (like the Settings
alert); it is not persisted or shared across windows.

## Outline

One PR, `plan/update-and-auto-update/01-always-update`, typed `feat`: the button and its tooltip, the shared save (made
reachable from the attention list), the in-prompt error with Try again, SPEC.md, the browser tests, the changelog
fragment, and the TODO.md removal. Split the shared-save refactor into its own `refactor` PR below it only if it is
large enough to review separately.

### Validation

`e2e/tests/host-updates.spec.ts` and `e2e/tests/settings.spec.ts` on the engines `.agents/validation.md` names, through
`scripts/record-test-run.py`; the UI crate's Rust tests for what changed; `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md`.

## Fallbacks

Examples of departures that need the scope reassessment: changing what the automatic updater does, adding retries,
moving or redesigning Update all, or a new notification surface outside the prompt.

Decisions to log: how the save is shared, the error's exact lifetime, and every review finding you decided not to
follow. Making the manual updates win the race against the automatic updater is a departure, not a fix.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-update-and-auto-update-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/update-and-auto-update/<nn>-<short-name>`, as the outline names them.
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
