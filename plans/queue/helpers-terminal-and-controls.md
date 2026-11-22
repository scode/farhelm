# Shared helpers: terminal input, controls, focus and trash

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers part of one TODO.md entry (bucket "Near term", directly under the bucket, not in a subsection). Four
sibling plans split that entry by area; this one owns terminal input mapping, the shared controls' rendering, focus
checks, the trash animation and the control tests. Quoted verbatim below: the entry's heading paragraph, the bullets
this plan has a share in, and the closing line with the plan references the planning PR adds. The entry's other bullets
belong to the sibling plans and to `wide-glyph-padding-cells`.

> - **Consolidate the helpers duplicated across Farhelm.** Work through the repeated helpers from the 2026-11-21 review.
>   Reuse the existing shared module where one fits and keep each helper scoped to its actual callers. Locations below
>   are from the reviewed revision `c9575db61abb31bebe43da0f11a76712e18725c1`. Read GUI locations relative to
>   `crates/gui/farhelm-gui/src/` and desktop locations relative to `crates/farhelm-desktop/src/`.
>
>   - `gui-controls-repeat-shared-rendering.md`. Share the absolute two-pixel focus-ring element across
>     `components/button.rs:1163–1200`, `select.rs:414–427`, `radio_group.rs:775–787,840–852,900–912,953–965`,
>     `menu_item.rs:312–326`, `list_row.rs:125–136`, `tab_strip.rs:240–256`, `copy_field.rs:288–299`,
>     `checkbox.rs:240–251` and `mod.rs:545–558` (all under `components/`). Pass insets and color, and keep corner
>     styling and ring visibility with each control. Share initial ASCII-letter capitalization for validation messages
>     in `components/custom_flags.rs:156–162`, `settings/recipes/editor.rs:125–131` and `templates/fields.rs:540–546`.
>     Keep punctuation and non-ASCII input intact.
>   - `gui-focus-checks-repeat-drawn-predicate.md`. In `focus.rs:761–765,1228–1234,1427–1431,1684–1688`, route repeated
>     current-frame root and focus-handle checks through `is_drawn`, keeping layer-specific conditions at each caller.
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
>   - `terminal-repeats-modifier-mapping.md`. Share the drag-selection modifier test and display name (Option on macOS,
>     Shift on Linux) between `terminal/drag_notice.rs:108–113` and `terminal/attached.rs:3717–3722`. In
>     `terminal/attached.rs:1942–1948,3522–3531`, share the five-field GPUI-to-terminal modifier conversion, with mouse
>     input calling `.bits()` on the result.
>   - `trash-repeats-animation-and-focus-wiring.md`. Share the ticker handle and redraw counter in `trash.rs:777–810`
>     and `components/spinner.rs:42–74`, deriving spinner steps modulo eight. In `trash/dialog.rs:185–191,239–245`,
>     share confirm/cancel focus restoration to the host Delete control or Delete all footer, taking the caller's mark.
>   - `control-tests-repeat-simulated-key-events.md`. Share simulated press/release helpers that construct the same
>     events, park the executor and check invariants. Move the component helpers to test support for the radio and
>     approval tests. See `components/tests/radio_group.rs:265–274`, `components/tests/mod.rs:76–97`,
>     `approvals/region_tests.rs:1447–1463` and `test_support.rs:221–227`.
>
>   Read the original findings with `git show 510f3b93721ee2393bd803d2cf91d73654415fbb:review_feedback_queue/<file>`.
>   Plan: `plans/queue/helpers-desktop-startup.md`. Plan: `plans/queue/helpers-terminal-and-controls.md`. Plan:
>   `plans/queue/helpers-hosts-and-sessions.md`. Plan: `plans/queue/helpers-launcher-and-templates.md`.

This plan's share of the entry:

