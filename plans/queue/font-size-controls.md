# Terminal text size controls: Cmd/Ctrl+Shift +/− and buttons, remembered per device

Written against main at 47ff0a55 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The user can make the text in every terminal pane larger or smaller with Cmd+Shift +/− (Ctrl+Shift +/− off macOS) or
with A− / A+ buttons at the right end of the session's terminal tab strip. Every open terminal, hidden tabs included,
takes the new size and refits, so the program inside sees the new row and column count. The size is remembered per
device. The rest of the UI is unchanged.

Acceptance criteria:

- Cmd+Shift with `=`/`+` or `-`/`_` on macOS, Ctrl+Shift with the same keys elsewhere, step the terminal font size up or
  down within a bounded range (planner default: 9-28 px in 1 px steps, default 14). They work whether or not focus is in
  a terminal, never reach the terminal program or the browser, and plain Cmd/Ctrl +/− keep the browser's own page zoom.
- A− and A+ buttons at the right end of the terminal tab strip do the same, with titles naming the shortcut and their
  own accessible names.
- Every mounted terminal applies the size and refits; the pty size changes accordingly.
- The size survives a reload and is stored per device (browser storage), not as a helm preference; a missing, invalid or
  unreadable stored value falls back to the default.
- SPEC.md and SPEC_impl.md describe the controls and per-device memory; the TODO.md entry is removed.

## Requirement sources

**The user's request:** "let's plan the following, each in its own plan: ... font size controls ...", for the TODO.md
`Near term` entry, verbatim as of cd8bd112: "**Easy font size changes.** Let the user make the font larger or smaller
with keyboard shortcuts, plus buttons for the same. Which shortcuts, where the buttons go, whether it covers the
terminal, the rest of the UI, or both, and whether the size is remembered, are to be decided when this is picked up."

**The user's decisions (2026-10-02), verbatim where quoted:**

- U1. Scope: "the font todo was meant to be terminal only for now. but if it's not a huge effort to amke a global zoom
  too that would be nice. but i feel like that's more work. let's put that in the maybe later todo bucket. keep this one
  to just terminal." The Maybe later entry for whole-app zoom was added by the planning PR; this plan does not touch it.
- U2. Shortcuts: "yeah lets do cmd/ctrl+shift +/-". Told that on Linux Ctrl+Shift+− currently sends Ctrl+_ (undo in
  readline and emacs) to the terminal and the shortcut would take it over: "use it anyway. if real useres complain we
  can r assess, i've never used that combo for undo in my many years of using linux."
- U3. Buttons: first "top right" of the session header; told the header has no room under its 650 px promise (below):
  "right end of terminal tab strip."
- U4. "yes remembered per device".
- U5. No reset shortcut (the planner's Cmd/Ctrl+Shift+0 was dropped: not requested, and Windows often reserves that
  key).
- U6. Review gate: a fresh-context Opus 5.5 reviewer at high effort. No-workhorse mode (see How to run).

**Binding repository constraints:**

- SPEC.md, SPEC_impl.md, an app.css comment and `e2e/tests/header.spec.ts` promise the session header's actions stay
  fully visible from a 650 px main pane; this plan does not put anything in the header.
- SPEC.md's session list section says per-client storage "is not wanted" for list preferences; the new text must say
  plainly that terminal text size is deliberately remembered per device, so the two do not read as contradictory.
- Desktop asset parity (`scripts/check-desktop-assets.sh`): avoid a new asset file unless it earns its place.
- Browser storage is wrapped in try/catch (it can be unavailable or throw).

**Planner choices (from the planning review):**

- P1. terminal.js (`crates/farhelm-ui/assets/terminal.js`) reads the stored size once (fallback 14, clamped) for the
  constructor's `fontSize` (today a hard-coded 14), and exposes one step function on the existing `window.farhelmTerm`
  bridge that clamps, stores, then for every mounted terminal sets `term.options.fontSize` and calls the refit that
  terminal already stores (setting the option alone does not change the pane size; the late font-family swap shows the
  pattern). Hidden tabs are only made invisible and keep their size, so they refit immediately; no deferred refit is
  needed. tmux needs nothing beyond the existing resize message.
- P2. One capture-phase keydown listener on `window` handles the shortcut (match on `event.code` `Equal`/`Minus` with
  Shift and Meta on macOS or Ctrl elsewhere, using the same platform test the rest of terminal.js uses), with
  preventDefault and stopPropagation. Capture on `window` runs before xterm's own handler, so the terminal's custom key
  handler stays as it is.
