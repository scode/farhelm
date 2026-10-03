# A hint when a drag cannot copy because the program handles the mouse itself

Written against main at cd8bd112 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

When a program in a terminal pane has turned on mouse reporting (Codex since 0.157, vim with `mouse=a`, htop), a plain
drag goes to that program, which draws its own highlight; Farhelm's terminal makes no selection and has nothing to copy.
Unless the program copies by itself (an OSC 52 write, which Farhelm forwards), the user sees text highlighted and
nothing on the clipboard, with no clue why. After this plan, Farhelm shows a short notice over the terminal in exactly
that case, telling the user the program handles selection itself, so they need its own copy command, or can hold Option
(Shift off macOS) while dragging to copy with Farhelm. When the session is known to be Codex, the notice says how to
copy in Codex.

Acceptance criteria:

- The notice appears after a plain left drag (pointer moved between press and release, no forcing modifier, so no local
  selection was made) in a pane whose program has mouse tracking on, when no OSC 52 write from that pane arrives between
  the press and about 1.5-2 seconds after the release.
- No notice for a click without a drag, for a drag the program answered with an OSC 52 write (Codex's conversation
  area), for an Option/Shift drag (Farhelm's own selection), or in a pane without mouse tracking.
- Each distinct notice text shows at most once per page load, and fades after a few seconds. It never takes focus, never
  blocks input, and is announced politely to screen readers.
- The modifier it names matches xterm's own rule for which modifier forces a selection (Option on the platforms xterm
  treats as Mac, Shift elsewhere).
- In the agent pane of a session whose agent kind is Codex, the notice gives Codex's copy instruction (verified on the
  installed Codex); shell tabs always get the generic notice.
- SPEC.md and SPEC_impl.md describe it; the TODO.md entry is removed.

## Requirement sources

**The user's request:** "let's plan the following, each in its own plan: ... codex prompt-box copy ...", for the TODO.md
`Near term` entry, verbatim as of cd8bd112: "**Copying text from Codex's prompt box does nothing.** Dragging over text
typed into Codex's prompt box (the composer at the bottom) highlights it, but nothing reaches the clipboard. Reproduced
2026-10-01 against Codex 0.159.3 in a plain tmux, outside Farhelm: Codex turns on mouse reporting, so a plain drag goes
to Codex rather than to the terminal's own selection (SPEC.md, Terminal experience). Over the conversation above the
prompt box, Codex copies on release by itself (an OSC 52 write, with "Copy sent to terminal" in its footer), and Farhelm
forwards that to the clipboard. In the prompt box it only highlights; the copy happens only if Ctrl+C is pressed while
the highlight is up, which Codex does not advertise there. Option-drag on macOS (Shift-drag elsewhere) already works,
because it makes Farhelm's own selection, which copies on release. So the gap is Codex's behavior, and the fix question
is what Farhelm should do about it: a hint, making a Codex prompt-box drag copy without the user knowing about Ctrl+C,
an upstream report, or a combination. Any Codex-specific handling goes where the harness map in
`crates/farhelm-supervisor/src/agent_kind/mod.rs` says. Not yet checked: whether other harnesses' prompt boxes behave
the same way."

