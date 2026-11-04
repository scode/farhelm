# Session list: agent marks in their makers' colors where allowed

Written against main at 856099b8 on 2026-11-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Replace harness icons with colored versions.** Replace the harness icons with colored versions so they are more
> easily identified.

The maintainer sharpened that on the planning page, after seeing a mockup of three approaches with the real marks at
real size on the real sidebar colors: color a mark only where its owner's written terms allow, in the owner's official
color where one exists, so each agent is recognizable at a glance. In their words: "yes to be clear the intent was to
match the official stuff where possible/allowed so its recognizeable." And on the set as a whole: "and pick colors to
make sure they try to be separate and not close to each other."

NOTE: This changes colors only. No mark's shape changes, no mark is replaced, and nothing outside the session list's
agent track is in scope.

### What exists today

From a read of main at 856099b8 (line numbers approximate):

- **The marks.** `crates/farhelm-ui/src/icons.rs`: the `HarnessGlyph` enum (around 410) and the harness-mark section
  (around 460-538) with one geometry constant per mark. `HarnessIcon` (around 548-599) renders an `svg` with
  `class="sidebar-glyph harness-glyph"` and a per-harness `data-glyph` token (`claude`, `codex`, `muse`, `cursor`,
  `goose`, `pi`, `omp`, `grok`, `opencode`, `terminal`). Every mark is filled or stroked with `currentColor`, so its
  color comes entirely from CSS. Several doc comments there speak about color: the Cursor constant (around 496-498,
  "monochrome rendering matches the owner's use") and the Muse constants (around 531-536, "the source icon's blue and
  its sparkle are dropped").
- **The one caller.** `crates/farhelm-ui/src/list/row.rs` around 1793-1810 puts `HarnessIcon` and `PermissionIcon` side
  by side in the row's `.session-agent` track, each in its own tooltip span. The harness mapping is around 623 and
  666-706. Nothing else renders `HarnessIcon` (the quick switcher and the launch composer name agents in text). If a
  caller has been added since, the same colors apply there.
- **The CSS** (`crates/farhelm-ui/assets/app.css`):
  - The dark palette is `:root` (around 120-362), the light palette `:root[data-appearance="light"]` (around 367-420);
    the theme switch is that attribute, not `prefers-color-scheme`. Sidebar surface `--bg-1` is `#1d222c` dark (around
    168) and `#e9edf4` light (around 382). A row takes `--bg-2` while hovered or holding focus: `#282e3a` dark,
    `#ffffff` light (around 171-176, 383).
  - `.session-agent` (around 4386-4394) sets `color: var(--fg-1)` (`#a2aabb` dark, `#4f5a70` light), which every harness
    mark inherits today. `.sidebar-glyph` (around 4577) only sizes the mark.
  - The precedent for per-mark color is the permission mark beside it: `.permission-glyph[data-glyph="yolo"]` and
    `unknown` use `--warn` (amber), `shielded` uses `--ok` (green), `own-setting` uses `--fg-1` (around 4615-4627).
  - The host identity colors (`--host-icon-*`, around 130-139 dark and 372-378 light) are the precedent for identity
    hues with a separate shade per theme; their comment says they sit "away from status and accent hues".
  - The accent comment (around 208-235) says "every other hue in this sheet is spoken for by a state". That is already
    wrong today, because the host identity colors exist; correct it.
  - The red neighbors in the same row: a local row's default host mark is the red laptop (`--host-icon-red`), and the
    waiting status uses `--danger`.
  - Selected rows: the inverse block (around 9245-9262) sets `.session-row.selected .sidebar-glyph` to `--on-fill`
    (white), which already covers harness marks. Host marks needed an `!important` override (around 9286-9291) only
    because they carry inline color. A `.harness-glyph[data-glyph="x"]` rule is (0,2,0) and the selected rule is
    (0,3,0), so the selected rule wins whatever the source order.
  - A disabled row's open button is at half opacity (around 3020-3023). That dims every mark on such a row, colored or
    not, and is out of scope. No stale-row rule touches the agent track (stale rows dim only the title and status dot,
    around 4952-4961 and 5111).
- **CSS tests** (`crates/farhelm-ui/js-tests/`): `app-css-contrast.test.js` computes WCAG ratios for both palettes
  (`PALETTES` around 28, `REQUIRED_BINDINGS` around 108-134, with the host icon colors at 4.5:1 around 119-121 and
  status shapes at 3:1 around 126). It pins which selector routes to which token in `SHIPPED_ROLE_BINDINGS` (around
  252-270), and `SELECTED_PRIMARY` (around 202) already includes `.session-row.selected .sidebar-glyph`, so the static
  selected-row pin exists. `app-css-tokens.test.js` refuses a hex literal outside `:root` and requires both palettes to
  cover every color role (around 248-262), so each new color is a token declared in both palettes.
- **Browser tests** that pin harness marks by `data-glyph`: `e2e/tests/sidebar.spec.ts` around 3900-3929 (column
  alignment and which mark each row shows; around 3937 a `toHaveCSS("color", ...)` check against a token is the pattern
  to copy) and `e2e/tests/terminal.spec.ts` around 318 and the compact-rows test around 4015-4060. That test asserts the
  permission mark's role color around 4055-4059 and branches to `--on-fill` when `data-session-selected` is `"true"`,
  but nothing in it selects a row (it mocks the listing, loads the page and never clicks one), so the selected branch
  probably never runs. Copying it for the harness mark would give a vacuous selected-row check.
- **Specs.** SPEC.md around 1794-1797 describes a row's first line ("agent label"), and around 1841-1842 says "On a
  solid selected row, host and permission marks use the inverse foreground": agent marks are not named there.
  SPEC_impl.md around 383-385 says the agent track is rendered as glyphs and around 401-406 gives the permission marks'
  colors; the harness colors belong beside that.
- **Provenance.** `docs/harness-marks.md` (binding: root `AGENTS.md`, "Code rules" and "Instructions by path") records
  each mark's source and its owner's terms, researched 2026-09-22, and says official geometry is scaled and nothing else
  (around 22-24). `THIRD_PARTY_NOTICES.md` "Harness marks" (around 10-14) says the vendored geometry is used unaltered;
  its Cursor entry (around 95-100) says the cube is "rendered in the current text colour as cursor.com itself renders
  it", and its Muse entry (around 62-68) says the color was dropped. Its list of Farhelm's own drawings names Claude and
  Terminal but omits Grok.
- **Per-harness map.** `crates/farhelm-supervisor/src/agent_kind/mod.rs` module docs, "How the browser shows it" (around
  74-77), say `list/row.rs` maps harnesses to their marks in `icons.rs`. The harness colors are a new per-harness place
  and belong in that map.
- **Published screenshots.** `e2e/docs-shots/session-list.spec.ts` captures the session list for the docs website. Those
  images will show the old colors until the next docs screenshot refresh; that refresh is not part of this plan.

### Acceptance criteria

- **Which marks are colored.** Claude, Muse, Cursor, Pi, OMP, Grok and OpenCode each draw in their own color on an
  ordinary (unselected) row, in both themes. Codex and Goose stay one-color in `currentColor`, inheriting the agent
  track's `--fg-1` grey as today (that is not the owners' exact black or white, and nothing here should claim it is).
  The Terminal mark (not an agent) stays `--fg-1` as today.
