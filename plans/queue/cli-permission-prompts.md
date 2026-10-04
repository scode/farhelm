# Permission prompts for `farhelm` CLI actions: the helm asks the user before an agent acts on the fleet

Written against main at b4cfe3a7 on 2026-10-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file directs a change to them (the spec PR below).

This plan runs after `plans/queue/launch-representation.md` (its `INDEX.md` line says so). That plan removes profiles,
introduces launch kinds (agent launches with structured choices, command launches with a YOLO assertion the caller makes
and Farhelm does not check) and named launch templates, replaces spawn's profile resolution with template and launch
resolution through the helm, and changes the agent CLI's flags. Everything below assumes its result: read what it
landed, including its SPEC.md, before starting. Names in this file come from main at b4cfe3a7 and some will have moved.

## The goal

Today an agent inside a Farhelm session can act on the whole fleet through the `farhelm` command: `farhelm spawn` makes
a session on its own host, and `farhelm agent create`, `clone`, `rename`, `stop` and `restart` reach every host through
the helm. Nothing asks the user. SPEC.md accepts agent-requested create and clone on another host as a temporary
exception to the rule that one host must not gain execution on another (SPEC.md, "Local authority and trust between
hosts"), and the YOLO confirmation is honour-system for agents: `--confirm-yolo` is a flag the agent passes itself.

After this plan, the helm asks the user before it carries out any acting verb an agent (or a person typing in a
session's shell, which the helm cannot tell apart) requests, unless the user has told it to stop asking for the
requesting host. That closes the one known way one host can cause execution on another, and SPEC.md's exception goes
away with it.

Acceptance criteria:

- Which requests ask:
  - Every acting verb asks: `farhelm spawn` in every form (including `--inherit-agent`), `farhelm agent create`,
    `clone`, `rename`, `stop`, `restart`, and the new template write verbs below. That includes an agent acting on its
    own session.
  - Read-only verbs never ask: the hosts, sessions and templates listings and `farhelm agent instructions`. Internal
    session traffic that is not a CLI action (conversation identity reports from hooks, and the like) is untouched; the
    gate is per verb, not a blanket rule on session-authenticated requests.
  - Requests from a host whose new setting "run farhelm commands from this host without asking" is on do not ask.
- The helm decides. The supervisor and everything on the requesting host are untrusted, so the approval check, the
  per-host setting, and the YOLO refusals below all live in the helm, keyed by the host connection the request arrived
  on, never by anything the supervisor claims. Spawn is the one action carried out by the requesting host's own
  supervisor: a compromised supervisor could skip asking, and that is accepted because spawn only acts on its own host,
  which the threat model already trusts (SPEC.md "Local authority and trust between hosts"). The spec says so
  explicitly.
- Waiting:
  - If no GUI is connected to the helm when a request needs approval, it is refused at once with a message saying no
    Farhelm window is open to ask the user, and to open Farhelm and retry.
  - Otherwise the request waits for the user's answer for at most 9 minutes, then is refused as not answered and the
    prompt disappears. (Nine, not ten, so that an agent whose shell tool is capped at ten minutes, as Claude Code's is,
    still sees the answer.)
  - Declined, not answered, and no GUI are distinct, readable messages on the CLI's stderr with a nonzero exit.
  - While waiting, the CLI itself prints one line to stderr after a couple of seconds without an answer, saying it is
    waiting for the user to approve the request in Farhelm. This is local to the CLI; no interim message travels the
    relay.
- The prompt, in the GUI:
  - A non-modal card in a fixed corner of the window, staying until answered or expired, stacking when several are
    pending. The rest of the app stays usable.
  - It names the requesting session and its host, the action, and its target. For a launch it shows the target host,
    directory, the agent and its choices or the full command text, and whether it is YOLO. For a template write it shows
    the full resulting definition, including command and resume-command text and the YOLO assertion, even though the
    templates listing hides that text from agents.
  - Agent-controlled text (session titles, directories, command text) is rendered as plain, labelled data, so it cannot
    pass for Farhelm's own wording.
  - Buttons: Allow, Always allow from <host>, Deny. "Always allow" turns the host's setting on, then approves this
    request, the way the YOLO confirmation's "stop asking for this host" already works.
- The per-host setting:
  - One switch per host, "run farhelm commands from this host without asking" (final wording is yours), in that host's
    settings dialog next to the YOLO setting. It covers every acting verb from that host.
  - Every host starts asking, including hosts that existed before the setting did. Adopting a new identity for a host
    resets it to asking, exactly as it resets the YOLO setting.
  - It persists across helm restarts.
- YOLO, for agent-originated requests:
  - On a host that asks before YOLO launches, an agent may not start anything that runs differently from what the user
    already approved. In the maintainer's words: "only the options that don't allow the agent to do anything different
    than what the user already approved, are ok to permit in this case."
  - Concretely, on such a host the helm REFUSES (does not prompt for) an agent's create, clone, spawn (including
    `--inherit-agent`) or Restart-with-changes when the resulting launch is YOLO, or when it is a command launch
    whatever its assertion says, because Farhelm cannot check the assertion. The refusal names the host and says that
    turning on "start YOLO sessions here without asking" for it is how to let agents do this.
  - A plain restart of an existing session re-runs exactly the launch the user already approved, so it is allowed and
    gets the ordinary prompt.
  - Agent launches whose permission is not YOLO get the ordinary prompt.
  - `--confirm-yolo` (and its old alias `--allow-yolo-on-sensitive-host`) is no longer honoured from the agent CLI. The
    helm ignores or refuses a `confirm_yolo` on every agent-originated request, whatever an old supervisor or a modified
    CLI sends; the CLI refusing the flag with a message is only a courtesy.
- Template writes from agents: `farhelm agent template create`, `edit` and `delete` (names may follow the template CLI
  launch-representation landed), behind the same prompt. See P6 for their shape.
- The safety details the planning review found:
  - After the user approves, the helm re-runs its existing check that the requesting host's connection is still the
    current one (`origin_is_live` in `crates/farhelm-helm/src/agent_requests.rs`) before acting, so a host replaced or
    re-identified during the wait does not inherit the approval.
  - Deleting a session first denies that session's pending prompts. Without this, the delete parks on the asking
    session's request lock until the prompt is answered or expires.
  - A host may have at most as many pending prompts as the helm's existing per-host limit on agent requests in flight
    (`AGENT_ANSWER_SLOTS`, 4, in `crates/farhelm-helm/src/client.rs`). No separate cap. While four are pending, that
    host's other agent requests, listings included, are refused with the existing "too many in flight" message. That is
    accepted; document it in SPEC_impl.md.
- SPEC.md:
  - Agent-spawned sessions: the consent rule, spawn needing a helm, the YOLO rule above, the template write verbs.
  - Local authority and trust between hosts: the temporary create/clone execution exception and its retry-record
    acceptance are removed, and cross-host stop, rename and restart are no longer free "bounded" operations: every
    acting verb needs the user's approval or the requesting host's setting. Keep the rule against adding cross-host
    execution by analogy.
  - The read-side acceptances that currently hang off that exception (any host may obtain resolved launches, now
    templates; command lines are not secret from agents; fleet listings) stay, re-anchored to the narrowed TODO.md entry
    instead of the removed exception. The Remote input section's references move the same way.
  - Topology's host settings paragraph gains the new setting.
  - The Creation section's YOLO paragraph drops `--confirm-yolo` for agent requests and points at the agent rule.
- TODO.md: the Maybe later entry "Close the cross-host execution hole in agent-requested session creation and cloning"
  is narrowed to what remains open, the read side (fleet listings and launch content readable by any attached host), in
  the spec PR. The Near term entry "Permission prompts for actions requested through the `farhelm` CLI" is removed by
  the last PR.
- SPEC_impl.md describes what was built: the pending-approval table and its lifetime, how "a GUI is connected" is
  determined, the relay's longer answer budget for verbs that may wait for the user, the per-host column and its reset,
  the slot consequence above, and the delete and origin-recheck rules.
- `farhelm agent instructions` explains the prompts: gated verbs may wait up to 9 minutes for the user, so run them with
  a long tool timeout, pass an idempotency key on creates so a retry after a killed tool call does not ask again for a
  second session, and what each refusal means.
- The docs website documents the prompt, the host setting, and the agent YOLO rule.
- Each user-visible PR carries a changelog fragment. Spawn refusing without a helm and `--confirm-yolo` going away are
  breaking for scripts, so the PR that does that is `feat!`.

## Requirement sources

Kept apart so a reviewer can challenge the planner's reading rather than merely check compliance with it.

### The original request

The TODO.md Near term entry, verbatim: "Every action an agent or a user attempts through the `farhelm` tool, against any
host or session, should ask the user for permission first, with an "always allow when coming from this host" option.
Model it on the per-host setting for starting YOLO sessions without asking: ask by default, and let the user turn asking
off for a host. Related to the Maybe later entry on closing the cross-host execution hole in agent-requested session
creation and cloning. Two things are required parts of this work, from the launch-kinds redesign
(`plans/queue/launch-representation.md`): until it lands, a command launch's YOLO assertion is trusted even from an
agent, so an agent can start a YOLO command on a host that asks before YOLO launches by asserting that it is not YOLO,
and with this work an agent's command launch on such a host must ask whatever its assertion says; and agents may apply
launch templates but not create, edit or delete them, and this work is what allows template writes from agents, behind
the same prompt."

### The maintainer's decisions (planning session, 2026-10-03)

- M1. Acting verbs only ask; listings and instructions never do. Self-actions ask. "The intent is that this closes the
  one known way one host can affect another host, and the SPEC should change with it (it has an exemption)."
- M2. Spawn asks too, through the helm. With no helm attached it is refused, `--inherit-agent` included.
- M3. No GUI open: refuse at once. Otherwise wait, at most 9 minutes (first 10, shortened to 9 so an agent at Claude
  Code's 10-minute shell cap sees the answer).
- M4. Close the execution side of the cross-host hole: remove the spec's exception, narrow the Maybe later entry to the
  read side.
- M5. Prompt UI: non-modal card, as above.
- M6. "Always allow" is per requesting host and covers all verbs; the switch lives in the host settings dialog next to
  the YOLO setting; reset on identity adoption.
- M7. "Let's not allow agents to yolo on a host that isn't marked 'always accept yolo w/o asking', for now." Command
  launches by agents on such a host are refused whatever they assert, since Farhelm cannot know whether they are YOLO.
  This replaces the TODO entry's "must ask whatever its assertion says" with "refused".
- M8. "Allow pure restart but don't allow 'start with' - that changes arguments. Only the options that don't allow the
  agent to do anything different than what the user already approved, are ok to permit in this case." The planner
  applied this to spawn `--inherit-agent` and clone too: both start a new session (new directory, or new host) and are
  refused on such a host when the launch is YOLO or a command launch.
- M9. "The HELM needs to make the final call here since the host is untrusted (supervisor included)."
- M10. Review gate: a fresh-context Opus 5.5 agent at high effort AND a gpt-6-astra agent at high effort, both with the
  general charter, no review swarm.
- M11. No-workhorse mode.

### Repository constraints

- R1. SPEC.md "One GUI at a time": the CLI is a fully supported surface alongside an open GUI; operations it performs
  concurrently with a GUI must behave correctly.
- R2. SPEC.md "Local authority and trust between hosts": a remote host must not gain unauthorized execution on another
  host through Farhelm; the helm and GUI treat remote supervisor messages and agent-controlled output as untrusted.
- R3. Root `AGENTS.md` "Harness-specific code": nothing here should need per-harness branching; if it seems to, follow
  the map in `crates/farhelm-supervisor/src/agent_kind/mod.rs`.
- R4. TODO.md's code cleanup bucket lists test hooks in production code as a smell. Tests must drive the real prompt
  (answer it through the GUI's endpoint, or turn the host setting on), never through a production bypass.

### Planner proposals, adopted after the planning review

- P1. Pending approvals live in memory in the helm. A helm restart drops them, and the waiting requests then fail the
  way any request does when the helm goes away. The GUI reads them through an ordinary REST listing and learns of
  changes through the existing fleet invalidation feed (`crates/farhelm-helm/src/feed.rs`); no new event channel.
  Answering is one REST call.
- P2. "A GUI is connected" means at least one subscriber to the helm's event feed. Verify that a desktop app whose
  window is closed but which is still running does not hold such a subscription; if it does, find the smallest way for
  the helm to tell that a GUI is showing, and if there is none, block (Unattended fallback).
- P3. The relay keeps its machinery. Add a "may wait for the user" classification on agent verbs beside the existing
  `is_mutating`, read by both sides, with an answer budget of the helm's 9-minute expiry plus the ordinary 30 s
  (`AGENT_UPCALL_TIMEOUT` in `crates/farhelm-supervisor/src/service/core.rs`), so the helm's own expiry answer always
  arrives first. Do not raise the budget globally: read-only listings and the queue deadline for a second change from
  the same session keep the short budget, so a retried change is still refused quickly with the existing "an earlier
  change requested by this session is still in progress" message (reword it to mention a pending approval).
- P4. Spawn needs no new upcall. After launch-representation, every spawn except `--inherit-agent` already asks the helm
  to resolve its launch through the agent relay; the approval happens inside the helm's handling of that request. Add
  one variant of it for `--inherit-agent` carrying the parent's stored launch, so the card and the YOLO rule have it.
  The existing ordering (the upcall outside the parent lifecycle claim, the credential recheck inside) already makes a
  spawn whose parent was deleted during the wait fail honestly.
- P5. No withdrawing a prompt when the CLI disconnects. An approval that arrives after the CLI was killed carries out
  exactly what the card showed; idempotency keys, the per-session change lock and the expiry bound the rest.
- P6. Template write verbs:
  - Reuse `farhelm agent create`'s launch flags for the fields, and resolve `--host` by display name at write time, as
    acting commands already do.
  - Refuse fresh-checkout fields, which the CLI already refuses to apply.
  - Edit sets only the fields given and offers no way to unset one. The listing hides command text, so an agent cannot
    round-trip a template it did not write, and a whole-replacement edit would silently drop that text. To drop a field,
    delete and recreate.
  - Write through the same code path as the GUI's Templates panel, the way the existing agent verbs share their REST
    counterparts' paths (`agent_requests.rs` module docs).

## Implementation outline

Moderate to large, spread over every component, but mostly extensions of existing paths. The new pieces are a small
in-memory pending-approval table in the helm with a REST listing and answer call, one host column with its setter and
reset (a sibling of `yolo_without_asking` in `crates/farhelm-helm/src/store.rs` and `hosts.rs`), a per-verb gate in the
helm's agent request handler, the YOLO refusals in `crates/farhelm-helm/src/yolo_guard.rs`, one verb classification and
budget in the relay, one spawn resolve variant, three template verbs, and the card and switch in the UI. No new event
channel, no new upcall type, no withdraw protocol, no fence restructuring, no pending-prompt cap beyond the existing
slots.

The riskiest part is the relay timing (P3): the supervisor's relay treats an answer timeout as "delivered, outcome
unknown" and retains the asking session's change lock, so get the budgets right before anything else and test a
deliberately slow approval end to end.

Existing tests that drive agent verbs will start waiting for approval. Fix them by turning the requesting host's new
setting on in their fixtures, or by answering the prompt through the REST call, per R4.

Suggested stack, bottom up (shape it differently if review would be helped, without churn). The gate must not land below
the GUI cards: with a GUI open and no cards, every agent action would wait 9 minutes for an answer nobody can give.

1. `docs`: SPEC.md changes and the Maybe later entry narrowing.
2. `feat`: the helm's pending-approval table, expiry, REST listing and answer call, the GUI-connected check, and the
   host setting's storage, setter and identity reset, with tests. Nothing asks yet.
3. `feat`: the GUI cards and the host settings switch, with a browser spec on Chromium and WebKit.
4. `feat!`: the gate. Relay classification and budget, the helm's per-verb gate with the origin recheck, delete denying
   pending prompts, the YOLO refusals and `confirm_yolo` no longer honoured, spawn's approval including the inherit
   variant and the no-helm refusal, the CLI's waiting line and messages, `farhelm agent instructions`. If this is too
   big for one review, split spawn into its own PR above it.
5. `feat`: template create, edit and delete for agents.
6. `docs`: SPEC_impl.md, the docs website, and removal of the TODO.md Near term entry.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-cli-permission-prompts-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/cli-permission-prompts/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits; `feat` PRs carry a changelog fragment under `releasing/changelog.d/` in the same commit, per
  root `AGENTS.md` (Releases and the changelog), with `kind: none` and a reason on the ones that are not user-visible on
  their own; validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).
- The last PR removes the TODO.md Near term entry "Permission prompts for actions requested through the `farhelm` CLI".
- If `plans/queue/hover-help.md` has landed by the time you build the cards, every control you add follows its tooltip
  rule (its browser test fails on a control without one). If it has not, use whatever the surrounding UI does today.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, nextest selections of
the helm's agent request, YOLO guard, host and store modules, the supervisor's relay and spawn handlers, the CLI, and
the e2e tests that drive agent verbs and spawn, `python -B scripts/check-test-sleeps.py` (a deliberately slow approval
in a test needs a readiness oracle, not a sleep), `dprint check` on changed files, the website build when docs change,
and the new browser spec plus the host settings specs on Chromium and WebKit through the recorder. Say in the report
which checks ran and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands (M10): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra agent at
high effort (shelled out to the other harness if the executing one cannot reach that model natively). No review swarm.
Both get the same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Requirement sources, and the
outline above. For the gate PR, ask both reviewers specifically whether any path lets a request act without the helm's
approval, and whether any agent-originated launch on a host that asks before YOLO launches can run something other than
what the user already approved. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim. Address what both reviewers find before moving on, and log the DECISION where you
decline a finding. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new event channel or upcall type, a withdraw or
cancel protocol, a persisted approval queue, per-verb or per-target grants, a pending-prompt cap, OS or desktop
notifications for prompts, a restructured relay fence; these are examples, not a blacklist), and whenever the same
component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with this
charter, supplying the request, the requirement sources above, this outline, the current diff and the proposed departure
(what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the relay budgets and how the classification is shared, how a connected GUI is
detected and what you found about the desktop app (P2), the exact list of agent operations the YOLO rule refuses and
allows after launch-representation (M7, M8), how `confirm_yolo` is handled for old supervisors, the template verbs'
names and flag set, the card's wording and the setting's label, whether the protocol needed a bump, how existing tests
were adapted, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. Anything that would let an agent act without the helm's approval, or start something on a
host that asks before YOLO launches that differs from what the user approved, is never an acceptable fallback. If
launch-representation landed something that makes an agreed behavior here impossible or ambiguous (for example, an
agent-reachable operation this file does not classify under M8), or the desktop app question in P2 has no small answer,
record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md Near term entry. Open, not merged: merging happens only after the maintainer has
reviewed this plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a
closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
