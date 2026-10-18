# Terminal links: refuse a click whose hover an approval card covers

Written against main at c709436a on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Execute the triage outcome recorded in `TRIAGE_OUTCOMES.md` under the heading
`approval-cards-can-obscure-file-disclosure.md` (outcome `fix spec+code`, revised the same day to include web links),
per `review_feedback_queue/.agents/execute-outcomes.md` ("Execute triage outcomes"). Read that ledger entry and the
feedback file `review_feedback_queue/approval-cards-can-obscure-file-disclosure.md` in full before starting; the entry's
Decision and Completion criteria are binding.

In product terms: hovering a link in a terminal shows a small hover card naming its real destination (for a file link,
the file's name, host, folder and size; for a web link, the URL, with a warning when the link's text claims a different
address). The spec treats that hover as the safeguard against program output that labels a link as one thing while it
points at another, and a file link already refuses a click while its hover does not fit in the window. Approval cards
(an agent asking for permission) float over the middle of the terminal and are drawn above the hover, so a link just
beside a card can be clicked while the card hides part or all of its hover. The fix refuses such clicks, for file links
and for web links that show a hover alike, and those web links also gain the fits-in-window check file links already
have.

### Acceptance criteria

- A file-link click or modifier-click (Cmd-click on macOS, Ctrl-click elsewhere, in the desktop app) whose hover
  overlaps a visible approval card is refused: no transfer starts, and a one-line explanation appears in the existing
  status line at the top of that terminal, in the spirit of "cannot download: file details are covered by an approval
  request; move the pointer and hover again". The explanation itself is visible above the cards (today the refusal is
  raised only to layer 41, below the cards' 45).
- A file link whose hover is fully visible still downloads (and opens, where it did before).
- Every web-link open whose link shows a hover requires that hover to be fully visible: inside the window and not
  overlapping the visible approval cards. That set is every program-emitted (OSC 8) web link, for plain clicks and the
  desktop app's Cmd-click tunnel opens alike, and a plain URL printed as text only when it shows the remote-localhost
  tunnel hint (when the tunnel hint applies at click time, which also covers a tunnel that became available after the
  hover). Within that set the check fails closed: a missing hover refuses. A refused open opens nothing and shows a
  one-line explanation in the same spirit ("cannot open: ..."). Plain URLs that show no hover open exactly as before
  (SPEC.md: "Plain URLs printed as text need no such display").
- A web link whose hover is fully visible still opens or tunnels as before.
- SPEC.md states the principle for file links and for web links that show a hover: the hover counts as disclosing a
  link's target only when it is fully visible, inside the window and not covered by another Farhelm surface such as an
  approval card; a click whose hover is not fully visible is refused with an explanation. It keeps the existing rule
  that plain URLs without a hover need none. Place it beside the existing file-link rule ("A click cannot transfer or
  open a file while the full hover disclosure does not fit in the window", in "Terminal experience") and the web-link
  hover rule in the same section.
- Browser regressions, run through the recorder on Chromium and WebKit, cover: a file link whose hover overlaps an
  approval card is refused on both a plain click and a Ctrl/Cmd-click and starts no transfer; a fully visible one still
  downloads; an OSC 8 web link whose hover overlaps a card is refused and opens nothing; an OSC 8 web link whose hover
  does not fit in the window is refused; a fully visible one still opens; a plain URL with no hover still opens beside a
  card. Native opening itself cannot be exercised in Chromium or WebKit; the report says so.
- The PR updates the ledger entry's Execution field (change id, bookmark, then PR URL in the same change) and removes
  the feedback file and its `review_feedback_queue/INDEX.md` line.

## Requirement sources

**The user's decisions (2026-10-17, recorded in `TRIAGE_OUTCOMES.md`):**

- Outcome `fix spec+code`: "Refuse when covered, and say so in the spec". Then, when shown the adjacent web-link gap:
  "same fix for web links".
- Complexity gate (from the ledger): an intersection test between the hover and the visible approval cards in the
  existing admission checks, layering the refusal message above the cards, and for web links the same shape plus the
  fits-in-window check. If the fix needs general occlusion detection, relayering the hover itself above the cards, or
  changes outside the file-link and web-link admission paths, stop and block (see Fallbacks).
- The refusal reuses the existing one-line status at the top of the terminal; text only, no new visual design. Final
  wording is the executor's, in the spirit of the examples above.
- Every web-link open is covered, including Cmd-click tunnel opens, accepting that a web link whose hover is cut off by
  a small window now refuses to open where it opened before. Clarified after the planning review showed that plain URLs
  printed as text have no hover by design: the rule covers "links that show a hover" (program-emitted hyperlinks, plain
  and tunnel opens, plus plain URLs only when they show the tunnel hint); plain URLs with no hover open as before. The
  maintainer declined adding a hover to every plain URL.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:**

- `review_feedback_queue/.agents/execute-outcomes.md`: one PR for this outcome, never split across PRs; the PR carries
  the spec change, the code change, the ledger Execution update, and the queue file and index removal.
- SPEC.md "Terminal experience" (file links: refusal while the hover does not fit; web links: the hover is the safeguard
  against a mismatched label). The layer list in `crates/farhelm-ui/assets/app.css`'s comment, where approval cards sit
  at 45 and the hover at 40; keep it accurate if the refusal message's layer changes.
- Root `AGENTS.md`: Tests and validation, Changelog fragments (a `fix` PR adds one), Conventional Commits.

**Planner proposals (checked by a fresh-context planning review):**

- Detect coverage by intersecting the hover's bounding rectangle with the rectangle of the approval-card region
  (`.approval-cards`), read at click time like the existing fit check. Use the region, not each `.approval-card`: the
  region scrolls, and a card scrolled out of its view still reports a rectangle over terminal area that nothing covers.
  An empty region has zero height and covers nothing. Any overlap counts as covered; a partial cover can hide exactly
  the part that matters. This is narrower than general occlusion detection (no `elementsFromPoint` sampling), which the
  gate excludes.
- Extend the existing pure check `disclosureFits` in `crates/farhelm-ui/assets/terminal-files.js` with the covering
  rectangles as an input, and extend its unit test in `crates/farhelm-ui/js-tests/terminal-files.test.js`. Supply the
  rectangles through one getter injected from `terminal.js` (the way the click-gesture check already is), so knowledge
  of the shell's approval cards stays out of `terminal-files.js`. `terminal.js` already requires
  `window.farhelmTerminalFiles` and calls into it, so the web path calls the same `disclosureFits`; no duplicate.
- Put the web-link check at the `terminal.js` call site by wrapping the open and tunnel callbacks its `activateWebLink`
  wrapper already passes. Those run only after `activateWebLink`'s own ignore rules (no click, a selection, program
  mode, an invalid URL), so no refusal appears for a click that would have been ignored anyway, and `terminal-links.js`
  and its tunnel unit test stay unchanged.
