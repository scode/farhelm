# Put the command field in charge of the session launcher

Written against main at b5c47a9fe on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, except where this goal explicitly changes them (the spec edits it lists are part of the work).

## The goal

Implement the TODO.md entry under Near term, **Session launcher: put the command field in charge.** The maintainer's
words when asking for it: "the session launcher is getting really crowded ... can you propose some realistic mockups of
improvements that still keep a unique look (not generic ai slop) and doesn't diverge from overall aesthetic we have, but
brings a little more ..... nuance/modern layout/style/look to things". Of the variants shown, they chose the one called
B2, with one change: "this on the bottom left [the footer summary] i think is not actually needed since it doesn't say
anything yoou don't see anyway and mostly just adds noise". On an earlier round they said: "the search field comes
across as its just searching thigns in the left to the right, but it's the "do everything command field" of the current
launcher ... I think it needs to be more "central and prominent and the obvious main place to be" ... and keyboard focus
must always start in that field so you can JUST start typing. and ideally helpful hints indicating a couple of things
you can do", and "the agent icons should of course be the actual icons we are using for those agents in the session
list".

The agreed mockups are published at `https://snippets.scode.org/s/farhelm-launcher-settings-agreed/`, section "Session
launcher" (the empty launcher and the launcher after typing `build`). They are the visual reference for the visual
review below. The words in this file are the design; where the mockup and these words differ, the words win.

### The agreed design, in words

**The field is the header.** The launcher's title row ("New session" with its subtitle) goes. In its place, at the very
top, is the search field, about 48 pixels tall with 14.5-pixel text, the launcher's rocket tile at its start and Close
(×) at the row's end. Its focus ring shows whenever it has focus. Its placeholder starts with the mode's name: "New
session: type an agent, host, folder, model or name…" (Clone, Replace with and Restart with use their own mode names the
same way). At its end, while it is empty, a small key cap reads "⏎ on empty launches". The dialog's accessible name
stays the mode's title ("New session" and so on). Restart with and Replace with keep their mode's summary ("Resume this
session with different settings", "Start a new session, then delete the source") as a quiet line under the field, since
it says what the action does; New and Clone show the hint row there instead.

**Hints.** Under the field, while it is empty, a row of hint chips, each a real example in the terminal font followed by
what it does: `claude` pick an agent · `host:build-box` another host · `gh:owner/repo` clone a repo · `tl:` templates ·
`fix login bug` names the session. The row hides as soon as the field has text. The examples must stay true to what the
search actually accepts (`crates/gui/farhelm-gui/src/launcher/search.rs`); use a real host name only if one exists, else
the literal placeholder shown.

**Focus.** Opening New, Clone or Replace with puts focus in the field, as today. After any pick made in the left column
or its results with the mouse, focus goes back to the field, so the next word can be typed straight away. Restart with
keeps starting focus on Cancel, as SPEC.md requires.

**The left column is the field's own list of rows.** The body splits into a left column (about 250 pixels, on the raised
`bg_1` ground) and the form on the right.

- With the field empty, the column shows a line "↑ ↓ to move, ⏎ to take" in key caps, then **Recent** (up to three
  recent setups, each led by its harness mark, then the agent name, a YOLO tag when it carries one, `host:folder` in the
  terminal font, and the rest of the words today's strip gives the row, including its wrapper prefix, on a further line,
  cut with an ellipsis when long and whole in its hover help), then **Templates** (every template, each as its mark,
  name, a one-line summary, and its number key cap when it has one). The column scrolls on its own when it is too long.
  A row whose setup or template filled the form shows the selection fill.
- **Keyboard:** the column is the field's list, not a set of Tab stops. With the field empty nothing in the column is
  highlighted for the keyboard, so Enter in the empty field launches the form, as today. ↓ from the field moves a
  keyboard highlight into the column (↑ from its first row returns to the field); Enter on a highlighted recent setup
  does what Enter on a focused strip row does today (launches it), and Enter on a highlighted template applies it, as
  taking its `tl:` row does. Tab goes from the field to the form, then the footer.
