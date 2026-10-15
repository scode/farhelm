# Keep host Settings usable during another host's Update

Written against main at 507dbcbb on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

In the Hosts panel, every host row's menu has a Settings item. Since #1932 it is disabled on **every** host whenever
**any** host has provisioning work in flight. That includes the whole run of a one-click Update, so a user who updates
one host cannot open another host's settings until that update finishes. The rule exists for one real hazard: while an
uninstall (or setup) confirmation is being prepared, opening Settings could put two modal dialogs on the page that make
each other inert, leaving the page unresponsive. Narrow the condition to that hazard.

### Acceptance criteria

- **Narrowed condition.** Settings on a host row is unavailable (as today: `aria-disabled`, and the click handler
  refuses) because of _another_ host only while some host has a confirmation in preparation or displayed that opens a
  modal dialog: Uninstall or ADD (setup) planning, or a pending Uninstall or ADD plan. Whatever opens that modal today
  counts, including the local row's automatic setup probe. A one-click Update (planning, waiting for the claim, and its
  tracked run), "update all", and any tracked run after confirmation no longer disable other hosts' Settings. The brief
  submission itself holds the page operation lock, which disables every row's Settings until the helm answers; that is
  separate and stays as it is.
- **The host doing the work keeps its own Settings unavailable**, for the whole time it is today. Its existing busy
  state covers that only once its work is submitted; while its own Update is planning or waiting for its claim, only the
  page-wide check covers it today. So each row (and `on_settings_start`, for that host) must also refuse on that row's
  own broad `planning` state, which each row already has. This plan only narrows what _other_ hosts' work does to a row.
- **Everything else that reads the broad "plan in flight" state is unchanged:** setup and update menu items, the
  "planning…" labels, rerun, `remote_update_available`, and whether the menu offers new provisioning commands. Publish
  the narrower fact separately rather than redefining the existing one.
- **The lockout stays covered.** `e2e/tests/provisioning.spec.ts` "Settings waits while an uninstall confirmation is
  being planned" still passes unchanged in what it asserts.
- **The regression is covered.** A browser test shows another host's Settings is enabled and opens while a one-click
  Update is in progress. The natural place is "single Update shows inline progress without opening its row" in the same
  spec, which already has a bystander host and holds the update, asserted after the update was accepted; or a new test
  beside it. The same test (or a neighbor) checks that the updating host's own Settings stays unavailable while its
  update is still planning. `ProvisioningMenuState::offered` only sees facts already reduced to booleans, so a unit test
  of it alone cannot tell an Update intent from an ADD plan; if the reduction that computes the new fact is easy to pull
  out into a small pure helper, do that and unit-test it near the existing `offered` tests; otherwise the browser tests
  are the coverage, and log which.
- **Comments and docs say the new rule.** The doc on the `HostRow` prop and the comment in `on_settings_start` (both in
  `crates/farhelm-ui/src/hosts.rs`) describe the narrowed condition. SPEC_impl.md gains a sentence where it describes
  the host actions menu and its modal confirmations, if nothing there already states when Settings waits; SPEC.md needs
  no change.
- **Queue bookkeeping.** `review_feedback_queue/host-update-blocks-all-host-settings.md` reports exactly this problem;
  remove it and its `review_feedback_queue/INDEX.md` entry in the same PR as the fix (root `AGENTS.md`,
  review_feedback_queue). The TODO.md entry is removed in the last PR.
- A changelog fragment (`kind: fixed`): another host's Settings stays usable while a host updates.

Out of scope: any other provisioning behavior, other dialogs' isolation, and changes to modal isolation itself.

## Requirement sources

**The user's request (2026-10-14):** plan this Near term TODO entry. Its text:

