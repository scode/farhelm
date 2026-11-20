# Shell sessions: Bash and Zsh beside the agents

Written against main at 0055c6beb on 2026-11-19. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> **First-class shell sessions.** support more directly "just run a shell" (no agent). technically possible already with
> a custom command but let's make it simple and first class by adding a "shell" type besides agent+command, and to start
> with supporting bash and zsh. both bash and zsh should be supported as typing shortcuts so you can just type "bash"
> and hit enter (like you can type claude and hit enter). it automatically means yolo. probably /usr/bin/env bash and
> /usr/bin/env zsh to remain portable. need to pick what to use for icons.

Planning settled several points differently from the entry's first guesses (see Decisions): a shell is **not** YOLO, it
has no permission mode at all; the shell starts as plain `bash` or `zsh`, not through `/usr/bin/env`. Despite the
entry's "Trivial" subsection, this is a protocol bump and touches every per-agent decision point; plan for that.

### Where it stands (from planning, at main 0055c6beb)

- The launcher has two tabs, Agent and Command (`crates/gui/farhelm-gui/src/launcher/draft.rs`, `Tab`). The Agent tab's
  chips come from `HARNESS_PICKER_ORDER` and its searches from `HARNESS_SEARCH_ORDER`
  (`crates/gui/farhelm-gui/src/launcher/words.rs`), both checked at compile time to name every `LaunchHarness`.
- The "type claude and press Enter" shortcut is the launcher's search box: every query offers a harness row whose search
  word (`harness_label(h)` lowercased) contains the text; an exact word preselects it (`launcher/search.rs`,
  `exact_word`, `default_index`); Enter takes the highlighted row and empties the box, and Enter on an empty box presses
  Launch (`launcher/view/search.rs`). So `claude ⏎ ⏎` launches. Today `bash ⏎` falls through to "Set session name".
- A command launch (`SessionLaunch::Command` in `crates/farhelm-proto/src/session_launch.rs`) carries the user's YOLO
  assertion, shows the generic Terminal mark, never appears in recent setups, and cannot Restart unless it declares an
  agent type and opts into Resume.
- `LaunchHarness` (`crates/farhelm-proto/src/launch.rs`) is what the user picked; `AgentKind`
  (`crates/farhelm-proto/src/lib.rs`) is the integration that runs. Cursor and OpenCode map to `AgentKind::Generic`
  (basic running/idle status from the generic screen reader, no conversation): that is the precedent, recorded in
  SPEC_impl.md ("Protocol 27 adds the Cursor harness variant, not a new runtime integration kind").
