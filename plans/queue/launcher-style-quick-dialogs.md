# Give Cmd+K, Cmd+J and Cmd+L the session launcher's look

Written against main at fe92c6859 on 2026-11-24. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, except where this goal explicitly changes them (the spec edits it lists are part of the work).

## The goal

Implement the TODO.md entry under Immediate, **Give Cmd+K, Cmd+J and Cmd+L the session launcher's look.** The
maintainer's words when asking for it: "we've recently improved the look of the launcher, and slightly settings. and
some buttons. but there is much to do still. i'd like to tackle "dialogs". so the cmd-k dialog is very basic for example
as is the cmd-j and cmd-l. it's just very very plain and bare bones, like cmd-l has weird super wide unused space etc
it's just very much naive and not tweaked at all." Shown eight directions, they chose the first, "Launcher sibling": "i
love terminal native, but also agree that launche sibling is consistent ... for the immediate purpose, let's go for
launcher sibling." Once this look is settled, the maintainer intends to carry it to a larger set of dialogs (Settings,
for example); that later work is not part of this plan, but it is why the pieces are built as shared components.

The three dialogs are the quick switcher (Cmd+K on macOS, Ctrl+Shift+K on Linux), the template list (Cmd+J,
Ctrl+Shift+J), both drawn by `crates/gui/farhelm-gui/src/session_list/quick_switcher.rs`, and Rename (Cmd+L,
Ctrl+Shift+L), `crates/gui/farhelm-gui/src/session_list/rename.rs`.

The mockup is published at `https://snippets.scode.org/s/farhelm-dialog-directions/`, section "1 Launcher sibling" (the
page has a theme switch at the top; the "Today" section shows the current dialogs). It is the visual reference for the
visual review below. The words in this file are the design; where the mockup and these words differ, the words win. The
mockup's session and template data is illustrative.

### The agreed design, in words

**Shared look.** All three follow the session launcher as it now stands (`crates/gui/farhelm-gui/src/launcher/view.rs`
and `launcher/view/search.rs`): the app's Steel palette, Inter and the terminal font, the solid accent selection fill,
key chips drawn with `key_chip`, and no new colors (a color the theme lacks is added in `theme.rs` as a named role,
never written in a view). Shortcut names in key chips follow the platform, through the existing shortcut-name constants
(`actions.rs`, `templates`), never a hard-coded ⌘.

**The field at the top.** Each dialog opens with the launcher's command field: about 48 pixels tall with 14.5-pixel
text, a 24-pixel square icon tile at its start (as the launcher's rocket tile is drawn), and a small key chip at its end
naming the dialog's own shortcut. Close (×) sits at the end of the field's row, as in the launcher. The icon is search
for Cmd+K, the templates icon for Cmd+J, and an edit (pencil) icon for Rename; use an existing icon where one fits, and
add one to the icon set only if none does. Placeholders: Cmd+K "Jump to a session, or type a name to start one…"; Cmd+J
"Start a template…". Focus starts in the field, as today, and its focus ring shows while it has focus.

**Hint chips (Cmd+K and Cmd+J).** Under the field, while it is empty, a row of hint chips in the launcher's style (an
example in the terminal font, in the accent ink, followed by what it does). Cmd+K: `tl:` "search templates", and
`fix login bug` plus a ⏎ key chip "on no match starts a session with that name". Cmd+J: ⚡ "quick launch" with the ⚡
shortcut's key chip, and "number keys" with the number-key chips' first and last forms. The row hides as soon as the
field has text, as the launcher's does. These chips replace the pinned templates hint section ("Templates" / "Type tl:
to search templates. Picking one opens New with it applied.", and in the templates section "Picking a template opens New
with it applied; nothing launches yet.") and Cmd+J's hint sentence ("Enter starts the highlighted template. A partial
template opens New to complete it."); those sections go.

**Section labels.** Small uppercase labels, as the launcher's RECENT and TEMPLATES are drawn: Cmd+K "RECENT SESSIONS"
with nothing typed and "SESSIONS" while typing, "TEMPLATES" for a `tl:` query and in Cmd+J. A quiet count sits at the
label row's right end (Cmd+K with nothing typed: the number of sessions and "every host"; Cmd+J: the number of
templates). The listing-cap notice and the loading and failed-read messages keep their words and their place in the
list.

