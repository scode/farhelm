# Auto-update: the Mac app checks, installs in the background, and offers Restart to update

Written against main at a925a3c7 on 2026-10-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and this plan changes them where its decisions require.

The hover-help work landed just before this plan was written (#1589, #1590): every clickable control has hover text
through a tooltip of Farhelm's own, and a browser test fails on a control without one. Every control this plan adds or
changes uses that mechanism; read what it landed before starting.

## The goal

The macOS desktop app keeps itself up to date the way Chrome does. It checks for a new stable release at startup and
about once a day. When one exists it installs it in the background by running the same installer a user runs by hand,
and then shows that an update is ready. One click restarts Farhelm into the new version; sessions keep running across
the restart, as they already do across any quit. A single setting turns the automatic part off, and it is on by default.

What exists today: since the side-by-side layout landed (SPEC_impl.md "Side-by-side versions inside Farhelm.app"),
running `scripts/install.sh` while Farhelm is open is safe. The installer adds a version folder inside
`~/Applications/Farhelm.app`, switches the app's main program, writes the `Contents/Versions/installed` record, and
quitting and reopening finishes the update. Nothing in the app checks for releases, shows an update, or relaunches
itself. SPEC.md also excludes automatic updates and notifications. This plan builds the missing parts on the existing
installer, not a second installer.

Acceptance criteria, as a user sees them:

- **Automatic checks.** An installed Mac app (running from `~/Applications/Farhelm.app` with its versions folder, and
  not a build from main) checks for the latest stable release shortly after startup and again whenever 24 hours of wall
  clock time have passed since the last check. A Mac that sleeps overnight still checks about daily.
- **Background install.** When the latest stable release is newer than the version that is installed, the app runs the
  installer in the background, pinned to that release. Nothing on screen changes while it works.
- **The update marker.** The version readout in the top bar normally shows the running version in grey. Whenever the
  installed version is newer than the running one, it shows a red up-arrow before the version and the whole readout
  turns red. The arrow takes about one character of width. This happens however the newer version got installed: by the
  app, or by the user running the installer in a terminal (picked up within about a minute), and also with automatic
  updates turned off.
- **Hover and click.** Hovering the red readout says that Farhelm X is installed and that a restart finishes the update.
  Clicking it opens a small two-item menu:
  - **Restart to update** quits Farhelm and opens it again on the new version. Sessions keep running.
  - **What's new** opens the project's GitHub release list in the system browser.

  Without an update, the readout behaves exactly as it does today.
- **Checking on demand.** In the desktop app, the local machine's host row offers Update unconditionally. It is no
  longer greyed out with "run the installer again". Choosing it checks for an update now, and installs it if there is
  one. The `?` menu gains a third item, **Check for updates**, which does the same. A check started this way ends in a
  visible state: up to date, installing, update ready (the marker), or check failed. It shows in one place for both
  triggers, the readout's hover text, not in a new UI surface.
- **The setting.** The settings dialog of the desktop app gains a checkbox, on by default, for installing updates
  automatically. Off stops the startup and daily checks and the background installs. The two on-demand triggers still
  work, and the marker still appears for a version installed by hand.
- **Failures.** A failed background check or install is logged and retried at the next check. Only a check the user
  started shows failure.
- **Out of scope:** the Linux helm and the browser UI (no installer exists there; the local row's Update stays as it
  is), builds from main, and remote hosts. After the restart, the existing `old version` / `needs update` host labels
  and per-host Update handle remote hosts.
- **Specs and docs.** SPEC.md, SPEC_impl.md and the user-facing docs describe all of this. Nothing in them says
  automatic updates are out of scope or that Farhelm does not auto-update.

## Decisions already made

The maintainer's words are quoted where they decide something; "planner" marks a choice the planning session proposed
and the maintainer accepted or did not object to.

- A1. The original request: "add actual in-GUI checking and notification and 'restart to update' functionality". Recent
  installer changes "were meant to also pave the way for adding auto-update".
- A2. Spec: the automatic-updates exclusion goes. "we are no longer initial scope, that spec item should go away." The
  notification non-goals go too: "it was likely a super early thing to simplify things there is no opinion we can't add
  it". "let's just do a proper auto-update, but we can have a simple opt-out in settings. default is always on."
- A3. No compatibility work for older releases: "assume I'll have a stable release out with whatever pre-requisite is
  needed. do NOT plan for any complexity around supporting old versions or whatever." The maintainer handles the release
  sequencing. Do not add code for apps or layouts older than the side-by-side layout.
- A4. Channel: "stable for now but I expect to add other channels later (not my current rc releases though)." Check
  GitHub's `releases/latest`, which excludes prereleases. Keep the release source a single function that a later channel
  setting can replace. Build no channel UI and no prerelease support now.
- A5. Timing: "on start-up and once a day", plus an on-demand check in the UI.
- A6. Install style: "chrome style. if you opt out, you just have to curl the installer manually. the default experience
  is just simple chrome style without a bunch of options (besides later on a channel pick, but out of scope for now)."
  The only option is the one on/off setting.
- A7. Install mechanism and trust: "for initial version, we just curl. same trust model as initial install." The app
  runs the installer the README tells users to run (`scripts/install.sh` from main on GitHub, piped to `sh`), with
  `FARHELM_VERSION` pinned to the release the check found. No signature verification. A separate Near term TODO entry
  (written by the planning PR, not by this plan) covers tightening release security later. This plan neither builds nor
  removes that entry.
- A8. Marker: the maintainer proposed "a red 'arrow' pointing up, that basically just takes the space of one more
  character", with the whole readout red instead of grey when there is an update. The hover says restart to update.
  Clicking opens the two-item menu (Restart to update, What's new); the maintainer accepted that "if simple" over their
  first idea of a click that only opened the release list.
- A9. Relaunch: the maintainer asked whether this needs a launcher layer. The planner answered no: a small detached
  helper waits for the app's process to exit, then opens the bundle with `open`, the pattern Sparkle and Electron use.
  The maintainer accepted.
- A10. Local host row: "let's not grey it out, let's have it instead be a consistent way to trigger an update check on
  demand." The `?` menu item does the same.
- A11. Planner defaults the maintainer agreed to: the marker shows whenever installed is newer than running, including
  after a manual install and with the setting off. The setting off stops only automatic activity. The marker appears
  only after an install finished. Background failures are logged, not shown, and retried at the next check.
- A12. Out of scope, agreed: Linux helm and browser UI; builds from main (`0.0.0-unreleased`) never check; remote hosts;
  backing up state before an upgrade, and downgrades (their TODO entries stay); signatures; prerelease channels.
- A13. Mac-only behavior cannot be checked by an executor running on Linux. This covers the real installer run, the
  relaunch, any macOS permission prompt when the app's own bundle changes, and quarantine attributes on what the
  installer downloads. The report lists these as checks owed on a real Mac. Accepted.
- A14. Review gate: "opus 5.5 + astra high (no swarm)". See Review gate below.
- A15. No-workhorse mode. New controls follow the landed hover-help mechanism.

Planner decisions from the planning-time scope review, with their reasons:

- P1. **No helm layer.** In the desktop build the UI components are native Rust in the same process as the desktop
  bootstrap, and they already call desktop-only code under `cfg(native_desktop)` (`auth.rs` and `api.rs` use
  `crate::desktop::…`). The updater lives in a desktop-only module of `farhelm-ui` and reaches components through the
  Dioxus context, the way `WebviewBootstrap` does. No helm endpoints, no helm injection, no "this helm has no updater"
  state. The webview still never talks to the internet: the network calls are native code, not page JavaScript. The
  feedback precedent forwards through the helm only because it starts in page JavaScript, which nothing here does.
- P2. **The setting lives in the desktop app's own state file** (`desktop/state.rs`, `desktop-client.json`, through its
  locked read-modify-write), as an optional field whose absence means on. It is a setting of this app installation. No
  browser can reach the embedded helm, so the shared helm preference row (a migration, new wire fields, and hiding the
  box in standalone helms) buys nothing. SPEC.md currently says the settings dialog has "exactly two checkboxes" and
  rules out per-client persistence "for these shared preferences". The spec PR allows a third, desktop-only checkbox and
  says this one belongs to the app installation.
- P3. **Gate.** The updater is active only when the running program is the main program of
  `$HOME/Applications/Farhelm.app` (canonicalized), that bundle has `Contents/Versions/`, and the compiled version is
  not a development build. `farhelm_supervisor::app_bundle::bundle_contents_of_main_program` is the existing helper. The
  reason: `install.sh` only ever writes that path, so an app running from anywhere else would install every day and
  never see its own `installed` record change. The same gate keeps the desktop smoke test and Linux CI off the network.
  Outside the gate, everything behaves as today.
- P4. **Probe.** Find the latest stable version by requesting `https://github.com/scode/farhelm/releases/latest` and
  reading the redirect's `Location` without following it, as `install.sh` does. This avoids the API's rate limit, and
  the installer itself always downloads, so it cannot serve as the check. Strip the leading `v`; `installed` holds the
  bare version.
- P5. **Success means the `installed` record.** `curl … | sh` exits 0 when curl fails, because `sh` reads an empty
  script. After the installer exits, re-read `Contents/Versions/installed`; the install succeeded only if it names the
  target version. Never trust the exit status alone. Whether to pipe or to download to a file first and run `sh` on it
  is the executor's choice; log it as a DECISION.
- P6. **Installer environment.** Start from the app's environment with every `FARHELM_*` variable removed (an ambient
  `FARHELM_INSTALL_TEST_BASE_URL` would redirect the download), then set `FARHELM_VERSION`. Keep `HOME`, `PATH` and
  proxy settings so the run stays "exactly as a user would". The installer's output goes to the app's log. A
  Finder-launched app's `PATH` has no tmux, so its log will show the installer's tmux advice; that is harmless. Record
  the choice in SPEC_impl.md.
