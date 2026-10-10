# The agent request card sits at the top of the main pane, one at a time

Written against main at 363ba6b1 on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan. The `resizable-sidebar` plan was written at the same time and also edits
`crates/farhelm-ui/assets/app.css` and the app layout in `crates/farhelm-ui/src/lib.rs` (`AppBody`), in other places; if
it lands first, rebase onto it carefully and position the card from the sidebar width it introduces; either way the card
must not assume a fixed 340px sidebar (D3).

## The goal

When an agent runs a `farhelm spawn` or `farhelm agent` command that needs the user's approval, the GUI shows a card
offering Allow, Always allow from the requesting host, and Deny. The first time the maintainer saw one, it came up as a
giant vertical box along the very right edge of the screen. Today's card column is a fixed 420px, pinned 12px from the
window's right and bottom edges, and may grow to the full window height. A launch request carries about thirteen
labelled rows; the longest labels ("requested from host", "new session on host") leave values about 230px, so the
session id, the folder and the command wrap into a tall strip, and since only the column scrolls, the buttons can sit
below the fold in a short window.

Move the card to the top of the main pane, centered and wide, lay it out compactly, and show one waiting request at a
time with the rest collapsed to one-line headers. The maintainer chose this from a mockup (suggestion B plus the
one-at-a-time add-on at `https://snippets.scode.org/s/farhelm-approval-card-layout/`; fetch it and look at it before
starting, it is the agreed picture), for the reason that it "makes for good horizontal space while not blocking your
view of the input area nor recent transcript output."

Acceptance criteria:

- The expanded card sits centered horizontally over the main pane (the area right of the sidebar), just below the
  session view's tab strip, about 660px wide and never wider than the main pane minus a small margin (D1, D3). It does
  not cover the sidebar, the session header's actions, or the bottom of the terminal where the prompt is. When no
  session is open the same placement applies to whatever the main pane shows.
- The card is laid out compactly, following the mockup: one header line with an amber "agent request" badge and the
  action in the UI's own words; the requesting host and session as labelled values; the short facts (target host, agent,
  model, effort, permissions, YOLO, title, resume) in a multi-column grid; folder and command at full width; the buttons
  below. For the mockup's launch request the card is roughly 320px tall rather than 470 (D1).
- Exactly one waiting request is expanded at a time. By default it is the oldest; clicking another request's header
  expands that one instead, and it stays expanded until it is answered or expires, after which the oldest remaining one
  expands (D2). Every other waiting request, from any host, shows as a one-line header (the action in the UI's words and
  the requesting host as a labelled value) below the expanded card, so the headers never push the expanded card's
  buttons down; the expanded card shows how many are waiting ("1 of 3 waiting" or similar).
- The region is capped at a fraction of the main pane's height (well short of the full window) and scrolls past it. For
  a request the size of the mockup's, at a common window size, the buttons are visible without scrolling; a request with
  a very long command may need the region scrolled, since command text is never cut (below).
