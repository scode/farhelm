# UI: an always-present list of hosts that need attention, above the sessions

Written against main at d81a3980 on 2026-11-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - **Always show a host list for hosts needing attention.** While hosts are installing or updating it is hard to find
>   where to look; today you must click the separate host-list icon. Restore an always-present host list at the top,
>   populated with any host that currently has a problem, needs an update, or is updating, so nothing needing the user's
>   attention is hidden.

### Background

Until #2750 (2026-10-27, the steel-sidebar plan) a host list sat permanently at the top of the sidebar. That plan moved
host management into the **Hosts** panel opened from the rail, recording "Host management in a panel opened from the
rail, over keeping a host list always visible; SPEC.md amended", and SPEC.md now says the Hosts control "swaps session
content for host management" and "Hosts never opens by itself" (search SPEC.md for both). Later plans added attention
surfaces inside the session list: a required-update banner (`HostUpdateBanner`) and an optional-update card with "Update
all now" and a ✕ dismissal (`HostUpdateCard`, `UpdateDismissal` in `crates/farhelm-ui/src/hosts/updates.rs`; the
host-update-discoverability plan, #2807), and a warning mark on a host's rail avatar after a failed or uncertain setup
or update, acknowledged by opening Hosts (`crates/farhelm-ui/src/hosts/attention.rs`, `reconcile_attention` in
`hosts.rs`). The maintainer's entry partly reverses the steel-sidebar decision: the full management panel stays where it
is, but hosts that need something are always visible.

### What changes

A compact **attention list** at the top of the session column (below the app bar, above the list title and status
filter), outside the session list's scroller so it stays visible while the list scrolls, visible whenever at least one
host needs attention and absent otherwise. Its rows sit in a bounded-height region that scrolls on its own, reusing the
optional card's bounded-height styling (SPEC_impl.md says the card "bounds its height and scrolls only its host names").
One row per host: its rail identity (avatar with its mark, short name), a one-line state, and the one action that state
offers. Each host is classified once, with this precedence: **Working** over **Problem** over **Update**.

