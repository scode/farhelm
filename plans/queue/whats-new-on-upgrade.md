# Show the release notes after an upgrade

Written against main at 4638a5334 on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, except where this goal explicitly changes them (the spec edits it lists are part of the work).

## The goal

Implement the TODO.md entry under Near term → Trivial, **Show release notes on upgrade.** The maintainer's words: "on
upgrades, show the release notes (only the highlighted ones) to the user. offer a "do not show release notes again"
option to suppresse (direct click button + separate toggle in settings). also link in that dialog to full release notes.
the release notes to contain everything since the last version the user ran, not just the ones for the version they
upgraded to". Their instruction when picking it: "please propose realistic mockups. one key question is: can we include
screenshots reasonably here? given that we intentionally dont ship a web stack today. what are our options. we can also
fall back to just having the text with a "(see screenshot)" link or similar also we should always say something like "N
additional changes not highlighted - see release notes for the full detail"".

Despite its bucket, this is a feature of several PRs: a data file on the website, a record in the helm, a download and a
dialog in the desktop app, a Settings switch, spec edits, and a change to the stable release procedure.

### The dialog the maintainer approved (mockup, in words)

- Title **What's new in Farhelm**. A subtitle naming the releases it shows ("Highlights from 0.42.0 and 0.43.0", or
  "from 0.40.0 to 0.43.0" for a longer range). Not "You last used X": a failed earlier download can make that untrue.
- A scrolling body listing each release shown, newest first, as its version and date, then its highlight categories in
  the changelog's order and names (Breaking, Added, Changed, Fixes worth highlighting, Removed), each entry in full.
  Under an entry that has a release screenshot, a **See screenshot ↗** link opens that entry on the website's Release
  notes page in the browser.
- A footer line, always present: "N additional changes not highlighted. See the release notes for the full detail.",
  with N counted over every release shown.
- Buttons: **Don't show again**, **All release notes ↗** (opens `https://farhelm.io/docs/release-notes/`, as Help →
  Release notes does), and **Got it** (the default).

### Where it stands (from planning, at main 4638a5334)

Line numbers are approximate.

- **Only the installed Mac app updates itself** (`crates/farhelm-desktop/src/updater.rs`, started from
  `crates/farhelm-desktop/src/lib.rs`, about lines 284-286). The Linux desktop app has no install or update path
  (SPEC.md, about lines 717-719), and builds from main report version `0.0.0-unreleased` (root `Cargo.toml`, about line
  84). The running version is `BUILD_VERSION` (`crates/farhelm-proto/src/lib.rs`, about line 365).