- In a narrow main pane (its floor is 320px, narrower than today's card) the fact grid reflows down to one column and no
  text is cut off.
- Unchanged by this plan (SPEC.md, Agent-spawned sessions, and the comments in `app.css` and `approvals.rs`): every
  value that matters for the decision stays visible on the expanded card without a further click; command and resume
  command text is shown in full and wrapped, never clipped and never in its own scroll box; agent-controlled text is
  shown as labelled data, never woven into a sentence that could pass for Farhelm's wording; the rest of the app stays
  usable; cards stay live and answerable while a dialog is open (`data-modal-exempt`, see D3); the 700ms arm delay keeps
  its current trigger (any change in which requests are listed) and also fires when the expanded request changes; the
  "answer arrived late" notice still shows.
- SPEC.md's "fixed corner of the window" and "several stack" wording, SPEC_impl.md where it describes the cards, the
  docs page `website/src/content/docs/docs/using/approve-agent-requests.md` ("bottom-right corner"), and the comments in
  `app.css` and `approvals.rs` describe the new placement and the one-at-a-time behavior. On the docs page that also
  means the sentence about cards moving and their buttons pausing, not only "bottom-right corner". SPEC_impl.md's
  "Permission prompts for agent actions" section says little about placement; touch it only where it does.
- Browser tests cover the placement (centered over the main pane, below the tab strip, clear of the sidebar), the
  one-at-a-time behavior with several waiting requests from more than one host, that the buttons of a mockup-sized
  request are visible without scrolling, and that a card can be answered while the quick switcher (a dialog) is open, on
  Chromium and WebKit. The several-requests tests may fake the helm's approvals listing (`GET /api/approvals`) the way
  other specs fake helm replies; they do not need a real two-host agent setup.
- The last code PR removes the TODO.md entry "Improve the agent action approval card." (Near term).
- A linear stack of draft PRs, each having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, Near term, the maintainer's words):**

"Improve the agent action approval card. When an agent asks to do something that needs the user's approval, the card
that asks (SPEC.md: shown in a fixed corner of the window, offering Allow, Always allow and Deny) came up as a giant
vertical box along the very right edge of the screen the first time it was seen. Improve how it looks and where it sits;
details to be decided with the maintainer."

**The user's decisions (2026-10-09):**

- D1. Placement and shape: suggestion B of the mockup, a compact card centered at the top of the main pane. The
  maintainer's words: "top-center with the add-on. it makes for good horizontal space while not blocking your view of
  the input area nor recent transcript output". The mockup's exact pixel values are a sketch, not a spec; its layout
  (header line, labelled requester values, fact grid, full-width folder and command, buttons below) is the agreed shape.
- D2. The one-at-a-time add-on: only one card expanded, the others as one-line headers that switch to their card when
  clicked, with a waiting count. The mockup showed the oldest expanded; keep that as the default, since the oldest
  expires first. The mockup drew the headers above the card because it showed the add-on with the corner placement; at
  the top of the main pane they go below it (a planner reading of the mockup, not the maintainer's words).
- D4. Review gate: a fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** (challenge them through Scope reassessment rather than treating them as requirements):

- D3. Keep the card region where dialogs cannot disable it, and position it over the main pane from the sidebar width.
  `crates/farhelm-ui/src/modal_isolation.rs` leaves a `data-modal-exempt` element live only while it is a sibling of the
  open dialog's ancestor chain; most dialogs (quick switcher, templates, host settings, settings, feedback) live in the
  sidebar, so a region placed inside `.app-main` would be made `inert` with the rest of the main pane, and the agent
  would wait on a card the user cannot click. Keep today's mount point (or make the region a direct child of
  `.app-shell`, which also scrolls with the shell in a narrow window) and position it with the sidebar width; if the
  `resizable-sidebar` plan has landed, use the width variable it introduces. Log the choice as a DECISION.
- The PR slicing in Outline, and keeping the existing data model (`ApprovalAction`, `action_rows` and the `CardRow`
  kinds) and only regrouping rows for display.

**What the planner found (at 363ba6b1).**

- The component is `crates/farhelm-ui/src/approvals.rs`: `ApprovalCards` (mounted once in `crates/farhelm-ui/src/lib.rs`
  beside `feed::FleetFeed`) renders `div.approval-cards` with `role="region"` and `data-modal-exempt`, the late-answer
  notice, and one `ApprovalCard` (`section.approval-card`, `data-approval-id`, `data-approval-kind`) per waiting
  request. Rows come from `action_rows`, `launch_rows`, `template_rows` and `session_rows` as
  `CardRow::{Value, Block, Note, Identity}`; `ARM_DELAY_MS` is the button pause. Rust unit tests at the bottom of the
  file cover row content only.
- The CSS is in `crates/farhelm-ui/assets/app.css` (search `.approval-cards`); z-index 45 is recorded in the file's
  z-index registry. The column is
  `position: fixed; right: 12px; bottom: 12px; width: min(420px, calc(100vw - 24px)); max-height: calc(100vh - 24px); overflow-y: auto`.
  The rows are a `max-content 1fr` grid.
- `crates/farhelm-ui/src/modal_isolation.rs`: the dialog inerts every sibling of its ancestor chain except a
  `data-modal-exempt` element that is itself such a sibling; "one nested deeper inside an element this inerts is inerted
  with it". No browser test covers answering a card over a dialog today.
- `crates/farhelm-ui/assets/terminal.js` (`keyboardHeldElsewhere`) checks `.closest(".approval-cards")` so a terminal
  does not take focus from a card; keep that class on the region or update the check.
- Playwright: `e2e/tests/helpers/approvals.ts` (`answerCard` expects one `.approval-card` and clicks
  `.approval-<answer>`) is used by `e2e/tests/spawn.spec.ts`; `agent-relay.spec.ts` and `terminal-multihost.spec.ts`
  touch the related host setting. No test checks position or size. No docs screenshot shows the card.
- SPEC.md, "Agent-spawned sessions": "While a request waits, the GUI shows a card for it in a fixed corner of the
  window. The card stays until it is answered or expires, several stack, and the rest of the app stays usable."
  SPEC_impl.md's "Permission prompts for agent actions" section and its modal-isolation paragraph (`data-modal-exempt`)
  describe the cards; at most four per host can wait (`AGENT_ANSWER_SLOTS`).
- The macOS desktop build pins its app bar in narrow windows and has a build-skew banner; check the card's placement
  against both (`window_chrome.rs`, the `.macos-window` rules in `app.css`).

**Binding repository constraints:** root `AGENTS.md`: Finishing work (targeted validation; browser specs on Chromium and
WebKit through the recorder; the test-sleep check when browser tests change), Releases and the changelog (a fragment for
`feat`/`fix`/`style`), Docs website (read `website/AGENTS.md` and `website/EDITORIAL_RULES.md` before editing a docs
page), Desktop/web UI bug triage, Sharing the machine, Agent scratch space, The live install is off-limits. SPEC.md's
"Upgrade compatibility and client scale". `.agents/test-authoring.md` for any test change.

## Outline

The PR slicing below is a proposal; reshape it if the code argues for it, without churn.

### PR 1: compact card at the top of the main pane

One PR for the whole change is fine and avoids writing SPEC.md, the docs page and the changelog twice; split into the
two below only if the diff reads better that way.

`feat:` (or `fix:` if you judge the change reads as a fix to users), with a changelog fragment. Move the region to the
top of the main pane, centered, below the tab strip (D3), and rework the card's internal layout to the mockup's shape.
Regroup rows for display only: the requester rows, a fact grid for the short single-line values, and full-width rows for
the folder, the command, the resume command and the template text. Keep `role`, `aria` labelling, data attributes and
button classes the tests and `terminal.js` rely on. Update the spec, SPEC_impl.md, the docs page and the code comments
for the placement. Browser tests for placement, for buttons visible without scrolling, and for answering over the quick
switcher; adjust `answerCard` only as far as the new markup needs.

### PR 2: one request expanded at a time

`feat:` with a changelog fragment. Show one waiting request expanded (the oldest, unless the user picked another) and
the rest as one-line headers below it that expand their request on click, with a waiting count; answering or expiry
brings up the oldest remaining one. The arm delay applies when the expanded request changes for any reason. Keep
keyboard access: a header is a button. Update SPEC.md's "several stack", SPEC_impl.md and the docs page. Browser tests
with several waiting requests from two hosts: only one expanded, switching works and re-arms the delay, answering
advances. Remove the TODO.md entry.

### Out of scope

What a card shows (no rows added or removed, no change to what counts as agent-controlled text), the approval protocol,
limits and timeouts, and the helm. The harnesses' own tool-permission prompts, which are not these cards.

### Validation

Per root `AGENTS.md` Finishing work, choose targeted checks per PR: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the `farhelm-ui` unit tests
through the recorder, `cd crates/farhelm-ui/js-tests && node --test` if asset JS changes, the relevant Playwright specs
(the new ones and `spawn.spec.ts`) on Chromium and WebKit through the recorder after `cargo build` and the `dx` web
build, the test-sleep check, `cd website && bun install --frozen-lockfile && bun run build` when the docs page changes,
`dprint check`, and `python3 releasing/check-changelog.py format`. Look at the result yourself in a browser screenshot
against a staged request (the spawn spec's fixtures can produce one) at a common window size and at a narrow one, and
compare it with the mockup; describe what you saw in the report.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-approval-card-layout-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/approval-card-layout/<nn>-<short-name>`.
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

Before implementing a substantial departure from the outline above (a new card data model, a draggable or
user-positionable card, per-host grouping of waiting requests, persisting which request is expanded; these are examples,
not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context
review through galaxy-brain with this charter, supplying the user's request and decisions above, this outline, the
current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled
out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how the card is anchored to the main pane, the final width and height caps, which
rows go into the fact grid for each action kind, how the waiting count and headers read, how the arm delay is triggered
on a switch, the final PR slicing, and every review finding you decided not to follow. The user will ask for these
later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee (in particular anything
that hides decision-relevant text or puts command text in a scroll box), or an omitted required behavior needs an agreed
fallback or the user's decision; a review finding or a log entry is not authorization. If the work needs such a
decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10). Because the PRs
form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the question, and close
the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