- **Working:** the host has live setup, update or uninstall progress (an entry in the rail's per-host progress map),
  whatever its phase, so a fresh host that is Connecting while being set up, or a host that blips mid-update, reads as
  "Setting up" or "Updating", not as a problem. Show the verb and a progress bar from `UpdateProgressSummary`
  (`{ done, total }`, `provisioning.rs`), which the same progress map carries. No action button.
- **Problem:** Unreachable (worded by the existing `phase_display_label`, "Unreachable, retrying"; no since-when, which
  nothing records), IdentityMismatch, IdentityUnverified, Duplicate, Retired, Unrecognized; a VersionSkew host that has
  no Update offer, or whose skew is on the newer side (its remedy is updating the helm); a Tensorlake host whose resume
  failed (`Suspended { resume_failed: Some(_) }`); a Tensorlake host whose tunnels failed (`tensorlake/tunnels.rs`,
  `failed()`; the rail already marks it); and a host whose setup or update needs attention per
  `HostAttention::unseen()`, worded with the rail's existing generic text ("setup or update needs attention") so
  `attention.rs` needs no new accessors. Action: **Details**.
- **Update:** a host with an available Update action, required or optional, as `updates::classify` reports (it already
  excludes busy hosts); an older-side VersionSkew host with an Update offer is an Update row marked required, and the
  required rows say that sessions on that host stay unreachable until it is updated (the explanation the banner carried,
  SPEC.md's required-update text). Action: **Update**.

The list's header reads "N hosts need attention" and carries **Update all** when more than one row is an Update row; it
updates exactly the listed Update rows (`request_shown_updates` with those hosts). The banner's "Update now / Update all
three" wording and `required_action` go. The Hosts panel header's own broader **Update all** is unchanged; if the shared
label reads ambiguously in SPEC.md, say which is which there.

**Dismissal stays for optional updates:** the card's ✕ dismissal is kept, reused unchanged from `UpdateDismissal` but
applied to the Update group's optional rows only: a ✕ in the list (on the Update group, or on the header when only
optional Update rows remain) hides those rows until a host outside the dismissed set gains an update or the helm build
changes, exactly as the card's dismissal works today. Required rows, Problems and Working rows cannot be dismissed. The
list is absent when nothing undismissed needs attention.

A host leaves the list as soon as its condition clears. Clicking **Details** or a row outside its button opens Hosts
(`show_hosts_from_update` in `list/view.rs` already does that and closes popovers) and brings that host's row into view
with a one-shot focus-and-`scrollIntoView` of `[data-host-id="{id}"]`, following the pattern in `list/trash.rs`
(`requestAnimationFrame`, focus with `preventScroll`, then `scrollIntoView({block:'nearest'})`; `FocusTarget::focus`
alone does not scroll). The focus ring on that row's menu button is the only highlight. Opening Hosts already
acknowledges the warning mark through `reconcile_attention`; no separate acknowledgement path is added.

The required-update banner and the optional-update card in the session list are removed, with `HostUpdateBanner`,
`HostUpdateCard`, `required_action`, `card_visible` and their CSS in `app.css` (`.host-update-banner`,
`.host-update-card` and related). The session-region movement hook that the banner and card feed (`update_prompt_layout`
and `use_rendered_movement_between` in `view.rs`, which closes popovers measured against old geometry) is re-keyed on
the list's layout: which rows are shown, their groups and their wrapped copy, but never the progress text, which changes
every poll and would close every open session menu each tick. The ↑ on the Hosts button and the automatic remote update
setting are unchanged.

### What does not change

The Hosts panel, its contents and how it opens; the rail and its marks, tooltips and filtering; Hosts never opens by
itself. Not listed: a host that is merely Connecting without progress; a connected, current host; a connected host
running a newer Farhelm ("Too new"), which is usable; a Suspended Tensorlake host whose resume has not failed, whoever
suspended it. While Hosts is open the attention list is hidden, as the banner and card are today.

### Acceptance criteria

- The list appears exactly when some host is in a listed, undismissed condition, with one correctly worded row per host
  under the precedence above, displayed in the order Problems, Working, Updates (rail order within each).
- Details and row clicks open Hosts with that host's row in view and focused; Update and Update all start the same
  updates the removed banner and card started, with the same confirmations and refusals.
- Optional-update dismissal works as the card's did and is invalidated under the same rules.
- Rows update live from the fleet feed; a host disappears when its condition clears; open session menus do not close on
  progress updates, and do close when the list's layout changes, as the movement hook requires.
- The banner and card are gone with their CSS; nothing only they offered is lost (their actions, the required-update
  explanation, dismissal).
- The list fits narrow sidebars (truncating names with tooltips as the rail does) and works in both themes; its rows
  scroll within the bounded region when there are many hosts.
- SPEC.md: the Hosts and session-list text gains the attention list, the banner and card text is replaced (including the
  dismissal and the required-update explanation now living on rows), and the steel-sidebar sentence about not keeping a
  host list visible is amended to say what is always visible now. SPEC_impl.md: the classifier and its inputs, where the
  lifted signals live, the movement key, and the removal of the banner and card. Docs:
  `website/src/content/docs/docs/using/manage-hosts.mdx` (intro and "Update a host", which describe the card and banner)
  and `website/src/content/docs/docs/get-started/add-a-remote-host.mdx` (the "until you open Hosts" text).
- Tests: unit tests for the classifier (every phase, the precedence, dismissal); browser tests for a host in each group
  with the right text and action, a host leaving when its condition clears, Details opening Hosts at the host, Update,
  Update all, dismissal and its invalidation, the list hidden when nothing needs attention and while Hosts is open, and
  an open session menu surviving a progress update. `e2e/tests/host-updates.spec.ts` is the only spec that asserts the
  banner or card; update or replace those cases. A suspended or resume-failed Tensorlake case fits in
  `e2e/tests/tensorlake-hosts.spec.ts` if it can be made without the real Tensorlake SSH gateway (see that spec's
  existing approach); otherwise cover it in the classifier's unit tests.
- A changelog fragment (`kind: changed`) and removal of the TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-11-04 on a planning page with mockups (a dark mockup of a bordered box at the top of
the session column headed "3 hosts need attention" with **Update all**, and rows for "devbox · Updating to 0.36.0" with
a progress bar, "gpu-1 · Unreachable since 14:02 · retrying" with **Details**, and "ci-2 · Update available: 0.35.0 →
0.36.0" with **Update**, above the usual list title and rows):

- **Hidden when empty:** no "All hosts up to date" line.
- **Optional updates count** as needing an update, as well as required ones.
- **The banner and card fold into the list** (Update all in its header) and are removed: one place to look.
- **Clicking a host row opens Hosts** with that host's row in view (not a host filter).
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

Stated on the page and not objected to: Connecting alone is not listed; Unreachable is; a failed setup or update stays
listed under the rail warning mark's rule.

Planner decisions made after the page closed, from the planning review against the code, to be reported to the
maintainer with this plan: the mockup's "since 14:02" is dropped because nothing records when a host became unreachable
(adding it would be a helm and protocol change); the card's optional-update dismissal is kept rather than silently lost,
since the maintainer asked to fold the card in, not to drop its behavior.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's Hosts, rail, update-banner and update-card (including dismissal), and
failed-setup attention text; SPEC_impl.md on the hosts panel's single mounted owner, the fleet feed, the banner and
card, and session-region movement; root `AGENTS.md`, "Code rules", "Tests and validation", "Conventional Commits",
"Changelog fragments"; `website/AGENTS.md`; `.agents/test-authoring.md`; `.agents/validation.md`. Docs screenshots under
`e2e/docs-shots/` are fenced media (`.agents/docs-media.md`): do not refresh them; name in the report the shots this
change makes stale.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION), refined by a fresh-context
planning review against the code: a pure classifier in a new child module of `hosts` (for example
`hosts/attention_list.rs`), so it can use the private helpers in `hosts.rs` (`rail_presentation`, `host_update_kind`,
`RailPresentation`, `remote_update_available`) and be unit-tested without rendering. Its inputs are the per-host
progress and attention signals, which today are local to `HostsPanel` (`row_update_progress` and `row_attention` in
`hosts.rs`): lift both to `ListView` (beside the maps it already owns; SPEC_impl.md has the precedent) and pass them to
`HostsPanel` as props, then render the list in `view.rs` where the banner sits, which already owns `host_updates`,
`request_shown_updates` and `show_hosts_from_update`. Two PRs: the list replacing banner and card, then spec, docs and
the TODO removal (or one, if that reviews better).

## Outline

1. **Classifier and signals.** The pure classifier with its unit tests; lift the two signals to `ListView`.
2. **Attention list.** Rendering, the bounded region, Details and row clicks, Update, Update all, optional dismissal,
   hiding while Hosts is open, and the re-keyed movement hook.
3. **Remove the banner and card** with their CSS and helpers.
4. **Spec, docs, bookkeeping.** SPEC.md and SPEC_impl.md, the docs pages (an incidental docs edit in an unattended run:
   start no preview server and name the changed pages in the report), the changelog fragment, the TODO.md entry removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy on the UI crate, the UI
crate's tests, `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, and, through
`python3 scripts/record-test-run.py`, a new attention-list spec plus `host-updates.spec.ts`,
`host-rail-warning.spec.ts`, `hosts-panel.spec.ts`, `provisioning.spec.ts` and the Tensorlake host spec if touched, on
Chromium and WebKit. Run `python -B scripts/check-test-sleeps.py` when browser tests change.

## Fallbacks

Departures that need the scope reassessment: putting the list in the rail instead of the session column; listing
connecting or healthy hosts; recording an unreachable-since time; changing the Hosts panel itself; keeping the banner or
card alongside the list; a new scroll-to-host signal or highlight state in `HostsPanel`; new accessors on
`attention.rs`.

Decisions to log: where the lifted signals live, the exact classification of each phase, the movement key's contents,
where the dismissal ✕ sits, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-host-attention-list-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/host-attention-list/<nn>-<short-name>`
  (for example `plan/host-attention-list/01-attention-list` and `plan/host-attention-list/02-docs`).
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
passed its code review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
