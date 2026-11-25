# Launcher consent, custom agents, and keyboard recents fixes

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Implement four launcher `TODO.md` entries from `## Plan follow-ups`, grouped as one linear stack (one PR per item) so
they do not collide across `crates/gui/farhelm-gui/src/launcher/`. Every item is subject to the complexity kick-back
rule below. Verbatim:

1. > **Keep one-time YOLO confirmation through a host wake.** When a sandbox suspends or a container stops while “Start
   > YOLO session anyway” is open, answering can wake the host and then ask the same question again. Capture the
   > selected installation when New, Clone, Replace with, or Restart with sends the original launch request. Check that
   > the question still applies before waking the host. Carry the one-time answer with the held launch through the wake
   > and any checkout preview. Retain the original folder and source consent guards for Replace with and Restart with.
   > Allow a fresh connection to the same verified installation. Clear the answer when that launch ends. Ordinary and
   > guarded launches carry no override. Update SPEC_impl.md under “Questions: the prompt”. Exercise suspension between
   > the question and answer, confirm the launch on the new connection, and show that changing installations before the
   > refusal or during the wait, or starting a later launch, requires its own answer. Binding the original dispatch and
   > testing the resumed launch across these paths exceeds the launcher triage budget.

2. > **Keep hook setup from launching into a folder that arrived later.** In the launcher, pick Grok or Antigravity to
   > open the Restart hook question. Switch to a connected host whose remembered folder is still loading and choose
   > **Set up and launch**. A history reply can settle the folder while the all-hosts setting saves. The continuation
   > then launches in a folder the question had yet to show. Bind the continuation in `launcher/view/hook_question.rs`
   > to the readiness, destination and source warning shown with the answer, per SPEC.md's “One GUI at a time”. Save the
   > setting and leave launch to a new press when the shown launch was refused or its destination changed. An unchanged
   > sleeping host can legitimately wake and prepare a folder or checkout, so a missing launch request is insufficient
   > to tell which answer was safe. Wake can take its folder from a newly selected session before the next frame. Settle
   > that default before comparing. For disposable environments, compare the actual placement and ignore any retained
   > host choice from the ordinary launcher. The fix and test exceed the triage size gate once they cover these paths.
   > Reproduce from New with no open session and Folder untouched. Use the real host picker and hold the new host's
   > history reply until the setting write starts. Draw the settled folder before replying to the write. Verify the
   > write succeeds and only a fresh press starts the session in the settled folder.

3. > **Distinguish unread custom agent registrations from deletion.** When the session launcher opens a saved custom
   > agent choice before its list arrives or after the helm fails to read it, it can wrongly say the registration was
   > deleted. Keep the choice visible on both tabs and keep Launch unavailable when there is no entry to use. Show
   > whether the list is still loading or the read failed. Keep using entries retained from a previous successful read,
   > and show deletion advice only after a successful list omits the ID. Apply the same distinction to a stale chip or
   > search choice and the Command tab's retained choice. Restart with must keep its required conversation field visible
   > when the list is loading or unreadable, even if the field is empty. Show the reason by the field. Keep loading text
   > neutral and avoid guessing whether resumption or reporting is supported while the entry is unknown. Check both tabs
   > in all three list states, with light and dark screenshots. Before pressing Launch, confirm that the fixture
   > installed the intended catalog state and that the saved ID is absent. The prototype exceeded the triage size gate
   > before it handled the empty Restart with field and the loading message beside Launch.

4. > **Show Custom approval flags in keyboard-highlighted Recent setups.** Two Recent setups in the launcher can show
   > the same agent and Custom approval mode while saving different flags. Pointer hover and the screen-reader label
   > give the full choice. When the command field keeps keyboard focus, show the saved flag words on the highlighted row
   > in both the command pane and narrow search results, before Enter. Keep the compact wrapper summary and avoid adding
   > a Tab stop. A prototype that wrapped the flags in place still clipped the last words when a highlighted row was
   > taller than the results area. Give keyboard users a way to read those words without changing the highlight, and
   > test two setups with a long shared flag prefix and different final arguments. The complete fix exceeds the triage
   > size gate.

