# Session view: session details move to a bottom bar that also shows the host

Written against main at 63439c2f on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan has no dependency on another plan. The plan `host-terminal-colors` (queued alongside this one) changes how the
local host's icon is drawn (it gains a choosable color and icon) and also reads the sidebar's hosts snapshot in the
session view; both plans draw the host through the same components (`LocalHostIcon` and `HostMark` in
`crates/farhelm-ui/src/icons.rs`) and share one `Signal<HostsRead>` context provided by `AppBody` (Acceptance criteria),
so use those rather than copying markup or adding a parallel read, and whichever lands second rebases mechanically.

## The goal

Today the open session's chrome is one header row at the top (status, session name, age, the managed-checkout branch
mark, the copyable directory, the copyable command line, then Restart, Restart with, Replace, Clone, Replace with,
Delete), the tab strip under it, and the terminal down to the bottom edge. The host is not shown anywhere in the session
view; only the sidebar row says which host a session runs on. Move the session's identity to a slim bar under the
terminal, add the host to it, and merge the tab strip and the action buttons into one row at the top, so the terminal
keeps about the height it has today while the active session and host become easier to see.

TODO.md entry this plan covers (Near term): "**Consider moving session details to the bottom.** Consider moving the
session name, host and related details from the top of the session view to the bottom. There appears to be room there,
and it could help highlight which session and host are active. Details TBD at planning time."

The agreed design, in words (the maintainer approved it from a mockup you cannot see):

- **Top row, one row:** the tab strip on the left (agent tab, terminal tabs, "+ terminal", and the A−/A+ text-size
  buttons at the right end of the tab area, as today), and the six action buttons on the right (Restart, Restart with,
  Replace, Clone, Replace with, Delete), in today's order and styling. The separate header row above the tabs goes away.
  On macOS this row is the one aligned with the sidebar's app bar beside the window buttons (today's 48px rule for
  `.macos-window .titlebar` moves to it).
- **Bottom bar, under the terminal:** in this order: the status badge, the session name, then the host (its icon,
  tinted, plus the host name, drawn the same way the sidebar draws it and made visually prominent, for example in bolder
  or brighter text than the fields after it), then the age, the managed-checkout branch mark, the directory (shown as
  today, with the home directory as `~`), and the command line. Directory and command keep their click-to-copy
  behaviour, tooltips, accessible descriptions and ellipsizing; they are the fields that give up width first, as today.
  The bar is about the height of one tab-strip row.
- Everything else keeps working as today: the restart, replace and delete confirmations still open anchored under their
  buttons (the buttons stay at the top), and Escape still cancels them (the header's `onkeydown` Escape handler moves to
  the top row with the actions, not to the bottom bar); the delete-in-progress label still takes the actions' place;
  keyboard order stays meaningful (top row, then terminal, then bottom bar).

### Acceptance criteria

- The steady-state session view is: one top row (tabs left, actions right), the terminal, and the bottom bar. There is
  no other header row. The terminal is no shorter than today in the steady state (off macOS it gains about 10px; on
  macOS, where the top row keeps the 48px alignment, it stays the same). Do not pad the layout to match today exactly.
- The bottom bar shows, in order: status badge, session name, host, age, branch mark when the session has a managed
  checkout, directory, command line. The host follows the sidebar row's rules exactly: the host name comes from the
  session's own `host_name` through `gui_host_name` (`crates/farhelm-ui/src/hosts.rs`, so the local host reads "local
  (this machine)"); the icon is `LocalHostIcon` for a local session and `HostMark` with the host's chosen icon and color
  for a remote one, by `session_locality` (`crates/farhelm-ui/src/list/shared.rs`). An `Unknown` locality draws no icon
  but still shows the name when the session carries one ("unknown locality never suppresses an available host label");
  only a session with no `host_name` at all shows no host. Host name and session name are peer text, escaped and
  direction-isolated like every other peer value (`peer.rs`).
- The sidebar row and the bottom bar draw the host through one shared component or helper extracted from the row's
  `session-locality-slot` match plus `gui_host_name`, so "drawn the same way" is enforced rather than copied.