- Show the web-link refusal on the file links' status line: the file-links addon is built in the same closure before the
  `activateWebLink` wrapper, so it is directly reachable.
- The existing message machinery needs three things the web path must share rather than re-implement:
  - The refusal is shown in the next task, because activation happens on mouseup and the addon's click listener would
    otherwise dismiss it at once; today that deferral is inline in `download()`. Factor it into one addon method both
    paths call.
  - The raised refusal drops back to its ordinary layer only in the addon's `clearHover`, on a file hover's leave. A web
    hover's leave (`hideLinkTarget`) must drop it too, or a raised web refusal would stay above later hovers and the
    cards until the next terminal click.
  - The raised layer moves from 41 to a value from 46 to 49: above the cards (45) and below tooltips (50). Update both
    CSS comments that describe the layers (the layer ladder and the `.terminal-file-message` comment).

## Outline

One `fix` PR (bookmark `plan/covered-link-hover-refusal/01-covered-hover`): the coverage input to the shared fit check,
its use in the file-link admission (`download()` in `terminal-files.js`) and the web-link admission (the wrapped
callbacks at the `activateWebLink` call site in `terminal.js`), the shared deferred refusal and its layer reset on a web
hover's leave, the refusal messages, raising the refusal message above the cards (`app.css`), the SPEC.md sentences, JS
unit tests for the pure check, the browser regressions (extend `e2e/tests/terminal-link-clicks.spec.ts`, staging an
approval card the way `e2e/tests/approval-layout.spec.ts` does with its routed `/api/approvals` feed), a changelog
fragment (`kind: fixed`, describing both refusals and the new small-window refusal for web links), the ledger Execution
update, and the queue file and index removal.

### Validation

Choose per `.agents/validation.md`; at least: `dprint check` on changed Markdown,
`cd crates/farhelm-ui/js-tests && node --test`, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`,
`python3 releasing/check-changelog.py format`, `scripts/check-desktop-assets.sh` if an asset file is added, and on
Chromium and WebKit through the recorder: the new tests plus `terminal-link-clicks.spec.ts`, `terminal-links.spec.ts`
and `approval-layout.spec.ts`. Not the full suite. If Rust changes, also `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` and `cargo check -p farhelm-ui --features desktop`. The report says the
desktop app's own webview was not exercised beyond WebKit standing in for it.

## Fallbacks

- The complexity gate above. When it trips (general occlusion detection, relayering the hover above the cards, adding a
  hover to plain URLs, or changes outside the two admission paths and the message machinery they share), push what is
  consistent and block per `plans/.agents/executing.md` (Executing one plan, step 10), with the concrete tradeoff as the
  question.

Examples of departures that need the scope reassessment: sampling with `elementsFromPoint`, a generic "covered by any
element" detector, moving approval cards or the hover to new layers, new message surfaces, changing `activateWebLink`
itself.

Decisions to log: the final refusal wording, the raised layer's value, and anything the planner proposals did not
settle.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-covered-link-hover-refusal-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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

- This outcome is exactly one reviewable commit, stable bookmark and draft PR, using the `jjstack` skill
  (`review_feedback_queue/.agents/execute-outcomes.md` forbids splitting a `fix spec+code` outcome across PRs).
- The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per `plans/.agents/executing.md`
  (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/covered-link-hover-refusal/01-covered-hover`.
- Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with a changelog fragment per root `AGENTS.md` (Changelog
  fragments).
- Record the change id and bookmark in the ledger entry's Execution field before creating the PR, then add the PR URL in
  the same change and update the PR (`review_feedback_queue/.agents/execute-outcomes.md`). `TRIAGE_OUTCOMES.md` and
  `review_feedback_queue/` are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing the PR, launch a review of its changes yourself and address what it finds before moving on. The user
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

The PR changes tests, so include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

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
Wording in this file that turns out inaccurate (a line number, a file name, a selector) is corrected to the truth; the
plan-specific agreed fallbacks are under Fallbacks above.

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

The plan is complete when the single open draft PR exists that meets the acceptance criteria above, it passed its review
gate, and it carries the ledger Execution update and the queue file and index removal. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a
closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