**Cmd+K session rows.** Two lines: the title (medium weight), then `host:path` in the terminal font in a quieter ink,
with home abbreviated as the sidebar does. On the left, the session's harness mark (the same mark, size rule and color
treatment the session list uses, `crates/gui/farhelm-gui/src/harness.rs` and `marks.rs`). On the right, in this order:
the host's chosen mark (as today's row and the sidebar draw it), the status glyph, and the age in the terminal font (the
sidebar's own age formatter, `session_list/age.rs`, against the ticker's clock, as the sidebar uses it). The status
glyph is the sidebar's: its live status mark, or `status::EndedGlyph` for an ended session (today's switcher draws none
there). The agent's name, which today is a word in the row, moves into the row's hover help. Every row is the same
height; an ended session's annotation ("Exited — stopped by user") goes into the hover help rather than a third line.
Matching characters keep today's marks (accent, weight 600) in each drawn value. The selected row takes the accent fill
with white ink and white marks, as today and as the launcher's rows do.

**Cmd+K pinned rows.** The New session row and the Quick launch row keep their behavior and their place below the
scrolling matches, separated from them by a hairline, drawn as one-line rows in the same row style: Quick launch as the
launcher's ⚡ icon (`Icon::BoltFilled` in `warn_bright`, replacing today's literal ⚡ character), a quiet "Quick launch"
and the template's name (or "Mark a template"), and its key chip at the right end; New as "New session named "X"" with
its own icon. Template rows in a `tl:` query are drawn as Cmd+J's rows are, without the key chips and the tag (SPEC.md
keeps the chips to Cmd+J).

**Cmd+J rows.** Two lines: the template's harness mark (the rule the launcher's template rows use, moved into
`templates`; see the planner proposals), the name (medium weight), and its summary on the second line, cut with an
ellipsis when long. On the right: an "opens New" tag on a template that needs more choices before it can launch, the ⚡
icon when it is the quick-launch template, and its number key chip when it has one. Whether a template opens New is
decided synchronously from the template's own fields by the rule the quick-launch dispatch itself applies
(`farhelm_proto::launcher::quick_launch_eligible`, which `templates/dispatch.rs` reaches through
`stacked_quick_launch_eligible`), not by the ⚡ marking rule `templates::shortcuts::may_mark`, even though the two are
the same code today; carry it on `templates::rows::Row`. The tag does not try to predict the rarer cases where a later
check (the catalog, a folder) still sends an eligible template to New.

**Footer (Cmd+K and Cmd+J).** A strip on the raised ground with key chips and their words: "↑ ↓ move", "⏎ open" (Cmd+J:
"⏎ start"), "esc close". It replaces today's "↑↓ choose · Enter pick · Escape close" line and the Close button. Mouse
users close with × or a click outside, as today. Tab goes from the field to ×.

**Rename (Cmd+L).** About 440 pixels wide. The title field is the top of the dialog, with the pencil tile at its start
and × at the row's end. Its text is selected on opening only in naming mode, as today and as SPEC.md says ("Only that
mode selects the whole initial name"). Under it, one line names the session being renamed: its harness mark and agent
name, its host mark and `host:path` in the terminal font, and its status glyph and status word. When the store holds no
row for the session (naming mode before the listing has it, or after the session is gone), the line is left out rather
than drawn partly. The footer holds a quiet "Rename session" on the left (the frame's existing footer `note`) and Cancel
and Save on the right, Save carrying a ⏎ key chip as the launcher's main action does. The header band ("Rename session"
/ "Change this session's title") and the current-title label above the field go. The dialog's accessible name stays
"Rename session", and the field's stays "New session title". The Tab order stays Close, the title field, Cancel, Save,
as SPEC.md requires: this is a deliberate exception to Tab following visual order, since Close is drawn at the end of
the field's row. The unavailable line, the refusal and every existing behavior (the title following pushes, naming mode
after a quick launch, Escape, focus return) stay.

**Rename group and New group.** The same editor draws them (`Subject::Group`, `Subject::NewGroup`). They take the same
look with their own words: the field with its tile, "Rename group" or "New group" as the quiet footer label, and no
context line.

**Sizes.** Cmd+K's card is 640 pixels wide at most (today 760), Cmd+J's 560, Rename 440, each narrowing with the window
as today. The window's minimum width is 720.

