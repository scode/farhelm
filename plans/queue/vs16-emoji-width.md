# Grid terminals: size emoji variation sequences the way Ghostty does

Written against main at 856099b8 on 2026-11-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Emoji made wide by a variation selector in grid terminals.** In tmux, an emoji followed by the emoji variation
> selector (U+FE0F, as in "❤️") takes two columns; the emulator behind terminals in a terminal host gives it one, as
> alacritty does. A program that counts it as two columns, as tmux does, finds the rest of that row one column off in
> such a terminal. It did not come up in the recorded Claude Code and Codex sessions the emulator was checked against.
> Decide whether to match tmux (a change to the vendored `alacritty_terminal`); what xterm.js shows today for the same
> output was not checked. The difference is pinned in the supervisor's emulator parity tests
> (`crates/farhelm-supervisor/src/emulator_parity/generated.rs`).

The maintainer decided not to match tmux but Ghostty: in a terminal in a terminal host, the emoji variation selector
(U+FE0F) and the text variation selector (U+FE0E) change a character's width exactly where Ghostty's default
configuration changes it, and leave it alone where Ghostty does. Old terminals (tmux) are not affected. The xterm.js
path is being dropped: do not change it or test it.

### What Ghostty does, as the planner read it

NOTE: This is the planner's reading of `ghostty-org/ghostty` on GitHub (main branch, read 2026-11-03; MIT licensed), not
ground truth. Read Ghostty's source yourself at a commit you record in the working log and in the report, and match what
it does there. Where it differs from what follows, Ghostty wins and the difference is logged.

