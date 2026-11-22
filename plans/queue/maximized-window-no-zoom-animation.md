# Open a maximized Mac window already zoomed

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", directly under the bucket). Verbatim:

> **A maximized Mac window grows into place on launch.** When the desktop app was maximized (zoomed) at quit, the next
> launch restores that, but the window first appears at its ordinary size and then visibly animates to full size for
> about a second. GPUI opens a maximized window at its ordinary frame and then asks macOS to zoom it, and that zoom
> animates. Low priority. A fix probably means opening the window at the zoomed frame directly, or zooming without the
> animation, in the vendored GPUI macOS code (with its patch file), while keeping the ordinary frame that Restore
> returns to. Plan: `plans/queue/maximized-window-no-zoom-animation.md`.

### Where it stands (from planning research at main b34ceee9f)

The cause in the entry is confirmed by the code:

- The desktop app turns a saved maximized placement into `WindowBounds::Maximized(<ordinary frame>)` (`bounds_of`,
  `crates/gui/farhelm-gui/src/desktop.rs`, about line 422), and the bootstrap passes it to GPUI as the main window's
  bounds (`window_options_at`, `crates/gui/farhelm-gui/src/bootstrap.rs`, about lines 171-198).
- GPUI's `Window::new` opens the platform window at `window_bounds.get_bounds()`, the ordinary frame
  (`vendor/gpui-pre/src/window.rs`, about lines 1712-1732), and only afterwards, for `WindowBounds::Maximized`, calls
  `platform_window.zoom()` (about lines 1764-1768).
- On macOS, `MacWindow::open` creates the NSWindow at the ordinary content rectangle (`initWithContentRect`,
  `vendor/gpui-pre-macos/src/window.rs`, about line 1088), puts it on screen with `makeKeyAndOrderFront:` or
  `orderFront:` (about lines 1326-1331), then corrects its origin with `setFrameTopLeftPoint:` (about line 1337).
  `MacWindow::zoom` (about lines 1989-2000) spawns AppKit's `zoom:` on the foreground executor, so it runs after `open`
  has returned and the window is already visible, and `zoom:` animates. That is the visible grow.
- The same `zoom()` serves user zooms: the Window menu's Zoom (`crates/gui/farhelm-gui/src/desktop/menu.rs`, about line
  100) and the title band's double-click (`crates/gui/farhelm-gui/src/desktop/title_band.rs`, about line 282) both call
  `window.zoom_window()`. AppKit's own title-bar double-click handling also calls `zoom:` (macOS crate, about lines
  2225-2262). User zooms animating is ordinary macOS behavior and stays.
- AppKit's `zoom:` toggles between the standard (zoomed) frame and the "user state", the frame the window had before it
  zoomed. That user state is what a later zoom returns to: the "Restore" in the entry. A window zoomed from its ordinary
  frame therefore restores to it; a window whose frame is simply set to the zoomed size has no such user state.
- `WindowParams` (`vendor/gpui-pre/src/platform.rs`, about line 2732) already carries macOS-only fields
  (`tabbing_identifier`, `use_display_p3`, constructed under `#[cfg(target_os = "macos")]` in `Window::new`).
- macOS `is_maximized` compares the window's size with the screen's visible frame exactly (about lines 893-905), and
  `window_bounds` never reports `Maximized` (about lines 972-980). The desktop app compensates: a window within two
  points of the visible area counts as zoomed, because a window GPUI zoomed as it opened came out a point short
  (`maximized` and `ZOOM_SLACK`, `desktop.rs`, about lines 393-420; SPEC_impl.md, about lines 7364-7368).
- The frame keeper ignores every report from a window restored maximized until one says it is maximized, and only the
  report the window settles on counts, because macOS reports each step of a zoom animation as an ordinary frame
  (`FrameKeeper::placement` and `observe`, `crates/farhelm-desktop/src/window_state.rs`, about lines 334-381; test
  `a_window_restored_maximized_is_followed_only_once_it_is_maximized`, about line 905; SPEC_impl.md, about lines
  7364-7371; SPEC.md, about lines 3867-3870).
- On Linux the app runs on Wayland only; its `zoom()` calls `set_maximized` on the toplevel
  (`vendor/gpui-pre-linux/src/linux/wayland/window.rs`, about line 1949). GPUI's test platform leaves `zoom()`
  `unimplemented!()` (`vendor/gpui-pre/src/platform/test/window.rs`, about line 476), so no test opens a maximized
  window there.
