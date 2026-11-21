# Give Settings grouped sections, cards and a pinned Save

Written against main at b5c47a9fe on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, except where this goal explicitly changes them (the spec edits it lists are part of the work).

## The goal

Implement the TODO.md entry under Near term, **Settings dialog: grouped sections, cards and a pinned Save.** The
maintainer's words: "settings dialog for example still feels very "basic" - like too simple, not really feeling
reasonably modern. can you propose some realistic mockups of improvements that still keep a unique look (not generic ai
slop) and doesn't diverge from overall aesthetic we have"; after the first round, "settings look better. it still feels
a bit flat, like lack of colors. especially the menu to the left. anything we can do while not going overboard?"; "we
also need to make sure for settings that for long lists where you have to scroll the overall cancel/save/etc is always
visible and those key controls never require scrolling"; and, choosing the colors, "for settings, lets go one hue per
group". They asked for "a realistic mockup of every page because there is a history of surprises here so i want to
review each page", and reviewed every page.

The agreed mockups are published at `https://snippets.scode.org/s/farhelm-launcher-settings-agreed/`, section "Settings,
every page", one drawing per page with a note under each heading. They are the visual reference for the visual review
below. The words in this file are the design; where the mockup and these words differ, the words win.

### The agreed design, in words: the frame

- **Size** stays 760 by 780 pixels, capped by the window, the same for every page, as today.
- **Header** stays: the gear tile, "Settings", the summary ("For every host on this helm", or "…, and for this device"
  when App is listed), and Close (×).
- **Section list** (about 178 pixels wide, on `bg_1`): the search field (placeholder "Search", with a Ctrl F / ⌘F key
  cap), then three groups, each under a small uppercase heading:
  - **General**: Sessions, Appearance, Terminal, Notifications, App (App only where it is listed today).
  - **Where sessions run**: Hosts, Checkouts, Containers, Recipes, Tensorlake.
  - **Agents**: Agent trust, Credentials. Each section shows its icon on a small tinted tile (20 pixels, 4-pixel
    corners) in its group's hue, then its label. The shown section keeps the solid selection fill, its tile turning
    white on it. A section with unsaved changes shows a small dot at its right end, and a changed field shows the same
    small accent dot after its label. The list stays put while the page scrolls, and scrolls on its own when it is
    taller than the dialog's body (at the app's minimum window, 720 by 560, it is).
