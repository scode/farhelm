# A host permission to manage sessions on that same host without asking

Written against main at a24da7d0 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The maintainer's request, verbatim:

> use the planning system to make a plan that allows the expression of the permission for sessions on one host to launch
> any sessions on the same host without human approval - while ACCEPTING that the helm must be reachable. ignore the
> discussion about making it non-helm reliant for now. make it a new setting in host-settings (default false as todays
> behavior) that a user can opt into, and also make the approval card that shows when its request have a button for
> "always allow sessions on this host to launch/delete sessions on the same host" (when a request that matcdhes that
> pattern comes in)

The TODO.md entry this plan comes from (bucket "Near term") restates it:

> **Let a host's sessions manage sessions on that same host without asking.** A new host permission, off by default,
> lets agents in a host's sessions create, spawn, clone, rename, stop, restart and delete sessions on that same host
> without an approval card, while requests aimed at other hosts still ask. The approval card for such a request offers
> "Always allow on <host> only" to turn it on.

Why it exists: today the only way to stop being asked is the host's "Run farhelm commands without asking", which lets
that host's sessions act on every host in the fleet and write the templates every host launches from. The maintainer
wants to let one host's agents start and close sessions on their own host (for example, a dispatcher that launches one
session per planned task and deletes it afterwards) without granting fleet-wide authority. The helm stays the party that
decides; the earlier idea of letting the supervisor decide on its own is explicitly out of scope.

How it works today, from a read of main at a24da7d0 (line numbers approximate):

- **The decision** is `ask_with_answer` in `crates/farhelm-helm/src/approvals.rs` (around 405). It returns allowed
  without a card when the origin host's row has `commands_without_asking` and the action does not
  `requires_direct_allow()` (around 442). Otherwise it needs a connected GUI and parks on a card. Every acting agent
  verb reaches it through `approved(state, origin, asking, action)` in `crates/farhelm-helm/src/agent_requests.rs`
  (around 2500), called by each verb handler after it has resolved its target.
- **The card's answers** are handled by `answer_approval` (approvals.rs around 672). `ApprovalAnswer::AlwaysAllow`
  refuses a card that requires a direct Allow, checks the origin connection is live, then under
  `state.manager.host_write_lock(origin.host)` calls `HelmStore::allow_commands_for_identity` (or
  `allow_browser_for_identity` for links), which writes the setting only while the host row still holds the install
  identity that asked.
- **The card's types** are in `crates/farhelm-proto/src/approvals.rs`: `ApprovalAction` (whose `requires_direct_allow`
  lists every kind explicitly), `PendingApproval`, `ApprovalAnswer`.
- **The card's UI** is `crates/farhelm-ui/src/approvals.rs`: the action buttons (around 560-600) and
  `always_allow_tooltip` (around 419).
- **Host settings**: the store's host row (`crates/farhelm-helm/src/store.rs`, `commands_without_asking` around 946, its
  migration around 2427 and 3728, schema version 78 at around 339). There is no shared settings write: each permission
  has its own route, handler and locked helper (for this one, `POST /api/hosts/{id}/commands-without-asking`,
  `set_commands_without_asking` and `set_commands_without_asking_for` in `crates/farhelm-helm/src/hosts.rs` around
  751-790), plus a UI API function in `crates/farhelm-ui/src/api.rs`. The host view carries each field (hosts.rs around
  93 and 528, and the UI's host model), and the listing has an HTTP contract fixture,
  `crates/farhelm-helm/http-contract/host-list.json`. The dialog's Permissions section and `commands_help` are in
  `crates/farhelm-ui/src/hosts/settings_dialog.rs` (around 290-305 and 630-700).
- **Launch cards name the target host only by display name**, which is not unique, so whether a request stays on the
  asking host cannot be recovered from the card; only the verb handlers, which resolve the target host id (and a clone's
  source session) before asking, know it.
- **Identity adoption** resets all four host permissions to asking (SPEC.md around 252-256); find where the store does
  it and add the new one.
- **The YOLO rule** for agent launches is a separate check (`crates/farhelm-helm/src/yolo_guard.rs`) judged by the
  target host's "Start YOLO sessions without asking"; it stays as it is.
- **Spec text** to update: SPEC.md Topology's host settings list (around 236-262), "Agent-spawned sessions" (around
  3317-3590, in particular 3449-3466 on who asks and 3513-3552 on the card's answers), and "Local authority and trust
  between hosts" (around 4390-4420); SPEC_impl.md "Permission prompts for agent actions" (around 8466).
- **Other wording**: `crates/farhelm/src/agent_instructions.rs` (the text `farhelm agent instructions` prints, which
  says acting verbs wait for a card "unless the user told Farhelm not to ask for this host"), and the docs pages
  `website/src/content/docs/docs/using/approve-agent-requests.md`,
  `website/src/content/docs/docs/using/manage-hosts.mdx` and
  `website/src/content/docs/docs/how-it-works/security-model.md` (grep the docs for "without asking" for others).

### Acceptance criteria

- **The setting.** Every host has a new permission, "Manage sessions on this host without asking", off by default on
  every host, Tensorlake sandboxes included, and off for every existing host after the upgrade. It is stored with the
  host's other permissions in the helm, shown in the host listing, and written through the host settings the same way as
  the other switches. Adopting a new identity for a host resets it to off, like the other four.
- **What it covers.** With it on, an acting agent command from a session on that host is carried out without a card when
  it is one of these and stays on that host:
  - `farhelm agent create` whose target host is the asking session's host, with or without an initial prompt (a
    prompt-bearing create is a separate card kind, but it is a create), and `farhelm spawn` (always that host);
  - `farhelm agent clone` whose target host and whose source session's host are both the asking session's host;
  - `farhelm agent rename`, `stop`, `restart` and `delete` whose target session is on the asking session's host.

  "On the asking session's host" compares host ids the helm resolved, never display names and never anything the
  requesting supervisor claims.
