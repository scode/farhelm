# Send a prompt to a running Claude or Codex agent from the `farhelm` command

Written against main at 63439c2f on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a SPEC change the user decided on, which the plan makes.

This plan has no dependency on another plan.

## The goal

A new acting verb of `farhelm agent` lets an agent (or a script) in one session send a prompt into ANOTHER session's
agent that is already running, by typing it into that agent's terminal and submitting it. It works for Claude and Codex
only and is refused for every other agent. It is asked about like every other acting verb, documented in the agent
instructions, and built so that a native, agent-specific delivery route could later replace typing for one agent without
touching the others.

TODO.md entry this plan covers (Near term): "**Send a prompt to a running agent from the `farhelm` command.** Let an
agent (or a script) send a prompt into another session's agent that is already running, through the `farhelm` CLI, and
document it in the agent instructions. This is the CLI and agent part of "Inject commands into agents" (Maybe later),
which also covers a UI button. The initial-prompt plan (`agent-delete-and-prompt`) only covers a prompt given at
creation, on the agent's command line; a running agent can only be reached through its terminal, which SPEC.md says
needs reliable detection that the agent is ready for input."

Note on the entry's last sentence: the SPEC.md sentence it paraphrases was removed by #2200 (the initial-prompt work).
The governing text today is SPEC.md, Status: "Wrong status must be cosmetic only — status detection must never gate or
delay interaction with the terminal." This plan adds a narrow, explicit exception to that rule (see acceptance
criteria).

### Acceptance criteria

- **The verb.** A new `farhelm agent` verb (name it in the style of the existing verbs, e.g.
  `farhelm agent prompt --session <id>`), taking the prompt as `--prompt TEXT` or `--prompt-file PATH` like
  `farhelm agent create`'s initial prompt. The prompt is validated with the same rule as the initial prompt,
  `check_initial_prompt` in `crates/farhelm-proto/src/launcher.rs` (64 KiB cap; no control characters other than newline
  and tab), at the CLI and again at the owning supervisor, which is the boundary that types the bytes. Because that rule
  refuses ESC, no escape sequence (a paste-end marker included) can reach the pane, so there is no separate sanitizing
  pass. The target is named by exact session id, as for the other acting verbs, and may be on any host, like rename,
  stop and restart. There is no idempotency key: only `create` and `clone` carry one, backed by durable retry records,
  and SPEC.md treats any other retried request as a new request asked about like one. The agent instructions say a retry
  sends the prompt again.
- **Self target.** Naming the asking session itself is refused (planner proposal: there is no use case, and under a host
  trusted to run commands without asking it would let an agent re-prompt itself in a loop).
