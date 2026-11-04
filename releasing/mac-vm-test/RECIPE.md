# Test a Mac release

For the standard **`brick test release vX.Y.Z`** request, use [BRICK-TEST.md](BRICK-TEST.md) to discover the private
machine profile, plan all required version pairs and record the checks below.

The host coordinates disposable Tart guests; an independent Codex tester **inside the Mac guest** executes this recipe
through genuine native CUA. It requires the two prepared base images, command and GUI access to the guests, readiness
checks, a way to launch the app and capture logs, and lifecycle scripts established by [BRING-UP.md](BRING-UP.md) and
documented in [OPERATIONS.md](OPERATIONS.md). If any prerequisite is missing, resolve it before testing. The script
mechanics were exercised on real guests; this procedure still requires evidence for every check and every candidate.

Each pass gets a fresh macOS clone and a fresh Linux clone. Install and update Farhelm only in the guests. Leave the
host's live installation and prepared bases untouched. Working clones contain real credentials and must never be
published. Store private evidence on the host outside repositories; redact any report you share.

## Inputs and readiness

Record the candidate tag (`v<version>`), the previous stable tag, the tag's commit and the SHA-256 of its `SHA256SUMS`,
and [the per-release addendum](#per-release-addendum-template). Run this gate before promotion: `/latest` is expected to
give the preceding promoted stable version while the candidate's version-path artifacts are available. Do not move
`/latest` to make the test runnable. After promotion it gives the candidate: find the preceding stable in the release
history, excluding RC and dev releases. `CHANGELOG.md` has stable `## vX.Y.Z` sections; cross-check the selected tags
against published releases and the site, since notes alone do not prove the bytes are available. Verify the ordering; do
not accidentally test candidate-to-itself. Choose a previous release from v0.23.0 onwards and require the candidate to
be newer by SemVer precedence.

Require the candidate's `install.sh`, `SHA256SUMS`, signature and needed payloads at
`https://get.farhelm.io/v<version>/`. The same applies to the previous release. A candidate already named by `/latest`
can be tested, but that run cannot prevent its initial exposure to users. This recipe does not alter publishing or
promotion. Capture `/latest` at the time the update path is selected, not only when the run starts.

Read the release's changelog fragments and protocol/schema changes to draft the addendum. The baseline below tests one
previous stable; SPEC.md requires clean upgrades from **every** stable release from v0.23.0 onward until upgrade epochs
exist. For this pre-promotion gate, the baseline range ends at the promoted stable named by `/latest`; an intervening
unpromoted tag is not another installed stable baseline merely because it exists. A passing previous-stable pair does
not prove that entire range. Name older-version risks and extra pairs in the addendum, and report the versions actually
covered.

Record the genuine agent CLIs selected with the maintainer in the addendum; Codex is the bring-up baseline. Additional
agents expand coverage and need their own authentication and conversation checks. Name omitted agents in the result.

Before each pass, use BRICK-TEST's supervised `serve` sequence, or OPERATIONS.md's lower-level `control.py init`,
`clone`, supervised `run`, `ready`, and `network.py` sequence. Establish Mac desktop and SSH readiness and verify the
Linux systemd user manager. Record guest OS/image versions privately. Confirm no Farhelm binaries or state exist in
either fresh clone. A broken fixture is “could not run,” not a product pass.

Fixture setup resolves OS/CUA permissions, tester installation and credential-copy consent before a release run. During
actual checks, a permission, login or tool refusal is a harness bug: stop and retain the failed attempt. Never grant
permissions or request manual approval to rescue that run. Ordinary Farhelm confirmations, including its YOLO launch
confirmation and per-run workspace trust, are authorized product actions handled by the guest tester. Do not change
confirmation preferences to suppress them. Use the actual native tool-returned image as GUI evidence; `screencapture`
can capture a different foreground app and cannot replace that image.

## P0 checks: install, upgrade and basic agent execution

Run these seven checks immediately after environment readiness and basic coverage planning, in order. P0 is a named
group: report all seven results and a separate P0 verdict. Stop at the first failed or unavailable check and retain its
diagnostics. A successful step never substitutes for a later step. A request to **stop after P0 checks** ends after P0
evidence is recorded and its successful fixtures are cleaned up, without sending feedback. Otherwise, report P0
promptly, run P1 on the last pair, and only then continue bonus work.

Use three independent guest pairs: `p0-fresh` for step 1, `p0-installer-upgrade` for step 2, and `p0-updater-upgrade`
for steps 3–7. Both upgrades start from the previous promoted stable and use its actual installed state. For each pair,
verify the candidate's published bytes against that stable's compiled trust ring as described in BRICK-TEST.md.
Export/record evidence and clean up each successful pair before starting the next. Unless scope is P0-only, keep the
last pair's candidate app running for P1 before that pair's final export and cleanup.

Identify genuine agent CLI/authentication prerequisites for the Mac and Linux hosts during environment readiness; Codex
is the baseline. Follow BRICK-TEST.md's permission preflight and obtain missing approvals together before product
checks, including any needed credential refresh on Mac and CLI/authentication setup on Linux. Prepare them when needed
for steps 4–5; do not delay available install/start checks while doing broader agent setup. Resolve missing fixtures
under the private operator policy. Authorization to copy a credential into Mac clones does not authorize copying it into
Linux clones. Keep credentials out of evidence. Agent version output or login status establishes only a prerequisite,
not successful agent execution.

1. **Fresh install and start.** Confirm both guests have no Farhelm binaries or state (`fixture-clean`). Install the
   candidate through its public version-path installer (`install`), open the installed Mac app, and require the
   candidate app version, a usable session list and a connected local supervisor at the candidate version (`app-start`).
   Save installer output, CLI version, GUI and supervisor evidence. End this fixture here: remote provisioning,
   conversations, templates, settings and feature checks wait until their designated P0 step or bonus coverage.
2. **Installer upgrade.** On a new pair, install and launch the previous stable with automatic updates disabled before
   first startup. Verify the old app and local supervisor versions (`old-fixture`); no old conversation, template,
   changed setting or remote workload is needed. Reobserve `/latest` with `select-update`, then run the verified
   candidate installer while the old app runs, with both `FARHELM_VERSION` and `FARHELM_INSTALL_SUMS_FILE` set as in the
   manual branch below. Require the old app to offer **Restart to update**, use that action, and require a usable
   candidate app and connected candidate local supervisor (`update-relaunch`). Do not manually open the candidate to
   hide a failed relaunch. This proves installer upgrade, not updater discovery. The local supervisor handover depends
   on the old app: one that starts its supervisor detached leaves it running when it quits to update, and the candidate
   replaces it on its first launch (an orderly stop, then its own); an older app's supervisor stops when the app quits.
   Either way, require the connected candidate supervisor; never stop a supervisor by hand to get there.
3. **Click-to-upgrade discovery and relaunch.** On another new pair, install the previous stable with automatic updates
   disabled and provision Linux through that old app. Verify old app/local/remote versions. Then, still on the old app,
   start one genuine Codex session on the local host and one on the remote host, send each one prompt that also asks it
   to remember a word, require a meaningful reply and the conversation captured in Farhelm (Restart is offered only
   after capture), and record each session's and conversation's identities and, if it runs in a terminal host, the
   host's and its child's process identities (`old-fixture`). Leave both sessions running. No saved template, changed
   preference or counter command is needed. Reobserve `/latest`, select the ordinary or stable-override path below, and
   explicitly click **Check for updates** in the old app. Require it to discover and install the candidate
   (`update-discovery`), then use the actual **Restart to update** action and require the candidate app to return, with
   the same supervisor handover as step 2 (`update-relaunch`). Verify override forwarding and removal when applicable.
   An RC/dev candidate or old app without override support may lack a genuine pre-promotion discovery path; report that
   step and P0 as could not run and stop. A manual installer cannot satisfy step 3.
4. **Local supervisor upgrade, one successful agent and the old session.** On the step 3 fixture, require the local
   supervisor to connect at the candidate version. The Mac app normally upgrades its managed local supervisor when it
   relaunches; observe that transition rather than requiring an extra Update click. If the product offers a local Update
   action, exercise it and verify the result. A stranded local host with an unusable Update action is a failure. Launch
   one genuine authenticated agent through Farhelm on the local host, send a small prompt, and require a meaningful
   reply and responsive terminal; read the new session's terminal kind (`host` or `tmux`) in its session details and
   check it against the terminal-host observation (`local-supervisor-agent`), and leave that session running: the
   old-session check samples its terminal. Then check the old local session from step 3 (`local-old-session`). First,
   before the session is touched, the fixed survival comparison checks that it survived the upgrade (same session, same
   launch generation) and settles the branch from its observed terminal kind:
   - **tmux:** it answers a new typed question whose answer (48) differs from the earlier turn's 42 still on screen. If
     the candidate creates terminal hosts (a host the old fixture did not have is running for the new session), the
     migration band shows on it and offers Restart (only Clone or Replace would be a failure for a captured
     conversation); Restart moves it to a host terminal, as its session details then say, and the resumed agent names
     the word from the earlier turn, which proves the resumed conversation rather than just any reply. If the candidate
     creates no terminal hosts, no band or restart is expected and the session keeps its launch generation.
   - **host:** the host and its child are the same processes as at step 3 (pid and start identity), the rediscovered
     terminal still shows its earlier screen and answers the new question; only then Restart, which must land on a new
     terminal host (not tmux) with the old host gone, and the resumed agent names the earlier turn's word.

   The check ends with a fixed final sample of the session and its terminals, which the import compares with the
   survival comparison.

   Keep version, GUI and command evidence. Preference preservation and other bonus conversations stay bonus checks.
5. **Remote supervisor upgrade, one successful agent and the old session.** On that same fixture, use the app's remote
   **Update** action. Require the formerly old remote supervisor to return connected at the candidate version and
   independently verify its running service. Launch one genuine authenticated agent through Farhelm on that remote host,
   send a small prompt, require a meaningful reply and responsive terminal, and read its terminal kind as in step 4
   (`remote-supervisor-agent`). A plain shell command does not count as an agent. Then check the old remote session from
   step 3 with the same branches as step 4 (`remote-old-session`); it runs after the remote Update because only then
   does that host create terminal hosts and show the band. Save version, service and GUI/command evidence. The counter
   workload's process survival and final quit/reopen belong to bonus coverage.

6. **Remove and re-add the remote host.** Keep both remote sessions from step 5 alive and record their identities. Call
   `remote_host_before`, then select the remote host menu's **Remove**, handling its ordinary confirmation if offered.
   Require the host and cached sessions to disappear; capture that native state. Call `remote_host_after` before
   re-adding: the original supervisor PID, binary, unit and data/database must remain. Re-add the same pinned SSH
   destination through Farhelm. Require discovery of the existing candidate supervisor and both original sessions with
   their conversations, without provisioning or creating replacements. Retain native images and `remote_version`
   evidence (`remote-host-remove`). Remove must not uninstall or end the sessions.
7. **Uninstall from the remote host.** Through Farhelm Stop both remote sessions, handle any ordinary confirmations,
   require all remote sessions ended and close any remote terminal tabs. Keep their data. Call `remote_host_before`
   while the connected supervisor still runs. Select the remote host menu's **Uninstall**, inspect and capture its
   actual removal plan and kept-data note, then confirm once. Require completion, disappearance of the host/cached
   sessions and the removal notice. Call `remote_host_after`: the lib directory/binary and unit file must be absent, the
   supervisor unloaded and stopped, and the data directory/database retained (`remote-host-uninstall`). Retain native
   completion evidence with the candidate app/local host still usable. Never use command-line uninstall or direct
   file/service changes to manufacture this result.

P0 deliberately carries no template, changed setting or counter workload. The old sessions in steps 3–5 are its one
exception, made at the maintainer's request while terminals move from tmux to terminal hosts: an upgrading user's
existing sessions are what that move can break. Branch only on terminal kinds observed during the run, never on a
release version: nobody can say in advance which release first creates terminal hosts. Where the previous tag's source
has no terminal host at all, its old session must be on tmux.

Keep setup, signature verification and bounded evidence capture alongside the P0 step they support. Do not expand P0
with release-specific feature checks or wait for historical-version investigations. Once all seven checks pass, answer
**did P0 checks succeed?** with that P0 result, candidate and previous stable, and the evidence references. Do not imply
that broader checks ran. Older manifests without these separate fixtures have no P0 verdict; retain their observations
as historical coverage rather than rewriting them.

## P1 check: send feedback from the candidate

Run only after all seven P0 checks passed with valid retained evidence. Use the Mac guest of `p0-updater-upgrade`, on
the candidate app those steps left running; do not create a new clone or install another app. Permission preflight must
already cover the live send to `farhelm.io`, which creates an issue in the maintainer's private feedback inbox. This
check does not run in stop-after-P0 scope.

1. Through computer use bound to this exact guest, open **?** and choose **send feedback**. Enter this exact message,
   replacing only `vX.Y.Z` with the candidate's full tag:

   ```text
   [brick-test] vX.Y.Z

   This is an automated release brick test checking that this release can send feedback; no action is needed.
   ```

   Use native CUA field entry inside the guest and inspect its reactive value before proceeding. Keep Tart's shared
   clipboard disabled and the host clipboard untouched. The first line must be exact so the inbox title is
   `Feedback: [brick-test] vX.Y.Z`.
2. Leave the contact field blank, clearing any prefilled contact. Inspect the dialog's display of what accompanies the
   message: the version must name this candidate and the surface must be the desktop app. Save an actual native PNG or
   JPEG of the filled dialog before Send. These are fixture premises, not inferred from which installer ran.
3. Press Send and observe it finish. Pass only if the dialog closes by itself without a failure message. Seeing the
   brief thanks is optional; it lasts only 1.5 seconds. A failure message or a dialog that stays open fails the check.
   Save another native image showing the closed dialog (or the thanks, if caught); on failure capture the visible
   failure state. Retain the agent's actual observation as the result detail. No command success receipt is required:
   the helm logs only failed sends, and the GUI closing is the success oracle.
4. The guest executor exports its native captures and imports `p1-feedback-send` automatically on `p0-updater-upgrade`,
   with GUI evidence and status pass, fail or could-not-run. Inspect `status` and report P1's named result and
   `p1.verdict` promptly, separate from P0. Retain failure evidence before cleanup; a P1 failure does not change a valid
   P0 verdict, does not prevent ordinary cleanup of this pair, and does not fence bonus passes.
5. Clean up this pair and wait for its foreground owner to exit. End here for **stop after P1 checks**; otherwise
   continue the bonus passes below. Whether a failed P1 should prevent promotion is the maintainer's decision for that
   release, not a rule this recipe chooses.

The full verdict includes P1. P0-only requested verdict follows P0; P1-only requested verdict follows P0 and P1
together. Historical manifests with no declared P1 check report it as not planned; do not retrofit a send into them.

## Bonus: full fresh install

1. Inside the Mac guest, fetch the candidate's public version-path installer and install with
   `FARHELM_VERSION=v<version>`. For example, after replacing the placeholder:
   `curl -fsSL https://get.farhelm.io/v<version>/install.sh | FARHELM_VERSION=v<version> sh`. This is the ordinary
   installer path, not a source build. Save output and check the installed CLI with `farhelm --version` (resolve the
   installed CLI path if the guest's shell has not refreshed PATH).
2. Open the installed `$HOME/Applications/Farhelm.app` in the guest using the verified launch/capture method. Confirm
   the app version, usable session list and connected local host. Save a screenshot and the app log.
3. Add the Linux guest as an SSH host through Farhelm. Confirm the candidate's helm provisions it from scratch and that
   the host becomes connected at the candidate supervisor version. Record the visible result and service evidence.
4. Start a real authenticated session for each selected agent on the local host. Give each a small conversation and
   require a meaningful reply, terminal input/output and a captured conversation. On Linux, start a plain command
   session and prove its terminal responds. Mock agents or a CLI `--version` alone do not satisfy this.
5. Check the window's title-bar band (`window-drag`). The Mac app draws its own title bar: empty background in the
   sidebar's top bar and in the main pane's top row of terminal tabs and session actions moves the window when dragged,
   and a double-click there zooms it and a second one restores the prior frame. Controls in those rows keep their own
   behavior. On the windowed app, with one of the sessions above selected so the main pane's row exists, first make sure
   the window has an ordinary, unzoomed frame with room to move (Window > Zoom toggles it). Drag from empty sidebar-bar
   background and then from empty main-row background, requiring the window to move each time with its size unchanged.
   On each of the two rows, double-click empty background and require a zoom, then find empty band background again in
   the zoomed window and double-click there, requiring the frame from before. Press and move on a terminal tab, on the
   delete button and on the settings gear and require the frame to stay put: a control never drags the window. Open a
   shell tab with the new-terminal button (+) after the tabs, then click the agent tab, the shell tab and the agent tab
   again, requiring each to become selected, then click delete and require its confirmation to open, cancel it without
   deleting, and close the shell tab again; click the settings gear and require its dialog to open, then close it, with
   the frame unchanged throughout. The contract fixes ten native images, which is all the executor retains: one before
   the first gesture, one after each drag, one zoomed and one restored per row, one with the shell tab selected by a
   click, one with the agent tab selected again and the delete confirmation open, and one with settings open. This check
   runs after the session steps because a fresh install has no session, and so no main-pane row, at startup.

   Native CUA crops images to the app, so an unchanged crop cannot prove that the window stayed put. The fixed
   `window_frame` operation reads the owned app's window ID, process ID and frame from the guest's public window-server
   API. Pin that identity throughout the check. Query it before the first image, immediately after each of the ten
   images and after every control interaction, including closing dialogs. The harness retains the measurements in one
   receipt and checks both moves, both zoom/restores and stationary controls. Derive gesture offsets from the actual
   image-to-frame scale returned with each image measurement; display pixel counts can describe a different scale. This
   command only measures geometry: all gestures and all ten images still come from native CUA. Retain separate image
   occurrences even when their cropped pixels are identical.

   Bonus fixture readiness must prove actual window motion and folder double-click navigation before installing Farhelm.
   OPERATIONS.md records a native runtime whose drags returned success while leaving Finder stationary; screenshots,
   clicks and text entry alone do not establish gesture readiness. (Its earlier scrollbar-drag and triple-click
   observations came from the host Tart binding.) If the native API lacks either gesture or refuses it, the check is
   could not run, not a product failure. Before reporting a failure, the tester checks that the gesture reached the
   guest: an unanswered drag is repeated on a Finder window's title bar, and an unanswered double-click is tried on an
   unselected folder icon in a Finder window in icon view, which should navigate into the folder (in column view a
   single click already shows a folder's contents). If that check gets no response either, the result is could not run.
   A Finder title bar is no oracle for double-clicks, because its action follows a user setting that may be off. Record
   the first run's actual gesture behavior in OPERATIONS.md's computer-use notes.
6. Run applicable addendum checks. Capture a screenshot per GUI check and command output for command checks. Export
   evidence with `control.py collect`, verify each exported archive, then `shutdown` and `delete` only this pass's owned
   clones. Bases remain stopped and unchanged.

## Bonus: full upgrade on existing state

1. Start fresh clones again and install the **previous stable** in the Mac guest with its version-path installer and
   `FARHELM_VERSION`. Use the verified bring-up mechanism to disable automatic updates before its first startup check;
   an immediate update would destroy the old-state fixture. Launch it and verify its version and connected local host.
   If that mechanism isolates the guest's network, restore it first; then provision the Linux guest through the old app
   and establish the remote supervisor's old version before proceeding.
2. Create realistic state: local sessions for each selected agent with conversations, a saved template, and a changed
   setting with an observable value. Save session and conversation identities and screenshots. Confirm automatic app
   updates remain off while seeding state, so `/latest` cannot update the old app before the intended action. Check it
   is still the old version with no candidate already installed. Choose settings/templates supported by that previous
   release.
3. Start a long-running plain command on Linux that emits an increasing counter and retains evidence of its original
   process identity/start time. Establish that it is alive and advancing before updating. Read its advancing output and
   process identity over SSH from outside Farhelm; a redisplayed terminal buffer is not proof of continued execution.
   Avoid a command that restarts itself and masks interruption. Settle the exact portable command and process oracle
   during bring-up. `continuity.py run <private-directory>` is the verified workload; call `observe` over independent
   guest SSH and compare observations with `--before` or `require_progress`. Require actual advancement before updating.
4. Select the update path using the next section, perform the install, wait for the old app's readout to show **Restart
   to update**, and use that action. Do not manually launch the new binary to bypass a failed relaunch. Capture the app
   log and readout, and the local supervisor's log, `~/.local/state/farhelm/supervisor.log` (and `supervisor.log.1` if
   present): an app that starts its supervisor detached sends the supervisor's diagnostics there rather than to the
   app's own stderr. With such an old app, quitting it to update leaves its supervisor running, and the candidate app
   replaces it on its first launch; with an older app the supervisor stops when the app quits. Record whether the
   trigger was automatic or manual for an in-app check; with automatic updates off, explicitly ask the app to check.
5. Confirm the app opens at the candidate version, `farhelm --version` names it, and the local host is connected at the
   candidate supervisor version. The CLI follows the installed record, so it confirms installation, not the restart;
   require the app readout and supervisor version too. “Needs update” with a disabled local Update action is a failure
   to record, not something to work around by killing a supervisor or deleting state.
6. Confirm sessions, template and chosen setting remain accessible as their documented contracts require. Prove each
   existing agent session responds; verify its captured conversation can be restarted and resumed through Farhelm and
   that the resumed agent retains the conversation. Keep the remote continuity command running while doing this.
7. Confirm the remote host offers Update (it may first show a version/protocol mismatch). Perform the update in the app,
   require a connected host with candidate supervisor version, and prove the original remote command remained alive and
   advanced across the update with the same process identity. A new command with the same text is a failure of
   continuity. Capture supervisor/service logs if the host fails to return.
8. Quit and reopen the app through the normal verified launch path. Recheck candidate version, connected local and
   remote hosts, existing sessions and responsiveness. Recheck the remote process's continuity. Run the addendum,
   collect evidence, and delete only this pass's owned clones after evidence is safe.

The supervisor is disposable; its sessions live in tmux or in terminal hosts, outside the supervisor, and must survive
app or supervisor restarts. For lifecycle details, read [SPEC.md](../../SPEC.md), “Durability and resume” and “Lifecycle
operations.” Do not reboot the guests during a continuity check: a host reboot legitimately ends its sessions.

## Select the upgrade path

Apply the first matching rule and record it in the report:

1. `/latest` names the candidate: use the ordinary in-app update. This is an after-the-fact check, including the
   bring-up dry run. It proves neither the override nor the manual candidate path.
2. `/latest` does not name the candidate, the candidate is **stable**, and the previous release contains the override:
   quit the old app, then launch that installed bundle with
   `open --env FARHELM_DESKTOP_UPDATE_LATEST=v<version> "$HOME/Applications/Farhelm.app"` **inside the Mac guest**. This
   command is a recommendation pending bring-up verification of the guest's `open` syntax, environment forwarding and
   log capture. Require the startup override announcement, then use the in-app update. Every existing verification check
   still applies. After Restart to update, ask the new app to Check for updates and require fresh native UI text to name
   the version `/latest` currently names as the latest release, rather than the candidate. The update control's
   accessibility text or visible tooltip can establish the exact version; a tooltip image is optional when accessibility
   text names it. Save that native observation, the endpoint observation and a candidate acceptance image together. An
   Up to date message alone cannot distinguish the two versions. With an unpromoted candidate they differ, so the exact
   latest-release readout observes that the override is gone. If `/latest` moved to the candidate meanwhile, the results
   no longer distinguish the probes: report that coverage gap, not proof that the override was removed. If support in
   the old release is unknown, establish it from its source before choosing this path; for example,
   `git grep FARHELM_DESKTOP_UPDATE_LATEST v<previous> -- crates/farhelm-ui/src/desktop/updater.rs` shows whether the
   source carries the variable. Do not infer support from a successful ordinary check.
3. Otherwise (including every RC or dev candidate and an old app without the override): use the candidate installer by
   hand **while the old app runs**. Inside the Mac guest, download its version-path `install.sh` and `SHA256SUMS` to a
   private run directory. Use the previous release's key ring, signature and trusted-comment verification, and
   installer-hash verification established by bring-up when available, and record its coverage or missing tooling. Run
   `/bin/sh` on that script with `FARHELM_VERSION=v<version>` and `FARHELM_INSTALL_SUMS_FILE` pointing at the downloaded
   candidate checksum file. Both variables are required: this exercises the permanent interface future installers must
   preserve for old updaters. The old app re-reads `Contents/Versions/installed` about once a minute; wait on the
   readout, then click Restart to update. This path skips the old app's probe and its own signature/trusted-comment
   checks, even if separate tooling verified the files. Report that gap. If the readout never changes, collect evidence
   and fail; do not replace its readiness check with a blind delay or relaunch workaround.

A malformed override fails each check without falling back to the site. It cannot select a prerelease, redirect the
origin, bypass verification or cause a downgrade. Do not change `/latest`, add a mirror, install a local certificate
authority or rebuild a specially versioned app to make the test pass.

## Per-release addendum template

Keep the filled addendum with that run's private host-side report, not as a new committed release document. It may add
checks or older-version pairs; it cannot silently omit a baseline check. Mark a required check that could not run.

```text
Candidate tag, tag's commit and SHA-256 of SHA256SUMS:
Previous stable and any extra upgrade pairs:
Latest endpoint observation and intended update path:
Changes from changelog fragments/release notes:
Protocol/schema changes and upgrade risks:
Extra hands-on checks (action, expected result, evidence):
Feature data agreed not to survive this upgrade (SPEC.md, Upgrade compatibility):
Unresolved questions or missing test environment:
Required permission scopes and approvals (no credential contents):
```

## Evidence and report

Collect one screenshot per GUI check, the first launch's app log and any relaunch logs the bring-up capture can actually
retain, the local supervisor's `supervisor.log` from the state directory when the release writes one, guest
`farhelm --version` output, remote supervisor version and command continuity evidence, and diagnostics for failures.
Copy them to private host storage before teardown. An unavailable relaunch log is a diagnostic coverage gap, not by
itself a failed functional check. Screenshots and logs can reveal account names, credentials, conversations and
addresses; do not commit or publish raw evidence. Shared reports use portable evidence labels and redacted copies, not
hostnames, usernames or private paths. `window-drag` instead keeps exactly the ten native images its contract names.

```text
Verdict: pass | fail | could not run
Scope: full | stop after P0 | stop after P1
P0 verdict and seven named results:
P1 verdict and p1-feedback-send result: pass | fail | could not run | not planned
Requested-scope verdict:
Candidate / previous stable / extra pairs:
Tag's commit; SHA-256 of SHA256SUMS; guest OS versions (portable summary):
/latest observation; selected update path; actual trigger:
Addendum summary:
Check | expected | observed | pass/fail/could not run | evidence label
...one row for each baseline and addendum check, per pass/pair...
Failures: what was seen, what should have happened, relevant evidence
Coverage gaps: unrun checks, manual-verification omissions, untested version pairs
Cleanup: owned clones deleted or retained by agreement; evidence retained
```

“P0 pass” requires all seven P0 checks to run and pass. “P1 pass” requires the feedback send after valid P0. “Full pass”
additionally requires P1 and every planned bonus check to run and pass. A product failure makes the relevant verdict
fail; a missing test environment with no demonstrated product failure makes it could not run. Never treat a skipped
check or a clean process exit as a pass. This result covers the actual versions, artifacts and paths exercised, not zero
risk for every supported installation.