- P3. Stateless buttons rendered by Rust in the tab strip (`crates/farhelm-ui/src/session_view.rs`) call the step
  function; no disabled state at the bounds (that would need a JS-to-Rust channel for nothing). Place them at the
  strip's right end without breaking its scrolling.
- P4. Keep the logic inline in terminal.js rather than a new asset module with node tests, unless the platform modifier
  check needs automated coverage that Playwright (Linux-only) cannot give; log the choice.
- P5. Desktop storage: on Linux the webview keeps the page's local storage across restarts (verified by the planning
  review); macOS was not verified. If it does not persist there, the size falls back to 14; that does not justify a
  native state file.

## Implementation outline

One PR, `feat:`. Line numbers drift; find code by name.

- terminal.js: P1, P2.
- session_view.rs and app.css: P3.
- Browser test (Chromium and WebKit through the recorder): press the shortcut and click both buttons; assert the
  terminal's font size changed, the pty size changed (`stty size` inside the pane, as "resize reaches the real terminal"
  in `e2e/tests/terminal.spec.ts` does), a second (hidden) tab got the size too, and the size survives a reload. Browser
  zoom blocking cannot be observed by Playwright; check it by hand if practical and log it.
- SPEC.md (Terminal experience) and SPEC_impl.md (where it is stored and how it applies). Changelog fragment
  `kind: added`. Remove the TODO entry.

### What not to build

No whole-app zoom, no helm preference, no desktop state-file storage, no reset shortcut, no settings screen, no change
to the session header.

## Plan-specific notes

### Validation

Likely relevant: `cargo fmt --all -- --check`, both clippy runs, `cargo check -p farhelm-ui --features desktop`, the new
and neighboring Playwright specs (`terminal.spec.ts` resize, `terminal-tabs.spec.ts` refit tests, `header.spec.ts`,
`terminal-font.spec.ts`) on Chromium and WebKit through the recorder, `python -B scripts/check-test-sleeps.py`, and
`python3 releasing/check-changelog.py format`.

### Decisions to log

The range and step, the storage key, the buttons' exact placement and labels, whether the logic stayed inline (P4), what
you found about desktop storage persistence, and any reviewer finding you declined.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-font-size-controls-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/font-size-controls/<nn>-<short-name>`.
- One commit, bookmark and draft PR per PR in the implementation outline, as a linear stack in the order given. Err on
  the side of bite-sized PRs, but do not create churn: code added in one PR and deleted in a later PR of the same stack
  means the stack should have been shaped differently. Within this run, a PR that needs correcting is restructured
  rather than corrected on top, and that applies to all of this plan's open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits with the types the outline names. A `feat`, `fix`, `perf`,
  `style` or `revert` PR, or one whose type carries `!`, adds its changelog fragment under `releasing/changelog.d/` in
  the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`). Never edit `plans/` in a PR; this plan's state moves only through
  `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.

### Validation

Follow root `AGENTS.md` "Finishing work" for each PR: the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for test execution, with the narrow-tests recipe in `.agents/narrow-tests.md` for
reproductions. When tests or their fixtures change, apply `.agents/test-authoring.md` and run
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`. `dprint check` on changed Markdown. The
plan-specific notes above list the checks this work most likely needs; they are suggestions, not a checklist. Report
checks run, reused (with the covered revision) and skipped (with the reason) in the report.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate one review of its changes. The user demands a
fresh-context agent on Opus 5.5 at high effort, shelled out to the other harness if the executing one cannot reach that
model natively. No review swarm. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially: assume there are defects and hunt for them. Cover general correctness
> (bugs, unhandled cases, broken invariants, wrong behavior against the acceptance criteria quoted below), design
> (whether the shape of the change fits the codebase and the goal, and whether a simpler shape would), and idiomatic
> code for the languages involved. Report findings to the named file, most severe first, each with the file, the quoted
> code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria: this file's goal, the user's request and decisions, and the
planner choices. When the PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Address what the reviewer finds before moving on, and log the DECISION where you decline a
finding. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (new persistent state, a new endpoint, protocol
message or subsystem, a compatibility layer, or anything the "What not to build" list names; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the request, the user decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the ones the plan-specific notes above name, and every reviewer finding you
declined. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. Anything
that needs a decision: push what is consistent, record the concrete tradeoff, continue independent authorized work, and
block per `plans/AGENTS.md` (Executing one plan, step 10) for what depends on it.

## Done criterion

The plan is complete when every PR in the implementation outline exists as an open draft, the stack satisfies the
acceptance criteria, every PR has passed the review gate, and the last code PR has removed the TODO.md entry this plan
covers. Open, not merged: merging happens only after the maintainer has reviewed this plan's report. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