- **Where each color comes from.** For each colored mark, re-check its owner's brand page and repository (the sources
  `docs/harness-marks.md` cites; terms change) for an official color: a brand color, or the colors of the mark in its
  own favicon or app icon. The rule for what counts:
  - A mark whose official rendering is black, white or grey has no official hue and gets a Farhelm hue. Today that is Pi
    (the pi.dev favicon is `#111111`, or `#f6f6f6` in dark mode), OpenCode (a white ring and grey square on near black)
    and Cursor (cursor.com renders the cube in `currentColor`; the downloads are light and dark variants).
  - OMP's favicon is a three-stop gradient (about `#ed4abf` pink, `#9b4dff` purple, `#5ad8e6` cyan). Use one stop, the
    one that separates best from the rest of the set, and record which.
  - Muse's blue comes from the unofficial community extension the mark was taken from, not from Meta, so it is not an
    official color. Muse gets a Farhelm hue, which may still be blue if that fits the set.
  - Grok's mark is Farhelm's own drawing and xAI's own artwork is black and white, so Grok gets a Farhelm hue.
  - Claude's mark is Farhelm's own drawing and takes Claude's terracotta, `#d97757`: the one fixed official hue today.
    That value is only 2.66:1 on the light sidebar, so the light theme's Claude shade is a darker color derived from it
    (about `#c0694d` reaches 3.3:1).

  If the re-check finds that an owner now publishes a real hue for its mark, use it and treat it as official below.
  Record the source and the date checked for every color, including the "black and white, so a Farhelm hue" findings.