- the whole `terminal-repeats-modifier-mapping.md` bullet;
- from the `templates-repeat-lookup-and-control-wiring.md` bullet, only its last two sentences, from "Share physical
  keypad digit lookup" to the end: the physical keypad digit lookup moving into the input mapping crate. The rest of
  that bullet belongs to `helpers-launcher-and-templates`;
- the whole `gui-controls-repeat-shared-rendering.md`, `gui-focus-checks-repeat-drawn-predicate.md`,
  `trash-repeats-animation-and-focus-wiring.md` and `control-tests-repeat-simulated-key-events.md` bullets.

The `terminal-grid-row-traversal-duplicated.md` bullet is in none of the four helper plans: it moved to the separate
plan `wide-glyph-padding-cells`.

Every change here is a refactor with no behavior change: the same bytes on screen, the same key handling, the same focus
moves.

### Where it stands (from planning research at main b34ceee9f)

No consolidation from this entry has landed since the review (`c9575db61`); every file still exists, and the duplicates
are still there at shifted lines. Re-scan each pattern rather than trusting the line numbers below or the entry's. Paths
below are under `crates/gui/farhelm-gui/src/` unless they name another crate.

- **Drag-selection modifier.** `terminal/drag_notice.rs` `forcing_key(platform)` (about lines 105-113) names Option on
  macOS and Shift on Linux; `terminal/attached.rs` `forces_selection(modifiers)` (about 3883-3888) tests `alt` on macOS
  and `shift` on Linux for `actions::THIS_PLATFORM`. The notice's doc comment already points at the forcing rule.
- **GPUI-to-terminal modifiers.** `terminal/attached.rs` builds `farhelm_input_mapping::Modifiers` from GPUI's five
  modifier fields twice: in the key path (about 1947-1955) and in `modifier_bits` for mouse reports (about 3687-3697,
  which calls `.bits()`).
- **Keypad digits.** `templates/armed.rs`: `MAC_KEYPAD_DIGITS` (about line 84) and `keypad_digit` (about 185-194) tell a
  keypad digit by its native identity (XKB `KP<digit>` or a macOS keycode).
  `crates/gui/farhelm-input-mapping/src/keys.rs` `keypad` (about 216-244) decodes the same identities to a digit, then
  adds the terminal's logical-character check and operator mapping, which stay in the terminal path.
- **Focus-ring element.** An absolute element with `.border_2()` drawn as a control's focus ring, 13 copies across
  `components/button.rs` (about 1176 and 1190), `checkbox.rs` (about 255-262), `copy_field.rs` (about 291-298),
  `list_row.rs` (about 165-172), `menu_item.rs` (about 397-404), `mod.rs` (about 598-605), `radio_group.rs` (about 939,
  1008, 1085 and 1153), `select.rs` (about 452-459) and `tab_strip.rs` (about 275-282). Insets, corner radius and when
  the ring shows differ per control. Other `.border_2()` users (`approvals/card.rs`, `prompt.rs`,
  `session_list/host_rail/status.rs`, `settings/tensorlake.rs`, `terminal/disclosure.rs`, `components/divider.rs`) were
  not checked; include one only if it is the same focus-ring element.
- **Validation-message capitalization.** Three identical `capitalized(reason)` helpers that uppercase the first byte
  when it is an ASCII letter: `components/custom_flags.rs` (about 156-162), `settings/recipes/editor.rs` (about
  125-131), `templates/fields.rs` (about 559-565, `pub(super)`). `approvals/model.rs` `sentence_case` is a different
  rule (it skips a leading ⚡) and stays out.
- **Drawn check in focus.** `focus.rs` has `is_drawn(handle)` (about 730) as "the root contains `handle` in the frame on
  screen". The same check is written out by hand at about 631 and 639 (inside the in-layer move, which already holds a
  `root`), about 1220-1225 (through `state.root.upgrade()`), about 1433 and about 1708. Layer-specific conditions stay
  at each caller.
