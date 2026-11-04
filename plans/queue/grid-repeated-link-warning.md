# Grid terminals: no false link warning when a program restarts the same hyperlink

Written against main at 856099b8 on 2026-11-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **A hyperlink a program starts twice shows a false mismatch warning in grid terminals.** When a program starts the
> same OSC 8 hyperlink twice in a row without naming an id, hovering the second piece shows the loud "Link text does not
> match where it goes" warning, with "Text says:" holding only the tail of the link's text, and the hover underline
> covers only that piece. Seen on a real agent's markdown link list (it showed `ews.com/article/…` for a link to
> `https://apnews.com/article/…`); it healed once the agent redrew the line in one go, which is why it comes and goes.
> Reproduce in any terminal with
> `printf '\e]8;;https://apnews.com/x\e\\https://apn\e]8;;https://apnews.com/x\e\\ews.com/x\e]8;;\e\\\n'` and hover
> `ews.com/x`. Cause: each OSC 8 start without an id gets a fresh id in the emulator
> (`vendor/alacritty_terminal/src/term/cell.rs`, `HyperlinkInner::new`, ids ending in `_alacritty`); the supervisor
> sends that id with the target as the link's identity (`crates/farhelm-supervisor/src/grid_output.rs`, `link_key`); the
> grid joins neighbouring runs into one link only when id and target both match
> (`crates/farhelm-ui/assets/grid-buffer.js`, `rowLinks` and `sameLink`, and `taggedLines` across rows), so the row
> holds two links; the hover then checks the text of the one under the pointer (`crates/farhelm-ui/assets/terminal.js`,
> `oscLink`, `found.ranges[0]` passed to `linkHoverText`). Old (tmux, xterm.js) terminals show the same bug (the repro
> above, 2026-11-03); fix only the grid path. Likely fix: treat neighbouring runs with the same target and no id from
> the program as one link, for the underline and for the text the mismatch check reads, across wrapped rows too. A link
> to a different target stays separate, so a disguised link still warns. Test with the repro bytes in the grid-client
> browser specs and the buffer's unit tests.

### What is known

From a read-only research pass over main (line numbers approximate; main has since moved only by TODO.md edits):

- **The emulator** mints `<n>_alacritty` from a process-wide counter for every OSC 8 start without an id (`cell.rs`,
  `HyperlinkInner::new` around 279-292), so the repro gives two links with one target and ids such as `0_alacritty` and
  `1_alacritty`. Snapshot restore keeps the spelling (`crates/farhelm-terminal-emulator/src/snapshot/mod.rs` around
  108-114), and the snapshot test oracle relies on it (`tests/common/compare.rs` around 196-200).
- **The supervisor**: the id reaches the wire from `row_from_cells` (`grid_output.rs` around 144-190), not `link_key`,
  which only dedupes the link table. It merges cells into one run only when their links are equal, so the repro row is
  two runs. The wire carries `[id, uri]` only (`crates/farhelm-proto/src/grid.rs`, module doc around 128-136, `Link`
  around 906-916); a run is `[col, width, text, style, link]`, built in tests with `wire([[col, text, link]], w)`.
- **The grid client** compares `id` and `uri` in four places in `grid-buffer.js`: `sameLink` (around 286), which
  `rowLinks` (around 300-336) merges by; `taggedLines`' `carries` (around 344-356); `answerLines`' key (around 399-405);
  and the adapter `xtermBuffer`'s `tagOf` (around 567-577, documented around 517-520). `linkHoverText`
  (`terminal-links.js` around 467-531) joins rows by the adapter's tag; with no tagged neighbour it judges the hovered
  range alone, and `ews.com/x` is URL-like, so it warns.
- **Nothing but the spelling** tells a generated id from a program's own. A program can send `id=7_alacritty` itself.

### Acceptance criteria

- **The repro is quiet and whole.** In a grid terminal, hovering either piece of the repro shows the ordinary display,
  the text the check reads is `https://apnews.com/x`, and the hover underline covers both pieces. The same holds for an
  id-less link continued across a soft wrap, with the underline per row.
- **A disguise still warns.** Pieces with different targets, a piece with the program's own id, and id-less pieces that
  are not a direct continuation stay separate and are judged as today.
