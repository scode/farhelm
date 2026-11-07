# UI: a printed path that a program broke across two lines links to the whole path

Written against main at 66e77cf4 on 2026-11-07. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Fix click-to-download when a printed absolute path is split by a line wrap.** On 2026-11-07 a macOS brick-test run
> printed `Before screenshot (/tmp/farhelm-tmp/8B3AAC70-9A39-4B45-8A69-0842AC8C77F4/before-reopen.jpg)`,
> `Failure screenshot (/tmp/farhelm-tmp/8B3AAC70-9A39-4B45-8A69-0842AC8C77F4/failure.jpg)` and
> `Report and logs (/tmp/farhelm-tmp/8B3AAC70-9A39-4B45-8A69-0842AC8C77F4/report/REPORT.md)` on one line, and the
> terminal wrapped that line after `/tmp/farhelm-`. Hovering the `failure.jpg` link disclosed `failure.jpg`, with the
> session's checkout directory followed by `tmp/8B3AAC70-9A39-4B45-8A69-0842AC8C77F4/` as the directory, and
> `Not found`. So recognition offered only the wrapped continuation (`tmp/8B3AAC70-…/failure.jpg`) as a path, the host
> resolved that relative path against the session cwd, and the click to download failed. Start by reproducing a printed
> absolute path that soft-wraps, then check both seams: whether the continuation row carries `wrapped` in the terminal
> that showed this (`crates/farhelm-ui/assets/terminal-files.js`, `pathLinks`, against `grid-buffer.js`'s `linkLines`),
> and whether a path-like token that opens a line should be offered at all when it is the visibly cut tail of the line
> above. A relative printed path resolving against the session cwd is by design
> (`crates/farhelm-supervisor/src/service/downloads.rs`, `resolve_path`), so any fix belongs in recognition, not there.

What the planning research found, from a read of main at 66e77cf4 (line numbers approximate):

- `pathLinks` in `crates/farhelm-ui/assets/terminal-files.js` (around 161-189) already joins rows that the terminal
  wrapped (it walks back and forward while a row's `wrapped` is true, up to 128 rows and 8192 characters), and both
  terminal kinds go through it: grid terminals through `grid-buffer.js`'s link lines, old terminals through `xtermLines`
  (`terminal-links.js` around 597-614). `pathTokens` (around 102-123) splits on whitespace and on quotes and brackets,
  keeps `-`, and strips trailing `.,;!?` and `:line[:col]`. On a joined line the incident's text yields exactly the
  absolute path.
- So the continuation row was read as not wrapped. The likeliest cause is that the program broke the line itself: agent
  interfaces (Claude Code, Codex) wrap their own output with real line breaks or cursor moves, and the break fell right
  after a hyphen, where text wrappers break and a terminal's own wrap would only land by coincidence. Then no wrap flag
  is ever set (`vendor/alacritty_terminal` sets `WRAPLINE` only on an automatic wrap; the supervisor's `grid_output.rs`
  `row_from_cells` around 211 and `Row.wraps` in `crates/farhelm-proto/src/grid.rs` around 820-822 report it
  faithfully).
- A smaller possibility: `grid-buffer.js`'s `lineView` (around 641, with `previous = source.line(abs - 1)` around 683)
  reads screen row 0's "continues the previous line" from the last history line, which can be missing when that line is
  not applied yet (`abs >= historyEnd`, around 592) or on the alternate screen, so a real wrap across the screen/history
  boundary reads as not wrapped.
- Today the head of the cut path (`/tmp/farhelm-` at the end of the first line) is offered too, as that absolute path,
  and also fails.
- SPEC.md around 2668-2672 (which paths count) and SPEC_impl.md around 3758-3764 ("conservative token recognition",
  "Whitespace, quotes and brackets delimit ordinary paths") and around 1757-1759 (link lines). Neither says anything
  about paths a program broke.
- Tests: `crates/farhelm-ui/js-tests/grid-buffer.test.js` around 396 (paths across soft wraps; `wire(runs, wraps)`'s
  second argument is the `w` flag), `crates/farhelm-ui/js-tests/terminal-files.test.js` around 196-240 (tokenizer), 306
  (wide, combining and wrapped cells through both the `lines()` fixture and `xtermLines`), 936 and 972 (wrapped redraw
  and cached endpoints); `e2e/tests/terminal-files.spec.ts` prints through `sh -c 'printf ...'` and hovers with
  `hoverPath`.

### Acceptance criteria

- **Establish the cause first.** Unit tests pin both shapes of the incident's text: a terminal wrap (the head row
  flagged as wrapping) and a program break (no flag). If the screen/history boundary loses a real wrap's flag, fix that
  and pin it with a test; if it does not, a test shows it is read correctly.
- **A program-broken path links to the whole path.** When a path is cut by a program's line break, the head and the tail
  are both offered as one link to the joined path, and hovering either half discloses the joined path. In the incident,
  `/tmp/farhelm-` and `tmp/8B3AAC70-…/failure.jpg` both link to `/tmp/farhelm-tmp/8B3AAC70-…/failure.jpg`.
- **What counts as cut.** All of these hold, otherwise both rows are read as today:
  - the head row's last non-blank characters form a path-like token run with no whitespace, quote or closing bracket
    after it (the row may end in blanks after the token, since TUIs pad);
  - the tail row has nothing but blanks before its first token, which starts with a path character (leading indentation
    is allowed and is not part of the path);
  - the tail's first token would not have fit on the head's row: the columns left after the head's last character, minus
    an allowance of up to 4 columns for a TUI's right margin, are fewer than the tail token's length. That is how a text
    wrapper decides to break, and it keeps two unrelated lines from joining when the first one ends early;
  - the joined token is a path by the existing rules.
- **Both terminal kinds** behave the same, through the one shared `pathLinks`; the same rows run through the `lines()`
  fixture and through `xtermLines` in `terminal-files.test.js`.
- **No other recognition changes:** soft wraps, single-row paths, URLs, `file://` links and the tokenizer's delimiters
  keep working as today. A relative path still resolves against the session's directory.
- SPEC.md (around 2668-2672) and SPEC_impl.md (around 3758-3764) state the rule, and state it as a deliberate exception
  to conservative recognition (see the decision below).
- A browser test prints a hard-broken path (an explicit newline where the head row ends near the right edge, with an
  indented tail) and checks that hovering either half discloses the joined path, on Chromium and WebKit.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`).
- The last code PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-07, on the planning page:

- **Join the halves.** Both the head and the tail become one link to the joined path. In the maintainer's words: "SPEC
  this as an intentional deviation from our typical conservatism about correctness because the UX benefit is
  sufficiently high." The page's alternative, offering neither half as a link, was the planner's recommendation and was
  not chosen.
- **What "cut" means** (planner rule from the planning page, made precise after the planning review; the precise
  thresholds are planner proposals): the criteria under Acceptance criteria above.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's terminal file paths (around 2668-2672); SPEC_impl.md around 3758-3764 and
1757-1759; `resolve_path` stays as it is (the TODO entry says so); root `AGENTS.md`, "Tests and validation",
"Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`;
`docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- One change in `pathLinks`: extend its backward and forward joins with a "cut" predicate beside `wrapped`, so ranges
  spanning rows, hover disclosure and the cached-endpoint checks work as they do for soft wraps. No per-terminal logic.
- The 4-column right-margin allowance and "blanks only" before the tail are the starting thresholds; tune them only if a
  real agent's output shows they miss the incident's shape, and log the change.
- The joined path includes no character from the tail row's indentation.

## Outline

One PR:

1. `plan/cut-path-links/01-cut-path-links`: the cause tests (and the boundary fix if it is real), the cut-path join in
   `pathLinks`, the JS unit tests for both terminal kinds, the browser test, SPEC.md and SPEC_impl.md, the changelog
   fragment and the TODO.md removal. If the boundary fix turns out real and sizeable, it may go first as its own PR.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, the JS
harness (`cd crates/farhelm-ui/js-tests && node --test`), the grid client suite if `grid-*.js` changes
(`cd e2e && npx playwright test --config grid-client/playwright.config.ts`), `python -B scripts/check-test-sleeps.py`
with the interpreter `docs/test-sleep-check.md` prepares, and, through `python3 scripts/record-test-run.py`,
`e2e/tests/terminal-files.spec.ts` and the link-click specs, on both engines.

## Fallbacks

Examples of departures that need the scope reassessment: changing how the host resolves paths, joining across more than
two rows by the cut rule, recognizing gutter characters (box drawing, quote bars) before the tail, or a per-agent rule.

Decisions to log: whether the boundary flag loss was real, the final thresholds, every test changed and why, and every
review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-cut-path-links-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/cut-path-links/<nn>-<short-name>`, as the
  outline names them.
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
