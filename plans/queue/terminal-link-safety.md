# Terminal links: act only on a real click, on the target the hover honestly shows

Written against main at 48c4cc83 on 2026-10-15. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where an outcome below tells you to change them.

This plan has no dependency on another plan.

## The goal

Clicking a link in a session's terminal opens a web page, or for a printed or program-emitted file path, transfers that
file from the session's host to the user's machine; in the desktop app a Cmd-click (Ctrl-click off macOS) also opens a
`.md` or `.txt` save in its default app. Before the click, hovering a file path shows the host, resolved path and size.
SPEC.md treats that click, made after that disclosure, as the user's authorization. Four review findings show ways the
authorization is weaker than it looks:

- A drag that starts and ends inside one link counts as a click when the program in the terminal has mouse reporting on
  (vim, less, agent TUIs), because the only drag guard is "is text now selected?" and mouse reporting suppresses local
  selection. A web URL opens, a file downloads, and with the modifier held an admitted save opens.
- When a program repaints only part of a wrapped link, the bundled terminal library can keep the old link target, so a
  click acts on a file the screen no longer shows.
- A long path in a short window clips the hover (folder, size, action), and the hover cannot be scrolled, yet the click
  still transfers.
- The hover's escaping covers only Unicode's format category, not Farhelm's shared list of presentation-unsafe
  characters, and it leaves literal `<U+…>` text unescaped while repeated spaces collapse, so two different paths can
  look identical.

Carry out these four triage outcomes, each recorded under its own heading in `TRIAGE_OUTCOMES.md` (read all four in full
first), in this order:

1. `mouse-reporting-drag-activates-file-link.md` (`fix code`, complexity gated)
2. `wrapped-file-link-partial-redraw-keeps-old-target.md` (`fix code`, complexity gated)
3. `file-disclosure-cannot-scroll-oversized-path.md` (`fix code`, complexity gated)
4. `file-disclosure-misses-invisible-path-characters.md` (`fix code`, not gated)

### The complexity gate (outcomes 1 to 3)

The maintainer, on 2026-10-15: "not a hard line but all of these are nice to fix if it's not terribly complicated,
ESPECIALLY the 'dragging across a link' that seems super POLA violating", then "fix code, with complexity gate. can't
turn large/very complex." Each of the first three fixes must stay small: confined to the existing link activation and
hover code (`crates/farhelm-ui/assets/terminal.js`, `terminal-files.js`, `terminal-links.js`, `app.css`) plus its tests,
with no patch to the vendored terminal library, no new helm API, and no new persisted state. Whether a fix meets the
gate is yours to judge and log.

If one of them cannot be built within the gate, do not build it. Build and push the others, then block (per
`plans/AGENTS.md`, Executing one plan, step 10) with a question that describes the larger fix that would be needed and
asks whether to build it or discard the outcome. The ledger entries say a failed gate returns the item to the user, so
this is a block, not a discard you decide yourself.

### Acceptance criteria

- Outcome 1: a primary-button drag across any terminal link (web URL from the URL matcher, program-emitted OSC 8 link,
  printed file path), with or without mouse reporting, neither opens a URL nor transfers or opens a file. A plain click
  still does, including a click right after selecting text. The existing selection guard stays.
- Outcome 2: a click whose link's on-screen text has changed since the hover was shown transfers and opens nothing; an
  unchanged link still works. The library's own invalidation bug stays an upstream matter under SPEC_impl.md's vendor
  policy.
- Outcome 3: a click on a file link whose hover does not fit the window transfers and opens nothing and tells the user
  why (for example, that the window is too small to show the full path); a hover that fits behaves as before.
- Outcome 4: the file-link hover and the transfer outcome messages show every character
  `farhelm_proto::text::is_presentation_unsafe` marks unsafe, escape literal `<U+…>` text so it cannot pass for an
  escape, and keep repeated spaces visible, so distinct paths never look alike.
- Each built fix has regression coverage: a JS unit test where the logic is a pure helper
  (`crates/farhelm-ui/js-tests/`), and for outcome 1 a browser test that drags across a link while a real
  mouse-reporting program runs, in both engines. Reuse the existing mouse-mode fixture in
  `e2e/tests/terminal-link-clicks.spec.ts` (`farhelm-fixtures fake-agent --script mouse-modes`, waiting for
  `mouseTrackingMode`) rather than a shell left in mouse mode, which would receive the reported mouse bytes as input.
- Each fix PR carries a changelog fragment (`kind: fixed`) per root `AGENTS.md`.
- If a SPEC.md sentence about link clicks or file disclosure would read as allowing the old behavior, correct it in the
  same PR; do not otherwise rewrite the link sections.

## Requirement sources

**The user's decisions (2026-10-15):** the explicit click is "not a hard line", but these are worth fixing within the
gate above, the drag case especially. Outcome 4 was chosen as plain `fix code` after the maintainer was told it is
small. Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm. No-workhorse mode, which
`plans/AGENTS.md` requires. Grouping today's sixteen triage outcomes into ten plans, these four together because they
edit the same hover and click code, was agreed with the maintainer.

**Binding repository constraints:** SPEC.md's terminal link rules (links open on click; the web-link press is withheld
from programs with mouse reporting; file links keep their existing gesture behavior) and its file-path rules (the hover
shows host, resolved path and size before the click can download; the click authorizes the transfer; only an explicit
modifier-click opens a `.md`/`.txt` save). SPEC.md's rule that host-supplied text outside a terminal shows invisible and
direction-changing characters visibly. SPEC_impl.md's vendor policy: upgrade the terminal library rather than work
around its non-security defects inside it. Root `AGENTS.md` "Execute triage outcomes" and its changelog rule.