- **Trash and spinner tick counters.** `trash.rs` `Ticks`/`Turn` (about 833-866) count ticker notifications on their own
  entity and redraw the owner; its doc comment says it has "the same shape as" `components/spinner.rs`'s `Spinner`/
  `Turn` (about 44-74), which counts steps modulo eight (`STEPS`).
- **Trash confirm and cancel focus.** `trash/dialog.rs`: the confirm path (about 184-191) and the dismissal path (about
  239-245) both look up the host's "Delete all on" button or the footer's Delete all and call `focus::focus_in_layer`
  with a mark; confirm already holds the mark it took.
- **Simulated key events in tests.** `components/tests/mod.rs` `press`/`release`/`held_key` (about 77-122),
  `components/tests/radio_group.rs` `key_up` (about 267), `approvals/region_tests.rs` (about 1447-1465), and
  `test_support.rs` (`keys`, about 237) each build key events, park the executor and check invariants.

### Acceptance criteria

1. Each sub-item above has one shared helper (or is skipped per "Decisions already made", with the reason logged and
   reported), placed in an existing shared module where one fits and scoped to its actual callers.
2. The drag notice names the key the forcing rule tests, from one mapping, for both platforms.
3. The keypad digit lookup lives in `farhelm-input-mapping` and both the template sequence and the terminal's `keypad`
   use it; the terminal keeps its logical-character check and operator mapping, the sequence keeps its cancellation.
4. Each control keeps its own insets, color, corners and ring visibility; only the element is shared.
5. Capitalization behaves exactly as now for punctuation, non-ASCII and empty input.
6. No behavior changes anywhere, and every GUI golden stays byte-identical (see "No visual change").
7. This plan's part of the TODO entry is removed by the rule under "PR discipline".

## Decisions already made

Settled with the maintainer during planning (2026-11-21):

- **Split by area into four plans**, of which this is one; siblings are `helpers-desktop-startup`,
  `helpers-hosts-and-sessions` and `helpers-launcher-and-templates`. This plan waits on nothing.
- **The keypad sub-item is here**, not with the rest of the templates bullet, because it moves into the input mapping
  crate.
- **Skipping a sub-item:** you may skip a sub-item you judge not worth sharing (for example two copies that differ in a
  way sharing would blur), recording why in the working log as a DECISION and in the report.
- **New copies are in scope by default.** Re-scan each pattern rather than trusting the old line numbers.
- **No dependency:** this plan waits on no other plan (the maintainer approved waits only for the two plans that note
  one); overlap with the in-flight `triage-gui-misc` is handled by a careful rebase (see "Fallbacks").
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

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation", and
  its "Instructions by path" for every path you touch (`.agents/validation.md` for any test).
- `.agents/test-authoring.md` for every test added or changed, and `python -B scripts/check-test-sleeps.py` for the
  test-only PR.
- `.agents/validation.md`: the GUI crates' tests (`farhelm-gui`, `farhelm-input-mapping`) and the GPUI goldens entry.

**Planner proposals:**

- One small per-platform mapping for the drag-selection modifier with a display name and a modifier test, beside the
  terminal's forcing rule; the notice keeps taking a platform so its tests can name both.
- A `From`-style conversion (or one function) for GPUI modifiers to `farhelm_input_mapping::Modifiers`; the mouse path
  calls `.bits()` on it.
- A public keypad-digit lookup in `farhelm-input-mapping` over its `NativeIdentity`, which `keys.rs` `keypad` and
  `templates/armed.rs` both call (the template side converts GPUI's native identity as `attached.rs` already does).
- A focus-ring element builder in the components module taking the insets (or offsets) and the color, returning the
  element so each control adds its own `rounded(...)` and decides whether to add it.
- The trash's `Ticks` becomes the shared tick counter and the spinner derives its step from the count modulo eight, or
  the other way round, whichever leaves both doc comments simplest; the spinner's reduced-motion behavior (a step that
  stands still) must hold.

## Outline

