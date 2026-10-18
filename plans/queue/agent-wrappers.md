# Agent wrappers: run an agent launch through a command prefix

Written against main at 63439c2f on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan. Two queued plans touch neighbouring code, so expect mechanical rebases if
they land first: `codex-trust-prompts` removes the launcher's workspace-trust control and its remembered value, and
`agent-quick-launch-templates` changes template fields, the template CLI and the templates listing.

## The goal

An agent launch (Claude, Codex and every other agent type the launcher composes for) can carry an optional command
prefix. Farhelm runs exactly the command it would compose for that agent, with the prefix's words in front, and treats
the session as that agent in every respect: status, conversation capture, Restart and resume, Clone, Restart with,
Replace with, the initial prompt, templates and quick launch. Today a wrapper can only be run as a hand-written custom
command (`website/src/content/docs/docs/agents/agent-wrappers.md` says "there is no wrapper mode to turn on"), which
gives up everything the agent tab composes.

TODO.md entry this plan covers (Near term): "**Official support for agent wrappers.** Allow running an agent through a
wrapper while treating it exactly like the real agent. Details TBD; the maintainer knows what they want here. Ask before
designing or implementing it."

The maintainer has now been asked; their answers are under Requirement sources and are the design.

### Acceptance criteria

Composing and storing:

- `LaunchSelection` (`crates/farhelm-proto/src/launch.rs`) gains an optional prefix, stored as a word list (like
  `permission_flags`), absent from the serialized form when unset so existing selections and launch fingerprints are
  unchanged. Adding a launch field follows the rule in the `PROTOCOL_VERSION` docs (`crates/farhelm-proto/src/lib.rs`):
  "any new launch field therefore earns the next bump". Bump it and rename its pinned test; no per-launch version
  handling and no translation layer.
- `launches::compile` (`crates/farhelm-helm/src/launches.rs`) puts the prefix words in front of both the start argv and
  the resume argv it composes, ahead of everything else including Goose's `env GOOSE_*=` words. `named_start` gets the
  same prefix, since it builds from the start argv. With no prefix, every composed argv is byte-identical to today's.
- The prefix is written as a command line split into words the way a shell would (shell-style quotes and escapes only
  decide word boundaries, as `split_permission_flags` does), with no placeholders: a word that is a Farhelm placeholder
  (`{cwd}`, `{conversation}`, `{farhelm_args}`, the Codex trust markers) is refused, as are an empty prefix and NUL.
  Refusals name the field, like the launcher's other refusals.
- Supervisor code that reads a composed agent argv to find the agent's program skips the selection's prefix words first:
  counting, not parsing. Today that is OMP's launch classification (`classify_omp_launch` in
  `crates/farhelm-supervisor/src/agent_kind/omp.rs`, reached from `record_launch_provenance` in `service/core.rs`),
  which otherwise classifies `with-endpoint --profile work -- omp ...` as `Unknown` and fails every OMP report closed.
  Check for any other reader of a new launch's argv; the `effective_program_index` users in hook injection and launch
  environment serve legacy sessions, which have no prefix. Attribution stays exactly as strict at runtime.

The launcher, templates and dialogs:

- The launcher's agent tab has a prefix field. Its value is remembered per agent type, helm-wide, after a successful
  user launch (like the launcher's other remembered choices, SPEC.md's launcher paragraph on remembered permissions),
  and "reset choices" returns it to that remembered value. Switching the agent type swaps the field to the new type's
  remembered prefix.
- Templates (`TemplateFields`, `crates/farhelm-proto/src/launcher.rs`) carry the prefix with the same three states as
  model, effort and permissions: unset (the template leaves New's remembered prefix for its agent in place), set to a
  prefix, or set to none (explicitly no wrapper). A template's prefix wins over the remembered one, so quick launch (⚡
  and the number keys) launches with it. The Templates dialog and "Save as template" show and save it.
- A session keeps the prefix it was launched with: plain Restart and resume re-run its stored argv. Restart with,
  Replace with and Clone compose from what their dialog shows, pre-filled from the session's own selection like its
  other choices, so they pick up whatever prefix is chosen there.
- The session's displayed command (session info, approval cards) shows the prefix, since it is part of the start argv.

Agents and the YOLO host guard (`crates/farhelm-helm/src/yolo_guard.rs`, SPEC.md "Agent-spawned sessions"):

- A prefix the user configured counts as part of the composed command: one typed in the launcher, the Restart with or
  Replace with dialogs, or one that comes from a template. In an agent's request, a prefix is user-configured only when
  it came from a template the request named (`--template`); a template an agent wrote through
  `farhelm agent template create|edit` was approved on a card showing the whole template, so it counts.
- Every other prefix in an agent's request is agent-supplied and untrusted: one set by the request's own flags, and one
  carried into the launch by `farhelm agent clone` or `farhelm spawn --inherit-agent` from a source session (the source
  supervisor reports that launch, and the helm cannot vouch for where its prefix came from).