- **What it does not cover.** Everything else asks exactly as today: any of the verbs above aimed at, or cloning from,
  another host; template writes and template deletes; host add, edit and remove; links (they have their own
  permissions); and `farhelm agent prompt` (turned off in this version; its call site passes "not covered", so
  re-enabling it later does not bring it under this setting unless that change decides so). Every kind that
  `requires_direct_allow()` (an agent-supplied wrapper, host changes) still needs its own Allow. The YOLO rule is
  untouched: a YOLO or command create, clone or spawn still needs the target host's "Start YOLO sessions without
  asking". A plain restart is exempt from the YOLO rule today (SPEC.md, Agent-spawned sessions), so a same-host restart
  of a YOLO session goes ahead without a card under this setting, exactly as it does under the fleet-wide setting today;
  SPEC.md and the docs must not suggest the YOLO switch gates restarts.
- **Relationship to the fleet-wide setting.** "Run farhelm commands without asking" is unchanged and continues to cover
  everything it covers today, including all of the above. The two switches are stored and changed independently.
- **The card button.** On a card whose request the new setting would cover, the card shows four buttons: Allow, "Always
  allow on <host> only", "Always allow from <host>" (the existing fleet-wide one), Deny, where <host> is the requesting
  host's display name shown as peer text the way the existing button shows it. Its tooltip says: "Always allow: let
  agents in <host>'s sessions start, stop, restart, rename and delete sessions on <host> without asking. Requests for
  other hosts still ask." Cards that do not qualify, and cards that need a direct Allow, do not show it. Choosing it
  turns the new setting on for the requesting host (only while that host is still the install that asked, as the
  existing Always allow does) and approves that request. The helm refuses that answer on a card that does not qualify,
  leaving the card answerable.
- **The settings switch.** The host settings dialog's Permissions section shows the switch "Manage sessions on this host
  without asking", placed between "Start YOLO sessions without asking" and "Run farhelm commands without asking", with
  help text for each state. The help names what it covers (start, stop, restart, rename, clone and delete sessions on
  this same host), what still asks (requests for other hosts, template changes, host changes), and that "Run farhelm
  commands without asking" already includes it. When the fleet-wide switch is on, the new switch still shows and changes
  its own stored value.
- **Compatibility.** The card change follows SPEC.md's upgrade-compatibility rules: an older UI that does not know the
  new eligibility field shows the card as it does today, without the new button; the new answer is only sent by a UI
  that shows it.
- **Wording.** SPEC.md, SPEC_impl.md, the agent instructions text and the docs pages above describe the new permission,
  what it covers, and the new button.
- **Tests:** helm tests of the decision for each covered verb with the target on the asking host and on another host
  (and clone with a source on another host), with the setting on and off, with the fleet-wide setting on; that
  direct-Allow kinds and template writes still ask with the setting on; that the YOLO rule still refuses with the
  setting on; that identity adoption resets it; that the new answer writes the setting under the identity check,
  approves the request, and is refused on a non-qualifying card; the host listing's contract fixture; UI tests for the
  switch and for the button's presence and absence; and a browser test that approves a same-host request with the new
  button and sees the next same-host request go through without a card, on Chromium and WebKit.
