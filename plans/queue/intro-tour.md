# In-app intro: a welcome card on first open

Written against main at 4c9e9612 on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan. The plan `release-notes-menu-item.md` also adds an item to the `?` menu;
whichever lands second rebases onto the other (see the menu order below).

## The goal

Farhelm has no first-run experience: a new user lands on the session list, and with no sessions the main area just says
"no sessions — create one". The first time Farhelm opens on a helm, show a short **welcome card** that introduces the
app in one line, points at the docs website's Get started guide, and points at sending feedback. Dismissing it once,
from any browser or the desktop app, hides it everywhere. A **welcome** item in the `?` menu shows it again.

The maintainer approved this mockup as is: https://snippets.scode.org/s/farhelm-welcome-card/ (the card, its wording,
and the `?` menu item). Treat its wording as the starting point; you may polish it, but keep every point it makes.

### Acceptance criteria

- **The card.** A modal dialog in the app's dialog style (`role="dialog"`, `aria-modal="true"`, focus moved into it and,
  when the `?` menu opened it, returned to the menu's toggle as the app's other dialogs do; when it opened on load there
  is nothing to return to, so focus goes wherever the app's initial focus goes), titled "welcome to farhelm", with:
  - the one-line introduction from the mockup (the README's: one window for every coding agent you run, on your Mac and
    on any Linux box you can ssh to; each agent keeps running on its machine when you close the window, and you pick it
    up where it was);
  - a Get started item with a link that opens `https://farhelm.io/docs/get-started/first-session/` (the guide's first
    page after installation, since anyone seeing the card has already installed) through `open_external_page`, like the
    `?` menu's documentation item: the system browser from the desktop app, a new tab from the web UI;
  - a feedback item saying that Send feedback, in the `?` menu at the top of the sidebar, sends a private message to
    Farhelm's maintainer, with a "send feedback now" link that closes the card and opens the feedback dialog;
  - a footer saying the card is in the `?` menu as welcome, and a "got it" button.
- **Dismissal.** Escape, the ×, "got it" and "send feedback now" all close the card and record it as seen. Following the
  Get started link does not close it.
- **When it shows.** Once per helm (maintainer decision): decided once per page load, when sign-in has finished and the
  shared preferences were read successfully and the helm's "welcome seen" preference is not set. A failed or timed-out
  preferences read (which today falls back to defaults) does not show the card. Every dialog sits behind the preferences
  loading gate and nothing opens one on load, so no other dialog can be open at that moment; a cheap guard is fine, but
  no waiting machinery and no test for it. "Hides it everywhere" means on each client's next load: a card already open
  in another window is not closed live. While the card is open, the other global chords stay off, as for any modal.
- **Everyone sees it once (maintainer decision).** Every helm, new or existing, starts unseen, so people already using
  Farhelm see the card once after updating to the release that adds it. Do not mark existing helms as seen in the
  migration.
- **Storage.** "Seen" is one new boolean field on the helm's shared preferences row (`Preferences`/`PreferencePatch` in
  `crates/farhelm-helm/src/store.rs`, `/api/preferences`, the UI's `PreferenceField`/`SharedPreferences`), defaulting to
  unseen. A failed write is not an error the user has to deal with: the card closes, and it may show again on a later
  load. The store's schema-rewind test fixtures in `store.rs` that rebuild older schemas need the new column, as they
  did for `quick_launch_template`.
- **Downgrade (maintainer accepted, 2026-10-14).** The new preference column bumps the helm's schema version, after
  which an older helm refuses to open its database, as with earlier preference columns. Say so in the changelog
  fragment.