**What the user was told (2026-10-02):** under mouse reporting the terminal makes no selection, so Farhelm cannot
override the program's copy; herdr (an agent multiplexer) forwards drags the same way and would show the same behavior;
the behavior is new in Codex (0.156 added a fullscreen interface with its own mouse selection, 0.157 made it the
default; `/tui` → Scrollback, saved as `tui.fullscreen_transcript = false`, restores the terminal's own selection);
Codex's `tui.copy_on_select` covers its transcript only, not the prompt box; sending Ctrl+C on the user's behalf was
ruled out (without a highlight it interrupts or quits Codex).

**The user's decision (verbatim):** "given this information, let's instead have a hint for the user telling them they
need use the apps native copy feature (find a good way of putting it). then register a maybe later todo to consider
adding a launcher option to turn of fullscreen or otherwise make this work better out of te box with codex. but the hint
at least gives the user a clue wtf is going on. maybe also make the hint understand codex - if it's known to be codex,
tell them how to perform the copy."

- The Maybe later TODO entry was added by the planning PR; this plan does not touch it.
- Frequency, proposed to the user and refined by the planning review: at most once per page load per notice text.
- Review gate: a fresh-context Opus 5.5 reviewer at high effort. No-workhorse mode (see How to run).

**Binding repository constraints:**

- SPEC.md Terminal experience (~987-997): selection copies; Shift-drag (Option-drag on macOS) forces a local selection
  under mouse reporting; OSC 52 writes are honored; clipboard operations are best-effort and silent on failure. Frame
  the notice as guidance, not a clipboard-failure report, so it does not collide with the silent-failure clause.
  SPEC_impl.md clipboard paragraphs (~584-598, ~714-730).
- Root `AGENTS.md` "Harness-specific code" and the map in `crates/farhelm-supervisor/src/agent_kind/mod.rs`: the Codex
  fact is an exhaustive per-kind function marked `#[warn(clippy::wildcard_enum_match_arm)]`, never a `kind ==
  Codex`
  test in shared code. The map's "How the browser shows it" bullet names only LaunchHarness-keyed places; update it in
  the same PR to name where this AgentKind-keyed browser fact lives.
- Desktop asset parity (`scripts/check-desktop-assets.sh`): a new asset file must be added on both sides; avoid one.

**Planner choices (from the planning review):**

- P1. The browser does not know a session's agent kind today: the UI's `Session` mirror in
  `crates/farhelm-ui/src/lib.rs` has no `agent_kind` field (the wire type does). Add one, decoded tolerantly: an older
  bundle meeting a kind a newer helm introduced must not fail to decode the session list (`farhelm_proto::AgentKind` has
  no catch-all; follow `HostKind`'s local catch-all precedent or a lenient `Option`). Key on agent kind, not the launch
  harness, so raw and profile Codex launches (no structured launch) get the Codex wording too.
- P2. One exhaustive function maps the kind to the optional copy instruction (Codex: the instruction; every other kind:
  none). `terminal_specs` in `crates/farhelm-ui/src/session_view.rs` passes it on the agent (primary) spec only.
- P3. Put the pure decision (drag vs click, tracking at press, no local selection, no OSC 52 since press, the
  xterm-exact Option/Shift platform predicate, once per page per text) in `crates/farhelm-ui/assets/copy-on-select.js`
  and its existing node test file, not a new asset module.
- P4. Detection reuses the existing copy-on-select listener pair in `crates/farhelm-ui/assets/terminal.js`
  (`handleTerminalMouseDown` / the deferred `handleCopyOnSelectMouseUp`): at press record coordinates,
  `term.modes.mouseTrackingMode` (present in the vendored xterm) and a per-pane OSC 52 counter; at release the existing
  `hasSelection` already tells whether Farhelm made a selection; re-check the counter after the window. No mousemove
  listener. Count OSC 52 per pane with a fall-through `term.parser.registerOscHandler(52, …)` returning false (the
  shared `clipboardProvider` cannot tell panes apart). Optionally ignore drags that start outside `.xterm-screen` (the
  scrollbar).
- P5. The notice is a sibling of the `.attach-status` overlay (absolutely positioned, `pointer-events: none`), not that
  node itself (its painter clears it). CSS fade, no second timer. Prefer the top-right corner so it does not cover
  Codex's prompt box at the bottom. A `role="status"` element present before its text is set announces reliably.
- P6. Codex wording: in Codex's prompt box, Ctrl+C without a highlight clears the draft or interrupts a running turn,
  and a second press quits. Verify the current behavior on the installed Codex (in a scratch tmux; opening Codex and
  typing into its prompt box spends no vendor turns) and keep "while the text is still highlighted" explicit.
  Ctrl+Insert (Codex #50215) may be a safer instruction; advertise it only if verified to work through Farhelm's browser
  terminal. Also check whether Scrollback mode leaves mouse tracking off (then the notice can never fire there, which is
  correct).

## Implementation outline

One PR, `feat:`. Line numbers drift; find code by name.

- UI: P1 (the `Session` field), P2 (the mapping function and the spec field), the harness-map bullet.
- Assets: P3 in copy-on-select.js with node tests in `crates/farhelm-ui/js-tests`; P4 in terminal.js; P5 markup and CSS
  in `crates/farhelm-ui/assets/app.css`.
- Browser tests, two, in an existing spec file (e.g. `e2e/tests/terminal-clipboard.spec.ts` or `mouse-modes.spec.ts`):
  - Generic: a stub program that enables mouse reporting (`?1000h` + `?1006h`) and answers its first input byte with an
    OSC 52 write, then stops. Order the negatives first because the notice shows once: a click shows nothing (a
    `sleep-ok` observation window), an OSC 52-answered drag shows nothing, then a plain drag shows the generic notice
    naming the platform's modifier.
  - Codex: a session whose invocation is an absolute-path stub named `codex` written into the stack's scratch directory
    (`derive_kind` uses the basename; a `#!/bin/sh` that ignores its arguments tolerates the hook flags the supervisor
    adds). Not the bare word `codex`, which the stack's PATH fake maps to a fake agent that never enables mouse
    reporting. A plain drag shows the Codex wording.
  - Check existing tests that drag under mouse reporting or capture pixels of such panes (terminal-clipboard,
    mouse-modes, readme-video beats) still pass with the overlay present.
- SPEC.md one sentence in Terminal experience; SPEC_impl.md a sentence by the clipboard-writes paragraph. Changelog
  fragment `kind: added`. Remove the TODO entry.

### What not to build

No change to Codex's launch flags or config (that is the Maybe later entry); no synthetic Ctrl+C; no fallback copy of
screen text; no upstream issue filing; no new asset module; no per-tab hint state.

## Plan-specific notes

### Validation

Likely relevant: `cd crates/farhelm-ui/js-tests && node --test`, `cargo fmt --all -- --check`, both clippy runs, the
farhelm-ui unit tests touched, `cargo check -p farhelm-ui --features desktop`, `scripts/check-desktop-assets.sh`, the
changed Playwright specs on Chromium and WebKit through the recorder, `python -B scripts/check-test-sleeps.py`, and
`python3 releasing/check-changelog.py format`.

### Decisions to log

The notice wording (generic and Codex), the window length, how the agent kind is decoded tolerantly, the notice's
placement, what the Codex verification found (Ctrl+C vs Ctrl+Insert, Scrollback mode's mouse state), and any reviewer
finding you declined.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-drag-copy-hint-log.md` in
the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before you
start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

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
  `plan/drag-copy-hint/<nn>-<short-name>`.
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
