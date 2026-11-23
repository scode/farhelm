# Make a pending upgrade more eye-catching

Written against main at bd95a669 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Trivial"). Verbatim:

> **Make a pending upgrade more eye-catching.** when there is an upgrade pending, make it a little more "eyey catching".
> in addition to being read, let's have some kind of "animation" like a bouncing arrow or something point at it every 15
> minutes while its being used or something liek that (details tbd during planning of todo item)

In product terms: when a desktop app update is installed and waiting for a restart, upgrade the version readout at the
top-right of the sidebar's top bar from bare red text (`↑ 0.40.0`) to a borderless tinted red pill that stands out at a
glance, and play a brief (~2.5-second) reminder animation — a bouncing right-pointing arrow (`→`) just to the left of
the pill combined with a gentle highlight pulse on the pill — once when the update first becomes ready and then every 15
minutes while the window is active and the user has interacted recently, skipping the animation under OS Reduce Motion.

### Where it stands (from planning research at main bd95a669; line numbers approximate)

- **Version readout & `Button` tier (`crates/gui/farhelm-gui/src/update_readout/view.rs:306-337`,
  `components/button.rs:66-120, 744, 883-932, 1176-1182`):**
  - `app-version-toggle` is the **sole** caller of `Tier::Readout { focus_fill: true }` across the entire crate (the
    other three `Tier::Readout` callers pass `focus_fill: false`).
  - Styling the pill ad hoc inside `Button::content(...)` while leaving `Tier::Readout { focus_fill: true }` would paint
    grey `palette.control_hover` behind the red pill on keyboard focus and leave `focus_fill` dead. Instead, simplify
    `Tier::Readout { focus_fill: bool }` to `Tier::Readout` and add a dedicated borderless pill tier (e.g.
    `Tier::UpdateReady { pulsing: bool }`) in `components/button.rs`:
    - Compact pill geometry: `py(px(2.))`, `px(px(7.))`, `rounded(px(5.))`, no border (`bordered = !matches!(...)`).
    - Excluded from the solid-blue open-menu look at `button.rs:744`
      (`!matches!(self.tier, Tier::Readout | Tier::UpdateReady { .. })`) so the toggle keeps its own red pill look while
      the update menu is open (`version-readout-menu` golden).
    - Focus ring uses `radius + px(3.)` (so the ring aligns cleanly with `5px` corners both before and after
      `plans/queue/button-restyle.md` changes `RADIUS_SM` from `3px` to `5px`).
    - Selecting a discrete `pulsing: bool` state on the tier lets `Button` own the pill background cleanly (no wash
      painted over the letters).
- **Theme tokens & font weight (`crates/gui/farhelm-gui/src/theme.rs:222-381`, `assets.rs:125-146`):**
  - `Palette` already defines translucent `danger_wash` (`--danger` at 14% alpha on both `DARK` and `LIGHT`, used by
    `prompt.rs:1316` and `goldens/foundations.rs:379`); do **not** alter `danger_wash` itself. Reuse `danger_wash` for
    the resting fill if 14% matches the mockup (adding one translucent hover/pulse token such as `danger_wash_hover` at
    ~24–30% alpha on `DARK` and `LIGHT`), or add dedicated rest + hover translucent tokens if rest needs ~16% on dark.
    Use `palette.danger_strong` for the pill text and `↑` (keeping `⚠` in `palette.warn` when
    `withdrawn || needs_reinstall`). Zero raw color constructors outside `theme.rs` (`crates/gui/clippy.toml:116-142`).
  - `assets::TERMINAL_FONT` (`JetBrainsMono Nerd Font`) bundles **only Regular (400) and Bold (700)** — not Medium or
    SemiBold. Use `FontWeight::BOLD` on the pending-upgrade pill text (matching the browser mockup where CSS weight 600
    on JetBrains Mono resolves to Bold 700 while keeping monospace consistency with the idle version readout).