- **Reopening.** The `?` menu (`crates/farhelm-ui/src/app_bar.rs`, `help_menu_items`) gains a **welcome** item with the
  description line "shows the welcome card again" and hover text in the style of the other bar menu items. It opens the
  card; neither opening nor closing a reopened card needs to write anything (the card is already seen). Menu order: send
  feedback, documentation, release notes (if that plan's item is on main), welcome, then check for updates where it is
  offered. The displayed order comes from the fixed `BAR_MENU_ITEMS` array, not from the list `help_menu_items` returns,
  so insert the item there. The `?` button's own hover text lists the menu's items; include welcome. The mockup's "new"
  tag on the item only highlighted it on the mockup page: the item has no badge.
- **Existing harnesses.** A fresh helm now opens with a modal over the app, which would cover the UI for every harness
  that drives it. Mark the card seen through the helm's preferences API, never by special-casing the card in product
  code:
  - `e2e/global-setup.ts` is shared by all four Playwright configs (the main suite, docs screenshots, README hero and
    README video) and already holds the helm credential, so one preferences write there covers them all;
  - the `resetPreferences` test helper (used before tests in several spec files; its comment says every preference
    belongs in the reset) must reset the flag to seen, not clear it, or the card returns over most of the suite;
  - the desktop smoke's CI legs do not interact with the page, but its opt-in click-driven leg makes a preferences write
    before restarting: add the flag there;
  - the Rust e2e tests do not drive the UI and only need their struct literals updated.

  The card's own tests run against the one shared helm, so "a fresh helm" there means clearing the flag, and they must
  restore it to seen even when they fail. Captured images must not change because of the card. If the release brick
  test's procedure (`releasing/mac-vm-test/BRICK-TEST.md`) walks a fresh install through the UI, add a line there that
  the card appears and how to dismiss it.
- SPEC.md: "Feedback" (which defines the `?` button and its items) gains the welcome item; describe the card and when it
  shows where first-run behavior belongs (for example near the start of the session list or in Feedback, your call; log
  it); wherever shared preferences are listed includes the seen flag. SPEC_impl.md: where the flag is stored and why it
  is shared rather than per device.
- Docs website: Get started's first-session page mentions the welcome card briefly, and wherever the `?` menu's items
  are described mentions the welcome item (`website/AGENTS.md`, `website/EDITORIAL_RULES.md`). Whether the docs need a
  screenshot of the card is the docs refresh's call, not this plan's; do not add one.
- Tests: the card appears when the flag is clear and not after dismissal, across a reload and in a second browser
  context (it is shared); each dismissal path records it; a failed preferences read does not show it; the Get started
  link opens the stated URL without closing the card; "send feedback now" opens the feedback dialog; the menu item
  reopens it; the store's default and migration leave existing helms unseen.
- A changelog fragment (`kind: added`) per `feat` PR, as root `AGENTS.md` requires.
- The TODO.md entry ("In-app intro tour.") is removed.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

Out of scope: a step-by-step coach-mark tour (the maintainer chose the card for this first version; the TODO entry
allows it to grow later), per-device or per-user seen state, changing the empty states, and a screenshot of the card in
the docs.

## Requirement sources

**The user's request (2026-10-14):** plan this Near term TODO entry. Its text:

> **In-app intro tour.** Introduce new users to the app from inside it. At minimum, point out how to send feedback and
> encourage reading the docs website's Get started section, with a link to it; it may grow into a fuller guided tour.
> When it appears, how it is dismissed, and how much it covers are to be decided when the entry is planned.

**The user's decisions (2026-10-14):**

- Form: a welcome card, chosen over step-by-step coach marks and over a card with an optional tour.
- Seen once per helm, as a shared preference, chosen over once per device and over showing it while there are no
  sessions.
- Existing users see it once too ("Everyone once"), chosen over marking helms that already have sessions or hosts as
  seen.
- A "welcome" item in the `?` menu brings it back, chosen over no way back.
- The mockup above, approved as is.
- The schema bump with no downgrade path, accepted.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md "Feedback" (the `?` menu), Session list (shared preferences), and the
SPEC.md statement that per-client persistence "is not wanted" for shared preferences; SPEC_impl.md's bar menu
description; root `AGENTS.md` Docs website, Docs screenshots, README hero screenshot, README demo video, Conventional
Commits, Finishing work, Releases and the changelog, Talking to the user; global instructions (tests never modify the
test process's own environment variables); `.agents/test-authoring.md` for every test change; `plans/AGENTS.md`,
Executing one plan.

**Planner proposals (some from the planning review):** the Get started link target (the first-session page), which
dismissal paths count as seen, deciding once per load and only after a successful preferences read, the failed-write
behavior, no badge on the menu item, marking the card seen in harness fixtures through the preferences API, and the test
list.

## Outline

Line numbers drift; find the code by name. Here's what exists today:

- The `?` menu: `BarMenuItem`, `help_menu_items`, `DOCS_URL`, `RELEASE_NOTES_URL` and `open_external_page` in
  `crates/farhelm-ui/src/app_bar.rs`; its send feedback item opens `feedback::FeedbackDialog`.
- Shared preferences: the `preferences` row in the helm store, served by `/api/preferences` (size check in
  `crates/farhelm-helm/src/preferences.rs`); the UI reads it once after sign-in (the gate in
  `crates/farhelm-ui/src/lib.rs`) and writes fields through `api::store_preference(PreferenceValue::…)`. The quick
  launch column (#1994) is the most recent example of adding one, migration included.
- Dialogs: the feedback dialog and the templates dialog show the modal pattern (focus, Escape, the × and the
  `aria-modal` that keeps global chords off).
- No first-run UI exists; empty states are plain text (`main-empty` in `lib.rs`).

Here's the work, as a proposal:

1. **The flag**: the preference field, migration, API and UI plumbing, tests.
2. **The card and the menu item**: the dialog, when it shows, dismissal, the links, the welcome item, harness fixtures,
   tests.
3. SPEC.md, SPEC_impl.md, docs website, changelog, TODO removal (folded into the PRs they describe where that reads
   better).

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- through the recorder, the farhelm-helm (store, preferences) and farhelm-ui tests touched;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, since tests change;
- Playwright on Chromium and WebKit for the new card spec, the help menu and tooltip coverage specs, and enough of the
  wider suite to show the fixtures keep the card out of the way (a modal over a fresh helm affects every spec);
- `cargo check -p farhelm-ui --features desktop`; the desktop smoke if its legs touch the page;
- the website build if docs change; `dprint check` on changed Markdown; `python3 releasing/check-changelog.py format`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-intro-tour-log.md` in the
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
a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/intro-tour/<nn>-<short-name>` (for example `plan/intro-tour/01-welcome-seen-flag`).
- Expected shape: the flag, then the card and menu item with the fixture changes, each leaving main correct on its own
  (a flag nothing reads yet is fine if its PR says so). The fixtures must change in the same PR that makes the card
  appear, so no commit leaves the suites covered by a modal. Avoid churn: code added in one PR and deleted in a later
  one means the stack should have been shaped differently. Within this run, a PR that needs correcting is restructured
  rather than corrected on top, and that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits; this is a new feature, so `feat`. Each such PR adds its
  changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the mockup's link, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check specifically: that the card cannot trap the user (Escape and the × always work) and does not
show after a failed preferences read; that seen is shared across clients and survives a reload; that no harness or
capture is left with the card covering the UI; and that the card is not special-cased in product code for tests.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example coach marks or a multi-step tour,
  per-device seen state, a test-only switch in product code, or a new dialog framework (examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular: where SPEC.md describes the card, how each harness marks it seen, wording
changes from the mockup, the stack's shape, and every review finding you decided not to follow. The user will ask for
these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

Agreed fallbacks:

- Wording in the mockup that turns out inaccurate is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: a guided tour, per-device seen state, hiding
the card from existing users, dropping the menu item.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