### Where it stands (from planning, at main fe92c6859)

Line numbers are approximate.

- `session_list/quick_switcher.rs` draws its own card rather than the shared `DialogFrame`, because the old switcher had
  neither the frame's title header nor its answer footer (its module docs). The new design gives it both a control
  header and a footer band, which is what the planner proposals below act on. The card is about lines 1075-1104, the
  field 877-895 (placeholders 535-538), the section heading `quiet()` 897-906, session rows `result_row` 1437-1561 on
  the `Columns` grid 1414-1430, template rows `template_row` 1280-1325, the New row `new_row` 1192-1229, the Quick
  launch row `quick_row` 1335-1378, the templates hint `templates_hint` 1383-1407, the cap notice 1043-1048, and the
  footer with its Close button 1050-1073.
- Session rows come from `SessionRow` (`crates/farhelm-proto/src/ui_channel.rs`) and carry status, `launch` and
  `agent_kind`, the host id, name and staleness, `cwd`, and `last_activity_at` / `effective_activity()`. The switcher's
  snapshot is read once per opening and is not live.
- `session_list/row_marks.rs` (`RowMarks::of`, `agent_track`) and `session_list/age.rs` (`age::of`) are reachable from
  the switcher, a child module of `session_list`. The sidebar holds the ticker's 30-second clock for ages
  (`session_list.rs`, about line 990).
- In the launcher, `Launcher::command_header` (`launcher/view/search.rs`, about 651-744) builds the field from the
  shared `TextInput` with `line_prefix` (the tile, `launcher-command-mark`), `line_suffix` (a `key_chip`) and
  `line_text_size`; `command_hints` (about 799-854) builds the hint chips and passes them as the `detail` of
  `DialogFrame::heading_control`; `command_heading` (about 1051-1070) draws the uppercase labels; `navigation_hint`
  (`launcher/view/search/start.rs`, about 405-423) draws "↑ ↓ to move, ⏎ to take" as key chips and words. The rows
  inside the shared `OptionRow` are `recent_choice` (about 439-537, three lines: agent with an optional YOLO chip, then
  `host:folder` in the terminal font, then the choices), `typed_choice` (about 552-690, the typed results, a plain quiet
  detail line) and `template_choice` (about 724-778, mark, name, `summary::line`, ⚡ and number chip); no launcher row
  has a fixed terminal-font second line. The template mark comes from the private `Launcher::template_mark`. The main
  action's ⏎ key chip is built inline with `Button::content` (`launcher/view.rs`, about 2847-2853). The launcher has no
  key-chip footer strip; its footer is the frame's answer row. All but `key_chip`, `TextInput`, `OptionRow`, `Button`
  and `DialogFrame::close` are private to the launcher.
- `components/option_row.rs` has `option_heading` (12 pixels, not uppercase), a near twin of `command_heading` used by
  other lists.
- Rename uses the shared `DialogFrame` (`components/dialog.rs`), which many dialogs use. `heading_control` (a control in
  the header band, with `close` at the row's end and an optional detail line) and the footer `note` give it most of the
  new look. But `heading_control`'s contract is that Tab visits the control before Close, and GPUI's Tab order follows
  draw order, so SPEC.md's Close-first order needs one new frame option (the smallest shape: a `heading_control` flag
  that puts Close first in Tab order while it is still drawn at the row's end). Its body draws the current title above
  the field (`rename.rs`, about 557-593). The same editor draws Rename group and New group.
- The frame's footer takes `footer_lead` and draws no footer at all with no answers, note, lead or notices (`dialog.rs`,
  about 539-543). `DialogPlacement::Top` in `layers.rs` already anticipates a frame placed at the top ("fits only if its
  root and the frame both shrink").
- Template rows are `templates::rows::rows` (`templates/rows.rs`), shared with the launcher's template search.
- Goldens: `rename-dialog` and `template-launcher` (Cmd+J) exist (`crates/gui/farhelm-gui/src/goldens/mod.rs`,
  `goldens/session_list.rs`, `goldens/templates.rs`). No golden opens the quick switcher listing sessions; the first
  Cmd+K PR adds one. A planning-time trial showed how: open the switcher in `Mode::Sessions` once the fleet's list has
  drawn, and answer the fake helm's commands in order from a callback deferred out of the draw (answering inside the
  draw panics on re-entry, and answering only the read fails the fake's in-order rule).
- Tests that pin today's structure, which change with the design: `session_list/quick_switcher/tests.rs` (the "Recent
  sessions" text, the Close button `quick-switcher-close` and the Tab order through it, the hint words, the
  placeholder), `quick_switcher/template_list_tests.rs` (the hint sentence, key chips per row),
  `session_list/rename/tests.rs` (the current-title label following pushes, `rename-body`), and
  `components/tests/dialog.rs`, `session_list/groups_tests.rs`, `templates/naming_tests.rs` where they look at Rename.
