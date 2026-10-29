# Feature: session tracking for Muse Code, on the Codex and Claude model

Written against main at da9d7411 on 2026-10-28. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where a decision below names the SPEC.md text it changes.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Session tracking for Muse Code.** Add what Muse Code still lacks beyond being a launchable terminal agent:
> per-launch hooks/instructions, capture of the correct conversation identity for resume, and recognition of its
> waiting/status signals. The built-in `muse` and `muse-yolo` profiles currently run with generic activity status and no
> hooks or conversation resume; these are Farhelm integration gaps, not established limitations of Muse.

The maintainer's words when choosing it for planning (2026-10-28):

> I just want session tracking to work. I don't remember exactly why we didn't add the support. I think it was just,
> "Hey, we don't need it yet," and I'm just adding basic support. I don't remember there being a huge blocker. Let's
> start by investigating what is needed to actually have Codex- and Claude-style session tracking that actually works
> here. Let's try to keep it as close as possible to the same model we're using for Codex and Claude, in terms of not
> breaking if there's a wrapper and stuff like that, the overall model being as similar as we can

Today Muse is a launch harness that runs as the generic agent kind (SPEC.md, the paragraph starting "Muse support uses";
the harness map in the module docs of `crates/farhelm-supervisor/src/agent_kind/mod.rs`). It has a workspace-trust
reader and a permission-footer reader in `agent_kind/screen_reader.rs`, no resume selector in the helm's
`resume_arguments` (`crates/farhelm-helm/src/launches.rs`), and no hooks. The helm's one-off named start already uses
Muse's `resume` subcommand (`LaunchHarness::Muse => &["resume"]` in `named_start`).

### What planning found about Muse

Checked on the planning machine with Muse Code 1.4.14 from help output and the binary's strings only; no session was
started. Treat these as leads to confirm, not facts:

- `muse` on the PATH is a vendor launcher shell script that updates itself (hourly by default) and `exec`s a versioned
  binary beside it (`muse-bin-<version>`), deleting older binaries after an update. The executable's name therefore
  changes with every release and can vanish under a running session.
- `muse resume <session-ref>` resumes by exact session UUID or session name; root options such as `--permission-profile`
  may appear on either side of `resume`. `muse resume --last` and the bare picker select by recency, which Farhelm's
  capture rules never accept.
- The binary carries a hooks system: event names `SessionStart`/`session_start`,
  `UserPromptSubmit`/`user_prompt_submit`, `Notification` and `turn_end`, a `hooks.json`, and settings named
  `hooks.admissions`, `hooks.allowed_sources`, `hooks.allowed_env_vars`, `hooks.denied_identities` and
  `hooks.fallback_enabled`. It also has plugin bundles with hooks (`muse plugins install|approve|hook test|validate`), a
  per-run `-c KEY=VALUE` setting override ("unknown or clashing keys refuse the run"), and an `--agents <JSON>`
  ephemeral overlay. Whether a per-run route can attach a hook is unknown: the admission settings suggest Muse may only
  run hooks from admitted, approved sources.
- `--provider echo` is a deterministic offline provider. If it starts without provider credentials, it may answer most
  of the hook questions (which events fire, payload shape, process tree, stdout handling) without spending real turns.

### Acceptance criteria

- **Investigation first.** Before building, establish from the installed Muse (help, `muse schema` exports, plugin
  validation, and runs per `.agents/real-agent-tui.md`, with the echo provider first where it is enough) how a hook is
  attached, what each relevant event's payload carries (the session UUID in particular), which events fire on start,
  resume, clear or new session and end of turn, whether Muse feeds a session-start hook's stdout to the model, whether
  Muse draws anything on screen when a hook runs, whether sub-agents or sub-sessions in the same process fire the same
  events (and with what marker), and where the hook process sits in the process tree. Record the findings in the working
  log as DECISION entries with the version and the evidence, and carry the ones that hold into SPEC_impl.md and the
  website (see Docs).
