# Update while running: side-by-side versions inside Farhelm.app, with a forwarder for long-lived sessions

Written against main at fa5313dd on 2026-10-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a passage the user decided to change.

This plan runs after `plans/queue/install-output-layout.md` and `plans/queue/desktop-internal-helm.md` (its `INDEX.md`
line says so): the first rewrites the installer, including the app-bundle step this plan replaces, and the second
changes desktop startup, where the restart wait goes. Read what they landed before starting; names below come from main
at fa5313dd and may have moved.

The design was reviewed with the maintainer on a published page,
https://snippets.scode.org/s/farhelm-update-while-running/, whose last section holds the evidence (code facts,
Chromium's update script, and a real-Mac probe report). This file restates everything you need; the page is background.

## The goal

Updating Farhelm on a Mac while it is running must be safe, and must stay safe for a future in-app auto-updater. Today
the running supervisor (Farhelm's session manager, a child of the desktop app) keeps starting `farhelm` by the path it
was started from, for session launches, agent hooks, reporters and agents' `farhelm` commands on PATH; the installer
replaces that path, so between an update and a restart the old supervisor starts the new binary: launches can fail
opaquely ("launch spec … malformed"), hooks silently lose conversation identity on a protocol change, and agents'
`farhelm spawn` fails. After this plan:

1. `~/Applications/Farhelm.app` is the whole installation and the only user-facing way to launch Farhelm. It holds each
   kept version's command-line program in its own folder, `Contents/Versions/<version>/farhelm`, next to the app's main
   program `Contents/MacOS/farhelm-desktop`, the app's `Contents/Info.plist`, an Installed marker, and a small
   **forwarder** shell script at exactly `Contents/MacOS/farhelm`. `~/.local/bin` holds only a `farhelm` symlink to the
   forwarder, for Terminal use; the copies of `farhelm` and `farhelm-desktop` there go away.
2. Two records exist. **Installed** is which version the next start of Farhelm uses; the installer changes it, possibly
   while the old Farhelm runs, by replacing `MacOS/farhelm-desktop`, the Installed marker and `Info.plist`. **Running**
   is which version the Farhelm running now started from; only Farhelm writes it, when its supervisor starts, in a small
   file in its state directory.
3. The supervisor starts sessions with its own version's `Versions/<own version>/farhelm`, and everything that outlives
   a restart (hook command lines, reporter and helper variables, the session PATH entry) names the forwarder. The
   forwarder runs `Versions/<running>/farhelm` with the arguments unchanged when it is reached from a session, and
   `Versions/<installed>/farhelm` otherwise.
4. An update writes `Versions/<new>/` completely, then renames a fully written new `farhelm-desktop` over the old one,
   then a new `Info.plist`, then touches the app and runs `lsregister -f` on it. It never replaces the app folder as a
   whole. Old version folders are removed unless Installed, Running, or the version just replaced.
5. A restarted Farhelm waits, bounded, until the old supervisor has released its state-directory lock before starting or
   reusing one, and an app quit never hard-kills the supervisor.
6. SPEC_impl.md states the compatibility rule that every change must keep.

Acceptance criteria: on a Mac, with Farhelm open and sessions running, an update leaves new session starts, agent
reports and `farhelm spawn` working against the running version until the user quits and reopens; after reopening,
sessions started before the update work against the new version; an interrupted update leaves the app launching either
the old or the new version; a quick quit-and-reopen starts reliably. Linux behavior is unchanged.

## Requirement sources

**The user's request and decisions (2026-10-03),** in order:

- U1. "we are going to have to add auto-updating soonish. we cannot have a system that doesn't support updating while
  farhelm is running. so any solution that would not be compatible with an auto-update doesn't fly. and we cannot break
  existing stuff including hooks or anything like that."
- U2. Command lines baked into long-lived sessions must keep working with newer binaries, and "agent instructions (which
  would already be present) need to remain compatible … let's make sure the SPEC_impl calls this out. Any changes made
  must take these two points in consideration. this reminds agent code reviewers etc to look for it."
- U3. The user chose side-by-side versions with a forwarder over a supervisor-private copy: "option B seems better for
  any kind of "emergency downgrade" flow on bricked upgrade. it also seems pretty easy to understand." Emergency
  downgrade itself, and what happens to state across downgrades, are explicitly out of scope for now.
