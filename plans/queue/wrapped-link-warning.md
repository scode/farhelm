# Wrapped web links: stop the false mismatch warning

Written against main at ea4a1545 on 2026-10-15. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records the maintainer's decision to change them.

This plan has no dependency on another plan.

## The goal

Hovering a hyperlink a program emitted (OSC 8) shows a loud warning when the link's underlined text looks like a web
address but names a different place than the link goes. Today only the hovered row's part of the text is compared, so an
honest link whose text spans several rows gets the warning anyway. The maintainer sees this in agent TUIs, where it is
"obvious to me the human that the link continues". Fix it: an honest multi-row link keeps the quiet display, a lookalike
still warns, and nothing is judged a match from a piece of a link that continues on another row.

The TODO.md Near term entry this plan covers is "Assess fixing the link warning for links that wrap." (its text mentions
`plans/queue/terminal-link-safety.md` as in flight; that plan has since landed). The maintainer chose to fix it rather
than only assess it, by the approach below.

### Acceptance criteria

- Hovering any row of an honest OSC 8 link whose URL-like text spans several rows shows the quiet display, for all three
  layouts under "What the agents emit" below: Claude's (closed at the row edge, reopened after indentation with the same
  `id`), Codex's (held open across a cursor move past the indentation, no `id`), and a line the terminal itself wrapped.
- A lookalike split across rows still warns when either row is hovered, including the split-host case today's row-only
  check misses (`https://git` on one row, `hub.com/login` on the next, target `https://hub.com/login`: today hovering
  the second row is quiet).
- When the link's tag cannot be read (the private field is gone after an xterm upgrade), behavior is exactly today's
  row-only comparison, never quieter.
- When the link continues onto an adjacent row but the join rule below cannot be satisfied (a bound is hit, a row holds
  more than one run of the link), the display warns whenever the hovered piece looks like a web address.
- A browser test against the real vendored xterm bundle pins the private field: an xterm upgrade that renames or drops
  it fails that test.
- SPEC.md's and SPEC_impl.md's accounts of the accepted false alarm are rewritten to the new behavior and its honest
  limits.
- The TODO entry is removed, and a new Near term TODO entry (below) asks the maintainer to consider an upstream fix.

## Requirement sources

**The maintainer's decisions (2026-10-15):**

- Approach: read xterm's per-cell link tag, `extended.urlId`, from the cell object the documented
  `term.buffer.active.getLine(y).getCell(x)` call returns, join the hovered link's tagged cells across rows, and compare
  the full text with the target. Fall back to today's per-row behavior when the field is absent. This deliberately
  overrides the 2026-09-30 decision (recorded in SPEC_impl.md next to `linkTextMismatch`) that accepted the false alarm
  because joining rows "would need xterm's private internals"; the maintainer asked what the caveats were and chose this
  after hearing them (an undocumented field that can only break at a manual xterm upgrade, caught by a test and failing
  safe; the reversal of a recorded decision; a tight join rule; untagged decoy text is no new hole). You are authorized
  to rewrite that reasoning in SPEC_impl.md and the matching sentence in SPEC.md.
- The bundle file itself is not edited. SPEC_impl.md's rule that the vendored xterm stays byte-identical (decided
  2026-10-10) stands; this plan only reads a field of an object the public API hands out.
- No library upgrade in this plan. The xterm.js 6.1.0 betas join only rows the terminal itself wrapped, which agents
  never produce.
- Upstream: you do not file anything with xterm.js. Instead the plan adds a Near term TODO entry for the maintainer to
  consider an upstream fix, "with enough detail so there is no confusion about what it refers to" (the maintainer's
  words). Content is specified under Outline below.
- A narrowed TODO entry for leftovers only if something is actually left unfixed; with this approach none is expected.
- Review gate: a fresh-context Opus 5.5 agent at high effort with the general charter (the default, taken when the
  maintainer left the question unanswered). No-workhorse mode, which `plans/AGENTS.md` requires. No GUI mockup: nothing
  visible is designed; the warning just stops firing falsely.

**Binding repository constraints:** SPEC_impl.md's vendoring rules for xterm.js (the "Terminal widget: xterm.js island"
section); `plans/AGENTS.md`; root `AGENTS.md` "TODO.md and FLAKES.md" (the maintainer authorized the one new entry
above), "Changelog fragments", and "Tests and validation"; `.agents/test-authoring.md` for the test changes.

**Planner proposals** (from the planning session and its independent review; change them if they prove wrong, and log
the DECISION): the three-state helper, the join rule details, the bounds, leaving file links alone, and the test shapes
below.

## What the agents emit (captured during planning)

Captured 2026-10-15 with `scripts/agent-tui.py` per `.agents/real-agent-tui.md`, at 100 columns, recording raw pane
bytes. Farhelm's supervisor reads tmux in control mode, which passes pane bytes verbatim, so these are the bytes the
browser's xterm receives. Claude Code 2.1.301 (`claude --permission-mode auto`) and Codex 0.163.0
(`codex -a on-request -s workspace-write`) were each asked to print a long bare URL, a markdown link, and a markdown
link whose text is its own URL.

