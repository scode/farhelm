# A path hover stays put while the terminal redraws under it

Written against main at 0c875c08 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term), "Fix the flickering hover on a path that cannot be downloaded": hovering a file path in
terminal output is meant to check it once per hover: a checking spinner, then the host, full path and size, or the
reason it is refused. Hovering a path that could not be downloaded (the maintainer thinks it was a directory) instead
made the hover flicker rapidly and keep flickering while the pointer stayed on it. The suspicion, not yet confirmed, is
that a refused check starts another check, so the hover loops between the spinner and the refusal for as long as it
lasts.

Planning found a different likely cause (see Outline): the terminal library ends and restarts the hover every time the
rows under it are redrawn, so a resting pointer over a constantly redrawn region gets a fresh check on every redraw.
That is inferred from the code, not observed.

Acceptance criteria:

- While the pointer rests on a path, redraws of the rows under it do not start another check and do not hide, blank or
  restart the tooltip. One resting hover sends exactly one check, whether the result is a refusal (a folder, say) or a
  downloadable file, and whether the check is still in flight or has answered when the redraw comes.
- Moving the pointer off the path still ends the hover, cancels an in-flight check and hides the tooltip, as today. A
  later hover of the same path checks afresh, as today. A different path, or the same path text at a different position,
  is a different hover.
- The hand cursor and click admission behave as today for the hovered link, and a link the pointer moves to is never
  left with a wrong cursor because of this change.
- Both kinds of path link are covered: paths recognized in plain terminal text and `file://` hyperlinks a program emits.
- SPEC.md's terminal section says that a redraw under a resting pointer is not a new hover, in the same PR.
- A browser regression on Chromium and WebKit and a JavaScript unit test, both described under Outline.
- One draft PR, passing its review gate, with its changelog fragment, and removing the TODO.md entry.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-11):** plan this TODO entry, through the todo planning flow.

**The user's decisions (2026-10-11):**

- A pointer resting on the same path (same text, same position) is one hover even when the terminal redraws under it:
  the shown result stays up and no new check starts. Moving off the path, or the text under the pointer changing, ends
  the hover as today. (Offered and not chosen: re-checking on every redraw but keeping the old result visible meanwhile;
  re-checking at a rate limit.)
- Amend SPEC.md's "Each hover checks afresh; leaving cancels it." so it says that a redraw under a resting pointer is
  not a new hover. (Leaving SPEC.md as is was offered and not chosen.)
- Reproduce first, then fix. In the user's words: "reproduce then fix but if reproduction isnt possible and no better
  hypotheis exists, fix anyway dont block".
- Tests: a browser regression on Chromium and WebKit (live redraws under a resting pointer, one check per hover, for a
  folder and for a downloadable file) plus a JavaScript unit test of the hover state logic.
- Scope: the hover fix only. Leave Farhelm's own 50 ms refresh while scrolled back, and every other redraw behavior,
  unchanged.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md "Terminal experience", the file-download bullet: hover underlines and shows a checking spinner, then the host,
  full resolved path and size or refusal; leaving cancels; only a successful check admits a click. SPEC_impl.md
  "Session-host file reads": no shell directory tracking or hover cache. The fix keeps a single hover's result for that
  hover only; it is not a cache across hovers.
- Root `AGENTS.md`: Conventional Commits; Finishing work (browser tests through the recorder on Chromium and WebKit, and
  `scripts/check-test-sleeps.py` for any test delay); Releases and the changelog; Agent scratch space; Sharing the
  machine with other agents.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanism and test shapes in the outline below, checked against main at 0c875c08 and
corrected by a fresh-context planning review. It found no unnecessary machinery. It confirmed from the vendored xterm
source that the re-hover arrives synchronously, inside the same redraw handler that ended the hover. It also caught that
deferring the cursor reset would switch off the cursor of whatever link is current when the deferred write runs.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- `crates/farhelm-ui/assets/terminal-files.js`, class `FileLinksAddon`. `activate()` registers a link provider whose
  `provideLinks` builds a new link object on every call, with `hover: (event) => this.hover(event, path, link)` and
  `leave: () => this.leave()`. `hover()` first calls `leave()`, creates a fresh state (`path`, `link`,
  `AbortController`, `info`), shows `['checking…', path]`, POSTs `stat`, and renders the result or the error.
  `this.hovered !== state` drops late replies. `leave()` aborts the request, sets the hovered link's
  `decorations.pointerCursor = false`, clears `this.hovered`, and hides the tooltip. `download(path)` admits a click by
  the hovered state's path and info; it needs no link object. The constructor already takes injectable
  `setTimeout`/`clearTimeout`, which is what the unit test uses for time.
- `crates/farhelm-ui/assets/terminal.js`, the terminal's `linkHandler`: `hover(event, uri, range)` calls
  `fileLinks.hover(event, target.path)` for `file://` hyperlinks without passing the link's range, and `leave()` calls
  `fileLinks.leave()`.
