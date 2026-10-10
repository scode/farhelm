# Remote hosts get a chosen icon and color, and the host settings dialog is refreshed

Written against main at 2af378bd on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan. The `resizable-sidebar` and `approval-card-layout` plans, queued before
this one, also edit `crates/farhelm-ui/assets/app.css` and the sidebar; if either lands first, rebase onto it carefully
(root `AGENTS.md`, Careful rebase).

## The goal

Every remote host shows the same cloud in the session list, so sessions on different hosts look alike. Let the user pick
each remote host's icon from a set Farhelm draws, and a color for it, so a session's host can be told apart at a glance.
While in the host settings dialog, give it a refresh: it should look a little less basic, but stay tasteful.

Acceptance criteria:

- Every remote host has an icon, one of the fifteen in the Appendix, and a color: `default` or one of the six named in
  D2. A host nobody has set shows the cloud in the default color, exactly as today. The local host keeps its red laptop
  and offers no choice (D1).
- The choice is stored on the helm, beside the host's alias, and every client sees it: the browser and the desktop app
  alike (D6), updated in other open clients through the existing change feed like every other host setting. It is stored
  as stable words (`rocket`, `teal`), never as indexes into a list, so reordering the set or adding to it later keeps
  existing choices. The helm refuses an icon or color word it does not know, and refuses a choice for the local host.
- The chosen icon, in its color, shows everywhere a host is identified (D3): the session row's host mark (where the
  cloud is today), the hosts panel's host rows, the quick switcher's session entries, and the launcher's host picker.
- The launcher's host picker becomes a Farhelm-drawn picker, consistent in overall style with the quick switcher dialog
  and adapted as needed, that shows each host's icon and name (D4). It keeps every behavior of today's native select
  (`select.create-session-host` in `crates/farhelm-ui/src/list/create_form.rs`): disabled for the whole round trip while
  busy; the same change handling (the draft-transition guard, the destination history retarget, the clone-host takeover,
  the intent-key reset, the directory-browse invalidation); Enter launching as today's `enter_choice` does; the empty
  value before the first hosts read lands; and the order and labels of `HostOption`. It is fully usable from the
  keyboard (open, arrow keys, Home and End, Enter or Space to choose, Escape to close without changing anything) and is
  exposed to assistive technology as a single-choice list named "host". Build it from the combobox pattern the
  launcher's model picker and the quick switcher already use, rather than a new widget. It never traps focus or leaves a
  menu open behind a dialog closing. The template editor's host field (`crates/farhelm-ui/src/list/templates.rs`) stays
  a native select: only the launcher was asked for.
- The host settings dialog is refreshed along the lines the maintainer approved from a mockup (D5), described under
  Outline. Every existing behavior stays: each setting saves the moment it is changed with its own outcome message, only
  one field is edited at a time (everything else disabled while one is open or a save is in flight), the destination and
  alias editors work as today, the local host's dialog has no destination, and the destination and alias keep their
  existing visibility rules. The dialog's base classes (`.host-settings-dialog`, `.host-settings-row` and friends) style
  about eight dialogs (app settings, feedback, the template editor, add host, uninstall, remove host and others): put
  the refresh under classes only the host settings dialog uses, and check that none of the others changed.
- The six colors become app.css tokens with comments saying they are host identity colors and carry no state, and each
  passes the contrast check in `crates/farhelm-ui/js-tests/app-css-contrast.test.js` against the surfaces it is drawn on
  (the sidebar, a selected row, a dialog, the quick switcher). If one fails, adjust its lightness, keep its hue, and log
  the DECISION. Never use a status color (green, amber, red, the accent or info blues) for a host.
- SPEC.md's Topology section, where the host settings dialog's fields are listed, adds icon and color and says the local
  host keeps its mark; the Session list section's locality-mark text says a remote row shows the host's chosen icon and
  color. SPEC_impl.md records the stored words, the validation, the route, the tokens, and the launcher picker's
  semantics. The docs website's host page (`website/src/content/docs/docs/using/manage-hosts.mdx`, "Host settings") says
  you can pick an icon and a color and where they show.
