# UI: make the bundled terminal font a hard requirement

Written against main at b1eed469 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The web UI and the desktop app ship JetBrains Mono Nerd Font as a bundled asset (`app.css`'s `@font-face` rules, served
from the flat `/assets/JetBrainsMonoNerdFont-*.woff2` paths `crates/farhelm-ui/src/lib.rs` registers). `terminal.js`
nevertheless treats it as a best-effort enhancement: its "## Font settling before mount" design waits up to 3 seconds,
then mounts the terminal in a fallback font, and keeps late paths to swap the family in and re-fit, and to repaint bold
cells, if the font shows up afterwards. The maintainer decided on 2026-10-01 that this is complexity to remove. The
TODO.md entry (bucket "Definite simplification"), verbatim:

> **Make the bundled terminal font a hard requirement.** Decided 2026-10-01: JetBrains Mono Nerd Font ships with
> Farhelm, so failing to load it is a broken deployment, like any other bundled asset, not something to survive. Remove
> the fallback in `crates/farhelm-ui/assets/terminal.js` ("Font settling before mount": the 3 s settle deadline,
> mounting in a fallback font, the late font swap and repaint paths, the do-nothing tracker for engines without the Font
> Loading API) and gate terminal mounts on the font the way `mountWhenReady` already gates on the other bundled files.
> The retry against the browser briefly reporting no face before the font is registered may still be needed. Accepted
> cost: in the web UI over a slow link, a terminal shows nothing until the roughly 2 MB font arrives instead of
> appearing in a fallback font after 3 s. Removes the cause of `terminal-font-promise-leak.md` (discarded 2026-10-01).

Background in `TRIAGE_OUTCOMES.md`, headings `terminal-font-promise-leak.md` (each mount subscribed to a page-wide font
promise that never settles on the fallback paths, retaining every terminal instance across long reconnect loops) and
`constructor-font-regression-observes-mutable-value-construction.md` (a weakness of the current constructor test that
this removal makes moot). Both were discarded on the basis that this plan removes the code involved.

### Acceptance criteria

- A terminal mounts only once both JetBrains Mono weights (regular and bold) have loaded, as one more condition in
  `mountWhenReady`'s gate beside the bundled scripts it already waits for. There is no deadline: a terminal waits for
  the font as long as it waits for its scripts.
- The terminal is always constructed with the JetBrains Mono family; no constructor option depends on whether the font
  loaded.
- Removed from `terminal.js`: the 3 s settle deadline (`FONT_SETTLE_DEADLINE_MS` and its timer), mounting in a fallback
  font, the late family swap and re-fit (`regularFont.loaded.then(...)` in `mount()`), the late bold repaint
  (`boldFont.loaded.then(...)`), the Font Loading API capability guard and the do-nothing tracker (`dummyFontWeight`),
  and the `settled`/`loaded` split that only the fallback needed. No per-mount subscription to a page-wide font promise
  remains, so the retention `terminal-font-promise-leak.md` described cannot recur.