- Getting the host's icon, color and locality into the session view adds no request: the session view does not start
  reading `/api/hosts` (its own `host_read` is deliberately refreshed only while a session is stale; keep that). Instead
  it reads the sidebar's hosts read through a shared Dioxus context. The plan `host-terminal-colors` needs the same
  context and may run first or concurrently, so: if it is already on `main@origin` when you start (a `Signal<HostsRead>`
  context that `AppBody` provides and `ListView` uses as its `hosts`), reuse it as it is; never build on another plan's
  PRs. Otherwise add it in exactly the shape both plans converge on: lift `ListView`'s `hosts` signal
  (`use_signal(HostsRead::default)` in `crates/farhelm-ui/src/list/view.rs`) into `AppBody`
  (`crates/farhelm-ui/src/lib.rs`), which provides it as a plain `Signal<HostsRead>` context with `use_context_provider`
  (no newtype); `ListView` takes it with `use_context::<Signal<HostsRead>>()` in place of its own `use_signal`, and
  `SessionView` reads it the same way. `SessionView` is a sibling of `ListView` under `AppBody`, not its child, which is
  why the signal lives in `AppBody`. No derived copies, no second refresh path. On a rebase conflict with the other
  plan's identical change, keep the one on main and drop yours.
- The copy buttons' "✓ copied" feedback and the hidden-characters `.copy-warning` still work; the warning now opens
  upward from the bottom bar instead of downward from the header, and is never clipped by the window edge.
- All six actions remain fully visible and keyboard reachable from a 650px main pane, with the tab strip taking the
  remaining width and scrolling horizontally (its existing `overflow-x: auto`) when tabs do not fit. Below 650px the row
  may clip as today, never wrap.
- The status badge's existing placement rule (`status_badge_destination` in `crates/farhelm-ui/src/session_view.rs`)
  holds with "header" now meaning the bottom bar: a stale session's badge still moves to the stale notice, and nothing
  shows a status in two places.
- Conditional bands (refused-restart prose, the stale / "helm stopped listing this session" notice, the interrupted
  card, the Claude trust hint band, tab errors) and the desktop app's tunnel strip sit between the top row and the
  terminal, in their current relative order (bands, then tunnel strip).
- On macOS desktop the top row lines up with the sidebar's app bar (48px), as the header does today; browser and Linux
  layouts get the same top-row/bottom-bar layout without the macOS inset.
- The approval region keeps today's placement relative to the terminal: its top follows the top of the terminal area,
  below any conditional bands and the tunnel strip (today it follows the tab strip's bottom, which sits under them).
  `crates/farhelm-ui/assets/approval-layout.js` currently anchors to `.tab-strip`, falling back to `.titlebar`; with the
  tab strip moved into the top row, anchor it to the terminal area (falling back to the top row's bottom when no
  terminal surface is shown), so the region never covers a band or a tunnel chip.
- SPEC.md and SPEC_impl.md describe the new layout (see Requirement sources), the docs pages that describe the header
  describe the new layout, and the browser tests that assert the old header geometry assert the new one.
- The PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-17):**

- Layout A from the mockup: identity moves to a bottom bar, the action buttons stay at the top. In their words: "I want
  to use up as little space as possible on the bottom, which is why I don't want all the buttons moving. It also feels
  like it kind of fits with the tabs at the top, and it also gives us more space to add additional horizontal
  information like the full path, hence the split."
- Show the host "just like the sidebar does, with the icon and everything. But after the session name, add the host
  name, visually prominent, followed by the information we already have in the bar."
- The buttons stay at the top and the tab strip remains at the top, in one row: tabs on the left, actions on the right
  ("This is actually looking really great").
- The directory stays as today, with home shown as `~` (asked separately; they declined a full-path display for now).
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC.md, the session view bullets (the paragraph beginning "The typical session header is one keyboard-reachable row…"
  and the sentence "the titlebar remains the identifier in that state"): rewrite them to describe the top row and the
  bottom bar, keeping every rule they carry that still applies (the 650px full-visibility rule for the actions, the
  greyed-out Restart explanations, copy behaviour, the anchored confirmations, the session-confirmation setting). The
  identifier sentence becomes the bottom bar's session name.
- SPEC.md, tunnels: "A strip under the session header shows one local-port → host-port chip per tunnel" becomes "under
  the top row".
- SPEC.md, "Desktop window chrome": "The session header and terminal tabs continue the app's surface to the top edge"
  stays true in substance; adjust the wording to the merged row.
