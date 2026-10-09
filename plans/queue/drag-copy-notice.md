# Show the "drag copied nothing" notice at the pointer

Written against main at adf78aba on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan.

## The goal

When a drag in a terminal copies nothing because the program there handles the mouse itself (Codex's prompt box is the
case the maintainer hit), Farhelm shows a notice saying why. It shows up as a small grey line in the terminal's
top-right corner for a few seconds, once per page, and the maintainer could not see it even while looking for it, while
dragging in Codex's prompt box at the bottom left. Show it just above the mouse pointer where the drag ended, keep it up
for 30 seconds with a dismiss button, and show it on every such drag.

Acceptance criteria:

- The notice appears slightly above the pointer's position at the end of the drag, kept fully inside the window (D1).
- It stays for 30 seconds, or until its × is clicked, or until the next mouse press in that terminal, whichever comes
  first (D2). Once gone, it neither shows nor takes clicks.
- It shows on every drag that copies nothing; the once-per-page limit is gone (D3).
- Clicking × dismisses it without taking keyboard focus from the terminal, without reaching the program in the terminal,
  and without counting as a press in the terminal. The rest of the notice never blocks the pointer.
- × has hover text.
- When it is decided to show, and its text, are unchanged (D4).
- SPEC.md and SPEC_impl.md describe the new placement, lifetime and frequency.
- The JS unit tests and the browser tests cover placement near the pointer, dismissal by × and by the next press, the
  timeout, and showing again on a second drag.
- A changelog fragment describes the change for users; no earlier release note is edited (D5).
- The last code PR removes the TODO.md entry "Fix the "drag copies nothing" notice."
- A linear stack of draft PRs as in Outline, each code PR having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words):** "Fix the "drag copies nothing" notice. PR #1499 added a
terminal notice for drags that copy nothing because the program in the terminal handles the mouse itself, as Codex does.
It does show when drag-selecting in Codex's prompt box, but it is so easy to miss that it looked like it never showed at
all: it appears off in a corner, away from where the user is looking. Show it at the mouse cursor, keep it up for a long
time, and give it a quick dismiss button. It was also seen showing when selecting text where copy and paste do work;
make it show only when a drag really copies nothing, and check that the release note's description of it holds. Discuss
the details with the maintainer when planning this for execution."

When planning, the maintainer added: "it was a greyish box at the top right. super hard to see even when literally
looking for it when you're selecting by dragging, or trying to, at the bottom left (line editing field). i think we need
to show it next to where the mouse is (slightly above)."

**What the planner found (at adf78aba; find code by name, line numbers drift).**

- The decision to show is in `crates/farhelm-ui/assets/copy-on-select.js` (pure functions such as
  `dragMayHaveCopiedNothing`, `dragCopyNoticeText`, `takeNoticeOnce`), wired in `crates/farhelm-ui/assets/terminal.js`
  (`handleTerminalMouseDown` records the press on `term.element`; `handleCopyOnSelectMouseUp` decides and calls
  `showDragCopyNotice` after a 1.5 s grace for an OSC 52 write). A page-wide `shownDragNotices` set limits each text to
  once per page.
- The notice element is a per-terminal `div.drag-copy-notice` with `role="status"`, rendered empty by
  `crates/farhelm-ui/src/session_view.rs` so the live region exists before its text is set (what screen readers announce
  reliably). It is a sibling of the terminal element, not inside xterm's element. In `crates/farhelm-ui/assets/app.css`
  it is `position:absolute; top:0; right:0`, `pointer-events:none`, 12px dim text, shown 6 s then faded over 1 s by a
  CSS animation; a banner offset is applied in JS.
- `showLinkTarget` in `crates/farhelm-ui/assets/terminal-links.js` places a `position:fixed` box near the pointer,
  clamped to the viewport and flipped when there is no room; `placeTooltip` in `assets/tooltip.js` explains why boxes go
  above the pointer. No ancestor of the notice sets `transform`, `contain`, `filter` or `will-change`, so a fixed box
  works from where the element is. The `app.css` z-index ladder lists the notice at 2 within its pane; a fixed box that
  can overlap the sidebar needs a rung below the modal backdrops.