- **Nothing records the last version run.** The helm database records only the build that created it
  (`crates/farhelm-helm/src/announcements.rs`, about lines 17-56; the write when a new database is created is in the
  helm's store, about line 3153 of `crates/farhelm-helm/src/store.rs`). The app's own state file `desktop-client.json`
  (`crates/farhelm-desktop/src/state.rs`, about lines 15-31) holds only the automatic-updates switch and a failure
  streak.
- **Shared preferences:** SPEC.md (about lines 2532-2541) says dismissals belong in the helm's shared preferences, not
  in anything a single app keeps for itself; the welcome card's `welcome_seen` flag is the model
  (`crates/farhelm-proto/src/ui_channel.rs`, about lines 691-695, with its existing read and write path).
- **Settings → App** is listed only where the updater runs (`crates/gui/farhelm-gui/src/settings.rs`, about lines 14-16
  and 460-482), and holds the automatic-updates switch.
- **Announcements** are the closest existing mechanism, and stay as they are: a compiled-in table
  (`crates/farhelm-helm/src/release_announcements.rs` and `release_announcements.json`), a helm-side queue, and a plain
  text dialog with **Got it** (`crates/gui/farhelm-gui/src/announcements.rs`), opened at window start after the Welcome
  card (`crates/gui/farhelm-gui/src/welcome.rs`, `StartDialogs`, about lines 412-545). SPEC.md (about line 4767) says
  "Only this producer interprets release versions or builds release-notes links"; this feature does both, so that
  sentence changes. The "first and only producer" sentence (about line 4762) is about announcements and stays.
- **Existing "What's new" item:** with an update waiting, the version readout's menu has **Restart to update** and
  **What's new**, which opens the website's release notes (`crates/gui/farhelm-gui/src/update_readout/view.rs`, about
  lines 57 and 545-595). It stays as it is.
- **Downloads:** the updater has capped download helpers built on `reqwest` (`fetch_capped`, `fetch_optional_capped`,
  `fetch_on_worker`, `fetch_with_client`, `updater.rs` about lines 1623-1671). SPEC.md's list of outbound connections
  (about lines 4818-4840) is exhaustive, so a new connection to farhelm.io is a spec edit.
- **The notes:** `CHANGELOG.md`, in the format `releasing/.agents/changelog.md`, "Format", gives: highlight categories,
  then a folded `<details>` "All other changes" block (v0.42.0 on). The newest section can carry the line
  `**DRAFT release notes - not yet finalized**` while the maintainer reviews it. The website's Release notes page is
  generated from it at every build by `website/scripts/release-notes.mjs` (per-release anchors `#vX.Y.Z`; per-entry
  screenshots from `website/src/release-notes/<version>.json`, keyed by a PR number the entry cites, highlights only,
  shown behind a closed "show screenshot" toggle in `ReleaseScreenshots.astro`). There is no data file and no per-entry
  anchor today. Entries use inline `code` and **bold**, cite PRs as plain "(#4333)", and rarely contain links.
- **Website deploys:** farhelm.io changes only when someone deploys it (`.agents/vercel.md`: git-triggered deploys are
  off). `releasing/.agents/stable.md` (about lines 19-25) justifies tagging with draft notes by "Nothing that ships
  reads the notes"; this feature changes that, through the website.
- **Rendering:** dialogs use `crates/gui/farhelm-gui/src/components/dialog.rs` (the compact variant serves Announcements
  and Welcome). There is no Markdown rendering; small styled-text precedents exist (bold in `welcome.rs`, about line
  309). The GUI's architecture rules (SPEC_impl.md, about lines 1244-1253) limit GPUI Kit to named primitives, so its
  Markdown view is not an option.

### Acceptance criteria

1. **Data file.** The website build publishes a machine-readable release notes file on farhelm.io, generated by
   `website/scripts/release-notes.mjs` from `CHANGELOG.md` (no second source). It holds releases from v0.42.0 on, leaves
   out a release whose section still carries the draft notice, and for each release gives its version, its date, its
   highlight entries grouped by category in order, the count of entries in its "All other changes" list, and, for an
   entry with a release screenshot, the address of that entry on the Release notes page. Entry text arrives as styled
   pieces (plain, code, bold); link markup becomes its text. The file carries a format version, and later changes to it
   only add fields, because every installed app keeps reading it.
2. **Per-entry anchors.** Each highlight with a screenshot gets a stable anchor on the Release notes page, so the
   dialog's link lands on that entry (next to its "show screenshot" toggle).
3. **Record.** The helm's shared preferences gain the last release whose notes were shown (or skipped) and the "show
   what's new" choice, kept together beside `welcome_seen` and read and written through the same path. A newly created
   helm database records the version it starts on, so a fresh install shows nothing. A helm with no record (one created
   by an older build) counts as an upgrade and shows only the running release's highlights.
4. **When it shows.** Only in the installed Mac app (where the updater runs), only on a release build, only while the
   choice is on, and only when the running release is newer than the record: then the app downloads the data file. The
   download does not depend on the automatic-updates switch. If the download fails, or the file does not list the
   running release (a website not deployed yet, a draft), nothing shows, the record stays as it was, and the next launch
   tries again. Otherwise the dialog shows every release in the file newer than the record and not newer than the
   running one. Closing it in any way (Got it, Don't show again, ✕) records the running release. A failure is logged,
   never shown as an error.
5. **Order.** At window start the dialog comes after the Welcome card and Announcements, as a separate dialog. "Don't
   show again" never hides announcements.
6. **Dialog.** As described under "The dialog the maintainer approved", with code and bold styled and links opened in
   the browser the way the app's other links are. The app opens only `https://farhelm.io/` addresses taken from the
   file.
7. **Switch.** Settings → App has a switch for showing what's new after updates, on by default. "Don't show again" turns
   it off; turning it on again shows the dialog at the next upgrade.
8. **Release procedure.** `releasing/.agents/stable.md` gains the maintainer's step: once the maintainer reports that
   the brick test passed, the release agent stops and asks them to deploy the website (the agent never deploys it), then
   checks read-only that farhelm.io's data file lists the new release, and only then hands over the line that moves
   `/latest`. The "Nothing that ships reads the notes" reasoning is reworded to match.
9. **Specs.** SPEC.md: the outbound-connections list gains this download (when it happens and when it does not, what it
   sends), "Installation and updates" and the Announcements / Help paragraphs gain the dialog, the Settings paragraph
   gains the switch and the record, and the "Only this producer interprets release versions or builds release-notes
   links" sentence is reworded. SPEC_impl.md: "Shared preferences and announcements" and "The desktop app's updater"
   gain how it is built.
10. **Tests.** The generator's selection, draft exclusion, styled pieces, counts and anchors; the selection of releases
    to show (including no record, fresh helm, record equal to running, file missing the running release); the record
    staying put on failure and advancing on close; and goldens for the dialog.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning pages (the first with the dialog mockup, a follow-up after a
fresh-context planning review):

- **Text source:** downloaded from farhelm.io, from a data file the website builds from `CHANGELOG.md` (final wording,
  plus each entry's screenshot). Rejected: building the notes into the app, which would show each release's own draft
  wording and no screenshots for it.
- **Screenshots:** a "See screenshot ↗" link under entries that have one, opening that entry on the website. Rejected:
  showing the picture inside the dialog, and no screenshots.
- **Announcements first, then this dialog,** as separate dialogs; "Don't show again" never hides announcements.
- **First launch of a build with this feature:** an upgrade shows this release's highlights; a fresh install shows
  nothing.
- **Website deploys** (follow-up page, the maintainer's own words): "the release agent should not do the release. it
  doesn't have authentication. but it should stop and tell me to do so after i say brick testing works and not suggest i
  move /latest until the agent has confirmed the website has the new release notes".
- **Mac app only, in practice:** stated on the page and not contested, since only the installed Mac app updates itself.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort with the general review charter.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

Planner decisions shown to the maintainer on the follow-up page, from the planning review:

- The data file leaves out draft sections and holds releases from v0.42.0 on (no older section can ever be selected,
  because the first build with this feature shows only its own release, and the v0.26.0 to v0.41.0 "Misc fixes" format
  never needs handling).
- Both records live in the helm's shared preferences beside `welcome_seen`, as the spec requires for dismissals; a new
  helm records the version it starts on.
- The download happens only when the version changed and the dialog is on, whatever the automatic-updates switch says.
- The subtitle names the releases shown rather than "you last used X".
- Entries keep code and bold styling; links show as their text.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and picker instruction quoted above, and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md", and
  its "Instructions by path" for every path you touch: `website/AGENTS.md` before touching `website/`, and
  `releasing/AGENTS.md` before touching `releasing/` (read both in full before their PRs).
- `.agents/validation.md` for the website build check, the GUI and desktop test selections, and the screenshot goldens
  (re-capturing them with `scripts/check-gui-goldens.py --update`, inspecting every changed picture, and publishing them
  with `python3 scripts/gui-goldens.py publish` before any rebase, as it describes).
- `.agents/test-authoring.md` for every test added or changed.
- SPEC.md's outbound-connections list is exhaustive; the dismissal rule (about lines 2532-2541); the GUI architecture
  rules in SPEC_impl.md (about lines 1244-1253).
- The release procedure fences in `releasing/AGENTS.md`: agents never deploy the website, sign, publish or move
  `/latest`; the new step is a stop that waits on the maintainer and sends the urgent notification that file requires.

**Planner proposals:**

- Reuse the updater's capped download helpers rather than adding a second HTTP path; choose a size cap that fits the
  file with room to grow, and a redirect policy that keeps the download on `https://farhelm.io/` (the updater's helpers
  follow redirects because every byte they fetch is checked against a signature, which does not hold for this unsigned
  file).
- Seed the record when a new helm database is created, beside the write of its creation build, rather than exposing the
  creation build to the window.
- Render the styled pieces as styled text in the dialog, with no general Markdown renderer.
- The exact data-file path on farhelm.io, the switch's wording, and the field names are the executor's, recorded in the
  spec.

## Outline

1. `plan/whats-new-on-upgrade/01-release-notes-data`: the website publishes the data file and the per-entry anchors
   (`website/scripts/release-notes.mjs` and whatever serves the file; its tests). Fragment per root `AGENTS.md`.
2. `plan/whats-new-on-upgrade/02-release-deploy-step`: `docs:` the stable release procedure's new stop (acceptance
   criterion 8), in `releasing/.agents/stable.md` and wherever `releasing/AGENTS.md` lists the procedure's stops.
3. `plan/whats-new-on-upgrade/03-whats-new-record`: the helm-side record and choice in the shared preferences, seeded on
   a new helm, with its tests and SPEC_impl.md's shared-preferences text.
4. `plan/whats-new-on-upgrade/04-whats-new-dialog`: the download, the selection, the dialog, its place after
   Announcements, the Settings → App switch, recording on close, the SPEC.md edits, the goldens, a changelog fragment,
   and removal of the TODO.md entry ("Show release notes on upgrade."). If this PR comes out much larger than the
   others, split the download and selection (with their tests) into their own PR below the dialog, rather than leaving
   code unused across PRs.

## Fallbacks

- If the website cannot serve a generated file from the same build without a new deploy mechanism, generate it into the
  site's static output by the same integration hook that writes the Release notes page; log the choice.
- If the updater's helpers cannot be reached from where the dialog decides to download, call them through the desktop
  app crate rather than adding another client; log it.
- If the start-dialog sequence cannot place a second dialog after Announcements without restructuring it, add the
  smallest step to the existing sequence; a restructuring is a scope reassessment.
- If an entry's markup goes beyond code, bold and links (italics, say), render its text plain and log it.
- Decisions to log: the data file's path, shape and size cap; the redirect policy; the switch's wording; any golden
  whose change you judged intended; and every review finding declined, with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-whats-new-on-upgrade-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-whats-new-on-upgrade-watchdog.json` beside it. Resolve both to
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: `plan/whats-new-on-upgrade/01-release-notes-data`,
  `plan/whats-new-on-upgrade/02-release-deploy-step`, `plan/whats-new-on-upgrade/03-whats-new-record`,
  `plan/whats-new-on-upgrade/04-whats-new-dialog` (and, if the fallback in the outline splits PR 4, a download PR below
  it named in the same pattern).
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
meeting the acceptance criteria above, each having passed its review gate, with the last one removing the TODO.md
entries this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied.
Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through
the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