- **Nothing else moves:** no wire, protocol, supervisor-code or `vendor/` change, the old terminals untouched, and the
  repeated-complete-address rule intact. One hover stays linear in the lines it examines (Outline, step 3).
- **Docs and bookkeeping** as Outline step 4 and the "One PR" planner proposal describe.

## Decisions already made

Settled with the maintainer on 2026-11-03, on the planning page:

- **Terminal hosts only.** In the maintainer's words: "i don't want to think about anything that is irrelevant once we
  nuke tmux/xterm.js". The plan does nothing for the old (tmux, xterm.js) terminals: no xterm.js tests, no spec or doc
  note that the old terminals keep the bug, and no change to the xterm.js path. Where a helper serves both (such as
  `linkHoverText`), its xterm.js behavior stays unchanged.
- **The fix lives in the browser's grid terminal code only**: `crates/farhelm-ui/assets/grid-buffer.js` and its docs,
  with no wire or protocol change. The maintainer asked whether a cleaner solution exists when only terminal hosts
  matter, and accepted this answer. Rejected alternative, not to be reopened: fixing it at the source, by having the
  vendored emulator reuse the previous piece's id. That is not cleaner. It is a vendored patch with upkeep; the emulator
  would have to inspect the cell before the cursor, across wraps, at every link start; and ids are saved in snapshots.
  The page-side rule is small, needs no protocol change, and matches the OSC 8 spec's suggestion that terminals treat
  neighbouring id-less pieces with the same target as one link. (Also ruled out: a supervisor-side id rewrite, which
  covers one row only, and a wire flag, which needs a protocol bump.)
- **Joining is narrow.** An id-less piece joins the piece before it only when it is a direct continuation: adjacent on
  the same row, or across a soft wrap from the previous row's last column into column 0. Not every neighbouring
  same-target id-less piece.
- **An explicit program id stays separate.** A piece the program named with its own id never joins a neighbouring
  id-less piece with the same target.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort for every PR (see Review gate). If it cannot
  be reached when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:**

- SPEC.md, the OSC 8 hover paragraph (around 2314-2331): rows of one hyperlink are compared together, and the warning is
  an aid, not a guarantee.
- SPEC_impl.md, the OSC 8 hover paragraph (around 1671-1704) and "The grid client" (around 1706).
- The "repeated complete addresses" rule in `linkHoverText` (terminal-links.js around 528-530), from
  `TRIAGE_OUTCOMES.md` around 16915-16932. Do not weaken it.
- `review_feedback_queue/AGENTS.md`, "Lifecycle": the fixing PR removes the queue item and its `INDEX.md` line.
- Root `AGENTS.md`: "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md",
  "Frozen material" (no `vendor/` change). Also `.agents/test-authoring.md` and `docs/test-sleep-check.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- **Generated id:** strict `^\d+_alacritty$`, the form `compare.rs` uses. `7_alacritty` sent by a program counts as
  generated; `x_alacritty` or `_alacritty` does not.
- **Underline per row**, as for explicit-id links today: `rowLinks` returns one range per row, and a soft-wrapped chain
  joins its text through the adapter's tags. The in-row underline is what the entry complains about.
- **Why trusting a forgeable spelling is safe**, written once, in SPEC_impl.md beside its `https://good.example` +
  `.evil.test/` precedent, because a future reader will otherwise see "trusts a program-controlled string" and either
  remove it or harden it at great cost. Joining only ever merges runs with equal targets, and a chain (contiguous on a
  row, or continued across a soft wrap into the next row) is a subset of what a shared explicit id already joins today
  (contiguous runs on a row, adjacent rows with one tagged run each). So spelling an id `<n>_alacritty` reaches at most
  what a shared explicit id gives. The compared text becomes exactly the underlined text of the chain, so it is quiet
  only if that text reads the real target, the honest case. A misclassification costs at most an underline spanning two
  touching same-target links, or a false alarm of the kind the existing rules already have; never a quieter display for
  text that names a different place.
- **One PR**, which also does the bookkeeping: removes the TODO.md entry, removes
  `review_feedback_queue/anonymous-hyperlink-restarts-cause-false-warning.md` and its `review_feedback_queue/INDEX.md`
  line, leaves the neighbouring `snapshot-comparison-ignores-link-grouping.md` item alone (a separate test-oracle
  issue), and adds a changelog fragment (`kind: fixed`) that describes the fix without mentioning the old terminals.