- **Group hues**: blue (the theme's `accent`) for General, teal (the host palette's teal) for Where sessions run, orchid
  (the host palette's orchid) for Agents, at both appearances' values. They appear only on the section tiles and the
  page heading's tile. Switches, buttons and the selection stay the app's blue. Add them to `theme.rs` as named roles
  that take their values from those existing tokens.
- **Page heading**, sticky at the top of the page while it scrolls, with a hairline and a soft shadow once content
  passes under it: a 32-pixel tile in the group's hue with the section icon, the section's name (16 pixels, semibold), a
  one-line description under it (below), and on the right a scope line with a small icon: "Every host on this helm" for
  Sessions, Appearance, Terminal, Notifications, Hosts, Checkouts, Containers and Agent trust; "This helm" for Recipes,
  Tensorlake and Credentials; "This Mac" for App. The scope line's hover help, where the page's switches save to the
  helm, is "Changes from the command line or an agent show here right away."
- **Descriptions**, as reviewed: Sessions "How session commands and quick launches behave"; Appearance "The interface
  colors"; Terminal "Links and files clicked in a terminal"; Notifications "Sounds and push alerts when a session needs
  you"; App "This Farhelm app's version and updates"; Hosts "Updating, adding and removing hosts"; Checkouts "Where
  managed checkouts go and how they start"; Containers "Podman hosts and leftover containers"; Recipes "Software and
  credentials for new containers and sandboxes"; Tensorlake "Cloud sandboxes in your Tensorlake project"; Agent trust
  "Answering agents' folder-trust questions in managed checkouts"; Credentials "Secrets Farhelm hands to hosts for you".
- **Page ground and cards**: the page sits on its own ground (dark: `bg_2`; light: `bg_0`), and settings sit in cards
  (6-pixel corners, a `border_dim` hairline, a fill between `bg_1` and `bg_2` in dark and white in light), with hairline
  rules between rows. A group of cards gets a small heading (13 pixels, semibold) with an optional quiet note beside it
  and, where a page has one, its action button (Refresh, New recipe, Add …) at the heading's right end.
- **Switch rows**: the title at 13 pixels, medium weight (today it is 16), the help under it at 12 pixels in `fg_1`, the
  switch at the row's right end, any per-row control (a play button) just before it. Failure lines and the "Not saved:
  another operation was in progress. Try again." line stay under the control they belong to, worded as today.
- **The repeated sentence goes.** Wherever a switch's help or a paragraph ends with "This choice applies to this helm;
  changes from the command line or an agent show here right away." (or its variants: the Hosts paragraph "The host
  choices apply to every host on this helm. Changes from the command line or an agent show here right away.", the Sounds
  paragraph's first sentence, Terminal's "this choice applies to this helm" clauses, ntfy's "Applies to this helm."), it
  is dropped, because the scope line now says it once per page. Search must no longer match on the dropped words.
- **Footer**, pinned, one row: on the left the note "Switches save at once; forms save with Save below."; on the right
  Close. Whenever the shown page has unsaved changes in a form, the left side instead reads "<form or page name> has
  unsaved changes" after a dot, and Discard (quiet) and Save (primary) sit before a thin rule and Close. An open editor
  (a recipe, a credential) puts its own answers in that row: Delete (danger, quiet, existing items only), Cancel, Save.
  The editors' error and busy lines stay pinned just above the footer row, as the recipe editor's are today, and the
  recipe editor's delete confirmation replaces the footer row's buttons while it is open (its text, Cancel, Delete). The
  footer's buttons never live in the frame's status text, which can be cut at narrow widths.
- **Closing with unsaved changes** anywhere (Close, ×, Escape) asks once: "Save changes before closing?" with the pages
  named under it ("Unsaved: ntfy alerts, Checkout settings"), and Keep editing, Discard and Save.
- **What "unsaved" means:** a form's draft differs from what the helm last stored (typing a value back to the stored one
  makes it saved again); an open editor with changes counts, as the recipe editor's own rule decides today; a form whose
  save is still out counts as pending, and while one is, the close question leaves out Discard (the recipe editor's
  rule) and closes only once it ends.
- **Saving several at once** (the close question's Save, or Checkouts' one Save with both fields changed): every
  Settings save holds the one preference key, and the operation store admits one request on it at a time (`settings.rs`
  module docs, the NOTE on quick flips), so the dialog runs the saves one after another, each through its form's own
  save, stops at the first refusal, shows that page with the failure where it belongs, and closes only after the last
  save succeeded. The two copies of the Tensorlake key form share one account: if both hold different unsaved keys, the
  close question's Save does not pick one; it shows the Credentials page with the line "Two different Tensorlake keys
  are typed. Save the one you want first." and closes nothing.

### The agreed design, in words: every page