- U4. The user asked how Chrome handles the app bundle and accepted Chrome's model: one real app, versions inside it,
  the main program replaced as the switch, `Info.plist` last.
- U5. "yes local macos only." Linux session hosts keep being updated by the helm as today. No in-app auto-updater and no
  "Restart to update" button in this plan.
- U6. The forwarder is a separate tiny program ("agree on tiny program"), not `farhelm` handing itself off; U13 makes it
  a shell script.
- U7. Keep Installed, Running, and the version just replaced; remove other version folders.
- U8. Wait for `install-output-layout` and `desktop-internal-helm` to land first.
- U9. Review gate: both a gpt-6-astra high reviewer and a fresh-context Opus 5.5 reviewer, for every PR.
- U10. The maintainer is the only user and will quit Farhelm before the one update that introduces this layout.
- U11. "App is the only installation yes. nevermind the separate binary for now. SPEC should be clear on that. the app
  bundle is the only user facing surface to launch farhelm (for now)." The `~/.local/bin` copies of `farhelm` and
  `farhelm-desktop` go away; `~/.local/bin/farhelm` is only a symlink for Terminal use.
- U12. Older releases: "dont add ANY complexity to deal with older versions besides like TINY TINY 1-5 lines". If the
  installer can refuse a release that predates this layout in about five lines, it does; otherwise it does nothing about
  them.
- U13. The forwarder is a POSIX `sh` script that the installer writes ("shell script"), not a compiled program: scripts
  need no code signature on Apple Silicon, so it needs no release packaging.
- No-workhorse mode.

**Real-Mac evidence (macOS 26.6.2, arm64, SIP on; probe report linked from the page above).** These are binding facts
for the design:

- A running app survived its own bundle being updated in place (new version folder, main program renamed over,
  `Info.plist` renamed over); its helpers kept running from the old version folder.
