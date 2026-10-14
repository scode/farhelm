# A "custom" permission choice: the user's own permission flags, always treated as YOLO

Written against main at e6c80c6c on 2026-10-13. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan runs after `guarded-permission-mode.md`: it may only start once that plan's work is on main. That plan renames
the non-YOLO launch to **guarded**, forces each agent's guarded mode with explicit flags, adds a stored `Guarded`
permission value with a protocol bump, and introduces the grey "agent's own setting" shield. Read what landed (SPEC.md's
permission sections, `crates/farhelm-proto/src/launch.rs`, `crates/farhelm-helm/src/launches.rs`, the launcher's
permission controls) before starting, and build on it; where this file's description of that work differs from what
landed, what landed wins.

## The goal

A third permission choice next to **guarded** and **yolo**: **custom**, where the user types the agent's permission
flags themselves. Farhelm passes those flags instead of its own permission flags for that agent, does not check what
they do, and treats every custom session as YOLO.

The maintainer approved this mockup as is: https://snippets.scode.org/s/farhelm-custom-permission/ (the launcher's
custom choice and its flags field, the row's shield and hover, the reason on the ask-before-YOLO confirmation, and the
templates field). Treat its wording as the starting point.

### Acceptance criteria

- **Which agents.** Custom is offered for every agent whose permission Farhelm sets with command-line flags: Claude,
  Codex, Grok, Muse, Cursor and OMP (as of planning; check against what the guarded plan landed). Not Goose (its modes
  are an environment setting), OpenCode or Pi (no permission flags). The offer goes through the per-harness tables in
  `crates/farhelm-proto/src/launch.rs`, never a `kind == X` branch (root `AGENTS.md` "Harness-specific code").
- **The flags.** The user types them in one field. One shared function in farhelm-proto splits the text into words the
  way a shell does (`shell_words::split`: quoting and backslash escapes, no expansion of any kind; note it treats a `#`
  starting a word as a comment, and say so in SPEC_impl.md) and refuses empty flags (no words) and an unterminated
  quote. The launcher and the helm both call it, so they cannot disagree. An empty custom choice would mean "the agent's
  own setting", which is a different thing. The helm also refuses any word that is a reserved placeholder, reusing
  `is_reserved_placeholder` from `crates/farhelm-supervisor/src/agent_kind/mod.rs` as `validate_model_id` does (it
  covers all five the supervisor substitutes, including the two Codex trust markers); no live UI-side placeholder check
  is needed.
- **The preview (maintainer decision after planning review).** The launcher shows the split words with a truthful label
  (for example "passed to Claude in place of Farhelm's permission flags"), not the mockup's "runs <command>" line: the
  browser never sees the composed command, and the mockup's line omits model, effort and trust flags. The exact command
  shows after launch in the row's hover ("Launched with").
- **Where they go.** The words go in the single permission slot the guarded plan's `compile()` uses for the guarded and
  YOLO flags (if those land in different places, use where the YOLO flag goes, and log it), in both the start and the
  resume command. Codex's `resume` follows them by construction, since the resume command is the start argv plus the
  resume suffix; pin it with one test. Nothing else in the command changes.
