# Sidebar: one useful summary card when hovering a session row

Written against main at d526038e on 2026-10-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Hovering a session in the sidebar today tells the user almost nothing: the title shows the title again, the host name
shows the host name again, and in compact mode the space around the title shows "{directory} — click to open" while the
title next to it shows only the title. Replace that with one styled summary card, the same wherever the pointer lands on
the row's title or its open area, in both sidebar densities, while every mark that has its own meaning keeps its own
hover text.

TODO.md entry this plan covers (Near term): "**Make the sidebar's session hover text useful.** Hovering a session in the
sidebar tells the user almost nothing the row does not already show: hovering the name shows the same name again. The
row also disagrees with itself depending on where the pointer lands. In compact mode, the space between the row's parts
shows the session's directory and "click to open", while the name right next to it shows just the name. Make the hover
text carry information worth hovering for, and make the row's parts agree. What it should show is TBD."

### Acceptance criteria

- Hovering a session row's title, or any part of its open button that is not a mark with its own hover text, shows the
  summary card after the tooltip's usual 300 ms rest delay. The card is identical for every such spot and identical in
  compact and normal modes. Keyboard focus reaching the open button (`:focus-visible`) shows the same card.
- The card has three parts, about 310 px wide:
  - Header (padding about 9px 12px 8px, bottom rule `--border-dim`): the session's full title in 12px/700 `--fg-0`,
    escaped and direction-isolated, wrapping, clamped to eight lines as today's tooltip is; the status at the right in
    its status color with the row's own dot shape for live statuses (the waiting triangle is the "▲" in the approved
    mockup), worded as the row's status word for live statuses (so "idle — new output" stays whole) and shortened to the
    state word ("exited", "error", "interrupted") only for ended ones; under the title, the agent summary in 11px
    `--fg-1`, the same runs the row menu's header shows, joined by " · " (agent · permission · model/effort, or a
    command's program name), without the state word, since the status already sits at the right.
  - Body: a label/value grid in the host settings style (label column about 6 rem in `--fg-1`, values `--fg-0`, paths in
    `--fg-dim` and wrapping anywhere). Rows, each omitted when there is nothing true to say: **host** (the host's own
    chosen icon and color, then its name, worded as the row words it); **checkout** (for a managed checkout: repository
    · checkout folder name); **directory** (the session's full, untouched working directory); **started** (absolute
    time); **last active** (relative age and absolute time).
  - Footer (top rule `--border-dim`, 11px `--fg-2`): "click to open" on the left, "⋯ for actions" on the right. "⋯" is
    the row's actions toggle (`.session-row-menu`, whose visible text is "⋯"); keep the footer worded to match that
    control.
- A stale session's card shows its status dimmed and not pulsing, exactly as the stale row does: a card must not show a
  live, pulsing status for a session whose host cannot be reached (SPEC.md's no-guessing rule; app.css calls that
  "fabricated liveness"). The card's root carries a stale class when `session.stale` (for example
  `.session-summary.stale`), added as a third alternative to the two existing stale status rules
  (`.session-row.stale .status-badge.running .status-dot, .stale-metadata ...` for the pulse, and
  `.session-row.stale .status-badge .status-dot, .stale-metadata ...` for the color, around app.css line 4318). Do not
  reuse `.stale-metadata` itself: it carries the session view's stale band layout and background. A test covers it.
- The card is the tooltip itself in a second shape, not a second popup: the same body-level `.farhelm-tooltip` element,
  the same timers, follow-on window, dismissals (leaving, any press, Escape, blur, scroll of a container holding the
  target, the target leaving the document, touch), the same placement rule (above with a 6 px gap, below with a 28 px
  gap only without room above, clamped inside the window), `pointer-events: none`, never focusable, `aria-hidden`.
- The marks keep their own hover text unchanged: the status dot (and its mark read/unread wording), the compact ended
  status icon, the pin, the locality mark, the agent and permission marks, the bell, the activity age, the managed
  checkout mark, the compact stale mark, and the noncompact ended-status and stale words.
- No part of the row shows a redundant or contradicting hover: the title no longer repeats itself, the compact open
  button no longer shows "{directory} — click to open", and the noncompact host name and directory text no longer repeat
  themselves (their full values are in the card).
