# Agents: add, edit and remove hosts for the user, always with approval

Written against main at 71dd7eba on 2026-10-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `alias-in-add-host` has landed. That plan gives the add host dialog an optional alias field,
carries the alias through the helm's add requests, and makes adding a destination that is already a host an error. Build
on it; do not re-implement any of it. `dialog-saves-oct22`, queued earlier, edits the help text of the host setting "run
farhelm commands from this host without asking", which this plan also changes; expect to rebase across it and keep both
changes.

## The goal

Agents running in Farhelm sessions use `farhelm agent ...` commands (`crates/farhelm/src/main.rs`, relayed to the helm
through `crates/farhelm-helm/src/agent_requests.rs`). They can list hosts (`farhelm agent hosts`) but cannot add, edit
or remove one. The maintainer wants to be able to tell an agent "$farhelm add host foobar.bla.com with alias baz".

Deliver three agent commands, each needing the user's explicit approval in the GUI every time, none ever approved
automatically:

1. **`farhelm agent host add <destination> [--alias <name>]`.** The request is listed like any other agent request. In
   the focused window it opens the GUI's own add host dialog, prefilled with the destination and alias and showing which
   session and host asked; other windows show an ordinary card whose action opens that dialog in its window. Pressing
   add in the dialog answers the request as Allow, and cancel as Deny, through the existing approval answer route, which
   settles it in every window. After add, the window that answered runs the ordinary add flow with whatever the dialog
   then holds: the same checks, the same refusals (including `alias-in-add-host`'s duplicate refusal and alias rules),
   the same setup confirmation when Farhelm must be installed, and the same "ssh needs 2FA?" guidance. The command
   reports to the agent as soon as the user answers: "accepted; Farhelm is adding <destination>" or "declined", or the
   ordinary no-GUI and not-answered refusals.
2. **`farhelm agent host edit <host> [--alias <name>|--no-alias] [--icon <icon>] [--color <color>]`.** Only the alias,
   icon and color; the verb has no fields for anything else. An Allow/Deny card shows each changed field's old and new
   value.
3. **`farhelm agent host remove <host>`.** An Allow/Deny card that repeats the GUI remove dialog's own words ("Farhelm
   will forget this host. Its supervisor and sessions keep running, and adding the destination again finds them.").
   Allowing it does exactly what the GUI's remove does. Removing the asking session's own host, or this machine, is
   refused without a card.

### Acceptance criteria

- The three commands exist with those names and flags, appear in `farhelm agent help` and in the text
  `farhelm agent instructions` prints (`crates/farhelm/src/agent_instructions.rs`), and `<host>` is resolved the way
  `--host` already is for `farhelm agent create`.
- **"Accepted" is documented as not meaning installed.** The command's own output, its help text and the agent
  instructions all say plainly that an accepted add means the user approved and Farhelm has started adding the host; it
  may still fail, need setup, or be cancelled at the setup question; and the agent checks `farhelm agent hosts` (and the
  host's state there) before relying on the host.
- **Approval is always explicit.** The per-host setting "run farhelm commands from this host without asking" never
  covers these commands. No Always allow is offered. Reuse the existing "always asks" mechanism for agent-wrapper
  launches (`ApprovalAction::requires_direct_allow` in `crates/farhelm-proto/src/approvals.rs`, honored by
  `crates/farhelm-helm/src/approvals.rs` and `crates/farhelm-ui/src/approvals.rs`), with new action kinds whose tags an
  older UI does not recognize, so it fails closed. The mechanism's docstring and the helm's Always allow refusal text,
  which today talk only about wrappers, are made general (or worded per kind) so a host card never says "wrapper".
- **The user's own confirmation-skipping settings do not apply to agent requests.** In code this is one branch: an add
  flow started from an agent request always shows the setup confirmation when Farhelm must be installed, whatever the
  skip-setup-confirmation preference says. An agent remove is an Allow/Deny card and never reaches the GUI's remove
  dialog, so "remove hosts without asking" needs no code. SPEC.md must state the rule and why, in the maintainer's
  framing: those settings exist to spare the user confirmation clicks on actions the user started in the UI; they are
  about avoiding accidental clicks, not about permissions, and an agent's request is inherently different. The spec
  keeps the two concepts distinct by rule, not by an exception list. Do not build a general "agent origin bypasses
  preferences" mechanism.
- **Edit** writes through the same routes as the settings dialog, after approval. **Remove** goes through the same route
  as the GUI's remove, after approval; refusing this machine reuses the route's existing refusal for the local row
  rather than a second rule.
- SPEC.md "Agent-spawned sessions" lists the new verbs, their approval rule and what "accepted" means for add; the
  security passage on what "without asking" grants says host changes are never covered; the host settings dialog's help
  text for "run farhelm commands from this host without asking" (`commands_help`,
  `crates/farhelm-ui/src/hosts/settings_dialog.rs`) says the same. The relay's design comment in
  `crates/farhelm-helm/src/agent_requests.rs` ("a supervisor cannot, for example, change a host's settings through this
  relay") and SPEC_impl.md's matching passage are rewritten to say host changes now happen only on the user's
  per-request approval, never on the connection's authority.
- The agent docs page that lists `farhelm agent` commands documents the three commands, if one exists.
- Tests: helm tests for each verb's approval path (never skipped by "without asking", no Always allow, refused without a
  GUI, refused for the asking session's own host and this machine on remove); browser tests for the add request (the
  dialog prefilled in the focused window, the card elsewhere, add answering Allow, cancel answering Deny, the setup
  confirmation shown despite the skip preference), the edit card and the remove card; an end-to-end test that a real
  `farhelm agent host add` reports accepted and declined.
- A `feat:` changelog fragment.
- The last code PR removes the TODO.md entry "Allow agents to add a host for the user." (Near term).

## Decisions already made

Settled with the maintainer on 2026-10-22, on the planning page:

- **Add uses the prefilled add host dialog**, not a plain approval card. The maintainer: "for now lets make it so this
  cannot be auto-approved. we likely want another level of approval here, this is expanding the blast radius of what an
  agent can do in surprising ways."
- **No extra approval level for now.** Asked what "another level" meant, the maintainer: "let's leave this out of scope
  for now. for now, you simply always have to approve." So: no app-wide opt-in switch; every request needs approval.
- **Scope: add, edit and delete.** The maintainer: "add, edit, delete (all must be approved, none allow auto-approve)".
- **Edit covers alias, icon and color only.** Never the four permissions (an agent could grant itself more), never the
  destination (changing a host's destination keeps its settings, permissions included, so it would move the host's trust
  to a machine the agent chose; SPEC.md's retargeting passage).
- **Edit and remove use Allow/Deny cards** that show the change.
- **Confirmation-skipping settings do not apply to agent requests, and the spec says why.** The maintainer: "this must
  be clearly speced. It's not really "ignored"; the intended meaning of those settings have to do with the UI, it's not
  about permissions it's about avoiding accidental clicks. It's inherently different and must remain so by spec."
- **Off-limits:** removing the asking session's own host (it would cut the connection mid-request, the same reason an
  agent cannot delete its own session) or this machine. Editing is allowed on any host.
- **Names:** `farhelm agent host add | edit | remove`, matching `template create | edit | delete` beside the `templates`
  listing, and the GUI's word "remove".
- **Add reports accepted or declined, not the final outcome** (decided after the planning review showed that following
  the client-side add flow back to the agent's request needs new machinery). The maintainer added: "but clearly
  documented to the agent that add success does not mean it is successfully installed".
- **The dialog opens in the focused window only**; other windows show a card that opens it there, so two windows never
  run the same add.
- **Reviewer:** gpt-6.1-sol at high effort (see Review gate). If it cannot be reached when a review is due (CLI missing,
  quota, outage) and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
  "Executing one plan" (step 10), after pushing what is consistent. No substitute reviewer.
- No-workhorse mode.

## Requirement sources

**The user's request:** the TODO.md entry "Allow agents to add a host for the user.", the maintainer's "I want to be
able to say "$farhelm add host foobar.bla.com with alias baz"", and the decisions above.

**Binding repository constraints:** SPEC.md "Agent-spawned sessions" (acting verbs need approval in the GUI; the
per-host "without asking" setting; the agent-wrapper rule that always shows Allow/Deny; the approval region's placement,
ordering and answer behavior); SPEC.md's host passages (adding is discovery-first with a setup confirmation; removing
forgets the host; retargeting keeps settings); SPEC.md's security passage on what a remote host must not gain;
SPEC_impl.md "What the helm believes from a supervisor"; root `AGENTS.md`, "Tests and validation", "Conventional
Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md` for tests; `website/AGENTS.md`
before editing any docs page.

**Planner proposals** (from a read of main at 71dd7eba, revised after a planning review; change them if the code says
otherwise, logged as a DECISION):

- New `AgentVerb` variants in `crates/farhelm-proto/src/lib.rs` and new `ApprovalAction` kinds (host add, host edit,
  host remove) returning true from `requires_direct_allow`, each with its own tag.
- Add is an ordinary held approval whose card, in the focused window, opens the prefilled add host dialog; the helm's
  Allow does nothing beyond settling the request, since the add itself goes through the existing probe and provision
  routes on the user's authority. No correlation between the agent's request and the probe or provision calls.
- No agent command for uninstalling Farhelm from a host, re-running setup, or changing permissions or the destination.

## Outline

A suggested stack; reshape it if the code says otherwise:

1. `feat:` the protocol and helm side of `host edit` and `host remove`, with their cards' action kinds, the never-auto
   rule, the off-limits refusals, and helm tests; SPEC.md, SPEC_impl.md and the relay comment.
2. `feat:` the edit and remove cards in the UI, with browser tests.
3. `feat:` `host add`: the held request, the prefilled dialog in the focused window and the card elsewhere, add and
   cancel answering the request, the setup confirmation always shown for agent requests, and the accepted or declined
   report; helm, browser and end-to-end tests.
4. Agent instructions, help text and docs page (including what "accepted" means), within the PRs above or as a final
   `docs:` PR; changelog fragment; TODO.md entry removed.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` (including the UI's web target), the proto, helm, CLI and UI unit tests
through `python3 scripts/record-test-run.py`, narrowly selected browser specs (approvals, adding hosts, host settings,
removing hosts) and the agent-request end-to-end tests that cover approvals.

## Fallbacks

Examples of departures that need the scope reassessment: a parallel request-holding system beside approvals, reporting
the add's final outcome to the agent, an app-wide opt-in switch (the maintainer put it out of scope), letting agents
change a destination or permissions, an agent command for uninstall or setup, or adding hosts without the GUI.

Decisions to log: how the add request opens the dialog and which window counts as focused, each new action kind's tag,
and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-agent-add-host-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewers included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/agent-add-host/<nn>-<short-name>` (for
  example `plan/agent-add-host/01-host-edit-remove`).
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit for `feat` and `fix`.
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
`$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch mechanics). If
gpt-6.1-sol cannot be reached, block as Decisions already made says; never substitute another model.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
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
