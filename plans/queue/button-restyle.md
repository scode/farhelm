# Restyle buttons and button-like controls ("9 + 1")

Written against main 62d8d4e6 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term"). Verbatim:

> **Restyle buttons and button-like controls.** Buttons lose their outlines and take a translucent tint of the text
> color (the solid blue and red stay, without edges), with 5-pixel corners and Medium-weight labels (bundling Inter
> Medium); split-button halves and the terminal text size control are joined by 1-pixel gaps, and the session toolbar
> grows to 26-pixel buttons. Choice chips follow the buttons; segmented controls keep their outlined container. Designed
> with the maintainer on 2026-11-22 against browser mockups, and validated through the GPUI goldens, with a new
> reference picture of the session toolbar. Plan: `plans/queue/button-restyle.md`.

In product terms: every ordinary button in the desktop app (Cancel, Close, Stop, Restart, Copy, Retry, the blue Save,
Launch and New, the red Confirm delete and Delete, and so on) stops drawing an outline. Ordinary buttons get a faint
translucent fill that shows on every background they sit on; blue and red buttons keep their solid fill without an edge.
Corners go from 3 to 5 pixels across the app, and the labels of anything you press to choose get Medium weight. The
buttons at the top of an open session become 26 pixels tall with 12.5-pixel labels and 14-pixel icons. The launcher's
agent chips, the host icon picker and the host filter chip follow the buttons. Segmented controls keep their outlined
container but get rounder corners and Medium labels. Deliberately outlined buttons, fields and the Settings switch keep
their outlines.

The design was settled on two browser mockups; this file states it in words, because you cannot see them. Where this
file gives a number, it is the decision.

### Acceptance criteria

1. Inter Medium (Inter 4.1, font version 4.001) is bundled and registered beside Regular and SemiBold, with its source
   and hash recorded in `THIRD_PARTY_NOTICES.md`. The four places that already ask for Medium (see "Where it stands")
   render in it.
2. Every button tier and button-like component draws as the per-tier table under "The design, in full" says, in both
   themes, including rest, hover, held (mouse down), disabled, busy, chosen and open looks.
3. `metrics::RADIUS_SM` is 5 pixels app-wide; checkboxes and key caps keep 3 pixels through a separate constant; the
   segmented controls' container is 7 pixels.
4. The session toolbar and the marked-sessions bar use the 26-pixel toolbar proportions; split halves and the text size
   control are joined by 1-pixel gaps; SPEC.md's sentence about the text size control's shared border is updated.
