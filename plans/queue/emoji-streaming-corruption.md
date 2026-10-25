# Fix: emoji drawn clipped while terminal output streams

Written against main at 36c8d7b0 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Fix emoji corruption while terminal output streams.** Emoji in agent output, such as status icons beside changed,
> unchanged, skipped, and failed counts, sometimes render as clipped or partial glyphs. Scrolling redraws them correctly
> until the agent emits more output, when they can break again. Diagnose the rendering or redraw path and keep the
> glyphs intact as new output arrives.

What planning found, from a read of main at 36c8d7b0 (line numbers approximate; the vendored xterm bundle is one
minified line, so it is cited by symbol):

- **Path.** The agent writes to a pty in a private tmux server. The supervisor's control-mode client forwards the pane's
  raw bytes over the WebSocket into `term.write()` (SPEC_impl.md around 1065-1070, 1566-1570, 1697-1703). tmux does not
  re-render live output; its width model matters only for the reconnect replay (`capture-pane -e`) and the cursor
  position restored after it (SPEC_impl.md around 1626-1632).
- **xterm.js 6.0.0, unmodified, DOM renderer.** Provenance in `crates/farhelm-ui/src/lib.rs` (around 1163-1239, script
  tags around 1752-1757). Loaded addons: fit, clipboard, web-links and Farhelm's file links. No unicode11, no
  unicode-graphemes, no WebGL. The constructor in `crates/farhelm-ui/assets/terminal.js` (around 3486-3569) sets no
  `allowProposedApi`, and nothing in the repository touches `term.unicode`, so xterm uses its default Unicode 6 width
  table: **every emoji is one cell wide**, including ✅ ❌ 🔄 and VS16 sequences such as ⚠️ ✔️ ⏭️.
- **The likely cause is that width mismatch.** Claude Code, Codex and tmux count these emoji as two cells. The DOM
  renderer gives a one-cell emoji a one-cell box with negative letter-spacing inside rows with `overflow: hidden`, so
  its roughly two-cell glyph spills into the next cell, and that cell's character or background covers the right half
  (upstream xterm.js #4813, still open). Agents that redraw cell by cell are also one column off after each emoji and
  leave stray characters where xterm thinks the next cell is. A full repaint puts a space back there, which fits
  "scrolling fixes it until more output arrives".
- The bundled JetBrains Mono Nerd Font has ⚠ but not ✅ ❌ ✔ ⏭; those come from the platform's colour emoji font.
- SPEC_impl.md's terminal widget section (around 1065-1090) keeps the bundle unmodified, and fixes non-security bugs
  inside it by upgrading rather than working around them. It names no Unicode width version yet.

### Acceptance criteria

- The terminal uses xterm.js's grapheme width provider (`@xterm/addon-unicode-graphemes`, version 0.4.0 at planning
  time, the release that ships with xterm 6.0.0; confirm compatibility), vendored unmodified like the other addons.
  `allowProposedApi` is on, and the active Unicode version is the addon's (`'15-graphemes'`) before `open()` and before
  any byte is written, replay prefill included.
- With it, ✅ ❌ 🔄 ⚠️ ✔️ ⏭️ and similar emoji each occupy two cells, and the character after each lands where the agent
  put it.
- One browser spec, run on both engines (`e2e/playwright.config.ts` runs Chromium and WebKit; the desktop app is
  WebKit), gated on no pixels, that fails on main. It writes bytes straight into the live terminal (live output reaches
  it as raw bytes, so going through a fake agent adds nothing) and checks:
  - each emoji's cell width in the buffer is 2, and the next character's column is right;
  - an agent-style in-place redraw stays aligned: a status line with emoji, then cursor-addressed updates that assume
    two-cell emoji, then more output. Both the buffer text and the rendered row text (`.xterm-rows`) match the intended
    line;
  - an emoji followed by a coloured-background cell keeps its full two-cell box (a geometry check, not pixels).