### Acceptance criteria

Each item implemented removes its own `TODO.md` entry in its PR and passes its tests and (for PR 3 and PR 4) light/dark
visual review + published golden updates where references change:

1. **PR 1 (YOLO confirmation through host wake):** One-time YOLO consent is anchored to a verified `Install`
   (`identity.is_some()`) captured at original dispatch across New, Clone, Replace with, and Restart with, carried
   through `Wake` and checkout preview (`Armed`), cleared when that launch ends, never carried for ordinary/guarded
   launches or across changed/unverified installations. Update `SPEC_impl.md` ("Questions: the prompt") and add full
   regression tests.
2. **PR 2 (Hook setup late-arriving folder):** `HookAnswer` binds continuation to the readiness, destination (or
   disposable placement, ignoring retained ordinary host choice), and source warning shown with the answer across all
   launcher modes; settles folder defaults before comparing; saves the setting and requires a fresh Launch press when
   refused or destination changed. Add the exact reproduction test named in the TODO entry.
3. **PR 3 (Unread custom agent registrations):** Loading, failed-read (`Unavailable`), and deleted states are
   distinguished across Agent and Command tabs, stale chips/search picks, and Restart with (keeping Restart with's
   conversation field visible with neutral reason when loading/unreadable, and never guessing resumption/reporting).
   Retained entries from a prior successful read remain usable during transient read failures. Light/dark visual check.
4. **PR 4 (Custom approval flags on highlighted Recent setups):** Both wide (`command_pane`) and narrow (`ListRows`)
   results views show a pinned, non-focusable, wrapping detail strip below the results scroll viewport displaying the
   highlighted Recent setup's full Custom approval flags whenever it uses Custom mode with flags, keeping the compact
   wrapper summary and adding no Tab stop. Test two setups with a long shared flag prefix and different final arguments;
   light/dark visual check.

### Complexity kick-back rule (applies to every item in this plan)

The maintainer's instruction for `Plan follow-ups` items: _"If this ends up being complicated, kick it back to the to-do
queue. The executor should not continue if this turns very complicated."_ Evaluate each item independently. If any item
turns very complicated during execution (sprawling beyond the reviewed outline into unforeseen framework/subsystem
complexity), stop work on that item instead of pushing through: leave its `TODO.md` entry in `## Plan follow-ups`
updated with concrete findings explaining what made it complicated, log a `DECISION`, continue with the remaining
independent items in this plan, and summarize any kicked-back item in the delivery report.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped into one plan:** Approved by maintainer.
- **Custom approval flags UX:** Pinned detail strip below the results list (outside the scroll viewport, wrapping,
  non-focusable / no Tab stop).
- **Complexity kick-back:** Applies to all four items individually.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md")
  and `.agents/test-authoring.md` / `.agents/validation.md` / `.agents/test-evidence.md`.
- Every PR changing what the desktop app draws must pass visual review in light and dark appearances and publish updated
  goldens if references change (`python3 scripts/check-gui-goldens.py`).
- Never mutate process environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live
  Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`launcher/launch.rs`, `view/yolo_question.rs`, `view.rs`, `launch/wake.rs`, `view/checkout.rs`,
   `SPEC_impl.md`):**
   - Replace `confirm_yolo: bool` on `Launcher::submit`, `begin_host_wake`, and `submit_or_wait_for_preview` with
     `yolo_consent: Option<Install>` so unanchored `confirm_yolo: true` is unrepresentable.
   - Extend `Sent::installation` in `launcher/launch.rs` to capture `Install` for Create, Replace, and Restart, **and
     guard `settle`'s `EARLIER_SETUP_REFUSED` / `EARLIER_SETUP_TIMED_OUT` rewrite** (e.g. on `source.is_none()`) so
     unrelated Replace/Restart refusal wording does not change unintentionally.
   - Before waking, require `consented.identity.is_some()` and `Install::of(&host) == consented` (since `Install`
     derives `PartialEq`, `identity: None` must never match).
   - Do **not** duplicate `Install` in `Wake` — `Wake` already stores and continuously enforces `install: Install`. Add
     only `confirm_yolo: bool` to `Wake`, passing `wake.confirm_yolo.then(|| wake.install.clone())` to
     `continue_after_wake`, and carry `yolo_consent: Option<Install>` on `Armed` (`view/checkout.rs`).