- The vendored xterm 6.0.0 (`crates/farhelm-ui/assets/vendor/xterm.js`, minified). In `Linkifier._handleNewLink`, a
  listener on `onRenderedViewportChange` runs `_clearCurrentLink` (our `leave`) and then `_askForLink` at the last mouse
  position whenever rows covering the current link are rendered, with no pointer movement. Every provider answers
  synchronously, so the new `hover` runs before any microtask. `_handleNewLink` also replaces `link.decorations` with
  accessors that write to whichever link is current when the setter runs.
- The redraw sources include an agent's live status line (Claude Code's footer shows the working directory, a folder)
  and Farhelm's 50 ms refresh while scrolled back with live output. The latter stays as it is.
- Server side needs no change: a folder is an ordinary result (`status: "folder"`), not an error.

Here's the work, in order:

1. **Reproduce.** Write the browser regression first and run it against the unfixed code. It must show more than one
   check for a resting pointer over a redrawn path. Log what it shows, including whether a downloadable file loops too.
   If it shows a different cause, fix that cause instead, as long as the fix delivers the decided behavior; log it as a
   DECISION. If it cannot reproduce at all and no better hypothesis exists, fix as below anyway and say so in the
   report. Do not block on this.
2. **Fix, in `FileLinksAddon`.** In `leave()`, set the hovered link's pointer cursor off at once, as today, then mark
   the state as leaving and schedule the abort and the tooltip hide for the next task through the injected `setTimeout`.
   In `hover(event, path, link, range)`, if a leaving state has the same path and the same range (start and end x and
   y), cancel the scheduled teardown and keep that state: its in-flight check, or its shown result. Swap in the new link
   object, and set its pointer cursor on if the kept result is ready, because xterm seeds the new link from the
   provider's `pointerCursor: false`. Otherwise tear down any leaving state immediately and start a fresh check as
   today. Pass `range` from `provideLinks` and from `terminal.js`'s `linkHandler.hover`, which xterm already supplies.
   Disposal and the terminal's lifetime must still clear a pending teardown timer.
3. **SPEC.md**: one clarifying clause in the file-download bullet of "Terminal experience", next to "Each hover checks
   afresh; leaving cancels it."
4. **Tests.**
   - JavaScript unit tests in `crates/farhelm-ui/js-tests/terminal-files.test.js`, with an injected clock and fetch:
     - a leave followed in the same task by a hover of the same path and range keeps the state and sends no second
       request, both while the check is in flight and after it answered;
     - a different range or path starts a fresh check;
     - a real leave (no re-hover before the timer fires) aborts and hides;
     - the cursor is switched off synchronously on leave and switched back on for a kept ready result.
   - A Playwright test in `e2e/tests/terminal-files.spec.ts`: output that keeps redrawing the row holding a folder path,
     and separately a downloadable file, with the pointer resting on it. Assert exactly one stat request over an
     observation window and that the tooltip keeps showing the result. The existing hover tests count stat requests and
     show the pattern. A deliberate observation window needs a `sleep-ok` annotation per `docs/test-sleep-check.md`, and
     the test must first prove that redraws really happened during the window (a readiness oracle, not just a delay).
     Run it on Chromium and WebKit.
5. **Bookkeeping.** Changelog fragment, `kind: fixed`. Remove the TODO.md entry.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cd crates/farhelm-ui/js-tests && node --test`;
- the changed Playwright spec on Chromium and WebKit through the recorder, after the builds the browser suite needs;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown;
- `python3 releasing/check-changelog.py format`.

No Rust changes are expected, so no Rust checks unless that changes. Do not run the full battery without a specific risk
that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-flickering-path-hover-log.md` in the parent directory of the checkout you run in, derived as that section
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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/flickering-path-hover/<nn>-<short-name>` (for example `plan/flickering-path-hover/01-steady-hover`).
- One PR: the reproduction test fails without the fix, so it lands with it. Keep it focused and avoid churn: code added
  in one PR and deleted in a later one means the stack should have been shaped differently. Within this run, a PR that
  needs correcting is restructured rather than corrected on top, and that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (`fix`). A `fix`
  PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog). Validate with `python3 releasing/check-changelog.py format`.
- The PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

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

The PR changes tests, so include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check:

- that a real leave still cancels the check and hides the tooltip;
- that no deferred write can touch another link's cursor;
- that a kept result never outlives its hover (no cache across hovers);
- that the browser test proves redraws happened during its window rather than merely waiting.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a cache of check results across
  hovers, a change to xterm's vendored code, a change to Farhelm's redraw scheduling, or a helm or supervisor change
  (examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular:

- what the reproduction showed, and whether the cause matched the outline;
- how a re-hover is recognized as the same hover, and how the teardown is deferred;
- how the browser test drives redraws and proves they happened;
- the SPEC.md wording;
- which browser specs ran;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: re-checking on redraw or on a timer, caching
results across hovers, changing the 50 ms scrolled-back refresh, or keeping the flicker for one of the two link kinds.
The user's reproduction fallback (fix anyway when reproduction fails and no better hypothesis exists) is agreed and
needs no further decision.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
