# Shared helpers: launcher, templates and golden scene fixtures

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

This plan runs after `triage-gui-misc.md` and `launch-wakes-suspended-host.md` have landed: both change the launcher
(and the first also templates), which this one consolidates.

## The goal

This plan covers part of one TODO.md entry (bucket "Near term", directly under the bucket, not in a subsection). Four
sibling plans split that entry by area; this one owns the launcher, templates and the golden scenes' fixtures. Quoted
verbatim below: the entry's heading paragraph, the bullets this plan has a share in, and the closing line with the plan
references the planning PR adds. The entry's other bullets belong to the sibling plans and to
`wide-glyph-padding-cells`.

> - **Consolidate the helpers duplicated across Farhelm.** Work through the repeated helpers from the 2026-11-21 review.
>   Reuse the existing shared module where one fits and keep each helper scoped to its actual callers. Locations below
>   are from the reviewed revision `c9575db61abb31bebe43da0f11a76712e18725c1`. Read GUI locations relative to
>   `crates/gui/farhelm-gui/src/` and desktop locations relative to `crates/farhelm-desktop/src/`.
>
>   - `launcher-repeats-choices-and-summary-formatting.md`. Recheck these call sites against the landed
>     `launcher-command-field` plan's redesign. Share the lowercase effort spelling and `default` fallback for search
>     and the change notice beside the labels in `permission/words.rs:16–27`, used from `launcher/search.rs:413–419` and
>     `launcher/view/changed.rs:104–110`. Share default-effort help across the select, radio group and search at
>     `launcher/view/choices.rs:231–235,260–265` and `launcher/search.rs:361–362`. Share `gh:owner/name` on top of
>     `repo_identifier` for `launcher/recents.rs:244–250`, `launcher/view/checkout.rs:1184–1191` and
>     `launcher/draft.rs:194–197`, adding preview status where applicable. In `launcher/entry.rs:348–361,371–379`, share
>     the harness-specific unsupported-resume formatter and uncaptured-conversation text, keeping the Agent and Command
>     conditions. In `launcher/view/folder_list.rs:89–106` and `crates/farhelm-proto/src/text.rs:113–132`, use one
>     iterator over whole escaped display units so truncation splits only between units. In
>     `launcher/view/recents.rs:354–383` and `launcher/view/search.rs:654–655,683–694`, the two recent-setup surfaces
>     repeated resolved permission rendering, including danger ink for YOLO. The old strip omitted the default. In the
>     redesigned launcher, look at `recent_choice` in `launcher/view/search/start.rs` and `recent_lines` in
>     `launcher/view/search.rs`. Share the permission projection and keep the Start column's chip and default omission,
>     search's inline text, and each surface's summary local.
>   - `gui-screenshot-scenes-repeat-fixture-wiring.md`. The landed `goldens-fast-and-targeted` plan changed the runner.
>     The scene fixtures still need consolidation. Share one-shot dialog setup when a fake connection first has a UI
>     session. Give the helper each scene's setup callback and retain the connection, fake helm and subscription. See
>     `goldens/hosts.rs:55–85,125–146,154–176,188–209,795–817,960–988`,
>     `goldens/other_dialogs.rs:235–253,282–299,332–350`, `goldens/templates.rs:52–69,129–167,208–228` and
>     `goldens/settings.rs:122–183`. Share the session fixture's title and `TEST_NOW` activity/seen timestamps while
>     scenes supply their status (`goldens/session_list.rs:42–47,114–119,154–160`, `goldens/other_dialogs.rs:112–117`,
>     `goldens/mod.rs:301–307`). Move the upgrade-floor reinstall hover formatter into GUI `update_readout` so
>     `goldens/desktop.rs:371–384` and desktop `updater.rs:588–609` use the same words.
>   - `templates-repeat-lookup-and-control-wiring.md`. In `templates/dispatch.rs:221–243,295–313`, share launcher and
>     switcher template lookup and unavailable/missing-name wording, with each surface supplying its sentence ID. Share
>     the available-name read in helm order in `templates/armed.rs:475–499` and `templates/editor.rs:1494–1507`. The
>     sequence's Enter lookup still uses retained names. Share the Base name/Fixed name radio and help in
>     `templates/editor/rows.rs:652–678` and `templates/save_panel.rs:308–340`, passing each control's ID, selection and
>     handler. Share the typed `GET /api/launch-catalog` request and decode in `templates/dialog.rs:766–789` and
>     `launcher/view.rs:477–500`, keeping retries, failure display and quick-launch continuation local. Share physical
>     keypad digit lookup for macOS keycodes and XKB names in the input mapping crate (`templates/armed.rs:84,186–194`,
>     `crates/gui/farhelm-input-mapping/src/keys.rs:218–244`). Keep the terminal's logical-character check and operator
>     mapping.
>
>   Read the original findings with `git show 510f3b93721ee2393bd803d2cf91d73654415fbb:review_feedback_queue/<file>`.
>   Plan: `plans/queue/helpers-desktop-startup.md`. Plan: `plans/queue/helpers-terminal-and-controls.md`. Plan:
>   `plans/queue/helpers-hosts-and-sessions.md`. Plan: `plans/queue/helpers-launcher-and-templates.md`.