Each page keeps every control, state and wording it has today (the inventories behind the mockups were read from the
code at b5c47a9fe; where this list is silent, today's behavior stands), with these changes:

- **Sessions**: one card with its three switches.
- **Appearance**: the "Color theme" choice becomes three preview tiles (System, Dark, Light), each a small drawing of
  the app in that appearance (System split half light, half dark) with a radio and the name under it, working as one
  radio group (arrow keys move it, its accessible name stays "Color theme"). The group's note: "System follows each
  device's own appearance. The terminal stays dark."
- **Terminal**: "Web links" (note "This choice does not change file downloads.") is a card of three radio rows, each
  with its description visible; "Tunnels" (note "Mac only", shown only on a Mac) a card with the tunnel switch; it still
  opens today's tunnel warning; "Downloaded files" a card with the file-types field, its help and its risky-type
  warnings. The field's Save moves to the footer; "File types saved", the parse errors and the refusal stay under it.
- **Notifications**: "Sounds" (note "The session open in an active window stays quiet.") is a card of four rows, each
  with its tooltip text as its visible description, a play button that plays that sound once, and its switch. "ntfy
  alerts" (note "Send a session's title and host when it waits for you. In the desktop app, keep the window open.") is
  one card: the Send ntfy alerts switch, then Server URL and Topic side by side, the topic warning, Access token and
  Priority side by side, the Clear saved token switch, and a card footer with Send test and its note. Save moves to the
  page footer; every token placeholder, tooltip and status line stays as today.
- **App**: one card with the Version row (version in the terminal font and Check for updates) and the Install updates
  automatically switch, plus the switch `whats-new-on-upgrade.md` adds, in the same card. Today's line "This one belongs
  to this Farhelm app on this Mac." goes; the scope line says it.
- **Hosts**: one card with its three switches; the "without asking" switches keep help that describes the current
  behavior.
- **Checkouts**: "New checkouts" is one card with the default checkout folder and the post-clone command, each with its
  help; one Save in the footer saves whichever of the two changed. Each field's Retry, saved line and failure lines stay
  under it. The spare-checkout switch, its help and its docs link are a card of their own and save at once.
- **Containers**: the intro, then "Container hosts" (Refresh in the heading) as a card of host rows, each with a host
  dot, name and state, then "Containers not on this helm" as a card of leftover rows with Delete, the delete question
  opening inside that card under the rows, then its help paragraph.
- **Recipes**: the intro, then "Recipes" (New recipe in the heading) as one card with a row per recipe holding today's
  name, summary, per-place lines with Rebuild, Tensorlake snapshot and prepared lines and build button, every status
  wording as today, and Edit on the right.
- **Recipe editor**: still replaces the list under the Recipes heading, with "‹ Recipes" and the title on one line. Its
  fields are grouped into cards: Name; Size (note "Tensorlake sandboxes only", the three fields side by side with their
  hints); Software (the four bases as a 2 by 2 grid of radio cards, the chosen base's fields, Ubuntu packages and the
  build help); Tunnels to your side; GitHub; Agents (each agent led by its mark, its mode as a small segmented control,
  the mode's fields under it). Its Delete, Cancel and Save join the footer row.
- **Tensorlake**: the EXPERIMENTAL box exactly as today (the maintainer's words and its red fill), the intro, then cards
  under "API key" (the storage warning, the field, its help, Test and Clear with the outcome line), "SSH key",
  "Sandboxes not on this helm" (Refresh in the heading; the block still appears only as today) and "Creating sandboxes",
  with the Usage Credits paragraph under the last. The key's Save moves to the footer.
- **Agent trust**: one card per agent, its first row led by the agent's mark from the session list, holding the trust
  switch and its help; Claude's caution box inside its card; the hint switch as the card's second row (Pi has none).
- **Credentials**: the delivery-failure lines at the top as today, the intro and storage warning, then cards for
  "Tensorlake API key" (note "The same key as in Settings → Tensorlake"), "GitHub tokens", "Agent subscription logins"
  and "API endpoint tokens", each list's Add button in its heading and "None saved." inside an empty card. The key's
  Save moves to the footer. While the credential editor is open the footer carries the editor's answers; an unsaved key
  draft is kept, keeps the section's dot, and is named by the close question.
- **Credential editor**: replaces the overview as today; its fields in one card, the agent choice as a segmented control
  led by each agent's mark; its Delete, Cancel and Save join the footer row.
- **Search**: works as today; the section list keeps its group headings for the sections with matches; the results use
  the page heading's place for "N settings match “x”" and the cards and group headings for the matching rows, with the
  matched text in bold accent and the selected row in the selection fill.

### Where it stands (from planning, at main b5c47a9fe)

Line numbers are approximate.

- `crates/gui/farhelm-gui/src/settings.rs`: module docs (the frame, sections, saving rules), the size constants (about
  lines 164-206, `WIDTH`, `HEIGHT`, `NAV_WIDTH`, the footer note at about 194, `HOST_CHOICES_HELP` at about 201), the
  section enum and its order (`Section::ALL`, labels and icons, about lines 460-590), the frame's `render` (about lines
  1451-1606: the section list at 1464-1529, heading and body at 1531-1553, the recipe editor's pinned answers at
  1572-1584), and each section's body (about lines 1231-1418). The module docs note the section list is not pinned while
  a long section scrolls.
- Switch rows: `settings/choices.rs` (`group`, about lines 604-646; labels, tooltips and help, about lines 240-404).
- The sections' own modules under `crates/gui/farhelm-gui/src/settings/`: `appearance.rs`, `terminal.rs`,
  `file_types.rs`, `ntfy.rs`, `checkouts.rs`, `checkout_field.rs`, `containers.rs`, `recipes.rs` and `recipes/` (the
  editor, its leave question in `recipes/editor/leaving.rs`), `tensorlake.rs`, `tensorlake_key.rs`,
  `tensorlake_ssh_key.rs`, `tensorlake_sandboxes.rs`, `credentials.rs` and `credentials/`, `agent_trust.rs`, `app.rs`,
  and `search.rs` with `search/` (its catalog of labels and help, about lines 166-402).
- Sounds: `crates/gui/farhelm-gui/src/sounds.rs` (a private `play`, about line 103, through the installed player).
- Theme: `crates/gui/farhelm-gui/src/theme.rs` (tokens by role; host palette order lavender, orchid, teal, steel, sand,
  copper, red).
- Goldens: `crates/gui/farhelm-gui/src/goldens/settings.rs` has scenes for every section, Containers, Checkouts,
  Notifications and App included, plus Recipes' states and editor, Tensorlake's two states, Agent trust's top and end,
  and search (matches and none). Some tests and goldens find the scrolling body by its `settings-body` region
  (`settings/tests.rs`, `recipe_editor_tests.rs`, the scrolled goldens) and need retargeting if it moves.
- Shared components with other users: the checkout field (`checkout_field.rs`) is also used by the host settings dialog
  and the launcher's checkout setup; the recipe editor and its answers and leave question are also used by Add host's
  Tensorlake form, whose own footer places them. Those other users stay as they are.
- Reusable pieces: the dialog frame's footer slots (`components/dialog.rs`) hold the new footer without a frame change;
  the search pane (`settings/search.rs`, `pane`, about lines 651-697) already draws a fixed heading over its own scroll
  area, the shape every page's sticky heading can take (the scroll area must be a focus scroll region so Tab still
  scrolls a control into view); the public palettes `theme::DARK` and `theme::LIGHT` give the preview tiles their
  colors; the radio group's card style (`components/radio_group.rs`) covers the tiles and the 2 by 2 base grid once it
  takes a column count; `sounds.rs`'s `play` made crate-visible covers the preview.
- SPEC.md "Session list" (the paragraph that starts "The gear at the foot of the host rail opens a settings dialog",
  about lines 2420-2440) fixes the section order, the solid selection fill, the sub-headings and "Closing discards other
  unsaved fields"; its Sounds paragraph (about line 2519) says three switches where there are four; "Topology" (about
  line 541) describes the recipe editor's actions at the bottom; the Credentials paragraph (about line 2449) says
  "Closing discards unsaved text without a recipe-style departure question". SPEC_impl.md (about lines 6444-6449)
  describes the editor's answers in one bottom-sticky wrapper. `docs/gpui/parity/settings-and-hosts.md` lists deviations
  this plan resolves (the section list not pinned, among others).