- Screen readers get at least what they get today: the open button's accessible name keeps its current content
  (including compact mode's visually hidden directory copy), and the card's hidden source is out of the accessibility
  tree. No native `title` attribute appears anywhere.
- Peer text (title, host name, paths, repository, agent values) reaches the card only as DOM text produced by Dioxus
  through `display_peer`, never as markup the tooltip script parses.
- SPEC.md (Session list) describes the card in product terms; SPEC_impl.md's sidebar paragraph and tooltip paragraph
  describe the mechanism; the docs page `website/src/content/docs/docs/using/session-list.mdx` gains one sentence about
  it. A changelog fragment covers the change. The last code PR removes the TODO entry.

## Requirement sources

**The user's decisions (2026-10-18):**

- One summary, the same wherever the pointer lands on the title or the open area, in both densities. Content: the full
  title; the host name with that host's own icon ("I'm interpreting this mockup as the icon there being whatever the
  host icon the user is using for that particular host"); the full directory, or for a managed checkout the repository
  and the session's checkout folder plus the directory; agent · permission · model · effort; when it started; when it
  was last active; what a click does.
- Icons with their own meaning keep their own hover: the status dot, the agent and permission marks, the bell, the
  activity age, and the row's other marks.
- The tooltip can only show plain text, so the styled summary is a small hover card reusing the existing floating
  surface pattern (the host update popup's `--bg-2` surface, 1px `--border`, `--radius-md`, `--shadow-menu`), not a new
  kind of surface. The tooltip's base rule already wears exactly that surface; its CSS comment and SPEC_impl.md describe
  it as the row menu panels' look (`.session-row-menu-panel`), which is the same set of tokens. The maintainer asked to
  "spruce the dialogue up and make it more styled and visually pleasing, consistent with our general pattern in this
  project" and picked design A, whose layout is the header/body/footer in the acceptance criteria above (header title
  12px/700, status right in its color, summary line 11px joined by " · " reusing `menu_header_summary_parts`;
  host-settings label/value body; footer "click to open" / "⋯ for actions"; about 310 px; placed like the tooltip).
- Accessibility and behavior at least as good as today's tooltip: same delay on hover and keyboard focus; hides on
  leave, blur, Escape and scroll; never steals focus or pointer events; screen readers keep what the row's accessible
  name provides.
- The row's parts must agree: no contradicting or redundant hover.
- Keep the mechanism small: extend the existing tooltip machinery so the card is one more kind of hover content rather
  than a parallel tooltip system.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:**

- SPEC.md, "Session list": full untouched values always available on the row (a tooltip on web and desktop); hover text
  on every clickable control and meaningful icon; hover text about 300 ms after rest or keyboard focus, above the
  control.
- SPEC_impl.md's sidebar row paragraphs (around "The sidebar uses an identity line") and its tooltip paragraph (around
  "Hover help is Farhelm's own tooltip"): no native `title`; one delegated listener set and one body-level element; peer
  text through `display_peer`; the element `aria-hidden`; "plain timestamps ... are hover-only"; two hover displays (the
  host update-progress popup, the terminal link display) keep their own shape so two popups never stack.
- Unknown locality draws no host mark (SPEC.md: a session whose host cannot yet be placed marks neither); the card
  follows the same rule.
- Root `AGENTS.md`: "Changelog fragments", "Tests and validation" (including `.agents/test-authoring.md` for test
  changes), "Instructions by path" (`website/AGENTS.md` before editing the docs page, `.agents/validation.md` before
  touching `e2e/`). `docs/harness-marks.md` applies only if you edit `crates/farhelm-ui/src/icons.rs`; this plan does
  not need to.

**Planner proposals (implementation choices, each with its reason):**

- **The card's markup is rendered by Rust as a hidden source; the tooltip clones it.** Each `SessionRow` renders, as
  another root node beside its `.session-row` div (a sibling, not a child), a
  `div { id: "session-summary-{session.id}", class: "session-summary-source", hidden: true, ... }` holding the card's
  header, body and footer. `SessionRow` already returns more than one root in `ListView`'s keyed loop (the
  `div.session-pin-separator` it renders when `pin_separator` is set, row.rs around line 2572), so this is an existing
  shape; whether the source goes before or after that separator does not matter, because the lookup is by id. The open
  button carries `data-tooltip-card="session-summary-{session.id}"`. Reason: the host mark
  (`SessionHostIcon`/`HostMark`), the status classes and `PeerTitle`/`PeerLine` isolation already exist as Rust
  components; rendering them in JS would duplicate them, and rendering the card only while hovered would need a second,
  Rust-side hover timing system. A sibling rather than a child because row-scoped browser locators
  (`row.locator(".host-kind-icon[...]")`, `.status-badge...`) would otherwise match a second, hidden element and break
  on Playwright's strict mode; the source must also never carry `data-session-id`, which the suite's `row()` helper
  matches on. `hidden` keeps it out of layout and the accessibility tree. The fleet is tens of rows (SPEC.md's scale),
  so one extra hidden subtree per row is cheap, and it re-renders only when the row itself does.
- **`assets/tooltip.js` gains one content kind.** `tooltipTarget` accepts an element with a non-empty `data-tooltip` or
  a non-empty `data-tooltip-card`. When the target has `data-tooltip-card` and `document.getElementById` finds that
  source, `position` fills the existing tooltip element with deep clones of the source's child nodes (never the source
  root, so no `id` is duplicated, and never by parsing a string) and sets `data-kind="card"` on it; otherwise it falls
  back to the `data-tooltip` text, and with neither it hides. `watch` re-fills when the source's serialization (read
  with `innerHTML`, never written) differs from what it last cloned, the way it already follows changed text, and
  re-places on movement as today. Everything else (timers, dismissal, focus, touch, placement) is shared unchanged.
  Reason: the decisions ask for exactly the tooltip's behavior, and this is the smallest change that gives it.
