# Help menu and feedback dialog: link to Farhelm's Discord

Written against main at 856099b8 on 2026-11-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Add a Discord link to the feedback dialog and the ? menu.** The website header already links Discord (#2869);
> surface the same link in the app, where users who just had a problem can find it.

NOTE: This is a small UI change. It adds one menu item and one line of text with a link, plus the spec, docs, test and
changelog updates that go with them. It is not a redesign of the help menu or the feedback dialog.

### Acceptance criteria

- **The `?` menu** has a **Chat on Discord** item directly after **Send feedback**, in the desktop app and the web UI
  alike. Choosing it opens the Discord invite `https://discord.gg/G3bukxTjXR` in the user's browser the way
  **Documentation** opens the docs (a new tab from the web UI, the system browser from the desktop app), and closes the
  menu. Its hover text and description line follow the other items' patterns. Every other item keeps its relative order,
  and the desktop app's **Check for updates** stays last.
- **The feedback dialog** shows the line "Rather talk in the open? Chat on Discord" under its header summary ("Goes
  privately to Farhelm's maintainer, not a public tracker"), with "Chat on Discord" as a link that opens the same invite
  in the browser through the same opener. The link has hover text, like every other control.
- **SPEC.md, "Feedback"** lists the menu's items in their new order and says what Chat on Discord does, and "Sending
  feedback" mentions the dialog's Discord line.
- **The docs page** `website/src/content/docs/docs/using/send-feedback.md` names the new menu item and the dialog's
  link.
- **Tests** pin the new order (the Rust menu-order test and the browser menu test) and prove that both the menu item and
  the dialog link open the invite through the shared link opener.
- **Changelog and TODO:** a changelog fragment per root `AGENTS.md`, "Changelog fragments", and the PR removes the
  TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-11-03, on the planning page:

1. **The menu item:** the `?` menu gets a "Chat on Discord" item placed directly after "Send feedback". It opens the
   Discord invite in the browser the way Documentation opens the docs.
2. **The dialog line:** the feedback dialog gets a short line under its header summary: "Rather talk in the open? Chat
   on Discord", with "Chat on Discord" as the link, opening the same invite in the browser.
3. **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort for every PR (see Review gate). If it cannot
   be reached when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute
   reviewer.
4. **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above. The two user-visible strings, "Chat on Discord" and
"Rather talk in the open? Chat on Discord", are the maintainer's words; keep them exactly.

**Binding repository constraints:**

- SPEC.md, "Feedback" (around 3319-3339) and its "Sending feedback" subsection (around 3357-3385).
- Root `AGENTS.md`: "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation",
  "Instructions by path" (which sends you to `website/AGENTS.md` before editing the docs page and to
  `.agents/docs-media.md` before editing anything under `e2e/docs-shots/`), and "Frozen material" (published release
  shots and their tests).
- `website/AGENTS.md` and `website/EDITORIAL_RULES.md`, read in full before the docs edit. This is an incidental docs
  edit in an unattended run: start no preview server, and name the changed page in the report instead
  (`website/AGENTS.md`, "Previews and reports").
- `.agents/test-authoring.md` for the test changes, `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.
- `releasing/EDITORIAL_GUIDANCE.md` for the fragment's wording.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- One PR, `plan/discord-link-in-app/01-discord-link`, titled along the lines of
  `feat: link to discord from the help menu and feedback dialog`.
- One `DISCORD_URL` constant beside `DOCS_URL` in `crates/farhelm-ui/src/app_bar.rs`, `pub(crate)` so the feedback
  dialog uses the same one, with a doc comment saying it is the invite the README and the website header use.
- Hover text and description in the existing items' pattern, for example tooltip "Chat on Discord: open Farhelm's
  Discord in your browser, to talk with the maintainer and other users in the open" and description "Opens Farhelm's
  Discord in your browser".
- The item reuses an existing glyph, `ActionGlyph::External` (the item leaves the app; `Feedback`, the speech bubble, is
  the item directly above it). No Discord logo and no new glyph: neither was asked for, and a third-party mark would
  need a `THIRD_PARTY_NOTICES.md` entry and a change to `crates/farhelm-ui/src/icons.rs`, which `docs/harness-marks.md`
  governs.
- The dialog's link follows the app's existing in-app external links (`welcome.rs` around 62-67, `checkout_spare.rs`
  around 73-81): an `a` with `href`, `target: "_blank"`, `rel: "noopener"`, a `data-tooltip`, and an `onclick` that
  prevents the default and calls `open_external_page`. Those links end with " ↗"; appending it to "Chat on Discord" is
  your call, logged as a DECISION. Following the link leaves the dialog open with whatever the user typed, as Read Get
  started leaves the welcome card open.
- The welcome card, the installer's closing message, the help menu's own header summary ("Farhelm guides and feedback")
  and the README stay as they are.

## What the code looks like at 856099b8

Line numbers are approximate.

**The `?` menu** lives in `crates/farhelm-ui/src/app_bar.rs`:

- `enum BarMenuItem` (around 69-85) holds both bar menus' items. Each item needs an arm in `glyph` (around 90-100),
  `label` (around 103-115; the label is also the item's `data-bar-menu-item` marker the tests select by), `tooltip`
  (around 121-151) and `description` (around 154-166).
- The order lives in two lists: `BAR_MENU_ITEMS` (around 169-179, a fixed-size array whose length also sizes
  `BarMenuOrder` and the menu wiring) and `help_menu_items` (around 185-198), which appends Check for updates only when
  an updater runs.
- `DOCS_URL` (around 204) and `RELEASE_NOTES_URL` (around 206-212) are the URL constant pattern.
- `open_external_page` (around 214-238) opens a constant URL through the page's shared link opener
  (`window.farhelmTerminalLinks.openTerminalUrl`, from `crates/farhelm-ui/assets/terminal-links.js`). Under the desktop
  webview (`dioxus:` origin) that navigates the page, which the desktop shell turns into a system-browser open; in a
  browser it is `window.open(url, '_blank', 'noopener')`. Its fallback repeats both branches for a click that lands
  before the opener script has loaded. The Discord invite is an ordinary https URL, so it needs nothing new here.
- The help menu's `on_choose` (around 953-978) maps each item to its action (`Documentation => open_external_page(...)`
  around 958). The update menu's `on_choose` (around 987-1002) lists every help item in a no-op arm, so the new variant
  goes there too.
- The unit test `the_help_menu_lists_release_notes_before_the_update_check` (around 1180-1200, under the comment
  starting "Spec (Feedback): the `?` menu lists ...") pins `help_menu_items` for both the web and the updater case. Its
  doc comment says the browser suite never has an updater, so it is the one place the desktop order is pinned. Update
  the list, the comment, and the name if it no longer fits.
- No native (OS) menu mirrors these items; nothing outside `farhelm-ui` names them.

**The feedback dialog** is `crates/farhelm-ui/src/feedback.rs`. Its header is a `crate::modal_dialog::DialogHeader`
(around 270-287) whose `summary` is "Goes privately to Farhelm's maintainer, not a public tracker" (around 272). The
summary is an `Element` rendered inside `div.dialog-summary.host-settings-summary`
(`crates/farhelm-ui/src/modal_dialog.rs` around 34-56); in `crates/farhelm-ui/assets/app.css` `.host-settings-summary`
is a wrapping flex row with a 12px gap (around 3968-3975) and `.dialog-summary` sets the size and colour (around 9480).
So a second child of the summary would sit beside the first on a wide dialog unless it is made a full-width line. The
maintainer asked for the line under the header summary: inside the header, on its own line below the summary text, is
the closest reading, and the way to do it: keep the line inside the header (two children in the summary plus one
full-width rule for this dialog, or the shared dialog header's existing `summary_class` option; the shared header needs
no new option). Do not move it to the top of the body: that would put the link between the close button and the message
field in Tab order, which `keyboard-scroll.spec.ts` ("sent feedback is keyboard-scrollable without changing the editing
Tab order") pins. In the header the link comes before the close button, which that test allows. The dialog's initial
focus goes to the message `textarea` (around 248-250), so a link in the header does not take it. The body's states
(editing, sending, sent, failed) are in a `ScrollRegion` (around 288-374); the header shows in every state.

**SPEC.md, "## Feedback"** (around 3319-3328) says the menu has "six items, in order: **Send feedback**,
**Documentation**, **Keyboard shortcuts**, **Release notes**, **SSH needs 2FA?** and **Welcome**", describes how
Documentation and Release notes open, and says the installed desktop app's menu has "a seventh item, **Check for
updates**". Both counts change (seven, and an eighth item), and Chat on Discord gets a sentence saying it opens the
project's Discord invite the same way as Documentation. "Sending feedback" (around 3359-3366) already says opening a
public issue stays possible; the dialog's Discord line belongs there. SPEC_impl.md has nothing that lists the items
(around 6415 it says the releases page uses the same external-link path as Documentation, which stays true).

**The docs page** `website/src/content/docs/docs/using/send-feedback.md` already mentions Discord in prose (around
12-14: "talk it through with the maintainer and other users on [Discord](https://discord.gg/G3bukxTjXR)"), and its last
paragraph (around 37-43) walks through the menu's other items. Add Chat on Discord there and mention the dialog's link,
in the page's own voice. The Overview page (`website/src/content/docs/docs/index.mdx`, around 88) also links Discord; it
is the maintainer's page and stays untouched (`website/AGENTS.md`, "The Overview page").

**The invite URL** is `https://discord.gg/G3bukxTjXR`, the same in `README.md` (around 21 and 109),
`website/astro.config.mjs` (around 94, the header's social link), the send-feedback page and the Overview page.

**The welcome card** (`crates/farhelm-ui/src/welcome.rs`) does not list the menu's items. It mentions Send feedback "in
the ? menu" and the Welcome item, neither of which moves, so it needs no change.

**Browser tests that see the menu or the dialog** (all under `e2e/tests/` unless noted):

- `feedback.spec.ts`: the URL constants (around 19-20); "the help menu sits below the gear and follows the menu
  conventions" (around 162-223) asserts six items by index (around 172-178) and walks them with ArrowDown, Home and End
  (around 184-200), so it needs the new count, order and walk; the loop "`${label}` opens the docs site through the
  shared link opener" (around 329-357) replaces the opener with a recorder and checks the URL, which is the pattern for
  the new item and for the dialog's link. "feedback uses dark fields and pinned chrome at normal and short heights"
  (around 60) checks the dialog's geometry at short heights, which the extra line could disturb.
- `tooltip-coverage.spec.ts` (around 312-322) sweeps the help menu and the feedback dialog for controls without hover
  text, `a[href]` included (`CONTROLS` around 55), and fails on any native `title`. The dialog's link needs a
  `data-tooltip`.
- `filters.spec.ts`, "rail Help menu points at its footer button" (around 490-510), requires the open menu to fit a
  240px-tall viewport. A seventh item makes the menu taller; check it.
- `keyboard-scroll.spec.ts`: its feedback-dialog Tab-order tests pin that Tab from the close button lands in the message
  field, which is why the new line stays in the header (above).
- `modal-layout.spec.ts` shows the feedback dialog at 360x220, the tightest size in the suite, where the taller header
  matters most.
- `menu-highlight.spec.ts` and several other specs open the menu but pick items by label, so a new item cannot affect
  them; no need to run them for this.
- The loop "`${label}` opens the docs site" in `feedback.spec.ts` gains the new item; retitle it, since Chat on Discord
  does not open the docs site. Add one small test for the dialog's link with the same stand-in opener (the desktop path
  depends on the link's click handler). The tooltip sweep's count is a minimum and needs no change.
- `e2e/docs-shots/visual-review.spec.ts`, `openHelp` (around 1988-1997), asserts six menu items and says so in its
  comment. It is a development-review harness whose shots no docs page shows (`docs/docs-shots/SPEC.md` around 134-140);
  update the count and the comment, after reading `.agents/docs-media.md` and `docs/docs-shots/SPEC.md` as "Instructions
  by path" requires.
- `e2e/docs-shots/release-notes.spec.ts` photographs the help menu and the feedback dialog for published release notes
  (around 432 and 1116-1120). Those shots are frozen: leave the file and the shots alone even though they will look
  dated (`website/AGENTS.md`, "Published release notes are frozen").

**Rust unit tests:** the menu-order test above, and `feedback.rs`'s test module (around 431-470), which covers contact
memory, OS names and close rules, none of which this changes.

## Outline

1. `plan/discord-link-in-app/01-discord-link`, one PR and one commit:
   - the `BarMenuItem` variant with its glyph, label, tooltip and description, its place in both order lists and both
     `on_choose` matches, and the `DISCORD_URL` constant;
   - the dialog's line and link;
   - SPEC.md's "Feedback" and "Sending feedback", and the send-feedback docs page;
   - the updated menu-order unit test, the browser tests above, and `visual-review.spec.ts`'s count;
   - the changelog fragment (kind `added`), and the TODO.md entry removed, including its `Plan:` reference.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect:

- `dprint check` on the changed files, `cargo fmt --check`, and clippy on `farhelm-ui`;
- `cargo check -p farhelm-ui --features desktop` (the desktop renderer compiles nowhere else);
- the `farhelm-ui` unit tests, through `python3 scripts/record-test-run.py`;
- `python3 releasing/check-changelog.py format`;
- `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares;
- `cd website && bun run build` for the docs page (it checks internal links);
- through the recorder, on Chromium and WebKit, after the web build `.agents/validation.md` describes:
  `feedback.spec.ts`, `tooltip-coverage.spec.ts`, `filters.spec.ts`, `keyboard-scroll.spec.ts` and
  `modal-layout.spec.ts`.

Whether to run `visual-review.spec.ts` (it needs a staged fleet) is your judgment; if you skip it, say why in the
report.

## Fallbacks

Examples of departures that need the scope reassessment:

- adding a Discord logo, a new glyph, or anything else in `crates/farhelm-ui/src/icons.rs`;
- changing the welcome card, the installer's closing message, or the help menu's layout beyond the new item;
- a setting to hide the link, or any behavior beyond opening the invite;
- touching published release shots or `e2e/docs-shots/release-notes.spec.ts`.

If the menu no longer fits the 240px-tall viewport in `filters.spec.ts`, fix the fit within the existing menu machinery
(what keeps the menu inside a short window is its computed maximum height in `crates/farhelm-ui/src/app_bar.rs`, around
578; no browser test has shown seven items before, though the desktop app ships them) rather than dropping the new item
or moving it; if that cannot be done without changing how the menu lays out, that is a departure for the scope
reassessment.

Decisions to log: the glyph, the hover and description wording, where the dialog line sits and how it is styled, whether
the link ends with " ↗", every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-discord-link-in-app-log.md` in the parent directory of the checkout you run in, derived as that section
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
without costing quality; for a change this small, doing it yourself is probably quickest. Concurrent agents that might
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

- One reviewable commit, stable bookmark and draft PR, using the `jjstack` skill. The stack's base is `main@origin`, or
  this plan's own open PR when it resumes, set up per `plans/.agents/executing.md`, "Executing one plan" (step 6); never
  another plan's PRs. The bookmark is `plan/discord-link-in-app/01-discord-link`, as the outline names it. If the work
  turns out to want more than one PR, they form one linear stack with bookmarks `plan/discord-link-in-app/<nn>-<name>`,
  logged as a DECISION.
- Avoid churn: within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to a PR an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and the changelog fragment in the same commit.
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). This PR changes tests, so include the full text of `.agents/test-authoring.md` verbatim in
the prompt, as root `AGENTS.md` requires.

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
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question. The
changed docs page goes there too, named by its path, in place of a preview link.

## Done criterion

The plan is complete when a draft PR (or a linear stack of them) exists that meets the acceptance criteria above, each
PR passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
