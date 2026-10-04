## What this was about

Updating Farhelm on a Mac while it was open broke the open Farhelm until you quit and reopened it. The installer
replaced the `farhelm` program that the running supervisor keeps starting, for new sessions, agent hooks, conversation
reporters and agents' `farhelm` commands, so those ran the new version against the old supervisor: new sessions could
fail with "launch spec … malformed", conversation tracking was silently lost, and `farhelm spawn` failed.

You decided on the model Chrome uses on a Mac: one real app, every kept version side by side inside it, and a small
forwarder that picks which version to run. You chose it over a copy private to the supervisor because it also leaves
room for an emergency downgrade later. You also decided that it covers the Mac only, that the app becomes the whole
installation, and that a reopened Farhelm should wait for the one that just quit. And you asked for a written rule in
SPEC_impl.md that keeps what running sessions already hold working with newer programs. Four stacked PRs build that,
listed under The PRs below: the rule, the restart wait, the supervisor side of the versions, and the installer and
uninstall.

## Things you should know

- After PR 4, `~/Applications/Farhelm.app` is the whole installation. Inside it:
  - each kept version's `farhelm` lives in `Contents/Versions/<version>/`;
  - the _Installed record_, `Contents/Versions/installed`, names the version a fresh start uses;
  - `Contents/MacOS/farhelm` is the forwarder;
  - the installer's _ownership record_ marks the app as this installation's and names the `~/.local/bin/farhelm` link
    that belongs to it.

  The supervisor writes a _Running record_, `running-version` in its state directory, naming the version it runs. The
  installer keeps up to three version folders: the new one, the one it replaced, and the one the Running record names
  (which stays after Farhelm quits). `~/.local/bin/farhelm` is now only a link to the forwarder, and `farhelm-desktop`
  is gone from `~/.local/bin`.
- The forwarder decides which version a command runs. Inside a Farhelm session it runs the version that session's
  supervisor runs (from the Running record). Anywhere else it runs the Installed version. It is a POSIX `sh` script the
  installer writes, as you chose, so it needs no signing or release packaging. It replaces itself with the chosen
  program rather than starting it as a child, because the hook check walks the process tree and refuses an extra process
  in it. Arguments, environment and streams pass through untouched. SPEC_impl.md states its contract: a later installer
  may replace it only with one that handles every older invocation the same way.
- The first update into this layout happens with Farhelm quit: you said you are the only user so far and would do that
  one update with Farhelm quit, so nothing more than a message handles it. The installer's closing message says to quit
  and reopen. If Farhelm is open during that one update, it behaves the way updates do today: sessions that are already
  open may run the new program against the old supervisor until you quit and reopen. After that, updating while Farhelm
  is open is the point.
