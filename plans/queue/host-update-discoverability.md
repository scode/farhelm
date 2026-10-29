# Hosts: update prompts in the sidebar and automatic host updates

Written against main at c2789e25 on 2026-10-29. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

Two TODO.md entries (bucket "Near term"), verbatim, planned together at the maintainer's choice:

> - **Add an "update all hosts" button and an auto-update setting.** In the current interface, updating hosts is hidden
>   behind clicks and hard to discover. Put a clearly visible update control (an "update all hosts" button or similar)
>   where users look for it, and add a setting to always auto-update. Plan:
>   `plans/queue/host-update-discoverability.md`.

> - **Show an upgrade arrow on the hosts icon when any host needs an update.** The hosts icon in the bottom-left only
>   surfaces update availability once the host list is open; when at least one host needs an update, show an upgrade
>   arrow on that icon itself, so updates are visible at a glance. Plan: `plans/queue/host-update-discoverability.md`.

The maintainer's framing, from the planning page: "The upgrade indicator makes you want to go and click. The problem
right now is that there's nothing that makes you go and click to the place where you get the actual host list to
upgrade."

What exists today: each remote host's rail avatar already gets an amber `↑` badge when it has an update
(`HostRailAvatar` in `crates/farhelm-ui/src/hosts.rs`), and the Hosts panel's header has `Update all`. The Hosts button
at the foot of the rail (built in `crates/farhelm-ui/src/list/view.rs`, placed in `div.host-rail-footer` by
`crates/farhelm-ui/src/app_bar.rs`) has no mark, and clicking a rail avatar filters sessions rather than leading to an
update. There is no setting that updates hosts automatically; the desktop app's own automatic-update switch explicitly
does not cover remote hosts (SPEC.md, "Installation and updates").

### What changes

"Has an update" means exactly what the rail avatar's `↑` means today: `remote_update_available(..)` and
`host_update_kind(&host.state).is_some()` (hosts.rs, where the rail builds each avatar). `host_update_kind` splits it
into **optional** (a connected host on an older build) and **required** (a host whose protocol is lower, so it cannot be
used until updated). Every new surface, and the helm's automatic update, uses this one test, so they all agree with the
rail.

NOTE: this is not what the Hosts header's `Update all` acts on. Its list (`available_remote_updates`) is every idle
remote SSH host whose Update menu item is offered, which today includes up-to-date, connecting and unreachable hosts;
SPEC.md words it as "each remote host whose individual Update action is available". This plan does not change
`Update all`. It must not reuse that list for the new surfaces either.

1. **Hosts button badge.** While any remote host has an update, the Hosts button carries an `↑`: red when any host's
   update is required, amber otherwise. Its tooltip and accessible name say how many hosts have updates. The badge stays
   when the card below is dismissed.
2. **Card, for optional updates.** While at least one host has an optional update and none has a required one, a card
   sits at the bottom of the session list with an arrow pointing at the Hosts button. It says "2 hosts have updates"
   (singular for one), lists the hosts' names, and has `Update all now`, `Show hosts` and `✕`. `Update all now` sends
   the existing per-host Update request for exactly the hosts the card lists. `Show hosts` opens the Hosts panel. `✕`
   hides the card until a new update appears, meaning a host with an update that was not in the dismissed set, or a
   different helm build. The dismissal lives in interface memory only and is forgotten when the interface restarts. The
   maintainer: "its fine if it reappears on UI restart. don't make this complicated so it requires helm state just to
   track whether to show the dialog."
3. **Banner, for required updates.** While at least one host's update is required, a danger-toned banner sits at the top
   of the session list. Wording for one host: "build-01 must be updated before you can use it", with a line saying its
   supervisor speaks an older protocol than this helm and its sessions show as unreachable until then. For several: "3
   hosts must be updated before you can use them". Buttons: `↑ Update now` (several: `↑ Update all three`, or the number
   in figures past nine), which sends the per-host Update for exactly the required hosts, and `Show hosts`. The banner
   has no `✕` and stays while any host must be updated. While the banner shows, the card does not; the card returns for
   optional updates once no update is required.
4. Neither the card nor the banner shows while the Hosts panel is open; the session list is hidden then anyway.
5. **Automatic updates.** A Settings switch, "Update remote hosts automatically", off by default for new and existing
   installations, stored as a helm-kept preference so every client sees the same value. While it is on, the helm itself
   starts the same authorized Update a click would for each remote host that has an update (the test above, optional and
   required alike), so it works with no window open. It follows Update's existing rules: never the local host, skip a
   host busy with setup or another run, never a downgrade. Progress and failure show exactly as for a manual Update. A
   failed automatic update is not retried until that host's connection changes (its incarnation, the token the UI
   already uses for its connection) or the helm restarts, so a broken host does not loop. If the update is refused at
   its planning step (a downgrade, a newer database schema, a frozen identity, a busy host, a backend failure), no run
   starts and no window can show it: the helm logs the refusal and records it in the same no-retry memory, and the host
   keeps its `↑`, card or banner, so a manual Update shows the reason. No new surface for this.