## Outline

`plan/grid-repeated-link-warning/01-join-idless-links`, one PR:

1. **One link-identity helper in `grid-buffer.js`**, replacing the four `id`/`uri` comparisons above. An explicit id's
   identity stays its `id` and `uri`. An id-less (generated-id) run takes the identity of the run it continues: the
   previous run on its row when that run ends where this one starts, or, for a run at column 0, the last run of the line
   above when that line soft-wraps (`row.wrapped`) and its last run ends at the line's width, or at width - 1 when
   `padsWideWrap` holds (a wrapped wide character leaves that padding cell, which `row_from_cells` skips). Either way
   the previous run must also have a generated id and the same `uri`; otherwise the run is its own link. The identity
   names the chain's first piece, so tag equality is transitive. Chains walk back at most 128 lines, the bound
   `taggedLines` and `linkHoverText` already use.
2. **`rowLinks`** merges contiguous runs by that identity, so `found.ranges[0]` covers `https://apn` + `ews.com/x`. The
   adapter's tags use it, so `linkHoverText` joins a soft-wrapped chain's rows. `taggedLines` must follow the chain for
   correctness only on chains of three or more rows (for two, the line beyond each end it already adds covers the other
   row); switch it anyway. `answerLines` already covers a soft-wrapped chain through `wrappedSpan`, so its key change is
   for consistency with the new identity. `terminal.js` and `terminal-links.js` should need no change; if one turns out
   to, it is a departure (Fallbacks).