- **Stored shape.** A unit `Custom` permission value plus `permission_flags: Option<Vec<String>>` on the selection, with
  one proto check that the words are present exactly when the permission is Custom. The new field is declared
  `#[serde(default, skip_serializing_if = "Option::is_none")]` like `workspace_trust`, so it falls into the create
  fingerprint and the `agent_create_v1` digest by serialization and leaves existing digests unchanged: do not add a tag
  or touch those frozen encodings. The size bound is already counted. Clone and Restart with re-display the words with
  `shell_words::join` (the user's original quoting style is not kept; store words only, not the text).
- **Carried.** Plain Restart, plain Replace, agent clone and resume copy the stored argv, so they need no new backend
  path. Clone, Replace with and Restart with compose from the selection, which carries the words, and let the user edit
  them. Templates store and apply them (the templates dialog's permission field gains custom with a flags input, as in
  the mockup).
- **Shared launcher rules** (`crates/farhelm-proto/src/launcher.rs`, which the GUI and `--template` apply alike):
  switching the agent type drops custom and its words, following `reconcile_harness_selection`'s fallback for
  unsupported permissions; a later stacked template that sets another permission clears the words; a template that sets
  words without custom is refused.
- **Vocabularies `Custom` leaks into.** Adding it to `LaunchPermission::ALL` reaches: the CLI's `parse_permissions`
  (`crates/farhelm/src/main.rs`), which must refuse `custom` with a clear message (no CLI form, see out of scope); the
  remembered default permission (`record_launch_history` and `is_known_remembered_permissions_word` in the helm's
  `store.rs`, the UI's `remembered_permissions` update in `list/view.rs`, `put_preferences`' refusal text), where custom
  is never remembered as the default (recent setups keep the full selection, words included, so reuse stays possible
  there); and the approval card for agent requests (`crates/farhelm-ui/src/approvals.rs`), since `--template` can make
  an agent's request a custom launch, which shows custom and its words.
- **Protocol and downgrade (maintainer accepted, 2026-10-13).** The bump is required: `LaunchSelection` rejects unknown
  fields and `LaunchPermission` is closed. Bump the helm–supervisor protocol version. The fragment and the PR
  description say, in user terms, that remote hosts must be updated before the helm can use them at all after upgrading
  (hosts on another protocol version refuse to connect), and that older releases cannot read sessions or templates
  stored with custom flags. The maintainer accepted the missing downgrade path.
- **Always YOLO.** Custom is an explicit value, so it never reaches the guarded plan's "no permission means the agent's
  own setting" rule: both halves of the split `effective_permission` return it unchanged, and the YOLO classifier treats
  it as YOLO before any own-setting branch. The helm's YOLO guard (`crates/farhelm-helm/src/yolo_guard.rs`) and the host
  setting that asks before YOLO launches then follow with no code change; test them. The YOLO confirmation
  (`crates/farhelm-ui/src/yolo_confirm.rs`) gains a reason for custom flags, as in the mockup. Farhelm does not try to
  work out what mode the flags select.
- **The row.** A custom session shows the amber YOLO shield. Its hover says the flags were the user's, that Farhelm
  doesn't check them and so counts the session as YOLO, shows the user's flags, and shows the full stored command
  ("Launched with", the same `display_command()` the guarded plan uses).
- **Search.** The launcher's search box offers custom (`perms:custom` or the guarded plan's equivalent word) and moves
  focus to the flags field.