- **Window-scoped user activity tracking (`crates/gui/farhelm-gui/src/focus.rs:306-344, 835-841, 1836`):**
  - `crates/gui/clippy.toml:156` bans `gpui::App::intercept_keystrokes` outside `actions.rs`; `focus::note_input`
    (`focus.rs:1836`) is already the single per-window funnel called for both every key-down (before `MovedOn`'s
    `is_held` early returns) and every mouse-down in capture phase.
  - Store `last_activity: Option<Instant>` and a single one-shot
    `activity_waiter: Option<Box<dyn FnOnce(&mut Window, &mut App)>>` on per-window `WindowFocus` (do **not** blur
    app-wide `MovedOn`'s contract and do **not** build a general pub-sub registry). In `note_input`, record
    `cx.background_executor().now()` and take/invoke `activity_waiter` outside the `WindowFocus` borrow (e.g. via
    `window.defer`). Expose two `pub(crate)` helpers: `focus::last_activity(window, cx) -> Option<Instant>` and
    `focus::on_next_activity(window, cx, callback)`.
  - Never call `Instant::elapsed()` (`crates/gui/clippy.toml:86`); compute elapsed durations with
    `cx.background_executor().now().saturating_duration_since(last)` (as `focus.rs:241` does).
- **Reminder scheduler & ~2.5-second burst (`crates/gui/farhelm-gui/src/update_readout/view.rs`, `ticker.rs`,
  `trash.rs:776-787`):**
  - Keep `VersionReadout::new` and `Sidebar::new` constructor signatures unchanged (do not thread `window`); register
    `observe_window_activation` lazily on the first `VersionReadout::render`, matching `terminal/attached.rs:2156-2161`.
  - **Cadence & deferral rules:**
    - Track `was_update_ready: bool` (starting `false`). On `false -> true`: if `!ticker::reduced_motion(cx)` and
      `ticker::is_active(window, cx)`, start a burst immediately; if inactive, set `reminder_due = true` so it fires
      when the window next activates. Do **not** reset or restart a burst when `update_ready` remains `true` across
      re-publishes (preserving `a_failed_restart_shows_nothing_inline` at `update_readout/view/tests.rs:515-532`).
    - Between bursts, hold at most one `ticker::Deadline` (`15 * 60` seconds) — **zero** `AnimationTicks`/`Animating`
      handles held during the quiet interval. While `ticker::reduced_motion(cx)` is true, arm no deadline at all; when
      reduced motion turns off, `render` sees `update_ready && !reduced_motion` and arms normally.
    - When the 15-minute deadline fires: if
      `update_ready && !ticker::reduced_motion(cx) && ticker::is_active(window, cx)` and `last_activity` is within the
      recent-activity window (5 minutes), start a burst; otherwise set `reminder_due = true` (and arm
      `focus::on_next_activity` if the window is currently active).
    - On `observe_window_activation` becoming active: if `reminder_due && update_ready && !ticker::reduced_motion(cx)`,
      fire immediately **without** rejecting on stale `last_activity` (activating the window is itself the user
      returning).
    - On `focus::on_next_activity` wake: if
      `reminder_due && update_ready && !ticker::reduced_motion(cx) && ticker::is_active(window, cx)`, fire immediately.
  - **Burst motion (reuse `ticker::AnimationTicks`, matching `TrashButton::wiggle` in `trash.rs:776-787`):**
    - Drive the ~2.5-second burst (~25 ticks at 10 Hz, 3 bounces toward the pill) with `ticker::AnimationTicks` and a
      pure step function returning `Option<BurstFrame>` (arrow opacity, horizontal offset `0..~5px`, and discrete pill
      pulse on/off). Dropping `self.burst = None` on completion, `update_ready == false`, `!ticker::is_active`, or
      `ticker::reduced_motion(cx)` immediately frees the 10 Hz tick handle and clears the arrow/pulse.
    - Always consult `ticker::reduced_motion(cx)` dynamically on every `render` (never cache it; works seamlessly with
      `plans/queue/toolkit-reduced-motion.md` without duplicating any GPUI observer hook).
  - **Narrow-sidebar & title-band safety (`sidebar/header.rs`, `farhelm-proto/src/ui_channel.rs:875`):**
    - Render `→` as text (matching `settings/recipes/editor.rs:1127`, no new SVG asset) in an `.absolute()` container
      just left of the pill using `.left(px(...))` so the pill itself never shifts horizontally and `Anchor` for
      `UpdateMenu` stays unchanged. Give the arrow no mouse listeners and no `title_band::claims_presses` so empty macOS
      title-band background remains draggable/zoomable (`SPEC.md:3817-3829`).
    - Do **not** add `overflow_hidden()` to `VersionReadout` or `sidebar/header.rs` (it would clip `Button`'s `-3px`
      keyboard focus ring).
    - Suppress the `→` arrow (keeping the pill highlight pulse!) whenever inline status (`Checking…` / `Installing…` /
      `Check failed`) is showing beside the toggle, or when the available horizontal space between the wordmark and the
      toggle is narrower than the arrow's room (~`24px`) at narrow sidebar widths (down to `296px`).
- **Goldens & specs (`crates/gui/farhelm-gui/src/goldens/desktop.rs:150-412`, `SPEC.md:816-825`):**
  - PR 1 updates the four existing `version-readout-*` scenes that render `update_ready: true`:
    `version-readout-update-ready`, `version-readout-progress`, `version-readout-outcomes`, and `version-readout-menu`.
  - PR 2 adds a frozen-frame burst row to `version-readout-update-ready` (or a dedicated scene) using a deterministic
    holding seam (bypassing `reduced_motion` when `update_ready == true` without creating live `AnimationTicks` or
    `Deadline` handles, matching `VersionReadout::holding_step` so `goldens/desktop.rs:153`'s `set_reduced_motion(true)`
    settles cleanly).
  - Update `SPEC.md:816-825` to describe the tinted red pill and 15-minute reminder animation; keep the idle tooltip
    text in `update_readout.rs:272` unchanged ("turns red and opens the Restart to update / What's new menu" remains
    accurate).

### Acceptance criteria

1. When `update_ready` is true, `app-version-toggle` renders as a borderless tinted red pill (`5px` corners, compact
   padding, bold monospace `↑ <version>`, translucent red fill on both Steel Dark and Steel Light via `theme::Token`,
   hover tint deepen, `2px` accent focus ring at `radius + 3px`, preserving `⚠` warning mark and open-menu appearance),
   and `Tier::Readout` drops its dead `focus_fill` field.
2. When `update_ready` transitions to true (or on next window activation) and every 15 minutes while the window is
   active and the user has interacted recently, `VersionReadout` plays a ~2.5-second reminder burst: a bouncing `→`
   arrow immediately to the left of the pill plus a synchronized highlight pulse on the pill, with zero horizontal
   layout shift on the pill and zero `AnimationTicks` handles held between bursts.
3. If 15 minutes pass while the window is unfocused or the user is idle (>5 minutes since last click/keystroke), the
   reminder defers and fires when the user next activates the window or interacts in it.
4. When `ticker::reduced_motion(cx)` is true, the periodic animation is skipped completely (no arrow, no pulse, no
   15-minute timer running; an in-progress burst stops cleanly if reduced motion turns on mid-burst) and the tinted red
   pill remains visible.
5. In a narrow macOS sidebar (`296px`) or when inline status is visible beside the toggle, the `→` arrow is suppressed
   instead of overlapping the Farhelm wordmark (while the pill pulse still plays), and `overflow_hidden()` is not added
   in a way that clips the focus ring.
6. Deterministic tests cover the 15-minute interval, arrival trigger, idle/unfocused deferral and wake on
   activation/input, reduced-motion skip and live mid-burst stop, zero held animation handles at rest, and
   narrow-sidebar wordmark clearance; affected GPUI screenshot goldens are updated and published (including a frozen
   burst frame); `SPEC.md` is updated; and the TODO.md entry is removed.

## Decisions already made

**The maintainer's request and decisions (2026-11-22, settled against HTML/CSS mockup):**

- Resting style: **Tinted red pill** (`↑ <version>`, borderless translucent red fill, `5px` corners, Dark & Light).
- Periodic animation: **Bouncing `→` arrow to the left of the readout + gentle highlight pulse on the pill** (~2.5 s, ~3
  bounces, inside the 48 px top bar without covering the session list).
- Cadence & activity rule: **Every 15 minutes while the window is focused and the user has interacted recently, plus
  once on arrival** (deferring when unfocused or idle until next focus/interaction).
- Reduced motion: **Skip the periodic animation** and rely on the resting tinted red pill.
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- `crates/gui/clippy.toml`: no raw colors outside `theme.rs`, no direct timers/clocks (`Instant::elapsed` included)
  outside `ticker.rs`, no `with_animation` outside `components/transition.rs`, no `intercept_keystrokes` outside
  `actions.rs`, no `deferred`/`anchored` outside `layers.rs`.
- `SPEC.md:816-825` (version readout), `SPEC.md:3039-3041` & `SPEC_impl.md:1325-1395` (animation cadence, inactive
  pause, reduced motion), `SPEC.md:3817-3829` (macOS title-band drag/zoom).
- `.agents/validation.md` GPUI goldens workflow (`scripts/check-gui-goldens.py` + `scripts/gui-goldens.py publish` for
  intended visual changes).
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md", and
  "The live install is off-limits".

**Planner proposals (reviewed by fresh-context scope review):**

- Use `FontWeight::BOLD` on `TERMINAL_FONT` (the heaviest bundled JetBrains Mono weight, matching the approved visual
  mockup) rather than unbundled Medium/SemiBold.
- Simplify `Tier::Readout` and add a dedicated `Tier::UpdateReady { pulsing: bool }` in `components/button.rs`.
- Track `last_activity` and one-shot `activity_waiter` on per-window `WindowFocus` in `focus::note_input` rather than
  app-wide `MovedOn`.
- Drive the ~2.5-second burst with `ticker::AnimationTicks` (matching `TrashButton::wiggle` in `trash.rs:776-787`) so
  zero new animation framework surface or `SPEC_impl.md` exceptions are needed.
- Slice into 2 PRs: (1) resting tinted red pill + `Button` tier + updated `version-readout-*` goldens, (2) activity hook
  - 15-minute reminder burst + tests + frozen burst golden + `SPEC.md` + TODO removal.

## Outline

1. `plan/eye-catching-pending-upgrade/01-tinted-pill-readout`:
   - Add translucent hover/pulse red tint token(s) on `DARK` and `LIGHT` in `crates/gui/farhelm-gui/src/theme.rs`
     (reusing `danger_wash` unchanged if 14% fits the resting pill).
   - Simplify `Tier::Readout { focus_fill: bool }` to `Tier::Readout` and add `Tier::UpdateReady { pulsing: bool }`
     (borderless `5px` pill, `py(2.) px(7.)`, excluded from solid-blue expanded look, focus ring `radius + 3px`) in
     `crates/gui/farhelm-gui/src/components/button.rs`.
   - Switch `app-version-toggle` in `crates/gui/farhelm-gui/src/update_readout/view.rs` to `Tier::UpdateReady` with
     `FontWeight::BOLD` monospace text and `palette.danger_strong`.
   - Update and publish the four affected `version-readout-*` GPUI screenshot goldens (`version-readout-update-ready`,
     `version-readout-progress`, `version-readout-outcomes`, `version-readout-menu`) and update any affected unit tests.
   - Add a `kind: changed` changelog fragment under `releasing/changelog.d/`.
   - Title: `style: highlight pending desktop updates as a tinted red pill`.
2. `plan/eye-catching-pending-upgrade/02-periodic-upgrade-reminder`:
   - Add `last_activity: Option<Instant>` and one-shot `activity_waiter` on `WindowFocus` in
     `crates/gui/farhelm-gui/src/focus.rs` (updated in `note_input`, exposing `focus::last_activity` and
     `focus::on_next_activity`).
   - Implement the 15-minute active-and-recently-used reminder scheduler and ~2.5-second `ticker::AnimationTicks` burst
     (bouncing `→` arrow `.absolute()` to the left of the pill with narrow-sidebar/inline-status suppression + pill
     `pulsing` toggle) in `crates/gui/farhelm-gui/src/update_readout/view.rs`, lazily registering
     `observe_window_activation` on first `render`, respecting `ticker::reduced_motion(cx)` live, and dropping handles
     between bursts.
   - Add deterministic unit/GUI tests (interval, arrival, idle/inactive deferral and wake, reduced-motion skip & live
     stop, zero idle handles, `296px` narrow-sidebar clearance) and a frozen-frame burst golden (e.g. a 4th row in
     `version-readout-update-ready`, published via `scripts/gui-goldens.py publish`).
   - Update `SPEC.md:816-825`, expand PR 1's changelog fragment in place (amending unreleased work per `AGENTS.md`), and
     remove the TODO.md entry ("Make a pending upgrade more eye-catching").
   - Title: `feat: remind periodically with a bouncing arrow when an app update is pending`.

## Fallbacks

- **10 Hz bounce smoothness:** Default to `ticker::AnimationTicks` (matching `TrashButton::wiggle`). Only if visual
  review shows 10 Hz horizontal bounce is unacceptably choppy, run scope reassessment and switch the burst motion to a
  bounded one-shot helper in `components/transition.rs` (with monotonic burst ID, 2.5 s retirement `Deadline`, and
  matching `SPEC_impl.md` update).
- **Landing order with `toolkit-reduced-motion.md` and `button-restyle.md`:** If this plan executes before
  `toolkit-reduced-motion.md` lands, live `ticker::set_reduced_motion(true, cx)` tests may need an explicit window
  refresh after toggling until `cx.set_reduce_motion` notifies automatically. If `button-restyle.md` has already landed,
  adapt `Button::colors()` to include its `held` fill slot cleanly for `Tier::UpdateReady`.
- Escalate via `queue block` only if a repository invariant conflicts with the settled behavior above.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-eye-catching-pending-upgrade-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-eye-catching-pending-upgrade-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted `farhelm-gui` tests (`update_readout`, `components::button`, `focus`, `sidebar`, `ticker`),
`python3 scripts/check-gui-goldens.py` (with `--scene 'version-readout-*'` for visual iteration, then a full `--update`

- visual check of the changes page + `scripts/gui-goldens.py publish` before finishing each visual PR, plus a clean
  compare pass), `python3 -B scripts/check-gui-lints.py` if any clippy ban touchpoint is near,
  `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`, `dprint check` on changed Markdown,
  `python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py` as
  `docs/test-sleep-check.md` prepares it. Apply `.agents/test-authoring.md` to every test you add or change. Never take
  a state-changing action against the live Farhelm install on this machine (root `AGENTS.md`, "The live install is
  off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Make a pending upgrade more eye-catching"), as
  root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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

No release note needed.