- Element ids used outside the crate: `scripts/gui-e2e-smoke.py` clicks `id:quick-switcher-new`; the docs screenshot
  batch (`docs/gpui/screenshot-scenes.json`, governed by `docs/gpui/screenshot-batch.md`) drives Rename and the quick
  switcher (`app-rename-dialog`, `app-quick-switcher`, `id:session-menu-rename`).
- SPEC.md, "Desktop window chrome" (the Cmd+K and Cmd+J paragraphs, about lines 3961-4002) says rows show "status,
  agent, host (with its chosen mark) and directory", that "a pinned templates section normally shows only a `tl:` hint",
  and that Cmd+J rows "show the template name, summary and its ⚡ or number shortcut". SPEC.md, "Lifecycle operations"
  (about line 2135) says Rename's editor "has a shared title and Close header, a scrolling body containing the current
  title, draft and refusal, and a Cancel/Save footer"; the next sentence (the Tab order) and the naming-mode selection
  rule (about 2139) stay. The group paragraphs ("**New group…** opens a naming …") describe the same editor. The Cmd+L
  shortcut paragraph (about 3949) is unchanged. SPEC_impl.md mentions the switcher's card and rows (about lines 741-744,
  815, 947-951, 1216).
- User docs that describe these dialogs: `website/src/content/docs/docs/using/session-list.mdx` ("Jump with the
  keyboard", about line 206), `launch-templates.mdx`, `keyboard-shortcuts.mdx`.

### Acceptance criteria

1. The quick switcher, the template list and Rename look as "The agreed design, in words" says, in both appearances.
2. Every behavior they had is unchanged: matching, ranking and marks; the first match selected; the arrows, Enter and
   click; the New session and Quick launch rows and their reachability rules; `tl:` template search; Cmd+J's exact-name
   choice, its quick-launch path and its errors; loading, failed-read and cap messages; Escape, a click outside and
   focus return; the chords' refusals; Rename's pushes, refusal, unavailable line and naming mode. Tab follows the new
   visual order (Rename's stays as SPEC.md gives it).
3. The pieces under "Shared pieces" in the planner proposals are shared components under
   `crates/gui/farhelm-gui/src/components/` (or, for the template mark rule, in `templates`), the session launcher is
   drawn with the ones it has a counterpart for (the field dressing, the chips-and-words item for its hints and its
   navigation hint, the section label, the row content for its recent, typed and template rows, the template mark rule),
   and its golden pictures are unchanged. The footer strip is new and is used by Cmd+K and Cmd+J.
4. Golden scenes exist, light and dark, for: Cmd+K with nothing typed (the fleet of every status, with the Quick launch
   row); Cmd+K while typing (marks on matches, with the New session row); Cmd+K with a `tl:` query; Cmd+J (with a ⚡
   template, number keys and a template that opens New); Rename; and New group. The visual review below passes for each.
5. SPEC.md's Cmd+K and Cmd+J paragraphs describe what rows now show (agent as its mark, and the age), the hint chips in
   place of the pinned templates hint section, and Cmd+J's "opens New" tag; SPEC.md's Rename editor sentence (about line
   2135) and the group naming paragraphs describe the new layout, keeping the Tab order and naming-mode rules as they
   are; SPEC_impl.md wherever it describes the switcher's card, rows or footer, or Rename's body. The user docs' words
   match where they describe these dialogs' looks; their screenshots are left for the next docs refresh, and the report
   says which are stale.
6. The e2e smoke script and the docs screenshot batch still find what they target (keep the ids, or update their users
   in the same PR, reading `docs/gpui/screenshot-batch.md` and, for `scripts/docs-shots/`, `.agents/docs-media.md`
   first).
7. The last PR removes the TODO.md entry ("Give Cmd+K, Cmd+J and Cmd+L the session launcher's look.").

## Decisions already made

Settled with the maintainer on 2026-11-24, on the mockup page and the planning page:

- **Direction 1, "Launcher sibling"**, as described above. Nothing from the other directions: no `tl:` token in the
  field (direction 5) and no template preview pane in Cmd+J (direction 4).
- **Cmd+K rows as mocked:** two lines, the agent as its mark (its name in the hover help), and the age added.
- **Hint chips replace the pinned templates hint section** and Cmd+J's hint sentence.
- **Footer:** a key-chip strip; the Close button goes, × beside the field stays.
- **Cmd+J shows an "opens New" tag** on templates that need more choices, decided by the existing synchronous check.
- **Rename as mocked:** the field at the top with its tile and ×, a context line, a quiet "Rename session" in the footer
  beside Cancel and Save ⏎; no header band, no current-title label. The maintainer saw the planning page say the Tab
  order stays × → field → Cancel → Save.
- **Shared components:** the pieces move into `components/`, and the launcher is switched to them without changing its
  pictures, so Settings and other dialogs can use them next.
- **Review gates:** a single fresh-context `gpt-6.2-sol` agent at `high` effort per PR, and, in addition, the mandatory
  visual review by `opus` at `high` effort described under "Visual review" (the maintainer's standing rule for GUI
  plans, chosen again on this plan's page).
- **Release note:** none (see "Release note" below).
- **No-workhorse mode:** the plans default, stated on the planning page.

Planner decisions:

- The sizes under "Sizes", taken from the mockup.
- The ⏎ key chip on Rename's Save, as the launcher's main action has one.
- The ended-session annotation moves to the hover help so every row keeps one height.
- The uppercase label: `command_heading` becomes a shared label beside `option_heading` (with an optional trailing
  count); `option_heading`'s other callers keep their look.
- Rename group and New group take the new look with their own words and no context line, since the same editor draws
  them and keeping two looks in one editor would be the larger change. The maintainer did not see these in the mockup;
  the report says how they came out.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the words quoted above, and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md",
  "Code rules" (no per-harness branching at the call site: harness marks and names come from the places
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` and `harness.rs` name), and its "Instructions by path" for every
  path you touch: `docs/harness-marks.md` before touching `marks.rs` or `marks/`, `website/AGENTS.md` before touching
  `website/`, `.agents/docs-media.md` before touching `scripts/docs-shots/`.
- `.agents/validation.md` for the GUI test selections and the screenshot goldens (re-capturing them with
  `scripts/check-gui-goldens.py --update`, inspecting every changed picture, and publishing them with
  `python3 scripts/gui-goldens.py publish` before any rebase, as it describes).
- `.agents/test-authoring.md` for every test added or changed.
- SPEC.md "Desktop window chrome" (the three shortcuts' paragraphs), "Lifecycle operations" (Rename's editor, its Tab
  order and naming mode) and "Keyboard focus"; the GUI architecture rules in SPEC_impl.md ("GUI architecture"),
  including "Layout costs show only at scale": a repeated item with text gets a computed width and `flex_none()` rather
  than `flex_1`, which is why the switcher computes its column widths (`Columns::for_card`) and why the shared row
  content must accept a computed text width for Cmd+K, which lists up to the helm's listing cap; the theme module's rule
  that views never hold a color (`crates/gui/farhelm-gui/src/theme.rs`, module docs).

**Planner proposals** (after a fresh-context planning review):

- **Shared pieces.** Each is the smallest thing the uses in hand need:
  - The field dressing: a 24-pixel icon tile function plus a small helper that dresses a `TextInput` with `height(48)`,
    `line_text_size(14.5)`, the tile as `line_prefix` and an optional key chip as `line_suffix`. Not the launcher's
    whole `command_header`, whose list follow, anchor tracker, notices, mode summaries and host lookup stay the
    launcher's. No new input widget.
  - One chips-and-words item: one or more chips (an example in the terminal font and accent ink, or a key chip in the
    quiet ink), then a word or two, in a wrapping row. It draws the launcher's hints, the launcher's `navigation_hint`,
    Cmd+K's and Cmd+J's hints, and the footer strip's contents.
  - The uppercase section label, as above.
  - Row content whose lines are elements (a title line, then one or more quiet lines), with a leading mark and trailing
    elements, inside the existing `OptionRow`, accepting a computed text width. It draws the launcher's `recent_choice`,
    `typed_choice` and `template_choice` content and the switcher's rows; no second row component and no second
    selection or focus mechanism. Cmd+J's short list may use flexible widths; Cmd+K uses computed ones.
  - The template mark rule moves out of `Launcher::template_mark` into `templates` as a function over the template's
    fields; `templates::rows::Row` carries the mark and the "opens New" flag, and the launcher calls the same function
    at both of its sites. Whether one function also draws the template row's content for all three uses is your call.
  - A "verb + ⏎" button content helper for Save and the launcher's main action is a close call either way; log the
    choice.
- **Cmd+K and Cmd+J move onto `DialogFrame`**: `heading_control` with the field and its hint chips as the detail,
  `close`, a body of the switcher's own (the scrolling results plus the pinned New and Quick launch rows in one list),
  and `footer_lead` for the strip, with no answers. This reuses the frame's header band, × placement and footer band
  rather than redrawing them in the switcher's card, and `heading_control`'s Tab order (field, then ×) is the one wanted
  here. Check that the frame shrinks correctly at `DialogPlacement::Top` without a fixed height, that only the results
  scroll, and the side margin (the frame keeps 20 pixels, the switcher today 16); rewrite the switcher's module docs.
  Fallback: keep the switcher's own card and factor the frame's control-header and footer-band drawing into `pub(crate)`
  pieces both call. A hand-drawn copy of either band is not acceptable.
- **Rename keeps the shared `DialogFrame`**, with `heading_control`, `close`, the footer `note` and one new frame option
  for Close-first Tab order. A frame of Rename's own is a scope reassessment.
- The "opens New" decision is computed in `templates::rows` by the dispatch's own rule, not by a second implementation.
- The age reuses `age::of` and the ticker's clock exactly as the sidebar does.

## Outline

1. `plan/launcher-style-quick-dialogs/01-shared-command-pieces`: the shared pieces (the field dressing, the
   chips-and-words item, the section label, the row content, the template mark rule) out of the launcher, and the
   launcher drawn with them. The launcher's goldens must not change. Optionally, the Cmd+K sessions-mode golden fixture
   as it looks before step 2, so step 2's change shows up as a picture diff; its harness is needed in step 2 anyway.
2. `plan/launcher-style-quick-dialogs/02-quick-switcher`: Cmd+K and Cmd+J moved onto `DialogFrame` (or the fallback),
   the field, hint chips, labels and footer strip they share; two-line session rows with age; the pinned rows; the hints
   replacing the templates hint section; the strip replacing the Close button. Goldens for Cmd+K empty, typing and
   `tl:`. SPEC.md and SPEC_impl.md for Cmd+K.
3. `plan/launcher-style-quick-dialogs/03-template-list`: Cmd+J's rows with marks and the "opens New" tag (the flag on
   `templates::rows::Row`), its hints and its footer words. Golden for Cmd+J. SPEC.md for Cmd+J.
4. `plan/launcher-style-quick-dialogs/04-rename`: Rename, Rename group and New group in the new look, with the frame's
   Close-first Tab option. Their goldens. SPEC.md's Rename editor sentence and group paragraphs, SPEC_impl.md for
   Rename's body; the user docs' words; the changelog fragment for the whole change if earlier PRs did not each leave
   their own; and the removal of the TODO.md entry.

Each PR carries its own SPEC edits for what it changes, its goldens, and the changelog fragment its type calls for (root
`AGENTS.md`, "Changelog fragments"; a `style:` PR that changes what users see needs one).

## Fallbacks

- If Cmd+K and Cmd+J cannot sit on `DialogFrame` (the top placement's shrink rules, or the results' scrolling), keep the
  switcher's own card and factor the frame's control-header and footer-band drawing into `pub(crate)` pieces that both
  call, as the planner proposals say. Log which one you built and why.
- If the uppercase label cannot be shared without changing `option_heading`'s other callers' looks, keep
  `option_heading` as it is for them and share `command_heading` alone.
- If the launcher's pictures change by a pixel when switched to the shared pieces and the cause is a deliberate
  improvement, keep it, log it, and let the visual review see it; otherwise match the old pictures exactly.
- If a docs-shot or e2e target id no longer exists, update its user to the nearest equivalent element in the same PR and
  say in the report which shots will look different.
- Decisions to log: the exact sizes and gaps you chose, the hint chips' final words, the icon used for each tile, every
  golden whose change you judged intended, and every review finding declined, with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-launcher-style-quick-dialogs-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-launcher-style-quick-dialogs-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what the PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change.

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

The review gate and the visual review keep the model and effort this file demands; the scope reassessment runs at your
own model and effort. The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the
resume check, and the cold reads of a blocked question and of the report) are exempt from the rules above and run as
that file says: native, read-only, at your own model.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/launcher-style-quick-dialogs/01-shared-command-pieces`, `plan/launcher-style-quick-dialogs/02-quick-switcher`,
  `plan/launcher-style-quick-dialogs/03-template-list`, `plan/launcher-style-quick-dialogs/04-rename`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides. `crates/gui/farhelm-gui/tests/goldens.txt`
  and the golden `SCENES` list conflict mechanically with other plans that add scenes: keep both sides, then re-run the
  goldens as `.agents/validation.md` describes.
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Visual review

Mandatory, in addition to the review gate, for every PR that changes what any dialog looks like, and for step 1, whose
claim is that the launcher looks the same: the maintainer demands a fresh-context agent on `opus` at `high` effort,
launched natively or through `$scode-harness-shellout`, which must be satisfied that the rendered result matches the
intent of the agreed mockup. Never substitute another model, and never skip it because the review gate passed. If it
cannot be reached after a reasonable retry, block per `plans/.agents/executing.md`, "Executing one plan" (step 10).

- **What it looks at.** Run `scripts/check-gui-goldens.py --update` (as `.agents/validation.md` describes) so the PR's
  golden scenes are captured, light and dark, into `target/gui-goldens/captures`, and give the reviewer the paths of
  every new or changed picture for the PR's surfaces (for step 1, the launcher's pictures and their references, which
  must match). Give it the mockup page, `https://snippets.scode.org/s/farhelm-dialog-directions/` (section "1 Launcher
  sibling"), with the instruction to compare in both of its themes (the page's Dark and Light buttons), and the design
  words of this file ("The agreed design, in words", the acceptance criteria and the decisions), which win where the
  mockup and the words differ. If the reviewer cannot fetch the page, it judges against the words alone and says so.
- **Its scope is this PR's.** Name in the prompt what this PR builds (its outline step) and what later PRs will add.
  Elements of the mockup that a later PR adds are not findings, and neither are older parts the PR does not touch; the
  last PR's review judges the whole surface.
- **Its charter**, carried in full in the prompt:

  > Compare the rendered screenshots of this PR's dialogs with the agreed mockup and the written design. Judge whether
  > each rendered picture matches the mockup's intent: layout and hierarchy, grouping, spacing rhythm, type sizes and
  > weights, colors and color roles, marks and icons, the states shown, and the wording. Pixel identity is not the bar;
  > an implementation detail the mockup could not know (a real font's metrics, a real control's focus ring) is fine. A
  > missing or misplaced element, a wrong hierarchy, a wrong color role, an element the design does not have, text that
  > is cut off or overlapping, or a state that looks broken is a finding. Report findings to the named file, most severe
  > first, each naming the picture, what differs from the intent, and what would bring it in line, and end with one
  > line: SATISFIED or NOT SATISFIED. Edit nothing and touch no VCS state.

- **Looping.** Fix what it finds that is a real mismatch, re-capture, and run a fresh visual review on the new pictures,
  until it reports SATISFIED. A finding you judge wrong (it misreads the mockup, or asks for something the written
  design rules out) is declined with a DECISION in the log and the picture and reason named in the report; a reviewer
  that still reports NOT SATISFIED only on declined findings counts as passed. If the same mismatch survives three
  rounds, run the scope reassessment below on it before another round.
- **Publishing.** Publish the accepted captures with `python3 scripts/gui-goldens.py publish` and commit the manifest
  change with the PR, as `.agents/validation.md` describes.

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
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file. If current code already does what an entry asks, or the code
it targets no longer exists, record that in the working log and the report, and build only what is still missing.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate and its visual review, with the last one
removing the TODO.md entry this plan covers ("Give Cmd+K, Cmd+J and Cmd+L the session launcher's look."). Open, not
merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