- While the field has text, the same column shows the field's results instead, grouped under small headings by kind
  (Host, Folder, Recent setup, Template, Name, and the search's other kinds), in the order the search gives them today.
  The matched text is underlined. Each row's second line says what taking it does where that is not obvious ("connected
  · switch host"). The preselected row (by `search.rs`'s rule) is highlighted and shows an ⏎ key cap. These replace
  today's results drop-down; their rules (what a query means, which row is preselected, what an empty result says) stay
  those of `search.rs`, and Escape or a click outside closes them as today.
- Taking a row (Enter, or a click) does exactly what taking that recent setup, template or search row does today. Enter
  on a recent setup or template row behaves as today's strip and search rows do.
- In Clone, Replace with and Restart with the layout is the same. A row that would change a field the mode holds is
  shown disabled, with the reason today's refusal gives, instead of being refused after the click.
- While results are open the form dims slightly (about 55% opacity) to show where the keyboard is; it stays live, and a
  click in it closes the results, as a click outside them does today.

**Narrow windows.** The launcher is 960 pixels wide and the window can be as narrow as 720. When the dialog is narrower
than it needs for both columns, the left column hides, and the field's rows drop down under the field while you type, as
they do today; recent setups and templates are then reached through the field (`recent:` and `tl:`).

**The form keeps round 1's layout.**

- Labels sit above their fields, in the small label style, instead of in today's label column on the left. Help
  sentences become a short note after the label ("Model · 1 for Claude", "Initial prompt · sent once, as Claude's first
  message", "Name · optional") or the field's placeholder ("Left empty, Farhelm names it"). Hover help stays as it is.
- A provenance pill at the top of the form, "From a recent setup" or "From a template" (with the template's name), when
  the form was filled by one, with the note "Change anything below before launching." It goes when the form is reset.
- **Destination** is a card (`bg_1`-like card fill, hairline border, 6-pixel corners) headed by the small uppercase
  label "DESTINATION", with the Folder / Managed checkout choice as a small segmented switch in the card's top right.
- **Host and folder are one address field**: one bordered row with the host segment (host icon in its color, host name,
  ▾) on the left, the folder in the terminal font in the middle, and Browse (folder icon) at the right end. Labels
  "Host" and "Folder" sit above their segments. The Name field sits beside it (or under it when narrow). With Managed
  checkout chosen, the repository and checkout rows take the folder segment's place as they do today. A held host or
  folder (Restart with, a new environment's home) shows as text in its segment, without the ▾ or Browse.
- **New container and New sandbox move into the host picker**: its menu lists hosts first under "Existing host", then a
  rule and "New environment" with New container (with its "experimental" tag) and New sandbox, offered by the same rule
  that decides today which environments Runs on offers. Choosing one makes the host segment read the environment's name
  (the picker itself, so choosing a host in it goes back) and puts the environment's own controls (Recipe, Container
  host) in the folder segment's place, as today's Runs on rows do. The separate "Runs on" row goes from the launcher.
- **Agent and Command tabs** stay, as underline tabs.
- **Harness chips** are 26 pixels tall, each led by that harness's mark from the session list (the same geometry, size
  rule and color treatment as `crates/gui/farhelm-gui/src/marks.rs` and `harness.rs` give the session list, including
  white on the selected chip and grey for Codex and Goose). Bash and Zsh follow after a thin vertical rule. Chips wrap
  to a second row as needed.
- **Model, Effort and Permissions share one row**: Model as its field, Effort and Permissions as today's controls for
  the chosen agent (the same values, labels and control kind: segmented, or a select where today's rule makes it one),
  with YOLO in the danger tone. When the row cannot fit an agent's controls, Permissions wraps under it.
- **Initial prompt** keeps its multi-line field, with the help in the label note and a placeholder naming the agent.
- **More options** folds Conversation ID (and the other conversation-selector labels it takes per harness) and Wrapper
  prefix into one collapsed row: a chevron, "More options", the names of what is inside, and on the right either "none
  set" or a pill showing each value that is set (a remembered wrapper prefix in the terminal font). A value set inside
  is always visible on the folded row; that rule is what makes the fold safe. The fold opens by itself, and stays open,
  whenever a field inside it is required (Restart with's required conversation field), refused or invalid.

**The footer** holds only the actions, right-aligned, in SPEC's order: Cancel, Reset choices (quiet), Save as template
(quiet), and the main action (Launch, Replace, Restart) with a ⏎ key cap. The summary line on its left goes in every
mode. The main action's accessible name keeps naming the operation and target. Refusals, the destructive warnings, the
inline YOLO and sandbox questions and the notices stay beside the actions in their bounded region, as today.

**Look.** The app's Steel palette, Inter and the terminal font, 3, 4 and 8 pixel radii, the solid selection fill. No new
colors except what the theme module already has; if a card fill is needed, it is added in `theme.rs` as a named role,
not written in a view.

### Where it stands (from planning, at main b5c47a9fe)

Line numbers are approximate.

- `crates/gui/farhelm-gui/src/launcher.rs` maps the launcher's modules. The dialog is drawn in `launcher/view.rs`
  (`render`, about lines 2225-2690; its width constant `WIDTH = 960`, about line 124; the footer summary `summary` and
  `choices`, about lines 1673-1770; the heading, about line 1582). The search field and results are
  `launcher/view/search.rs`; the recents strip `launcher/view/recents.rs` (rules in `launcher/recents.rs`); the host
  picker `launcher/view/host_picker.rs`; Runs on, Recipe and Container host `launcher/view/placement.rs` and
  `launcher/destination_rows.rs`; the harness chips `launcher/view/choices.rs`.
- `launcher/destination_rows.rs` is shared with the Templates editor (`crates/gui/farhelm-gui/src/templates/editor`).
- Templates' rows are `crate::templates::rows::rows`, shared with the quick switcher and the Cmd+J list.
- The session list draws harness marks through `crates/gui/farhelm-gui/src/marks.rs` and `harness.rs`
  (`session_list/row_marks.rs`); `docs/harness-marks.md` governs them.
- SPEC.md "Creation" (about lines 1404-1417) pins the launcher's header, the order "Search and recent setups precede
  labeled destination rows (Where, Host, Folder or Repository, Name)", the footer summary and the action order.
- No golden opens the real launcher today: `crates/gui/farhelm-gui/src/goldens/foundations.rs` draws the inline YOLO
  question and a `dialog-launcher-frame` stand-in, neither the launcher itself. The first PR therefore builds a golden
  fixture that opens the real launcher against the test helm, which every later scene uses.
- The dialog frame (`components/dialog.rs`) already has a header slot that takes any element (`DialogFrame::heading`);
  the search rows are drawn by `draw_search_rows` in `view/search.rs`; the environments Runs on offers come from
  `build_placement_rows` (`view/placement.rs`); `small_segments` and `chips` in the components cover the small segmented
  switch and the harness chips (a chip's mark needs its own color, a small addition to the chip options).
- The Templates editor shares only the Runs on, Recipe and Container host controls and the Folder or Managed checkout
  choice with the launcher; its Host is a plain select over recorded installs, and Host and Destination are separate
  optional fields with their own ✕.
- Element ids in the launcher are targets for the docs screenshots' bridge (for example `launcher-destination-section`
  in `view.rs`); the docs-shot page modules are under `scripts/docs-shots/`, governed by `.agents/docs-media.md`.
- The docs page `website/src/content/docs/docs/using/start-a-session.mdx` describes the launcher's layout and shows
  published screenshots of it.

### Acceptance criteria

1. The launcher looks and behaves as "The agreed design, in words" says, in all four modes, in both appearances.
2. Focus starts in the field for New, Clone and Replace with, returns to it after a mouse pick, and starts on Cancel for
   Restart with. Every existing keyboard path (Tab order through the fields, Enter rules, a held Enter never launching,
   Escape) still works; Tab follows the new visual order.
3. Everything the launcher could do before it can still do, with the same effects: every search label and row kind,
   recent setups, templates, held-field refusals, the YOLO and sandbox questions, Save as template, Browse, the folder
   list, managed checkouts and new environments.
4. The visual review below passes for every new or changed golden scene.
5. Golden scenes exist, light and dark, for: New with the field empty (recent setups and templates listed); New while
   typing (grouped results); New with a set wrapper prefix showing on the folded More options row; the host picker open
   with the New environment entries; a new container chosen; Restart with (held fields, disabled rows); Replace with;
   the launcher at a narrow window width (left column hidden); and the inline YOLO question in its new place.
6. SPEC.md "Creation" (and wherever else it describes the launcher's header, rows, recents strip, Runs on rows or footer
   summary, including "Restart with"'s "with the agent type, host and folder beside it", which becomes the held fields
   shown in the form) describes the new layout; SPEC_impl.md wherever it describes the launcher's view. The docs page's
   words match the new launcher; its screenshots are left for the next docs refresh, and the report says which ones are
   stale. The docs-shot modules still run against the new launcher (keep the element ids they target, or update the
   modules in the same PR, reading `.agents/docs-media.md` first).
7. The Templates editor keeps its own Host and Destination fields and its Runs on control; where it shares a control
   with the launcher (the Folder or Managed checkout choice, Recipe, Container host), that control takes the new look in
   both, and the editor gets a golden scene so the visual review sees it.
8. The last PR removes the TODO.md entry ("Session launcher: put the command field in charge.").

## Decisions already made

Settled with the maintainer on the 2026-11-20 mockup rounds and the 2026-11-21 planning page:

- **Design B2**, as described above; round 2's "B3" (the choices shown as tokens inside the field) is out.
- **Footer summary dropped in every mode**; the main action's accessible name keeps naming the target.
- **Round 1's form changes all come along**: labels above fields with help as short notes or placeholders; host and
  folder as one address field with Browse at its end; Folder / Managed checkout as a small switch in the Destination
  card; New container and New sandbox in the host picker; harness chips with the session list's marks, shells after a
  divider; Model, Effort and Permissions on one row; More options folding Conversation ID and Wrapper prefix and always
  showing a set value; the provenance pill; the form dimming while results are open. The maintainer's note: "my
  understanding this will make it look like it looked in the mockup above"; it does.
- **Same layout in all four modes**, with rows that would change a held field shown disabled with the reason.
- **Narrow windows**: the left column hides and the rows drop down under the field.
- **Left column contents**: up to three recent setups, then every template, the column scrolling on its own.
- **Effort keeps its real label "Medium"** (the first round's mock abbreviated it to "Med").
- **Ordering**: this plan was to run after `gui-launcher-feedback-fixes.md`, which changed the recent setups; that plan
  landed (#4406 and the PRs after it) before this one was queued, so the plan's code reading includes it.
- **Review gates**: a single fresh-context `gpt-6.2-sol` agent at `high` effort per PR, and, in addition, the mandatory
  visual review by `opus` at `high` effort described under "Visual review". The maintainer's words: "for the plans
  please have a special extra mandatory opus review for visual inspection as part of the plan. opus needs to be happy
  that the actual rendered result matches the intent of the mockup".
- **Release note**: the maintainer's own wording, under "Release note" below.
- **No-workhorse mode**: the plans default, stated on the planning page.

Planner decisions:

- The Templates editor keeps its own Host and Destination fields (they are optional, separately clearable fields, which
  one address field does not fit) and keeps Runs on; only the controls it already shares with the launcher change look.
  The report says how it came out, since the maintainer did not see it in the mockup.
- Enter in the empty field launches, and nothing in the column is highlighted for the keyboard until ↓, so opening Clone
  and pressing Enter launches the clone, never the newest recent setup.
- Recent setup rows keep today's strip words (including the wrapper prefix, which `gui-launcher-feedback-fixes.md` added
  so similar setups can be told apart).
- The More options fold opens by itself when a field inside is required, refused or invalid.
- A card fill, if the theme lacks one, is added to `theme.rs` as the role `card` (dark `#20252f`, light `#ffffff`);
  `settings-dialog-refresh.md` adds the same role with the same values, so whichever lands second finds it there and
  uses it.
- The docs page's text is updated in the plan; its screenshots are recaptured by the next docs refresh, which owns
  publishing them.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the words quoted above, and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md",
  "Code rules" (no per-harness branching at the call site: the marks and harness answers come from the places
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` and `harness.rs` name), and its "Instructions by path" for every
  path you touch: `docs/harness-marks.md` before touching `marks.rs` or `marks/`, `website/AGENTS.md` before touching
  `website/`, `.agents/docs-media.md` before touching `scripts/docs-shots/`.
- `.agents/validation.md` for the GUI test selections and the screenshot goldens (re-capturing them with
  `scripts/check-gui-goldens.py --update`, inspecting every changed picture, and publishing them with
  `python3 scripts/gui-goldens.py publish` before any rebase, as it describes).
- `.agents/test-authoring.md` for every test added or changed.
- SPEC.md "Creation" and "Lifecycle operations" (Restart with's held fields and focus on Cancel); the GUI architecture
  rules in SPEC_impl.md ("GUI architecture"); the theme module's rule that views never hold a color
  (`crates/gui/farhelm-gui/src/theme.rs`, module docs).

**Planner proposals:**

- The left column is drawn from the same row builders the search and the recents strip already use (`search.rs`,
  `recents.rs`, `templates::rows`); no second ranking or matching.
- The field goes in the dialog's existing header slot (`DialogFrame::heading`); no new frame.
- The left column draws its results with the existing `draw_search_rows` (also used for the narrow-window drop-down),
  and its empty state with the existing recent-setup selection and one recent-row drawing; the old strip's own drawing
  and per-row focus handling are deleted, not kept beside it. The one new piece is the column's open/closed state for
  results, since no popover layer handles Escape and click-to-close there.
- The host picker's New environment entries come from the rule that decides today which environments Runs on offers,
  pulled out of `build_placement_rows` so the picker and the Templates editor's Runs on both use it.
- The Folder or Managed checkout switch uses `small_segments`; the harness chips use `chips`, extended so a chip can
  carry a mark in its own color.
- Disabled held-field rows reuse the refusal reason the search already computes for a held field.

## Outline

1. `plan/launcher-command-field/01-launcher-form-layout`: a golden fixture that opens the real launcher against the test
   helm; labels above fields with help as label notes or placeholders; harness chips with the session list's marks and
   the shells after a rule; Model, Effort and Permissions on one row; the Destination card. Goldens for New as it then
   looks.
2. `plan/launcher-command-field/02-launcher-address-field`: the host and folder address field with Browse at its end,
   the Folder / Managed checkout switch in the card, and New container and New sandbox in the host picker (removing the
   launcher's Runs on row; the Templates editor keeps its own). Goldens for the open picker, a chosen new container, and
   the Templates editor.
3. `plan/launcher-command-field/03-launcher-more-options`: the More options fold with its set-value summary. Golden with
   a remembered wrapper prefix.
4. `plan/launcher-command-field/04-launcher-command-field`: the field as the header, the hints, focus returning to the
   field, and the footer without its summary.
5. `plan/launcher-command-field/05-launcher-start-column`: the left column (recent setups and templates when empty,
   grouped results while typing, disabled held-field rows), the provenance pill, the form dimming, and the narrow-window
   fallback; SPEC.md and SPEC_impl.md brought in line (if earlier PRs already edited the spec for their own parts, they
   did so in their own PR); the docs page's words; a changelog fragment; and removal of the TODO.md entry.

Each PR carries its own SPEC.md edits for what it changes, its goldens, and the changelog fragment its type calls for.

## Fallbacks

- If the header slot cannot hold the field (focus or sizing), extend the slot rather than building a frame of the
  launcher's own; a new dialog frame is a scope reassessment.
- If a docs-shot module targets an element that no longer exists, update the module to the nearest equivalent element in
  the same PR (reading `.agents/docs-media.md` first) and say in the report which shots will look different.
- Decisions to log: the exact sizes and gaps you chose, the hint chips' final examples, how the narrow-window threshold
  is decided, every golden whose change you judged intended, and every review finding declined, with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-launcher-command-field-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-launcher-command-field-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/launcher-command-field/01-launcher-form-layout`, `plan/launcher-command-field/02-launcher-address-field`,
  `plan/launcher-command-field/03-launcher-more-options`, `plan/launcher-command-field/04-launcher-command-field`,
  `plan/launcher-command-field/05-launcher-start-column`.
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
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
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

Mandatory, in addition to the review gate, for every PR that changes what the session launcher or the Templates editor
looks like: the maintainer demands a fresh-context agent on `opus` at `high` effort, launched natively or through
`$scode-harness-shellout`, which must be satisfied that the rendered result matches the intent of the agreed mockup.
Never substitute another model, and never skip it because the review gate passed. If it cannot be reached after a
reasonable retry, block per `plans/.agents/executing.md`, "Executing one plan" (step 10).

- **What it looks at.** Run `scripts/check-gui-goldens.py --update` (as `.agents/validation.md` describes) so the PR's
  golden scenes are captured, light and dark, into `target/gui-goldens/captures`, and give the reviewer the paths of
  every new or changed picture for the PR's surfaces. Give it the mockup page,
  `https://snippets.scode.org/s/farhelm-launcher-settings-agreed/` (section "Session launcher"), with the instruction to
  compare in both of its themes (the page's "Steel dark" and "Steel light" buttons), and the design words of this file
  ("The agreed design, in words", the acceptance criteria and the decisions), which win where the mockup and the words
  differ. If the reviewer cannot fetch the page, it judges against the words alone and says so.
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
it targets no longer exists, record that in the working log and the report, remove the entry in the plan's PR, and build
only what is still missing.

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
meeting the acceptance criteria above, each having passed its review gate and, where it changes what the GUI draws, its
visual review, with the last one removing the TODO.md entry this plan covers ("Session launcher: put the command field
in charge."). Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

Session launcher dialog has been significantly revamped (hopefully for the better!).