- Replacing the whole bundle while running (today's installer) made the old app's later helper launches fail with
  "launch path not accessible".
- On Apple Silicon unsigned programs do not run at all ("ASP: Security policy would not allow process"), and a program
  whose signed bytes change is killed on next run ("Code Signature Invalid"). Every program placed must be signed (Rust
  and Apple's linker ad-hoc sign arm64 builds; never strip or rewrite a signature) and placed only by renaming a
  complete file.
- After an in-place update the bundle's resource seal is invalid, and the app still launches because it is not
  quarantined. Do not re-sign the bundle after updating (re-signing could rewrite the running program's file in place).
- `curl` sets no quarantine attribute; a quarantined copy was blocked by Gatekeeper. Nothing in this plan may quarantine
  anything.
- Spotlight and Launch Services kept the old version until the bundle was touched or re-registered.
- No App Management prompt appeared for updates from the terminal or from a shell the app started, on that host's
  permission state (terminal's App Management switch off). Not tested: Intel Macs, other permission states.
- Opening a running app (`open -a`, path, Dock, Spotlight) brought the running one forward, also after its files were
  replaced; relaunching after quit started the new version by every route.
- A forwarder reached through a symlink in `~/.local/bin` forwarded arguments with spaces and quotes unchanged (the
  probe used a compiled forwarder; a script forwarder in `Contents/MacOS` is not yet observed on a Mac).

**Code facts (main at fa5313dd; verify, they may have moved):**

- The supervisor records `current_exe()` once in `Supervisor::new_for_startup`
  (`crates/farhelm-supervisor/src/service/core.rs`) and uses it for the launcher command (`launch::window_command`),
  hook command lines (`agent_kind/mod.rs`), `FARHELM_PI_REPORTER_EXE`, `FARHELM_OMP_REPORTER_EXE`,
  `FARHELM_GOOSE_REPORTER_EXE`, and the launch spec's `farhelm_bin_dir` put first on session PATH
  (`launch::launch_child_command`). Tests construct supervisors through `new_with_exe`/seams.
- The desktop resolves `farhelm` as its executable's sibling (`bundled_farhelm` in
  `crates/farhelm-ui/src/desktop/bundle.rs`) and discovers or spawns the local supervisor in `DesktopBootstrap::start`
  (`crates/farhelm-ui/src/desktop.rs`); `impl Drop for DesktopBootstrap` calls `supervisor.kill()` (SIGKILL).
- The supervisor stops serving at once on quit and then spends up to `SHUTDOWN_OUTPUT_BUDGET` (about 10 s) closing tmux
  output clients while holding `supervisor.lock` (`StateDirOwnership`); an abrupt close can make tmux abort its whole
  server. During that window `discover_local_supervisor` reports none, a new child finds the lock held and exits, and
  the app refuses to start; or the app reuses an old supervisor that still answers.
- Sessions carry `FARHELM_SUPERVISOR_SOCK`, `FARHELM_SESSION_ID`, `FARHELM_SESSION_TOKEN` and the reporter variables.
  Goose stores `sh -c 'exec "${FARHELM_GOOSE_REPORTER_EXE:-farhelm}" internal goose-hook'` in its own session metadata,
  which outlives the session.
- `LaunchSpec` (`crates/farhelm-supervisor/src/launch.rs`) is unversioned serde JSON.
- Release packaging (`dist-workspace.toml`, `packaging/farhelm-desktop/dist.toml`, sign-sums, the installer's asset
  table, `RELEASE_ARCHIVES`) needs no change: the forwarder is a script the installer writes (U13).

## Implementation outline

Four PRs, in this order. Find code by name. Everything below applies only when Farhelm runs from the new bundle layout
on macOS; anywhere else (Linux, development builds, tests, a hand-started supervisor outside the layout) the supervisor
keeps recording its own executable as today.

### PR 1: the compatibility rule (`docs:`)

- SPEC_impl.md: a section, linked from wherever binaries and sessions are described, stating that a newer `farhelm` must
  accept everything a running or resumable session holds from an older one, because after a restart the forwarder (PRs 3
  and 4) hands those to the newer version: the hook command lines and their flags, `internal goose-hook` (stored by
  Goose beyond the session), the reporter and session environment variables, and the `farhelm` commands agents were
  instructed to use; and that the forwarder must forward unchanged. Say that reviewers check every change against it
  (U2).
- Planning review found that a version marker on the session launch description has no consumer in this design: the
  launcher that reads it is always the supervisor's own versioned binary, and cleanup keeps that version while it runs.
  Do not add one.
- No changelog fragment.

### PR 2: a restart waits for the old Farhelm to finish (`fix:`)

- Put the bounded wait inside the supervisor's own state-directory claim (`StateDirOwnership::claim`): retry the
  non-blocking lock until a deadline (longer than `SHUTDOWN_OUTPUT_BUDGET`, for example 20 s) instead of giving up at
  once, the way the helm's `SERVING_CLAIM_WAIT` claim in `token_control.rs` already does. The desktop keeps spawning its
  child as today; the child waits for the old supervisor to release the lock. No code outside the supervisor probes
  `supervisor.lock` (taking a flock to test it can itself block a starting supervisor). An answering supervisor is still
  reused at once, and a genuine second instance must still be refused promptly (check against what
  `desktop-internal-helm` landed for the helm's own claim).
- An app quit must not SIGKILL the supervisor: the stdin tether already triggers an orderly shutdown. Confirm what a
  normal Cmd-Q runs (GUI exits may skip destructors, so `Drop for DesktopBootstrap`'s `kill()` may never run), keep a
  hard kill only for a genuinely stuck supervisor after a bound longer than `SHUTDOWN_OUTPUT_BUDGET`, and log the
  DECISION.
- Tests: a start during the shutdown window waits and then succeeds; a timeout refuses with a message saying the
  previous Farhelm is still shutting down.
- Changelog fragment `kind: fixed` (reopening Farhelm right after quitting could fail to start).

### PR 3: the Running record, and sessions name the forwarder (`refactor:` or `feat:`, no user-facing change yet)

This PR is inert until PR 4 creates the layout: everything keys on the supervisor's executable being
`…/Farhelm.app/Contents/Versions/<v>/farhelm`, which nothing creates before PR 4.

- The supervisor, when its own path has that shape (take `<v>` from the path, not from its compiled version): after
  taking the state-directory lock and before serving, writes the Running record (`<v>`) by rename next to its socket, in
  the state directory; uses its own versioned path for the launcher; and uses `…/Contents/MacOS/farhelm` (the forwarder)
  for hook command lines, the reporter variables and the session PATH entry. Any Mac supervisor that takes the lock
  outside the layout (a development build) removes a stale Running record. The forwarder's path is load-bearing:
  desktop-managed supervisors before this plan ran from `Contents/MacOS/farhelm`, so sessions they started already hold
  that path, and putting the forwarder exactly there is what keeps them working without migration. Never rename it.
- The desktop chooses the `farhelm` it starts its managed supervisor from: `FARHELM_DESKTOP_FARHELM` if set (keep it;
  the desktop smoke depends on it); else, if its bundle has `Contents/Versions/`,
  `Contents/Versions/<its compiled
  version>/farhelm`, refusing with a clear error if that folder is missing (never
  fall back to the sibling, which is the forwarder); else its sibling, as today. Unit tests beside
  `resolve_sibling_farhelm`.
- SPEC_impl.md: the layout, the two records, and the forwarder's contract (where it lives, what it reads, that it passes
  arguments and environment unchanged and never probes `supervisor.lock`).
- Tests: given a bundle-shaped executable path, the supervisor writes Running, hands out the forwarder for hooks,
  reporters and PATH, and its own versioned path for launches; outside the layout nothing changes.
- No user-facing change, so no fragment, or `kind: none` with the reason.

### PR 4: the installer installs the app as the only installation and updates it in place (`feat!:`)

- SPEC.md (U11): the app bundle is the whole installation and the only user-facing way to launch Farhelm, for now;
  `~/.local/bin/farhelm` is only a link for Terminal use; updating while Farhelm runs is supported on the Mac, sessions
  keep working, and quitting and reopening finishes the update. Adjust whatever `install-output-layout` landed about
  `~/.local/bin` copies and the installer.
- `scripts/install.sh` (as `install-output-layout` left it):
  - Fresh install, or an app in the old layout (no `Versions/`): build the new layout fresh, as today's whole-bundle
    assembly does; the old-layout transition is done with Farhelm quit (U10), which `docs/install_uninstall.md` says.
    Remove the old `~/.local/bin/farhelm` and `~/.local/bin/farhelm-desktop` copies the installer owns, and create the
    `~/.local/bin/farhelm` symlink.
  - Update of a new-layout app: add `Versions/<new>/` (complete, signed files renamed into place), then rename a
    complete new `farhelm-desktop` over the old one, then the Installed marker, then `Info.plist`, then `touch` the app
    and run `lsregister -f` on it. Never replace the app folder as a whole, never re-sign it, never set quarantine.
    Remove version folders other than Installed, Running (read the record in the default state directory), and the one
    just replaced; a desktop run with an overridden state directory is protected for one update by the last rule
    (document, do not add machinery).
  - The forwarder (U13): a short POSIX `sh` script the installer writes into `Contents/MacOS/farhelm` by rename. It
    resolves its own real path (it may be reached through the `~/.local/bin` symlink) to find its bundle; if
    `FARHELM_SUPERVISOR_SOCK` is set and a Running record exists beside that socket, it uses that version, otherwise the
    Installed marker; it then `exec`s `Contents/Versions/<version>/farhelm "$@"` with the environment unchanged, and
    prints one clear line and exits non-zero if that version folder is missing. It never probes `supervisor.lock`. Keep
    its text fixed and minimal; a later installer replaces it only by rename and only with a version that forwards every
    older invocation unchanged.
  - Older releases (U12): if refusing a release that predates this layout fits in about five lines (for example a fixed
    marker string the new desktop binary contains, checked with `grep -q` on the extracted binary before anything
    changes), refuse with one `❌` line in the style `install-output-layout` landed; otherwise do nothing about them.
  - Messages: the update message is the one `install-output-layout` landed ("ready … quit and reopen Farhelm to finish
    updating"); do not change message texts.
- Ownership and uninstall (`crates/farhelm/src/uninstall/`): one ownership record, kept with the app, that names the
  `~/.local/bin/farhelm` symlink and records no digests for files that change on update; drop the flat `~/.local/bin`
  record model. Uninstall removes the app, the symlink, and the Running record; it still refuses while Farhelm runs.
  Update the installer's own bundle checks (which today expect exactly `farhelm` and `farhelm-desktop` as regular files
  in `Contents/MacOS`) to the new shape.
- Tests: `scripts/test-install-sh.sh` (macOS-shaped): fresh install creates the layout and the symlink and no flat
  copies; an update adds a version folder, swaps the program, the marker and `Info.plist` in that order, keeps the right
  folders, and leaves an interrupted run launchable (simulate interruption at each step); the old-layout transition; the
  forwarder script's choices (session with a Running record, session without, no session, missing folder) and argument
  pass-through (spaces, quotes, empty arguments); the older-release refusal if added. Uninstall tests for the layout.
  Run the macOS CI suite (`gh workflow run ci.yml --ref <bookmark> -f suite=uninstall-macos`).
- Remove the TODO.md entry "Update Farhelm while it runs". This is the plan's last PR.
- Changelog fragment `kind: breaking` (the app is the whole installation; `farhelm-desktop` is no longer in
  `~/.local/bin`; updating while Farhelm is open is now safe, and quitting and reopening finishes it).

### What not to build

- No auto-updater, "Restart to update" button, rollback command, or state downgrade handling (U3, U5).
- No change for Linux hosts or Linux helms (U5).
- No staging area, no "is Farhelm running" detection in the installer, no symlinked or stub app, no bundle re-signing.
- No compiled forwarder and no release-packaging change for it (U13).
- No handling of older releases beyond the optional five-line refusal (U12).

### Needs a real Mac

Executors run on Linux. Before delivering, list in the report exactly what still needs confirming on a real Mac (at
least: an update with Farhelm open and a Claude session running, then quit and reopen; the script forwarder in
`Contents/MacOS` from a live session's hooks and through the `~/.local/bin` symlink; the quit-and-reopen timing), so the
maintainer can run a Mac agent against the stack. Do not claim Mac behavior you did not observe.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-update-while-running-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/update-while-running/<nn>-<short-name>`.
- PR 1 to PR 4 in order, one commit, bookmark and draft PR each. Within this run, if a PR needs correcting, restructure
  it rather than stacking a correction on top, and do not add code in one PR that a later PR of this plan deletes.
- Conventional Commits; types as suggested per PR, chosen by user-visible effect. Each `feat`/`fix` PR adds its
  changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog); validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist:

- Every PR: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo check -p farhelm-desktop`, `dprint check` on changed Markdown.
- PR 3: the supervisor's launch, hook and listing tests, the desktop-feature tests for the supervisor choice, and the
  e2e modules that launch sessions with hooks.
- PR 2: the desktop-feature nextest selection for the bootstrap; the desktop smoke if this host can run it (say so if
  not).
- PR 4: `sh -n` and `shellcheck` on the installer, the forwarder text and the installer test,
  `bash scripts/test-install-sh.sh`, the uninstall tests, the macOS uninstall CI suite.
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, and `.agents/test-authoring.md`, for any PR
  that changes tests.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate **two** independent reviews of that PR's
changes, as the user demands (U9): one by a gpt-6-astra agent at high effort (shelled out through the harness that
reaches it), and one by a fresh-context Opus 5.5 agent at high effort. No review swarm. Each prompt carries the full
charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Check specifically that nothing a
> running or resumable session already holds (hook command lines and flags, the launch description, stored Goose
> commands, environment variables, agent-facing commands) stops working with a newer binary, and that the macOS rules
> quoted below are kept. Report findings to the named file, most severe first, each with the file, the quoted code, what
> is wrong, and the smallest fix. Edit nothing and touch no VCS state.

Each prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory), and the acceptance criteria: that PR's section above, The goal, U1-U10, and the
real-Mac evidence list. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim. Address what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (staging updates, detecting whether Farhelm is
running from the installer, a symlinked or stub Farhelm.app, re-signing the bundle, a dispatcher folded into `farhelm`
itself, changes for Linux hosts; these are examples, not a blacklist), and whenever the same component has needed
repeated corrective review rounds, run a fresh-context review through galaxy-brain with this charter, supplying the
request, the user decisions above, this outline, the current diff and the proposed departure (what changed, why it is
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
alternatives considered: in particular the restart wait's bound and messages, what a normal quit runs, where the Running
record and the Installed marker live and how the forwarder finds them, the forwarder's exact text, the installer's step
order and interruption handling, the old-layout transition, the ownership record's new shape, whether the older-release
refusal fit in about five lines, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. In particular: if anything would require replacing the app folder as a whole, re-signing it,
a stub or symlinked app, or changing what running sessions already hold, block. Anything that needs a decision: record
the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10). Because the PRs form one linear
stack, if one PR blocks, do not build the later ones on top of it.

## Done criterion

The plan is complete when the four draft PRs exist as one linear stack, each satisfies its section above, each has
passed both reviews of the review gate, and the last has removed the TODO.md entry. Open, not merged: merging happens
only after the maintainer has reviewed this plan's report, which lists what still needs a real Mac. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