- The docs page `website/src/content/docs/docs/using/settings.md` describes Settings; published docs screenshots show it
  (for example `manage-hosts/app-settings`).

### Acceptance criteria

1. Every page looks and behaves as the two "agreed design, in words" sections say, in both appearances, at the dialog's
   size and in a window small enough to cap it.
2. No control, state or wording is lost except the ones this file removes (the repeated sentence and its variants, the
   App line, the in-form Save buttons, the old footer note); every save, retry, refusal and busy rule works as today.
3. The pinned footer's Save and Discard work for every form (file types, ntfy alerts, the checkout fields, both copies
   of the Tensorlake key form) and the editors (recipe, credential); a page with nothing unsaved shows only the note and
   Close; Save, Discard and Close are reachable without scrolling on every page.
4. Closing with unsaved changes asks as described; with none, it closes at once.
5. The visual review below passes for every new or changed golden scene.
6. Golden scenes exist, light and dark, for every page in the mockup: Sessions, Appearance, Terminal (with an edited
   file-types field), Notifications (with an edited ntfy form), App, Hosts, Checkouts (edited), Containers (with the
   delete question open), Recipes (rows in several states), the recipe editor (scrolled to its top and to its end),
   Tensorlake (top and end), Agent trust (top and end), Credentials, the credential editor, search results, and the
   unsaved-changes question; keep the existing scenes' coverage (empty states and failures).