- P7. **One update at a time.** When installed is already newer than running, the automatic check does nothing until a
  restart. This saves a download a day while an update waits, and it closes a rare case where installing a third version
  would remove the folder of the version still running (the installer keeps only Installed, the version it replaced, and
  Running, and Running is recorded by a supervisor the app may not own). An on-demand check may still install. Checks
  and installs are single-flight across all triggers.
- P8. **Clock.** One short tick, about a minute: re-read `installed` (cheap, and it is how a terminal install shows up),
  and run the automatic check when 24 hours of wall clock time have passed since the last one. Keep the last-check time
  in memory; the startup check covers restarts. A plain 24-hour timer would not do: tokio timers on macOS use a clock
  that stops while the Mac sleeps.
- P9. **Restart.** Spawn a detached helper (its own process group) that waits, bounded, for the app's process id to
  exit, then runs `open` on the bundle path. Then quit through the ordinary path that Cmd-Q takes; the managed
  supervisor's stdin tether and the existing 20-second state-directory wait (#1543) cover the handover, and sessions
  keep running. Only the version folder the new main program names is used on relaunch; nothing else is needed. Compile
  it for macOS only, or keep it behind the P3 gate.
- P10. **The menu.** The `?` menu's wiring (`HelpMenu` in `app_bar.rs`) is inline. Extract it into a small reusable
  bar-menu component that takes its items, and use it for both menus; do not copy it. Extract the external-link opening
  out of `open_documentation` into a shared helper for What's new. **Agreed fallback** (A8's "if simple"): if that
  extraction turns out not to be small, clicking the red readout restarts directly, with hover text saying so, and
  What's new is left out. Log the DECISION and put it under "things you should know" in the report. TODO.md's Near term
  entry "Help menu opens behind the terminal header" records that the `?` menu currently opens hidden behind the
  terminal pane's header. A menu built from the same machinery inherits that. If the entry is still open, fixing it is
  not this plan's work: say in the report whether the update menu is affected.