5. The terminal drag notice's dismiss × looks like a ghost button while keeping its current focus and input behavior
   (SPEC.md, the drag notice's "Dismissal preserves terminal focus and sends no input").
6. A golden scene draws the real session toolbar, added in today's look before the restyle so the restyle shows on the
   changes page; the `buttons` scene gains the per-state looks; every golden reference is updated and published, and the
   changes page has been reviewed with no unintended change left.
7. In the final toolbar reference pictures (dark and light), the vertical center of the capital letters of a label
   without descenders (Restart, Clone, Delete) is within one device pixel of the center of its button's fill, and each
   icon's center likewise. The measurement is a one-off check recorded in the PR description and the working log, not a
   committed harness.
8. The tests that pin today's geometry and colors are updated deliberately (listed under "Where it stands"), docs that
   state today's numbers are corrected, one changelog fragment covers the restyle, and the last PR removes the TODO.md
   entry above.

### Where it stands (from planning research at main 62d8d4e6; line numbers approximate)

Paths below are under `crates/gui/farhelm-gui/src/` unless they start at the repository root.

- **Theme** (`theme.rs`): `Palette` holds the dark and light tokens (`bg_1`, `bg_2`, `control_hover`, `border`,
  `border_dim`, `well`, `accent_fill`, `accent_fill_hover`, `accent_edge`, `danger_bg`, `danger_fill_hover`,
  `danger_soft`, `fg_0`, `fg_2`, …); `rgba_token` already exists for translucent tokens (`warn_chip_bg`, `danger_wash`).
  `metrics` (~497): `RADIUS_SM` 3 ("controls and inputs"), `RADIUS_MD` 4 ("panels, cards, dialogs, tooltips"),
  `RADIUS_LG` 8 (the switch track), `TEXT_SM` 12.
- **Buttons** (`components/button.rs`): the `Tier` enum (26 tiers) and `Button::colors()` (~690-860) returning rest
  fill, hover fill, edge and ink per tier and state, including the open look (`expanded`), the chosen look (`pressed`,
  which means "the choice in force", not mouse down), the muted split caret and the "retired" look (`bg_2`,
  `border_dim`, `fg_2`) for disabled, busy or refused Primary and Danger and for busy Neutral. Every tier draws
  `border_1`, with a transparent edge where it has none, so all tiers are the same size. Disabled Ghost, Neutral,
  OnBanner, RowMenu, Rail, Chip, Link, Readout, Nav, QuickMark, NumberKey, TemplateRow, BrowseRow, FieldRemove,
  IconChoice, ColorChoice, HostUpdate and Waiting are drawn at `opacity(0.5)` (~1203-1227). No tier draws a mouse-down
  state today. Geometry: `BUTTON_PAD` 6×10 ordinary; compact (`COMPACT_PAD` 2×8, a 17-pixel line, `COMPACT_ICON` 13,
  `COMPACT_GAP`, ~246-257), condensed (`Condensed::Words` 21 pixels, 11-pixel text, ~259-270, 483-498), `TOOLBAR_HEIGHT`
  26 with 3×7 padding for the marked bar's Stop and Delete (`Button::toolbar()`, ~236-242), `Button::compact_height`,
  split parts (`SplitPart::Main`/`Caret`, corner squaring ~1077-1085, caret padding `CARET_PAD_X`).
- **Compact users outside the session toolbar** (keep their size; only colors, corners and weight change): `welcome.rs`,
  `announcements.rs`, `sign_in.rs`, `sign_in_form.rs`, `hosts.rs` (Update all), `session_list/groups.rs`,
  `session_list/attention.rs`, and others the compiler finds.
- **Session toolbar** (`session_pane/header.rs`, `session_pane/header/fit.rs`): Stop, Restart▾, Replace▾, Clone, Delete,
  Suspend/Resume; whether the row condenses is measured every frame from widths (`fit.rs`, "Measured, not remembered"),
  so there are no breakpoint constants. The header duplicates the compact spinner size and gap (~1280-1283, from
  `button.rs` ~254-257) for the environment control's pending state, and Stop's "Stopping…" spinner is a hard-coded 12
  pixels (~1189). Docs state "about 23 pixels" (~105-107).
- **Marked-sessions bar** (`session_list/marking.rs` ~700): Stop and Delete with `.toolbar()`.
- **Text size control** (`terminal/text_size.rs` ~189-229): a bordered `bg_2` box around two ghost magnifier buttons and
  a display-only readout, exactly `Button::compact_height` tall; its docs say "23 and 21 pixels", "13-pixel icon" and
  "sharing one border". SPEC.md (the terminal text size bullet, ~3287-3292) says "The buttons share one border with a
  fixed-width, display-only readout".
- **Radio groups** (`components/radio_group.rs`): Segments, SmallSegments, CompactSegments and HeadingSegments draw a
  container on `well` with a `border_dim` edge at `RADIUS_MD` and 2-pixel padding (~755-766), segments at `RADIUS_SM`,
  chosen `accent_fill` in SemiBold, unchosen transparent with a `control_hover` hover (~840-922); Chips draw `well` with
  a `border` edge (`CHIP_EDGE` 1) at `RADIUS_SM`, chosen `accent_fill`, no hover fill (~939-1003); Cards, Radios and
  RadioRows (titles already ask for Medium, ~662).
- **Other components**: `checkbox.rs` (box `RADIUS_SM` ~167, its focus ring `RADIUS_SM + 3` ~244; the switch at
  `RADIUS_LG`), `key_chip.rs` (~16), `sequence_pill.rs` (~154), `tab_strip.rs`, `menu_item.rs`, `option_row.rs`,
  `select.rs`, `list_row.rs`. The Settings search shortcut cap (`settings/search.rs` ~603).
- **Drag notice** (`terminal/drag_notice.rs` ~174-219): the dismiss × is a raw element on purpose: a `Button` is a tab
  stop and takes focus on a Linux click, while SPEC.md requires that dismissal preserves terminal focus; the element
  occludes and stops mouse-down propagation. `terminal/attached/tests.rs` (~7585-7615) pins this.
- **Medium already requested** (rendered as Regular today, because only Regular and SemiBold are bundled):
  `settings/card.rs` ~124 (Settings card titles), `components/radio_group.rs` ~662 (radio-row titles),
  `hosts/settings.rs` ~1287 (host settings section titles), `launcher/view/more_options.rs` ~202 ("More options").
- **Fonts** (`assets.rs` ~114-122): `ui_fonts()` registers `assets/fonts/Inter-Regular.ttf` and `Inter-SemiBold.ttf`;
  both are Inter 4.1 (name table "Version 4.001;git-9221beed3"). `THIRD_PARTY_NOTICES.md`, "Inter interface font", says
  they are decompressed from the website's vendored WOFF2 files; its JetBrains Mono section records a face copied from a
  named release archive with SHA-256s, which is the precedent this plan follows for Medium.
- **Goldens** (`goldens/`, manifest `crates/gui/farhelm-gui/tests/goldens.txt`): about 205 scenes in light and dark,
  rendered at scale 2 on Linux; `buttons` (`goldens/foundations.rs` ~128), `segmented-control`, `radio-group-variants`,
  `switch`, `checkbox`, `tab-strip`, launcher, Settings and dialog scenes exist. No scene draws the session toolbar or
  the drag notice. `header_tests.rs` has `RealPane` and `open_tunnel` helpers a scene may reuse.
- **Tests that pin today's looks** (update deliberately, never loosen): `components/tests/button.rs` ~670-727 (compact
  widths 68/56/52, heights 22/23/21, caret widths 23/15) and ~295-330 (`a_muted_caret_reads_retired_until_it_opens`, the
  retired look) and other `colors()` tuple asserts there; `session_pane/header_tests.rs` ~3341-3393 (the text size group
  spans Stop's top and bottom in both looks), ~2919 (the environment control keeps one width through its states), and
  the 650-pixel tests (~2263, 2583, 2919; SPEC.md's floor for the session's top row).
- **Docs stating today's numbers**: `session_pane/header.rs` ~105-107, `terminal/text_size.rs` ~189-197, `button.rs`
  ~7-8 and ~296-298 ("joined with no gap"), ~483-489 (23/21/19), `theme.rs` ~497-503 (radius roles).

## The design, in full

### Tokens

New `Palette` fields (use `rgba_token`), both themes:

| Role                           | Dark                                                          | Light                |
| ------------------------------ | ------------------------------------------------------------- | -------------------- |
| button tint, rest              | white at 6.5%                                                 | `#1b2130` at 6%      |
| button tint, hover             | white at 11%                                                  | `#1b2130` at 10.5%   |
| button tint, held (mouse down) | white at 8.5%                                                 | `#1b2130` at 14%     |
| button tint, retired           | white at 4%                                                   | `#1b2130` at 3.5%    |
| primary button hover           | `#5c72da` (lighter)                                           | `#4059c7` (as today) |
| danger button hover            | `#b44757` (lighter)                                           | `#983644` (as today) |
| primary and danger held        | `accent_fill_hover` and `danger_fill_hover`, as today's hover | same                 |

The tint is translucent on purpose: buttons sit on several backgrounds (the toolbar, sidebar and dialog footers on
`bg_1`; dialog bodies and question panels on `bg_2`, which in dark mode is exactly today's button fill and in light mode
is white), and an opaque fill without an outline vanishes on one of them. The primary and danger hovers are button-only
tokens: `accent_fill_hover` is also the open look's fill, a chosen segment's hover and the selected row menu's hover,
and `danger_fill_hover` is the red banner button's hover, so neither shared token changes.

### Corners

`RADIUS_SM` becomes 5 (app-wide: buttons, menu items, option rows, selects, inputs, chips, segments, cards that use it).
A new constant at 3 pixels keeps checkboxes (the box and its focus ring, which stays the box's radius plus 3) and key
caps (`key_chip.rs`, `Tier::NumberKey`, the Settings search shortcut cap) at 3. The quick-launch sequence pill's chips
are names, not key caps, and take 5. The segmented controls' container takes 7 (a new constant), so the 2-pixel gap
around 5-pixel segments stays even. `RADIUS_MD` (4, panels and dialogs) does not change; update the `metrics` docs so
the roles no longer imply that controls are rounder than panels by accident, and expect surfaces that use `RADIUS_SM`
(the interrupted-session card, scroll boxes, command wells, approval card parts, Welcome and Send feedback parts) to
show rounder corners on the changes page as intended.

### Weight

Labels of anything you press to choose are Medium: every `Button` tier that shows words except Link, Readout, Nav,
TemplateRow, BrowseRow, Rail and Waiting (which keep their weight; Waiting's content is a laid-out row), plus
radio-group chips, unchosen segments (chosen segments stay SemiBold), `tab_strip.rs` tabs and terminal tabs. Menu items,
option rows and list rows stay Regular. The four places that already ask for Medium keep asking for it and now render
it.

### Per tier (`components/button.rs`)

"Unchanged" means colors and edge as today; every tier still gets the new `RADIUS_SM` unless it says otherwise, and the
Medium rule above.

| Tier                                                                                                    | Edge                       | Fill (rest / hover / held)           | Notes                                                                                       |
| ------------------------------------------------------------------------------------------------------- | -------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------- |
| Neutral                                                                                                 | none                       | tint rest / tint hover / tint held   | ink `fg_0`; disabled keeps today's 0.5 opacity over the rest look; busy is the retired look |
| Primary                                                                                                 | none                       | `accent_fill` / primary hover / held | disabled, busy, refused: retired look                                                       |
| Danger                                                                                                  | none                       | `danger_bg` / danger hover / held    | disabled, busy, refused: retired look                                                       |
| Retired look                                                                                            | none                       | tint retired                         | ink `fg_2`; also the muted split caret until it opens                                       |
| Chip (host filter chip)                                                                                 | none                       | tint rest / tint hover / tint held   |                                                                                             |
| IconChoice                                                                                              | none                       | tint rest / tint hover / tint held   | chosen: `accent_fill`, no edge                                                              |
| Ghost, DangerGhost, RowMenu, FieldRemove, QuickMark, TerminalTab, TerminalTabClose, Link, Readout, Rail | unchanged                  | unchanged                            | TerminalTab Medium                                                                          |
| DangerOutline, PrimaryOutline, HostUpdate                                                               | kept (outlined on purpose) | unchanged                            | they keep the outline when disabled or busy, as today                                       |
| OnBanner                                                                                                | kept (contrast on red)     | unchanged                            |                                                                                             |
| Tunnel                                                                                                  | kept                       | unchanged                            | keeps its 8-pixel corner                                                                    |
| Waiting                                                                                                 | kept (amber)               | unchanged                            | keeps its corner and weight                                                                 |
| NumberKey                                                                                               | kept (solid or dashed)     | unchanged                            | 3-pixel corner (key cap)                                                                    |
| BrowseRow                                                                                               | kept (a field)             | unchanged                            |                                                                                             |
| Nav, TemplateRow (incl. chosen looks)                                                                   | unchanged                  | unchanged                            | Regular                                                                                     |
| ColorChoice                                                                                             | unchanged                  | unchanged                            | a circle                                                                                    |
| Open look (`expanded`)                                                                                  | none (as today)            | `accent_fill_hover` (as today)       |                                                                                             |

The held fill is new: draw it as GPUI's active style. Name it `held` (or `active`) in code, never `pressed`, which
already means "the choice in force". Unit tests cover it; goldens cannot.

### Joined controls

Split halves (Restart▾, Replace▾) are separated by a 1-pixel gap instead of sharing a border; the outer corners stay
rounded and the inner ones square. The text size control becomes the same kind of joined group: the two magnifier
buttons in the Neutral look and the readout as a tinted, square-cornered cell between them, 1-pixel gaps, outer corners
rounded, the whole group exactly `Button::compact_height` tall in both looks. Factor the corner squaring that
`SplitPart` does today into a small "joined on this side" option that `SplitPart` sets and the magnifiers use; do not
reuse `SplitPart::Caret` for them (it carries caret padding and the muted look), and do not generalize to an N-part
group abstraction. Update SPEC.md's sentence ("The buttons share one border with …") to describe buttons joined to the
readout by 1-pixel gaps, and the docs that say "joined with no gap" or "sharing one border".

### Choice chips and segmented controls (`components/radio_group.rs`)

- Chips: the button tint (rest, hover, held) with no visible edge (keep the transparent 1-pixel edge so `CHIP_EDGE`'s
  ring math and the chip height do not move), `RADIUS_SM`, Medium; chosen `accent_fill` with the inverse ink. Brand
  marks inside the chips are unchanged. Read the same palette tokens as the buttons; keep the radio group's own drawing
  code (its keyboard and roles stay its own).
- Segments (all four segment looks): keep the container's `well` fill and `border_dim` edge; container corner 7,
  segments `RADIUS_SM` (5); unchosen labels Medium with the tint hover instead of `control_hover`; chosen as today
  (`accent_fill`, SemiBold).
- Cards, Radios and RadioRows: unchanged apart from the radius and the Medium that RadioRows' titles already ask for.

### Session toolbar proportions

The ordinary look of the session toolbar's buttons (Stop, Restart▾, Replace▾, Clone, Delete, Suspend/Resume and the text
size control) and the marked-sessions bar's Stop and Delete share one toolbar geometry: 26 pixels tall (the 1-pixel
transparent edge, 3 pixels of padding above and below, an 18-pixel line), 10 pixels of side padding, 12.5-pixel Medium
labels, 14-pixel icons with a 6-pixel gap between icon and label, and 8 pixels between toolbar buttons (the bar's gap).
Carets keep today's side padding. Icons keep their current drawings at the new size. The condensed look keeps its
current geometry (21 pixels, 11-pixel text) and takes the new colors and weight. Other `compact()` users keep their
size. Derive the header's copy of the spinner size and gap from the button's constants rather than duplicating them, and
check Stop's 12-pixel "Stopping…" spinner against the 14-pixel icon so the button keeps its width while stopping.

### Drag notice dismiss

Keep the raw element and every behavior it has (no tab stop, no focus on press, occlusion, mouse-down that stops
propagation). Give it the ghost tier's look: its padding, `RADIUS_SM`, the `control_hover` hover fill and Medium weight
if it shows text.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- The request: "suggest 10 ways we could adjust the style of our clickable buttons in farhelm while maintaining
  reasonable consistent look, not being over the top and not going for generic slop visuals but feeling more
  modern/nice", "without changing the choice of font" (Inter). From ten mocked options the maintainer chose "9 + 1":
  option 9's proportions for the session toolbar and option 1's look (no outlines, a lifted tonal fill, 1-pixel gaps in
  joined controls, solid blue and red without edges), "properly centered".
- Scope: "everywhere ... we need to find every single place in the entire app" with button-like controls. After the
  inventory: buttons outlined on purpose (the permanent "don't ask again" answers, Trash's outlined deletes, the Hosts
  row's inline Update) keep their outlines; fields (dropdowns, the launcher's browse rows, inputs) keep theirs; the
  Settings switch keeps its outline; app-wide small corners become 5 pixels with checkboxes and key caps at 3; Medium
  weight for anything you press to choose, menu items and list rows stay Regular; bundle Inter Medium rather than using
  Regular or SemiBold.
- Light mode and the translucent tint as mocked: "seems fine". Choice chips change as mocked. Segmented controls: option
  A, keep the outlined container.
- "Let's go with 9+1 properly centered", validated "by pulling a screenshot out of actual GPUI rendering so we don't
  have to deal with this later on after changes land on main": the golden suite, with the toolbar's centering measured
  in its reference pictures. A browser mock showed labels about 0.75 pixels high on a Mac browser; today's app on a Mac
  centers capitals exactly, so that was a browser artifact, not a target to correct for.
- No Mac render step: rely on the Linux GPUI render plus the centering measurement.
- Review gate: a single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- SPEC.md: the terminal text size bullet (the shared-border sentence this plan changes), the 650-pixel floor of the
  session's top row, and the drag notice's "Dismissal preserves terminal focus and sends no input".
- `THIRD_PARTY_NOTICES.md` for the bundled font's source and license.
- `.agents/validation.md`: the GPUI goldens procedure (iterate with `--scene`, one full `--update` before publishing,
  inspect the changes page, treat an unintended changed scene as a finding, publish with
  `scripts/gui-goldens.py publish`), and `python3 -B scripts/test-gui-screenshot-batch.py` when the golden manifest
  gains a scene. `.agents/validation.md`'s host-text check on a Mac applies when the bundled fonts change; per the
  maintainer's "No Mac step", skip it and say so in the report.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", "Instructions by path" and "Code rules".

**Planner proposals (from the planning review, logged so you can revisit them):**

- Bundle Medium from the Inter 4.1 release archive (decompress its `web/Inter-Medium.woff2` byte for byte, as the other
  faces were, and check its name table says "Version 4.001;git-9221beed3"), recording the archive's URL and SHA-256 in
  `THIRD_PARTY_NOTICES.md` like the JetBrains Mono precedent, instead of vendoring it into `website/public/fonts/`,
  which nothing on the website would use.
- The four existing Medium requests stay Medium: they were the original design, and the radio-row titles are something
  you press to choose.
- One toolbar geometry for the session toolbar and the marked-sessions bar, rather than two different 26-pixel looks.
- The condensed toolbar look keeps its geometry; other `compact()` users keep their size.
- The drag notice's × keeps its element and behavior and only takes the ghost look (a real `Button` would break the SPEC
  rule above).
- Disabled Neutral keeps today's 0.5 opacity over the rest look; the retired look carries its own tint.
- The toolbar golden is one real header scene (an environment session in its Resume state shows Primary, Neutral,
  Danger, both split buttons and the text size control in one row, plus the condensed look); the per-state looks
  (compact split pair, muted caret, open caret, disabled compact button, held look where a still frame can show it) go
  into the existing `buttons` scene.

## Outline

1. `plan/button-restyle/01-inter-medium`: add `assets/fonts/Inter-Medium.ttf`, register it in `assets::ui_fonts()`
   (update its docs), record its source in `THIRD_PARTY_NOTICES.md`; update and publish the goldens the four existing
   Medium requests change. Add the restyle's one changelog fragment here (`kind: changed`), which later PRs edit as they
   add to it. `style: draw medium-weight text in inter medium`.
2. `plan/button-restyle/02-toolbar-golden`: add the session toolbar scene and the per-state additions to `buttons`, in
   today's look; publish; run the screenshot-batch test. `test: add a golden of the session toolbar`.
3. `plan/button-restyle/03-buttons-without-outlines`: the tokens, the per-tier table, the held state, `RADIUS_SM` 5 with
   the 3-pixel exceptions, the Medium rule, joined controls (split gap, the text size control, SPEC.md sentence), choice
   chips and segmented controls, docs, tests, one full `--update`, the changes page reviewed, publish; edit the
   fragment. `style: draw buttons without outlines`.
4. `plan/button-restyle/04-toolbar-proportions`: the shared toolbar geometry for the session toolbar and the marked bar,
   the spinner constants, the 650-pixel tests, docs and the centering measurement (acceptance criterion 7); goldens;
   edit the fragment. `style: give the session toolbar larger buttons`.
5. `plan/button-restyle/05-drag-notice-dismiss`: the drag notice's × in the ghost look, keeping its behavior and its
   test; remove the TODO.md entry. `style: draw the drag notice's dismiss like a button`.

## Fallbacks

- Centering misses acceptance criterion 7: try the other ways of splitting 26 pixels (a 17-pixel line with 3.5 pixels of
  padding) and pick whichever centers best; if none is within one device pixel, ship the best, record the measurements
  in the report, and do not block.
- The session's top row no longer fits 650 pixels in some state with the new proportions: let it condense earlier (that
  is what condensing is for); if even the condensed row cannot fit, reduce the gap between toolbar buttons to 6 pixels
  before anything else, and block only if that still fails.
