# Start a session in a conversation the user names

Written against main at e6c80c6c on 2026-10-13. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

Let the user name the agent conversation a session should start in (what some agents call a session id), wherever a
launcher-style dialog starts or restarts a session: **New**, **Clone**, **Replace with** and **Restart with**. Apart
from that one choice, each of those works exactly as it does today.

The maintainer's motivating case: a session whose conversation Farhelm never captured (conversation reporting wasn't
working in that environment). After upgrading Farhelm they knew the conversation's id and wanted to restart the session
into it, but had no way to tell Farhelm to use it.

### How it works (settled in planning)

- **One-time.** The id applies to that one start. Farhelm starts the agent in its resume form with the user's value (for
  example `claude --resume <id>`, `codex resume <id>`) instead of its fresh-start form. The session's stored launch
  stays the ordinary one, so a later Clone of that session starts fresh as Clone always does, and the value is not saved
  in templates.
- **Identity still comes from the agent.** SPEC.md (Durability and resume, "Confirmed 2026-10-01") says Farhelm
  identifies a conversation only from the agent's own report. That rule stands: the user's value is only a start
  argument, and the session's captured conversation is whatever the agent then reports, exactly as for any launch. For
  agents that report, later Restarts resume it.
- **Every agent that can start in a named conversation.** Today Farhelm has resume forms for Claude, Codex, Grok, Goose,
  Pi and OMP (`resume_arguments` in `crates/farhelm-helm/src/launches.rs`). Muse, Cursor and OpenCode have none; offer
  them too if the agent has a command-line way to start in a given conversation, verified on the real agent. For those
  three, and for Goose (whose reporter is not attached on resume), Farhelm still won't learn the conversation
  afterwards, so plain Restart stays unavailable for those sessions, as it is today; the interface says so where it
  offers the field. Pi and OMP take a session file path rather than an id; the field is labeled accordingly.

### Acceptance criteria

