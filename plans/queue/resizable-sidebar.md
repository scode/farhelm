# The sidebar can be resized by dragging its edge, remembered per device

Written against main at 363ba6b1 on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan. The `approval-card-layout` plan was written at the same time and also edits
`crates/farhelm-ui/assets/app.css` and the app layout in `crates/farhelm-ui/src/lib.rs` (`AppBody`), in other places; if
it lands first, rebase onto it carefully and check that its card still centers over the main pane at every sidebar
width.

## The goal

The sidebar (the session list column) is a fixed 340px today, by an earlier decision recorded in the CSS ("no drag
handle, no persistence") and in `BUGS_BURNDOWN.md`. Let the user resize it by dragging a handle on its right edge,
between 240px and 600px, and remember the width per device the way terminal text size is remembered.

Acceptance criteria:

- A handle on the sidebar's right edge resizes the sidebar by dragging, live, clamped to 240px–600px (D1). The handle is
  easy to hit (a hit area a few pixels wide, the column-resize cursor) without stealing clicks from the rows' `⋯` menu
  toggles, which end a couple of pixels inside the sidebar's edge, or from the main pane, and a drag never selects text.
  The handle lives at the shell level between the two panes, not inside `.app-sidebar`, which clips and scrolls its
  contents and would cut the handle off or scroll it away. At rest it looks like today's 1px sidebar border, so the docs
  screenshots need no refresh.
- The width is remembered per device: in the browser and in the desktop app, through the same storage terminal text size
  uses (`localStorage`), read once at startup with the same rules as text size: digits only, an out-of-range value
  clamped to the bounds, and 340px when the value is absent or malformed or storage is unavailable (D1). Nothing goes to
  the helm. A fresh profile looks exactly like today.
- Double-clicking the handle resets the width to 340px. The handle is keyboard accessible: focusable, exposed as a
  vertical separator with its current, minimum and maximum values, and the Left and Right arrow keys step the width
  within the bounds (D1). A connected terminal takes focus when it is revealed; make sure that does not pull focus off a
  handle the user is stepping with the keyboard, and write the arrow-key test so it cannot pass or fail on that race.
- Terminals in the main pane re-fit to the new width through their existing `ResizeObserver`, so the program inside sees
  the right columns once the drag ends. No coalescing of terminal resize messages: dragging the window's own edge
  already sends the same stream today, undebounced, and that is accepted behavior.
- Everything that hard-codes the sidebar's 340px follows the variable width: the shell's minimum-width arithmetic and
  its comments, the macOS narrow-window `@media (max-width: 661px)` rule (340 + 320 + 1), the build-skew app bar's
  `width: 341px`, the `@media (max-width: 601px)` rule that hides the row-menu pointer (its cutoff is the sidebar width
  plus about 262px, so at a 600px sidebar it would point the wrong way in windows of roughly 602–862px), row-menu flyout
  placement (it already closes on a sidebar resize), and tests that assume 340px (`e2e/tests/header.spec.ts`'s
  `SIDEBAR_WIDTH`, the fixed-width assertions in `e2e/tests/sidebar.spec.ts`). The main pane keeps its 320px floor and
  the shell keeps scrolling horizontally rather than crushing either pane. Values of 340 that are not the sidebar's
  width (the templates dialog's `min-height: 340px`, for one) stay as they are.
- SPEC.md says the sidebar width is user-adjustable, its bounds, and that it is remembered per device for the same
  reason as text size; the shared-preferences paragraph's "terminal text size, by contrast, is deliberately per device"
  parenthetical names it too. SPEC_impl.md records the storage key, validation and fallback beside the text size
  paragraph and drops "the existing sidebar width" wording that assumes a fixed width. The CSS comment that says FIXED
  by decision is rewritten. The docs website says, in the page that introduces the session list, that you can drag the
  sidebar's edge to resize it and double-click to reset.
- Browser tests on Chromium and WebKit: drag resizes and clamps at both bounds, the width survives a reload,
  double-click resets, arrow keys step it, a terminal re-fits (its program sees the new column count), and a fresh
  profile is 340px. Pure helpers (parsing and clamping the stored value) get `js-tests` coverage if they live in asset
  JS.
- The last code PR removes the TODO.md entry "Resizable sidebar." (Near term).
- A linear stack of draft PRs, each having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, Near term, the maintainer's words):**

"Resizable sidebar. Let the user resize the sidebar by dragging a handle on its edge, with sensible minimum and maximum
widths. Decide whether the width is remembered per device, like the terminal text size, or shared by every client."

**The user's decisions (2026-10-09):**

- D1. Remembered per device, 240px–600px, default 340px, with double-click to reset and arrow keys on a focused handle.
  The maintainer picked this from the options presented, whose reasoning was SPEC.md's own for text size: the width is
  about the screen in front of the user, not a choice every client should share.
- D2. Review gate: a fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** (challenge them through Scope reassessment rather than treating them as requirements):

- Render the handle in Rust (in `AppBody`, with its fixed ARIA attributes), and put the pointer and keyboard handling
  and the storage in a small asset script (beside `terminal.js`, registered like the others in `lib.rs`) that listens
  for input, updates the current value, and sets the width as a CSS custom property on the document root. Reasons: the
  desktop app runs its Rust natively and the DOM in a webview, so `localStorage` is reachable from JS in both builds,
  which is why text size lives in `terminal.js`; the root, unlike `.app-shell`, does not remount behind the preferences
  read; and a CSS variable keeps every width-derived rule in one place. If Rust pointer handlers plus `eval` for storage
  read better, do that instead and log the DECISION.
- The two width-dependent media queries cannot read a CSS variable. Replace them with a class or attribute the same
  script toggles from the window and sidebar widths (or an equivalent you find simpler), matching today's behavior
  exactly at the default width.
- One PR (see Outline).

**What the planner found (at 363ba6b1).**

- Layout: `AppBody` in `crates/farhelm-ui/src/lib.rs` renders `div.app-shell` (class from
  `window_chrome::shell_class()`, plus `macos-window` on the native macOS build) holding `div.app-sidebar` (with
  `ListView`) and `div.app-main`. The sidebar already has an `onresize` that bumps `layout_epoch`, which closes open row
  menus; its comment says the width is fixed.
- CSS in `crates/farhelm-ui/assets/app.css`: the two-pane shell comment ("The sidebar width is FIXED by decision"),
  `.app-shell { display: flex; height: 100%; overflow-x: auto }`, `.app-sidebar { width: 340px; flex-shrink: 0; ... }`,
  `.app-main { flex: 1; min-width: 320px; height: 100% }` ("Below 340+320px the shell scrolls"), the macOS
  `@media (max-width: 661px)` block that pins the app bar in narrow windows,
  `.macos-root:has(.build-skew) .app-bar {
  width: 341px }`, a `@media (max-width: 601px)` rule for the row-menu
  pointer, and comments assuming 340px elsewhere in the file and in `crates/farhelm-ui/src/hosts.rs`. Media queries
  cannot read a CSS variable, so the 661px rule needs another mechanism (a class toggled from the same script, a
  container query, or an equivalent); pick the simplest that keeps today's behavior at the default width.
- Text size, the per-device pattern to follow: `crates/farhelm-ui/assets/terminal.js`
  (`FONT_SIZE_KEY =
  "farhelm.terminal-font-size"`, `storedFontSize()` accepting only digits, clamping, falling back on
  any exception, `stepFontSize` writing inside a `try`). SPEC.md, Terminal experience: "The size is remembered per
  device, deliberately unlike the session list's preference". SPEC_impl.md's text-size paragraph notes desktop
  persistence relies on the webview keeping its local storage, verified on Linux, not on macOS; the same caveat applies
  here and is acceptable.
- Terminal resizing: each terminal in `terminal.js` re-fits on a window `resize` and on its own `ResizeObserver`, and
  `term.onResize` sends `{type: "resize", cols, rows}` over the terminal WebSocket, which the helm forwards to the
  supervisor's tmux `resize-window`. There is no debounce today.
- No splitter or drag-resize exists anywhere in the UI. JS asset files export pure functions for `js-tests` with the
  `typeof module !== "undefined"` pattern (`click-detail.js`, `tooltip.js`).
- The sidebar hides its scrollbar (`scrollbar-width: none`) and clips its contents; the rows' `⋯` toggles end about 2px
  inside its right edge.
- Docs screenshots (`e2e/docs-shots`) crop session-list shots at the default width; with 340px as the default and a
  handle that looks like today's border at rest, they need no refresh.
- `BUGS_BURNDOWN.md` records "Sidebar width: fixed (no drag handle, no persistence, no collapse)." Leave that file
  alone; the CSS comment and the spec are where the current decision lives.

**Binding repository constraints:** root `AGENTS.md`: Finishing work (targeted validation; browser specs on Chromium and
WebKit through the recorder; the JS harness; the test-sleep check when browser tests change), Releases and the changelog
(a fragment for `feat`), Docs website (read `website/AGENTS.md` and `website/EDITORIAL_RULES.md` before editing a docs
page), Docs screenshots, Desktop/web UI bug triage, Testability (no tests that change the process environment), Sharing
the machine, Agent scratch space, The live install is off-limits. SPEC.md's "Upgrade compatibility and client scale".
`.agents/test-authoring.md` for any test change.

## Outline

The PR slicing below is a proposal; reshape it if the code argues for it, without churn.

### PR 1: drag, reset, keyboard, remembered per device

`feat:` with a changelog fragment. One PR: replacing the hard-coded widths only matters once something changes the
width, so a separate refactor PR would land the toggles before anything needs them. Replace every sidebar-width
340/341/601/661 with the one width source and the script-toggled replacement for the media queries; add the handle, the
drag with clamping and live update, writing the width on drag end and on keyboard steps, reading it once at startup,
double-click reset, the separator semantics and arrow keys. SPEC.md, SPEC_impl.md, the CSS comments and the docs page.
Browser tests per the acceptance criteria, updating `header.spec.ts` and `sidebar.spec.ts` to read the actual width
instead of assuming 340, and checking the row-menu pointer and the macOS narrow-window rule at a non-default width where
a spec can reach them. Remove the TODO.md entry.

### Out of scope

Collapsing or hiding the sidebar, a keyboard shortcut for resizing, a phone layout, sharing the width through the helm,
and resizing anything else (the templates dialog's panes, terminal tabs).

### Validation

Per root `AGENTS.md` Finishing work, choose targeted checks per PR: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
`scripts/check-desktop-assets.sh` if a new asset file is added, `cd crates/farhelm-ui/js-tests && node --test`, the
relevant Playwright specs (the new ones, `header.spec.ts`, `sidebar.spec.ts`, `shell-scroll.spec.ts`, and the terminal
tabs spec's resize coverage) on Chromium and WebKit through the recorder after `cargo build` and the `dx` web build, the
test-sleep check, `cd website && bun install --frozen-lockfile && bun run build` when the docs page changes,
`dprint check`, and `python3 releasing/check-changelog.py format`. Look at the result yourself in browser screenshots at
the minimum, default and maximum widths and in a narrow window, and describe what you saw in the report.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-resizable-sidebar-log.md`
in the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before
you start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

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
  `plan/resizable-sidebar/<nn>-<short-name>`.
- Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack deletes. Within this run, if
  a PR needs correcting, restructure it rather than stacking a correction on top; that applies to all of this plan's own
  open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entry this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
that PR's changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review
swarm: a fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the
executing one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (a native desktop state file for the width, a resize
protocol change between helm and supervisor, resize coalescing, a general-purpose splitter component, a collapse
feature; these are examples, not a blacklist), and whenever the same component has needed repeated corrective review
rounds, run a fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions
above, this outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler
alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular where the drag and storage live (asset JS or Rust), the storage key, what
replaced the 601px and 661px media queries, where the handle sits and its hit area, the keyboard step size, how the
terminal focus race was handled, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