**Planner proposals:** the press-position drag check shared by all three activation paths; the click-time recheck of the
link text; refusing the click while the hover is clipped rather than making the hover scrollable. The maintainer saw
these as stated planner choices and did not veto them. They also saw "the helm sends an already-escaped path" for
outcome 4; the planning scope review then showed that several surfaces never receive a helm reply, so the plan now
extends the browser's single escaping helper instead. The goal the maintainer agreed to (one list, every surface) is
unchanged.

## Outline

All three activation paths currently guard only with `term.getSelection()`: the OSC 8 `linkHandler.activate` and the
URL-matcher callback in `terminal.js`, and `FileLinksAddon.download` in `terminal-files.js`. The hover is
`FileLinksAddon.show`/`hover`; its CSS is the `.terminal-file-target` rules in `app.css`, which cap the height and set
`pointer-events: none`.

- PR 1 (drag): record where the primary press landed (the file already observes mousedown/mousemove/mouseup for link
  press withholding and copy-on-select), and have every activation path ignore a release more than a few pixels from the
  press. Verify which event xterm passes to `activate` (it should be the mouseup) before relying on its coordinates. Put
  the comparison in a pure helper in `terminal-links.js` with a unit test.
- PR 2 (stale target): at activation, re-read the text under the link's current range (the OSC hover already captures
  displayed text through `linkRowText`; printed paths come from `bufferLinks`) and refuse when it differs from what the
  hover recorded. A browser test needs a program that prints a wrapped link and repaints half of it; if building that
  fixture is the expensive part, a unit test of the comparison plus a note in the report is acceptable. Log that
  substitution as a gate DECISION, so a reviewer does not read it as dodging the gate.
- PR 3 (clipped hover): in `download`, check the live hover element (`scrollHeight > clientHeight`) and refuse with a
  message when it overflows. Checking at click time needs no stored flag and handles a resize between hover and click.
  Only vertical clipping can happen: the hover wraps long lines and `show` clamps its position into the window.
- PR 4 (invisible characters): every file-link surface (the hover's lines, including the `checking…` and error lines,
  and the `downloaded …` and `saved to …` messages) already goes through one helper, `visibleFormatCharacters` in
  `terminal-links.js`, which escapes `\p{Cf}` only. Extend that helper to escape every character in the shared list
  (`farhelm_proto::text::is_presentation_unsafe`, documented in `crates/farhelm-proto/src/text.rs`) in addition to
  `\p{Cf}`, and to escape the `<` of literal `<U+` text. Keep repeated spaces with `white-space: pre-wrap` on the hover
  and message lines rather than by escaping spaces. Add no helm field. A JS unit test covers each family in the list;
  point the helper's comment and `text.rs`'s doc comment at each other so a change to one prompts the other. A
  cross-language test that compares the two lists is welcome only if it is cheap.

### Validation

Per root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` if
any Rust changes, `cd crates/farhelm-ui/js-tests && node --test`, the changed browser specs (likely
`e2e/tests/terminal-link-clicks.spec.ts`, `terminal-links.spec.ts` and the file-link specs) on Chromium and WebKit
through the recorder after the documented builds, the test-sleep checker when a browser test changes, `dprint check` on
changed Markdown, and `python3 releasing/check-changelog.py format`.

## Fallbacks

- The complexity gate above, for outcomes 1 to 3.
- Outcome 4: one escaping mechanism, never two. The browser-side helper above is the planned one. Having the helm send
  display strings for every path the hover and messages render is the alternative, if the browser approach proves
  inadequate; then the helper keeps its current behavior for those surfaces. Log which you chose.

Examples of departures that need the scope reassessment: patching the vendored terminal library, making the hover
interactive or scrollable, a confirmation step before downloads, or a new endpoint.

Decisions to log: for each gated outcome, whether the gate was met and why; the drag threshold; where the escaping
happens.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-terminal-link-safety-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md`
(Executing one plan, step 7) before any work.

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

- Follow root `AGENTS.md` "Execute triage outcomes": one reviewable commit, stable bookmark and draft PR per outcome, in
  a single linear stack in the order given under The goal above. Each PR carries its outcome's agreed change, the
  removal of its feedback file and `review_feedback_queue/INDEX.md` entry, and its `TRIAGE_OUTCOMES.md` Execution
  update. Record the jj change ID and bookmark in that Execution field before creating the PR, then add the PR's URL
  (with `/changes` appended, per the user's PR-link rule) to the same change and push it again; no separate bookkeeping
  PR. Replace the Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/terminal-link-safety/<nn>-<short-name>` (for example `plan/terminal-link-safety/01-drag-click`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits. Type `fix` for each built fix, with its changelog fragment
  (`kind: fixed`) in the same commit, per root `AGENTS.md`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
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
- the acceptance criteria: this file's goal, the user's decisions, the outline, and the outcome's full
  `TRIAGE_OUTCOMES.md` entry.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

PRs in this stack that only remove feedback files and update the ledger, and change nothing else, may share one review
run that covers all of them together, instead of one run each.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above. If current code or specs invalidate a recorded triage decision, that is a block: root `AGENTS.md`
(Execute triage outcomes) returns such an item to the user rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outcome listed under The goal unless the
acceptance criteria above say otherwise, that together meets the acceptance criteria above, each PR passed its review
gate, and each outcome's `TRIAGE_OUTCOMES.md` Execution field names its change ID, bookmark and PR. Open, not merged. If
a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