- SPEC_impl.md, the paragraph beginning "The open session's chrome is ONE header row…": rewrite for the new layout and
  keep its history (the four bands → one row consolidation) plus one sentence on why identity moved to the bottom (the
  user's reason above). The paragraph on the macOS shell class ("aligns the session header's height") follows the row.
  The approval-region paragraph ("the current tab strip's bottom (the titlebar or pane top when there are no tabs)") is
  rewritten to the terminal-area anchor above.
- Root `AGENTS.md`, "Changelog fragments" and "Tests and validation"; `website/AGENTS.md` before editing any page under
  `website/`; `.agents/docs-media.md` before touching anything under `e2e/docs-shots/` or `e2e/readme-hero/`.

**Planner proposals (checked by a fresh-context planning review):**

- Keep the identity markup's existing pieces (`StatusBadgeView`, `.title`, `.status-time`, `ManagedCheckoutMark`,
  `.header-copy` and their tooltip/description ids) and move them into the bottom bar rather than rewriting them; the
  top row reuses `.titlebar-actions` unchanged.
- Names: the top row is the `header` element with a new class (for example `session-topbar`); the bottom bar is a `div`
  with a new class (for example `session-details`). The `titlebar` class goes away rather than staying on the bottom
  bar: a "titlebar" at the bottom would mislead every future reader of the CSS, the specs and SPEC_impl.md, while the
  selector rename is a one-time mechanical change. Rename the CSS descendant rules (`.titlebar .title`,
  `.titlebar .status-badge`, …) and the browser selectors accordingly: `.titlebar .title`, `.titlebar .header-copy` and
  `.titlebar .status-badge` alone occur over a hundred times across 29 specs under `e2e/tests/` plus
  `e2e/tests/helpers/term.ts`. Prefer a small shared selector constant in the helpers for the bottom bar where several
  specs repeat it.
- Docs screenshots and the README hero are not refreshed by this plan: refreshing them has its own maintainer-triggered
  flows (`.agents/docs-media.md`), and nothing in CI checks them. Leave `e2e/docs-shots/` and `e2e/readme-hero/` alone
  even though their specs select `header.titlebar`; the refresh procedure rewrites shot specs itself. This plan updates
  the docs page text that describes the header (`website/src/content/docs/docs/using/work-in-a-session.mdx`, "The
  header", and `stop-restart-resume.mdx`, "The header's actions" and its later mentions), and the report lists as
  follow-ups: the out-of-date `header`, `tabs` and `header-actions` shots and their specs' selectors, and the README
  hero, for the next "refresh the docs screenshots" / "refresh the README screenshot".
- No new setting: the layout simply changes.

## Outline

One PR, `feat: move session details to a bar under the terminal`: the shared `Signal<HostsRead>` context (unless already
on main) and the shared host-label component, the layout change in `crates/farhelm-ui/src/session_view.rs` and
`crates/farhelm-ui/assets/app.css` (top row, bottom bar with the host, upward copy warning, macOS rule moved, Escape
handler on the top row), the approval-region anchor in `crates/farhelm-ui/assets/approval-layout.js`, unit-test updates
for any moved helper, the browser selector and geometry updates, SPEC.md and SPEC_impl.md updates, the two docs pages, a
changelog fragment (`kind: changed`) saying the session's name, host, directory and command now sit in a bar under the
terminal and the tabs and action buttons share the top row, and the removal of the TODO.md entry. If lifting the hosts
signal turns out large enough to review on its own, it may go first as a `refactor:` PR with no behaviour change.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the `farhelm-ui` unit tests for `session_view` through the recorder,
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` (browser tests change),
`python3 releasing/check-changelog.py format`, `dprint check` on changed files, the website build (`website/AGENTS.md`),
and through the recorder on Chromium and WebKit: every spec whose selectors or assertions changed (the selector rename
touches most of `e2e/tests/`, so this is close to the full suite; that is expected for a change to the whole session
view's chrome), with `header.spec.ts`, `chrome.spec.ts`, `window-chrome.spec.ts`, `approval-layout.spec.ts`,
`shell-scroll.spec.ts` and `sidebar.spec.ts` among them. Say in the report which specs ran.

## Fallbacks

- If the 650px rule cannot hold for all six actions with tabs in the same row without hiding tabs entirely, the actions
  win: the tab strip shrinks and scrolls (it already scrolls); log the DECISION with the measured widths.
- A session with no `host_name` (a legacy row) shows no host in the bar; never "unknown host" text.

Examples of departures that need the scope reassessment: a new setting to choose top or bottom; showing the full
directory path; moving the conditional bands into the bottom bar; restyling the action buttons; a second bar; a new
`/api/hosts` read from the session view.

Decisions to log: whether the shared hosts context was reused from main or added here, the bottom bar's class name and
height, how the host is made prominent, the copy warning's anchoring, the approval-region anchor, whether the hosts
context went first as its own PR.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-session-details-bottom-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
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

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/session-details-bottom/<nn>-<short-name>` (for example `plan/session-details-bottom/01-bottom-bar`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
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
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