- **Which bases a selector affects:** `src/unicode/grapheme.zig`, `graphemeWidthEffect(prev, cp)`. U+FE0F makes the
  previous codepoint `.wide`, and U+FE0E makes it `.narrow`, only when that codepoint is a valid variation-sequence base
  (an `emoji_vs_base` property from its props table, which corresponds to Unicode's `emoji-variation-sequences.txt`).
  For any other base it returns `.ignore`: the selector does not change the width. It does not use Emoji_Presentation or
  Extended_Pictographic for this. Its tests: U+2764 + FE0F is wide; `#` + FE0E is narrow and `#` + FE0F is width 2;
  U+231A (wide by default) + FE0E is width 1; `x` + FE0F is ignored, width 1.
- **How the screen changes:** `src/terminal/Terminal.zig`, `print()`, with grapheme clustering on.
  - A widening selector when the base sits in the last column with wraparound on writes a spacer head there (or a blank
    narrow cell inside a right margin), wraps, and writes the base as wide at the start of the next row, keeping its
    grapheme's codepoints. With wraparound off, the selector is dropped and the base stays narrow.
  - A narrowing selector on a wide cell makes it narrow, removes its spacer tail, and leaves the cursor one cell after
    the base.
  - With grapheme clustering off (legacy), selectors are only attached and never change a width.
- **Which of those is the default:** mode 2027 (grapheme clustering) defaults to off in `src/terminal/modes.zig`, but
  Ghostty's default configuration, `grapheme-width-method = unicode`, gives Unicode widths. Confirm how the default
  configuration maps onto the mode, and match Ghostty's default user-visible behavior, not the mode's bare default.

### Acceptance criteria

- **The rule, in the emulator:** for every case below, the emulator's screen and cursor after the output are what
  Ghostty's default configuration gives, with the Ghostty source line that decides each case logged as a DECISION with
  that line as evidence. The cases include the ones above and these, which the planner did not read:
  - the base directly before the cursor with room after it, the base one before the last column, and the base in the
    last column with a wrap pending, each with autowrap on and off;
  - a selector on a base that is already wide (U+231A + FE0F) or already narrow (`#` + FE0E);
  - insert mode;
  - a selector after a combining mark (`❤` U+0301 FE0F), and a selector with no base (column 0, nothing before it);
  - a selector the emulator drops because the terminal's cell extra storage is full (`vendor/README.md`, "Caps on
    hostile output");
  - REP (`CSI Ps b`) right after a sequence, and with a selector as the character REP repeats;
  - a widened base whose spacer lands on half of another wide character, and a one-column terminal.

  Where Ghostty has no counterpart (the storage cap, say), take the choice closest to its behavior, preferring layout
  that follows the program's bytes over memory accounting, and log it.
- **Where tmux differs, pinned:** tmux 3.7c widens any one-column character followed by U+FE0F and never narrows. Every
  case where the new rule and tmux disagree is pinned in the parity tests' known differences, confirmed against a live
  tmux through the harness.
- **Snapshots:** the frozen v1 format holds. A snapshot taken by today's supervisor restores exactly as it was saved,
  and a snapshot of a terminal under the new rule round-trips exactly.
- **What pinned the old behavior changes in the same PR,** as listed under Outline.
- **Changelog:** a fragment, `kind: fixed` (v0.34.0 shipped the caveat in its release notes).

## Decisions already made

Settled with the maintainer on 2026-11-03:

- **Follow Ghostty, not tmux.** The maintainer's words: "for the width emoji thing, what does ghostty doe? do whatever
  ghostty does". That covers every case, including the ones nobody listed; and the text selector U+FE0E narrows where
  Ghostty narrows. Where tmux differs, the difference is pinned, not followed.
- **Where the code goes:** a patch to the vendored emulator (`vendor/alacritty_terminal`, `Term::input`), with its patch
  file and the `vendor/README.md` records per that file's rules. The planner decided this and told the maintainer: it
  reuses alacritty's existing wrap, overwrite-cleanup and wide-character handling, which the narrowing and the move to
  the next row need.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **xterm.js:** out of scope; do not change or test its path.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:**

- `vendor/README.md` in full: never edit the trees without the patch file; `Modified for Farhelm` and `Farhelm patch`
  markers; regenerating and verifying the patch ("Checking a tree against upstream, and upgrading"); the per-patch
  record template and its hunk-header bookkeeping ("Per-patch records for upstream reports", "Hunks shared between
  records"); running the vendored tests ("Running their tests"); keeping the patch warning-free.
- SPEC_impl.md, "The supervisor's terminal emulator" (around 2517) and "Terminal snapshots" (around 2549), and the
  snapshot module docs (`crates/farhelm-terminal-emulator/src/snapshot/`): v1 is frozen, every byte it writes and every
  rule it reads by.
- Root `AGENTS.md`: "Frozen material" (the published `CHANGELOG.md` section stays as it is), "Conventional Commits",
  "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation", "The live install is off-limits".
- `website/AGENTS.md` and `website/EDITORIAL_RULES.md` before the docs edit.
- `.agents/test-authoring.md`, `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code or Ghostty says otherwise, logged as a DECISION):

- One PR. The parity test pins the old emulator behavior and the round trips need the snapshot change, so splitting the
  rule from its tests and docs leaves a red PR in between. Split only if every PR in the stack passes its own checks
  without churn.
- Which bases count: `unicode-width` plus a hand-written list of 8 exceptions, or one literal list of the 371 bases. No
  generator script either way. Planning evidence (a throwaway probe during the planning review, against `unicode-width`
  0.2.3, the workspace's, Unicode 18.0, and 0.2.2, the vendored lockfile's, Unicode 17.0; both gave identical results):
  `emoji-variation-sequences.txt` lists the same 371 bases in Unicode 16.0, 17.0 and 18.0. For U+FE0F, all 213 narrow
  bases measure 2 as the string `[base, '\u{FE0F}']`, and no character outside the file changes width with either
  selector, so for widening `unicode-width` gives exactly Ghostty's `emoji_vs_base` set. For U+FE0E, 150 of the 158 wide
  bases measure 1 as `[base, '\u{FE0E}']`; the 8 that do not are U+3030, U+303D, U+3297 and U+3299 (no
  Emoji_Presentation) and U+1F202, U+1F21A, U+1F22F and U+1F237 (Enclosed Ideographic Supplement). Ghostty gates both
  selectors on `emoji_vs_base` alone, so it narrows those 8. The smallest design is `unicode-width` for both directions
  plus a hand-written, commented list of the 8, citing `emoji-variation-sequences.txt` and its Unicode version; one
  literal 371-entry list (about 200 lines as ranges) is the alternative if you would rather not lean on
  `unicode-width`'s sequence logic, whose docs say it "may be tweaked in the future". The comparison is done; confirm
  only the 8 against Ghostty's own table at the commit you read, and record whether the list needs a
  `THIRD_PARTY_NOTICES.md` entry (more likely for the 371-entry list) with your reasoning. Copy no Ghostty code.

## Outline

Line numbers are approximate (main at 06c1e7cd or 856099b8).

1. `plan/vs16-emoji-width/01-emoji-width`, everything below.

**The rule in the vendored `Term::input`** (`vendor/alacritty_terminal/src/term/mod.rs`, around 1235-1329). Today a
character's width is `c.width()` from `unicode_width::UnicodeWidthChar`, and a zero-width character is appended to the
cell before the cursor (or to the cursor's cell when a wrap is pending, stepping left over a `WIDE_CHAR_SPACER`) through
`extra_storage.push_zerowidth`, without moving the cursor. So `❤️` is one narrow cell holding FE0F as a mark. Note the
`saturating_sub` around 1247: at column 0 with no wrap pending, the mark lands on the cell under the cursor.

- Widening re-places the base as a wide character at its own column, reusing what `input` already has for a wide
  character: the one-column guard (around 1262-1268), the wrap (around 1271), the insert-mode shift (around 1275-1285,
  adjusted to whatever Ghostty does), and the wide write with its `LEADING_WIDE_CHAR_SPACER` at the margin and its drop
  with autowrap off (around 1287-1318). The guard and the shift sit outside that last range, so factoring out only it
  would leave the panic fix behind.
- NOTE: That wide write goes through `write_at_cursor` (around 1125-1161), which copies the cursor template into the
  cell: its colors, flags and extra (underline color, hyperlink), and no marks. Reused unchanged on an existing base it
  would give the base the current pen (`❤ ESC [ 31 m` U+FE0F would turn the heart red), put the current hyperlink on it,
  and drop its combining marks and the selector. The re-placed base keeps its own cell's attributes, hyperlink,
  combining marks and selector, with the extra moved through `ExtraStorage` so the storage cap still holds.
  `write_at_cursor` also re-applies the charset map; check that this is the identity for every eligible base.
- Narrowing clears `WIDE_CHAR` and the spacer and moves the cursor back as Ghostty does.
- The rule decides from what is on screen: the cell the selector attaches to (its character, marks and flags) and the
  cursor's wrap flag. It never decides from new state carried between characters, such as a "last printed codepoint"
  field: a snapshot can fall between a base and its selector (both are clean boundaries), and v1 carries only the grid
  and vte's REP character, never a new field.
- Add a `pub` switch on `Term` for the rule, on by default, for the snapshot restore below.
- The existing wrap-flag, `CSI J` and one-column patches keep holding. A vendored test in the `term::tests` hunk using
  `fed_term`, covering the cases in the acceptance criteria.

**`vendor/README.md`:** a new per-patch record with every field of the template (upstream behavior, a byte-level
reproduction, xterm and tmux 3.7c with the source checked, plus Ghostty at your recorded commit, the fix, why Farhelm
needs it, the upstream code, the hunks, the tests, and upstream status checked against `alacritty/alacritty` master
`aecf0f6`). The xterm-412 source may not be on this machine: fetch it or write "not checked". Update the intro's count
of patches, and re-read every `term/mod.rs` hunk header the records and "Hunks shared between records" cite from the
regenerated diff, since a change in `Term::input` moves every later one. Regenerate and verify the patch per the README.

**Snapshots** (`crates/farhelm-terminal-emulator/src/snapshot/`). The switch is needed because today's encoder paints a
narrow `❤` holding U+FE0F as a mark as a `Unit::Single` (`classify`, `v1/paint.rs`, around 583-641), writing `❤ U+FE0F`
into the stream, and a wide `⌚` holding U+FE0E as a `Unit::Pair`, writing `⌚ U+FE0E`; fed with the rule on, the first
widens and the second narrows. The width-table fingerprint cannot catch that: it covers per-character widths only
(`WidthTable::current()`, around 224), so it does not change, no redraw is requested, and the cells are lost silently
(the next cell, placed by CHA, lands on the new spacer, as "Character widths" describes for a newly wide character).
Making the fingerprint cover the rule would change the meaning of a v1 header field and make every running terminal ask
for a redraw at the upgrade. So `v1::decode` turns the rule off right after `Emulator::new` (around 399), for both
widening and narrowing, and back on before it returns. The switch must not leak into the host-journal replay after a
restore (`FeedMode::ReplaySeen` is shared with journal replays, so it cannot be the switch).
`history_seed_from_lines_until`'s scratch emulator (`snapshot/mod.rs`, around 746) is an ordinary `Emulator` and gets
the rule's default, on, which is closer to tmux's layout of the captured lines; leave it unless you find a reason not
to. The encoder needs no change: a widened cell (wide, narrow character) fails the `Pair` test, its spacer fails the
`Single` test, a narrowed cell (narrow, wide character) fails the `Single` test, so all are `Unit::Fix`, and a leading
spacer from a widening wrap is an ordinary `Unit::Leading`. With the rule off, the pending-wrap reprint and the REP
trick (around 1254-1300 and 1331-1367) behave as today. Leave the fingerprint alone;
`the_width_fingerprint_is_as_documented` keeps passing. State the rule in the v1 docs' reading steps and in "Character
widths". Tests:

- the real guard: a hand-built old-style blob, through `V1::header` in `tests/snapshot.rs` (as
  `a_hand_built_v1_blob_restores_as_its_layout_says` does), whose stream is the bytes today's encoder writes for `❤️x`
  and for `⌚` with U+FE0E, restores narrow and wide as saved;
- widened and narrowed states round-trip, and the switch is back on after a restore;
- eligible pairs in `tests/common/synthetic.rs` (it emits only `e` + FE0F today), which put widened and narrowed cells
  and their wraps through the round-trip, delegation and boundary property tests.

No new golden script is needed: a golden recorded after the change holds no old-style stream, and the fixup records it
would hold are already pinned by the `fixups` and `fixup-at-the-limits` goldens. If you add one anyway, do not describe
it as compatibility coverage.

**What needs no change, to confirm:** grid output (`crates/farhelm-supervisor/src/grid_output.rs`, around 144-214) takes
run width from `WIDE_CHAR`, and the grid client draws a width-2 run as a two-cell box and pads a wrapped row's uncovered
last cell (`grid-buffer.js`, `padsWideWrap`); `screen_text`, cursor reports and the delegation test follow on their own.
A narrowed cell reaches the client as a one-cell run holding a glyph that is wide by default: check that it draws inside
one cell. If the grid client needs a change for that, make it in the same PR.

**Everything that pins today's behavior:**

- Parity (`crates/farhelm-supervisor/src/emulator_parity/generated.rs`): remove the known difference (around 873-878);
  reword the `GLYPHS` doc (around 75-79), the `Stream::piece` doc (around 145-160) and the `KNOWN_DIFFERENCES` doc
  (around 833-847); add one or two agreeing scenarios (`❤` U+FE0F mid-row, say) to `hand_picked_screens_match_tmux`; pin
  each disagreement (a letter + FE0F, FE0E narrowing, the last column, insert mode, autowrap off, a mark before FE0F,
  and any others you find) as a known difference. Selectors in the generated streams are optional: add one to `GLYPHS`
  only if every position agrees with tmux. The generator does not track where the cursor lands, so position-aware
  placement would be a new mechanism with no requirement behind it.
- `crates/farhelm-terminal-emulator/tests/screen.rs`, the "variation selector" case (`❤️z`, around 74-78): it checks
  capture text, which skips spacers, and should keep passing; run it.
- `e2e/tests/terminal-unicode.spec.ts` (around 255-275): the grid branch expects the cursor at
  `CURSOR_PREFIX.length + 2` and loses its known-difference annotation and comment. Leave the xterm.js branch as it is.
- `SPEC_impl.md`: drop the "open decision" sentence (around 1734-1735); one sentence in "The supervisor's terminal
  emulator" that variation selectors size emoji as Ghostty does, with the tmux differences named; and in "Terminal
  snapshots", that v1 streams replay with the rule off and changed cells travel as fixups. SPEC.md says nothing about
  cell widths and needs no change.
- `website/src/content/docs/docs/using/new-terminals.md` (around 75-77): the "no decided fix yet" paragraph. Read
  `website/AGENTS.md` and `website/EDITORIAL_RULES.md` first. In this unattended run start no preview server; name the
  changed page in the report.
- The emulator crate's module docs, where they list departures from tmux.
- `TODO.md`: remove the entry (around 189-195).
- `releasing/changelog.d/<mnemonic>.md`, `kind: fixed`, worded per `releasing/EDITORIAL_GUIDANCE.md`, with caveats where
  the result differs from tmux. The published `CHANGELOG.md` section stays as it is.

Out of scope: tmux's merging of regional-indicator pairs, skin-tone modifiers and ZWJ sequences; list them in the report
as possible follow-ups, not as TODO entries.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Run every runtime test through
`python3 scripts/record-test-run.py`, with the selections and the pinned nextest and tmux setup that
`.agents/validation.md` gives. Expect:

- the three vendored crates (alacritty_terminal, vte, and the paired tests), all three because `vendor/` changes, and
  the patch check from `vendor/README.md` (apply it to a fresh unpack, `diff -r` prints nothing);
- `cargo nextest run -p farhelm-terminal-emulator` (golden, round-trip, snapshot, delegation, boundary and screen tests,
  the "variation selector" case of `tests/screen.rs` among them);
- the emulator parity tests with tmux required; read the retained output for `SKIPPED`;
- the grid stream selection of `farhelm-supervisor`;
- the host-terminal run of the `terminal-unicode` spec in Chromium and WebKit. Skip the ordinary (xterm.js) run, with
  the reason;
- the grid-client specs and JS tests only if a grid asset or fixture changes;
- `dprint check` on the changed files, clippy on the touched workspace crates,
  `python3 releasing/check-changelog.py format`, `python -B scripts/check-test-sleeps.py` with the interpreter
  `docs/test-sleep-check.md` prepares, and `python3 scripts/check-agents-md.py` if a heading is renamed.

## Fallbacks

Examples of departures that need the scope reassessment:

- changing a byte the v1 encoder writes, the width-table fingerprint, or the grid protocol;
- putting the rule in Farhelm's own handler (`crates/farhelm-terminal-emulator/src/queries.rs`) instead of the vendored
  `Term`;
- following tmux, or anything other than Ghostty, for a case Ghostty decides;
- a generator script or fetched data file for the base list;
- state carried between `input` calls for the rule;
- a grid client change beyond drawing a narrowed cell;
- taking in the regional-indicator, skin-tone or ZWJ merging.

Decisions to log: the Ghostty commit read; every case with its Ghostty source line and the emulator's result; which base
list you chose and whether Ghostty's table confirms the 8 exceptions; every tmux difference pinned; every test changed
and why; and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-vs16-emoji-width-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files, including the unpacked crates for the patch work and any
Ghostty or xterm source you fetch, go in the agent scratch directory that `AGENTS.md`, "Agent scratch space", describes,
not in the checkout.

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
without costing quality (reading Ghostty's source while you work on the patch, say). Concurrent agents that might
otherwise conflict can work in separate jj workspaces whose results you merge back; you still own the stack and every
PR. Launch a sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you
need is not available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies
the launch mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/vs16-emoji-width/<nn>-<short-name>`, as
  the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`; this is a `fix`), and a changelog fragment in the same commit.
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
findings file in the scratch directory, the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline), and the Ghostty commit you read, so the reviewer can check the cases against it. The PR
changes tests, so include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md`
requires.

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
Wording in this file that turns out inaccurate (a line number, a file name, the planner's reading of Ghostty) is
corrected to the truth.

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

## Blocked

Blocked on 2026-11-04 (claim 782ee7).

### Emoji width: exact Ghostty matching needs more saved terminal state

This plan fixes emoji sequences such as a heart followed by U+FE0F taking one column in a terminal host when a program
expects two. The agreed target is Ghostty, including U+FE0E narrowing and edge cases, while preserving existing
snapshots and adding no state between input characters.

U+FE0F requests emoji presentation; U+FE0E requests text presentation. Those requirements conflict at the right edge
with autowrap disabled. Ghostty distinguishes an empty cell from a space the program printed. Farhelm stores both
identically. On a five-column terminal, compare these histories:

- Disable autowrap, print a space in column 5, erase that cell, then print a heart in column 4.
- Disable autowrap, print a space in column 5, then print a heart in column 4.

Farhelm reaches the same cells, cursor and character remembered for repetition, and writes exactly the same 69-byte v1
snapshot for both. A following U+FE0F makes Ghostty widen the heart in the first history and leave it narrow in the
second. The distinction is absent from the snapshot, so a decoder change cannot recover it. This is confirmed by a
private Rust probe against the current emulator, recorder run `9bbde772-6194-4132-8264-b8da97ce8bfc`. Ghostty's result
is source-derived at commit `c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed`,
[Terminal.zig, lines 1939–1967](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L1939-L1967);
Ghostty itself was not built or run.

There is also a conflict on a terminal only one column wide. With autowrap on, `x`, then a wide watch character, then
U+FE0E makes Ghostty wrap after `x` before discarding the too-wide watch. Farhelm drops the watch before processing the
pending wrap, so it stays on the original row. The selector itself is ignored by Ghostty at column 1. Matching that
final screen and cursor would require changing ordinary wide-character handling that the plan also says to preserve.
This follows
[Terminal.zig, lines 2189–2262](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L2189-L2262).

May the plan retain these two explicit exceptions to Ghostty, or should its scope expand to match them?

- **Keep the fix bounded:** treat a plain space as empty when choosing which cell receives the selector, retain today's
  one-column wide-character handling, and pin both disagreements with Ghostty. This keeps the existing snapshot format
  and delivers the ordinary emoji-width fix, but a selector can widen the heart when a printed space in the final column
  makes Ghostty leave it narrow, and the one-column cursor can stay on a different row.
- **Expand exact matching:** add persistent cell information and a new snapshot format beside unchanged v1, and change
  ordinary wide-character handling on one-column terminals. This is a larger terminal-state change. Existing v1
  snapshots still cannot reveal whether their spaces were printed or empty, so this option also needs a specified
  fallback for old terminals after restore.

I recommend keeping the fix bounded with these explicit exceptions. It keeps restore behavior consistent with
uninterrupted output without expanding the snapshot format, and makes the remaining differences testable and documented.
This would narrow the agreed exact-Ghostty guarantee, so I have not implemented it. A fresh scope review independently
confirmed the conflicts. No product changes or draft PR exist for this plan; the source findings are retained in its
working log.
