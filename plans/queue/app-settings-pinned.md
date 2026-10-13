# Pin the app settings dialog's header and footer, and add an × close

Written against main at ca36300b on 2026-10-12. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `feedback-dialog-polish.md`. That plan restyles the send feedback dialog, which shares classes with
this one, and may generalize the host dialogs' header for a dialog that is not about a host; build on what it landed
rather than adding a second way to do the same thing.

## The goal

The TODO.md entry (Near term), "Pin the app settings dialog's header and footer, and add an × close": the settings
dialog opened from the gear at the top of the sidebar has grown (host choices, checkout folder, post-clone command, and
more) until it runs past the bottom of the window, and its close button only appears once you scroll down to it. Give it
a header and footer that stay in the same place and are always visible, with only the settings between them scrolling,
and an × close button at the top like the host settings dialog has.

The target design is the maintainer-approved mockup at https://snippets.scode.org/s/farhelm-app-settings-pinned/, built
"as drawn, with sections". Read it first. Exact spacing is yours to settle; the listed changes are requirements.

Acceptance criteria:

- **Header**, in the host dialogs' pattern: a tile with the app's existing gear icon (the one on the sidebar's gear
  button; no new glyph is needed), the title "settings", one summary line ("for every host on this helm, and for this
  device", or close to it), and a × that does exactly what close does, with a tooltip. The × uses the host header's own
  dismiss class and an accessible name distinct from the footer button's, such as "close settings": about ten test
  lookups find the footer button by its exact name "close", and one test and the Escape fallback by
  `.app-settings-close`, so the × must take neither.
- **Pinned footer** laid out like the host settings footer: a quiet note on the left ("switches save as you flip them",
  or close to it) and close on the right, always visible.
- **Only the body scrolls**, between the header and footer, when the window is too short for the dialog; on a tall
  window nothing scrolls.
- **Sections**, as drawn: the settings are grouped under small section labels in the host settings dialog's style
  ("CONNECTION", "PERMISSIONS"): hosts (set up and remove without asking), managed checkouts (checkout folder,
  post-clone command), sounds, and, only in the desktop app where the updater exists, updates (install updates
  automatically). Each scope note sits under its section's label: "These apply to every host on this helm. Other open
  clients pick them up when they reload." under hosts, the sounds note under sounds, "This one belongs to this Farhelm
  app on this Mac." under updates. Today's big "sounds" heading becomes one of these labels, so the dialog has one
  heading style; keep it inside `SoundSettings` (an alignment test looks for `section[aria-label='sounds'] h3`).
- **Wording**: keep every existing string except where the mockup deliberately changes it: the hosts scope note moves
  under its label unchanged, and the sounds note keeps its current text ("These choices apply only to this device…");
  the mockup's paraphrases of help lines are not changes. The new strings are the header summary, the footer note and
  the section labels.
- **The updates section cannot be seen on the executing machine**: the updater only starts in an installed release app
  under `~/Applications/Farhelm.app`, and the live install is off-limits. Check that section by reading the diff, keep a
  web-build test that it is absent, add no test hook to force it, and ask the maintainer in the report to look at it on
  a Mac.
- **Behavior unchanged**: every setting, its saving, its help text that changes with the switch, the automatic-updates
  error path, the checkout folder and post-clone save buttons, Escape, initial focus, and focus return all stay as they
  are. Keep the selectors tests and docs shots rely on (`.app-settings-dialog[aria-label="settings"]`,
  `.app-settings-choice`, `.app-settings-close`, `.checkout-root-field`, `.checkout-post-clone-field`,
  `section[aria-label=sounds]`, `[data-sound-event]`), or update every test that uses one.
- **Nothing else restyles**: the feedback dialog and host settings share classes with this dialog
  (`app-settings-dialog`, `host-settings-title`, `host-settings-actions`, `.app-settings-choice`); scope new rules to
  this dialog (the existing `[aria-label="settings"]` scoping is the precedent) or opt in to framed-dialog classes, and
  change no shared rule's meaning.
- Tests as listed under Outline, on Chromium and WebKit.
- One draft PR, passing its review gate, with its changelog fragment, removing the TODO.md entry. No PR is marked ready,
  and nothing is merged.

## Requirement sources

**The user's request (2026-10-12):** plan this TODO entry, with an SVG mockup first. The maintainer reviewed the mockup
and chose "as drawn, with sections" (offered and not chosen: as drawn without the section labels; changes first). The
section labels were the planner's suggestion, marked optional in the mockup, and are now a requirement.

**The user's decisions (2026-10-12):**

- The mockup above is the target design, sections included.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md says nothing about this dialog's layout; check it and SPEC_impl.md for any sentence the restructure makes
  untrue.
- The host dialogs' header and footer pattern on main is the one to follow; `feedback-dialog-polish.md` may have
  extended it.
- Published release-notes docs shots are frozen (`website/AGENTS.md`, "Release notes"); a restyle does not recapture
  them. The ordinary `manage-hosts` "app settings" docs shot is refreshed by the next docs screenshot refresh, not here.
- Root `AGENTS.md`: Conventional Commits; Finishing work (browser tests through the recorder on Chromium and WebKit,
  `scripts/check-test-sleeps.py` for test delays); Releases and the changelog; Agent scratch space; Sharing the machine
  with other agents.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanism and test shapes in the outline below, checked against main and corrected by a
fresh-context planning review.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- `crates/farhelm-ui/src/settings.rs`: `div.host-settings-backdrop` > `div.app-settings-dialog` (`role=dialog`,
  `aria-label="settings"`), with `h2.host-settings-title`, a scope note, the two host switches with their help lines,
  `crate::checkout_root::CheckoutRootField`, `crate::checkout_post_clone::CheckoutPostCloneField`,
  `crate::sounds::SoundSettings` (its own `section[aria-label=sounds]` with a heading and note), the desktop-only
  automatic updates block, and `div.host-settings-actions` with `button.app-settings-close`. Focus is set up by
  `install_dialog_with_selector`; Escape closes.
- `crates/farhelm-ui/assets/app.css`: `.app-settings-dialog` (its `max-height` already uses the shared dialog margin)
  and the `[aria-label="settings"]`-scoped rules; the host dialogs' framed pattern (`.host-dialog-framed`,
  `.host-settings-header`, the scrolling body, the host settings footer) and its section labels
  (`crates/farhelm-ui/src/hosts/settings_dialog.rs`).