- The GPUI trees are published crates plus ordered patch layers under `vendor/patches/gpui/<layer>/`; a change goes into
  the tree and its patch file together, and `vendor/README.md`, "GPUI", lists every layer with its reason, the
  application order and the reproduction commands. Every file this change is likely to touch already carries the
  `Modified for Farhelm` notice.

What can be checked where: `vendor/README.md`, "GPUI", says Linux cannot check the macOS tree, and planning confirmed
it. A `cargo check --target aarch64-apple-darwin` from Linux fails before reaching `gpui-pre-macos`: the registry crate
`gpui-pre-apple` compiles its Metal shaders in a build script that only does anything when the build host is macOS, and
its renderer then `include_bytes!`s the missing compiled shader library (and the Apple targets are not installed in the
pinned toolchain). CI has no macOS GUI build (the `uninstall-macos` suite builds no GUI); the release workflow builds
the Mac app only at a tag. So on Linux the executor can compile and test everything except the macOS crate.

### Acceptance criteria

1. On macOS, a launch that restores a maximized window shows it already zoomed from the moment it first appears: no
   ordinary-size frame on screen first and no grow animation.
2. After such a launch the next zoom (title-band double-click, the Window menu's Zoom) returns the window to the saved
   ordinary frame, and the launch itself leaves the saved ordinary frame and maximized state in the frame record
   unchanged (the frame keeper's rules hold as they are).
3. User-initiated zooms on macOS behave exactly as today, animation included.
4. Linux behavior does not change: the Linux build compiles the same code for opening a window, maximized or not, as it
   does today.
5. The change is a new GPUI patch layer: its patch files under `vendor/patches/gpui/<layer>/`, appended to the
   application order, with its row in the layers table, its paragraph saying what and why, and its `git apply` lines in
   "Reproducing the trees" (`vendor/README.md`, "GPUI"); the trees and the patch files agree.
6. Comments and spec text that the fix makes untrue are corrected (for example text saying a restored maximized window
   animates into place); text that stays true (user zooms animate and report each step; the two-point slack) stays.
7. The TODO entry is removed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Make the fix.** Open the window at the zoomed frame directly, or zoom without animating, keeping the ordinary frame
  that Restore returns to. In the maintainer's words: "if we think we know the cause yeah lets just do the fix. maybe as
  you say mine wasn't maximized now and it's just a large window. i can try to repro later but if it wasn't fixed itps
  robably still htere".
- **Verification:** you cannot see the animation on Linux, so verification is by reasoning plus whatever builds you can
  run; the maintainer checks on their Mac after the plan lands.
- **Linux maximize behavior must not regress.**
- **No visual review:** the goldens cannot show a macOS window animation, and nothing the app draws changes.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- `vendor/README.md` (its NOTE and "GPUI"): vendored trees change only together with their patch files; a new layer is
  documented with the others.
- SPEC.md, about lines 3867-3870, and SPEC_impl.md, about lines 7364-7371 (the restore rules this must keep).
- `.agents/validation.md` for the vendored GPUI and terminal crates' selections; root `AGENTS.md`, "Conventional
  Commits", "Changelog fragments", "Tests and validation", and "Instructions by path" (`vendor/` names
  `vendor/README.md`; tests name `.agents/validation.md`).
- Root `AGENTS.md`, "The live install is off-limits": nothing here needs the installed app.

**Planner proposals:**

- One new layer (a name like `zoomed-open`), applied last, after `mixed-font-ellipsis`, with one patch for `gpui-pre`
  and one for `gpui-pre-macos`.
- Mechanism: carry "opens maximized" to the macOS platform as a `#[cfg(target_os = "macos")]` field of `WindowParams`,
  in the style of `tabbing_identifier`; in `MacWindow::open`, when it is set, call `zoom:` once the window has its final
  ordinary frame and before it is ordered on screen, so AppKit records the ordinary frame as the user state; and in
  `Window::new`, skip the later `platform_window.zoom()` for `Maximized` on macOS only (also cfg-gated), so the Linux
  path is untouched. That is the whole change. Planning did not establish whether AppKit still animates `zoom:` on a
  window that is not yet on screen (an animation run while hidden could only show as a slower launch). Do not add
  anything to GPUI's NSWindow subclass to suppress it (such as an `animationResizeTime:` override): that class serves
  every window, and code nobody can compile or run before it lands must not risk every Mac launch for a hypothetical
  delay. If the maintainer's Mac check finds a maximized launch slower than an ordinary one, that override is the ready
  follow-up; name it in the report's follow-ups.
- Pitfalls found in planning: (a) `open` calls `setFrameTopLeftPoint:` with the ordinary origin after ordering the
  window front; done after the zoom it would move the zoomed window. Change the order on the maximized path only: set
  the origin, zoom, then show, and skip the post-show `setFrameTopLeftPoint:` on that path. The ordinary (non-maximized)
  path keeps exactly its current order and calls, since a change there would alter every launch with nothing to check
  it. (b) AppKit sends resize and move notifications synchronously from `zoom:`, and GPUI's handlers lock the window
  state's mutex, so never hold that lock across the `zoom:` call. (c) Do not change `MacWindow::zoom` itself: it serves
  the user's zooms. (d) Keep the frame keeper's fence and the two-point slack: a user zoom still animates and reports
  its steps, and planning could not establish whether the opening zoom's point-short result changes. (e) Only
  `Window::new` constructs `WindowParams`; the unpatched `gpui-pre-windows` and `gpui-pre-web` crates already do not
  build against these trees (`vendor/README.md`, native-key) and are not targets.
- Generate the patches as `vendor/README.md`, "Upgrading", describes (`git diff --no-index --no-prefix a b` run where
  that tree's before and after copies sit, so paths start at the tree's root), and in a scratch copy check that each new
  patch reverse-applies cleanly to the current tree (`git apply --reverse --check --directory=...`); this layer being
  last, no later layer needs reversing first.

## Outline

1. `plan/maximized-window-no-zoom-animation/01-open-zoomed`: the new layer in both trees and its patch files, the
   `vendor/README.md` layer table, paragraph and reproduction lines, the comment and SPEC_impl.md corrections acceptance
   criterion 6 calls for, a changelog fragment, and the TODO entry's removal. `fix:` (a user on a Mac sees the
   difference), so a fragment with `kind: fixed`, whose context paragraph says the fix was reasoned about and built on
   Linux only and not yet seen on a Mac.