- P11. **Version comparison.** The semver helpers (`build_is_newer`, `is_development_build`) are `pub(crate)` in
  `crates/farhelm-helm/src/hosts.rs`, and `farhelm-ui` has no `semver` dependency. Export what is needed from the crate
  that owns it, or add the workspace dependency; do not write a third comparison.
- P12. **The local row's Update item** currently appears only when the provisioning menu state offers it. In the desktop
  build behind the P3 gate, offer it on the local row unconditionally; update the `update_menu_item` test and its doc
  comment. Elsewhere it stays as it is.
- P13. **Notification non-goals.** The pending `plans/queue/session-notifications.md` edits the same two SPEC.md lines
  ("Notifications (desktop or otherwise) are explicitly out of v1…" and "Notifications of any kind"). Remove them if
  still present and do not reintroduce wording that plan relies on. The update marker is an in-app indicator; do not
  invent notification semantics that plan owns.

## Implementation outline

A suggested stack. Reshape it if the code argues otherwise, without churn.

1. **Spec** (`docs:`). In SPEC.md:
   - Delete "Automatic updates are outside this initial scope" and describe the behavior above in "Installation and
     updates".
   - In Security, add the update check and the installer download to the outbound-connection paragraph. They are made by
     the desktop app automatically unless the setting is off.
   - Reword "Updates are user-controlled: optional or version-pinnable, never silently forced". Opting out keeps updates
     optional; the background install is the default. Also reword the mixed-versions rationale "since updates are
     user-controlled".
   - Settings dialog: a third, desktop-only checkbox stored with the app installation (P2).
   - The local row's Update (A10, P12).
   - The `?` menu's third item.
   - The notification non-goals (P13).

   In SPEC_impl.md:
   - The updater's design: gate, probe, installer invocation and environment, success test, clock, single flight, one
     update at a time, relaunch helper.
   - The app bar's readout states.
   - In "Release signing key", say that this updater does not verify signatures, so it does not yet trigger the rotation
     rule; the rule stays for the day one does.