- **The field.** New, Clone and Replace with (the launcher: `crates/farhelm-ui/src/list/create_form.rs` and
  `crates/farhelm-ui/src/launch_controls.rs`) and Restart with (`crates/farhelm-ui/src/restart_with.rs`) gain an
  optional conversation field on agent launches only (a command launch can already write the id into its command), shown
  only for agent types that offer it, labeled per agent ("conversation id", "session file", or the agent's own term).
  Empty means today's behavior. For agents where Farhelm won't capture the conversation afterwards, short help text says
  Restart won't be available. In Restart with, a conversation value counts as a change that enables the primary action.
- **Restart with must become reachable in the motivating case.** Today it is unavailable whenever nothing was captured,
  at every layer: SPEC.md ("This action is available exactly when Restart is"), the UI (`restart_with_unavailable` in
  `session_view.rs` requires `RestartOffer::Resume`), and four gates in the supervisor's `restart_session`
  (`service/core.rs`): the offer check, Codex's `unverified_resume_refusal`, the requirement for a captured conversation
  before `filled_resume_argv`, and `verify_report_only_resume` for Codex, Pi, OMP and Grok. Make Restart with available
  for an agent launch whose stored launch has a resume command even when nothing is captured; in that state the
  conversation field is required and the dialog shows why. A named Restart with bypasses those four gates, which exist
  to protect a captured identity that a named restart does not use. Muse, Cursor and OpenCode have no resume command, so
  Restart with stays unavailable for them; they get the field on New, Clone and Replace with only.
- **Validation.** Reuse the existing id checks rather than writing a new validator: `is_plausible_conversation_id` and
  `is_reserved_locator_token` in `crates/farhelm-supervisor/src/agent_kind/mod.rs`, which refuse a leading `-` (the
  value would become a flag) and reserved placeholders (a second substitution pass would rewrite `{cwd}` or a Codex
  trust marker). They refuse spaces, which matters for Pi and OMP paths: decide and log whether those two accept spaces
  and whether a `~` or relative path is refused or passed through (no shell, so `~` is never expanded; a relative path
  resolves against the session's folder). The value is passed as one argument. Farhelm does not check that the
  conversation exists; an agent given an unknown id fails or starts the way the agent itself does, and the session shows
  that like any other failed start.
- **The argv.** For Claude, Codex, Grok, Goose, Pi and OMP the argument form is already the stored launch's resume
  command: a named start is that resume command with `{conversation}` filled from the raw value through its own fill
  path reusing `fill_slots` (`filled_resume_argv` parses each kind's stored locator vocabulary and cannot take a raw
  value). No second per-harness argv table for these six. The new per-harness facts are only "offers a named start" and
  "captured afterwards" (exhaustive `LaunchHarness` methods in `crates/farhelm-proto/src/launch.rs`, root `AGENTS.md`
  "Harness-specific code") and the labels and help text (`launch_composer.rs`). For Muse, Cursor and OpenCode, if a flag
  is verified, the helm composes a separate one-time start for the create request; do not add them to
  `resume_arguments`, which would give them a resume command and change when Restart is offered.
- **Not stored.** The value stays outside `SessionLaunch` and `LaunchSelection`, so the stored launch, recent setups,
  remembered defaults, templates and "save as template" never record it, and a later Clone starts fresh.
- **Capture afterwards (maintainer decision after planning review: keep the old capture if simple).** Admission does not
  refuse a newly reported conversation (the planning review checked: Codex's and OMP's compare-and-swap is a concurrency
  fence on the row just reloaded, the Codex process pin clears on every relaunch, Grok's `selected_at` is the report's
  own event time, and Claude, Goose and Pi replace with no comparison). So a named Restart with keeps the session's
  captured conversation until the agent reports the new one, which then replaces it through the ordinary path; a
  mistyped id loses nothing. The exception is an agent Farhelm won't capture afterwards (Goose, whose reporter is not
  attached on resume): keeping the old capture there would make plain Restart resume a conversation the agent is not in,
  indefinitely, so for those agents a named restart clears the captured conversation inside the `begin_relaunch`
  transaction (settle what `abort_relaunch` restores and log it). Codex reports only at the first prompt, so until then
  plain Restart still resumes the old conversation; say so in the docs.
- **Real-agent verification.** Following `.agents/real-agent-tui.md`, for each offering agent start it the way the new
  start will, with the value of an existing conversation created for the purpose in a scratch directory, and show from
  the screen that it opened that conversation. For Muse, Cursor and OpenCode, find and verify the flag (or confirm there
  is none). Reporting itself needs no separate real-agent check: resumed starts already report on every plain Restart
  for the reporting agents, and unit tests through the report path cover capture after a named start. Record versions
  and exact commands in the working log and the report. Do not touch user-level vendor configuration.
- **Managed checkouts.** The field is refused when the destination is a new managed checkout: those creates have their
  own idempotency and recovery encodings (`FreshCreateFingerprint`, the helm's `github_create_request_v2`, the stored
  recovery snapshot that relaunches after a lost reply) that would otherwise all need versioning, and the motivating
  case reuses an existing folder.
- **Idempotency and the wire.** The value joins the ordinary create fingerprint (`create_fingerprint`, frozen tag
  `session_launch_v1`), emitting a new tag only when a value is present so value-less creates stay byte-identical, and
  stays within the create request's size bound. The protocol bump is required (a field whose purpose is to change what
  the supervisor does is not additive, per `PROTOCOL_VERSION`'s rules in `crates/farhelm-proto/src/lib.rs`). After it,
  no remote host connects until its supervisor is updated, the ordinary consequence of every bump: use the standard
  wording for that in the fragment, not a bespoke caveat.
- SPEC.md: Creation (the field), Lifecycle operations (Restart with's availability and its "resuming the session's own
  conversation" wording; Clone, Replace with and Restart with may start in a named conversation; Clone is otherwise
  fresh), and Durability and resume (the named conversation is a start argument; identity still comes from the report;
  which agents capture it afterwards; what happens to the old capture). SPEC_impl.md: the per-harness facts, the
  one-time start argv, the named Restart with path past the four gates ("Restart-with backend wire and persistence" and
  "Restart only resumes" both change), and the capture rule.
- Docs website: the pages on launching and on restarting explain the field, per agent where it differs
  (`website/AGENTS.md`, `website/EDITORIAL_RULES.md`).
- Tests: argv per offering agent (fresh start replaced by the filled resume command, stored launch unchanged);
  validation; the field's per-agent visibility and labels; Restart with available with nothing captured and requiring
  the field; capture after a named start and after a named Restart with (old capture kept until the report; cleared for
  Goose); Clone of such a session starts fresh; agents that don't offer it, command launches and new managed checkouts
  refuse a value.
- A changelog fragment (`kind: added`).
- The TODO.md entry ("Launch a session with a given conversation id") is removed.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

Out of scope: the CLI (`farhelm agent create` / `spawn` gain no flag); templates; checking that a conversation exists;
attaching Goose's reporter on resume; reading agents' conversation stores to offer a list of ids.

## Requirement sources

**The user's request (2026-10-13):** plan this Near term TODO entry. Its text:

> **Launch a session with a given conversation id.** When creating a session, let the user name the agent conversation
> it should use (what some agents call a session id), for example to continue a conversation started outside Farhelm in
> a Farhelm session. Today a session only ever resumes the conversation Farhelm captured for it.

**The user's decisions (2026-10-13):**

- In the maintainer's words: "The use case here is that for whatever reason I have a specific session ID. This is the
  exact thing that happened to me. I had a session open, I knew the session ID, but I had a version of Farhelm in an
  environment where conversation reporting wasn't working, and then after upgrading Farhelm, I wanted to restart it. I
  knew the session ID, but I had no idea to tell Farhelm to just use this session ID. So basically, I want the launcher
  to allow me to pick a specific session ID. Otherwise, it works precisely like anything else. If I'm cloning,
  restarting, replacing, or just creating a new thing, it's the same thing, no other semantic difference. Except that
  for any of these options that show the launcher dialog, there should be an option to pick my own session ID. ... And
  let's just support it for every agent type that allows us to pass a session, unless there's some complication here
  that I'm not aware of."
- The maintainer accepted the planner's statement of the semantics: one-time, the agent's report makes it the session's
  conversation, the stored launch stays ordinary so a later Clone starts fresh, and the stated caveats (no Restart
  afterwards for Muse, Cursor, OpenCode and Goose; Pi and OMP take a session file path), with each agent's flag verified
  on the real agent and any agent whose flag doesn't work left out and reported.
- On whether a named Restart with forgets the old captured conversation: "keep it if its simple but im worried it adds
  complexity around when to allow the convo to be overwritten and not, but if its reasonably straight forward keep it".
  The planning review found keeping needs no new overwrite rules (see Capture afterwards); the planner added the Goose
  exception, where keeping would leave Restart resuming the wrong conversation.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md Creation, Lifecycle operations (Restart, Restart with, Clone, Replace,
Replace with), Durability and resume (identity only from the agent's report; "A resume command is never run with
`{conversation}` unfilled"), Upgrade compatibility and client scale; SPEC_impl.md "Restart only resumes", "Launch kinds:
one resolved launch", "Restart-with backend wire and persistence"; root `AGENTS.md` Harness-specific code, Driving real
agent TUIs and `.agents/real-agent-tui.md`, The live install is off-limits, Docs website, Conventional Commits,
Finishing work, Releases and the changelog, Talking to the user; global instructions (tests never modify the test
process's own environment variables); `.agents/test-authoring.md` for every test change; `plans/AGENTS.md`, Executing
one plan.

**Planner proposals** are the outline below, the Goose exception to keeping the old capture, refusing the field for new
managed checkouts, limiting it to agent launches, and the help text for agents Farhelm won't capture.

## Outline

Line numbers drift; find the code by name. Here's what exists today:

- Create: `CreateSessionForm` builds the body for `POST /api/sessions`; the helm's `CreateReq` in
  `crates/farhelm-helm/src/sessions.rs` compiles a `SessionLaunch` with `launches::compile` and sends
  `ControlMsg::CreateSession` to the supervisor. Restart with goes through `restart_with.rs` and the helm/supervisor
  restart path (`restart_session`, `relaunch_argv` in the supervisor's `service/core.rs`).
- Resume argv: `resume_arguments(harness)` in `launches.rs`; the supervisor fills `{conversation}` through
  `IntegrationSnapshot::filled_resume_argv` in `agent_kind/mod.rs`; `verify_report_only_resume` re-checks captured
  locators for kinds where `verifies_resume_target` is true (a user's value is not a captured locator; decide whether
  any check applies and log it).
- Capture: `captured_conversation` in the supervisor store, `CaptureState` in `service/capture.rs`, admission in
  `service/core.rs` and `service/core/vendor/`.
- Clone, Replace with and Restart with compose a new launch from the stored choices; plain Replace and agent clone copy
  the stored commands. Either way the stored launch never carries the value, so all of them stay fresh unless named.

Here's the work, as a proposal:

1. **Verify on real agents** which agents can start in a named conversation and how.
2. **Backend**: the per-harness facts, the one-time start argv, the create and restart wire (fingerprint, bound,
   protocol), the named Restart with path and its capture rule, tests.
3. **Interface**: the field in the launcher (New, Clone, Replace with) and Restart with, per-agent labels and help,
   tests.
4. SPEC.md, SPEC_impl.md, docs website, changelog, TODO removal.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- through the recorder, the farhelm-proto, farhelm-helm (launches, sessions) and farhelm-supervisor (capture, restart,
  admission) tests touched, and the farhelm-ui tests for the launcher and Restart with;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, since tests change;
- Playwright on Chromium and WebKit for the launcher and Restart with specs;
- the website build; `dprint check` on changed Markdown; `python3 releasing/check-changelog.py format`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-launch-with-conversation-id-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
  `plan/launch-with-conversation-id/<nn>-<short-name>` (for example
  `plan/launch-with-conversation-id/01-named-conversation-start`).
- Expected shape: backend, then interface, each leaving main correct on its own, or together if the backend alone would
  be unreachable for long. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits; this is a new feature, so `feat`. Each such PR adds its
  changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
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
- the acceptance criteria: this file's goal, the settled semantics, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check specifically: that the stored launch never carries the user's value; that the captured
conversation is only ever set from the agent's report; that the value reaches the agent as one argument with no shell;
and that Clone of a session started this way starts fresh.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example seeding the captured conversation from
  the user's value, verifying ids against agents' stores, a new admission protocol, or a second per-harness argv table
  (examples, not a blacklist);
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
alternatives considered. In particular: each agent's verification result, version and exact command; the final list of
offering agents and their labels; the named Restart with path past the supervisor's gates; Pi and OMP path handling; how
`abort_relaunch` treats a cleared Goose capture; the stack's shape; every review finding you decided not to follow. The
user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

Agreed fallbacks:

- An agent whose way of starting in a named conversation doesn't exist or doesn't work on the real agent is left out of
  the offer and named in the report with what was tried.
- If a reporting agent's admission path does refuse a newly reported conversation after a named Restart with (the
  planning review found none that would), clear that agent's captured conversation on a named restart as for Goose, and
  record it in the report.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: writing the user's value into the captured
conversation, a CLI flag, or saving the value in templates.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