- **Separate from each other.** The governing intent is the maintainer's: "pick colors to make sure they try to be
  separate and not close to each other." Place the Farhelm hues for as much separation as the set allows, aiming for a
  CIEDE2000 of at least 15 between every pair within a theme. That number is a target, not a gate: compute it for every
  pair in both themes and report the table in the working log and the PR description. Never move an official hue for
  distinctness; only its lightness changes per theme, to reach the contrast floor. If two official colors land close
  together, keep both and report the pair and its distance as a follow-up.
- **Contrast.** Each color reaches at least 3:1 against `--bg-1` (the sidebar) and `--bg-2` (hovered, focused and
  menu-open rows) in both themes. A separate dark and light shade per mark is fine (one token per theme), as long as the
  two stay recognizably the same color; every official color's light-theme shade will likely be darker than the official
  value, and the colors section says so.
- **Selected rows.** On a selected row every harness mark is white (`--on-fill`), like the rest of the row, never
  colored.
- **Provenance kept true.** `docs/harness-marks.md` gains a colors section: per mark, its color or colors, the source
  and the date checked, and the terms basis for coloring it or for keeping it one-color, including the black-and-white
  rule above. The distance numbers stay out of it (they go stale with every retune). It also gains a rule for future
  harnesses (the Kimi Code agent TODO.md plans to add, say): check the owner's terms for coloring, take the official hue
  where allowed, keep one-color where not, pick a Farhelm hue that stays separate from the existing set, and clear the
  contrast floor. The Claude section records that coloring Farhelm's starburst in Claude's terracotta, which moves it
  closer to Anthropic's spark, was accepted by the maintainer on 2026-11-03. Every other sentence in that file, in
  `THIRD_PARTY_NOTICES.md`, in the doc comments in `icons.rs`, and in app.css comments that says or implies a mark is
  monochrome is corrected.
- **Specs.** SPEC.md and SPEC_impl.md say what the agent marks' colors mean and that a selected row inverts them, in
  their own registers (SPEC.md the behavior, SPEC_impl.md the mechanism).
- **Tests.** A test pins which marks are colored, that Codex, Goose and Terminal are not, that each color clears 3:1 in
  both themes, and that a selected row's harness mark is `--on-fill`. The selected-row assertion must actually select a
  row and confirm `data-session-selected` is `"true"` before checking the color.
- **Visual check.** Look at both themes at real size, ordinary and selected rows, with every harness present, and
  describe what you saw in the report. Screenshots are not committed.
- **Changelog and TODO.** A changelog fragment per root `AGENTS.md`, "Changelog fragments" (kind `changed`). The PR
  removes the TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-11-03, on the planning page:

- **Approach B: color only where the owner's terms allow.** Codex (OpenAI's blossom, whose terms say "exactly as
  provided" and forbid modification) and Goose (the LF Projects trademark policy asks that a logo not be "displayed with
  color variations") stay one-color in `currentColor`, as today. The Terminal mark stays the agent track's color.
  Rejected: recoloring every mark (violates those two terms), and white marks on colored tiles (shrinks every mark at
  12px, and a white Goose on a tile is still a color variation).
- **The intent**, verbatim: "yes to be clear the intent was to match the official stuff where possible/allowed so its
  recognizeable."