2. **PR 2 (`launcher/view/hook_question.rs`, `launcher/view.rs`):**
   - Keep a dedicated destination snapshot on `HookAnswer` (not `YoloAsk`'s `Option<Request>`, because sleeping hosts
     can wake while `request()` is `None`): `shown_launch: Option<ShownHookDestination>` (`None` when `self.refusal(cx)`
     is `Some`), carrying `source: SourceConsent` and `Disposable(LaunchTarget)` vs
     `Host { install: Option<Install>, checkout: bool, folder: String, folder_default: FolderDefault, can_wake: bool }`.
   - Extend `Launcher::render`'s `HookQuestion` refresh (currently limited to Replace/Restart) to **all** modes right
     after `settle_folder_default(cx)`.
   - In `continue_launch`, settle folder default first, compare `answer.shown_launch` against current
     `ShownHookDestination`, and clear `hook_question` without calling `press_at` if `None` or mismatched.
3. **PR 3 (`launcher/draft.rs`, `launcher/view.rs`, `launcher/view/choices.rs`, `launcher/request.rs`,
   `launcher/view/more_options.rs`):**
   - Replace the 4 scattered custom-agent booleans on `LaunchDraft` with one enum:
     `Present { supports_resumption: bool, supports_reporting: bool } | Loading | Unavailable(HostText) | Deleted`.
   - Do **not** add a new cache: `Collections` already retains entries from the last successful read when
     `Availability::Unavailable` arrives. Resolve a custom agent ID with one shared helper that checks
     `collections.get::<CustomAgent>(id)` **first** (`Present`), and only when absent checks
     `collections.meta(Collection::CustomAgents)` (`None` -> `Loading`, `Unavailable` -> `Unavailable`, `Available` ->
     `Deleted`).
   - Reuse that helper in `launcher/view/choices.rs`, `view.rs` (Command tab retained choice), `request.rs` (keeping
     `offers_conversation()` true when `requires_conversation()` in `Loading`/`Unavailable`), and `more_options.rs`
     (only showing the non-reporting warning when `Present { supports_reporting: false, .. }`).
4. **PR 4 (`launcher/view/field_list.rs`, `launcher/view/search/start.rs`, `launcher/view/search.rs`):**
   - Add `pub(super) footer: Option<AnyElement>` to `Drawn` in `launcher/view/field_list.rs` (other callers pass
     `None`). In `ListRows::render`, move `layers::surface(cx)` onto an outer flex-col div wrapping the scrolling
     `drawn.list` (`max_h(LIST_MAX_HEIGHT)`) followed by `.children(drawn.footer)` (`flex_none`, outside scroll). In
     `start.rs` (`command_pane`), make the pane a flex-col with `drawn.list` as `flex_1 min_h_0` followed by
     `.children(drawn.footer)` (`flex_none`).
   - Share **one** helper building the detail strip from the highlighted `LaunchHistoryEntry`'s custom
     `permission_flags`, rendered via `HostSentence` / `HostText::from_host` with wrapping, no `.truncate()`, and no
     focus handle / Tab stop, reused across both wide and narrow search/recents modes.

## Scope reassessment

Before implementing a substantial departure from the outline (or if repeated corrective reviews hit the same component),
run a fresh-context review at your own model and effort with this charter verbatim:

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

Supply the request, decisions, outline, current diff, and proposed departure. A close reuse-versus-duplication call
within agreed scope is decided after reassessment and logged as `DECISION`. If a same-model sub-agent discovers an
unapproved new mechanism mid-unit, leave that part unimplemented and report it rather than inventing it.

## How to run

### Checkout check