- One changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: added`),
  added by the first `feat` PR and edited by the later ones (see Outline).
- The last code PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08, on the planning page:

- **Off by default** on every host ("default false as todays behavior").
- **The helm still decides and must be reachable**; making the supervisor decide is out of scope ("ignore the discussion
  about making it non-helm reliant for now").
- **Covered actions**: every session action on the asking session's own host: create, spawn, clone, rename, stop,
  restart, delete.
- **Clone** is covered only when its source session is on the same host too; otherwise the card asks.
- **The button** is added beside the existing "Always allow from <host>" (four buttons), only on qualifying cards.
- **Button label**: "Always allow on <host> only".
- **Switch label**: "Manage sessions on this host without asking".
- **Overlap**: when "Run farhelm commands without asking" is on, the new switch keeps showing and changing its own
  stored value; its help text says the fleet-wide switch already covers it.
- **Stated on the planning page and not objected to**: YOLO and command launches still need "Start YOLO sessions without
  asking"; agent-supplied wrappers still need a per-request Allow; host changes, template writes and links never fall
  under this setting; requests for other hosts ask as today; the fleet-wide setting is unchanged; the permission is tied
  to the host's identity like the other four.
- **Found by the planning review after the questions were settled**, and following from existing rules rather than new
  product decisions: a prompt-bearing create counts as a create; template deletes are excluded like template writes;
  `prompt` stays outside the setting; a same-host restart of a YOLO session needs no card, as the YOLO rule already
  exempts plain restarts; the new answer is its own variant so an older helm cannot read it as the fleet-wide grant.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the request quoted above and the decisions above.

**Binding repository constraints:** SPEC.md's Topology host permissions (around 236-262), "Agent-spawned sessions"
(around 3317-3590), "Local authority and trust between hosts" (around 4390-4420) and "Upgrade compatibility and client
scale"; SPEC_impl.md "Permission prompts for agent actions"; the explicit listing in `requires_direct_allow` (a new card
kind or flag must decide there rather than inherit a default); root `AGENTS.md`, "Code rules", "Tests and validation",
"Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", and "The live install is off-limits" (tests make
their own helms; never touch the maintainer's installation); `docs/desktop-web-triage.md` for UI work;
`website/AGENTS.md` for the docs pages; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and
`scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- The verb handlers, which already resolve their target before asking, pass `approved` a plain boolean saying whether
  the request stays on the origin host (target host id, a target session's host id, and a clone's source host id
  compared with `origin.host`). Only the seven covered call sites can set it; every other caller, `prompt` included,
  passes false. The decision in `ask_with_answer` becomes: allowed without a card when the action does not require a
  direct Allow and either the fleet-wide setting is on, or the new setting is on and the request stays on the origin
  host.
- The waiting request's entry stores that boolean, and `PendingApproval` exposes it as a serde-defaulted field saying
  the card qualifies for the new answer. It is never set on a card that requires a direct Allow, so the UI can show the
  button from the field alone, and the answer handler checks the same stored bit the decision used rather than
  recomputing it.
- The choice is a new `ApprovalAnswer` variant, not a field on the existing `AlwaysAllow`: an older helm would ignore an
  unknown field and grant the fleet-wide setting, while an unknown variant makes it reject the answer.
- The new column follows the pattern of `commands_without_asking`: one migration with an idempotent column check, its
  own per-field route, handler and locked helper beside `set_commands_without_asking`, the UI API function, and the
  field in the helm's and the UI's host views and the contract fixture. For the identity-checked write, prefer
  generalizing the existing writers (as `allow_browser_for_identity` takes a permission enum) over a fourth near-copy.

## Outline

Four PRs:

1. `plan/own-host-sessions-without-asking/01-permission`: the store column and migration, the identity reset, the host
   listing field and contract fixture, the per-field write route, the decision with each verb handler's same-host fact,
   and SPEC.md and SPEC_impl.md, with the helm tests of the decision matrix. It adds the changelog fragment, in the same
   commit, as root `AGENTS.md`, "Changelog fragments", requires of a `feat`.
2. `plan/own-host-sessions-without-asking/02-settings-switch`: the host settings switch and its help text, with UI
   tests. It edits PR 1's fragment rather than adding a second one (a `feat` amending unreleased work).
3. `plan/own-host-sessions-without-asking/03-card-button`: the card's eligibility field, the new answer and its handling
   under the identity check, the button and tooltip, with helm and UI tests and the browser test. It edits the same
   fragment.
4. `plan/own-host-sessions-without-asking/04-docs`: the agent instructions text, the docs pages and the TODO.md removal,
   with no fragment of its own. Fold the docs into earlier PRs instead if they read better with the behavior.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy, the
helm crate's approvals, agent request, hosts and store tests, the HTTP contract tests, the UI crate's approvals and host
settings tests, the e2e tests that exercise agent approvals (`crates/farhelm/tests/e2e/`, through
`python3 scripts/record-test-run.py` with the pinned nextest and tmux setup), `python -B scripts/check-test-sleeps.py`
with the interpreter `docs/test-sleep-check.md` prepares, and the browser specs for approval cards and host settings on
both engines. Build the website (`cd website && bun install --frozen-lockfile && bun run build`) for the docs changes,
and follow `website/AGENTS.md` for docs edits (an unattended run starts no preview server and names the changed pages).

## Fallbacks

Examples of departures that need the scope reassessment: covering template writes, links or host changes; letting the
new setting waive the YOLO rule or a direct Allow; a supervisor-side decision; a new card kind instead of a field on the
existing cards; changing what "Run farhelm commands without asking" covers.

Decisions to log: how the same-host fact is computed for each verb, how the card's eligibility is carried, the migration
shape, every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-own-host-sessions-without-asking-log.md` in the parent directory of the checkout you run in, derived as
that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
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
  `plan/own-host-sessions-without-asking/<nn>-<short-name>`, as the outline names them.
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
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
