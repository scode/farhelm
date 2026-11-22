# Remove "Agent's own setting" permission mode and treat all unproven launches as YOLO

Written against main at 8007a789 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Trivial"). Verbatim:

> **Replace on a guarded Codex session asks to confirm a YOLO launch.** it's a guarded session and i clicked replace and
> got a dialog indicating farhelm wants to replace with a yolo session instead of guarded. codex session. macos
> supervisor. (Screenshot, 0.44.0-rc.4: a Codex session on Local (this machine), shown with the guarded shield mark in
> the session list, is selected. After Replace, the session pane shows a red "Confirm YOLO launch" banner: "YOLO means
> the agent bypasses its safety checks: no person, classifier, AI reviewer or sandbox stops its actions on Local (this
> machine) without asking you. This launch uses the agent's own setting, which Farhelm cannot override or classify. It
> is treated as YOLO on this host. Local (this machine) asks before every YOLO launch. Nothing has been started." with
> Cancel, "Start, and don't ask again on this host" and "Start YOLO session anyway".)

Investigation established that the screenshot's session was an old own-setting (`permissions: None`) Codex launch whose
dashed shield (`permission-own-setting.svg`) rendered solid at 10 px and turned white on the selected row, making it
look identical to a guarded shield. In response, the maintainer chose to remove the "Agent's own setting" concept
completely: any non-shell agent launch that is not proven to be guarded (or in an explicit approval mode) is classified
and shown as **YOLO**, Cursor defaults to **YOLO** (`--force`) while keeping **Custom**, existing old sessions without a
recorded permission mode are retroactively migrated to `YOLO` on startup (keeping their saved command line intact), and
plain **Replace** offers **"Replace as guarded instead"** in its YOLO confirmation banner when the agent supports
Guarded.

### Where it stands (from planning research at main 8007a789; line numbers approximate)

- **Proto classification (`crates/farhelm-proto/src/launch.rs`, `yolo.rs`, `session_launch.rs`):**
  - `LaunchHarness::omitted_permission` (`launch.rs:327-340`) returns `None` for `Cursor`, `Bash`, `Zsh` (and
    `Some(Guarded)` for Codex/Claude/Muse/OMP/Grok/Antigravity, `Some(Yolo)` for Pi/OpenCode/Goose). Change `Cursor` to
    return `Some(LaunchPermission::Yolo)`.
  - `LaunchHarness::stored_permission` (`launch.rs:385-406`) returns `None` for `Cursor`, `Codex`, `Claude`, `Muse`,
    `Grok`, `Antigravity`, `Bash`, `Zsh` when `permissions` is `None`. Change every non-shell harness to map omitted
    stored permission (`None`) to `Some(LaunchPermission::Yolo)` so any unmigrated or cached row is classified as YOLO;
    keep `Bash` and `Zsh` returning `None`.
  - `LaunchHarness::offers_only_yolo` (`launch.rs:347-360`): Cursor offers `Yolo` and `Custom` (neither of which is a
    guarded or approval mode), so `offers_only_yolo(Cursor)` is currently `false`. Once `YoloReason::OwnSetting` is
    removed, ensure a YOLO Cursor launch does not fall through to `YoloReason::ChosenPermission`'s misleading sentence
    (*"Another permission mode would keep the agent's safety checks instead"*) — classify harnesses with no
    non-YOLO/non-Custom mode (or Cursor specifically via an exhaustive `LaunchHarness` method) under `OnlyMode` or an
    accurate sentence.