> **Narrow when a host's Settings is unavailable.** Since the UI layout and escaping fixes (#1932), a host's Settings
> menu item is disabled on every host while any host has setup work being planned, which includes the whole run of a
> one-click Update. The triaged decision was narrower: wait only while an uninstall confirmation is being prepared,
> because opening Settings for another host then could stack two dialogs and lock the page. Narrow the condition to the
> pending confirmations that can actually open a dialog, so Settings stays usable during an ordinary Update on another
> host, and keep the lockout case covered. The gap was noted when #1932 landed.

**The user's decisions (2026-10-14):** review gate is a single fresh-context gpt-6.1-sol reviewer at high effort, no
swarm; no-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** the triage outcome `delayed-provisioning-confirmations-make-two-dialogs-mutually.md`
in `TRIAGE_OUTCOMES.md` (its completion criteria: Settings cannot open while an uninstall plan is pending, so two
dialogs never lock each other); SPEC.md's maintainer-confirmed rule that confirming a dialog another covers confirms
that dialog (which accepts covered dialogs, not a mutual lockout); SPEC_impl.md's host actions menu and modal isolation
sections; `crates/farhelm-ui/src/modal_isolation.rs`, which explains why two isolated dialogs inert each other.

**Planner proposals:** counting ADD as well as Uninstall (the entry says "the pending confirmations that can actually
open a dialog", and both do), the separate published fact, the test placement, the changelog kind.

## Outline

All of it is in `crates/farhelm-ui`; find code by name, since line numbers drift.

- `provisioning.rs`: `ProvisioningMenuState` carries `planning`, set in `ProvisioningMenuState::offered(MenuFacts)` from
  `plan_in_flight`, which `ProvisioningPanel`'s menu effect computes as `is_planning || has_pending_plan || owned`
  (`owned` covers an Update intent in any phase and any live tracked run). Add a second fact, for example
  `confirmation_may_open`, computed from the panel's own `planning` signal (ADD and Uninstall only; Update never sets
  it) and a pending plan. Filtering the pending plan to ADD or Uninstall is optional: a pending Update plan only arises
  in a defensive branch the code documents as unreachable, so "planning or any pending plan" is simpler and errs on the
  safe side. Publish it beside `planning` without changing `planning`.
- `hosts.rs`: `HostsPanel` passes `provisioning_planning` as "any host's `planning`" to every row and checks the same in
  `on_settings_start`. Switch both to "any host's new fact, or this row's own `planning`". The busy host's
  `provisioning_busy_hosts` and `activity.busy` checks stay. Rename the row prop if its old name no longer fits.
- Tests: the browser assertions above, and the unit test of a pure helper if one falls out naturally (the tests of
  `offered` in `provisioning.rs`'s tests module are its neighbors).

Expected shape: one PR (`fix:`), with the queue removal and changelog fragment, removing the TODO.md entry too.

### Validation

Per root `AGENTS.md` "Finishing work", choose the smallest set that covers the change: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the `farhelm-ui` unit tests touched (through the recorder), the changed and
neighboring specs of `e2e/tests/provisioning.spec.ts` on Chromium and WebKit through the recorder (builds first, as root
`AGENTS.md` describes), `cargo check -p farhelm-ui --features desktop` if desktop-gated code is touched, the test-sleep
checker (`python -B scripts/check-test-sleeps.py`, per root `AGENTS.md`) since browser tests change, and
`python3 releasing/check-changelog.py format`.

## Fallbacks

Agreed fallbacks: if some other flow is found that opens a modal from the provisioning panel, count it in the new fact
and log it.

Examples of departures that need the scope reassessment: changing modal isolation, closing Settings when a confirmation
arrives instead of preventing it from opening, or changing what any other menu item does.

Decisions to log: the new fact's name and exact definition, where the regression test went, the SPEC_impl.md wording.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-settings-lockout-scope-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

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
a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/settings-lockout-scope/<nn>-<short-name>` (for example `plan/settings-lockout-scope/01-settings-scope`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `fix`. Add a changelog fragment (`kind: fixed`) in the
  same commit as the fix, per root `AGENTS.md`.
- The last PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section exists,
its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.