- An update changes the app in place, in this order:
  1. the new version folder;
  2. the app's main program (the switch);
  3. the icon;
  4. the forwarder, only if its text changed;
  5. the Installed record;
  6. `Info.plist`;
  7. the ownership record;
  8. finally `touch` and `lsregister -f`, so Spotlight sees the new version.

  The installer tests kill the installer after each of those steps. They check that the app still launches and that
  running the installer again finishes the update. The installer does not re-sign anything (the programs keep the ad-hoc
  signature Apple's linker gives every arm64 build), and nothing is quarantined.
- Reopening right after quitting now works. A starting supervisor waits up to 20 seconds for a predecessor that still
  holds the state directory but no longer answers. A supervisor that does answer still makes the new one refuse at once,
  as before. The app also waits for its supervisor to answer before it starts its built-in helm, because the helm's
  connection retries could otherwise outlast the app's 30-second startup check. When the app shuts its supervisor down
  through its own cleanup path, it now waits for an orderly shutdown before it would force-kill it.

  Review found that `farhelm uninstall` holds the same lock without answering. A waiting supervisor now refuses to start
  if its own program was removed while it waited. Without that, an app opened during an uninstall would have come up
  after the uninstall finished.
- You asked for no extra code to handle older releases beyond a few lines. These simplifications follow from that;
  please confirm them:
  - Moving into the new layout: an installation in the old layout is accepted only when the old layout's own ownership
    record names this `~/.local/bin`, or when it is an app from the earliest app installers, which wrote no ownership
    record. The old installer's support for moved or custom install directories is gone, and so is its support for
    another installation's leftover uninstall receipt. Such an installation is refused before anything changes, and you
    remove it by hand first. Since you are the only user so far, this matters only if your own installation is one of
    those shapes, and the installer would say so.
  - The Mac uninstall keeps no receipt for resuming a failed run. Running `farhelm uninstall` again works because of the
    removal order:
    1. the Running records, so a second run uses the Installed version;
    2. the other versions, the main program, the icon and `Info.plist`;
    3. last, the Installed version, the Installed record, the forwarder, the ownership record, the empty folders and the
       link.

    A failure in steps 1 or 2 leaves `farhelm uninstall` working. A failure in step 3 can leave no command to run. That
    needs a filesystem error while deleting a few files of your own inside your home directory, so I expect it to be
    rare. What remains is the app folder with at most one version's program and the `~/.local/bin/farhelm` link; it
    holds no data, and docs/install_uninstall.md says to move it to the Trash.
  - Installing an older release (one built before this layout) with the new installer is refused before anything
    changes. To spot such a release, the installer looks for one fixed phrase in the new app program, "needs its own
    version of the farhelm binary at" (part of the error it prints when its version folder is missing); older programs
    lack it. That costs one line, where a version comparison would need a table. A Rust test fails if the sentence
    changes in the app without the installer.
- That old-layout restriction does not apply to the new layout. An app in the new layout whose ownership record names a
  different `~/.local/bin`, because the home or bin directory moved, is still updated: the installer accepts any valid
  new-layout record and rewrites it to name the current link. A reviewer asked for this, so that a moved installation
  updates instead of being refused. It is a convenience you can veto.
- The macOS installer has dropped its old machinery for separate binaries: the two-binary recovery journal, the
  bin-directory lock and the separate ownership record. Its one lock is the existing one beside the app. Linux uninstall
  of installations made by older installers is unchanged.
- SPEC.md changes in two places. The installation section now says the app is the whole installation and the only way to
  launch Farhelm, as you asked. The other is the sentence saying that a session's `PATH` reaches "that exact build": it
  now says that in the app layout the `PATH` reaches the running version through the forwarder.

## Open questions and possible follow-ups

- Should a supervisor that shuts down cleanly remove its Running record? Today the record stays after a clean quit.
  While Farhelm is closed, commands from sessions that are still running keep using the version that last ran, and the
  installer keeps that version's folder. Removing the record would switch those commands to the Installed version and
  let the installer prune the old folder one update sooner. It changes what "running version" means in your design (the
  version the last supervisor ran), so I did not do it. I recommend leaving it as is: the cost is one extra kept folder.
- When the restart wait times out (a predecessor stuck for over 20 seconds), the supervisor's message says a just-quit
  Farhelm may still be shutting down, but the desktop's alert only says the supervisor exited with status 1; the reason
  is in the log. Surfacing the supervisor's last error line in the alert would be a small follow-up.
- Needs a real Mac before landing (nothing here could observe it):
  - An update with Farhelm open and a Claude session running: new sessions start, hooks report, `farhelm spawn` works
    from the session; then quit and reopen, and the old session works against the new version.
  - The forwarder script in `Contents/MacOS/farhelm`, reached from a live session's hooks and through `~/.local/bin`,
    including whether `codesign` or Gatekeeper object to a script inside an ad-hoc-signed app. The real-Mac probe done
    while planning used a compiled forwarder, so a script there has not been observed on a Mac.
  - Quit-and-reopen timing, including a reopen inside the shutdown window.
  - Spotlight/Launch Services showing the new version after `touch` + `lsregister -f`.
  - The one-time move from the old layout, on a Mac with a real old-layout installation.

## The PRs

They land in order. PRs 1 to 3 are safe without PR 4: PR 2 stands on its own, and PR 3's supervisor changes do nothing
until an app in the new layout exists. The stack is rebased onto main as of
`docs: plan permission prompts for farhelm CLI actions (#1556)`. Main had moved by the conversation-identity work
(explicit reports) and the docs website's Get started pages; nothing conflicted. The compatibility rule in PR 1 still
points at sections that exist, and PR 4 now also updates the website's Install and Uninstall pages for the one-app
layout.

1. https://github.com/scode/farhelm/pull/1541/changes `docs:` the SPEC_impl.md rule for what running sessions hold
   across versions.
2. https://github.com/scode/farhelm/pull/1543/changes `fix:` reopening Farhelm right after quitting waits for the old
   one instead of failing; the app no longer force-kills its supervisor without waiting; a supervisor that waited
   refuses to start if an uninstall removed it meanwhile.
3. https://github.com/scode/farhelm/pull/1551/changes `refactor:` the supervisor records the version it runs and points
   sessions at the forwarder when the app layout exists.
4. https://github.com/scode/farhelm/pull/1557/changes `feat!:` the installer builds and updates the one-app layout in
   place, with the forwarder; uninstall removes the app as the whole installation. Removes the TODO entry and adds a
   breaking changelog fragment.

## Checks

Run now, on the final commits. Brackets hold the start of each run's id in the test-run recorder's retained records.

- After the rebase: a compile and 509 focused Rust tests, all passing [ccc1f022]: the restart wait, the supervisor's
  launch and hook tests (the hook-identity end-to-end tests among them), the app layout, the desktop app's supervisor
  handling, and uninstall.
- Installer harness (`scripts/test-install-sh.sh`): 414/414 with GNU tools and 409/409 with BusyBox's minimal tools (in
  an Alpine container, the way CI runs it), including killing the installer after every rename of an update. `sh -n` and
  shellcheck clean.
