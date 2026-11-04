# In-app announcements, starting with serious breaking changes

Written against main at 5e155da1 on 2026-11-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **An in-app signal for serious breaking changes.** Release notes alone are easy to miss. Give Farhelm a way to tell
> users inside the app about a serious breaking change in a release they just updated to, rather than relying on the
> release notes. Commands and wrappers starting to run through bash with full shell meaning
> (`plans/queue/command-shell-meaning.md`) is the kind of change that would have used it.

While planning, the maintainer widened this into a general mechanism. In their words: "make the implementation be
required to not hard code that this is about release notes. the _mechanism_ should be a "pop up on start" notification.
we may later have it trigger for other reasons than an upgrade, and it may also potentialy trigger in a running app. the
plan should call for the mechanisms to be minimally tied to (a) it being about new version upgrades, and (b) being tied
to on-app-start." On the content: "(for now, but again plan should minimize assumptions here, the current hard coding
solution should be the current implementation, but not an assumption sprinkled everywhere) also: this shoudl NOT just be
"everything in breaking changes section". This should only be things EXPLICITLY selected to be a pop-up when the
maintainer picks them during the release process. make the release process interactively ask the maintainer for any
in-app announcements to be baked into the release up-front before it gets kicked off."

After the planning page, the maintainer made the model explicit: "I want most of the implementation to be modeled as
there simply being a queue of announcements. It's just that right now, the only delivery mechanism of this queue is that
new releases have built-in announcements in them."

So the goal has three parts: an announcement queue that the helm keeps, with everything around it (the dialog that
drains it, the history of what was shown, a `?` menu item) knowing nothing about where announcements come from; its
first and only producer for now, a release's built-in announcements; and a release procedure step that asks the
maintainer for a version's announcements up front.

NOTE: The plan ships the mechanism with no real announcements. Nothing user-visible appears until a release adds one.

### Acceptance criteria