- `LaunchSelection.permissions` is an `Option<LaunchPermission>`; `omitted_permission`, `stored_permission`,
  `offers_permission` and `offers_only_yolo` on `LaunchHarness` decide what a harness offers and how an omission is
  stored. A stored `None` today means "the agent's own setting" (Cursor's case), and every reader treats that as the
  most dangerous case: the host's "ask before YOLO launches" guard and the refusal of agent-started YOLO launches both
  read `SessionLaunch::yolo()`, which is `farhelm_proto::yolo::selection_is_yolo` and returns true for a stored `None`;
  the row shield maps `None` to the own-setting shield; the launcher offers an "Agent's own setting" choice. (SPEC.md,
  the YOLO confirmation paragraph under creation, explains the two groups: an omission on Claude, Antigravity, Codex,
  Grok, Muse or Cursor is the agent's own setting, conservatively treated as YOLO; on Pi, OpenCode, OMP and Goose it is
  YOLO.)
- The helm's `record_launch_history` (`crates/farhelm-helm/src/store.rs`) writes the remembered permission from every
  explicit agent selection, so a shell launch through the same path would rewrite it.
- The helm compiler (`crates/farhelm-helm/src/launches.rs`) emits one program word per harness (`program()`), then
  model, effort, permission flags and `{farhelm_args}`. Every agent-terminal launch runs inside the user's login shell
  and execs the program through the launch shim; a missing program becomes **Error** naming it.
- Sidebar marks: `harness::glyph` in `crates/gui/farhelm-gui/src/harness.rs` picks a `HarnessGlyph`; geometry lives in
  `crates/gui/farhelm-gui/src/marks.rs` and `marks/`, SVGs are generated, never hand-edited, and `docs/harness-marks.md`
  governs it all. The Terminal mark (`assets/icons/harness-terminal.svg`) is a Farhelm original: a window, a `>` chevron
  and an underscore, in inherited grey.

### Acceptance criteria

1. **Two new launch types, Bash and Zsh,** shown on the launcher's Agent tab in their own "Shells" group after the agent
   chips, and in their own "Shells" group in search results. Typing `bash` or `zsh` in the launcher's search box
   preselects the shell row, Enter picks it, and Enter again launches.
2. **Started as plain `bash` or `zsh`,** found through the session's PATH like every other program, not as a login shell
   and not through `/usr/bin/env`. Without a wrapper prefix, a host without the shell shows the session as Error naming
   the missing program. The session's wrapper prefix (such as `nix develop -c`) still applies; with one, a missing shell
   exits 127 as Exited, as for any wrapped agent (SPEC.md, launch failures).
3. **No permission mode.** A shell launch stores no permission. The launcher shows no permission row (and no model,
   effort, initial prompt or conversation fields) for a shell. The session row shows no permission shield; its hover
   says it is a Bash (or Zsh) shell with no agent. Nothing anywhere shows "Agent's own setting" for a shell: not the
   launcher's permission choices or search rows, the recent-setups strip, the template editor's permission field, the
   row shield, or an approval card's Permissions row. A shell launch is never YOLO: creating, cloning or replacing a
   shell on a host that asks before YOLO launches asks nothing, exactly as opening a terminal tab asks nothing.
   Switching the launcher from a Guarded agent to a shell shows no escalation banner, and the notice shown when
   switching types clears choices speaks of a shell, not a harness. An agent-supplied wrapper prefix still needs the
   user's Allow on such a host, as for any session.
4. **Agents may start a shell session** (`farhelm spawn --agent bash`, `farhelm agent create --agent zsh`) through the
   ordinary approval rules: the ordinary card, or none where the host lets its sessions manage sessions without asking.
   The YOLO-launch refusal for agents does not apply, because a shell is not YOLO. Shells take no initial prompt.
5. **Launcher memory:** a shell launch leaves the remembered permission choice untouched (launching Bash never changes
   how the next Claude draft starts). It still appears in recent setups.
6. **Restart and Replace:** Restart is unavailable for a shell, greyed out with a shell-specific reason (a shell has no
   conversation to resume; Replace starts a fresh shell). The reason is worded in the desktop app from the session's
   launch type; the wire keeps today's "no conversation reporting" restart offer, so no new restart-offer variant is
   added. Replace starts a fresh shell. Clone works as for an agent launch.
7. **Status:** the generic screen reader (Idle at a quiet prompt, Running while output changes; never Waiting).
8. **Command tab:** Bash and Zsh are not offered as the "agent this command runs" of a command launch, and a new
   command-launch request declaring one is refused. Stored rows are not re-validated (`CommandLaunch::validate_stored`
   documents that new rules never apply to loaded rows, and no stored row can name a shell yet).
9. **Templates** accept `agent: bash` and `agent: zsh`; quick launch works for them.
10. **Icons:** Farhelm's own marks, a window like the Terminal mark's with `$_` for Bash and `%_` for Zsh in place of
    the chevron and underscore, in the Terminal mark's inherited grey, drawn on the marks guide's 12-unit grid and
    recorded as Farhelm originals.
11. **Wording:** Stop, hovers and help say "shell" where they say "agent" for a shell session.
12. **Protocol:** the new variants bump `PROTOCOL_VERSION` with its pin test; SPEC_impl.md records the bump beside the
    Cursor and OpenCode notes.
13. **Spec and docs:** SPEC.md amended wherever it lists launch kinds, harnesses, permission defaults, the YOLO guard,
    the conversation selector ("every agent launch"), the initial-prompt list, Restart availability, session marks and
    shields, approval cards' Permissions row, agent-spawned session types, first-class harnesses, and supported user
    environments (a shell session needs that shell on the host). The agent instructions
    (`crates/farhelm/src/agent_instructions.rs`) list the shell types. The website's supported-agents table gains a
    Shells row, a short page explains shell sessions, and the start-a-session, launch-templates, stop/restart/resume and
    session-list pages mention them where they list types.
14. The last PR removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-19 planning page:

- **Design:** shell types as agent types (two `LaunchHarness` variants on `AgentKind::Generic`), not a third launch kind
  and not a launcher shortcut over Command.
- **Placement:** their own "Shells" group after the agents on the Agent tab.
- **Permissions:** neither YOLO nor Guarded. The maintainer's words: "the yolo bit is not really a fit. for example it
  makes no sense to trigger approval flows. there is no agent. there is just deterministic commands, and we don't treat
  you starting a terminal with a shell next to a guarded agent as yolo." Settled as "no permission mode": no permission
  row, no YOLO confirmation, no shield, launcher memory untouched (acceptance criteria 3 and 5).
- **Agent-started shells:** allowed, through the ordinary approval rules.
- **Icons:** option A, the window with `$_` and `%_`, in the Terminal mark's grey.
- **Program:** plain `bash` / `zsh`. **Not a login shell.**
- **Restart:** none, greyed out with a shell-specific reason; Replace starts a fresh shell.
- **Release note:** the maintainer's own wording, in `## Release note` below.
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **No-workhorse mode**, the plans default, stated on the planning page.

Planner decisions shown on the page without objection: Bash and Zsh are left out of the Command tab's declared-agent
list; the wrapper prefix applies to shells; a Shells row in the docs table and a short shells page; a shell session
keeps its session's credentials, so `farhelm spawn` typed in it works as from a command session.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the planning page answers above, and the maintainer's words
on permissions quoted there.

**Binding repository constraints:**

- Root `AGENTS.md`, "Code rules": every per-harness decision lives in the named places mapped in the module docs of
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` (read the map before coding) and the exhaustive methods on
  `LaunchHarness`; no `kind == X`, `matches!` over harnesses, or `_` arm over harnesses in shared code. "Is this a
  shell" is itself a per-harness answer, given by an exhaustive method, never by comparing variants at a call site.
- `docs/harness-marks.md` for the marks (12-unit box, generated SVGs, no SVG `text`, contrast, provenance row).
- `crates/farhelm-proto/src/lib.rs`: a new wire variant bumps `PROTOCOL_VERSION`; never two wire-variant commits on main
  under one version.
- SPEC.md "Restart always means the conversation is preserved" (2026-10-03): shells get no Restart rather than a new
  meaning for it.
- Root `AGENTS.md` "Conventional Commits", "Changelog fragments", "Tests and validation"; `.agents/validation.md`;
  `.agents/test-authoring.md`; `website/AGENTS.md` for docs edits.

**Planner proposals** (verified against the code and reviewed by an independent planning reviewer):

- **`LaunchHarness::Bash` and `LaunchHarness::Zsh` on `AgentKind::Generic`**, answering the exhaustive methods the way
  OpenCode does except for permissions: no model, effort, initial prompt, workspace trust or named conversation (the
  first harnesses whose `offers_named_conversation` is false), no resume arguments, `program()` = `bash` / `zsh`.
  Existing tests that iterate `LaunchHarness::ALL` pick the variants up.
- **One exhaustive answer for "is this a shell"** on `LaunchHarness`, named for what the harness is (for example
  `is_shell()`, or a two-variant agent/shell enum), not for permissions, because the Shells groups, the shell wording,
  the Restart reason and the Command-tab filter read it as well as the permission sites. Keep it an explicit per-harness
  answer; do not derive it from "offers no permission choice", which would quietly make a future agent with no
  permission flags never-YOLO. Add a proto test tying the tables to it: a shell offers no permission, its
  `omitted_permission` is `None`, and `selection_is_yolo` is false for it.
- **Where the answer is read.** Two lists, because the compiler finds neither:
  - Admission, one site: `farhelm_proto::yolo::selection_is_yolo` (read by the host YOLO guard and the agent-spawn YOLO
    refusal through `SessionLaunch::yolo()`). The helm's normalization and its refusal of an explicit permission need no
    new code: they already follow `omitted_permission` and `offers_permission`, and a shell offers none.
  - Every reader that interprets a missing permission as the agent's own setting: find them by grepping for
    `omitted_permission().is_none()`, `stored_permission(` / `request_permission(` returning `None`, and `danger(None)`.
    At planning they included the launcher's permission row (`launcher/view/choices.rs`) and search permission rows
    (`SearchPermission::offered_by` in `launcher/search.rs`), the recent-setups strip (`launcher/recents.rs`), the
    template editor's permission field (`templates/fields.rs`), the row shield (`permission::launched` in
    `permission.rs`, which needs a "no shield" outcome), the escalation banner (`danger` in `launcher/draft.rs`), the
    type-switch notice (`reconciliation_notice` in `draft.rs`), and the approval cards' Permissions row.
- **Launcher memory:** `remembered_permissions` and `remembered_wrapper_prefixes` are written by one statement in
  `store.rs`; a shell launch skips only the permission part. The wrapper map is keyed per harness, so a shell's wrapper
  memory works unchanged.
- **Launcher groups:** a "Shells" chip group and a "Shells" search group driven by that same answer, so the order arrays
  still name every harness once and a later shell needs no new grouping code. The search group needs entries in both
  `default_index` priority lists (`Scope::All` and `Scope::Harness` in `launcher/search.rs`), or `bash` will not
  preselect.
- **Marks:** two new `HarnessGlyph` geometries beside the Terminal mark, generated SVGs, a "Farhelm original" provenance
  row, no identity color.
- **PR shape:** the wire variants, every per-harness answer, the launcher, the marks and the spec land together in one
  PR, so the protocol bump lands once and no intermediate PR has a half-wired type or a placeholder mark that a later PR
  replaces. The website pages and the TODO removal follow in a docs PR.

## Outline

1. `plan/shell-sessions/01-shell-session-types` (`feat:`): the proto variants and bump, the exhaustive answers, the helm
   compiler and launch-memory gate, the supervisor's per-harness matches, the GUI vocabulary, groups, permission row,
   shield, Restart reason, Command-tab filter and marks, the agent instructions, SPEC.md and SPEC_impl.md, tests (a
   compile test per shell; launcher search `bash`/`zsh` preselecting the shell row and `⏎ ⏎` launching; no YOLO
   confirmation for a shell on an asking host; an agent spawn of a shell going through the ordinary card; the remembered
   permission surviving a shell launch; a command launch declaring Bash refused), and the changelog fragment
   (`kind: added`).
2. `plan/shell-sessions/02-shell-session-docs` (`docs:`): the website pages, and the TODO.md entry removed. Read
   `website/AGENTS.md` first; this plan's main work is not the docs, so start no preview server and name the changed
   pages in the report.

If PR 1 grows past what one reviewer can review well, split it along a line where each PR builds and passes its tests on
its own and none adds something a later one deletes (for example the marks' geometry and SVGs first, if they can land
unused without dead-code warnings); log the split.

## Fallbacks

- If "no permission mode" cannot be expressed without changing the wire shape of `LaunchSelection` beyond the new
  variants (a new field, say), that is still inside the one protocol bump; log it.
- If some screen or confirmation still needs a permission word for a shell (a template editor field, an approval card
  line), say "none (shell)" or leave the line out, whichever that surface already does for a missing value; log each.
- If the generic screen reader misreads a shell (for example flickering between Running and Idle while the user types),
  keep it unless it breaks an existing contract, and note it in the report.
- If the session's agent credentials in the shell's environment conflict with a rule (a test asserting only agents hold
  them, say), keep them as command sessions do and log it.

Decisions to log: the exhaustive method's name and shape, every call site that asks it (with the grep used to find the
own-setting readers), how the launcher groups are driven, the mark geometry, any split of PR 1, and every review finding
declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-shell-sessions-log.md` in the parent directory of the checkout you run in, derived as that section says,
and the watchdog's status file is `farhelm-plan-shell-sessions-watchdog.json` beside it. Resolve both to absolute paths
before you start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change. A protocol
bump and new marks reach the reference screenshots and the end-to-end smoke; decide from `.agents/validation.md` which
of those to run.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/shell-sessions/<nn>-<short-name>`, as the
  outline names them.
- Keep each PR bite-sized within the constraint the outline gives. Avoid churn: code added in one PR and deleted in a
  later one means the stack should have been shaped differently. Within this run, a PR that needs correcting is
  restructured rather than corrected on top, and that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what that PR
  built. Where what got built departs from the release note entry (a renamed control, a part left out, a limitation it
  does not mention), the fragment's context paragraphs say how (`plans/.agents/executing.md`, "Executing one plan", step
  8). Never edit the section itself.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides. A protocol version conflict at rebase (main
  bumped meanwhile) is resolved by taking the next version, never by sharing one.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last PR of the stack removes this plan's TODO.md entry.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or the logged
split of PR 1), together meeting the acceptance criteria above, each having passed its review gate, and the last PR
removing this plan's TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its
report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

You can now start a plain Bash or Zsh shell as a session, just like you can start agents. Typing "bash" or "zsh" in the
launcher text field is the quickest way to do so.