- macOS uninstall Rust tests 37/37 [13af788d]; the desktop marker test that ties the older-release check to its message
  [2d30f51f]; the installer asset-table parity test [ba17355a].
- Clippy (all targets, and the shipped `farhelm` binary configuration), `cargo fmt --check`, dprint, the test-sleep
  checker and the changelog format lint: clean.
- Website build (`bun run build`), for the two edited pages: passes, all internal links valid.

Earlier in the stack, still applicable:

- PR 2: desktop smoke under Xvfb passed twice [5c8bc299, 87efdec1]; supervisor 40/40 [669a63d4] and desktop 5/5
  [33c33715] on the pushed commit. The smoke restarts only after the old supervisor exits, so it is regression evidence,
  not proof of the overlap case.
- PR 3: supervisor and UI 324/324 [41619f4f]; the hook-identity end-to-end tests 23/23 [24f21170].
- The macOS uninstall acceptance suite (`scripts/test-uninstall.py`, which drives the real installer and uninstaller)
  can only run on a Mac, so it ran on the repository's on-demand macOS CI job: success on PR 4 before the rebase
  (https://github.com/scode/farhelm/actions/runs/37171187804). Reused: the rebase brought no change to the installer,
  uninstall or desktop code.

Skipped: the full workspace nextest battery and browser tests. The changes sit in the supervisor's startup, the desktop
app's startup and shutdown, the installer and uninstall, all covered by the focused runs above; nothing touches the
helm, the UI or session behavior those would add. The CentOS provisioning gate was skipped because Linux hosts are
untouched.

## Review gate

Every PR went through two review rounds with the two independent AI reviewers the plan required (OpenAI's GPT-6 Astra,
and Claude Opus with no prior context), and every finding was fixed except these, declined with the reason:

- Surfacing the supervisor's error in the desktop alert after a timed-out restart wait (listed above as a follow-up).
- An end-to-end test that a stopping supervisor refuses connections while it still holds the state directory; verified
  in the code instead, since a fixture would need a wedged tmux.
- A test seam to pause the Running record write mid-operation; the function signature now makes the guarantee
  structural.
- A test for restoring an incomplete version folder when its replacement fails halfway; that branch is two lines, and
  completing an incomplete folder is tested.