- Both turn bare URLs and markdown links into OSC 8 links, break long ones themselves, and indent the continuation row
  by 5 spaces. Neither lets the terminal wrap, so `isWrapped` is never set on the continuation row.
- Claude breaks at the exact column edge. It closes the link at the row end, writes the indentation unlinked, and
  reopens the link with the same `id=` parameter and the same URI (`ESC ] 8 ; id=1vkvujc ; https://...`, the id as
  captured). Per the OSC 8 spec, cells with the same nonempty id and the same URI are one link.
- Codex opens the link without an id and keeps it open while it moves the cursor with `CSI <row> ; <col> H` past the
  indentation and continues printing. It breaks at `-` or `/`, so its first row ends short of the edge. While streaming,
  it also redraws parts of a link with the link open, and an unmatched markdown link can end up with a target that
  includes `](...)`.
- Do not re-capture: the fix depends only on xterm's cell tag, not on either agent's layout. Use these shapes as
  synthetic bytes in tests.

## How xterm tags link cells (verified in the vendored 6.0.0 bundle)

- `BufferLineApiView.getCell(x, cell?)` calls `loadCell(x, cell || new CellData)` and returns xterm's internal
  `CellData`. Its `extended` object always exists; `extended.urlId` is a positive number on a cell inside an OSC 8 link
  and 0 otherwise. The name is unmangled in the minified bundle and identical in 6.1.0-beta.315. It is not declared in
  `typings/xterm.d.ts`.
- `OscLinkService` keys a link with an id by `id;;uri`: a live entry with the same key reuses its urlId, so Claude's
  reopened pieces share one urlId. A link without an id gets a fresh urlId (`_nextId++`, never reused), and a link held
  open across cursor moves keeps tagging every printed cell with it, so Codex's pieces share one too. Erased cells
  (indentation written by EL/ED) carry no tag. A wide character's spacer cell carries the tag.
- xterm's OSC 8 link provider still reports one link per contiguous run per row; the hover range it passes is the
  hovered row's run (6.1.0-beta.315 may pass a multi-row range for terminal-wrapped links; do not assume one row).

## Outline

### The helper (`crates/farhelm-ui/assets/terminal-links.js`)

Replace what `linkRowText(term, range)` feeds into the web-link hover with a helper that returns one of three results:

- **Whole**: the link's full text, joined across rows (an ordinary one-row link is also Whole). It goes to the existing
  `linkTextMismatch(text, uri)` unchanged.
- **Fragment**: the hovered link continues onto an adjacent row (adjacent cells carry the same tag) but the join could
  not be completed. The display warns if `claimsAddress(fragment)`; it is never treated as a match. "Fragment" must only
  arise when an adjacent row carries the tag, so that the same link printed twice on one row with the same id does not
  get a new false alarm.