### What does not change

The rail avatars' badges and the Hosts panel, including `Update all`, its button and tooltip; the per-host Update action
and its refusals; Update never confirming (the click, or the switch, is the authorization); the desktop app's own
automatic-update switch and its scope; the dev-build rule that a helm built from main treats host age as unknown, so
only required updates appear there.

### Acceptance criteria

- The Hosts button's `↑` appears whenever, and only when, some rail avatar shows `↑`; it is red if any update is
  required, else amber; its tooltip and accessible name give the count.
- The card appears for optional updates under the rules above, its `Update all now` sends Update for exactly its listed
  hosts and no others, `Show hosts` opens Hosts, and `✕` hides it until a host outside the dismissed set gains an update
  or the helm build changes; a reload shows it again.
- The banner appears whenever a host's update is required, cannot be dismissed, replaces the card while present, and its
  button updates exactly the required hosts.
- Neither shows while Hosts is open.
- The Settings switch is off by default, its value survives helm restarts and is shared by every client, and an older
  helm refuses the new schema (the repository's pattern).
- With the switch on, a host that gains an update is updated by the helm without any click, also with no window open;
  turning the switch on updates hosts that already had one; busy and local hosts are skipped; a failed or refused
  automatic update is not retried until that host's connection changes or the helm restarts; with the switch off,
  nothing happens automatically.
- SPEC.md and SPEC_impl.md describe all of this, including the "has an update" test as distinct from `Update all`'s
  list, and the logged-not-surfaced planning refusal.
- Each PR carries its own changelog fragment (PR 1 `kind: added`; PR 2 `kind: added`, or an edit of PR 1's fragment if
  it is still unreleased).
- Both TODO.md entries quoted above are removed in the plan's last code PR (`plans/.agents/planning.md`: a plan's last
  code PR removes the entries it covers).

## Decisions already made

Settled with the maintainer on 2026-10-29 on a planning page with mockups:

- **One plan for both entries.**
- **Surfaces:** "let's do C when hosts CAN be updated. let's then add (D) when hosts _MUST_ be updated (protocol change)
  with wording adjusted appropriately." C is the card, D the banner, as described above. Not chosen: an "Updates
  available" section at the top of the Hosts panel.
- **The Hosts button's `↑` is always shown while an update is available**, including after the card is dismissed. Chosen
  over showing it only after dismissal, or not at all.
- **The card has `✕` and stays dismissed until a new update appears; the banner has no `✕`.** The dismissal is not
  stored anywhere; see the maintainer's words above.
- **Auto-update:** one switch in Settings, "Update remote hosts automatically", off by default, acting on optional and
  required updates alike, with no retry after a failure until something changes. Chosen over a global switch plus a
  per-host opt-out, and over a per-host switch only.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Refused-before-start automatic updates are logged, recorded as failed, and not surfaced separately.** Proposed on
  the planning page after the planning review raised it.

## Requirement sources

**The user's request:** the two TODO.md entries, the maintainer's words quoted above, and the decisions.