7. SPEC.md "Session list", "Topology" and the Credentials paragraph, and every other paragraph describing the dialog,
   the sections' order, their sub-headings, their Save buttons or closing, and the Sounds paragraph's switch count;
   SPEC_impl.md wherever it describes the Settings view; the parity doc's resolved deviations. The docs page's words
   match the new dialog; its screenshots are left for the next docs refresh, and the report says which ones are stale.
8. The last PR removes the TODO.md entry ("Settings dialog: grouped sections, cards and a pinned Save.").

## Decisions already made

Settled with the maintainer on the 2026-11-20 mockup rounds and the 2026-11-21 planning page, which drew every page:

- **Groups and order** as listed under the frame, App in General. The maintainer: "i assume my seleted options means to
  do what i just saw in the mockups"; it does.
- **One hue per group** (blue, teal, orchid), on the tiles only.
- **The repeated "This choice applies to this helm…" sentence moves to the scope line.**
- **Forms' Save goes to the pinned footer only**, with the editors' answers joining the footer row; Test, Clear, Send
  test, Retry, Refresh and Delete in lists stay in the page.
- **Closing with unsaved changes anywhere asks first**, naming the pages.
- **Kept additions**: the unsaved dots on sections and fields; the sticky page heading and pinned section list; a
  one-line description under each heading, with the wording reviewed on the mockup; play buttons beside the sounds;
  preview tiles for System, Dark and Light; agent marks on the Agent trust cards and in the credential editor.
- **Ordering**: this plan runs after `whats-new-on-upgrade.md`, whose Settings → App switch this plan includes.
- **Review gates**: a single fresh-context `gpt-6.2-sol` agent at `high` effort per PR, and, in addition, the mandatory
  visual review by `opus` at `high` effort described under "Visual review". The maintainer's words: "for the plans
  please have a special extra mandatory opus review for visual inspection as part of the plan. opus needs to be happy
  that the actual rendered result matches the intent of the mockup".
- **Release note**: the maintainer's own wording, under "Release note" below.
- **No-workhorse mode**: the plans default, stated on the planning page.

Planner decisions:

- The pinned Save generalizes what the recipe editor already does (its answers pinned above Close). The close question
  is one dialog-level question on the shared prompt component; the recipe editor's own leave question stays as it is for
  its Back and Cancel and for Add host, because its close path closes Settings outright and cannot sit inside a
  several-page save.
- Each form keeps its own save and reports how it ended; the frame only sequences them.
- The "changed from the default" dot from the first mockup round is not built: a dot means unsaved, and only that.
- The docs page's text is updated in the plan; its screenshots are recaptured by the next docs refresh.
- The card fill is the theme role `card` (dark `#20252f`, light `#ffffff`); `launcher-command-field.md` adds the same
  role with the same values, so whichever plan lands second finds it there and uses it.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the words quoted above, and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md",
  "Code rules" (no per-harness branching at the call site: the agent marks come from `marks.rs` and `harness.rs`), and
  its "Instructions by path" for every path you touch: `docs/harness-marks.md` before touching `marks.rs` or `marks/`,
  `website/AGENTS.md` before touching `website/`.
- `.agents/validation.md` for the GUI test selections and the screenshot goldens (re-capturing them with
  `scripts/check-gui-goldens.py --update`, inspecting every changed picture, and publishing them with
  `python3 scripts/gui-goldens.py publish` before any rebase, as it describes).