- **Unknown**: the tag cannot be read (the hovered cell's `extended?.urlId` is not a positive number). Today's row-only
  comparison applies unchanged.

The join:

- Read the tag from a cell inside the hover range. Pass one reusable cell object to `getCell(x, cell)`; `loadCell`
  assigns the buffer's own `ExtendedAttrs` to `cell.extended`, so only ever read it, never write it. Do not use
  `hasExtendedAttrs()`: it is another undeclared name, and `extended.urlId` alone is enough.
- Scan both directions from the range's first and last rows, row by row. A row joins when it holds exactly one
  contiguous run of cells with that tag; stop at the first row without the tag. A row with more than one run, or hitting
  a bound, yields Fragment. Reaching the first row of the buffer while still tagged is fine (the trimmed head is not
  visible to the reader either).
- Bounds: reuse `linkRangeText`'s 128 rows and 8192 characters.
- Read each run with `translateToString(false, from, to)` and trim only the ends of the joined string; trimming each run
  would drop a real space at a row edge.

### Call sites (`crates/farhelm-ui/assets/terminal.js`, the `linkHandler.hover` around line 3602)

- The web-link hover (`links.showLinkTarget(...)`) gets the new result.
- File links keep `links.linkRowText(term, range)`: in `terminal-files.js` `hover(...)` that text only identifies a
  hover so it can be reclaimed across redraws, and nothing is judged from it.

### Honest limits to document

Joining makes some split-host lookalikes quiet that warn today only by accident of the split: `https://good.example` on
one row and `.evil.test/` on the next, target `https://good.example.evil.test/`, is quiet, exactly as the same text on
one row already is (`terminal-links.test.js` has the one-row pair). That is no new capability, and the quiet display
still bolds the real host. Do not add a positional guard for it (Codex breaks before the edge, so position proves
nothing). The SPEC_impl.md rewrite must say that the link's tagged cells, read row by row, spell the target. It must not
claim that "what the reader sees" matches the target: untagged decoy text around the link is outside the comparison, as
it already is within one row.

### Specs

- SPEC_impl.md, the paragraph beginning "An OSC 8 link's hover compares the link's underlined text": replace the
  accepted-false-alarm reasoning with the mechanism (the tag, the join, the three results), why reading an undocumented
  field is accepted (decided 2026-10-15: read-only, can only break at a manual upgrade, pinned by a browser test, fails
  to today's behavior), and the honest limits above. Keep the rest of the paragraph (the URL-like gate, strict parsing,
  visible format characters). Also correct its claim that "a fragment can never equal the full target", which the
  `https://git` / `hub.com/login` split already disproves today.
- SPEC.md, Terminal experience, the sentence "Only the part of a link on the hovered line is compared, so a long web
  address that wraps onto another line gets the warning even when it is honest; that false alarm is accepted (see
  SPEC_impl.md)." and the warning-is-an-aid sentence after it: rewrite to the new product behavior.
- Update the `linkTextMismatch` and `linkRowText` doc comments in `terminal-links.js` to match.

### Tests

- Unit tests (`crates/farhelm-ui/js-tests/terminal-links.test.js`, fake terminal whose cells expose `extended.urlId`):
  short first row with an indented continuation (one case covers Claude and Codex at this level), a terminal-wrapped
  link, hovering either row, two runs of the same tag on a continuation row (Fragment), the missing field (Unknown,
  today's result), a lookalike split across rows that still warns, the `https://git` / `hub.com/login` case hovered on
  the second row (now warns), and the row bound.
- Browser test (`e2e/tests/terminal-links.spec.ts`, real vendored bundle): write the Claude shape (closed at the edge,
  indentation, reopened with the same `id`) and the Codex shape (no id, held open across `CSI row;col H`) as synthetic
  bytes, hover each row, and assert the quiet display; add a split lookalike that warns. This is the test that pins the
  private field.
- Run `python -B scripts/check-test-sleeps.py` per root `AGENTS.md` (Tests and validation).

### TODO.md

The last PR removes the Near term entry "Assess fixing the link warning for links that wrap." and adds a Near term entry
asking the maintainer to consider an upstream xterm.js fix. That entry must stand on its own months from now. It should
cover:

- What Farhelm now does: it reads the undocumented `extended.urlId` field from the cell `getCell` returns, to join an
  OSC 8 link's rows for the hover mismatch warning. Name the file and function, and the SPEC_impl.md paragraph.
- Why upstream would help: a public API would retire the private read and its pinning test.
- The concrete ask, either of:
  - extend the OSC 8 link provider's row joining (`_getRangeWithLineWrap` in the 6.1.0 betas, which joins only while
    `isWrapped` holds and the urlId matches) to consecutive rows that carry the same urlId without being wrapped, which
    is exactly what agent TUIs emit (the Claude and Codex shapes above);
  - or expose a cell's link id, or a link's full extent, in the public buffer API.
- That nothing has been filed upstream, and that filing is the maintainer's call.
- That Farhelm vendors xterm.js unmodified, so any upstream fix arrives only with a stable release.

No narrowed leftover entry unless something is actually left unfixed; if something is, write one and explain it in the
report.

### PRs

Expected: one `fix` PR (helper, call site, tests, specs, changelog fragment, TODO changes), or two if separating the
spec rewrite or the browser test makes review clearer. Add a changelog fragment under `releasing/changelog.d/` per root
`AGENTS.md` (Changelog fragments): users of agent TUIs no longer see the loud mismatch warning on honest links that wrap
onto another line; a lookalike still warns.

### Validation

Read `.agents/validation.md` and `.agents/test-evidence.md` first, and run tests through
`python3 scripts/record-test-run.py`. Expected: the JS unit harness (`cd crates/farhelm-ui/js-tests && node --test`),
the browser test file `e2e/tests/terminal-links.spec.ts` narrowly per `.agents/narrow-tests.md`, `dprint check` on the
changed files, and the test-sleep checker. Not the whole battery.

## Fallbacks

- If reading the tag turns out not to work as described above (the field is not on the object `getCell` returns, or
  pieces of one link do not share a urlId for one of the two captured shapes), that is a block: the maintainer chose
  this approach on that basis. Write what you found, with the evidence, as the question.
- If the browser harness cannot load the real bundle in a way that pins the field, a unit test that loads the vendored
  `xterm.js` file directly in node and checks the field is an acceptable substitute; log the DECISION.
- If the join changes behavior for file links' hover after all, keep file links on today's row text and log it.

Decisions to log: the helper's shape and name, the bounds, how the three results map to the display, the PR split, and
every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-wrapped-link-warning-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs)
rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on, or
that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
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
  `plan/wrapped-link-warning/<nn>-<short-name>` (for example `plan/wrapped-link-warning/01-join-link-rows`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits; the fix is `fix`, with its changelog fragment in the same
  commit.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- The plan's last code PR removes the TODO entry it covers and adds the upstream entry described above.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on Opus 5.5
at high effort (a native sub-agent where the executing harness serves that model; otherwise shelled out, and the
harness-shellout skill owns the launch mechanics). The prompt carries the full charter, because the reviewer has nothing
else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the maintainer's decisions, "What the agents emit", "How xterm tags link
  cells", and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above (for example a parser hook that follows OSC 8
  sequences itself, a library upgrade, or a positional heuristic; these are examples, not a blacklist);
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
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are
under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for
what cannot proceed without the maintainer.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes the covered TODO entry and adds the upstream entry. Open, not merged. If
a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