- A shared token change turns out to move something outside the restyle on the changes page: switch that use to a
  button-only token rather than accepting the drift.
- Golden publishing fails for lack of push access to the goldens storage: push what is consistent and block.
- `gpt-6.2-sol` unreachable: see Review gate.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-button-restyle-log.md` in the parent directory of the checkout you run in, derived as that section says,
and the watchdog's status file is `farhelm-plan-button-restyle-watchdog.json` beside it. Resolve both to absolute paths
before you start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". For each visual PR: iterate on the touched scenes with `python3 scripts/check-gui-goldens.py --scene …`,
then one full `--update`, read every picture on the changes page (`target/gui-goldens/changes/`) against this file's
design, fix what is unintended, and publish with `python3 scripts/gui-goldens.py publish` before the PR's commit carries
the manifest. Give the review gate the changes page directory along with the design section of this file. Targeted Rust
tests in `farhelm-gui` (`components::tests::button`, `session_pane::header_tests`, `terminal` text size and
`attached::tests` drag notice tests, radio group, checkbox and tab strip tests), `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, `python3 -B scripts/test-gui-screenshot-batch.py` when a scene is added,
and `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it. Apply
`.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live Farhelm
install on this machine (root `AGENTS.md`, "The live install is off-limits").

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Restyle buttons and button-like controls"), as
  root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides. A conflict in the golden manifest after a
  rebase means re-running `--update` and publishing again, never hand-merging blob ids.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, design,
decisions and outline). For a PR that changes pictures, also give it the changes page directory. When a PR changes tests
or their fixtures, include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md`
requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question. The docs
website's screenshots and the README hero will show the old look until their own refresh flows run; that is a follow-up
to mention in the report, not work for this plan.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.