- Kept: the retry of an empty `document.fonts.load()` result (the browser reports no matching face when asked before
  `app.css` has registered it; `pollFontWeight`'s header explains the race), now without a deadline, and the font work
  still starts from the first `mountWhenReady` call rather than at script evaluation.
- A font download that fails outright (a rejected `load()` or one that throws) logs exactly one error to the browser
  console, however many weights failed, and stops trying, so the terminal never mounts until the page is reloaded. No
  in-terminal message.
- Nothing new is shown while the terminal waits for the font: no "connecting…" line before mount. This needs no code:
  the placeholder is painted only inside `mount()`, which the gate holds back, so do not add one.
- The "## Font settling before mount" header, `mountWhenReady`'s docs, and every other comment in `terminal.js` that
  describes the fallback are rewritten to the new contract (the font is a mount precondition like the bundled scripts,
  and why it is set at construction: xterm measures cell metrics at `open()`, and a later family change needs a re-fit
  that reflows tmux).
- Browser tests in `e2e/tests/terminal-font.spec.ts`, replacing the fail-open ones:
  - F7b (a hung fetch mounting at the deadline) becomes: a held font request keeps the terminal unmounted past the old 3
    s deadline, and once the request is released the terminal mounts and renders real output in the JetBrains Mono
    family.
  - F7a (a rejected fetch mounting in the fallback font) becomes: a rejected font fetch leaves the terminal unmounted
    and logs exactly one Farhelm font error to the console. This is the only coverage of the failed-download decision,
    and it also proves each engine rejects rather than answering with an empty list (which would loop silently).
  - F8 (the family set by the constructor) is deleted, with `attachAtFontMountBoundary`, which only F7a and F8 use: once
    the family is unconditional, reading it back checks a literal. Its family check folds into the held-request test.
  - The text-size tests stay. The file's header (the F1/F2 fail-open contract, `FONT_SETTLE_DEADLINE_MS`) and its "F9 —
    deliberately NOT covered" section are rewritten to match.
- Every comment that describes the fallback or the late re-fit as live behavior is corrected. Known on main at b1eed469
  (line numbers approximate): `terminal.js` around 5812 (the `fit` field's "font-swap backstop"); `app.css`'s
  `font-display: swap` comment (the property stays, for chrome); `crates/farhelm-ui/src/lib.rs` around 1299 (missing
  fonts now mean terminals never mount, not a wrongly rendered page); `e2e/tests/helpers/real-agent.ts` around 163-171
  and 224-240; `e2e/tests/terminal-clipboard.spec.ts` around 93-100, 141-147 and 457-468;
  `e2e/tests/terminal-keys.spec.ts` around 205-213; `e2e/tests/terminal-links.spec.ts` around 215-218;
  `e2e/tests/fixture-contracts.md` around 30-38 (the "post-mount font swap" bullet). Search for others before finishing.
- Test waits whose only stated reason is the removed backstop (the `document.fonts.ready` waits in
  `terminal-keys.spec.ts` and in `terminal-clipboard.spec.ts` around lines 477, 542, 586 and 640, and any others the
  search finds) are deleted, or kept with a corrected reason when something else still needs them. Log each one, and run
  the specs whose behavior changed.
- SPEC_impl.md's xterm.js island paragraph on JetBrains Mono gains the rule that a terminal mounts only once both
  bundled weights have loaded, the same as its bundled scripts, with no fallback font.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (probably
  `kind: changed`): the change is user-visible whatever its commit type. Over a slow link the web UI's terminal stays
  empty until the font arrives instead of showing a fallback font after 3 seconds, and a terminal no longer appears at
  all when the font cannot be downloaded.
- The PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **A failed font download** is treated like a missing bundled script: the terminal never mounts until the page is
  reloaded, with one console error. Chosen over an in-terminal "font failed to load, reload" line, which the maintainer
  declined.
- **While the font downloads,** the terminal area shows nothing, as the TODO entry accepted. Chosen over showing the
  existing "connecting…" line during the wait.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. No substitute
  reviewer.
- **Galaxy brain mode, with Mercury 3 via omp as the delegate.** The maintainer's words: "I want it in galaxy brain mode
  (not no-workhorse), and mercury 3 (via omp) as the delegated to agent." This overrides, for this plan only, the
  no-workhorse default `plans/.agents/planning.md` sets for plans. See Galaxy brain below.

Stated to the maintainer on the planning page, without objection:

- Both weights are required before mount, because waiting on regular alone would keep the late bold repaint the entry
  says to remove; the two files download in parallel.
- The Font Loading API is assumed present: every engine Farhelm supports and both desktop webviews have it.
- The empty-result retry stays, without a deadline.
- One PR.

## Requirement sources

**The user's request:** the TODO.md entry quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md's "Terminal experience" section; SPEC_impl.md's design-tokens paragraph on
`--font-ui`/`--font-mono` and its xterm.js island section; root `AGENTS.md`, "Tests and validation", "Conventional
Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md` for the test changes;
`docs/test-sleep-check.md` and `scripts/check-test-sleeps.py` for any timed wait the new test needs.

**Planner proposals** (from a read of main at b1eed469; change them if the code says otherwise, logged as a DECISION):

The planning review shaped these. Separate tracking per weight existed only because regular alone decided the fallback
(a combined `Promise.all` would discard one weight's success when the other failed); under the new contract either
failure means no mount, so that reason is gone.

- One readiness for both weights: one flag the gate reads, one started flag, and one attempt that loads both descriptors
  together (`Promise.all` over the two `document.fonts.load()` calls). Both results non-empty sets the flag; any empty
  result retries on the existing short interval; a rejection logs once and stops. Wrapping the attempt in
  `Promise.resolve().then(...)` turns a synchronous throw, a missing Font Loading API included, into the same rejection,
  which gives the single console error by construction.
- Remove the `__farhelmTestFontMount` instrumentation and `isPending`, which exist only for F7b's deadline evidence. The
  new tests observe from the host: the route records the font request; `__farhelmTermReady` still unset after a window
  well past 3 s shows the terminal waited (a loose lower bound under load: a slow run can only pass falsely, never fail
  falsely); `route.continue()` on the held routes releases them. Mark the window `sleep-ok` with its reason per
  `docs/test-sleep-check.md`.

## Outline

One PR, `plan/terminal-font-required/01-font-required`, holding the `terminal.js` change, the test changes, the comment
corrections, the SPEC_impl.md sentence, the changelog fragment if any, and the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `python -B scripts/check-test-sleeps.py` (with
the interpreter `docs/test-sleep-check.md` prepares), `dprint check` on the changed files, and through
`python3 scripts/record-test-run.py`: `terminal-font.spec.ts` on both browser engines, plus every spec whose waits or
comments changed and the specs that mount terminals heavily enough to catch a gate regression (`terminal.spec.ts`,
`terminal-clipboard.spec.ts`, `terminal-keys.spec.ts`, and the reconnect specs), on both engines.

## Fallbacks

Examples of departures that need the scope reassessment: a loading indicator or error message in the terminal area, a
deadline of any length, retrying a definitively failed download, a new asset-loading framework shared with the scripts,
or keeping a fallback font path "for safety".

Decisions to log: the tracker's final shape, what remains of the test instrumentation, every test changed or dropped and
why, the commit type and whether a fragment was added, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-terminal-font-required-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Galaxy brain

The user requires you to use `$scode-galaxy-brain` for this entire plan. Invoke it immediately after the resume protocol
and keep it active through every delegation; reading it for its mechanics does not satisfy this requirement.

The user demands one delegate model for every unit of your own decomposition that you delegate: Mercury 3, launched
through omp (`$scode-harness-shellout`, whose omp harness file names the selector; on OpenRouter it is
`openrouter/inception/mercury-3`). Pass that demand to routing as an explicit user demand. The effort for each unit is
yours to choose from the levels omp lists for that model. Whether a unit is worth delegating at all stays your judgment,
as galaxy-brain says. If Mercury 3 cannot be reached through omp, or fails a unit, do that unit yourself; never
substitute another delegate model.

Neither the review gate nor the scope reassessment is a unit of your decomposition. The review gate keeps the model and
effort this file demands; the scope reassessment runs at your own model and effort. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from galaxy-brain and run as that file says: native, read-only, at your own model.

### Resource watchdog

Immediately after activating galaxy-brain, and before the first delegation, start a resource watchdog as a background
process and keep it running for the whole run. Every 60 seconds it samples free space on the filesystems holding the
checkout, any jj workspaces or delegate trees you create, the agent scratch directory and `/tmp`, plus available memory
and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample to a
private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run, delegation or review launch. A dead monitor,
or a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead
watchdog.

An alert is an instruction to act:

1. stop launching delegates and work;
2. remove gated delegate trees, build output and scratch you own;
3. wait for or stop the delegate or job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/terminal-font-required/<nn>-<short-name>`
  (the outline's `plan/terminal-font-required/01-font-required`).
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

Before finishing each PR, launch a review of its changes through galaxy-brain and address what it finds before moving
on. The user demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high
effort. Pass it to galaxy-brain as an explicit demand for that model and effort; `$scode-harness-shellout` owns the
launch mechanics when your harness cannot serve that model natively. If it cannot be reached, block as Decisions already
made says; never substitute another model.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Delegates get
bounded units within the outline; an unapproved new mechanism is outside their task, and one discovered mid-unit is left
unimplemented and reported at the delegate's existing checkpoint, with checks and incomplete work stated honestly. Apply
the same decision boundary as Unattended fallback below.

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