This plan's share of the entry:

- the whole `launcher-repeats-choices-and-summary-formatting.md` bullet;
- the `templates-repeat-lookup-and-control-wiring.md` bullet except its last two sentences, from "Share physical keypad
  digit lookup" to the end (the physical keypad digit lookup, which belongs to `helpers-terminal-and-controls`);
- the `gui-screenshot-scenes-repeat-fixture-wiring.md` bullet except its last sentence (the upgrade-floor reinstall
  hover formatter, which belongs to `helpers-desktop-startup`).

The `terminal-grid-row-traversal-duplicated.md` bullet is in none of the four helper plans: it moved to the separate
plan `wide-glyph-padding-cells`.

Every change here is a refactor with no behavior change: the same bytes on screen, the same words, the same requests.

### Where it stands (from planning research at main b34ceee9f)

No consolidation from this entry has landed since the review (`c9575db61`); every file still exists, and the duplicates
are still there at shifted lines. The launcher changed heavily since the review (the landed `launcher-command-field`
plan's redesign), and the two plans this one waits on will move it again. Re-scan each pattern rather than trusting the
line numbers below or the entry's. Paths are under `crates/gui/farhelm-gui/src/` unless they name another crate.

- **Effort search and change words.** `launcher/search.rs` `effort_word` (about lines 439-445) and
  `launcher/view/changed.rs` `effort_word` (about 103-109) both lowercase `effort_label` and use `default` for none; the
  effort labels are in `permission/words.rs` (`effort_label`, about 14-30). One lowercases with `to_ascii_lowercase`,
  the other with `to_lowercase`; the labels are ASCII, so the results are the same.
- **Default-effort help.** "Use the agent's default reasoning effort" is written out in `launcher/view/choices.rs`
  (about 281 and 309, the select and the radio group) and `launcher/search.rs` (about 388, the search row).
- **Managed checkout label.** `gh:owner/name` is spelled in `launcher/recents.rs` `destination_label` (about 244-250,
  from a history entry's owner and name), `launcher/view/checkout.rs` (about 1286, and about 1290 with
  ` (preview pending)`), both on `launcher/draft.rs` `repo_identifier` (about 195). New since the review:
  `templates/save.rs` (about 186, from a template destination's text).
- **Restart refusals.** `launcher/entry.rs` (about 344-386): "<agent type> sessions can't be resumed", formatted with
  `harness_label`, appears in the Agent arm (about 353-357, when `selection.harness.agent_kind().restart_readiness()` is
  `None`) and in the Command arm (about 380-382, for `RestartOffer::NoConversationReporting`), and "No captured
  conversation to resume" in both (about 366 and 384). The two "has no resume command" sentences differ ("use Clone or
  Replace with" against "use Replace with") and stay distinct. The Agent and Command conditions stay as they are.
- **Folder label escape units.** `launcher/view/folder_list.rs` `tail_label` (about 86-115) re-implements, character by
  character, the rule `crates/farhelm-proto/src/text.rs` `escaped` (about 122-135) uses for which characters become
  `<U+XXXX>` notation (a presentation-unsafe character, or the `<` of a literal `<U+`), so it can cut only between whole
  escapes.
- **Recent-setup permission rendering.** `launcher/view/search.rs` `recent_lines` (about 999-1070) and
  `launcher/view/search/start.rs` `recent_choice` (about 432-505) both resolve the effective permission with
  `selection.harness.request_permission(selection.permissions)`, test it for YOLO, and render its label in the danger
  ink. The Start column draws a chip and omits the default; search writes it inline. (The old strip in
  `launcher/view/recents.rs` that the entry names is gone.)
- **Template lookup refusals.** `templates/dispatch.rs` `search_row_batch` (about 216-245) and `pick_from_switcher`
  (about 298-330) look a template up by name in `stored_templates` and build the same "Couldn't load templates: …;
  search tl: and retry" and "No template is named "…"; it may have been deleted" parts, with different sentence ids
  (`LAUNCHER_REFUSAL`, `PAGE_REFUSAL`).
- **Available template names.** `templates/armed.rs` `advisory_names` (about 476-483, with `names_in` about 493-499) and
  `templates/editor.rs` `stored_names` (about 1516-1531) both require the templates collection to be available and
  collect names in the helm's order. The sequence's Enter lookup (`current_names`, about 486-490) deliberately uses the
  retained names whatever the availability, and stays separate.
- **Name-kind radio.** `templates/editor/rows.rs` (about 680-700) and `templates/save_panel.rs` (about 308-340) build
  the Base name / Fixed name radio with the same labels and help.
- **Model catalog read.** `templates/dialog.rs` (about 789-810) and `launcher/view.rs` (about 477-500) each build and
  decode `GET /api/launch-catalog`; retries, failure display and quick-launch continuation stay with each caller.
- **One-shot dialog setup in golden scenes.** A scene observes the fake connection until it has a UI session, runs its
  setup once, and keeps the connection, fake helm and subscription alive: `cx.observe(&connection, ...)` in
  `goldens/hosts.rs` (about 102, 170, 202, 252, 448, 890, 952 and 1056), `goldens/templates.rs` (about 56, 157, 234),
  `goldens/other_dialogs.rs` (about 278, 373, 424), `goldens/settings.rs` (about 144) and `goldens/session_list.rs`
  (about 719); `goldens/terminal.rs` (about 252) has a variant. More copies than the review found.
- **Timestamped session fixture.** `.activity(TEST_NOW - ago).seen(Some(TEST_NOW - ago))` with a title, in
  `goldens/session_list.rs` (about 47, 143, 184), `goldens/other_dialogs.rs` (about 186) and `goldens/mod.rs` (about
  327), and since the review also `goldens/session_pane.rs` (about 40, 167) and `goldens/sign_in.rs` (about 33). Scenes
  set their own statuses.

### Acceptance criteria

1. Each sub-item above has one shared helper (or is skipped per "Decisions already made", with the reason logged and
   reported), placed in an existing shared module where one fits and scoped to its actual callers.
2. Per-harness wording keeps coming from the existing per-harness helpers; no harness branching is added (see
   "Requirement sources", Code rules).
3. Folder labels still cut only between whole escape units, with the escaping rule defined once in `farhelm-proto`.
4. Each launcher surface keeps its own summary, chip and default omission; each template surface keeps its own sentence
   id, control id, selection and handler; each catalog caller keeps its own retries and failure display.
5. No behavior changes anywhere, and every GUI golden stays byte-identical, including the scenes whose fixtures this
   plan rewires (see "No visual change").
6. This plan's part of the TODO entry is removed by the rule under "PR discipline".

## Decisions already made

Settled with the maintainer during planning (2026-11-21):

- **Split by area into four plans**, of which this is one; siblings are `helpers-desktop-startup`,
  `helpers-terminal-and-controls` and `helpers-hosts-and-sessions`.
- **Dependency:** this plan runs after `triage-gui-misc.md` (in flight at planning time) and
  `launch-wakes-suspended-host.md` (pending at planning time), which both change the launcher.
- **The keypad sub-item and the reinstall hover formatter are not here:** they belong to `helpers-terminal-and-controls`
  and `helpers-desktop-startup`.
- **Skipping a sub-item:** you may skip a sub-item you judge not worth sharing (for example two copies that differ in a
  way sharing would blur), recording why in the working log as a DECISION and in the report.
- **New copies are in scope by default.** Re-scan each pattern rather than trusting the old line numbers.
- **No visual change:** byte-identical GUI goldens are the oracle. A golden that changes is a red flag, not something to
  accept (see "No visual change").
- **Commit types:** every PR is `refactor:` (or `test:` for a test-only one) and behavior-free, so none needs a
  changelog fragment.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- **Code rules (a fence).** Root `AGENTS.md`, "Code rules", governs the restart refusals and the effort help:
  "Everything Farhelm does differently per agent harness is answered per harness in a few named places, not by comparing
  harnesses at the call site. [...] do not write `kind == X`, `matches!` over specific harnesses, or a `_` arm over
  harnesses in shared code: each of those hands a new harness whatever the branch does for the ones it does not name." A
  shared formatter keeps taking the harness value and routes through the existing per-harness helpers (`harness_label`,
  `agent_kind().restart_readiness()`, `request_permission`); it adds no harness branching. Read the module docs of
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` before touching the restart refusals.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation", and
  its "Instructions by path" for every path you touch (`.agents/validation.md` for any test).
- `.agents/test-authoring.md` for every test added or changed, and `python -B scripts/check-test-sleeps.py` for the
  test-only PR.
- `.agents/validation.md`: the GUI crate's and `farhelm-proto`'s tests, and the GPUI goldens entry.

**Planner proposals:**

- Effort search word and default-effort help as a function and a constant beside `effort_label` in
  `permission/words.rs`.
- The `gh:` label as one function beside `repo_identifier`; whether the history entry and the template destination can
  use it depends on their types; log the answer for each.
- The restart refusals as one formatter taking the harness (through `harness_label`) and one constant for the
  no-conversation words, both in `launcher/entry.rs`.
- An iterator in `crates/farhelm-proto/src/text.rs` over whole display units (each character, or one escape as a unit
  with its width), which `escaped` itself uses and `tail_label` walks.
- The recent permission projection as one function returning the label and whether it is YOLO, used by both surfaces.
- One golden scene helper for the one-shot dialog setup that takes the scene's setup callback and keeps the connection,
  fake helm and subscription alive, and one session fixture builder taking the title and the age.

## Outline

1. `plan/helpers-launcher-and-templates/01-effort-words`: the effort search word and the default-effort help.
   `refactor:`, no fragment.
2. `plan/helpers-launcher-and-templates/02-checkout-label`: the `gh:owner/name` label. `refactor:`, no fragment.
3. `plan/helpers-launcher-and-templates/03-restart-refusals`: the restart refusal formatter and words. `refactor:`, no
   fragment.
4. `plan/helpers-launcher-and-templates/04-escape-units`: the display-unit iterator in `farhelm-proto` and the folder
   label on top of it. `refactor:`, no fragment.
5. `plan/helpers-launcher-and-templates/05-recent-permission`: the recent-setup permission projection. `refactor:`, no
   fragment.
6. `plan/helpers-launcher-and-templates/06-template-lookups`: the template lookup refusals and the available-name read.
   `refactor:`, no fragment.
7. `plan/helpers-launcher-and-templates/07-name-kind-and-catalog`: the name-kind radio and the typed model catalog read.
   `refactor:`, no fragment.
8. `plan/helpers-launcher-and-templates/08-golden-fixtures`: the golden scenes' one-shot dialog setup and session
   fixture builder; this plan's part of the TODO entry removed. `test:`, no fragment.

Small neighbouring sub-items may share a PR, and a large one may split, as long as each PR stays one reviewable
refactor; log the shape you chose.

## Fallbacks

- A sub-item whose copies turn out to differ in a way sharing would blur: skip it, log a DECISION, and say so in the
  report. Likely candidates: the `gh:` label where a caller's type does not hold a `GithubRepo`; a golden scene whose
  setup differs from the others (the terminal variant).
- If a sub-item has already been consolidated on main by other work (the two plans this one waits on included) when you
  reach it, record that and move on. If one of them moved a site so far that the entry's description no longer matches,
  re-derive the duplicate from the finding (`git show` line in the entry) and log what you did.
- **Concurrency.** The siblings `helpers-desktop-startup`, `helpers-terminal-and-controls` and
  `helpers-hosts-and-sessions` touch disjoint files except: `helpers-desktop-startup` also adds a function to
  `crates/farhelm-proto/src/text.rs` (the peer text formatter) and edits `goldens/desktop.rs`;
  `helpers-terminal-and-controls` edits `templates/armed.rs` (the keypad lookup, near the name read) and
  `templates/fields.rs` (capitalization); `helpers-hosts-and-sessions` may edit `templates/shortcuts.rs` (the preference
  command). Those rebases are mechanical: keep both sides. The `interrupted-upload-notices` plan, planned at the same
  time and not a dependency, edits `goldens/mod.rs` (where one session fixture copy lives, about 327) and adds a golden
  picture to `crates/gui/farhelm-gui/tests/goldens.txt`; rebase across it carefully (`.agents/careful-rebase.md`). A
  manifest line that arrives through such a rebase is that plan's, not a change of yours: compare against your PR's own
  base.
- **Fence: no visual change.** See "No visual change" under "How to run".

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-helpers-launcher-and-templates-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-helpers-launcher-and-templates-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes: the touched modules' GUI tests (launcher,
templates, permission words), `farhelm-proto`'s tests for PR 04, and the goldens for every PR that touches drawing code
or `goldens/`. Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change. Never take a
state-changing action against the live Farhelm install on this machine (root `AGENTS.md`, "The live install is
off-limits").

### No visual change

These PRs must not change what the app draws, so there is no visual review. Byte-identical GUI goldens are the oracle,
and the golden fixture PR leans on them most: run the goldens (`python3 scripts/check-gui-goldens.py` through the
recorder, `.agents/validation.md`) for every PR that touches drawing code or `goldens/`, in compare mode, never
`--update`. Any golden that changes must be investigated and the change reverted, unless it is a pre-existing
nondeterminism, which you show by reproducing it on the PR's base and log (with the scene and the evidence) in the
working log and the report. Never publish new golden pictures from this plan.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/helpers-launcher-and-templates/<nn>-<short-name>` as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- **The shared TODO entry.** Four sibling plans (`helpers-desktop-startup`, `helpers-terminal-and-controls`,
  `helpers-hosts-and-sessions`, `helpers-launcher-and-templates`) split the TODO entry "Consolidate the helpers
  duplicated across Farhelm.", and which of them lands last cannot be known. So each plan's last PR deletes only that
  plan's own part of the entry: its group bullets, and for a bullet split between plans only its own sentences (the
  `templates-repeat-lookup-and-control-wiring.md` bullet: the last two sentences, from "Share physical keypad digit
  lookup" to the end, are `helpers-terminal-and-controls`'s, the rest `helpers-launcher-and-templates`'s; the
  `gui-screenshot-scenes-repeat-fixture-wiring.md` bullet: the reinstall hover formatter sentence is
  `helpers-desktop-startup`'s, the rest `helpers-launcher-and-templates`'s). The text of a sub-item the plan skipped, or
  found already consolidated, is deleted with the rest of its share; the report carries the skip and its reason. The
  same PR deletes the plan's own `Plan:` reference from the entry's closing line. The plan that removes a split bullet's
  last part deletes the bullet. If that deletion leaves the entry with no group bullets at all, the same PR also deletes
  the entry's heading paragraph and its closing "Read the original findings" line with the plan references. While any
  bullet remains, the heading stays; that includes the `terminal-grid-row-traversal-duplicated.md` bullet, which the
  separate plan `wide-glyph-padding-cells` removes under this same rule. Apply this rule to `TODO.md` as it is on
  `main@origin` when you rebase for delivery; on a rebase conflict in this entry, keep every deletion from both sides
  and apply the rule again.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`). Every PR in this plan is `refactor:` (or `test:` if test-only) with no user-visible effect, so none adds
  a changelog fragment (root `AGENTS.md`, "Changelog fragments"). This plan's `## Release note` section is the
  maintainer's answer for the release's highlights, not an instruction to you (`plans/.agents/executing.md`, "Executing
  one plan", step 8). Never edit that section.
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
and outline). Tell it that every PR here must be behavior-free, so any behavior difference is a finding, and quote it
the Code rules fence above. When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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
alternatives considered: in particular those named under Fallbacks above, every sub-item you skip, and every review
finding you decided not to follow. The maintainer will ask for these later.

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
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, with
every GUI golden unchanged, and the last removing this plan's part of the TODO.md entry by the rule under "PR
discipline". Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