- SPEC.md: the custom choice wherever the permission choices, their argv, the YOLO classification, the YOLO
  confirmation's reasons and templates are described. SPEC.md says an agent launch takes "no custom arguments" in two
  places (Concepts, "Launch", and Creation's launch-composer paragraph); both gain this one exception, limited to the
  permission slot, and why. SPEC_impl.md: the stored shape, the splitting rule, and where the words are compiled.
- Docs website: the permission modes section the guarded plan added explains custom, the user's responsibility for the
  flags, and that it is always YOLO (`website/AGENTS.md`, `website/EDITORIAL_RULES.md`).
- Tests: argv for every offering agent, fresh and resume; splitting and its refusals; placeholder refusal;
  classification, the guard and the confirmation reason; the launcher rules on agent switch and template stacking;
  template round trips and the forms that re-display words (Clone, Replace with, Restart with); the CLI refusal; custom
  never remembered as the default; agents that don't offer custom refuse it; the interface wording.
- A changelog fragment (`kind: added`) carrying the upgrade and downgrade caveats.
- The TODO.md entry (`add a "custom" permission mode next to guarded/yolo`) is removed.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

Out of scope: detecting what mode the flags select; validating that the flags only touch permissions (the user's
responsibility); custom flags anywhere other than the permission position; custom for Goose, OpenCode or Pi; the CLI's
`--permissions` gaining a custom form (templates already reach the CLI through `--template`; if they do so naturally,
fine, but no new CLI flags).

## Requirement sources

**The user's request (2026-10-13):** plan this Near term TODO entry. Its text, in full:

> add a "custom" permission mode next to guarded/yolo

**The user's decisions (2026-10-13):**

- What "custom" means, in the maintainer's words: "user supplied flags. user is responsible for ensuring they do nothing
  but alter permissions. it removes any default permissions. for now we treat all such sesesions as yolo, letsnot add
  complexity trying to figure out what the mode is."
- The mockup above, approved as is (including its "Open details": the flags go where Farhelm's own permission flags
  would, for start and resume; empty flags aren't allowed; the search box offers custom).
- After the planning review: the launcher shows the split words only, not a full command ("Words only"); the missing
  downgrade path for launches stored with custom flags is accepted ("Accept all three", covering this and two other
  plans).
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** whatever `guarded-permission-mode.md` landed in SPEC.md and SPEC_impl.md; SPEC.md's
agent launch rules ("no custom arguments", which this plan amends), YOLO classification, the YOLO guard, Upgrade
compatibility and client scale; root `AGENTS.md` Harness-specific code, Docs website, Conventional Commits, Finishing
work, Releases and the changelog, Talking to the user; global instructions (tests never modify the test process's own
environment variables); `.agents/test-authoring.md` for every test change; `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the outline below, the placeholder refusal, the stored shape, and the list of offering agents
(derived from which agents take permission flags).

## Outline

Line numbers drift; find the code by name. Here's what exists (before the guarded plan; re-read after it):

- Permission values and per-harness tables: `LaunchPermission`, `offers_permission`, `omitted_permission`,
  `effective_permission` in `crates/farhelm-proto/src/launch.rs`; classification in `crates/farhelm-proto/src/yolo.rs`
  and `SessionLaunch::yolo()` in `session_launch.rs`; `LaunchSelection` rejects unknown fields.
- Argv: `compile()` in `crates/farhelm-helm/src/launches.rs`, with tests pinning every agent and choice.
- Interface: `launch_controls.rs` (permission buttons), `launch_composer.rs` (search words, labels), `list/row.rs`
  (`agent_badge`, the shield hover), `list/templates.rs` and `save_template.rs`, `restart_with.rs`, `create_form.rs`,
  `yolo_confirm.rs`.
- Shared launcher rules: `TemplateFields`, `LauncherState`, template application and `reconcile_harness_selection` in
  `crates/farhelm-proto/src/launcher.rs`; the CLI's `parse_permissions` in `crates/farhelm/src/main.rs`; the remembered
  permission in the helm's `store.rs` and `preferences.rs`; the approval card in `crates/farhelm-ui/src/approvals.rs`.
- Command launches (`CommandLaunch` in `session_launch.rs`) are the existing whole-command escape hatch; custom is
  narrower and stays an agent launch.

Here's the work, as a proposal:

1. **Data and argv**: the custom value and its words, the per-harness offer, the shared splitter and refusals, argv for
   start and resume, classification as YOLO, the launcher rules, the CLI refusal and remembered-default rule, the
   protocol bump, tests, and the SPEC.md text these make true.
2. **Interface**: the launcher choice and flags field with its words preview, search, the row's shield hover, the
   approval card, templates, Restart with, Clone and Replace with.
3. Docs website, changelog, TODO removal.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- through the recorder, the farhelm-proto, farhelm-helm launch and YOLO-guard tests, and the farhelm-ui tests for the
  launcher, row, templates and confirmation;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, since tests change;
- Playwright on Chromium and WebKit for the specs covering the launcher's permission choices, row tooltips, templates
  and the YOLO confirmation;
- the website build; `dprint check` on changed Markdown; `python3 releasing/check-changelog.py format`.

No real-agent verification is required: Farhelm passes the user's words through unchanged. If you choose to sanity-check
one agent's resume form with custom flags, follow `.agents/real-agent-tui.md`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-custom-permission-flags-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/custom-permission-flags/<nn>-<short-name>` (for example `plan/custom-permission-flags/01-custom-argv`).
- Expected shape: one PR, or two (data and argv, then the interface) only if each carries the SPEC.md text it makes
  true, so main never contradicts SPEC.md's "no custom arguments" rule; a data-only first PR's fragment is `kind: none`
  with its reason, and the user-facing fragment arrives with the interface. Avoid churn: code added in one PR and
  deleted in a later one means the stack should have been shaped differently. Within this run, a PR that needs
  correcting is restructured rather than corrected on top, and that applies to PRs an earlier run built too.
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
- the acceptance criteria: this file's goal, the mockup's link, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check specifically: that a custom session can never classify as anything but YOLO; that the words
reach the agent exactly as split, in the permission position, for start and resume, with no shell expansion and no
placeholder smuggling; and that the stored launch round-trips through every relaunch path and template.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example inferring the mode from the flags,
  per-agent flag validation, or custom arguments outside the permission position (examples, not a blacklist);
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
alternatives considered. In particular: the final list of offering agents; the permission slot used; the splitting and
refusal rules as implemented; wording changes from the mockup; the stack's shape; every review finding you decided not
to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

Agreed fallbacks:

- If the guarded plan landed something that changes an assumption here (an agent's permission is no longer set by flags,
  say), follow what landed: offer custom exactly where Farhelm sets the permission with flags, and record it.
- Wording in the mockup that turns out inaccurate is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: classifying any custom session as not YOLO,
accepting empty flags, or custom arguments outside the permission position.

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