## Fallbacks

- Verification on macOS: you cannot compile `gpui-pre-macos` from Linux (see "Where it stands"); do not spend time
  trying to make a cross-target check work, and do not look for a Mac. Keep the macOS change small and close to code the
  file already has (its existing `msg_send!` patterns), and re-read it for compile errors with extra care, since nothing
  will compile it before the maintainer's devbuild or the next release build.
- The report warns, in its "things you should know", that main's macOS build is unverified from the moment this lands
  until a Mac builds it, and that a compile error would break every devbuild and the next tag's release build until
  fixed. It gives the devbuild line for the PR (`.agents/devbuild.md`), to run on the PR before it lands or right after,
  with this check list: quit maximized and relaunch; the window opens already zoomed (filling the screen), with no grow
  (an ordinary-size window with no grow means the zoom did not take, which is a regression, not a pass); the maximized
  launch is as fast as a non-maximized one; double-clicking the title band brings back the ordinary frame; zooming again
  animates as usual.
- If, while working, you conclude the cause is not what "Where it stands" says (the entry's grow has another source), do
  not ship a speculative change: log it, and block with the evidence.
- Keep the change out of Linux. A design that would alter the Linux path (for example asking Wayland for the maximized
  state before the first commit) is out of scope, however tidy; record it as a possible follow-up in the report.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-maximized-window-no-zoom-animation-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-maximized-window-no-zoom-animation-watchdog.json`
beside it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the
agent scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

For this plan that means at least: the vendored gpui-pre selection and, since `vendor/` changes, the vendored terminal
crates' three selections (`.agents/validation.md`); a Linux build and clippy of the crates that build on the patched
core (the GUI and the desktop app); the desktop app's frame keeper tests if you touch `window_state.rs`; and
`dprint check` on the changed Markdown. The macOS host-text selection needs a Mac; report it as skipped with that
reason.

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
  `plan/maximized-window-no-zoom-animation/01-open-zoomed`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("A maximized Mac window grows into place on
  launch"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
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
in the prompt, as root `AGENTS.md` requires. For this plan, also tell the reviewer that the macOS crate cannot be
compiled where the work was done, and ask it to check the macOS code for compile errors and AppKit misuse by reading.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.