- On a host that asks before YOLO launches, an agent-supplied prefix needs the user's approval: such a launch always
  goes to the approval card, even when the host's "run farhelm commands from this host without asking" setting is on,
  and the card shows the prefix as the agent's own, which Farhelm cannot check. That card offers Allow and Deny only
  (Always allow would turn on the host's command setting, which does not stop these cards). A launch whose permission is
  YOLO stays refused outright there, as today. A host whose "start YOLO sessions here without asking" setting is on has
  no guard, so its launches get no forced card.
- `check_agent` runs before the card, after approval and at dispatch, so the guard gains a third verdict ("vouched only
  with an approval") that an approved request satisfies at the later two checks.
- `farhelm agent create` and `farhelm spawn` take the prefix as a flag; `farhelm agent template create|edit` can set it.
  The agent instructions (`crates/farhelm/src/agent_instructions.rs`) describe the flag and that an agent-supplied
  prefix needs the user's approval on hosts that ask before YOLO. The instructions tests check verbs, not flags; add a
  flag assertion if it is wanted.

Docs and bookkeeping:

- `website/src/content/docs/docs/agents/agent-wrappers.md` leads with the prefix as the supported way to wrap an agent,
  keeping the custom-command route for wrappers that need more than a prefix (a `{cwd}` argument, a rewritten resume
  command). Keep its existing sections on what a wrapper must pass through and what happens on stop, and extend the
  per-agent rules: Claude and Grok accept an `exec`'d wrapper or one resident level (Grok's `procs/grok.rs` skips the
  pane process itself, which is the wrapper); Codex accepts any ancestry above the innermost `codex`; OMP needs the
  wrapper to `exec` (its corridor allows no unknown link). It also says not to put secrets in a prefix: the prefix is
  part of the launch's argv (visible in the process table) and agents on every host can read a template's prefix in the
  templates listing. The launcher, templates and agents docs pages that list launch fields mention the prefix.
- SPEC.md (launcher, templates, Agent-spawned sessions, the conversation-capture paragraphs that already describe
  wrappers) and SPEC_impl.md describe the prefix. One `feat` changelog fragment (`kind: added`) covers the whole plan.
- The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-17), in their words where they gave them:**

- What it is: "we want the user to basically say, 'Okay, trust me, bro.' Just run whatever you would normally run with
  this prefix, and I am telling you to trust me that all this does is run the agent, and anything else it does is not
  relevant to Farhelm's functionality, and it is up to the user to assert that. It simply means that we prefix the
  command we would otherwise run for a given agent type."
- Why: "A real-life example that prompted me actually wanting to add this is command line tools that wrap an agent
  invocation with a wrapper that installs API endpoint settings changes, for example. The point is that we don't want to
  push people to having to use a custom command just because of that, so that they can actually have Farhelm treat it as
  a Codex or a Claude, etc., but still apply their own wrapper. Simple prefix wrapping is assumed. Nothing more
  complicated than that."