- **A rule for stale agent versions, in SPEC.md.** SPEC.md has no rule today about which versions of an agent harness
  Farhelm supports. Add one, for every agent type, in the maintainer's words: "we don't care about agents after the
  later version has been out for 2 weeks. Things move fast in this space. We do not add complexity to support stale
  harnesses." That is: Farhelm supports an agent harness version until a newer release of it has been out for two weeks,
  and adds no code or complexity for older ones. Put it beside the supported login shells and tmux paragraphs (SPEC.md,
  near "Confirmed 2026-09-28: bash and zsh are the supported login shells"), in the same style, and point to it from the
  Muse paragraph. It changes no existing agent's code.
- **Muse is its own agent kind.** It gets its own integration the way Grok, Goose, Pi and OMP did, in the places the
  harness map names, instead of running as generic. A new `AgentKind` variant is a wire change: bump the protocol
  version as Grok's kind did (see `protocol_version_is_pinned_at_64` in `crates/farhelm-proto/src/lib.rs`), and extend
  the browser's `SessionAgentKind` mirror (`crates/farhelm-ui/src/lib.rs`) and the command-launch declared-type choices.
  `AgentKind::restart_readiness` and `display_name` get Muse entries, so Restart wording names Muse.
- **Hooks reach Muse the Grok way when they have to.** Prefer a per-launch route (a command-line hook, `-c` setting or
  similar) that writes nothing Muse owns, as Claude's and Codex's hooks are (SPEC.md, "The resume promise is
  per-session", the paragraph's "A hook passed on the command line for one launch is allowed" sentence). If Muse only
  runs hooks that are installed or approved in its own configuration, do it as Grok does: document a Farhelm plugin or
  hook entry that the user installs, and Farhelm itself never writes, edits, approves or removes it (SPEC.md, "Grok is
  the documented opt-in exception"). Then Muse becomes the second documented opt-in exception, in that sentence and on
  the website's Muse page, and a launch on a host where the user has not installed it runs untracked, as a Grok launch
  without its hook entries does. If the per-launch route is `-c`, carry over Codex's skip for an invocation that already
  configures hooks (`agent_kind/codex.rs`) so a user's own setting does not clash.
- **Instructions announcement.** If Muse feeds a session-start hook's stdout to the model, as Claude and Codex do
  (`crates/farhelm/src/hook.rs`, the `POINTER_LINE` docs), the session-start report carries Farhelm's agent instructions
  announcement as theirs does. If not, omit it as Grok does, with the same SPEC sentence form, and say so in the report.
  Do not build another delivery path (an `--agents` overlay, say) without a scope reassessment.
- **Conversation capture on Claude's model.** The conversation comes only from Muse's own explicit report, never from
  Muse's session files. Muse uses Claude's positional attribution (SPEC.md, "Claude attribution is positional";
  `procs/claude.rs`), because its executable has a versioned name that disappears on update, the reason Claude is
  positional. Reuse Claude's corridor, the shared report pin and Claude's admission rules (pin at any depth, ancestor
  correction with a once-per-launch notification, dead-pin release, relaunch clears, ownership version 0) behind the
  harness map's per-kind dispatch; refactoring shared code without changing Codex's or Claude's behavior is allowed. A
  resident wrapper prefix or launcher at any depth keeps capture.
- **No exact-record check unless a hazard needs one.** Add a check against Muse's session files only if the
  investigation shows that sub-agents or sub-sessions in the same process fire the subscribed events without a child
  marker that the common report path already refuses (the reason Codex has one: "threads may share a process",
  SPEC_impl.md, "Codex attribution and exact-record validation"). Otherwise follow Claude: no check, and a SPEC.md
  sentence saying so beside Claude's. A check, if needed, reads only the exact file a report names, through
  `agent_kind/records.rs`, never Muse's live `session-index.db`.
- **Restart resumes the conversation.** `resume_arguments` gives Muse the exact `resume <uuid>` form the investigation
  confirms, with the permission flags kept, so Restart and Restart with resume the captured conversation. Keep the
  `named_start` arm for older stored launches that have no resume command, with a comment saying why. The Restart
  dialog's conversation field keeps SPEC.md's "conversation id" label for Muse.
- **Waiting and working status from a Muse screen reader**, as Codex and Claude have: a dedicated reader of what Muse
  itself draws (its busy indicator, its composer at rest, its question dialogs), held to real captured screens, with the
  generic behavior for screens it does not recognize. No status from hook events. The existing Muse trust and
  permission-footer readers keep working. If Muse's screen cannot show waiting reliably, say so in SPEC.md the way OMP
  and Grok are described, rather than adding a hook channel. Captures follow `docs/agent-screen-fixtures.md`;
  `scripts/capture-agent-screens.py` drives only Claude and Codex today, so either extend it (a script change: update
  the inventory in `.agents/validation.md` and the lists in `.agents/run-all-tests.md` in the same change) or capture by
  hand per `.agents/real-agent-tui.md`, logged as a DECISION.
- **Invisible to the session.** Whatever Farhelm attaches to a Muse launch obeys SPEC.md's "Anything farhelm attaches to
  a launch ... must be invisible" paragraph; anything Muse itself prints about hooks is documented there as a
  vendor-printed exception, the way that paragraph handles others.
- **Muse versions:** a Muse older than the stale-version rule allows is outside the "still launches" promise, and the
  website's Muse page names the minimum version the integration needs. A supported Muse that cannot report (hooks
  refused at launch, the user's plugin not installed) still starts and runs as today, with generic status where the
  reader cannot tell, and Restart shows the same "conversation not captured" explanation other agents show.
- **Existing sessions:** existing Muse agent launches become the Muse kind (the kind derives from the stored harness in
  `IntegrationSnapshot::of`) and get hooks at their next start through their stored `{farhelm_args}`, but keep no resume
  command, so Restart explains "no resume command" until they are recreated. That is the expected outcome; no backfill
  or migration (Claude's precedent: no upgrade machinery compensates for that gap). Check that SPEC.md's "Upgrade
  compatibility and client scale" accepts it and that the wording is right.
- **Tests** at the layers the Claude wrapper-capture and Grok work used: corridor (`procs.rs`), admission
  (`service/core.rs`), store only where the pin's storage changes, the reporter's payload parsing
  (`crates/farhelm/src/hook.rs`), the helm's argv compiler, the screen readers against captured fixtures, and end to end
  with a fake Muse fixture modeled on the fake Claude one (wrapper depth, capture, resume, a rival report refused, an
  ancestor's correction).
- **Docs:** SPEC.md's Muse paragraph (drop "Muse-specific hooks, conversation capture/resume, and waiting-state
  recognition are not implemented"), the resume-promise paragraph if Muse becomes an opt-in exception, the Claude
  attribution paragraph if Muse shares it, the invisibility paragraph, the new stale-version rule, and every SPEC.md
  list that names which agents report, resume or label a conversation (search for "Muse"); SPEC_impl.md's per-launch
  hook text, ownership and notification sections; the website pages `agents/muse.md`, `agents/agent-wrappers.md` and
  `how-it-works/agent-hook-injection.md` under `website/src/content/docs/docs/` (read `website/AGENTS.md` first); doc
  comments in every file the change touches.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: added`),
  mentioning the minimum Muse version and, if it applies, the plugin the user installs.
- The last code PR removes the TODO.md entry quoted above.

Out of scope: Muse's own cross-session messaging (`muse session-message`), and the switched-off feature that lets agents
prompt running sessions.

## Decisions already made

Settled with the maintainer on 2026-10-28, on the planning page, in two rounds:

- **All three parts, as one stack:** conversation capture with Restart resuming it, waiting/working status, and
  recognition of Muse's questions on screen.
- **Keep the model as close to Codex and Claude as possible**, including capture through wrappers (the maintainer's
  words above).
- **The Grok way for any hook Muse only runs from its own configuration:** a documented plugin or hook entry the user
  installs; Farhelm writes, edits, approves and removes nothing in Muse's configuration. The maintainer chose this over
  an automatic install after being told it would otherwise need a SPEC.md exception and would make every Farhelm
  supervisor on a machine share one Muse plugin. A per-launch route that writes nothing Muse owns stays the first
  choice.
- **A minimum Muse version, no version probing.** A Muse too old for Farhelm's per-launch setting is outside the
  no-refusal promise. In the maintainer's words: "SPEC.md SHOULD already say something like we don't care about agents
  after the later version has been out for 2 weeks. Things move fast in this space. We do not add complexity to support
  stale harnesses." SPEC.md does not say it yet, so this plan adds it (acceptance criteria).
- **A supported Muse that cannot report still launches as today**, with the usual "conversation not captured"
  explanation, rather than being refused.
- **Real Muse runs are allowed**, spending real Muse turns, to verify hooks end to end and to capture Muse screens for
  status recognition.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

A fresh-context planning review settled these planner choices, shown to the maintainer on the planning page without
objection: status comes from a screen reader only; attribution is Claude's positional model; no exact-record check
without the same-process hazard; the announcement is omitted if Muse ignores hook output; the Restart label stays
"conversation id"; existing sessions get no migration.

## Requirement sources

**The user's request:** the TODO.md entry and the maintainer's words quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md and SPEC_impl.md, in particular the sections named under Docs, the capture
rules ("Screen reading is allowed to be wrong; capture is not" in the `agent_kind` module docs, and "The resume promise
is per-session" in SPEC.md), the invisibility paragraph ("Anything farhelm attaches to a launch"), and "Upgrade
compatibility and client scale"; root `AGENTS.md`, "Code rules" (read the harness map in the module docs of
`crates/farhelm-supervisor/src/agent_kind/mod.rs` first; per-harness behavior goes where it says, never `kind == X` or
matches over harnesses in shared code); `.agents/real-agent-tui.md`; `docs/agent-screen-fixtures.md`;
`website/AGENTS.md` for the docs pages; root `AGENTS.md`, "The live install is off-limits", "Tests and validation",
"Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`;
`docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code or the investigation says otherwise, logged as a DECISION):

- Precedents: the Claude wrapper-capture plan's report (`plans/reports/claude-wrapper-capture.report.md`, PRs #2719,
  #2725 and #2727) for the positional corridor, the shared pin and the end-of-turn hook; the Grok capture work
  (`git show --stat 8655cf6f`) for adding a whole integration and a user-installed hook. That commit predates the move
  of `docs/agent-hook-injection.md` and `docs/harnesses/grok.md` into `website/src/content/docs/docs/`.
- The narrowest hook set that meets the criteria: session start (capture and the announcement) and end of turn (so a
  wrongly pinned child is corrected at the parent's next turn), Claude's `SessionStart` and `Stop`.
- For verification runs, prefer the echo provider and a private Muse configuration and data directory when Muse lets one
  be set without copying the maintainer's credentials. Never install, approve or edit anything in the maintainer's own
  Muse configuration; if testing the user-installed route needs a plugin installed, do it only in a private
  configuration directory, and if Muse allows none, test with the plugin's validation and hook-test commands and say in
  the report what could not be exercised end to end.

## Outline

The investigation decides the details; adjust the split if it shows a better one, logged as a DECISION, and keep the
stack free of churn.

1. Investigation (no PR of its own): the findings, recorded in the working log.
2. `plan/muse-session-tracking/01-stale-agent-versions`: the SPEC.md rule for stale agent versions (`docs:`).
3. `plan/muse-session-tracking/02-muse-kind`: Muse becomes its own agent kind with the protocol bump, the browser mirror
   and the declared-type choice, every per-kind decision answered as generic behaves today, so nothing visible changes
   yet.
4. `plan/muse-session-tracking/03-muse-capture`: the per-launch hook or the documented user-installed entry, the
   reporter's Muse payload parsing, positional attribution with the shared pin, admission, the resume selector, Restart
   readiness, the announcement, tests at every layer, SPEC/SPEC_impl and website pages, and the changelog fragment.
5. `plan/muse-session-tracking/04-muse-status`: the Muse screen reader with captured fixtures, tests, the docs that
   describe status, and the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
the crates you touch, `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares,
and, through `python3 scripts/record-test-run.py`, the supervisor's corridor, admission, store and screen-reader tests,
the hook binary's tests, the helm's launch compiler tests, the proto tests for the protocol bump, and the identity and
wrapper end-to-end tests for Muse plus the Claude ones, since shared code moves. Read the retained output: an end-to-end
test that returned early on missing systemd is not evidence.

## Fallbacks

- If no route (per-run, or a documented entry the user installs) gets Muse to run Farhelm's reporter, or no event
  carries the exact session id, block with the findings: capture by any other means is outside the capture rules. The
  stale-version rule, the agent kind and the screen reader can still be built and pushed first, since they stand alone.
- If the investigation shows hooks run in a separate, short-lived Muse process (a plugin host, say), so the positional
  emitter is a per-hook process that is gone by the next report, that is a departure from Claude's model: run the scope
  reassessment before designing around it.
- Other examples of departures that need the scope reassessment: discovering a conversation from Muse's session files
  rather than verifying a reported one; recognizing Muse by executable name; status from hook events; an exact-record
  check without the same-process hazard; changing Codex's or Claude's behavior beyond a no-behavior-change refactor of
  shared code; a Muse-specific store column when the existing pin storage serves.

Decisions to log: the investigation's findings (Muse version, command lines, quoted payload and screen facts), which
hook route was chosen and why, how reports are judged, the minimum Muse version and why, how existing sessions behave,
every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-muse-session-tracking-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/muse-session-tracking/<nn>-<short-name>`,
  as the outline names them.
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
Wording in this file that turns out inaccurate (a line number, a file name, a Muse fact from planning) is corrected to
the truth.

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

## Decisions

### 2026-10-29: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-10-29 (claim 1138ff).
>
> ### Muse questions need a supported test provider or a scope decision
>
> Muse sessions previously had no conversation resume and only basic working/idle status. The agreed scope includes
> resuming the right conversation, showing work and rest, and noticing questions Muse asks during a conversation. The
> draft stack now implements resume through wrappers and reads Muse's real working, idle and directory-trust screens. It
> does not yet recognize questions or approval dialogs during a conversation, so the TODO remains open.
>
> The private Muse 1.4.14 runs verified hooks and those screens with the offline echo provider. No supported way to make
> echo ask a structured question was found. The private authenticated-provider setup refused for missing credentials; an
> attempt through a custom localhost endpoint stopped at an undocumented model-catalog response before any question
> could run. Extending that probe would mean guessing the provider protocol. No operator credentials or saved Muse
> configuration were copied or changed. This does not establish a limitation in Muse's question screens.
>
> Which supported provider setup should the executor use to capture a real question and its dismissal without signing in
> or changing operator configuration? The recommended option is an already usable authenticated test provider, or a
> documented provider fixture protocol: that preserves the requested question recognition and needs only normal Muse
> screen captures. The alternative is to accept the current resume/work/idle/trust support as a smaller scope and defer
> conversation questions and approvals; those can still look idle while Muse waits for an answer. That narrows the
> explicit all-three-parts decision and needs approval before this plan can be called complete.

The maintainer's answer:

Answered by the landing monitor on the maintainer's behalf under `plans/.agents/landing.md`, "Answering blocks".

Decision: accept the smaller scope now and defer question recognition.

- Finish and deliver what is built: Muse conversation resume through the wrappers, and working, idle and directory-trust
  status from Muse's real screens. Do not sign in, use operator credentials or saved Muse configuration, or reverse
  engineer the provider protocol to reach a question.
- Recognizing questions and approval dialogs during a Muse conversation is deferred. Until then such a session can look
  idle while Muse waits for an answer; say so in SPEC.md where Muse's status is described, and in the report.
- Narrow the plan's TODO.md entry to the deferred part instead of removing it, or replace it with an entry for that part
  alone, noting that it needs a provider that can ask a real question (an authenticated test provider or a documented
  fixture protocol).

This judgment call is recorded as a near-term TODO.md entry so the maintainer can revisit it; a later change of mind is
a follow-up, not a reason to stop this plan.

### 2026-10-29: answer to a blocked question

The question, as the executor put it:

> Blocked while landing on 2026-10-29 (claim cef6c2).
>
> ### Landing stopped: three existing tests still expect Muse to have no conversation tracking
>
> The landing monitor did not merge this plan. None of its four PRs (#2785, #2786, #2788, #2789) has merged, and main
> holds nothing of it. The plan gives Muse sessions conversation resume and working, idle and trust status; recognizing
> Muse's questions was deferred by the previous answer.
>
> A fresh-context review found the previous answer carried out, the per-agent code rules followed, no database change,
> the protocol version raised for the new Muse values, and Claude's behavior unchanged. The source checks, the website
> build, the UI's JavaScript tests and the release web build passed. Three tests that existed before this plan still
> expect Muse to behave like an agent Farhelm does not track, and fail now that it is tracked:
>
> - In the whole Rust workspace (`cargo nextest run --workspace --exclude farhelm-desktop`, run
>   `61bf0e8f-7e59-4c60-9a2e-1213e0c05264`), the end-to-end test "structured launches forward to ready processes and
>   survive a fresh generation" expects a Muse session's restart offer to be "no conversation reporting" ("Muse remains
>   Generic; this fixture must not invent resume support") and gets "no resume command". Its other failure in that run
>   was the known terminal flow-control flake.
> - The UI's Rust test "restart with availability explains each unavailable case" (`cargo nextest run -p farhelm-ui`,
>   run `4b274466-8d9b-49d7-84f0-088c79181d41`) expects Restart with to say "Muse sessions can't be resumed" and gets
>   "No captured conversation to resume". The review had already noted that for a Muse session without a resume command
>   (one created before this change, say) that new message is inaccurate: the real reason is the missing resume command,
>   and the way out is Clone or Replace with.
> - The browser test "named conversation labels and reporting help follow the selected agent" (`npx playwright test`
>   with `tests/named-conversations.spec.ts`, run `9f76f468-811e-4b98-9635-6cb2b6d1d86c`, both engines) expects the
>   launcher's help that "Farhelm will not capture" a Muse conversation, which is now gone.
>
> The same browser run had one more failure, a WebKit focus check in the Restart with tests, which also failed once at
> an earlier landing on another plan and passed on rerun there; it is not this plan's.
>
> ### What should happen to this plan?
>
> - **Send it back to its executor (recommended).** Answer with: update these three tests to Muse's new, tracked
>   behavior; make Restart with's explanation for a Muse session without a resume command say that, the way it already
>   does for a command launch without one; run the whole Rust workspace and the named-conversations and restart-with
>   browser specs on both engines.
> - **Land it now and fix the tests afterwards.** Main would carry three failing tests and an inaccurate Restart with
>   message until then.

The maintainer's answer:

Answered by the landing monitor on the maintainer's behalf under `plans/.agents/landing.md`, "Answering blocks": a
landing that failed mainly because tests fail goes back with an instruction to fix them.

Decision: send it back to its executor.

- Give the end-to-end fixture's Muse launches (the fake agent launch in the farhelm end-to-end tests) the resume command
  the helm compiles, and assert what a tracked Muse session is offered in "structured launches forward to ready
  processes and survive a fresh generation"; drop the "Muse remains Generic" premise.
- Rebuild the Muse case of the UI test "restart with availability explains each unavailable case" around a tracked Muse
  session without a resume command.
- When a Muse session has no resume command (one created before this change, for example), Restart with explains that it
  has no resume command and that Clone or Replace with starts it again; update the stale comment beside that code.
- In "named conversation labels and reporting help follow the selected agent", expect no capture warning for Muse and
  measure the warning's width with an agent that still shows it.
- Run the whole Rust workspace and the named-conversations and restart-with browser specs on both engines.

No judgment call beyond the earlier decisions is made here, so no TODO entry is recorded.