- Tests that open this dialog: `e2e/tests/settings.spec.ts`, `sounds.spec.ts`, `provisioning.spec.ts`,
  `tooltip-coverage.spec.ts` (`tooltip.spec.ts` only hovers the gear); docs shots in
  `e2e/docs-shots/manage-hosts.spec.ts` and the frozen `release-notes.spec.ts`.

Here's the work:

1. Restructure `SettingsDialog` into header, scrolling body and pinned footer, reusing the framed-dialog pattern (and
   whatever `feedback-dialog-polish.md` landed for a non-host header).
2. Group the body into the four sections with the host settings section and section-title styles. `SoundSettings`
   renders its own heading and note today; restyle its heading as the section label in place, without changing its
   controls or the `section[aria-label=sounds]` wrapper.

   The smallest design uses what the host dialogs already have: `host-dialog-framed`, the host settings body, sections
   and section titles, and the host settings footer give the pinned layout, the scrolling body and the sections (the
   cascade order works). The one new capability is a caller-supplied tile on the shared host dialog header: reuse it if
   `feedback-dialog-polish.md` added one, otherwise add it as an optional prop the host dialogs do not pass. Do not
   hand-render a second copy of the header. The only new CSS should be the gear icon's sizing in the tile.
3. Tests, mostly in `e2e/tests/settings.spec.ts`:
   - the × closes like close, has a tooltip and its own accessible name, returns focus to the gear, and leaves the page
     usable;
   - in a short viewport, after first checking that the body really overflows, the header and footer stay fully visible,
     the body scrolls, and close and the × stay reachable without scrolling; in a generously tall viewport the body does
     not scroll; one narrow width too;
   - the sections appear in order with their labels (read `textContent`, since the labels are uppercased by CSS), and
     the web build has no updates section;
   - existing tests keep passing, including `provisioning.spec.ts`, which opens the dialog and clicks close by name.
4. Bookkeeping: changelog fragment, `kind: changed`, saying close and × are always visible and the settings are grouped.
   Remove the TODO.md entry.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`;
- the changed Playwright specs (`settings`, `sounds`, `provisioning`, `tooltip-coverage`) on Chromium and WebKit through
  the recorder, after the builds the browser suite needs;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` if any test delay is added;
- `dprint check` on changed Markdown, and `python3 releasing/check-changelog.py format`.

Look at the dialog yourself in a browser at a normal and a short window height, at the top and scrolled, and compare it
with the mockup before the review gate. Do not run the full battery without a specific risk that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-app-settings-pinned-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/app-settings-pinned/<nn>-<short-name>` (for example `plan/app-settings-pinned/01-pinned-settings`).
- One PR. Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped
  differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (`feat`). Such a
  PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check:

- that the result matches the mockup's listed changes, sections included;
- that no shared class changed meaning for the feedback dialog, host settings or any other dialog;
- that every setting, its saving and its help text behave exactly as before, in the web build and the desktop app;
- that the × has a tooltip and an accessible name and closes exactly as close does;
- that the short-window assertions test what they claim on both engines.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a shared dialog component for every
  dialog, reordering or renaming settings, or changing how a setting saves (examples, not a blacklist);
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
alternatives considered. In particular:

- how the header and footer reuse the host dialogs' pattern (and `feedback-dialog-polish.md`'s work);
- how the sound settings' heading becomes a section label;
- the summary line and footer note wording;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: dropping the sections, restyling the feedback
or host dialogs, adding or removing settings, or changing how any setting saves.

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