- Where the wrapper goes: in front of the command Farhelm composes; Farhelm's flags pass through unchanged.
- Where it is configured: "Per launch, and of course in templates, which also means also in the quick launch flow."
- YOLO host guard: a wrapper the user configured counts as part of the composed command, so agents may start such a
  launch on a host that asks before YOLO (subject to the launch's own permission, as today).
- Restart: the session keeps the prefix it was launched with; Restart with and Replace with pick up the current choice.
- The launcher pre-fills the last prefix used with that agent, like its other remembered choices.
- Agents may pass a prefix through the `farhelm` command, "but a prefix the agent typed is not trusted: on hosts that
  ask before YOLO it needs your approval; a prefix that comes from a template is trusted."
- The prefix is "a command line split into words the way a shell would, no placeholders", e.g.
  `with-endpoint --profile work --`.
- The templates listing agents see shows a template's prefix text; the docs say not to put secrets in a prefix.
- Upgrade path: the user accepts that this plan's helm database schema bump and protocol bump leave no downgrade path
  (SPEC.md, "Upgrade compatibility and client scale", asks for that alert; this is the answer). The upgrade path itself
  must stay intact.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:**

- SPEC.md "Agent-spawned sessions", the YOLO host guard paragraph: an agent may only start, on a host that asks before
  YOLO, what runs the way the user already approved; an agent launch is vouched for only when its command lines are
  exactly what this Farhelm composes for its choices (`yolo_guard::check_agent` recomposes and compares). A prefix in
  the selection keeps that comparison exact but does not by itself say who chose the prefix, hence the provenance rule
  above.
- `LaunchRequest` (`crates/farhelm-proto/src/session_launch.rs`): an agent never supplies composed argv. The prefix is a
  choice in the selection, not argv the agent hands over.
- Conversation capture's process rules stay as strict as they are; the prefix does not relax them (SPEC.md, the Claude,
  Grok and OMP capture paragraphs; the native Codex executable must still be named `codex`; `executable_basename` in
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` still feeds the process-tree checks). Farhelm's arguments
  (`{farhelm_args}`) and its environment variables must reach the agent.
- `PROTOCOL_VERSION` docs: a new launch field earns the next bump; there is no cross-version compatibility, and a
  mismatched peer is refused at the hello.
- Root `AGENTS.md`, "Code rules": the prefix applies to every agent type the same way; no `kind == X` or `matches!` over
  harnesses in shared code for it. The OMP classification change lives in OMP's own module.
- Root `AGENTS.md` (Tests and validation, Changelog fragments).

**Planner proposals (checked by a fresh-context planning review):**

- One optional field on `LaunchSelection`, so the prefix rides every path that already carries a selection (history,
  recent setups, Clone, Restart with, approvals, the YOLO guard's recomposition) instead of a parallel mechanism.
- Remembered prefix: one helm-wide map from agent type to its last prefix on the `preferences` row (a helm database
  schema bump), written after a successful user launch (an agent's launch does not change it), and cleared for that
  agent type when the user launches it with an empty prefix.
- Provenance in an agent's request is one boolean computed where the helm resolves the request: `agent_launch.rs`
  applies the named templates and then the request's flags as one more template, so the prefix is agent-supplied exactly
  when the flags set it (or when it was copied from a source session by clone or `--inherit-agent`). An agent that
  passes the same prefix a template already set takes the agent-supplied path, which is harmless.
- The guard rule for clone and `--inherit-agent` lands in PR 1, because PR 1 already lets selections carry prefixes into
  clones; until PR 2 adds the forced card, PR 1 refuses agent-supplied prefixes on hosts that ask before YOLO. That
  refusal is a temporary state inside this stack, not a delivered behaviour.
- CLI and field naming: `--wrapper <command line>` on `farhelm agent create`, `farhelm spawn` and template writes; the
  launcher field labelled "wrapper prefix". Rename freely if the codebase's vocabulary suggests a better word; log it.

## Outline

Two `feat` PRs:

1. `feat: run agent launches through a wrapper prefix`. The `LaunchSelection` field and its validation; the protocol
   bump; `compile` and `named_start`; OMP's classification skipping the prefix; the launcher field with its remembered
   per-agent value, "reset choices" and the agent-type switch; templates (proto field with its three states, store,
   Templates dialog, Save as template, applying, quick launch); Restart with, Replace with and Clone carrying it;
   display; the guard treating a cloned or inherited prefix as agent-supplied (refused on hosts that ask before YOLO);
   SPEC.md and SPEC_impl.md for this part; the rewritten `agent-wrappers.md` and the other docs pages; the changelog
   fragment. If this PR grows large, split the launcher and template UI into its own PR on top of the compose change,
   keeping each PR a working state.
2. `feat: let agents launch through a wrapper prefix`. The `--wrapper` flag on `farhelm agent create`, `farhelm spawn`
   and template writes; provenance of a template prefix; the guard's third verdict and the forced Allow/Deny card for an
   agent-supplied prefix, replacing PR 1's refusal; the agent instructions; SPEC.md "Agent-spawned sessions". It edits
   PR 1's fragment rather than adding a second one, and removes the TODO.md entry.

### Validation

Choose per `.agents/validation.md`; at least:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, `dprint check`.
- Unit tests through the recorder: `launches.rs` (start and resume argv with and without a prefix for every harness,
  Goose's `env` order, `named_start`, refusals), OMP's classification with a prefix, `yolo_guard.rs` (a user prefix
  vouched; a flag-set, cloned or inherited prefix not; a template prefix vouched; the approval-only verdict satisfied by
  an approval at the later checks; YOLO still refused), the store's migration and remembered-prefix tests, proto serde
  round trips including an absent prefix staying absent and a template's explicit "no wrapper", CLI parsing.
- The Rust e2e wrapper test (`crates/farhelm/tests/e2e/wrapper_launch.rs`) extended with an agent launch whose prefix is
  an `sh` wrapper (stand-in, as the existing test does), checking that the argv reaches the stand-in agent unchanged,
  that Farhelm's arguments and environment arrive, and that Restart re-runs the prefixed resume command.
- Browser tests on Chromium and WebKit through the recorder for the launcher field, its remembered value and the
  agent-type switch, and a template carrying a prefix (the existing launcher and templates specs are the place). Not the
  full suite.
- `python3 releasing/check-changelog.py format`; `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`
  when Rust or browser tests change.

## Fallbacks

- If the forced approval card for an agent-supplied prefix turns out to be infeasible, that is a block per
  `plans/.agents/executing.md` (Executing one plan, step 10), not a refusal: the user said such launches need their
  approval, and refusing them would change that. Push PR 1 (with its interim refusal) and block on PR 2.
- If a harness's capture rules make a resident wrapper lose capture beyond what the docs rules above say, document the
  requirement (exec or one level); do not relax the attribution rules.

Examples of departures that need the scope reassessment: placeholders inside the prefix, a per-host or global wrapper
setting, a wrapper registry or named wrappers, parsing the wrapper to find the agent (counting the stored prefix words
is not parsing), relaxing a capture rule for a declared wrapper, any per-launch version handling.

Decisions to log: the field and flag names, where the remembered prefix lives, how provenance is computed, the guard's
third verdict and where an approval satisfies it, the template field's three states.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-agent-wrappers-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
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
  `plan/agent-wrappers/<nn>-<short-name>` (for example `plan/agent-wrappers/01-launch-prefix`).
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