- **CSS:** a `.farhelm-tooltip[data-kind="card"]` variant (block display, width `min(310px, calc(100vw - 8px))`, no
  padding of its own, no line clamp; the base rule already wears the menu panels' surface, the same tokens as the host
  update popup) plus `.session-summary-*` rules for the header, status, summary, body grid and footer. Status color and
  dot shape reuse the existing status rules (give the card's status element the badge's status classes, or extend those
  selectors), not restated colors. Reuse the status classes, not the `StatusBadgeView` component: it wires the dot's
  mark-read click handler and puts its own `data-tooltip` on the badge and the dot. `.session-summary-source` needs no
  rule beyond what `hidden` does.
- **The title and the row's text parts defer to the card.** The row's `PeerTitle` passes `tooltip: false`; the open
  button loses its `data-tooltip` text (compact "{cwd} — click to open" and noncompact "click to open this session");
  the noncompact host name (`SessionHostName`, which the session details bar also uses and keeps its own hover there)
  and directory (`.session-cwd`) drop their own hover text in the row. The marks listed in the acceptance criteria keep
  theirs. Reason: the decisions make the card the one summary for the title and open area, and these text parts' own
  hovers only repeated what they show (the directory's full path is in the card). If every caller of `PeerTitle` then
  passes `tooltip: false`, remove the prop in the same PR.
- **Card content sources:** title via `PeerTitle` with a `session-summary-title` class; status from `status_badge`: its
  `text` for live statuses, and for ended ones only the first word of its `class` (the same reduction `SessionRow`
  computes into its local `menu_state` for the menu header, which the new module recomputes or receives), since an ended
  session's full detail (exit code, annotation, error detail) stays on its row line and compact icon; summary via
  `menu_header_summary_parts(&session, None)` rendered with `PeerLine` (that function is a private `fn` in `row.rs`
  today, so make it `pub(super)` or move it next to the card); host via `SessionHostIcon` and the same `gui_host_name`
  wording `SessionHostName` uses, omitted when there is neither a known locality nor a host name; checkout from
  `session.working_copy` (`working_copy.repo.identifier()`, defined on the repository in
  `crates/farhelm-ui/src/github_checkout.rs`, and the last segment of the working copy's `canonical_path` field);
  directory from `session.cwd` through `display_peer`; started from `session.created_at` (omitted when not positive, an
  older helm); last active from the row's existing `ActivityStamp` (omitted when `None`). Absolute times use the
  existing UTC format: split `activity.rs`'s `absolute_stamp` into a prefix-free UTC formatter that both the "last
  activity …" text and the card use. Started is absolute only: a relative age there would need the 30-second clock and
  would re-render every row each tick, which `ActivityStamp`'s design exists to avoid.
- **The card component lives in a new module** (for example `crates/farhelm-ui/src/list/summary_card.rs`) with its own
  unit tests, because `row.rs` is already about 3,800 lines; `row.rs` only renders it and wires the attribute.
- **No `aria-describedby` to the card.** The open button's accessible name already carries title, status, locality,
  agent description, age, host and directory; pointing a description at the card would read most of it twice, and the
  one new fact (start time) is a plain timestamp, which SPEC_impl.md already keeps hover-only.
- **Naming:** call it the "summary card" in SPEC.md, SPEC_impl.md, the docs page and code (`.session-summary-*`
  classes). SPEC_impl.md already uses "session card" for the unreachable-host card in the session view.
- **The coverage sweep** (`e2e/tests/tooltip-coverage.spec.ts`) counts a control as covered when it carries a non-empty
  `data-tooltip` or a `data-tooltip-card` naming an element in the document.

## Outline

1. `refactor` PR, `plan/session-hover-text/01-tooltip-card`: the tooltip's card kind in `assets/tooltip.js` (module docs
   updated: a target's content is the card when its source exists, else its text), the card variant's base CSS,
   SPEC_impl.md's tooltip paragraph, and a browser test in `e2e/tests/tooltip.spec.ts` that injects a target and a
   hidden source with `page.evaluate` (scripts may create tooltip targets) and checks only what is new to the card kind:
   the card appears holding copies of the source's content with no duplicated `id`, follows a change to the source while
   showing, hides when the source is missing or removed, and the element keeps `aria-hidden`. Delay, focus, Escape and
   scroll are the same code for both kinds and are already covered for text by that spec. If a pure helper is added to
   `tooltip.js`, test it in `crates/farhelm-ui/js-tests/tooltip.test.js`. No user-visible change on its own, so no
   changelog fragment.
2. `feat` PR, `plan/session-hover-text/02-session-summary-card`: the card component and its hidden source per row, the
   row wiring and hover removals above, the UTC formatter split, the `.session-summary-*` CSS (including the stale class
   on the two stale status rules), the coverage sweep's acceptance of `data-tooltip-card` (its first consumer), SPEC.md
   (Session list: one or two sentences on the card and that marks keep their own hover) and SPEC_impl.md's sidebar
   paragraph ("the untouched string rides along in the row's tooltip" becomes the summary card; the agent mark still
   carries the full invocation), updated row doc comments that describe the old tooltips (the "Host and staleness"
   section, the compact comments near the open button and the ended icon), the docs sentence on the session list page
   (read `website/AGENTS.md` first; this run is unattended, so start no preview server and name the page in the report
   instead), a changelog fragment (`kind: changed`), and the TODO.md entry's removal. Tests:
   - Rust unit tests for the card: each body row present or omitted per the rules above (managed vs plain directory,
     legacy row without host name, unknown locality, no `created_at`, no activity); live status keeps its full word and
     ended status its state word; the stale class present exactly when the session is stale; peer values escaped and
     isolated; the open button's `data-tooltip-card` names the source id; the title carries no `data-tooltip`. Update
     the row test that today requires a tooltip equal to the escaped title (`titles_and_folders_render_escaped_...`) to
     require the escaped title in the card and still no raw control character in any tooltip.
   - Browser tests on Chromium and WebKit: hovering the title and an empty part of the open button shows the same card
     in normal and compact modes (compare the tooltip's markup), with the host's own icon glyph and color, the full
     directory, a managed checkout's repository and folder (stub the listing as `tooltip.spec.ts`'s `loadRow` does), the
     agent summary, both times and the footer; keyboard focus on the open button shows it; a stale running session's
     card dot is dimmed and not animated; the status dot, agent mark and age still show their own text. A check that
     counts `[data-tooltip]` across the page while a card shows must skip the tooltip element's descendants: the copied
     host mark keeps its own (inert) `data-tooltip`. Rewrite the existing assertions on the old row hovers rather than
     deleting their intent: `sidebar.spec.ts` around the compact toggle test (`.session-row-open` "— click to open"),
     the width-contention test (title and `.session-host` tooltips; keep its escaping proof by checking the card's
     text), and the tilde-fold test (`.session-cwd` tooltip; check the card's directory instead).

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the `farhelm-ui` unit tests for the row, the card and `activity` through
the recorder, `cd crates/farhelm-ui/js-tests && node --test` when `tooltip.js`'s exported functions change,
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, `python3 releasing/check-changelog.py format`,
`dprint check`, the website build for the docs edit, and on Chromium and WebKit through the recorder: all of
`tooltip.spec.ts`, the sidebar sweeps of `tooltip-coverage.spec.ts`, the `sidebar.spec.ts` tests named above, and the
new card tests. Not the full suite. Before the PR's review gate, look at the card yourself in both themes through
Playwright screenshots kept in your scratch directory (never committed), and check it against the layout in the
acceptance criteria. The report says the desktop app itself was not exercised (WebKit stands in for its engine).

## Fallbacks

- Never put the source inside `.session-row` or the open button: inside the button it is invalid content, and inside the
  row it doubles every row-scoped locator's match.
- If cloning the source into the tooltip turns out unworkable in WebKit, stop and run the scope reassessment before
  choosing anything else.
- If the noncompact host name or directory turns out to need its own hover after all (for example a test or SPEC
  sentence you find that requires it), keep that one hover, say why in the report, and keep the card unchanged.

Examples of departures that need the scope reassessment: a Rust-side hover state or pointer handlers driving the card,
building the card's markup in JavaScript from data attributes, a second tooltip element, an `aria-describedby` to the
card, edits to `icons.rs`.

Decisions to log: where the source lives, how `watch` detects a changed card, which row parts keep their own hover, the
footer wording, the SPEC.md wording.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-session-hover-text-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/session-hover-text/<nn>-<short-name>` (for example `plan/session-hover-text/01-tooltip-card`).
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
