# Give What's new the session launcher's look

Written against main at 210c22183 on 2026-11-25. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, except where this goal explicitly changes them (the spec edits it lists are part of the work).

This plan runs after `launcher-style-quick-dialogs.md` (Cmd+K, Cmd+J and Cmd+L in the launcher's look), which pulls the
launcher's pieces out into shared components under `crates/gui/farhelm-gui/src/components/`. Build on those components;
do not pull the same pieces out a second time.

## The goal

Implement the TODO.md entry under Immediate, **Give What's new the session launcher's look.** The upgrade highlights
window (SPEC.md calls it **What's new in Farhelm**; `crates/gui/farhelm-gui/src/whats_new.rs`) opens at window start
after an upgrade, so most users see it at every release, and it has to look good and be very functional. The maintainer
asked for mockups ("A lot of people will see it very often, so it is important that it looks good. But it also needs to
be very functional."), was shown twenty directions, and chose direction 2, "Launcher sibling", with one change: "For
'whats new' use launcher sibling, but don't clamp. Just use the whole entry. Note- need to adjust release notes process
during release to identify what portion is ot be highlighted."

So the work has two halves:

1. **The window.** Restyle it as the launcher sibling, every entry shown in full, each with a headline.
2. **The release notes process.** From the next stable release on, curation marks each highlight's headline in
   `CHANGELOG.md`, so the window can tell the headline from the rest of the entry.

The mockup is published at `https://snippets.scode.org/s/farhelm-whats-new-directions/`, section "2 Launcher sibling"
(the page has a theme switch at the top; its "Today" section shows the current window). That mock clamps the rest of
each entry to two lines; the agreed design does not clamp. The words in this file are the design; where the mockup and
these words differ, the words win. Its release data is the real 0.45.0 and 0.44.1 notes, which predate the headline
rule, so its headlines are first-sentence stand-ins.

### The agreed design, in words

**Frame.** The window keeps the shared `DialogFrame` (`components/dialog.rs`) and its behavior, and takes the launcher's
look as `launcher-style-quick-dialogs` left it: the Steel palette, Inter and the terminal font, key chips drawn with the
shared key chip, no new colors in views (a color the theme lacks is added in `theme.rs` as a named role). Width stays
about 664 to 680 pixels; the body scrolls as today. The frame needs no new option: the header is
`DialogFrame::heading(..)` (which draws its content in place of the title and keeps the title as the accessible name),
built from the shared 24-pixel tile function `launcher-style-quick-dialogs` extracts (not the frame's own 44-pixel
`tile`); the footer's left side is `footer_lead(..)` beside the answers, as Settings already uses it. The Welcome plan
(`welcome-empty-state.md`) builds its dialog header the same way; neither plan adds a header option to the frame.

**Header.** A 24-pixel icon tile (with a sparkle or similar "new" icon; add one to the icon set only if none fits), the
visible title **What's new** (the accessible name stays SPEC.md's **What's new in Farhelm**), and a version pill in the
accent ink: "<last seen> → <running>" when the helm's record names an older release, and just the running version when
there is no such record. Under the title, one line of counts, each with its category mark: "N new", "N changed", "N
fixed", and, only when present, "N breaking" and "N removed", counting the highlights shown across every release in the
window. The × stays at the header row's end.

**Category marks.** Each highlight category gets a small mark (an icon in a tinted square, as the mock draws them):
Added a plus, Changed a refresh arrow, Fixes worth highlighting a wrench, Breaking a warning, Removed a minus or a
removal icon. Their colors are theme roles (green-ish for added, the accent or a neutral for changed, amber-ish for
fixed, red for breaking, neutral for removed), added to `theme.rs` if missing, legible in both appearances. An unknown
category name (the feed allows future ones) gets the neutral mark.

**Release labels.** Each release starts with an uppercase label drawn with the shared section label (with its optional
trailing count) from `launcher-style-quick-dialogs`, "0.45.0 · NOV 24" (version and short date), with that release's
total change count at the row's right end ("32 changes"). These per-release counts replace today's footer note ("{count}
changes in total…").

**Entries.** One row per highlight, in the feed's order (categories, then entries): the category mark on the left; then
the headline in the strong ink and semibold; then the rest of the entry, in full, never clamped or truncated, in the
body ink at a slightly smaller size, wrapping freely; then the entry's screenshot link at its end when it has one
("Screenshot ↗", opening the same URL as today). Bold and code pieces inside the rest keep their styles. An entry with
no headline (see "Headlines" below) shows the whole entry as that body paragraph, with no headline line. The category
headings of today's dialog go; the marks carry the category. Because today's headings are what a screen reader hears,
each entry's accessible label starts with its category name ("Added: …"), or the mark carries it.

**Footer.** The frame's `footer_lead` holds the shared chips-and-words items from `launcher-style-quick-dialogs` ("⏎ got
it", "esc close"), as Cmd+K and Cmd+J use them; the answers on the right are the existing buttons, unchanged in
function: Don't show again, All release notes ↗, and Got it (primary, focused at open, with a ⏎ key chip as the
launcher's main action has one: reuse the "verb + ⏎" helper if `launcher-style-quick-dialogs` built one, otherwise build
it the way the launcher does with `Button::content`; never a third copy).

**Headlines.** A highlight's headline is the leading bold sentence of the entry, as the website feed delivers it: the
entry's first piece is bold, its text ends with ".", "?" or "!", and at least one non-empty piece follows. That piece is
the headline; the pieces after it, with the joining space trimmed and any piece left empty dropped, are the rest.
Anything else has no headline: an entry that opens with a bold UI name ("**Suspend** and **Resume** require …"), with
plain text, or with a bold run the feed split (italics or code inside the bold make the first bold piece stop before the
punctuation). Nothing is guessed from plain text. The feed builds pieces in `website/scripts/release-notes.mjs`,
`entryPieces`: the closing punctuation is in the bold piece only when it sits inside the `**`; a link inside the bold
stays in the one bold piece; italics or code inside it split it. Every real highlight ends in its `(#N)` reference, so a
headline always has a rest.

**Behavior unchanged.** When the window opens, which releases it shows, focus, the buttons and what they record, the
opt-out failure notice, Escape and ×, and the "never opens over the user's work" rules all stay as SPEC.md gives them.

### The release notes process change

Today a highlight is one paragraph, and entries commonly open with a bold UI name, so nothing marks a headline. From the
next stable release curated after this plan lands:

- Every highlight entry (the `###` categories of a current section, `releasing/.agents/changelog.md`, "Format") opens
  with its headline: `**…**` holding one sentence whose final `.`, `?` or `!` is inside the closing `**`, with plain
  words and links only inside (no code, italics or nested bold), then a space and the rest:
  `- **You can register your own agent types.** Settings → Custom agents takes … (#N)`. The full list's entries are not
  affected. This shape is exactly what the feed turns into one bold first piece ending in punctuation (see "Headlines"
  above).
- The release agent proposes a headline for every highlight it drafts, and for every plan release note it uses as a
  highlight that lacks one, and the maintainer approves it on the review page like the rest of the wording. A plan's
  entry is the maintainer's wording, so its headline is a suggestion shown on the page and enters the text only once
  approved. The draft section that the stable procedure lands before approval therefore carries plan entries word for
  word, with or without a headline.
- `releasing/check-changelog.py format` refuses a highlight without a headline only in a section that no longer opens
  with the draft notice (a finalized section), and only in sections that were not on main when this plan's first PR
  lands: every section on main then is exempt, a draft in flight included, so no release mid-curation needs a
  retroactive edit. The app never sees a draft (`website/scripts/release-notes.mjs`, `hasUpgradeNotes`, drops drafted
  sections), so enforcing at finalization protects the window fully. The checker's pattern is no looser than the shape
  above (a stricter checker is harmless; a looser one would pass headlines the app cannot see). The cutoff follows the
  checker's existing version-keyed compatibility registry (`LEGACY_FOLD_SUMMARY_VERSIONS`, `FIRST_FOLD_VERSION`).

Edit `releasing/.agents/changelog.md` ("Format", and "Plans' release notes" where it says what may change in a plan's
entry), `releasing/.agents/stable.md` (the curation steps that draft highlights, and the words for the review page,
which the release agent writes by hand each release: say that each proposed headline is shown as a suggestion on its
entry), and `releasing/check-changelog.py` with its self-tests. There is no review page tooling to change; do not touch
the legacy `releasing/notes-review-page.py`, which the stable procedure forbids using. `releasing/EDITORIAL_GUIDANCE.md`
takes only rules the maintainer stated, and the maintainer stated no wording rule for headlines, so it does not change;
its existing rule on opening with what the user sees or can do covers headline wording. Read `releasing/AGENTS.md` and
its "Release tooling edits" and "Changelog curation" reading lists before touching any of these. The website feed
(`website/scripts/release-notes.mjs`, `/whats-new.json`) does not change: a bold piece already reaches the app as a
`bold` piece, and apps that predate this plan show the headline as bold text. Published release sections are never
edited to add headlines.

`plans/.agents/planning.md` asks the planner to propose a release note at planning time; it would ideally propose a
headline too, but plans may not edit `plans/`. Leave it, and name it in the report as a follow-up.

### Where it stands (from planning, at main 210c22183)

- `whats_new.rs` draws the dialog on `DialogFrame` with `.summary(subtitle(..))`, `.note(..)` for the total,
  `.width(px(664.))`, `.scroll_stop(..)`, `.close(..)`, and three buttons. `release_section` draws the version heading,
  category headings and bulleted entries; `paragraph` turns pieces into one `StyledText` with bold and code ranges and
  one accessible label per entry. Its tests are in `whats_new/tests.rs`.
- The feed types are in `crates/gui/farhelm-gui/src/desktop/updates/notes.rs`
  (`ReleaseHighlights { version, date, highlights, total_changes }`, `HighlightCategory { name, entries }`,
  `Highlight { pieces, screenshot_url }`, `HighlightPiece { style, text }`, `HighlightStyle::{Bold, Code, Plain}` with
  unknown styles read as plain). The category names are the feed's: "Breaking", "Added", "Changed", "Fixes worth
  highlighting", "Removed" (`website/scripts/release-notes.mjs`, `HIGHLIGHT_NAMES`).
- The previous release the user saw is the helm's `last_whats_new_release` preference, read in `whats_new::check` as
  `previous`; it is `None` for an older helm with no record. The version pill needs it passed to `open`.
- The golden scene is `whats-new-dialog`, registered in `crates/gui/farhelm-gui/src/goldens/mod.rs` with its setup
  `whats_new_dialog_setup` in `goldens/other_dialogs.rs`, whose feed has two one-line entries. That hides what matters
  here; give the scene a feed shaped like a real catch-up (two releases, several categories, long entries, an entry with
  a headline and one without, a screenshot link, a code piece), and keep it short enough to be a readable picture.
- The docs screenshot batch lists the `whats-new-dialog` golden (`docs/gpui/screenshot-scenes.json`, about line 2820),
  titled "Upgrade highlights from two releases with a screenshot link, the total count of changes and the opt-out";
  update its title with the golden (read `docs/gpui/screenshot-batch.md` first).
- SPEC.md, "Installation and updates" (about lines 900-916) describes the dialog: names the releases shown "with their
  dates and highlights in the release notes' categories", **See screenshot**, and "The footer gives the total number of
  changes across those releases". SPEC_impl.md mentions the dialog near line 8007 and with the preferences near 5176.
  The Welcome plan edits the same SPEC.md paragraph ("after Welcome and any pending Announcements"); whichever lands
  second rebases over the other's wording knowingly.

### Acceptance criteria

1. The window looks as "The agreed design, in words" says, in both appearances, with every entry in full.
2. Headlines are recognized exactly by the rule under "Headlines", by a small pure function with unit tests covering: a
   bold sentence then text (the real shape, ending in a `(#N)`-style rest); a bold UI name then text; a bold sentence
   ending in "?" or "!"; plain text; a bold sentence whose rest starts with an empty or space-only piece.
3. Every behavior of the window is unchanged (see "Behavior unchanged"), and its existing tests still pass, updated only
   where they pin the old structure (the subtitle, the category headings, the footer note, the "See screenshot" label).
4. The `whats-new-dialog` golden shows the real-shaped feed described above, light and dark, and the visual review
   passes.
5. SPEC.md's What's new paragraphs describe the header pill and counts, the per-release labels and counts in place of
   the footer total, the marks, the headline and the full entry; SPEC_impl.md where it describes the dialog.
6. The release notes process change is in the files named under "The release notes process change". The checker's
   self-tests cover: a new finalized section with headlines (accepted) and without (refused); a new draft section
   without headlines (accepted); an exempt older section without them (accepted); and the slips (the period outside the
   `**`, code or italics inside the headline: refused). `python3 releasing/check-changelog.py format` passes on main's
   `CHANGELOG.md`.
7. The last PR removes the TODO.md entry ("Give What's new the session launcher's look.").

## Decisions already made

Settled with the maintainer on 2026-11-25, on the mockup page and the planning page:

- **Direction 2, "Launcher sibling", with every entry in full** ("don't clamp. Just use the whole entry").
- **The headline is curated, not guessed**: a bold sentence at the start of the entry, recognized by the app.
- **Entries without a headline** (sections curated before the rule) show whole, with no headline.
- **The checker refuses** a new highlight without a headline (planner refinement after the planning review: in finalized
  sections only, so the pre-approval draft can carry plan entries word for word).
- **Review gates:** a single fresh-context `claude-opus-5-5` (Opus 5.5) agent at `high` effort per PR for the code, and,
  in addition, the mandatory visual review by Opus 5.5 at `high` effort described under "Visual review". The
  maintainer's words for the visual review: "make sure there is a visual review spec by opus to visually inspect it
  besides usual code review. no nit picking on precise pixels but has to look basically right."
- **Release note:** none (see "Release note" below).
- **No-workhorse mode:** the plans default, stated on the planning page.
- **Runs after `launcher-style-quick-dialogs`**, whose shared components it uses.

Planner decisions:

- The version pill uses the helm's record of the last release seen; with no record it shows the running version alone.
- Per-release change counts replace the footer total, as in the mock.
- The category marks and their colors listed above; the screenshot link's label "Screenshot ↗".
- The visible title reads "What's new", as in the mock, beside the version pill; the accessible name stays **What's new
  in Farhelm**, which `DialogFrame::heading` keeps.
- No feed change: the headline rule reads the existing pieces. A feed-side headline field would add a second recognizer
  for no gain. Today's feed has no entry that opens with a bold sentence, so old releases get no false headlines.
- No new `DialogFrame` option; the header and footer use `heading` and `footer_lead`.
- No `EDITORIAL_GUIDANCE.md` change (see "The release notes process change").

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the words quoted above, and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md", and
  its "Instructions by path" for every path you touch: `releasing/AGENTS.md` before anything under `releasing/` (its
  "Release tooling edits" and "Changelog curation" reading lists), `website/AGENTS.md` before anything under `website/`.
- `releasing/.agents/changelog.md`: published sections are historical records and are never edited; a process change
  applies to sections curated after it; the checker stays compatible with every older section.
- `.agents/validation.md` for the GUI test selections, the release tooling self-tests, and the screenshot goldens
  (re-capturing them with `scripts/check-gui-goldens.py --update`, inspecting every changed picture, and publishing them
  with `python3 scripts/gui-goldens.py publish` before any rebase, as it describes).
- `.agents/test-authoring.md` for every test added or changed.
- SPEC.md, "Installation and updates" (the What's new paragraphs) and "Keyboard focus"; the GUI architecture rules in
  SPEC_impl.md ("GUI architecture"), including "Layout costs show only at scale"; the theme module's rule that views
  never hold a color (`crates/gui/farhelm-gui/src/theme.rs`, module docs).

**Planner proposals** (after a fresh-context planning review): see "Planner decisions" above and the outline below.

## Outline

1. `plan/launcher-style-whats-new/01-headlines`: the release notes process change (the format rule, the curation steps,
   the checker and its self-tests). A `docs:` or `chore:` PR with no user-facing effect.
2. `plan/launcher-style-whats-new/02-window`: the window in the new look, the headline function and its tests, the
   golden with the real-shaped feed, SPEC.md and SPEC_impl.md, the changelog fragment (a `style:` PR that changes what
   users see needs one), and the removal of the TODO.md entry.

If the window PR grows past a comfortable review size, split the headline function and the frame's header and footer use
into their own PR below it; log the split.

## Fallbacks

- If a shared piece from `launcher-style-quick-dialogs` (the tile, the chips-and-words item, the section label) does not
  fit this window as it stands, extend it as little as these uses need, in the shared component; never draw a private
  copy. A new `DialogFrame` option is a last resort: run the scope reassessment below first, and log it so the Welcome
  plan can reuse it.
- Decisions to log: the exact sizes, gaps and colors you chose, the icons, every golden whose change you judged
  intended, and every review finding declined, with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-launcher-style-whats-new-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-launcher-style-whats-new-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: `plan/launcher-style-whats-new/01-headlines`,
  `plan/launcher-style-whats-new/02-window` (and a split below `02-window` if the outline's split applies).
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
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `claude-opus-5-5` (Opus 5.5) at
`high` effort, launched natively, or through `$scode-harness-shellout` when your harness cannot serve that model
natively. If it cannot be reached when a review is due and is still unreachable after a reasonable retry, block the plan
per `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute
another reviewer.

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

Mandatory, in addition to the review gate, for every PR that changes what any dialog looks like (here, the window PR):
the maintainer demands a fresh-context agent on `claude-opus-5-5` (Opus 5.5) at `high` effort, separate from the review
gate's agent, launched natively or through `$scode-harness-shellout`, which must be satisfied that the rendered result
matches the intent of the agreed mockup. Never substitute another model, and never skip it because the review gate
passed. If it cannot be reached after a reasonable retry, block per `plans/.agents/executing.md`, "Executing one plan"
(step 10).

- **What it looks at.** Run `scripts/check-gui-goldens.py --update` (as `.agents/validation.md` describes) so the PR's
  golden scenes are captured, light and dark, into `target/gui-goldens/captures`, and give the reviewer the paths of
  every new or changed picture for the PR's surfaces. Give it the mockup page,
  `https://snippets.scode.org/s/farhelm-whats-new-directions/` (section "2 Launcher sibling", in part 1), with the
  instruction to compare in both of its themes (the page's Dark and Light buttons) and that the mock's two-line clamp is
  not the design (every entry shows in full), and the design words of this file ("The agreed design, in words", the
  acceptance criteria and the decisions), which win where the mockup and the words differ. If the reviewer cannot fetch
  the page, it judges against the words alone and says so.
- **Its scope is this PR's.** Name in the prompt what this PR builds (its outline step) and what later PRs will add.
  Elements of the mockup that a later PR adds are not findings, and neither are older parts the PR does not touch; the
  last PR's review judges the whole surface.
- **Its charter**, carried in full in the prompt:

  > Compare the rendered screenshots of this PR's dialogs with the agreed mockup and the written design. Judge whether
  > each rendered picture matches the mockup's intent: layout and hierarchy, grouping, spacing rhythm, type sizes and
  > weights, colors and color roles, marks and icons, the states shown, and the wording. The bar is that it looks
  > basically right; do not nitpick precise pixels. Pixel identity is not the bar; an implementation detail the mockup
  > could not know (a real font's metrics, a real control's focus ring) is fine. A missing or misplaced element, a wrong
  > hierarchy, a wrong color role, an element the design does not have, text that is cut off or overlapping, or a state
  > that looks broken is a finding. Report findings to the named file, most severe first, each naming the picture, what
  > differs from the intent, and what would bring it in line, and end with one line: SATISFIED or NOT SATISFIED. Edit
  > nothing and touch no VCS state.

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
removing the TODO.md entry this plan covers ("Give What's new the session launcher's look."). Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