This goal runs inside the checkout the executing session runs in, which must be a checkout of GitHub repository
`scode/farhelm` (the nearest `.git` or `.jj` checkout containing the working directory). Confirm that first. If it is
not, stop before the resume protocol, write nothing (not even the log), and report the mismatch. Every project path in
this file is relative to that checkout's root.

### Resume protocol

Right after confirming the checkout, invoke the `agent-resumeable` skill with the working log at
`farhelm-plan-launcher-consent-and-custom-agent-fixes-log.md` in the parent directory of the executing checkout. If the
log already exists, read it and resume where the previous session left off (cross-checking against VCS state and open
PRs) rather than starting over. If it does not exist, this is a fresh start.

### Execution mode

No-workhorse mode (explicit). Do not send any unit of your own decomposition to a weaker model or lower effort than your
own, and do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point (if
`scode-galaxy-brain` is active, stop it). Same-model sub-agents at your own effort are allowed at your judgment when
they speed up independent work without losing quality (using separate worktrees or jj workspaces if concurrent); you
still own the `jjstack` stack and every PR. The sub-agents required by `plans/AGENTS.md`, "Sub-agents this file
requires" (resume check, cold reads of Blocked questions and reports), are exempt from any delegation restriction. Scope
reassessment runs at your own model and effort; the per-PR review gate keeps its demanded model and effort below.

### PR discipline

Build a linear stack of reviewable draft PRs using the `jjstack` skill, based on `main@origin` (or this plan's own open
PRs on resume, never another plan's PRs), per `plans/.agents/executing.md`, "Executing one plan". Use bookmarks
`plan/launcher-consent-and-custom-agent-fixes/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never
merge. Never edit files under `plans/`. Restructure existing open PRs of this plan instead of stacking corrections on
top. Remove each completed item's `TODO.md` entry in its PR (or leave it updated if kicked back).

### Review gate

Before finishing any PR in this stack, run a fresh-context review of its changes using a **single fresh-context agent on
`gpt-6.2-sol` at `high` effort** (launched natively if available or through `scode-harness-shellout`, never hardcoding a
launch command here). Pass the executing checkout root, commit range or bookmark, and a private findings file in your
scratch directory, plus this full charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved; report findings to the supplied file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix; edit nothing and touch no VCS state.

When the PR changes tests or test fixtures, also supply `.agents/test-authoring.md` verbatim per `AGENTS.md`, "Tests and
validation". Address all valid findings before moving on.

### Resource watchdog

Right after the resume protocol and before launching sub-agents or builds, start the `scode-build-goal` skill's
`scripts/watchdog` (resolved via `realpath` next to that skill's `SKILL.md`), with status file
`farhelm-plan-launcher-consent-and-custom-agent-fixes-watchdog.json` beside the working log, watching the executing
checkout root, `$HOME`, `/tmp`, the log's parent directory, your scratch directory, and any worktrees on other
filesystems, at a 60-second interval with default thresholds. Follow the `scode-build-goal` skill's Resource watchdog
section for harness wiring, startup verification, acting on alerts, and stopping the monitors at close.

### Decision logging and unattended fallback

Log major design or tradeoff choices in the working log as `DECISION` with alternatives considered.

This plan runs unattended. Never ask the maintainer anything in conversation — no direct questions, no harness question
tool, no optional follow-up offers, and no turn ending waiting for confirmation. Decide routine implementation forks
within the agreed scope and log them as `DECISION`. If a genuine fork requires a maintainer decision (material scope
expansion, weakened guarantee, omitted requirement, or unresolved design conflict), push all consistent work and block
per `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Things the maintainer should know and
possible follow-ups go into the delivery report.

### Done criterion

Every non-kicked-back item's PR exists as a draft on GitHub, satisfies its acceptance criteria and the latest
`## Decisions` entry (if any), and has passed its `gpt-6.2-sol` high review gate (and any kicked-back item has its
updated `TODO.md` note and `DECISION` log entry). Delivering the report is the executor's closing step (step 11 of
`plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.