1. `plan/helpers-terminal-and-controls/01-terminal-modifiers`: the drag-selection modifier mapping and the GPUI modifier
   conversion. `refactor:`, no fragment.
2. `plan/helpers-terminal-and-controls/02-keypad-digit`: the keypad digit lookup in the input mapping crate.
   `refactor:`, no fragment.
3. `plan/helpers-terminal-and-controls/03-focus-ring-element`: the shared focus-ring element across the components.
   `refactor:`, no fragment.
4. `plan/helpers-terminal-and-controls/04-validation-capitalization`: one capitalization helper for the three forms.
   `refactor:`, no fragment.
5. `plan/helpers-terminal-and-controls/05-focus-drawn-check`: route `focus.rs`'s hand-written drawn checks through
   `is_drawn`. `refactor:`, no fragment.
6. `plan/helpers-terminal-and-controls/06-trash-ticks-and-focus`: the shared tick counter and the trash dialog's opener
   focus helper. `refactor:`, no fragment.
7. `plan/helpers-terminal-and-controls/07-control-test-keys`: shared simulated press and release helpers in test
   support; this plan's part of the TODO entry removed. `test:`, no fragment.

Small neighbouring sub-items may share a PR, and a large one may split, as long as each PR stays one reviewable
refactor; log the shape you chose.

## Fallbacks

- A sub-item whose copies turn out to differ in a way sharing would blur: skip it, log a DECISION, and say so in the
  report. Likely candidates: a `focus.rs` site whose surrounding code holds its own `root` binding for a second check
  (about 631-639), where routing one half through `is_drawn` would read worse; a focus ring whose geometry does not fit
  a shared builder without a special case.
- If a sub-item has already been consolidated on main by other work when you reach it, record that and move on.
- **Concurrency.** The siblings `helpers-desktop-startup`, `helpers-hosts-and-sessions` and
  `helpers-launcher-and-templates` touch disjoint files except: `helpers-launcher-and-templates` also edits
  `templates/armed.rs` (the available-name read, near the keypad code); `helpers-hosts-and-sessions` adds a drawn
  predicate to `test_support.rs` beside its bounds helper, may put a progress-title helper near the spinner, and edits
  the approvals module (`approvals/host_add.rs`, `approvals/tunnel.rs`), whose `approvals/region_tests.rs` this plan's
  control-tests PR edits. Those rebases are mechanical: keep both sides. The in-flight `triage-gui-misc` plan touches
  the terminal view (input-method guards in `terminal/attached.rs`), the input mapping crate, templates and Settings,
  and the `interrupted-upload-notices` plan (planned at the same time) also edits `terminal/attached.rs`; rebase across
  either carefully (`.agents/careful-rebase.md`). Neither is a dependency.
- **Fence: no visual change.** See "No visual change" under "How to run".

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-helpers-terminal-and-controls-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-helpers-terminal-and-controls-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes: the touched modules' GUI tests,
`farhelm-input-mapping`'s tests for PR 02, and the goldens for every PR that touches drawing code. Apply
`.agents/test-authoring.md` to every test you add or change, and run `python -B scripts/check-test-sleeps.py` as
`docs/test-sleep-check.md` prepares it when Rust tests change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

### No visual change

These PRs must not change what the app draws, so there is no visual review. Byte-identical GUI goldens are the oracle,
and the focus-ring PR leans on them most: run the goldens (`python3 scripts/check-gui-goldens.py` through the recorder,
`.agents/validation.md`) for every PR that touches drawing code, in compare mode, never `--update`. Any golden that
changes must be investigated and the change reverted, unless it is a pre-existing nondeterminism, which you show by
reproducing it on the PR's base and log (with the scene and the evidence) in the working log and the report. Never
publish new golden pictures from this plan.

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
  `plan/helpers-terminal-and-controls/<nn>-<short-name>` as the outline names them.
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
and outline). Tell it that every PR here must be behavior-free, so any behavior difference is a finding. When a PR
changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root
`AGENTS.md` requires.

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