- **The queue** is the model. The helm keeps a durable queue of announcements, each carrying everything needed to show
  it: an id (stable, unique, never reused), a title, a body, and optional extras the dialog can show (for a release
  announcement: its version and a link to that release's notes). Producers enqueue; enqueueing is idempotent by id, so
  an announcement is enqueued at most once per helm, ever. Clients read the pending queue, show it, and dequeue what the
  user dismissed into the helm's history of shown announcements. Nothing downstream of the queue (the dialog, the
  history, the menu item, the client's check) knows about versions or releases; it renders what the queue holds.
- **When a window checks the queue:** today, once per page load, after sign-in, in the same place the welcome card is
  decided, and after the welcome card closes when it was showing (the only modal that can be up at that point). The
  check is one self-contained entry point ("read the pending queue and show it if non-empty"), not interleaved with
  load-only state, so a later producer that enqueues while the app runs can trigger it from an event without reworking
  it. This plan builds no event-feed reader for announcements: the only producer runs before the helm serves anything,
  so a running window can never miss one today. (The maintainer said a later trigger "may" fire in a running app and
  asked for the mechanism to be minimally tied to app start; keeping the check callable meets that without building the
  live path now. This is the planner's reading; log it.)
- **The release producer**, the only producer for now: a table, built into the helm, of announcements each attached to
  the exact tagged release it first ships in, a stable `X.Y.Z` or a prerelease such as `X.Y.Z-rc.N`. When the helm
  starts, it enqueues every entry that applies and has never been enqueued. An entry attached to version `V` applies
  when the helm was first set up on a version older than `V` and the running build is `V` or newer, compared in full
  release order (semver precedence, prereleases included: `0.36.0-rc.3` is newer than `0.35.0` and older than `0.36.0`).
  In the maintainer's words: "make sure the rc releases aren't somehow excluded here. any "version tagged release" needs
  to be part of the version comparison. so if a user goes 0.35.0 -> 0.36.0 they should see announcements attached to
  0.36.0-rc.3 as well." So a fresh install gets no old announcements: "if somebody installs the app from scratch,
  there's a new helm. We don't start eating the queue from the very bottom. We start eating the queue from the version
  that was initially installed. Otherwise, a new user just gets a bunch of announcements." Entries attached to the very
  version a helm was first set up on are not enqueued either (the maintainer's answer on the page). Builds from main
  (`0.0.0-unreleased`) enqueue none, and a baseline recorded by such a build counts as no baseline. Someone skipping
  several releases gets all the skipped releases' announcements together. Dev releases (`X.Y.Z-dev.N`) take part in the
  same ordering like any tagged release, though the release procedure never attaches announcements to them. Version
  comparison, the release-notes link and the table live in this producer alone. Enqueueing finishes before the helm
  starts serving requests. The table ships empty. Rust tests pass the producer a test table by dependency injection;
  nothing in a shipped build can enqueue a test announcement.
- **The dialog**, in the app's dialog style like the welcome card: titled **Announcements**, a summary line under the
  title (the schematic used "Important changes in the version you just updated to"; word it so it does not assume every
  queued announcement comes from an upgrade, logged as a DECISION), each announcement with its title, its body and its
  extras (a release announcement's version and release-notes link), newest first, and a **Got it** button. **Got it**, ×
  and Escape all dismiss. Focus moves inside; other global chords are inactive while it is open. It never opens on top
  of the welcome card. Bodies are plain text, with blank lines separating paragraphs; no markup.
- **Dismissal** is per helm, like welcome-card dismissal: dismissed once, gone in every browser and the desktop app on
  that helm. Dismissing dequeues every announcement the dialog showed into the history, with when it was shown. The
  write is best-effort: if it fails, those announcements stay queued and may show again on a later load (SPEC.md's
  best-effort list gets this case). A dialog already open in another window when the queue is drained elsewhere may stay
  open, as the welcome card does.
- **The `?` menu item** **Announcements**, directly after **Release notes**, with the description "Shows previously
  shown announcements again" (the maintainer's wording) and hover text along the lines of "Announcements: show the
  announcements Farhelm has shown you before, newest first, such as serious changes you needed to act on". It is hidden
  until this helm has shown at least one announcement and then stays. It opens the same dialog listing every
  announcement this helm has shown, newest first; reopening records nothing.
- **The release procedure** (`releasing/`): when the first release candidate of a new version is requested, the
  maintainer is asked, in the same reply that settles (or announces) that candidate's version and base, which
  announcements to add. That holds even when the request states everything else ("cut an rc with this stack"), which
  today needs no question: the announcement question is then asked in the reply that acknowledges the request, before
  any other work. New announcements are attached to that candidate's exact version, and their text lands on main in a
  small PR before the tag, so every later candidate and the stable release carry them. Each later candidate and the
  stable request show the announcements already attached to this version cycle (`X.Y.Z` and its prereleases) as they
  stand on main, and ask whether to add or change any; "no" is enough. A new one is attached to the release being cut.
  At an RC it lands in that small PR before the tag; at the stable release it rides in the changelog PR, which is
  already an authorized merge and whose merge commit is exactly where the release branch is cut. A stable release with
  no earlier candidate asks the same question in its request reply and uses the changelog PR too. Rewording an entry
  that already shipped keeps its id, and helms that already enqueued it keep the old text; the procedure says so when it
  offers a change. A candidate cut from an unmerged PR stack also carries the announcements (planner proposal: the
  closed-unmerged bump PR carries the announcements commit underneath the unchanged bump commit, so the bump keeps its
  shape and no other flow's PRs are rewritten). Dev releases are not asked, and fix-forward attempts and stable re-cuts
  carry the list over without asking again. Landing the RC announcements PR becomes one of the merges a release request
  authorizes (`releasing/AGENTS.md`, "PR merges these procedures authorize"). The maintainer's announcement wording is
  reproduced exactly, as the changelog rules require for their wording. The announcements PR's commit type and fragment
  handling must pass `python3 releasing/check-changelog.py` (a `chore` needs no fragment), and a helm unit test that
  validates the table (unique ids, valid versions, non-empty text) is a pre-merge check the procedure runs on an edit.
- **SPEC.md** gains a subsection beside "Welcome card" describing announcements, their shown state and the menu item,
  and its "Feedback" passage lists the new `?` menu item; the best-effort list (around 3274-3286) and the shared
  preferences passage (around 1704-1713) cover the per-helm dismissal. **SPEC_impl.md** records the queue's storage
  (schema migration), its routes, and how a producer enqueues.
- **Tests:** helm store and route tests for the queue (idempotent enqueue by id, dequeue into history, history newest
  first); helm unit tests for the release producer's rules (versions, release candidates, fresh-install baseline,
  unreleased builds, skipped versions, an entry attached to a release candidate reaching someone who jumps from the
  previous stable to the next stable) and the table validation; browser tests on Chromium and WebKit modelled on
  `e2e/tests/welcome.spec.ts` for the dialog, dismissal, a failed write, never stacking on the welcome card, and the
  menu item's hidden/shown rule and its list. The browser tests fake the announcement routes with `page.route`, as
  `welcome.spec.ts` does for preferences: every browser spec shares one helm whose history cannot be reset, so a real
  dismissal would leave the menu item visible for every later spec (breaking `feedback.spec.ts`'s item list and possibly
  the 240px menu fit). The Rust tests cover the real queue.
- **Changelog and TODO:** a changelog fragment per root `AGENTS.md`, "Changelog fragments". The plan ships nothing a
  user can see, so it is `kind: none` with that reason unless something visible ships after all. The plan's last PR
  removes the TODO.md entry, including its `Plan:` reference.

## Decisions already made

Settled with the maintainer on 2026-11-03, over three rounds of the planning page:

1. **A general announcement mechanism**, minimally tied to upgrades and to app start (their words above). Upgrades are
   its first and only source for now.
2. **Shape:** a dialog on first open after the update (the only shape that cannot be missed), headed "Announcements",
   with a "Got it" button.
3. **Content:** hand-written per release and built into the app, as the release producer's implementation, not an
   assumption spread through the mechanism. Only announcements the maintainer explicitly picks during the release
   process; never "everything in Breaking". Not chosen: fetching from the website, or nominating entries from changelog
   fragments.
4. **When the release procedure asks:** at the first release candidate of a new version, in the reply that settles its
   version and base; the text lands on main before the tag; later candidates and the stable request show the version's
   current list and ask whether to change it. Not chosen: asking afresh every time with no carry-over, or asking only at
   the stable release. When the maintainer asked what "the current list" means, the answer agreed on the page was: the
   release's own announcements (the entries attached to the version being released), never a computation of what a given
   user will see; the app does that filtering.
5. **Shown state per helm**, like the welcome card (SPEC.md forbids per-client storage for shared preferences).
6. **Release candidates are releases like any other** for announcements: an announcement attaches to the exact tagged
   release it first ships in, release candidates included, and applicability uses full release order (the maintainer's
   words quoted under Acceptance criteria). A fresh install gets nothing attached to its own first version or earlier.
7. **No real announcement ships with the plan**; tests use test-only ones. Not chosen: shipping the 0.33.0 Bash
   announcement retroactively.
8. **The `?` menu item:** "Announcements", directly after Release notes, description "Shows previously shown
   announcements again", listing every announcement this helm has shown, newest first, hidden until one has been shown.
   Built into this plan rather than a separate TODO entry.
9. **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort for every PR (see Review gate). If it cannot
   be reached when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute
   reviewer.
10. **No-workhorse mode**, the plans default, stated on the planning page.
11. **A queue of announcements is the model** (the maintainer's clarification after the page, quoted under The goal).
    Releases' built-in announcements are only the queue's first delivery mechanism; most of the implementation is the
    queue and what drains it.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words quoted under The goal, and the decisions above. The
user-visible strings "Announcements", "Got it" and "Shows previously shown announcements again" are the maintainer's;
keep them exactly.

**Binding repository constraints:**

- SPEC.md: "Welcome card" (around 3341-3355), the shared preferences passage (around 1704-1713), the best-effort list
  (around 3274-3286), "Feedback" (the `?` menu, around 3319-3328), "Installation and updates" (around 361-467), and
  "Upgrade compatibility" (around 4221-4238) for the schema migration.
- `releasing/AGENTS.md` and the files it names for the stable and RC reading lists, read in full before editing anything
  under `releasing/` (root `AGENTS.md`, "Instructions by path"). Every text that the new question contradicts must be
  rewritten in the same PR: `releasing/AGENTS.md`, "Rules for every release" (it says base and version are what a
  request settles, and that nothing waits for the maintainer after that, which is why the question joins the settling
  reply) and "PR merges these procedures authorize"; `releasing/.agents/rc.md` (its "announce the exact version and
  proceed without asking", and the stack request that asks nothing today); `releasing/.agents/stable.md` (its 2026-10-16
  "Announce it and carry on" for a version settled by an RC); `releasing/.agents/dev.md` (a dev release follows `rc.md`,
  so exempt it explicitly); and `releasing/.agents/fixing-forward.md` (fix-forward attempts and re-cuts carry the list
  over). A change to an instruction file runs `python3 scripts/check-agents-md.py`.
- Root `AGENTS.md`: "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation",
  "Talking to the user", and "Frozen material" (published release notes are untouched).
- `.agents/test-authoring.md` for the tests, `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`. No test may
  modify the test process's environment variables; test announcements go in through dependency injection in Rust and
  route fakes in the browser.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Three PRs: `plan/in-app-announcements/01-announcement-queue` (the helm's queue and history, schema migration, routes,
  the release producer with its empty table, the table validation, store and producer tests, SPEC_impl.md),
  `plan/in-app-announcements/02-announcements-dialog` (the client side: the check, the dialog, the menu item, browser
  tests, SPEC.md, the fragment), and `plan/in-app-announcements/03-release-procedure` (the `releasing/` changes and the
  TODO.md removal). Merge 01 into 02 if the split leaves 01 unreviewable on its own.
- Storage: one table holding each announcement's id, content (title, body, and two named optional fields for the release
  version and the release-notes link, rather than a generic "extras" bag), an insertion sequence, and when it was shown
  (null while pending). Pending is "not yet shown", in insertion order; history is "shown", newest first by shown time
  and then insertion sequence (a batch of skipped releases is enqueued at one moment, so a timestamp alone cannot order
  it). The queue stores the content itself, so clients and later producers never need the release table to render or
  list an announcement.
- Two routes are enough: one read returning pending and history, and one write marking a list of ids shown.
- One narrow enqueue function in the helm that every producer calls; the release producer is one caller, run at helm
  start before serving. A later producer calls the same function.
- Ids of release announcements are minted as `<version>-<short-slug>` (for example `0.36.0-rc.1-bash-commands`), which
  keeps them unique and never reused as long as tags are never reused.
- The fresh-install baseline: record the build a helm database was created on, at creation. A database upgraded from
  before this plan has no baseline; treat it as older than every entry, which is harmless while the table is empty and
  correct for real upgrades later.
- The release table as one small data file in the helm crate, compiled in, which the release procedure edits; each entry
  holds the version, id, title and body. The release-notes link is derived from the entry's version: for a stable
  version, `https://farhelm.io/docs/release-notes/#vX.Y.Z` (the anchor format the release notes page uses); a prerelease
  has no curated notes, so link to the `X.Y.Z` section it leads to, or show no link; your call, logged.

## What the code looks like at 5e155da1

Line numbers are approximate; verify before relying on them.

**Nothing notices an update today.** Nothing in the helm or UI records a last-seen version. The `?` menu's Release notes
item opens the general release notes page (`crates/farhelm-ui/src/app_bar.rs`: `BarMenuItem::ReleaseNotes`, label around
108, description around 159, `RELEASE_NOTES_URL` around 206-212, handled around 965). The update menu's **What's new**
(around 201, 993) exists only in the Mac app while an installed update waits for a restart.

**The `?` menu** is in `app_bar.rs`: `enum BarMenuItem` (around 69-85) with `glyph`, `label`, `tooltip`, `description`
arms (around 90-166), `BAR_MENU_ITEMS` (a fixed-size array whose length sizes `BarMenuOrder`, around 169-179), and
`help_menu_items(has_updater)` (around 185-198), which is where a conditionally shown item goes. The help menu's
`on_choose` (around 953-978) and the update menu's (around 987-1002, which lists help items in a no-op arm) both need
the new variant. The unit test that pins the help menu's order is around 1180-1200. The pending plan
`plans/queue/discord-link-in-app.md` adds "Chat on Discord" after Send feedback in the same lists; whichever lands
second fits around the other.

**The welcome card** is the model for the dialog and its preference: `crates/farhelm-ui/src/welcome.rs`, shown from
`PreferencesGate` in `crates/farhelm-ui/src/lib.rs` (around 2137-2197; around 2189 it shows the card when the
preferences read succeeded and `welcome_seen != Some(true)`), owned by `app_bar.rs` (around 702-738, which writes
`WelcomeSeen(true)` on first-open dismissal; reopening from the menu, around 967-970, writes nothing). The dialog
building blocks are `ModalDialog`, `DialogHeader`, `DialogFooter` and `DialogHandle` in
`crates/farhelm-ui/src/modal_dialog.rs`.

**Shared preferences** are one row in `helm.db`, read and written through `GET`/`PUT /api/preferences` (SPEC_impl.md
around 238-249; the welcome flag arrived in schema 51, around 213-223). The current schema is 68
(`crates/farhelm-helm/src/store.rs` around 337). `welcome_seen` appears in `store.rs` around 1074, 1204, 2404,
3618-3626, 5124-5139, 5206-5323, in `crates/farhelm-ui/src/api.rs` around 404, 811, 870, 905, 929, 1044, 1202, 1357, and
in the route tests in `crates/farhelm-helm/src/preferences.rs` around 502-545. A set of ids may fit better as its own
table than as a column of the preferences row; that is your call.

**Versions:** the UI's own build is `skew::CLIENT_BUILD` (`crates/farhelm-ui/src/skew.rs` around 49, from
`CARGO_PKG_VERSION`); `displayed_version` (`app_bar.rs` around 39-44) shows the helm's reported build when it differs.
Main's version is always `0.0.0-unreleased` (`releasing/AGENTS.md`); a release sets the real version only in a release
commit under its tag. The release producer runs in the helm and keys on the helm's own build, which every user gets; the
UI's build plays no part, so version skew does not affect what is enqueued.

**The release pipeline:** fragments in `releasing/changelog.d/` carry a `kind:` (root `AGENTS.md`, "Changelog
fragments"); the stable procedure curates `CHANGELOG.md` in a changelog PR that lands on main before the release branch
is cut (`releasing/.agents/stable.md`). An RC carries no curated notes and is cut from main or from an unmerged stack's
tip, through a version-bump PR that is closed without merging (`releasing/.agents/rc.md`). Both procedures settle base
and version when the release is requested, by defaults or by asking right then, and nothing waits for the maintainer
after that. The Breaking category is too broad to drive announcements: the fixed remote-hosts entry appears there
whenever the protocol changes.

**The example that motivated it:** `plans/queue/command-shell-meaning.md` landed on 2026-10-31 (#2847, #2848) and
shipped in v0.33.0 as several Breaking entries: commands, resume commands and wrappers run through Bash; wrappers saved
earlier lost their quotes; a placeholder must stand alone; a container with a wrapper needs Bash. It is not shipped as
an announcement (decision 7).

**Tests to build on:** `e2e/tests/welcome.spec.ts` (dismissal shared across clients and surviving reload around 50, a
failed or timed-out preference read around 85, waiting for sign-in around 112, a refused write around 139, modal
isolation around 161, reopening without rewriting around 225); `e2e/tests/feedback.spec.ts` for the `?` menu's items and
keyboard walk; `e2e/tests/tooltip-coverage.spec.ts` for hover text; `e2e/tests/filters.spec.ts` "rail Help menu points
at its footer button" (the open menu must fit a 240px-tall viewport). `e2e/docs-shots/release-notes.spec.ts` and the
release shots are frozen; leave them.

## Outline

1. `plan/in-app-announcements/01-announcement-queue`: the schema migration (the queue and history, and the database's
   creation build), the enqueue function, the two routes, the release producer at helm start with its empty table and
   its validation test, store, route and producer tests, SPEC_impl.md.
2. `plan/in-app-announcements/02-announcements-dialog`: the client's check at load (and after the welcome card); the
   dialog; the `?` menu item over the history; browser tests on route fakes; SPEC.md's new subsection, "Feedback", the
   best-effort list and shared preferences; the fragment.
3. `plan/in-app-announcements/03-release-procedure`: every `releasing/` file listed under Binding repository constraints
   (the question at the first candidate even for a fully specified request, the confirmation at later candidates and the
   stable request, the stable path through the changelog PR, stack-based candidates, dev and fix-forward exemptions, the
   new authorized merge), and how an agent edits and validates the table; `python3 scripts/check-agents-md.py`; the
   TODO.md entry removed.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect:

- `dprint check` on the changed files, `cargo fmt --check`, and clippy on the changed crates;
- `cargo check -p farhelm-ui --features desktop`;
- the `farhelm-helm` store and preference tests and the `farhelm-ui` unit tests, through
  `python3 scripts/record-test-run.py`;
- `python3 releasing/check-changelog.py format`;
- `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares;
- `python3 scripts/check-agents-md.py` for PR 3;
- through the recorder, on Chromium and WebKit, after the web build `.agents/validation.md` describes: the new
  announcements spec, `welcome.spec.ts`, `feedback.spec.ts`, `tooltip-coverage.spec.ts` and `filters.spec.ts`.

The release procedure change is documentation for agents; it needs no runtime tests (root `AGENTS.md`).

## Fallbacks

Examples of departures that need the scope reassessment:

- fetching announcements from the network, or any producer other than the compiled-in release table;
- announcements in the CLI or for agents;
- per-device shown state;
- tying announcements to changelog fragments or the Breaking category;
- any change to the signing, publication or tagging steps of a release.

Decisions to log: the enqueue function's shape, the storage shape, how the fresh-install baseline is recorded, how tests
inject announcements, the dialog's summary wording, where the client's check lives and how a later trigger would call
it, the prerelease link rule, the release procedure's exact wording, every test changed and why, and every review
finding you decided not to follow.

Also out of scope for this plan: an event-feed reader that shows announcements enqueued while a window is open. Building
it before a producer needs it is a departure for the scope reassessment.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-in-app-announcements-log.md` in the parent directory of the checkout you run in, derived as that section
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
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

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

- A linear stack of reviewable commits, one stable bookmark and draft PR each, using the `jjstack` skill. The stack's
  base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/in-app-announcements/<nn>-<name>`, as the
  outline names them.
- Avoid churn: code added in one PR and deleted in a later PR of the same stack means the stack should have been shaped
  differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and the changelog fragment in the same commit as the change.
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
already made and outline). For a PR that changes tests, include the full text of `.agents/test-authoring.md` verbatim in
the prompt, as root `AGENTS.md` requires. Ask the reviewers of PRs 1 and 2 specifically whether anything outside the
release producer assumes announcements come from releases or only appear at start.

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

The plan is complete when a linear stack of draft PRs exists that meets the acceptance criteria above, each PR passed
its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
