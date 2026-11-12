# Finish removing tmux and xterm.js: specs, docs, comments and a final audit

Written against main at 683a9a72 on 2026-11-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says; this plan changes their text, never the behavior they describe.

Its TODO.md sources are the Near term entry "Finish removing tmux and xterm.js after old terminals end" and the Doc todo
entry it deletes. This is the last of four chained plans that together remove tmux and xterm.js from Farhelm ("phase
2"). It runs after `tmux-end-old-terminals.md` (the running product: a startup shutdown of the private tmux server is
the only tmux left), `tmux-drop-from-install.md` (no tmux in setup, the desktop app, provisioning, uninstall, the
installer or release payloads) and `tmux-drop-from-tooling.md` (no tmux in test tooling, CI, the brick test or agent
instructions) have landed. Each of those updated the specs and docs for what it changed. Read their reports
(`plans/REPORTS.md`, or `git log` for their PRs) first: this plan finishes what remains and checks that nothing was
missed.

## The goal

### The maintainer's request, in their words

> plan the work to completely nuke tmux/xterm.js support.

> besides certain narrow helpers that are copied from xterm.js (should be with attribution), the intent is to have no
> trace of them. unless you find something that seems like it's worthy of my attention and is somehow needed. we want to
> not find in 2 months there's some crazy complexity somewhere due to tmux/xterm.js still

> it's not JUST about deleting references to those things, also understanding how the code works today and if we have
> complexity that is only there either because (a) of tmux specific concerns or xterm.js specific concerns or (b)
> complexity just because we had two different types for a while.

### What stays, on purpose

- The startup shutdown of the private tmux server (SPEC_impl.md's remaining tmux section, its code and tests), until a
  later phase 3 removes it.
- Everything about the USER's own tmux: no `TMUX`/`TMUX_PANE` passed on, `TERM_PROGRAM=farhelm`, the 2026-11-10 rules
  for jobs under a user's tmux, the per-user shared tmux server, startup files that autostart tmux.
- Code ported from xterm.js or VS Code with its attribution, THIRD_PARTY_NOTICES.md's "Code ported from xterm.js" and
  `grid-ported-code-LICENSES.txt`. "xterm" as the terminal name in `TERM=xterm-256color` and "the same modifier xterm
  itself uses" (the X terminal, not xterm.js).
- Farhelm's own behavior that began as tmux's or xterm.js's (the 12,000-line scrollback, the screen-text format status
  detection reads, terminal query answers, key and mouse encodings, the grid's fit and 14px scrollbar reserve, cursor
  and selection defaults): kept, with rationales in their own terms.
- Frozen material (root `AGENTS.md`, "Frozen material"): `lore/`, released CHANGELOG sections, release notes JSON and
  the release shots, TRIAGE_OUTCOMES records, older FLAKES.md entries, `plans/`, the captured agent screens (tmux-era
  ones included), the README hero and video designs, the shipped release announcement in
  `crates/farhelm-helm/src/release_announcements.json`.
- Supervisor schemas 32-34 and their history in SPEC.md "Upgrade compatibility" and SPEC_impl.md (installs from v0.33 or
  older can still upgrade directly).

### Acceptance criteria

1. **SPEC.md and SPEC_impl.md** describe one kind of terminal. Remaining text about tmux, xterm.js, old terminals or the
   coexistence of two kinds is deleted or rewritten to describe terminal hosts and the grid, except what stays on
   purpose above. In SPEC_impl.md, "Terminal widget: xterm.js island" holds about 190 lines of general GUI content
   (fonts, sidebar width, focus, sounds, text size, the quick switcher, the shortcut systems, naming) and the grid's
   generated-id joining: move them under a heading that names what they are; do not delete them. The desktop window
   frame persistence paragraph inside the tmux section moves to the desktop section. The remaining tmux section states
   only the startup shutdown. Planning research listed paragraph-level dispositions; re-derive them from the current
   text rather than trusting line numbers.
