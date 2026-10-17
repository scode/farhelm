# Agent session deletion and an initial prompt at creation

Written against main at 0e7c3dd5 on 2026-10-16. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

Two TODO.md entries (Near term), merged into one plan by the maintainer because both change the `farhelm agent`
commands, the agent instructions and SPEC.md's "Agent-spawned sessions":

> **Let agents delete sessions.** Agents can currently stop sessions but cannot delete them. Add session deletion to the
> CLI and agent instructions so an agent can remove a session rather than only stopping it.

> **Supply an initial prompt when creating a session.** Allow an initial prompt to be injected when creating a session
> through the CLI, and document it in the agent instructions. Consider initial-prompt support in the launcher GUI as
> well. The separate "Inject commands into agents" item covers commands sent to sessions that are already running.

Build it as one stack, deletion first.

### Acceptance criteria: deletion

- **`farhelm agent delete --session <id> [--stop-if-running]`**, beside `stop`, with the same exact-id targeting and the
  same relay path (the CLI talks to its own supervisor with the session credential; the supervisor relays to the helm).
- **Approval**: like stop and restart, the user approves each delete on an approval card unless the host is set to run
  farhelm commands without asking; that setting covers delete exactly as it covers the other acting verbs (the
  maintainer's choice). The card says what the delete will kill when anything is alive.
- **Running targets**: without `--stop-if-running`, only a session with nothing alive is deleted; a running one is
  refused with a clear error naming the flag. With the flag, the delete stops what is running (agent and tabs) and
  deletes, under exactly the guard the card showed (the UI's existing "a confirmation authorizes only what its prompt
  showed" rule, SPEC.md "Lifecycle operations").
- **Self-delete is refused** with a clear error saying a session cannot delete itself (stop still works on self). The
  relay holds a lock keyed by the asking session for the whole request and the supervisor's delete waits on the same
  key, so a self-delete as built would hang, and the session's credential would be revoked mid-request. (SPEC.md
  declines `farhelm agent replace` because the agent would be "killing itself mid-request"; cite that as related, not as
  the lock reason.) The maintainer wants self-deletion later; the planning PR adds a Maybe later TODO entry for it.
- **Crossing deletes**: check whether two sessions deleting each other (both approved, which is easy with the "without
  asking" setting) can stall each other on those locks. If they can, prevent the stall in the smallest way you find and
  log it; if they cannot, log why.
- **Output**: success prints a line like stop's (`deleted <id>`), plus the checkout notice the UI's delete can produce
  (a managed checkout not archived, or archived to the trash). A lost reply is reported as outcome-unknown, like the
  other mutating verbs.
- **Same deletion as the UI**: extract the helm's inline delete logic from the HTTP handler into one function the UI and
  the agent verb both call (denying the session's waiting approval cards first, the supervisor teardown, clearing pins,
  seen state and notification marks, forgetting the session), so they cannot drift. Replace's `finish_replacement` has a
  third copy of the cleanup; reuse the shared part there too if that stays a small change, otherwise leave it and say so
  in the report.

### Acceptance criteria: initial prompt

- **One-time per session.** A session's prompt is sent once, at creation, as part of the agent's own command line
  (SPEC.md's sketch: "for agents that accept an initial prompt on the command line (Claude Code and Codex both do), add
  the prompt to the composed command — no readiness detection needed"). It is not stored in the session's launch:
  Restart, Clone, Replace and "Replace with" do not send it again, and recent setups do not remember it (the
  maintainer's choice). Applying a Clone or "Replace with" prefill clears a prompt typed in the launcher, since the
  prefill describes the old session and the old session's prompt is not carried.
- **Transport (planner proposal, from the planning review):** carry the prompt as a new template field, because
  `farhelm agent create` already sends its flags as template fields ("launcher edits") and the create's retry digest
  already covers them, so templates, `--prompt` and the digest become one mechanism (do not change the digest's frozen
  format). On the create message, add one optional prompt field, not a second ready-made command line like
  `named_start`; the supervisor places it on whichever start command it picked (plain or named conversation) by a
  per-agent placement rule, after Farhelm's own arguments where the agent allows. It works with a managed checkout.
  Whether a named conversation plus a prompt is allowed follows from whether the agent's named start accepts the prompt;
  refuse the combination where it does not, and log what you found.
- **Templates carry a prompt**: a template may set an initial prompt; every launch from that template starts with it,
  because a template fills in the launcher (the maintainer's words: "one time, but for a template it's applied every
  time of course"). An agent's `farhelm agent template create`/`edit` gains the prompt through the shared flags; its
  approval card shows the prompt, and the agent's template listing includes it.
- **Per-harness support**, answered where `crates/farhelm-supervisor/src/agent_kind/mod.rs`'s module docs say a new
  per-harness capability goes (root `AGENTS.md`, "Code rules": no `kind == X` in shared code). "Takes a prompt" means
  the agent accepts a starting prompt on its command line and stays interactive; an agent that switches to a one-shot
  mode when given a prompt does not count. Check each harness's installed CLI (read `.agents/real-agent-tui.md` first,
  as root `AGENTS.md` requires before relying on a claim about how an installed agent behaves) for how it takes one
  (positional argument or flag, and where it must sit relative to Farhelm's own arguments), and record what you found.
  An agent whose support cannot be confirmed on its installed CLI counts as not taking one. An agent that does not take
  one is refused with a clear error, and the UI does not offer the box for it.
- **Custom-command launches** (the launcher's command tab, `--command`) refuse a prompt.
- **CLI**: `farhelm agent create` and `farhelm spawn` gain `--prompt TEXT` and `--prompt-file PATH` (`-` for stdin),
  mutually exclusive. The prompt counts toward SPEC.md's 64 KiB launch budget; multi-line text is allowed (control
  characters other than newline and tab are refused, and say so). A prompt that starts with `-` must still reach the
  agent as the prompt, not as an option. `farhelm spawn --inherit-agent` with a prompt: allow it when the inherited
  agent takes one, if the shared flag handling permits that without a special case; otherwise refuse it clearly and log
  the choice.
- **Approval card**: an agent's create request that carries a prompt shows the full prompt on the approval card, escaped
  like other host-supplied text, in a scrollable block, so the user approves exactly what will be sent. A browser tab
  still running an older UI must not show the card without the prompt: carry it in a form an older UI fails on (for
  example a field its decoder refuses, or a card kind it does not know) rather than one it silently ignores, and test
  that.
- **Launcher (as the maintainer approved from the mockup)**: in the agent tab's right column, directly after the
  named-conversation (session id) field, a labeled "initial prompt" box: a resizable multi-line textarea in the
  launcher's input style, with a help line below ("Sent once, as <agent>'s first message when it starts. Restart, Clone
  and Replace do not send it again."). The summary line above the columns gains "prompt: N lines" when one is set. The
  box is not shown for an agent that takes no prompt (the way an agent without a model control gets no model box), and
  not on the command tab. Picking a recent setup leaves the box as it was.
- **Save as template**: the panel lists "initial prompt: <first line…>" as a choice, checked by default when a prompt is
  typed, its value escaped like the panel's other values.
- **Templates editor**: a new "initial prompt" field with a textarea, added through "+ add field" (offered only when the
  template's agent takes a prompt, or no agent is set) and removed with ✕ like the other fields, with a help line ("Sent
  as the agent's first message on every launch from this template. You can still edit it in the launcher before
  launching."); the template list's summary mentions it.
- The mockup the maintainer approved as drawn is https://snippets.scode.org/s/farhelm-initial-prompt-mockup/ (a
  reference only; the criteria above are the agreed design).

### Acceptance criteria: both

- **Agent instructions** (`crates/farhelm/src/agent_instructions.rs`): delete in the verb list and the approval
  paragraph; the prompt options on create and spawn, which agents take one, and that it is sent once. Leave the existing
  "once the user submits the first prompt" readiness wording alone: it describes when a session becomes ready and is
  shared with other messages.
- **SPEC.md**: "Agent-spawned sessions" (the acting verbs "rename, stop, restart" become including delete, with the
  self-delete refusal and its reason); "Lifecycle operations" if needed; the launcher's "you type your first prompt into
  its terminal. Automatic initial-prompt delivery is post-v1" paragraph, the spawn paragraph "spawning launches the
  agent without an initial prompt in v1", and the v1 non-goals entry, rewritten for what now exists; templates.
  SPEC_impl.md's "CLI" and agent-request sections. The docs website pages for the agent CLI, the launcher and templates
  (`website/AGENTS.md` governs them).
- **Protocol version**: both the new verb and the new create field change the helm–supervisor protocol, so bump its
  version (the maintainer accepted this; once for the stack is enough if both land in it); the changelog fragments say
  remote hosts need an Update after upgrading.
- **Changelog fragments**: one per feature, `kind: added`, each in the same commit as its feature PR (root `AGENTS.md`),
  not saved for the end.
- **Tests**: CLI tests in `crates/farhelm/tests/agent_cli.rs` (missing `--session`, refusals, output lines, the shared
  verb tables), helm agent-request tests (approval, guard, self-delete refusal, outcome-unknown), an e2e relay test
  modelled on `crates/farhelm/tests/e2e/agent_relay.rs`, per-harness argv tests for the prompt (its position, a prompt
  starting with `-`, refusal for harnesses without support), template round-trip tests, UI unit tests for the launcher
  and template field, and browser tests on Chromium and WebKit for the launcher box (shown and hidden per agent), save
  as template, and the template field.
- The last PR removes both TODO.md entries.

## Requirement sources

**The user's decisions (2026-10-16):** merge the two entries into one plan; refuse self-delete and add a Maybe later
entry for allowing it; refuse a running target unless `--stop-if-running`; the auto-approve setting covers delete;
prompt in the CLI, the launcher, and templates ("templates should be included here - so you can have a template that
adds an initial prompt"); refuse agents without a command-line prompt; one-time per session, every launch for a template
("no re-injection on restart or replace"); refuse for custom commands; `--prompt` and `--prompt-file`; the mockup as
drawn; recent setups do not remember the prompt; accept a protocol bump; review gate: one fresh-context gpt-6.1-sol
reviewer at high effort, no swarm; no-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md "Agent-spawned sessions" (every acting verb asks the user; deleting a
session first denies the requests it has waiting); SPEC.md "Lifecycle operations"; SPEC.md's 64 KiB launch budget; the
per-harness map rule in root `AGENTS.md` "Code rules"; SPEC.md's escaping of host-supplied text; "Upgrade compatibility
and client scale" (existing templates without the new field keep loading; give it a serde default);
`.agents/test-authoring.md`.

**Planner proposals:** the verb shape and flag name; extracting a shared delete function; carrying the prompt as a
template field plus one optional create field; a per-harness "takes a prompt" capability beside the existing `offers_*`
checks in `crates/farhelm-proto/src/launch.rs`; the summary-line count; clearing a typed prompt on a Clone prefill.

## What planning found

- CLI stop: `AgentCmd::Stop` (`crates/farhelm/src/main.rs`), `AgentVerb::Stop` (`crates/farhelm-proto/src/lib.rs`). A
  new verb must be added to `is_mutating` and `may_wait_for_user` (`crates/farhelm-proto/src/lib.rs`, exhaustive
  matches), `ReplyKind::of_verb`/`of` (`crates/farhelm/src/agent_client.rs`), `validate_agent_verb`
  (`crates/farhelm-supervisor/src/service/handlers.rs`) and `validate_authoritative_verb`
  (`crates/farhelm-helm/src/agent_requests.rs`). The helm handles stop by resolving the target, routing the session,
  showing an `ApprovalAction::Stop` card, then `sessions::do_stop_session`.
- UI delete: `DELETE /api/sessions/{id}` with `only_if_nothing_alive` / `only_if_agent_ended` guards (`DeleteGuard`),
  handled inline in `crates/farhelm-helm/src/sessions.rs`; the supervisor's `handle_delete_session` claims the target's
  fence, then checks liveness against the guard (Conflict if more is alive than confirmed) and tears down.
- Session creation from the CLI: `farhelm agent create` and `farhelm spawn` share `LaunchFlags`; the helm compiles
  choices with `compile()` (`crates/farhelm-helm/src/launches.rs`) into
  `ControlMsg::CreateSession { cwd, launch, named_start, … }`; the supervisor picks the argv in `create_start_argv`
  (`crates/farhelm-supervisor/src/service/core.rs`). Start argvs end with the `{farhelm_args}` placeholder, and a
  comment in `launches.rs` notes start commands carry no positional prompt today; a prompt changes that assumption for
  resume selectors.
- Templates: `TemplateFields` (`crates/farhelm-proto/src/launcher.rs`), checked by `check_template_shape`; the editor's
  `Field` enum (`crates/farhelm-ui/src/list/templates.rs`) with `label`/`key`/`is_set`/`add`/`remove`/`offered`; the
  save panel in `crates/farhelm-ui/src/list/save_template.rs`; the launcher in
  `crates/farhelm-ui/src/list/create_form.rs` (draft building in `launcher_snapshot` and `template_edits`, Clone/Replace
  prefill in `prefill_from`, which must leave the prompt out).
- No textarea exists in the launcher or template editor yet; `.feedback-field textarea` and
  `.checkout-post-clone-field textarea` in `crates/farhelm-ui/assets/app.css` are the styles to borrow.

## Outline

1. **Delete**: shared helm delete function; verb, reply, CLI, approval card, self-delete refusal, instructions, its SPEC
   and docs text, its changelog fragment, tests.
2. **Prompt transport**: per-harness capability and placement; the template field and the create field; CLI flags;
   approval cards; instructions; tests; the prompt's changelog fragment.
3. **Templates editor and save as template.**
4. **Launcher**: the box, summary count, per-agent visibility.
5. **Remaining SPEC, SPEC_impl and docs website text, and the TODO removal.**

One PR per step is the expected shape; merge steps if one is too small to review alone. Each PR carries the spec and
docs changes for what it builds where that keeps it coherent.

### Validation

Per `.agents/validation.md`: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, the affected crates' tests through the recorder (CLI, helm agent
requests, supervisor create, proto, UI), the relay e2e test, the doctests if touched, the selected browser specs on
Chromium and WebKit through the recorder (with the builds they need), the test-sleep checker,
`python3 releasing/check-changelog.py format`, `dprint check`, and the docs website build.

## Fallbacks

- A harness whose CLI takes a prompt only in a way that cannot be placed safely in its start argv counts as not taking
  one: refused and not offered; say which in the report.

Departures that need the scope reassessment (examples, not a blacklist): allowing self-delete, typing a prompt into the
terminal after startup, storing the prompt in the session's launch, a prompt for custom commands, deleting by name, or
changing the UI's own delete behavior.

Decisions to log: each harness's prompt support and argv placement, named conversation plus prompt, spawn
`--inherit-agent` with a prompt, crossing deletes, the verb and reply shapes, the protocol bump, how older UIs are made
to fail on a prompt-bearing card, and whether Replace's cleanup was folded into the shared delete function.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-agent-delete-and-prompt-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
("Sub-agents this file requires") requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from the no-delegation demand and run as that file says, not through galaxy-brain.

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
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/agent-delete-and-prompt/<nn>-<short-name>` (for example `plan/agent-delete-and-prompt/01-agent-delete`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `feat` (a pure extraction PR may be `refactor`). Add the
  changelog fragments (`kind: added`) per root `AGENTS.md`.
- The last PR removes both TODO.md entries this plan covers.
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
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes both TODO.md entries. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing one
plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