2. **Updater core** (desktop-only module in `farhelm-ui`). Gate, probe, comparison, installer run, success check, state
   (idle, checking, installing, up to date, update ready, check failed) with a watch channel, the tick, the setting in
   the desktop state file. Probe, installer command and clock are injected as plain functions or arguments so unit tests
   run on Linux without network. No trait hierarchy.
3. **Marker, menu and restart**. The readout states and hover text, the extracted bar-menu component, Restart to update
   with the relaunch helper, What's new, the settings checkbox.
4. **On-demand triggers**: the local row's Update and the `?` menu's Check for updates, with the visible end states.
5. **Docs**:
   - `docs/install_uninstall.md`: "There is no automatic updater yet" (SPEC.md requires that document to stay accurate).
   - `README.md`'s install paragraph: "Does NOT auto-upgrade (yet)".
   - The website stub `website/src/content/docs/docs/using/update-and-uninstall.md`: write its Mac-update part.
   - `get-started/install.md` and `using/manage-hosts.mdx`: "update is greyed out".
   - `how-it-works/security-model.md`: connections leave "only when you ask".
   - The page that describes the `?` menu, if any.

   Read `website/AGENTS.md` and `website/EDITORIAL_RULES.md` first. Do not edit the docs Overview page or the intro
   SVGs; if they need a change, propose it in the report.

The `feat` PR that makes the feature visible carries the changelog fragment (`kind: added`, written for someone running
Farhelm, per `releasing/EDITORIAL_GUIDANCE.md`). The last code PR removes the Near term TODO.md entry "Auto-update the
Mac app", and only that entry: the release-security entry and the pre-upgrade backup entry stay.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-auto-update-log.md` in
the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before you
start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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
  `plan/auto-update/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within a run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to all of this plan's open PRs, including ones an earlier run built.
- Conventional Commits. The `feat` PR carries its changelog fragment under `releasing/changelog.d/` in the same commit,
  per root `AGENTS.md` (Releases and the changelog); validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; the planning system's monitor lands them (`plans/AGENTS.md`).
- Never edit `plans/`; this plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- Root `AGENTS.md` rules apply throughout, in particular: no test that changes the test process's own environment
  variables (inject the environment and the commands instead); the documentation pass over every touched file; and the
  live install on this machine is off-limits, so never run the real installer, the real updater against
  `~/Applications`, or `open` on a real bundle.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, run through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`
- `cargo check -p farhelm-ui --features desktop` and `cargo check -p farhelm-desktop`
- nextest selections of `farhelm-ui` (with and without `--features desktop`) and of any crate whose helpers you export
- `scripts/check-desktop-assets.sh` if assets change
- `python -B scripts/check-test-sleeps.py` when tests change
- `dprint check` on changed files
- `cd website && bun install --frozen-lockfile && bun run build` when website pages change

The updater's behavior is tested through its injected seams: decisions on probe results, success judged by the record,
the gate, the clock, single flight, and one-update-at-a-time. The mapping from state to readout class, glyph and hover
text is tested as pure functions. Run hover-help's coverage spec, and any Playwright spec whose controls changed (the
`?` menu, the host row menu), on Chromium and WebKit when the web build's DOM for them changed. Say in the report which
ran and why. The real installer run, the relaunch, and macOS prompts or quarantine are not executable here (A13); list
them in the report as checks owed on a real Mac, each with what to do and what to expect.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands (A14): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra agent at
high effort (shelled out to the other harness if the executing one cannot reach that model natively). No review swarm.
Both get the same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim. Address what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain before implementing a substantial departure from the outline above. Also
run one whenever the same component has needed repeated corrective review rounds. Examples of a departure (not a
blacklist):

- a helm endpoint or helm-side updater state;
- signature or checksum verification of the installer;
- a compiled relauncher program;
- a channel setting;
- persistence of check times;
- download progress UI;
- code for releases older than the side-by-side layout.

Supply the request, the decisions above, this outline, the current diff and the proposed departure (what changed, why it
is necessary, and which simpler alternative was ruled out). The charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular log:

- pipe versus a downloaded script (P5);
- how the comparison is shared (P11);
- the bar-menu extraction, or the P10 fallback;
- the hover and failure texts;
- the SPEC wording, especially the outbound-connection and user-controlled lines;
- every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The one agreed fallback is P10's direct-click restart. Anything else that needs a decision
(for example, a macOS fact discovered in the code that makes the relaunch or the background install unworkable as
designed): record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).

## Done criterion

The plan is complete when its linear stack of draft PRs exists, meets the acceptance criteria, has passed the review
gate, and has removed the Near term TODO.md entry "Auto-update the Mac app". The PRs are open, not merged: the planning
system's monitor lands them. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, with the
checks owed on a real Mac under "things you should know". Then write a closing entry in the log and stop the watchdog.