3. **Hover cost stays linear.** `rowLinks` calls `taggedLines` per link, which checks up to 257 lines, and computing a
   walk-back identity for every run on each would make one hover quadratic or worse on a screen of one-cell id-less
   restarts of one target, which any program can print. Remember identities within one `rowLinks`/`taggedLines`/hover
   call (or compute each line's incoming chain key once) so the work stays linear in the lines examined. No new bound is
   needed. Add a unit test on such a screen, large enough that quadratic work would be plainly slow, asserting the
   answer rather than a time (`.agents/test-authoring.md`).
4. **Docs:** the safety argument has one copy, in SPEC_impl.md (Planner proposals), with the chaining rule and the
   spelling dependency. In `crates/farhelm-proto/src/grid.rs`, fix the sentence "Two runs belong to the same link ...
   when their entries have equal `id` and `uri`", say that a sender-chosen id is the emulator's `<digits>_alacritty` and
   that the client treats a direct continuation of such ids with the same `uri` as one link, and point to SPEC_impl.md;
   do it at length in one of the module doc or `Link.id`, with a short pointer in the other. The grid-buffer.js "Links"
   header comment (around 36-45), the docstrings of what changed, and the adapter's tag paragraph say briefly what a
   chain is and point to SPEC_impl.md. SPEC.md gets one clause, for example that a hyperlink a program starts again
   right where its previous piece ended, with the same target and no id of its own, continues that hyperlink. No note
   about old terminals anywhere.
5. **Tests** (below), the bookkeeping above, and the changelog fragment.

A known behavior change to pin: keying a chain by its first piece can change which rows join beyond the chain. When an
agent continues one generated id onto the next row by cursor movement rather than a soft wrap, the row holding a later
piece used to join that next row through the piece's own tag; now the piece carries the chain's tag and the next row is
judged alone. Both results are incomplete, and the new one is never quieter for text naming a different place than the
explicit-id rules allow. Pin it with a unit test and a one-line note in the `rowLinks` or adapter docs.

**Buffer unit tests** in `crates/farhelm-ui/js-tests/grid-buffer.test.js`, next to the existing link tests:

- The repro: one row `wire([[0, "https://apn", 0], [11, "ews.com/x", 1]])` with links
  `[["0_alacritty", "https://apnews.com/x"], ["1_alacritty", "https://apnews.com/x"]]`. One OSC link covering columns 0
  to 20 (end exclusive), equal adapter tags on both pieces, and `linkHoverText` over the adapter returning the whole
  text `https://apnews.com/x`. A third contiguous piece joins the same chain.
- Two links stay two: different targets side by side, and an explicit id next to an id-less piece with its target.
- A soft-wrapped chain (the first row's piece ending at the width, `w` set, the next piece at column 0) gets one tag on
  both rows, and its evidence covers both; a three-row chain's evidence covers all three; a wrap after a wide-character
  padding cell still joins.
- No join: adjacent rows without a wrap; a wrapped row whose piece stops short of the width; a next-row piece that does
  not start at column 0; same-target id-less pieces on one row separated by unlinked text.
- `answerLines` covers the chain's rows; `x_alacritty` is explicit; the existing link tests pass unchanged.

**Grid-client browser specs** in `e2e/grid-client/grid-attach.spec.ts`, Tier 3 (around 743 onward). They take wire
messages through a fake socket, so the repro becomes the wire rows the supervisor makes from it (`showScreen`). Hovering
`ews.com/x` shows the target without `terminal-link-target-mismatch`, and the underline covers both pieces. Control: the
same layout with the second piece targeting another host warns. Optionally, the soft-wrapped chain.

**Supervisor test** in `crates/farhelm-supervisor/src/grid_output/tests.rs`, next to `links_share_one_table_entry`
(around 256): feed the literal repro bytes through `emulator(...)` and assert the row's two runs carry ids matching
`^\d+_alacritty$` with the same uri. A spelling change in a vendor upgrade then fails here directly; the emulator's
snapshot comparisons (`link_eq`) would most likely fail too, but confusingly.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect:

- `dprint check` on the changed files; `cargo fmt --check` and clippy on `farhelm-supervisor` for the Rust test;
  `python3 releasing/check-changelog.py format`; `python -B scripts/check-test-sleeps.py` with the interpreter
  `docs/test-sleep-check.md` prepares.
- The JS unit harness through the recorder, as `docs/test-run-evidence.md` gives it:
  `python3 scripts/record-test-run.py --kind development --selection 'asset JavaScript unit harness' --concurrency 'node default discovery' --timeout 60 --tmux none -- bash -c 'cd crates/farhelm-ui/js-tests && node --test'`.
- The grid-client specs (`cd e2e && npx playwright test --config grid-client/playwright.config.ts`, both engines, no
  stack or build) through the recorder. The recorder's playwright mode refuses config overrides, so this probably needs
  generic mode; settle it from `docs/test-run-evidence.md`.
- The "grid streams" selection from `.agents/validation.md`, with the pinned nextest on `PATH`:
  `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'grid streams' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-supervisor --lib -E 'test(/^grid_output::/) | test(/^service::grid_stream::/)'`.
- Not needed: xterm.js specs, the host-terminal suite, the vendor tests; `scripts/check-agents-md.py` unless a heading
  changes.

## Fallbacks

Examples of departures that need the scope reassessment:

- any change under `vendor/`, in `row_from_cells`, to the wire format or to the protocol version;
- changing `terminal.js` or `terminal-links.js`, or anything the old terminals use;
- joining beyond a direct continuation, or joining an explicit id with an id-less piece;
- a real-stack or host-suite test.

Decisions to log: the generated-id test, how identities are remembered within a hover, every place switched to the new
identity, every test added and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-grid-repeated-link-warning-log.md` in the parent directory of the checkout you run in, derived as that
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

- One reviewable commit, stable bookmark and draft PR, using the `jjstack` skill. The base is `main@origin`, or this
  plan's own open PR when it resumes, set up per `plans/.agents/executing.md`, "Executing one plan" (step 6); never
  another plan's PRs. The bookmark is `plan/grid-repeated-link-warning/01-join-idless-links`; a split into more PRs is a
  logged DECISION and keeps one linear stack under `plan/grid-repeated-link-warning/<nn>-<name>`.
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`); this is a `fix`, with its changelog fragment in the same commit.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before touching `review_feedback_queue/`, read `review_feedback_queue/AGENTS.md` in full.
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
already made, planner proposals and outline, including the safety argument). When a PR changes tests or their fixtures,
include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

The plan is complete when a draft PR (or a linear stack of them) exists that fixes the false warning and the partial
underline for a program's restarted id-less hyperlink in grid terminals as the outline describes, the PR passed its
review gate, and the last code PR removes this plan's TODO.md entry and the queued review item with its `INDEX.md` line.
Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
