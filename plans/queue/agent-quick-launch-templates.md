# Agents can set the ⚡ quick-launch template and number keys

Written against main at 63439c2f on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Agents in Farhelm sessions can already list, create, edit and delete launch templates through the `farhelm` command,
each write behind an approval card (SPEC.md, Agent-spawned sessions, the paragraphs starting "Agents can also write
templates" and "Templates are ordinary fleet metadata"). What they cannot do is touch quick launch: the ⚡ mark (the one
template Cmd+P / Ctrl+Shift+P launches) and the ten number keys 1–9 then 0 (SPEC.md, Sessions › Creation, the "Quick
launch" and "Template number shortcuts" bullets). The template listing does not show them either. This plan lets an
agent set and clear both through the existing template verbs, shows them in the listing, and documents it.

TODO.md entry this plan covers (Near term): "Follow up on the recent quick launch system so an agent can populate the
templates."

The entry is short. The maintainer confirmed the reading above: template create, edit and delete already exist for
agents (#1609), so "populate" means the quick-launch side.

### Acceptance criteria

- `farhelm agent template create NAME` and `farhelm agent template edit NAME` accept `--quick-launch` (mark this
  template with ⚡) and `--key DIGIT` (give it number key `1`–`9` or `0`). `edit` also accepts `--no-quick-launch` and
  `--no-key` to clear them from this template. An `edit` that only changes the mark or key, with no template field, is
  accepted (today `edit` refuses a request with no field to change; that refusal now also counts these flags).
- The same rules as the Templates dialog (SPEC.md, the two bullets named above):
  - ⚡ goes only on a template that chooses an agent or a nonblank command, judged on the template as it will be stored
    after the write. The helm enforces this for the agent path; today only the UI does (`quick_launch_eligible` in
    `crates/farhelm-ui/src/list/templates.rs`, documented there as "an editor affordance, not backend validation").
  - A number key can go on any template, including one that cannot launch alone.
  - A template holds at most one key, and each key at most one template: giving this template a key replaces its old
    key, and taking a key or the ⚡ that another template holds moves it from that template.
  - `--no-quick-launch` or `--no-key` on a template that does not hold the ⚡ or a key is refused, naming what it holds
    (nothing to clear is a mistake the agent should see, not a silent success).
- The approval card shows the change: whether the template gains or loses the ⚡, which key it gains or loses, and, for
  a move, the template it moves from by name. The card's existing destination row (for a managed checkout, "managed
  checkout of <repo>") stays as it is; no extra consent wording is added.
- An older UI never approves a mark or key change it cannot show. A write that changes the ⚡ or a key travels under a
  distinct approval kind (the `LaunchWithPrompt` precedent in `crates/farhelm-proto/src/approvals.rs` and SPEC_impl.md
  "A prompt-bearing approval uses `LaunchWithPrompt`"), because `ApprovalAction::TemplateWrite` does not deny unknown
  fields and an older UI would silently drop new fields while still offering Allow. A write with no mark or key change
  keeps today's `TemplateWrite` card unchanged.
- Accepted consequence, not a blocker: an older UI decodes `GET /api/approvals` as one list (`fetch_approvals` in
  `crates/farhelm-ui/src/api.rs`), so while a card of the new kind waits, that UI fails the whole decode and keeps
  showing the last list it read. The prompt-bearing card kind shipped with exactly this behavior (SPEC.md, "under a
  distinct card kind an older UI cannot decode"; SPEC_impl.md's `LaunchWithPrompt` paragraph), and SPEC.md's "Upgrade
  compatibility and client scale" constrains the update path itself, which this does not touch. State it in the
  SPEC_impl.md text that introduces the new kind, with that citation.
- The approved write lands only on what the card showed. Today's rule (the template is refused if it changed or appeared
  while the card waited; SPEC.md "This is the one exception to last-write-wins for templates") extends to the bindings:
  the ⚡ name and the whole number-key map are read when the card is built, and the write is refused with nothing
  written if either differs at write time. The template and its binding change are written in one transaction: either
  both land or neither does.
- `farhelm agent template delete` and its card stay exactly as they are; deleting already clears a template's mark and
  key through the shared delete path (`delete_launch_template_if` → `clear_template_bindings` in
  `crates/farhelm-helm/src/store.rs`).
- `farhelm agent templates` shows, per template, whether it holds the ⚡ and which key; `--json` carries both as fields
  of each template.
- `farhelm agent instructions` describes the new flags (its test checks that every verb is mentioned; flag usage lines
  are generated from the CLI definition). SPEC.md's agent template paragraphs and the quick-launch bullets say agents
  can set these through a card. The docs pages `website/src/content/docs/docs/using/launch-templates.mdx` ("Templates
  and your agents") and `website/src/content/docs/docs/using/approve-agent-requests.md` (its bullet on what a template
  card shows) say so too.
- The upgrade rule holds (SPEC.md, "Upgrade compatibility and client scale"): no change breaks updating from v0.23.0 on.
  New wire fields and the new card kind follow the repository's existing protocol-bump pattern.

## Requirement sources

**The user's decisions (2026-10-17):**

- Scope: the user picked "Set and clear the ⚡ template and the number keys, and see them in the listing" over adding a
  one-file bulk import or something else.
- Conflicts: the user picked "It moves, and the approval card says from which template" (what the Templates dialog does)
  over refusing.

- Downgrades: the user accepted, for this plan among others, that its wire change (the new card kind and the protocol
  bump) leaves no supported path back to an older Farhelm after installing it, while the upgrade path must stay intact
  (answer to "Accept no downgrade path for these…?": Yes).

(These are options the planner wrote and the user chose, with no comment; they are not the user's own words.)

- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:**

- SPEC.md, Agent-spawned sessions: every acting verb waits for a card unless the host's "run farhelm commands from this
  host without asking" setting is on; template writes land only on the template the card showed; the listing withholds
  command text.
- SPEC.md, Sessions › Creation: the ⚡ eligibility rule, key uniqueness, keys on partial templates, and marking or
  keying as consent to a managed-checkout destination.
- SPEC.md, "Upgrade compatibility and client scale"; SPEC_impl.md's `LaunchWithPrompt` paragraph (an older UI must
  refuse a card it cannot fully show).
- Root `AGENTS.md`: Tests and validation, Changelog fragments.

**Planner proposals (to be checked by the planning review):**

- Flags on the existing `template create` / `template edit` rather than new verbs. An agent setting up quick launch
  usually writes a template and its key together, and one card then covers both; new verbs would add a second card for
  the common case and duplicate the template-write path (host resolution, precondition, listing reply). The clear flags
  exist only on `edit`, since a new template holds nothing to clear.
- The ⚡ eligibility check moves into shared code (`crates/farhelm-proto/src/launcher.rs`, next to
  `check_template_shape`) so the helm and the Templates dialog use one rule. Its doc comment ("an editor affordance, not
  backend validation") is rewritten, since the helm now enforces it for agents.
- A new approval kind (working name `TemplateWriteWithShortcuts`, carrying everything `TemplateWrite` carries plus the
  mark and key change and the template each moves from) used only when the write changes a mark or key.
- The template write and the binding change happen in one store transaction, through a new conditional store method
  modelled on `put_launch_template_if` and `delete_launch_template_if` in `crates/farhelm-helm/src/store.rs`: both
  already open one `Immediate` transaction and check the template precondition (`template_row_is`), and the delete
  already rewrites the bindings inside it (`clear_template_bindings`). The new method also takes the expected ⚡ name
  and key map and updates them before commit. The GUI's paths (`put_launch_template`, `PUT /api/preferences` with its
  whole-map replace, last write wins) stay as they are; only the agent path gets the precondition, matching the existing
  template exception.
- The binding precondition is one snapshot compare (⚡ name plus the whole key map) rather than separate checks of each
  affected holder. It can refuse when someone changed an unrelated key while the card waited; that is the same "changed
  while the card waited" refusal the agent already handles, and it keeps the rule and its test simple.
- Clearing a mark or key the template does not hold is refused rather than ignored.

## Outline

Two `feat` PRs:

1. **Helm and wire.** The shared eligibility rule; the new fields on `AgentVerb::TemplateCreate` / `TemplateEdit`
   (`crates/farhelm-proto/src/lib.rs`) and on `AgentTemplate` for the listing; the new approval kind
   (`crates/farhelm-proto/src/approvals.rs`); the helm's write path (`write_template_for_agent` in
   `crates/farhelm-helm/src/agent_requests.rs`, and the new conditional store method in `store.rs`); the protocol bump;
   helm unit tests for every rule above, including the stale-binding refusal, the move, the ineligible ⚡, the
   shortcut-only edit, and the refused clear; the GUI card rendering in `crates/farhelm-ui/src/approvals.rs` with its
   unit tests. SPEC.md and SPEC_impl.md updates. A changelog fragment (`kind: added`).
2. **CLI, listing and docs.** The flags on `TemplateCmd` (`crates/farhelm/src/main.rs`), the listing's text and JSON
   output (`crates/farhelm/src/render.rs`), `crates/farhelm/src/agent_instructions.rs`, the CLI tests in
   `crates/farhelm/tests/agent_cli.rs`, and the two docs pages. This PR removes the TODO.md entry. It edits PR 1's
   changelog fragment rather than adding a second one.

If PR 1 grows past comfortable review size, the GUI card can be its own PR between the two, as long as no PR lets the
helm emit a card kind the GUI of that same PR cannot render.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
the `farhelm-proto`, `farhelm-helm` (`agent_requests`, `store`, `preferences`) and `farhelm-ui` (`approvals`,
`list::templates`) unit tests and `crates/farhelm/tests/agent_cli.rs` through the recorder,
`cargo check -p farhelm-ui --features desktop`, `python3 releasing/check-changelog.py format`, `dprint check` on changed
Markdown, and the docs website build since the docs pages change. A browser run of `e2e/tests/templates.spec.ts` and the
approvals specs on Chromium and WebKit only if the card change touches shared card layout; say in the report which was
run and why.

## Fallbacks

No plan-specific fallbacks: the transaction follows an existing store pattern, and the older-UI behavior of the new card
kind is accepted precedent (Acceptance criteria above).

Examples of departures that need the scope reassessment: new verbs instead of flags, a bulk import, a preferences-wide
precondition for the GUI, a changed delete card, letting agents toggle "name sessions on quick launch".

Decisions to log: the CLI shape as built, the card kind's name and fields, how the listing shows the mark and key.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-agent-quick-launch-templates-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

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

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/agent-quick-launch-templates/<nn>-<short-name>` (for example
  `plan/agent-quick-launch-templates/01-quick-launch-verbs`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
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
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

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
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