**Binding repository constraints:** SPEC.md, "Topology" (host provisioning, Update and `Update all`) and "Session list"
(the host rail, the Hosts toggle, the rail's update badge, the inline Update button and the Hosts header); SPEC.md,
"Installation and updates" (the desktop app's automatic-update switch and its exclusion of hosts); SPEC_impl.md's
preferences paragraph (the `preferences` singleton row in `helm.db` behind `GET`/`PUT /api/preferences`, a column and
schema bump per preference, older helms refusing newer schemas); root `AGENTS.md`, "Code rules", "Tests and validation",
"Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`;
`.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): the card's dismissal remembered as
the set of host ids shown plus the helm build stamp the UI already latches for its build-skew notice; the no-retry key
(host, incarnation) in helm memory; the singular/plural and number wording; hiding the card and banner while Hosts is
open; the two-PR split. A fresh-context planning review checked the outline against the code and contributed the
corrected "has an update" test, the exact-set update action, the data-ownership note, the dismissal and no-retry
simplifications, the planning-refusal handling and the per-PR bookkeeping.

## Outline

### PR 1: badge, card and banner (interface only)

1. **One classifier.** A pure function over the hosts read and the provisioning menu states that returns the required
   and optional sets, built from the rail-avatar test above. The rail avatar, the Hosts button badge, the card and the
   banner all use it. Unit-test it (optional, required, none, busy, too new, local, unreachable).
2. **Data ownership.** The menu states (`provisioning_menu_states`) and the Update request queue
   (`provisioning_action_requests`) live inside `HostsPanel` today, while the Hosts toggle, the session list and
   `hosts_panel_open` belong to `ListView`; only `provisioning_busy_hosts` is passed down from `ListView`. Lift the two
   signals into `ListView` beside `provisioning_busy_hosts` and pass them down the same way. (The alternative, having
   `HostsPanel` render the card and banner itself and publish a summary for the badge, also works; pick one and log it.
   `HostsPanel` stays mounted while concealed, so the data exists while the session list shows.)
3. **One update action** for the card and the banner: queue the existing per-host Update request for an exact set of
   host ids. Do not call `Update all`.
4. **Badge** on the Hosts toggle; **card** and **banner** in the session list, as described above. The banner can follow
   the existing build-skew band's structure (`crates/farhelm-ui/src/skew.rs`) with the danger tone. The card's dismissal
   memory is a signal in `ListView`. Keep the rail's per-host `↑` as it is (always amber); the red/amber distinction is
   for the Hosts button, card and banner.
5. **Spec, changelog, TODO.** SPEC.md's Session list paragraphs (the Hosts toggle, the rail's update badge) and the
   Topology paragraph on `Update all` (to define "has an update" next to `Update all`'s own list); SPEC_impl.md where it
   describes the rail and panel data flow; a changelog fragment.
6. **Tests.** Unit tests for the classifier and the card's visibility and dismissal logic. Browser tests for the badge,
   the card (including that `Update all now` sends exactly the listed hosts and that `✕` survives until a new host
   appears and not past a reload) and the banner. Look at how the existing Hosts and rail tests fake host states and
   versions before writing new fixtures; a dev-build helm sees no optional updates for real version gaps, so the tests
   need the harness's way of presenting them.

### PR 2: automatic updates (helm and Settings)

1. **Preference.** A new `preferences` column (for example `auto_update_remote_hosts`, default false) with the schema
   bump and `GET`/`PUT` plumbing the existing preferences use, and a Settings switch "Update remote hosts automatically"
   with one line of help saying it updates remote hosts as soon as an update is available and never the helm's own
   machine.
2. **Helm trigger.** When the preference is on, scan the remote hosts whenever the host fleet changes (the helm's
   existing fleet revision bump and per-host status channels) and when the preference is switched on, and for each host
   that has an update by the same test (in helm terms: connected on an older build, or a version skew with a lower peer
   protocol; a kind that allows panel updates; not busy in the provisioning service), run the same plan-then-start
   Update path a click uses. Keep an in-memory set of (host, incarnation) pairs whose automatic attempt failed or was
   refused, and skip those.
3. **Spec, changelog, TODO.** SPEC.md (the switch, what it acts on, the no-retry rule, the logged planning refusal, and
   the sentence in "Installation and updates" that says remote hosts are handled by the host list's own actions);
   SPEC_impl.md (the preferences column list and schema note, and the trigger); a changelog fragment or an edit of PR
   1's; remove both TODO entries.
4. **Tests.** Helm tests for the trigger: updates a host that gains an update; acts on switching on; skips busy and
   local hosts; does not retry a failed or refused attempt on the same incarnation and does retry on a new one; does
   nothing when off. Store tests for the schema bump and default. A UI test for the switch.

Before touching these paths, read the instruction file root `AGENTS.md`, "Instructions by path", names for them:
`.agents/validation.md` and `.agents/test-authoring.md` for the tests.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy with `-D warnings` on the
crates touched, the UI crate's tests, the helm crate's tests around preferences, the store and provisioning (through the
pinned nextest setup and `python3 scripts/record-test-run.py`, per `.agents/test-evidence.md`), `dprint check` on
changed Markdown, `python3 releasing/check-changelog.py format`, and the new and affected browser tests (Hosts, rail,
Settings) on Chromium and WebKit through the recorder. Run `python -B scripts/check-test-sleeps.py` when Rust or browser
tests change.

## Fallbacks

Departures that need the scope reassessment: changing `Update all`'s list or behavior; storing the card's dismissal
anywhere but interface memory; a per-host auto-update setting; a new surface for automatic-update failures; retrying
automatic updates on a timer; updating the helm's own machine automatically; driving automatic updates from the
interface rather than the helm.

Decisions to log: where the shared data lives (lifted signals or `HostsPanel`-rendered surfaces), the exact banner and
card wording, the preference column name, how the helm trigger observes host changes, and every review finding you
decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-host-update-discoverability-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
available as a native sub-agent type (the code reviewer below, for instance), `$scode-harness-shellout` supplies the
launch mechanics.

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
  `plan/host-update-discoverability/<nn>-<short-name>` (expected: `01-update-prompts` and `02-auto-update`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Code review gate

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
findings file in the scratch directory, and the acceptance criteria (this file's goal, what changes and what does not,
acceptance criteria, decisions and outline). When a PR changes tests or their fixtures, include the full text of
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
Wording in this file that turns out inaccurate (a line number, a file name, a class name) is corrected to the truth.

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
passed its code review gate, and the last PR removes both TODO.md entries. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.

## Decisions

### 2026-10-29: answer to a blocked question

The question, as the executor put it:

> Blocked while landing on 2026-10-29 (claim ed19df).
>
> ### Landing stopped: two database migration tests and the new setting's hover text
>
> The landing monitor did not merge this plan; its PRs (#2807, #2808) have not merged and main holds nothing of it. The
> plan makes host updates easier to find (an update prompt in the session list) and adds a Settings switch, "Update
> remote hosts automatically", under Hosts, which updates remote hosts on older builds when they connect.
>
> The round's checks ran on this plan stacked with three others on current main. A fresh-context review found the
> automatic updates safe as specified (off by default, remote hosts only, through the manual Update path). Three checks
> failed because of this plan:
>
> - Two helm tests of the settings database's history, "checkout open existing refuses older schema without migrating"
>   and "checkout migration from 26 recreates the config tables"
>   (`cargo nextest run --workspace --exclude farhelm-desktop`, run `43959a88-f565-461e-b3dd-760d8725cf4a`), fail. The
>   tests rebuild a version-26 database by removing what later versions added and compare it with the frozen version-26
>   definitions; that removal step was not told about the plan's new preferences column for the automatic update
>   setting, so the rebuilt database still has it. The frozen definitions and the migration are correct.
> - The browser sweep "sidebar surfaces give every control hover text" (`e2e/tests/tooltip-coverage.spec.ts`, run
>   `10af6125-cdc5-4afc-bc9a-369377c00195`, both engines) finds the new "Update remote hosts automatically" switch in
>   Settings without hover text, which every control needs; its neighbour "Set up new hosts without asking" has one.
>
> The review also noted that the Update guide on the docs website still says remote hosts are never updated
> automatically, and that the Manage hosts page does not mention the new setting or prompt.
>
> Two things for you to know, neither a reason to stop: the plan raises the settings database to version 68, a one-way
> upgrade, so a helm built before it (the v0.32.0 release candidates included) cannot open the store afterwards, which
> matters for rolling back and for devbuilds; and a remote host whose automatic update keeps failing is retried on every
> reconnect.
>
> ### What should happen to this plan?
>
> - **Send it back to its executor (recommended).** Answer with: in the tests' step that rebuilds a version-26 database,
>   also remove the new automatic-update preferences column, as it already removes every other column added after
>   version 26, leaving the frozen definitions and the migration as they are; give the new switch hover text the way its
>   neighbour has it; update the Update guide's sentence and mention the setting and prompt on the Manage hosts page;
>   run the helm crate's tests and the settings, host-updates and tooltip-coverage browser specs on both engines.
> - **Land it now and fix afterwards.** Main would carry two failing migration tests, a control without hover text, and
>   a docs page that contradicts the new setting.

The maintainer's answer:

Answered by the landing monitor on the maintainer's behalf under `plans/.agents/landing.md`, "Answering blocks": a
landing that failed mainly because tests fail goes back with an instruction to fix them.

Decision: send it back to its executor.

- In the helm's settings-store tests that rebuild a version-26 database ("checkout open existing refuses older schema
  without migrating", "checkout migration from 26 recreates the config tables"), also drop the new automatic-update
  preferences column beside the other columns added after version 26. Leave the frozen version-26 definitions and the
  migration as they are.
- Give the "Update remote hosts automatically" switch in Settings hover text, the way its neighbour "Set up new hosts
  without asking" has it, so the tooltip-coverage sweep passes.
- In the docs website's Update guide (`update-and-uninstall.md`), replace "Your remote hosts are not updated
  automatically." with a sentence saying they are not updated automatically unless you turn on **Update remote hosts
  automatically** in Settings, under **Hosts**, and keep the pointer to Manage hosts. On the Manage hosts page, mention
  the setting and the session list's update prompt in the section on updating a host.
- Run the helm crate's tests, the website build, and the settings, host-updates and tooltip-coverage browser specs on
  both engines.

No judgment call is made here, so no TODO entry is recorded.