- **Color sources.** The planning page said "Most of these brands publish a one-color mark ... so 'official' often means
  no color at all" and offered "Official color where one exists, Farhelm hue otherwise", which the maintainer picked. So
  a black, white or grey official mark takes a Farhelm hue (see "Where each color comes from" above). Record source and
  date in `docs/harness-marks.md`. Claude takes Claude's terracotta; the planning page stated that this brings Farhelm's
  starburst closer to Anthropic's spark, and the maintainer chose that option. Grok's mark is Farhelm's own drawing.
- **Distinct colors**, verbatim: "and pick colors to make sure they try to be separate and not close to each other."
  Official hues are kept; the Farhelm hues are the ones that move.
- **Contrast:** at least 3:1 against the sidebar background in both themes; separate light and dark shades are allowed.
- **Selected rows:** marks turn white like the rest of the row.
- **Candidate hues from the mockup, a starting point only:** Claude `#d97757`, Muse `#4c8df6`, Cursor `#9b7bf0`, Pi
  `#e05aa0`, OMP `#17b3c9`, Grok `#e5534b`, OpenCode `#7fb33a` (Codex `#10a37f` and Goose `#d4a017` were shown but are
  not used). These were mockup values, not research: Grok's red sits close to Claude's terracotta (CIEDE2000 about 9-10)
  and next to the red laptop host mark, OpenCode's green is near the shield's `--ok`, and the amber clearly missed 3:1
  on the light sidebar.
- **If a re-check changes the terms:** a mark whose terms turn out not to allow coloring stays one-color, and the report
  says so; that needs no block. If Codex's or Goose's terms now seem to allow it, keep them one-color anyway (the
  decision rests on the recorded terms) and list it in the report as a possible follow-up.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:**

- `docs/harness-marks.md` in full, especially the sizing rule, "Official geometry is scaled by the rule and nothing
  else", and "Adding or changing a mark".
- Root `AGENTS.md`: "Code rules" (per-harness behavior lives in the places `agent_kind/mod.rs` maps, never in
  comparisons at a call site), "The live install is off-limits", "Sharing the machine with other agents", "Frozen
  material" (the docs screenshots), "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and
  FLAKES.md".
- `.agents/docs-media.md`, "Docs screenshots": only the publish scripts push screenshots; never commit one on main.
- The CSS invariants the js-tests enforce: colors are `:root` tokens, declared in both palettes.
- SPEC.md's row and selection text (around 1794-1797 and 1841-1842).
- `.agents/test-authoring.md` for any test change, and `docs/test-sleep-check.md` with `scripts/check-test-sleeps.py`
  for browser test changes.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- One color token per colored harness in each palette (named like `--harness-claude`), applied by
  `.harness-glyph[data-glyph="<token>"]` rules in app.css next to the permission-mark colors. No geometry change and no
  change to `HarnessIcon`'s markup, so the "unaltered geometry" statements stay true. Codex, Goose and Terminal get no
  rule at all, so they keep inheriting `--fg-1`.
- The selected rule keeps winning by specificity, not by `!important`; keep the per-harness rules at (0,2,0).
- Tests, using the existing facilities rather than new machinery: in `app-css-contrast.test.js`, add one
  `REQUIRED_BINDINGS` entry binding each harness token at 3:1 against `--bg-1` and `--bg-2` (not every neutral surface:
  the agent track never sits on `--chip-bg` or `--control-hover-bg`); add a
  `[".harness-glyph[data-glyph=\"x\"]", "color", "var(--harness-x)"]` row to `SHIPPED_ROLE_BINDINGS` for each colored
  mark; and assert that Codex, Goose and Terminal have no `color` declaration
  (`declarationsForSelector(...).has("color")` is false). No new selector scanner and no CIEDE2000 code in tests. In the
  browser, add a `toHaveCSS` line beside the permission-color line in the compact-rows test in
  `e2e/tests/terminal.spec.ts` (it already covers codex, muse, claude, opencode, omp and terminal), checking a colored
  mark against its token and Codex against `--fg-1`. For the selected case, select a row (or extend a test that already
  does) and assert `data-session-selected` is `"true"` before checking the mark is `--on-fill`.
- Prefer Farhelm hues that keep clear of the permission shield's amber `--warn` and green `--ok`, of the red used by the
  local laptop host mark (`--host-icon-red`) and the waiting status (`--danger`) in the same row, and of the neutral
  greys. This is a preference, not a gate. An official color that lands near them is kept, and the report says so.