- Tests per Validation: the helm store migration (`a_migrated_database_matches_a_freshly_created_one`), the route
  (valid, unknown word, local host, unknown host, the feed bump), and browser tests on Chromium and WebKit for picking
  in the dialog and seeing it in every place listed above, in this client and in a second one, plus the launcher
  picker's keyboard behavior and Enter-to-launch. Existing tests that count tooltips, read `data-glyph`, or drive the
  host select or the settings checkboxes are updated, not deleted.
- The last code PR removes the TODO.md entry "Pick an icon and color per host." (Near term).
- A linear stack of draft PRs, each having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, Near term, the maintainer's words):**

"Pick an icon and color per host. Every remote host shows the same cloud in the session list, so sessions on different
hosts look alike. Let the user pick a host's icon from a set of about ten Farhelm draws, with the cloud kept as one
choice, and ideally a color as well, so a session's host can be told apart at a glance."

**The user's decisions (2026-10-09):**

- D1. Remote hosts only. The local host keeps its red laptop, which already reads as "this machine".
- D2. Colors: `default` (today's text color, `--fg-0`) plus six muted colors chosen to sit away from every status hue,
  all six kept: lavender `#b8a4f0`, orchid `#e090d8`, teal `#4fc8c0`, steel `#93aec4`, sand `#c8b48e`, copper `#d0906a`.
  The color tints the icon only.
- D3. Shown on session rows and in the hosts panel (where the cloud is today), and also in the quick switcher and the
  launcher.
- D4. The launcher's host dropdown is replaced by a custom picker, in the maintainer's words: "custom picker, be
  consistent with the quickswitch dialog in overall style etc but adapt as needed".
- D5. The icon set is the fifteen the maintainer picked from a sheet of 63 candidates (Appendix, with their geometry).
  The maintainer also asked, about the host settings dialog: "feel free to spruce it up a little while we're here, look
  a little less basic but still tasteful", and was shown the mockup described under Outline.
- D6. Stored in the helm's database. The maintainer accepted the usual tradeoff of a schema change: an older helm cannot
  open the upgraded database, and updating from v0.23.0 or later keeps working (SPEC.md, Upgrade compatibility and
  client scale).
- D7. Review gate: a fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** (challenge them through Scope reassessment rather than treating them as requirements):

- Two nullable text columns on the `hosts` table (`icon`, `color`), null meaning the default, added at the end after the
  existing per-host settings, with the schema version bumped as `apply_schema`'s doc comment describes. One route,
  `POST /api/hosts/{id}/appearance`, taking both words with `deny_unknown_fields` like `AliasSpec`, taking the host
  write lock, saving, and bumping the feed only on a change. `HostView` carries both; the UI's `Host` decodes them with
  plain defaults, as it does the YOLO and commands settings (the UI is served by the helm it talks to, so a missing
  field is not a case to design for).
- The icon and color vocabulary lives in one Rust enum pair shared by the helm and the UI (`farhelm-proto` is the
  natural home), so the helm's validation and the UI's rendering cannot drift.
- One `HostMark` component in `crates/farhelm-ui/src/icons.rs` draws any of the fifteen, with `data-glyph` set to the
  word, in the existing 16-unit box at the 1.3 stroke weight, with the word written straight into `data-glyph` (`cloud`
  for the default); the two test lines that read `remote` change to `cloud`.
- The settings toggles stay real checkboxes underneath, styled as switches, so keyboard behavior, labels and the
  existing test hooks survive.
- The icon and color choices are plain buttons with `aria-pressed`, each group labelled, not radio groups: arrowing
  through a radio group checks each option in turn, which here would save on every key press and drop focus as each save
  disables the controls.
- The icon geometry in the Appendix was drawn for Farhelm during planning, not copied from an icon set, so it needs no
  third-party notice.

**What the planner found (at 2af378bd).**

- Icons: `LocalHostIcon` (laptop, `data-glyph="local"`) and `RemoteHostIcon` (cloud, `data-glyph="remote"`) in
  `crates/farhelm-ui/src/icons.rs`, stroke-only in a 16-unit box at weight 1.3, `currentColor`, shown at 12×12px by
  `.host-kind-icon`. App.css colors a local session's mark red with
  `.session-locality-slot .host-kind-icon[data-glyph="local"]`. `docs/harness-marks.md` covers the harness marks, a
  different convention; read it, and add a short section for host marks if the file is where such rules are kept.
- Where the mark is drawn: `list/row.rs` (`span.session-locality-slot`, chosen by `session_locality` in
  `list/shared.rs`) and `hosts.rs` (`HostRow`, chosen by `HostKind`). A session row knows only its host id; the row
  looks the host up in the hosts read `ListView` already holds. The quick switcher (`list/quick_switcher.rs`, styled by
  `.quick-switcher-dialog` rules in app.css) and the launcher's host list (`HostOption::label`) show text only today.
  The quick switcher has no hosts read of its own; give it the hosts `ListView` already holds rather than a new fetch.
  The launcher's model picker is already a custom combobox.
- Storage: `crates/farhelm-helm/src/store.rs`, `helm.db`, `SCHEMA_VERSION = 43`; the fresh-create block's
  `PRAGMA user_version` literal must match it (a mismatch has shipped before). Per-host settings and their routes:
  `crates/farhelm-helm/src/hosts.rs` (`/api/hosts/{id}/alias`, `/yolo-without-asking`, `/commands-without-asking`),
  registered in the helm's `lib.rs`. The UI side: `Host` in `crates/farhelm-ui/src/lib.rs`, calls in `api.rs`, the
  dialog in `crates/farhelm-ui/src/hosts/settings_dialog.rs`.
- Dialog styles: `.host-settings-backdrop`, `.host-settings-dialog` (shared with `.app-settings-dialog`),
  `.host-settings-row`, `.host-settings-yolo`, `.host-settings-commands`, `.host-settings-help`,
  `.host-settings-actions` in app.css. The app has one dark theme today and plans a light one as a second `:root` block,
  so every new color is a token.
- Tests that touch this: `e2e/tests/sidebar.spec.ts` (glyphs, the local red, aliases), `terminal-multihost.spec.ts`
  (local and remote glyphs, opening host settings), `tooltip-coverage.spec.ts` (the settings dialog's tooltip count),
  `provisioning.spec.ts` (the settings menu item), and the docs-shots spec for the host settings screenshot. The
  launcher's host select is driven in about 38 places across six browser specs and a screenshot-capture spec
  (`selectOption` on `select.create-session-host` and similar); move them to one shared helper rather than editing each
  by hand.

**Binding repository constraints:** root `AGENTS.md`: Finishing work (targeted validation; browser specs on Chromium and
WebKit through the recorder; the JS harness; the test-sleep check when browser tests change), Releases and the changelog
(a fragment for `feat` and `style`), Docs website (read `website/AGENTS.md` and `website/EDITORIAL_RULES.md` before
editing a docs page), Docs screenshots (screenshots are regenerated in bulk by their own procedure: this plan does not
refresh them, and the report says which shot is now out of date), Harness marks in the sidebar, Desktop/web UI bug
triage, Testability, Sharing the machine, Agent scratch space, The live install is off-limits. SPEC.md's "Upgrade
compatibility and client scale". `.agents/test-authoring.md` for any test change.

## Outline

The PR slicing below is a proposal; reshape it if the code argues for it, without churn.

### PR 1: refresh the host settings dialog

`style:` with a changelog fragment. No new setting yet, so the refresh is reviewed on its own. The approved mockup, in
words:

- A header band with a slightly lighter top, holding a 44px rounded tile on the left that shows the host's mark at about
  24px (the local host's red laptop, a remote host's cloud), then the host's shown name in bold and, under it, the
  host's connection state with its status dot and the supervisor's version where known, and a quiet close `×` on the
  right that does what the close button does.
- The body grouped into sections, each under a small, letter-spaced uppercase label in the faint text color, separated
  by the quiet rule: Connection (destination and alias, label column on the left, value, and a light `edit` button that
  only shows a border on hover), and Permissions (the two existing settings).
- The two settings drawn as switches on the right, the setting's name on the left in the primary text color and the
  existing on/off help text directly under it in the secondary color. The labels are sentence case ("Start YOLO sessions
  without asking", "Run farhelm commands without asking"); keep their meaning and tooltips.
- A footer bar on a slightly darker surface with "changes save as you make them" on the left in the faint color and the
  close button on the right, replacing today's lone close button.

Use existing tokens; add one only where no existing token fits, with a comment. Keep the dialog's width, its scrolling
when the window is short, and its modal behavior. Update the browser tests that drive the dialog and the tooltip count.

### PR 2: pick an icon and a color for a remote host

`feat:` with a changelog fragment. The shared vocabulary, the two columns and the schema bump, the route, `HostView` and
the UI's `Host`, the mark component, the six tokens with their contrast tests, and an Appearance section in the
refreshed dialog, between Connection and Permissions, shown only for remote hosts:

- "icon": the fifteen marks as a grid of square buttons (about 32px, the mark at about 14px), eight to a row; the
  selected one filled with the accent fill and outlined in the accent, and drawn in the host's chosen color. Plain
  buttons with `aria-pressed`, in a group labelled "icon", each named by the icon's word.
- "color": seven round swatches, the selected one ringed, followed by the chosen color's name. Also plain buttons with
  `aria-pressed`, in a group labelled "color".
- Under them, a small preview box on the sidebar's surface labelled "in the session list", showing a mock session row
  drawn the way a real row draws its host mark (on the row's first line, where the locality slot is), in the host's
  color.
- The header tile follows the choice live.

Each click saves at once, like the switches, with the outcome shown for the section. Draw the chosen mark on session
rows and in the hosts panel. SPEC.md, SPEC_impl.md and the docs page. Browser tests: pick, see it on the row and in the
hosts panel, see it in a second client after the feed bump, reload and keep it; the local host has no Appearance
section; an unknown word is refused by the helm.

### PR 3: the mark in the quick switcher and a new launcher host picker

`feat:` with a changelog fragment. Show each session's host mark in the quick switcher's entries. Replace the launcher's
native host select with the custom picker described in the acceptance criteria, styled like the quick switcher's list
(its row height, hover and selected treatment, and fonts), showing each host's mark and name. Browser tests for the
picker: mouse choice, keyboard choice and type-ahead, Escape, disabled while busy, Enter launching, the host change
resetting what today's select resets, and the empty state before hosts arrive. Remove the TODO.md entry.

### Out of scope

A choice for the local host, free-form colors or uploaded icons, coloring anything but the mark (the row, the name, the
terminal), host marks in the template editor's host field, and refreshing the docs screenshots.

### Validation

Per root `AGENTS.md` Finishing work, choose targeted checks per PR: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the helm store and host
route tests through the recorder, `cd crates/farhelm-ui/js-tests && node --test`, the relevant Playwright specs (the new
ones, `sidebar.spec.ts`, `terminal-multihost.spec.ts`, `tooltip-coverage.spec.ts`, `provisioning.spec.ts`, and the
launcher's specs) on Chromium and WebKit through the recorder after `cargo build` and the `dx` web build, the test-sleep
check, `cd website && bun install --frozen-lockfile && bun run build` when the docs page changes, `dprint check`, and
`python3 releasing/check-changelog.py format`. Look at the result yourself in browser screenshots (the dialog for a
remote and the local host, rows with several marks and colors, a selected row, the quick switcher, the launcher picker
open) and describe what you saw in the report.

## Appendix: the fifteen icons

Each is drawn in a 16-unit box with `fill="none"`, `stroke="currentColor"`, `stroke-width="1.3"`, round caps and joins.
The first word is the stored word. Adjust geometry only to fix something that renders badly at 12px, and log it.

| Word       | SVG body                                                                                                                                                                                                                       |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `cloud`    | `<path d="M4 12.5a2.5 2.5 0 0 1-0.3-4.98A3.7 3.7 0 0 1 11 6.3a2.6 2.6 0 0 1 1.2 4.9V12.5Z"/>` (today's cloud)                                                                                                                  |
| `house`    | `<path d="M2 7.5 8 2.5l6 5"/><path d="M3.5 6.3v7.2h9V6.3"/><path d="M6.8 13.5V10h2.4v3.5"/>`                                                                                                                                   |
| `flask`    | `<path d="M6 1.8h4M6.8 1.8v4.4L2.9 12.7A1.2 1.2 0 0 0 4 14.5h8a1.2 1.2 0 0 0 1.1-1.8L9.2 6.2V1.8"/><path d="M4.6 10h6.8"/>`                                                                                                    |
| `database` | `<ellipse cx="8" cy="3.8" rx="5" ry="2"/><path d="M3 3.8v8.4c0 1.1 2.2 2 5 2s5-.9 5-2V3.8"/><path d="M3 8c0 1.1 2.2 2 5 2s5-.9 5-2"/>`                                                                                         |
| `chip`     | `<rect x="4" y="4" width="8" height="8" rx="1"/><rect x="6.3" y="6.3" width="3.4" height="3.4"/><path d="M6 1.8V4M10 1.8V4M6 12v2.2M10 12v2.2M1.8 6H4M1.8 10H4M12 6h2.2M12 10h2.2"/>`                                          |
| `rocket`   | `<path d="M8 1.8c2.3 1.6 3.2 4 3 6.8l-1.4 2.2H6.4L5 8.6C4.8 5.8 5.7 3.4 8 1.8Z"/><path d="M5.2 8.4 3.2 10.5l.3 2.3 2.6-1.4M10.8 8.4l2 2.1-.3 2.3-2.6-1.4"/><path d="M7.2 12.6 8 14.4l.8-1.8"/><circle cx="8" cy="6" r="1.1"/>` |
| `gear`     | `<circle cx="8" cy="8" r="2.2"/><circle cx="8" cy="8" r="4.6"/><path d="M8 1.6v1.8M8 12.6v1.8M1.6 8h1.8M12.6 8h1.8M3.5 3.5l1.3 1.3M11.2 11.2l1.3 1.3M3.5 12.5l1.3-1.3M11.2 4.8l1.3-1.3"/>`                                     |
| `gem`      | `<path d="M4.3 2.5h7.4L14 6 8 13.8 2 6Z"/><path d="M2 6h12M6 2.5 5.2 6 8 13.8 10.8 6 10 2.5"/>`                                                                                                                                |
| `hexagon`  | `<path d="M8 1.8 13.4 4.9v6.2L8 14.2 2.6 11.1V4.9Z"/>`                                                                                                                                                                         |
| `triangle` | `<path d="M8 2.2 14 13.3H2Z"/>`                                                                                                                                                                                                |
| `ring`     | `<circle cx="8" cy="8" r="5.5"/><circle cx="8" cy="8" r="2"/>`                                                                                                                                                                 |
| `square`   | `<rect x="2.8" y="2.8" width="10.4" height="10.4" rx="1.5"/>`                                                                                                                                                                  |
| `bug`      | `<ellipse cx="8" cy="9.3" rx="3.4" ry="4.4"/><path d="M8 5v8.6M5.6 3.4 6.7 5M10.4 3.4 9.3 5M1.8 7.5h2.8M11.4 7.5h2.8M1.8 11h2.8M11.4 11h2.8"/>`                                                                                |
| `factory`  | `<path d="M1.8 13.8V7.4l3.8 2.4V7.4l3.8 2.4V2.2h3.6v11.6Z"/>`                                                                                                                                                                  |
| `castle`   | `<path d="M2.5 14V3.5h2v2h2v-2h3v2h2v-2h2V14Z"/><path d="M6.6 14v-3a1.4 1.4 0 0 1 2.8 0v3"/>`                                                                                                                                  |

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-host-icons-log.md` in the
parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before you
start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
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
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/host-icons/<nn>-<short-name>`.
- Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack deletes. Within this run, if
  a PR needs correcting, restructure it rather than stacking a correction on top; that applies to all of this plan's own
  open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entry this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
that PR's changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review
swarm: a fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the
executing one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (a free-form color or icon upload, a per-client
appearance setting, a generic dropdown framework used beyond the launcher's host picker, restyling dialogs other than
host settings and remove-host; these are examples, not a blacklist), and whenever the same component has needed repeated
corrective review rounds, run a fresh-context review through galaxy-brain with this charter, supplying the user's
request and decisions above, this outline, the current diff and the proposed departure (what changed, why it is
necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the stored words and the route's shape, where the shared vocabulary lives, what
`data-glyph` holds for the default cloud, any color whose lightness changed for contrast, how the custom launcher picker
is exposed to assistive technology and how it handles keys, and any dialog-refresh detail that departs from the mockup
description, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