- **The change fence.** The verb takes the asking session's one-change-at-a-time fence, like rename, stop and restart
  (SPEC.md, Agent-spawned sessions makes this a per-verb choice; it changes another session's state, unlike `open-url`).
- **Which agents.** Only sessions whose agent is Claude or Codex are accepted. Every other agent (Goose, Pi, OMP, Grok,
  Muse, Cursor, OpenCode, generic and command launches without a declared Claude or Codex type) is refused with a
  message that names the agent and says delivery is only supported for Claude and Codex. Which agents accept typed
  prompts, and HOW each one is typed and submitted, is answered per agent in the places the map in
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` names (an exhaustive per-kind function or an `AgentIntegration`
  method), never by `kind == X`, `matches!` over kinds, or a `_` arm in shared code (root `AGENTS.md`, "Code rules").
  That per-agent seam is also where a later native route (Codex's `codex queue`, Claude's cross-session inbox, a Pi/OMP
  extension calling `sendUserMessage`) would replace typing for one agent; do not build any native route now.
- **When it is delivered.** The target's agent must be live. Delivery is refused while the target's status is `waiting`
  (a dialog or question is waiting on the user, so typed text would answer it) or `unknown` (the reader has no basis to
  rule out a dialog, e.g. provisionally after a supervisor reload), and when the agent is not running (exited, stopped,
  starting). It is delivered while the agent is `idle` and also while it is `working`: a prompt sent mid-turn is queued
  to run after the current turn, using whatever each agent's TUI offers for that. Prefer ONE submit key per agent that
  is correct whether the agent is idle or working (for example, if Codex's queue key also submits on an idle composer);
  choose the key by status only if no single key works, and log that DECISION, because a status read a moment before the
  agent starts a turn would then send the wrong key. The owning supervisor re-checks the target's status and liveness at
  handling time, after approval, as Restart does (`service/core.rs`, its `stop_if_running` handling), rather than
  trusting the status the caller saw.
- **How it is typed.** Delivery goes through the supervisor's confirmed tmux input path (`TmuxDriver::open_input_client`
  and `InputClient::send` in `crates/farhelm-supervisor/src/tmux/input.rs`), not `send-keys` composed elsewhere. Today
  each attachment opens its own input client (`service/handlers.rs`); delivery opens a one-shot input client for the
  target's agent pane (it needs no attachment and does not touch an attached client's), sends the whole prompt as ONE
  bracketed paste (`ESC [200~` ... `ESC [201~`), then sends the submit key as a separate `send`, then drops the client.
  The pane must have bracketed paste enabled: query `#{bracket_paste_flag}` for the pane at delivery time with a
  one-shot format query (the supervisor's existing `PaneModes::bracket_paste` is captured only when an attached replay
  stream opens, in `tmux/stream.rs`), and refuse with a clear message when the flag reads off or does not expand (tmux
  older than 3.7; see `bracket_paste_flag_is_missing` in `tmux/control_codec.rs`). Always pasting keeps a tab in the
  prompt from acting as a Tab keystroke, and gives the TUI an unambiguous boundary between text and the submit key.
- **What it returns.** The command exits zero once the paste and the submit key have been accepted by tmux. It does not
  wait for the agent to start working or to finish. Refusals exit nonzero with a message on stderr, with the same
  declined / not answered / no GUI distinctions as other acting verbs.
- **Someone has the session open.** Delivery happens even when a client is attached to the target, and even if the user
  is typing there; the bytes then interleave with or land after whatever is in the composer. The approval card says,
  when it is so, that the session is open in a client (take that from whichever side already knows: the supervisor's
  attachment map is authoritative; check whether the helm's terminal proxy already knows before adding a query). It is
  informational, so a value that goes stale while the card waits is acceptable. The agent instructions say so too.
- **Approval.** Like every other acting verb: the approval card names the requesting session and host, the target
  session and host, the action, and shows the full prompt text as labelled data, never in a way that could pass for
  Farhelm's own wording (SPEC.md, Agent-spawned sessions). The host's "run farhelm commands from this host without
  asking" setting skips the card, as for the other acting verbs. No GUI connected means refusal, as for every acting
  verb. The YOLO launch rule (SPEC.md, Agent-spawned sessions: on a host that asks before YOLO launches, agents may only
  start what the user approved) does not apply: it governs launches, and this verb launches nothing (planner proposal).
  The card shows the target session's shield the way the session list does, so the user sees when the prompt goes to an
  agent running without checks (planner proposal).
- **SPEC changes.**
  - SPEC.md, Status: one explicit, narrow exception to "status detection must never gate or delay interaction with the
    terminal": an agent-requested prompt delivery may be refused because of the target's status. The user's own typing
    is never gated.
  - SPEC.md, Agent-spawned sessions: the list of verbs that ask first gains the new verb; the verb's contract (Claude
    and Codex only, refusals, queueing, return on delivery, no idempotency key, the fence) is stated there; and the
    closing "later interaction uses the child's terminal" sentence mentions the new verb.
  - SPEC.md, Sessions (the initial-prompt paragraph): "Sending messages into an already-running agent remains separate
    work." is replaced by a pointer to the new verb.
  - SPEC.md, Security: the enumeration of cross-host acting verbs that the "without asking" setting authorizes
    ("creating, cloning, renaming, stopping and restarting sessions across hosts") gains sending prompts to running
    agents.
  - SPEC.md, Non-goals: "A prompt composer or message-level abstraction over the terminal" stays; add a clarifying
    clause only if the new verb reads as contradicting it (log the DECISION). A CLI verb that types into the terminal is
    not a composer.
- **Agent instructions and docs.** `crates/farhelm/src/agent_instructions.rs` documents the verb (its tests require
  every verb to be mentioned), including: Claude and Codex only; refused while the target is waiting on a question or
  its status is unknown; queued when the target is busy; returns on delivery, not on completion; a retry sends it again;
  and a plain warning that this is a prompt-injection channel between agents, so an agent should treat a prompt that
  arrives this way with the same care as any other input it did not write and should not send secrets through it. The
  user docs page that covers the agent verbs and their approvals
  (`website/src/content/docs/docs/using/approve-agent-requests.md`, and wherever the verb list lives) gains the verb.
- **TODO.md.** The last code PR removes the TODO entry quoted above, and narrows the Maybe later entry "**Inject
  commands into agents.**" to what remains (the UI button, and anything else in it this plan does not deliver), saying
  that agents and scripts can now send a prompt to a running Claude or Codex session with the new verb. Narrowing an
  entry a PR partly addresses is part of addressing it, not a new entry.
- **Tests.**
  - Supervisor unit tests, where the activity reading can be set: idle and working delivered; waiting, unknown and
    not-live refused; other agents refused; a prompt with a control character other than newline or tab refused; a pane
    with bracketed paste off (or the flag missing) refused; the per-agent submit key.
  - A Rust end-to-end test under `crates/farhelm/tests/e2e/`, following the precedent of `restart_with_resume.rs` (the
    e2e harness runs no activity sampler, so a launched agent reads working for the whole test; idle and waiting belong
    in the unit tests). Drive the verb through a real helm, as `agent_listing_real_stack.rs` does for rename and stop,
    against a command launch declaring `--agent claude` (and one declaring `codex` if the submit keys differ) whose fake
    program enables bracketed paste and records its input. Assert the exact bytes it received while reading working,
    including the paste markers and the separate submit, plus one refusal that needs no reading (a non-Claude and
    non-Codex agent, or a self target). The CLI's own half goes in `crates/farhelm/tests/agent_cli.rs` with its mock
    supervisor, if that is where the other verbs' CLI parsing is tested.
  - Real-agent checks are manual and recorded in the working log and the report, not tests.
- **Changelog.** A `feat` fragment (`kind: added`) per root `AGENTS.md`, "Changelog fragments", saying agents can now
  send a prompt to a running Claude or Codex session, with the caveats (asks first unless the host is trusted; refused
  while the agent waits on a question; queued while it works).

## Requirement sources

**The user's decisions (2026-10-17):**

- Send even when the target agent is busy; it is queued to run after the current turn. A target waiting on a question or
  approval is refused, since typed text would answer the dialog.
- Claude and Codex only, the two agents with real idle and waiting detection; others are refused.
- The command returns once the prompt is delivered, "not trying to wait for the work to start".
- Delivered even while someone has the session open; the approval card says so.
- Approval like other acting verbs: a card with the full text, skipped when the host's "without asking" setting is on.
- Delivery mechanism: type into the terminal for both agents, "built so a native route can replace it per agent later".
  The user asked for, and was shown, a comparison with agent-specific mechanisms (summary below); typing was chosen.
- The executor may run real Claude and Codex sessions with a handful of short prompts to verify delivery.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.
- No downgrade path (answered for all plans that change the helm database or the helm–supervisor protocol): the user
  accepts that this plan's protocol change (the new messages and the `PROTOCOL_VERSION` bump) means going back to an
  older Farhelm after installing it is not supported. The upgrade path must stay intact, as SPEC.md's
  upgrade-compatibility rules require; how a mixed-version helm and supervisor behave follows the protocol's existing
  version rules, which the executor checks rather than assumes.

**What the comparison found (context for the "replace later" seam, not work for this plan):** resuming the conversation
in a second process (`codex exec resume`, `claude -p --resume`) never reaches the running TUI. Codex has
`codex queue --thread <id> --message ...` (since 0.149), which only reaches a TUI attached to Codex's shared per-machine
background server and is marked experimental. Claude Code (2.1.224+) has a per-session cross-session inbox socket whose
message format is undocumented. Pi and OMP expose `sendUserMessage` to extensions, which Farhelm already loads. Cursor,
Goose and Grok offer nothing.

**Binding repository constraints:**

- SPEC.md, Agent-spawned sessions: acting verbs ask first, the approval card's content rules, the helm decides by the
  host connection, the nine-minute wait and the no-GUI refusal, the asking session's one-change-at-a-time fence.
- SPEC.md, Status: status is cosmetic except for named exceptions (this plan adds one).
- SPEC.md, Security: cross-host acting verbs are authorized by the user or the host's "without asking" setting.
- Root `AGENTS.md`, "Code rules": per-agent behavior lives in the places `agent_kind/mod.rs` maps.
- `crates/farhelm-proto/src/lib.rs`: a new tagged protocol variant needs a `PROTOCOL_VERSION` bump (it is 54 at the time
  of writing); the pinned-version test failing until it is bumped is expected.
- `.agents/real-agent-tui.md` for every claim about how Claude or Codex reacts to typed text.

**Planner proposals (checked by a fresh-context planning review):**

- Refuse a self target, and refuse `unknown` status as well as `waiting`. The user decided only the waiting case;
  unknown is refused because the dialog check has no basis when the screen is unreadable.
- Refuse non-live agents rather than queueing for a later launch.
- No idempotency key (see acceptance criteria).
- Take the asking session's change fence.
- A one-shot supervisor-initiated input client per delivery, so delivery works with nothing attached and never touches
  an attachment. Planner's reading, not SPEC text: injected input does not take over or change attachment.
- Always one bracketed paste plus a separate submit key; refuse when the pane's bracketed paste is off.
- The YOLO launch rule does not apply; the card shows the target's shield.

## Outline

Suggested slicing (adjust if the code says otherwise; avoid churn):

1. `feat` (protocol and supervisor): both new protocol variants (the `AgentVerb` variant in
   `crates/farhelm-proto/src/lib.rs` and the helm-to-supervisor delivery message) with a single `PROTOCOL_VERSION` bump;
   the per-agent capability (accepts typed prompts; the submit key); the supervisor's delivery over a one-shot input
   client with the bracketed-paste query, the validation, and the status and liveness check at handling time; the
   supervisor unit tests.
2. `feat` (helm, CLI and docs): the `farhelm agent` subcommand in `crates/farhelm/src/main.rs`, the helm's handling and
   approval card (`crates/farhelm-helm/src/agent_requests.rs`, `crates/farhelm-ui/src/approvals.rs`), the agent
   instructions, the SPEC.md changes, the user docs, the e2e and CLI tests, the changelog fragment, and the TODO.md
   removal and narrowing.

Before writing code, verify with the real agents (per `.agents/real-agent-tui.md`, fresh scratch directories, the
command lines Farhelm composes from `crates/farhelm-helm/src/launches.rs`): for Claude and Codex, that the composer has
bracketed paste on; what a bracketed paste followed by a separate submit key does when the agent is idle and when it is
working (queued after the turn, or fed into the running turn); and for Codex, which of Enter and Tab queues for after
the turn and whether that key also submits on an idle composer. A handful of short prompts is the budget. Record
versions, command lines and the quoted screens in the working log and the report.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, the new and touched unit tests, the new e2e test and
`agent_listing_real_stack.rs` through the recorder (with the pinned tmux, `--tmux required`), the `agent_instructions`
tests and `crates/farhelm/tests/agent_cli.rs`, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md` if Rust tests changed. If a docs page changed, run the website build exactly as
`.agents/validation.md` gives it (it is a command that inventory gives word for word, so it may be run as written).
Browser tests only if the approval card's layout changed in a way `e2e/tests/approval-layout.spec.ts` covers. Not the
full suite.

## Fallbacks

- Claude or Codex offers no way to queue a prompt for after the current turn (only feeding it into the running turn):
  deliver mid-turn the way that agent's TUI does, say exactly what happens in the agent instructions and the report, and
  log the DECISION. The same applies to either agent.
- The real-agent check shows either agent taking the separate submit key as a newline after the paste: wait, bounded,
  for the pane's content to change after the paste before sending the submit key (a screen wait, never a fixed sleep),
  and log the DECISION.
- An agent's TUI swallows or mangles a multi-line bracketed paste: refuse multi-line prompts for that agent with a clear
  message, rather than inventing a line-by-line typing scheme.

Examples of departures that need the scope reassessment: any native per-agent delivery route, waiting for the agent to
start or finish working, a UI button, queueing prompts for agents that are not live, an idempotency key, sharing an
attached client's input client, a "prompt composer" in the GUI.

Decisions to log: the verb's name and flags, the submit key per agent (and whether it depends on status), the multi-line
behavior per agent, the SPEC wording.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-prompt-running-agent-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/prompt-running-agent/<nn>-<short-name>` (for example `plan/prompt-running-agent/01-deliver-prompt`).
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