- For the visual check, a throwaway Playwright capture against a mocked session listing (the way the tests above mock
  one) in your scratch directory, never the maintainer's running Farhelm. Do not run or publish the docs screenshots.
- Add the colors to the "How the browser shows it" entry in `agent_kind/mod.rs`'s map, and list Grok among Farhelm's own
  drawings in `THIRD_PARTY_NOTICES.md`.
- Commit type `feat` (a visible change for users); the fragment's `kind:` is `changed`.

## Outline

1. `plan/colored-harness-marks/01-colored-marks`: the brand re-check and color choice, the CSS tokens and rules, the
   tests, `docs/harness-marks.md`, `THIRD_PARTY_NOTICES.md`, the doc comments in `icons.rs` and app.css, SPEC.md and
   SPEC_impl.md, the `agent_kind/mod.rs` map line, the changelog fragment, and the TODO.md removal. One PR.

The research comes first and goes into the working log as you go: for each of the seven colored marks, the pages
checked, the color found (or "none", or "black and white"), and the terms basis. Then pick the set, compute the contrast
and pairwise CIEDE2000 numbers for both themes, and adjust the Farhelm hues before writing CSS. If a source page cannot
be reached, use what `docs/harness-marks.md` recorded and the mark's pinned upstream file (its favicon or source SVG),
say so in the colors section and the report, and carry on; that is not a block.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, the JS unit
harness under `crates/farhelm-ui/js-tests`, `python3 releasing/check-changelog.py format`, `cargo check` (or clippy) on
`farhelm-ui` and `farhelm-supervisor` if their sources change, `python -B scripts/check-test-sleeps.py` with the
interpreter `docs/test-sleep-check.md` prepares if a browser test changes, and, through
`python3 scripts/record-test-run.py`, the browser tests you changed plus the other harness-mark tests named above, in
both engines. Run `python3 scripts/check-agents-md.py` if you rename a heading in `docs/harness-marks.md`.

## Fallbacks

Examples of departures that need the scope reassessment:

- changing any mark's geometry, or replacing a mark;
- coloring Codex, Goose or the Terminal mark;
- coloring marks anywhere other than the session list's agent track;
- putting color in Rust (inline styles or per-harness classes in `HarnessIcon`) rather than CSS;
- relaxing the 3:1 contrast floor to fit an official color;
- moving an official hue (rather than only its per-theme lightness).

Decisions to log: every color chosen, with its source, date, contrast in both themes and nearest neighbor with its
CIEDE2000 distance; which OMP gradient stop was used and why; every terms re-check that changed a conclusion; every test
changed and why; and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-colored-harness-marks-log.md` in the parent directory of the checkout you run in, derived as that section
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
without costing quality (the per-brand research in parallel, say). Concurrent agents that might otherwise conflict can
work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a sub-agent
natively, with model and effort set explicitly where the mechanism allows; when the model you need is not available as a
native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

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

- One reviewable commit, stable bookmark and draft PR, using the `jjstack` skill. The stack's base is `main@origin`, or
  this plan's own open PR when it resumes, set up per `plans/.agents/executing.md`, "Executing one plan" (step 6); never
  another plan's PRs. The bookmark is `plan/colored-harness-marks/01-colored-marks`, as the outline names it.
- Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and the changelog fragment in the same commit.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- The PR stays a draft. Never mark it ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing the PR, launch a review of its changes and address what it finds before moving on. The user demands
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

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). Ask the reviewer to check the recorded color sources, contrast numbers and pairwise distance
table against the CSS as well. When the PR changes tests or their fixtures, include the full text of
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
Wording in this file that turns out inaccurate (a line number, a file name, a color value) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing what can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question. Expect
at least these there: the final color set with its sources, any pair below the CIEDE2000 target and why, any brand whose
terms changed on re-check, and that the docs website's session-list screenshots show the old colors until the next docs
screenshot refresh.

## Done criterion

The plan is complete when a draft PR exists that meets the acceptance criteria above, it passed its review gate, and it
removes this plan's TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its
report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