2. **The docs website and `docs/`** have no stale tmux or xterm.js text: remaining website pages
   (`agents/agent-wrappers.md`, `get-started/uninstall.md`, `website/EDITORIAL_RULES.md` if its rule is now moot, and
   any others a search finds), `docs/farhelm-architecture.svg` (embedded by `README.md`; it still draws the system
   around `tmux -C` and a private tmux server), `docs/install_uninstall.md` leftovers, `docs/desktop-web-triage.md`,
   `docs/watch-items.md`, `docs/agent-screen-fixtures.md` (its reason for keeping tmux-era screens: old sessions no
   longer exist, but hosts started on v0.34-v0.35 still carry the old environment and show Claude's tmux hint), and
   `docs/codex-input-investigation.md` (keep as a historical investigation record, say so at its top, or remove it if
   nothing links to it; log which).
3. **Comments and rationales** in code: about 300 comparisons with xterm.js or old terminals remain in the grid client,
   its CSS, the emulator and the supervisor (planning counted grid-terminal.js 75, grid-input.js 28, grid-buffer.js 26,
   app.css 28, terminal.js 171 before the first plan). Rewrite each to stand on its own or drop it; keep the attribution
   comments of ported code verbatim. Constants and behaviors kept on purpose get a rationale in their own terms where
   the earlier plans did not already give one. `crates/terminal-host/SPEC.md`'s rationale for `VERASE` and `IUTF8`
   ("what an old Farhelm terminal's tmux pane has") is reworded under that file's "Change policy" (wording only).
   `vendor/README.md`'s patch rationales that cite tmux or xterm.js as the reason are reworded as Farhelm's choices, per
   `vendor/README.md`'s rules (no change to upstream trees or patch files).
4. **TODO.md:** delete the Doc todo entry "Tell users that agents and terminal tabs run without the private tmux
   server's `TMUX` and `TMUX_PANE`" (the maintainer: accepted behavior, already documented on the new-terminals page).
   Rewrite only the tmux or xterm.js wording, keeping each entry's substance, of: Doc todo "Tell users that closing,
   exiting, or deleting a terminal tab also stops services..." (only its new-terminal half survives); Maybe later
   "Require a systemd user manager on Linux, with no fallback." (its "successful tmux kill" case and "the process
   numbers tmux reports" have terminal-host equivalents); "Replace `install.sh`'s park/journal/rollback with plain
   idempotency." ("PATH and tmux advice"); "Investigate harness-native notifications." (where escape codes end up);
   "Close the desktop window's filesystem read fallback (hardening only)." (its xterm.js clauses). Leave "Restore
   automatic recovery from a withdrawn waiting update" and "Input-method typing in grid terminals" as they are. The last
   PR of this plan removes the Near term entry "Finish removing tmux and xterm.js after old terminals end", which
   references plans 2 to 4; leave the phase 3 entry ("Remove the startup shutdown of the old private tmux server") in
   place. Follow root `AGENTS.md`, "TODO.md and FLAKES.md", for fetching main before editing.
5. **Final audit.** Search the whole repository (case-insensitive `tmux`, `xterm`, `old terminal`, `old-terminal`,
   `terminal kind`, `byte stream`, `grid=1`, `pending kind`, and anything else the earlier reports suggest) and account
   for every hit outside frozen material: fixed here, or kept on purpose with its reason. Fix what is documentation,
   comments, dead code or small behavior-preserving cleanup of tmux-, xterm.js- or two-kind-shaped complexity. Anything
   larger, or anything that would change behavior, goes in the report as a finding for the maintainer with what it is
   and why it exists, rather than being changed here. The report lists every kept category with its reason so the
   maintainer can see nothing was silently left. Hits in `review_feedback_queue/` are re-checked against the code as it
   now is, then removed or narrowed per `review_feedback_queue/AGENTS.md` where their subject is gone, and left alone
   where it is not (planning noted, among others, `tab-reap-deadline-excludes-listing-request`,
   `stop-annotation-exit-witness`, `tab-survival-missing-live-exchange`, `restart-unwind-omits-tree-sweep-failure`,
   `host-restart-history-painter-omits-palette`, `attachment-failure-announcement-unresolved`,
   `grid-shift-enter-loses-independent-altgraph`, `initial-oversized-grid-cannot-attach`,
   `replacement-cleanup-missing-pane-root-source-gap`; `tmux-image-role-recognition-unresolved` concerns the user's own
   tmux and stays).
6. Each PR leaves main coherent.

## Decisions already made

Settled by the maintainer while planning on 2026-11-11:

- **The Doc todo on `TMUX`/`TMUX_PANE` is deleted**, not rewritten: a startup file that checks for tmux no longer seeing
  it inside Farhelm is accepted and expected.
- **The other picked TODO.md entries get their wording updated**, as listed in criterion 4.
- **Reviewer:** a single fresh-context agent on Opus 5.5 at high effort (see Review gate). If it cannot be reached when
  a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default.
- **Size gate:** about 3,000 added lines across the stack (mostly rewritten docs and comments; deletions not counted).
  Over it, block and bring it back.

## Requirement sources

**The user's request:** the quotes and decisions above.

**Binding repository constraints:** root `AGENTS.md` ("Frozen material", "Talking to the user", "Conventional Commits",
"TODO.md and FLAKES.md", "Tests and validation", the Claude Code shell rule, and "Instructions by path":
`website/AGENTS.md` for the website and `README.md` (an unattended run starts no preview server and names the changed
pages instead), `crates/terminal-host/SPEC.md` "Change policy", `vendor/README.md`, `lore/AGENTS.md` (only to confirm
you leave `lore/` alone), `.agents/docs-media.md` if a docs image changes, `review_feedback_queue/AGENTS.md` for queue
items); the `scode-voice` skill for prose. Do not change behavior SPEC.md or SPEC_impl.md specifies; a text change that
would is a finding, not an edit.

**Planner proposals** (change them if the text says otherwise, logged as a DECISION):

- Rename "Terminal widget: xterm.js island" to name the GUI content it holds, and check `.agents/old-sections.md` and
  `plans/AGENTS.md`, "Old section names", for whether a renamed heading needs an alias entry (root `AGENTS.md`, "Reading
  these instructions"); `plans/` itself is never edited by a plan PR, so if an alias is needed there, name it in the
  report.
- Redraw `docs/farhelm-architecture.svg` by hand-editing its text and shapes to show terminal hosts and the grid; keep
  its style.

## Outline

Bookmarks are `plan/tmux-drop-from-docs/<nn>-<name>`. Split further if a step is too large to review.

1. `01-specs`: criterion 1. `docs:`.
2. `02-docs`: criterion 2. `docs:`.
3. `03-comments`: criterion 3. `docs:` or `refactor:` as fits.
4. `04-audit`: criterion 5's fixes, and criterion 4 (TODO.md) as the last PR. `docs:` or `refactor:`.

No changelog fragment unless a PR changes something a user sees.

### Validation

Documentation and comment changes: `dprint check`, the website build
(`cd website && bun install --frozen-lockfile && bun run build`) for website changes,
`python3 scripts/check-agents-md.py` for heading renames and instruction files, and for comment-only code changes
`cargo fmt` check and clippy (comments can break doc links: run `cargo doc --no-deps` on touched crates if rustdoc links
changed) and the JS harness if JS comments moved code. Any small code cleanup from the audit gets the targeted tests
that cover it, through the recorder. No full test batteries for documentation (root `AGENTS.md`). Report checks run,
reused and skipped.

## Fallbacks

Examples of departures that need the scope reassessment: changing behavior to match a spec sentence, rewriting frozen
material, or deleting general content that happened to sit under a tmux or xterm.js heading.

Decisions to log: every audit hit kept and why (grouped), each heading moved or renamed, how the architecture diagram
changed, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tmux-drop-from-docs-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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
available as a native sub-agent type, `$scode-harness-shellout` supplies the launch mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/tmux-drop-from-docs/<nn>-<short-name>`,
  as the outline names them.
- Keep each PR reviewable. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too. Splitting a large outline step into several PRs is fine.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high effort,
launched natively when your harness serves that model, otherwise through `$scode-harness-shellout`. If it cannot be
reached when a review is due and is still unreachable after a reasonable retry, block the plan per
`plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute another
reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires. For a mostly-deleting PR, also ask the
reviewer to look for what the deletion left behind: dangling references, comments and docs describing removed behavior,
and code whose only reason was the removed path.

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
passed its review gate, and the PRs make the TODO.md and review-queue changes the decisions name. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