- The reconnect replay stays aligned. The fix can break it: if tmux counts VS16 emoji such as ⚠️ as one cell, replayed
  rows would now be misaligned where they lined up before. First read tmux's `variation-selector-always-wide` on a test
  tmux server (never the live install); the minimum supported tmux, 3.7c, has the option. Add one replay assertion
  showing the replayed emoji row and the cursor line up. If the option is off by default, pin it on in the generated
  tmux config (`crates/farhelm-supervisor/src/tmux.rs` around 2309-2321), noting that servers adopted from before keep
  their old values.
- Existing terminal behavior still holds: run the link, file-link, clipboard, copy-on-select, scroll-freeze and font
  browser tests, which touch wide characters and text extraction.
- The add-on's license notice is added the way the other vendored add-ons' are. Wiring it in `lib.rs` is the whole asset
  job: `scripts/check-desktop-assets.sh` derives its list automatically, and `scripts/desktop-smoke.sh` deliberately
  checks only one representative add-on, so neither script changes.
- SPEC_impl.md's terminal widget section records the width provider and why: agents and tmux count emoji as two cells,
  and xterm's default table counts one.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`).
- The last code PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **Seen in the desktop app on macOS** (WebKit). The maintainer does not recall whether the agent was Claude or Codex,
  or whether it was in a full-screen view; they were scrolling with the touchpad over the terminal area. So the
  mechanism (the agent repainting on scroll, or xterm's own scrollback) is not known; find out what you can while
  reproducing.
- **Use the grapheme width add-on**, despite its experimental label upstream. It is the only option that makes the VS16
  emoji (⚠️ ✔️ ⏭️) two cells wide. The stable Unicode 11 add-on leaves those at one cell.
- **If the exact clipped glyphs cannot be reproduced, ship the width fix anyway**, with the tests above showing the
  misalignment it fixes, and state the gap in the report.
- **No pixel-comparison tests as gates.** CI may lack a colour emoji font. Screenshots may go in the report as evidence.
- **The DOM renderer stays.** The terminal tests read the DOM rows, and SPEC_impl.md chose DOM rendering for
  testability.
- **No workaround patched into the vendored bundle.** If an emoji glyph wider than two cells still spills after the fix
  (upstream #4813), that goes in the report as a follow-up waiting on an xterm release.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md's terminal experience (around 1813-1818, "If it works over plain SSH it must
work here"); SPEC_impl.md's terminal widget section (around 1065-1090), replay and cursor re-synthesis (around
1566-1570, 1626-1632), and the link and copy code's handling of wide glyphs (around 1402, 2275), whose inputs change;
`docs/desktop-web-triage.md`; root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments",
"TODO.md and FLAKES.md"; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Reach the live terminal in tests through the islands registry (`window.__farhelmIslands[id].term`, terminal.js around
  1682) and write with a callback before reading cells. Read rendered rows the way
  `e2e/tests/terminal-scroll-freeze.spec.ts` (around 160-185) does.
- Add the add-on's global to the mount precondition list in terminal.js (around 3220-3235), which waits for the add-on
  scripts to load before the terminal mounts.
- An optional look at a real agent with `scripts/agent-tui.py` (`.agents/real-agent-tui.md`) can show how Claude Code or
  Codex redraw on scroll. It is not required. Submit no prompt unless you need one, since each spends a real turn.

## Outline

One PR, `plan/emoji-streaming-corruption/01-grapheme-widths`: vendor the add-on with its notice and asset wiring, enable
it before `open()`, the browser spec, the replay assertion, the SPEC_impl.md paragraph, the changelog fragment and the
TODO.md removal. Only if tmux's VS16 option turns out to be off does pinning it get a second PR,
`plan/emoji-streaming-corruption/02-tmux-vs16-wide`, below the first, with the replay assertion moving there.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
the crates you touch, `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares,
and, through `python3 scripts/record-test-run.py`, the new browser tests and the terminal regression tests named above
on both engines. If you pin the tmux option, add the supervisor's tmux config tests.

## Fallbacks

Examples of departures that need the scope reassessment: switching renderers, writing a Farhelm-owned width provider,
patching or forking the bundle, upgrading xterm.js itself, or changing how output reaches the terminal.

Decisions to log: the add-on version and how compatibility was confirmed, what reproduction showed (and what it could
not), whether tmux options were pinned, every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-emoji-streaming-corruption-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/emoji-streaming-corruption/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