- **Helm launch compilation & Replace (`crates/farhelm-helm/src/launches.rs`, `sessions.rs`, `yolo_guard.rs`):**
  - `launches::compile` (`launches.rs:537-706`) calls `normalize_selection` (`launches.rs:514-519`), which fills
    `permissions: None` via `omitted_permission()`. For `Cursor`, `Some(LaunchPermission::Yolo)` already emits `--force`
    (`launches.rs:657`).
  - Once `omitted_permission(Cursor) == Some(LaunchPermission::Yolo)`, `LaunchHarness::request_permission`,
    `launches::normalize_selection`, and `record_launch_history` (`crates/farhelm-helm/src/store.rs:219-223`) already
    handle stored templates, recent setups, and remembered defaults — **no helm SQLite migration is needed**.
  - `yolo_guard.rs:29` error text still says _"this launch is YOLO or uses the agent's own setting"_; remove the
    own-setting phrasing.
  - `replace_session_inner` (`sessions.rs:4685-4906`) already supports
    `ReplaceSessionBody { with: Option<CreateSessionBody>, ... }`
    (`crates/farhelm-proto/src/session_requests.rs:143-150`). For plain Replace's new **"Replace as guarded instead"**
    button, the GUI can send `ReplaceSessionBody` with `with: Some(CreateSessionBody)` built from the source row's
    `SessionLaunch::Agent` selection with `permissions: Some(LaunchPermission::Guarded)` and `confirm_yolo: false`
    (preserving host, cwd, title, harness, model, effort, wrapper, and `DeleteGuard`; do NOT copy retired `container`,
    which `refuse_retired_source` already rejects). Note that `with: Some(...)` uses `Replace with` semantics (applying
    current Settings' workspace trust and updating remembered defaults, matching the launcher's "Start guarded session
    instead").
- **Supervisor startup migration (`crates/farhelm-supervisor/src/store.rs`):**
  - Currently `SCHEMA_VERSION = 38`. Add a schema step (v39, with fresh-schema stamp and historical fixture test) that
    rewrites existing `sessions` rows (`create_reservations` does not store `session_launch`) whose
    `SessionLaunch::Agent` selection has `selection.permissions == None` on a non-shell harness to
    `selection.harness.stored_permission(selection.permissions)` (`Some(LaunchPermission::Yolo)`), reusing the single
    canonical rule in `farhelm-proto` while keeping `start` and `resume` argv verbatim.
- **GUI (`crates/gui/farhelm-gui/src/`):**
  - `permission.rs`: remove `Shield::OwnSetting`; `launched()` maps any non-shell `stored_permission == None` (or
    `Yolo`/`Custom`) to `Shield::Yolo`.
  - `assets.rs` and `assets/icons/permission-own-setting.svg`: remove `Icon::PermissionOwnSetting` and delete the SVG.
  - `permission/words.rs`: remove the `"Not classified · Agent's own setting"` arm in `permission_help`.
  - `launcher/yolo.rs` and `launcher/view/yolo_question.rs`:
    - Remove `YoloReason::OwnSetting`; update `YoloAsk::for_stored_launch`'s `SessionLaunch::Legacy` arm
      (`yolo_question.rs:77`, e.g. to `YoloReason::Asserted` or `ChosenPermission`).
    - Parameterize the guarded-alternative button label/tooltip on `ask_question` / `GuardedAction`
      (`yolo_question.rs:315` currently hardcodes `"Start guarded session instead"`) so the launcher keeps
      `"Start guarded session instead"` and plain Replace shows **"Replace as guarded instead"**.
    - Enable **"Replace as guarded instead"** on `YoloAsk::for_stored_launch` when the stored launch is a non-custom
      YOLO agent launch on a harness that `offers_permission(LaunchPermission::Guarded)`, wired through
      `session_list/replace_yolo.rs` and `session_list/replace.rs`.
  - Remove `"Agent's own setting"` choices, labels, chips, and search values across:
    - `approvals/model.rs` (`ChipKind::OwnSetting`) and `approvals/model_tests.rs`
    - `session_list/summary.rs` (`AGENT_OWN_SETTING`)
    - `hosts/settings.rs:416`, `templates/summary.rs:192`
    - `launcher/view/choices.rs`, `launcher/view/changed.rs`, `launcher/recents.rs`, `launcher/search.rs`
      (`SearchPermission::OwnSetting`), `launcher/draft.rs`, and `goldens/mod.rs:348`.
    - Cursor shows `YOLO` (default) and `Custom`.
  - Agent instructions (`crates/farhelm/src/agent_instructions.rs:171,254`), `SPEC.md` (lines `650`, `1086-1148`,
    `1526-1550`, `2817`, `4523`), `SPEC_impl.md`, and docs (`website/src/content/docs/docs/agents/cursor.md`,
    `index.mdx`, `docs/using/start-a-session.mdx`): remove all references to "Agent's own setting".

### Acceptance criteria

1. **No "Agent's own setting" concept remains:**
   - `LaunchHarness::omitted_permission(Cursor)` returns `Some(LaunchPermission::Yolo)`. Every non-shell harness has a
     `Some(...)` `omitted_permission` and a `Some(...)` `stored_permission` (any omitted stored permission on a
     non-shell agent classifies as `Some(LaunchPermission::Yolo)`). Shells (`Bash`, `Zsh`) stay `None`.
   - Cursor offers **YOLO** (`--force`, default) and **Custom** flags in the launcher and helm admission, and Cursor's
     YOLO confirmation banner does not show the misleading _"Another permission mode would keep the agent's safety
     checks instead"_ sentence.
   - `Shield::OwnSetting`, `Icon::PermissionOwnSetting`, `assets/icons/permission-own-setting.svg`,
     `YoloReason::OwnSetting`, `ChipKind::OwnSetting`, `SearchPermission::OwnSetting`, and `AGENT_OWN_SETTING` are
     removed.
   - Goose (`Approve`, `Smart approve`, `Chat`), OMP (`Approve`), `Custom` flags, and `Legacy` pre-launch-kind sessions
     (`?` mark) remain unchanged.
2. **Retroactive migration on startup (metadata only, argv intact):**
   - A new supervisor database migration upgrades existing stored `SessionLaunch::Agent` rows whose non-shell
     `selection.permissions` is `None` to `Some(LaunchPermission::Yolo)` (reusing `stored_permission`) without rewriting
     their stored `start` or `resume` argv.
   - On any older/unmigrated wire row or cached row with `permissions: None` on a non-shell agent, `stored_permission`
     still classifies it as `Some(LaunchPermission::Yolo)` so it displays the amber slashed YOLO shield and hits the
     YOLO gate everywhere.
   - Saved templates, launch history, and remembered defaults with `permissions: None` for Cursor resolve cleanly to
     `YOLO` via existing normalization helpers (no helm DB migration needed), and guarded-capable harnesses continue to
     resolve to `Guarded` when composing new launches.
3. **"Replace as guarded instead" on plain Replace:**
   - When plain Replace hits the YOLO confirmation banner on a host that asks before YOLO launches, and the session is a
     YOLO agent launch (not Custom flags, not Command) on a harness that `offers_permission(LaunchPermission::Guarded)`
     (such as Codex, Claude, Muse, Grok, Antigravity, or OMP), the inline banner offers **"Replace as guarded instead"**
     between **Cancel** and **"Start, and don't ask again on this host"**, matching the launcher's Tab/button order.
   - Choosing **"Replace as guarded instead"** replaces the session on the same host with the same working directory,
     title, harness, model, effort, wrapper, and `DeleteGuard`, recompiled with
     `permissions: Some(LaunchPermission::Guarded)` and `confirm_yolo: false` (no host setting write, no YOLO consent).
4. **Specs, docs, goldens, and tests:**
   - `SPEC.md`, `SPEC_impl.md`, `crates/farhelm/src/agent_instructions.rs`, and the affected docs pages under
     `website/src/content/docs/` are updated consistently.
   - Affected GUI golden screenshots are refreshed and published (`check-gui-goldens.py` + `gui-goldens.py publish`).
   - Unit and integration tests cover: Cursor default YOLO (`--force`) and Custom; supervisor DB migration upgrading
     `permissions: None` to `Some(Yolo)` while preserving stored `start`/`resume` argv; GUI shield & summary rendering
     as YOLO; Cursor's YOLO banner wording; and plain Replace's **"Replace as guarded instead"** flow (including
     verifying `Custom` flags do NOT offer the guarded button).

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Remove the "Agent's own setting" permission concept completely; any non-shell launch not proven guarded (or in an
  explicit approval mode) counts and shows as YOLO.
- Cursor defaults to YOLO (`--force`) and keeps Custom flags.
- Existing old sessions with no recorded permission mode are retroactively migrated to `YOLO` on startup while keeping
  their stored command line (`start`/`resume` argv) unchanged.
- Keep Goose's `Approve`/`Smart approve`/`Chat`, OMP's `Approve`, `Custom` flags, and `Legacy` (`?`) sessions unchanged.
- Add **"Replace as guarded instead"** to plain Replace's YOLO confirmation banner when the agent supports Guarded.
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: verbatim text recorded under `## Release note` below.

**Binding repository constraints:**

- `AGENTS.md`, "Code rules": keep per-harness decisions in the exhaustive places named in
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` and `farhelm_proto::LaunchHarness`; no `kind == X` or `_` arms in
  shared code.
- `website/AGENTS.md` when editing `website/src/content/docs/` (unattended run: start no preview server, report changed
  pages).
- `.agents/validation.md` for GUI goldens (`check-gui-goldens.py` + `gui-goldens.py publish`) and website checks.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals:**

- Split cleanly into 3 bite-sized PRs: (1) proto + helm + supervisor DB migration, (2) GUI removal of own-setting +
  "Replace as guarded instead" + goldens, (3) docs/specs + TODO removal.
- For "Replace as guarded instead" on plain Replace, reuse the existing `POST /api/sessions/{id}/replace` `with` body
  (`ReplaceSessionBody { with: Some(CreateSessionBody), ... }`) constructed from the source row's `SessionLaunch::Agent`
  selection with `permissions: Some(LaunchPermission::Guarded)` (accepting `Replace with` semantics: applying current
  Settings workspace trust and updating remembered defaults, matching the launcher's "Start guarded session instead")
  rather than adding a new REST field.
- Do not add a helm SQLite migration; reuse existing `omitted_permission` / `request_permission` / `normalize_selection`
  normalization.

## Outline

1. `plan/remove-own-setting-permission/01-proto-helm-migration`: update `farhelm-proto` (`omitted_permission`,
   `stored_permission`, non-guarded/YOLO-only harness check, docs/tests), `farhelm-helm` (`launches.rs`,
   `yolo_guard.rs`, template/history tests), `crates/farhelm/src/agent_instructions.rs`, and add the supervisor SQLite
   migration rewriting non-shell `SessionLaunch::Agent` rows with `permissions: None` to `Some(LaunchPermission::Yolo)`
   via `stored_permission` while preserving `start` and `resume` argv (plus historical-schema migration test).
   `feat: remove own-setting mode and migrate old sessions to YOLO` (with `kind: changed` changelog fragment).
2. `plan/remove-own-setting-permission/02-gui-own-setting-and-replace-guarded`: remove `Shield::OwnSetting`,
   `Icon::PermissionOwnSetting`, `permission-own-setting.svg`, `YoloReason::OwnSetting`, `ChipKind::OwnSetting`,
   `SearchPermission::OwnSetting`, `AGENT_OWN_SETTING`, and all own-setting UI/summary/launcher/host-settings copy;
   parameterize `yolo_question.rs`'s guarded button label and wire **"Replace as guarded instead"** on plain Replace via
   `ReplaceSessionBody.with`; fix Cursor's YOLO confirmation sentence; update GUI unit tests and refresh/publish
   affected golden screenshots. `feat: show all non-guarded sessions as YOLO and offer guarded Replace` (amends PR 1's
   changelog fragment).
3. `plan/remove-own-setting-permission/03-specs-docs-and-todo`: update `SPEC.md`, `SPEC_impl.md`, and docs pages
   (`cursor.md`, `index.mdx`, `start-a-session.mdx`), and remove the TODO.md entry ("Replace on a guarded Codex session
   asks to confirm a YOLO launch"). `docs: update specs and docs for removing own-setting mode`.

## Fallbacks

- If preserving `settings_from_source = true` (avoiding updating `launch_history` / `remembered_permissions` and keeping
  the source's live `cwd`/`title`/`workspace_trust`) turns out cleaner than reusing `ReplaceSessionBody.with`, a narrow
  REST-only optional field on `ReplaceSessionBody` (no supervisor `PROTOCOL_VERSION` bump) is an acceptable alternative;
  log the choice as a DECISION.
- If `scripts/gui-goldens.py publish` cannot reach the private goldens storage repository in the executor's environment,
  block with the exact error rather than committing a broken `goldens.txt` manifest.
- If a schema version bumps on `main@origin` before this lands, renumber the supervisor migration step on rebase per
  `.agents/careful-rebase.md`.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-remove-own-setting-permission-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-remove-own-setting-permission-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted tests for `farhelm-proto`, `farhelm-supervisor` (including schema migration tests),
`farhelm-helm` (`launches`, `yolo_guard`, `sessions`), and `farhelm-gui` (`permission`, `launcher::yolo_tests`,
`launcher::search_view_tests`, `session_list::row_marks_tests`, `session_list::lifecycle_tests`,
`approvals::model_tests`, `templates`), plus `check-gui-goldens.py` + `gui-goldens.py publish`, website build/check for
edited docs, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `dprint check` on changed
Markdown, and `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it. Apply
`.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live Farhelm
install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Replace on a guarded Codex session asks to
  confirm a YOLO launch"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
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
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

Simplified Farhelm's permission modes - we now only have "guarded" and "yolo" and removed an arguably confusing "agent's
own settings" permission mode. Any existing settings with that mode are now treated as yolo from an approval dialog
perspective (conservative/safe behavior). If you don't know what this mode was, it's safe to ignore this change.