- Tests: `crates/farhelm-ui/js-tests/copy-on-select.test.js` ("The drag that copies nothing", including once-per-page);
  `e2e/tests/mouse-modes.spec.ts` (two drag-copy notice tests selecting `#drag-copy-notice-terminal`).
- SPEC.md's clipboard paragraph says "the terminal shows a brief notice, at most once per page load for each distinct
  text"; SPEC_impl.md's paragraph on the notice names `takeNoticeOnce`.

**The user's decisions (2026-10-08):**

- D1. Show it next to where the mouse is at the end of the drag, slightly above the pointer.
- D2. It stays 30 seconds, or until its × is clicked, or until the next mouse press in that terminal.
- D3. Show it on every drag that copies nothing.
- D4. The false-positive half of the TODO is probably stale or a mistake: the maintainer tried a Codex transcript and
  could not reproduce it, and will record a new TODO entry if it shows up again. Do not change when the notice is
  decided to show.
- D5. Do not touch old release notes; this change's own fragment describes the new behavior.
- D6. Review gate: gpt-6.1-sol high, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** SPEC.md's terminal clipboard paragraph and the hover text rule; SPEC_impl.md's
notice paragraph; root `AGENTS.md` (Finishing work, including the JS harness and browser test rules; Conventional
Commits; Releases and the changelog; Sharing the machine with other agents; Agent scratch space; The live install is
off-limits), `docs/test-sleep-check.md`, and `.agents/test-authoring.md` for test changes.

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

## Outline

### PR 1: the notice at the pointer, dismissible, every time

`fix:` with a changelog fragment (kind `changed`: the notice now appears just above the pointer where the drag ended,
stays up to 30 seconds with a button to dismiss it, and shows on every drag that copies nothing).

- Keep the Rust-rendered per-terminal element (it keeps the screen-reader live region, the pane's lifecycle and the test
  selector); give it a text span and a × button so setting the text does not wipe the button. Reposition it with
  `position: fixed` from the drag's release coordinates, slightly above the pointer, clamped to the viewport, reusing
  the existing placement code where it fits rather than writing a third copy. Remove the banner offset, which no longer
  applies.
- The box stays `pointer-events: none`; only × takes pointer events. × uses `tabindex="-1"` and prevents default on
  `mousedown` so the terminal keeps keyboard focus, has `data-tooltip` hover text, and stays outside xterm's element so
  a press on it reaches neither xterm nor the program.
- Lifetime: 30 seconds (a timer or the existing CSS animation, extended), × click, or the next press in that terminal
  (`handleTerminalMouseDown`). Whichever mechanism, the end state is hidden for the pointer too (for example
  `visibility: hidden`), so an invisible × cannot eat clicks. A new drag that copies nothing while one shows replaces it
  at the new position.
- Remove the once-per-page limit (`takeNoticeOnce`, `shownDragNotices`) and its test; add tests for showing again.
- `app.css`: the notice's new z-index rung, with the ladder comment updated.
- SPEC.md and SPEC_impl.md: the new placement, lifetime and frequency.
- Remove the TODO.md entry.

Out of scope: when the notice is decided to show and its words (D4); copy behavior itself; earlier release notes (D5).

### Validation

Follow root `AGENTS.md` "Finishing work": `cd crates/farhelm-ui/js-tests && node --test`,
`e2e/tests/mouse-modes.spec.ts` on Chromium and WebKit through the recorder after building the web UI as root
`AGENTS.md` describes, and, if Rust changed, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
and `cargo check -p farhelm-ui --features desktop`. `python -B scripts/check-test-sleeps.py` (per
`docs/test-sleep-check.md`). `scripts/check-desktop-assets.sh` only if the set of asset files changes.
`python3 releasing/check-changelog.py format` for the fragment.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-drag-copy-notice-log.md`
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
  `plan/drag-copy-notice/<nn>-<short-name>`.
- The stack follows Outline. Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack
  deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top; that
  applies to all of this plan's own open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entries this plan covers.
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

Before implementing a substantial departure from the outline above (a new toast component shared with other surfaces, a
box created per show and appended to the page body, or changes to when the notice is decided to show; these are
examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions above, this
outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative
was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the placement code reused or adapted, the lifetime mechanism (timer or
animation), and what happens when a new notice replaces one still showing, and every review finding you decided not to
follow. The user will ask for these later.

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