- `.agents/test-authoring.md` for every test added or changed.
- SPEC.md "Session list" (Settings), "Errors and diagnostics" ("A failed save restores the last known value and shows an
  adjacent error"); the theme module's rule that views never hold a color (`theme.rs`, module docs); the Tensorlake
  warning's text is the maintainer's and changes only on their say (`settings/tensorlake.rs`, module docs).

**Planner proposals:**

- The frame owns one notion of "this page has unsaved changes, and here are its save and discard": each form and editor
  exposes it, and the footer, the section dots and the close question read it. Build it by generalizing the recipe
  editor's pinned answers, not beside them; the frame sequences saves and never saves anything itself.
- The sound preview calls the existing `sounds::play`, made crate-visible.
- The preview tiles are drawn from `theme::DARK` and `theme::LIGHT`, not from pictures or new roles.

## Outline

1. `plan/settings-dialog-refresh/01-settings-frame`: the grouped section list with hue tiles and the pinned, scrolling
   list; the sticky page heading with tile, description and scope line; the page ground and card roles in `theme.rs`.
   Pages keep their bodies and the footer keeps today's note. SPEC.md's section order and the frame sentences.
2. `plan/settings-dialog-refresh/02-settings-pinned-save`: the pinned footer's Discard and Save for every form and
   editor (the in-form Save buttons removed), the new footer note, the unsaved dots, sequenced saves, and the close
   question; SPEC.md's saving and closing sentences, Topology and Credentials; SPEC_impl.md.
3. `plan/settings-dialog-refresh/03-settings-switch-pages`: cards for Sessions, Appearance (with the preview tiles),
   Terminal, Notifications (with the play buttons), App, Hosts and Agent trust (with the marks); the repeated sentence
   removed, in the pages and in the search catalog; search results restyled; the Sounds paragraph's switch count.
4. `plan/settings-dialog-refresh/04-settings-list-pages`: cards for Checkouts, Containers, Recipes and the recipe
   editor, Tensorlake, Credentials and the credential editor; the parity doc; the docs page's words; a changelog
   fragment; and removal of the TODO.md entry.

Each PR carries its own goldens, SPEC.md edits for what it changes, and the changelog fragment its type calls for. If PR
4 comes out much larger than the others, split the editors into their own PR above it.

## Fallbacks

- If `whats-new-on-upgrade.md` landed with its App switch worded or placed differently than expected, put it in the App
  card as it is; log it.
- If a page's content cannot fit its cards without dropping a control or a state, keep the control and the state and log
  how it was placed; never drop one silently.
- If a form's save cannot report how it ended without changing its own rules (refusal, read-back, kept drafts), add the
  smallest completion signal to it and keep every rule; log it.
- If sticking the page heading needs a scroll container the dialog component does not have, add it to the Settings view
  rather than to the shared dialog component unless the component already offers one; log it.
- Decisions to log: the exact sizes and gaps you chose, the theme roles added and their values, every golden whose
  change you judged intended, and every review finding declined, with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-settings-dialog-refresh-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-settings-dialog-refresh-watchdog.json` beside it. Resolve
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: `plan/settings-dialog-refresh/01-settings-frame`,
  `plan/settings-dialog-refresh/02-settings-pinned-save`, `plan/settings-dialog-refresh/03-settings-switch-pages`,
  `plan/settings-dialog-refresh/04-settings-list-pages` (and, if the outline splits PR 4, an editors PR above it named
  in the same pattern).
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

Mandatory, in addition to the review gate, for every PR that changes what the Settings dialog looks like: the maintainer
demands a fresh-context agent on `opus` at `high` effort, launched natively or through `$scode-harness-shellout`, which
must be satisfied that the rendered result matches the intent of the agreed mockup. Never substitute another model, and
never skip it because the review gate passed. If it cannot be reached after a reasonable retry, block per
`plans/.agents/executing.md`, "Executing one plan" (step 10).

- **What it looks at.** Run `scripts/check-gui-goldens.py --update` (as `.agents/validation.md` describes) so the PR's
  golden scenes are captured, light and dark, into `target/gui-goldens/captures`, and give the reviewer the paths of
  every new or changed picture for the PR's surfaces. Give it the mockup page,
  `https://snippets.scode.org/s/farhelm-launcher-settings-agreed/` (section "Settings, every page"), with the
  instruction to compare in both of its themes (the page's "Steel dark" and "Steel light" buttons), and the design words
  of this file ("The agreed design, in words", the acceptance criteria and the decisions), which win where the mockup
  and the words differ. If the reviewer cannot fetch the page, it judges against the words alone and says so.
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
visual review, with the last one removing the TODO.md entry this plan covers ("Settings dialog: grouped sections, cards
and a pinned Save."). Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied.
Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through
the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

Settings dialog has been improved somewhat. Still needs work, but should be a bit more pleasant.
