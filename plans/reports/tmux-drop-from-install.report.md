## What this was about

You asked for tmux and xterm.js to be removed from Farhelm completely, in four chained plans. This was the second: the
installation. After the first plan, no supervisor ran tmux any more, but the Mac app still refused to start without tmux
3.7c or newer, `farhelm helm setup` still looked for one and wrote it into the supervisor's service, the installer still
told Mac users to install tmux with Homebrew, setting up a remote Linux host still checked its tmux and installed
Farhelm's own build when the host's was too old (most Linux distributions), and every release still published two static
tmux builds for that. This stack removes all of it, apart from the step that ends the old private tmux server at
supervisor startup, which none of the four plans removes; a later phase after all four does (its own TODO.md entry,
"Remove the startup shutdown of the old private tmux server").

What lands, in product terms:

- **The Mac and the helm's machine.** The app starts without tmux, `farhelm helm setup` neither looks for nor records
  one, and `farhelm helm setup --tmux` is gone (a script passing it now gets an error). The installer no longer checks
  for tmux or gives Homebrew advice. The supervisor's service no longer names a tmux or puts the directory of the tmux
  that setup had approved on its `PATH`.
- **Remote Linux hosts.** Adding or updating a host no longer checks for, downloads or installs tmux. Add and Update now
  remove the tmux an earlier release installed there, as a step named "Remove the bundled tmux" in the plan you approve,
  run after the new supervisor has started. If the removal fails, the step shows a warning and the run still completes.
  Uninstall no longer runs a tmux command.
- **Releases.** A release publishes four archives and `install.sh`, no tmux builds. The release workflow refuses a
  release that carries one.

## Things you should know

- **Your signing tooling must change before the next release.** The signed `SHA256SUMS` now lists exactly five files:
  `farhelm-aarch64-apple-darwin.tar.gz`, `farhelm-aarch64-unknown-linux-musl.tar.gz`,
  `farhelm-desktop-aarch64-apple-darwin.tar.gz`, `farhelm-x86_64-unknown-linux-musl.tar.gz` and `install.sh`. The two
  bare tmux builds (`tmux-aarch64-unknown-linux-musl`, `tmux-x86_64-unknown-linux-musl`) are gone. Tooling that expects
  six payloads will find two fewer files. `releasing/AGENTS.md` states the list, and the stable and RC procedures now
  tell the release agent to say so when it hands you the release to sign.
- **`tmux` in sessions on remote Linux hosts changes.** On a host where Farhelm had installed its own tmux, `tmux` typed
  in a session was that copy, because its directory comes first on the `PATH` sessions start with. After the update it
  is the host's own tmux, or none. A personal tmux server someone started from the bundled copy keeps running from the
  deleted file until they end it. The changelog entry says this, and is typed as breaking for that reason.
- **The `PATH` sessions start with loses a directory where the user's own tmux lived somewhere unusual.** Setup and
  provisioning used to add the approved tmux's directory (for example Linuxbrew's `bin`) to the supervisor's `PATH`.
  They no longer do, so programs only reachable through it are found from a session only if the login shell adds that
  directory. A service written by an earlier release keeps its `FARHELM_TMUX` line (the line that named the tmux program
  for the supervisor) until the next setup or Update rewrites it; nothing reads it.
- **Every Add and Update plan for a remote Linux host now shows "Remove the bundled tmux"**, including the first Add of
  a host that never had Farhelm, where it finds nothing to do. It has to stay as long as Update can reach hosts that may
  carry the copy (see the open questions).
- **Uninstall's accepted residual**, as decided while planning: a private tmux server that had already outlived an
  earlier stop of the supervisor (on a host never updated to this release, say) survives uninstall.
- **The installer change went into the first PR**, not a third one of its own, as the outline allowed. The first PR
  makes the app stop needing tmux, so the install page's "If the installer asks for tmux" section became false in it; a
  later installer PR would have had to rewrite text the first had just rewritten. The stack therefore has three PRs, and
  the release-payload step is the third.
- **`farhelm internal verify-release-sums` is removed.** Its only user was the Mac development build script's download
  of the tmux builds, which went with it. The signed release fixtures the tests use were signed again, without the tmux
  files, by a new throwaway key whose secret was deleted.
- **Removed as moot, with their triage records archived where they had one:** the review-queue items about the desktop
  smoke's tmux check, the offline-uninstall tmux history, and three about the development build's tmux download and
  checksum handling (the one about the development build reading the release's unauthenticated checksum files without a
  size limit was not named in the plan, but the code it concerned is gone). A BUGS.md entry about an abrupt supervisor
  death crashing the private tmux server was removed too: it said it stayed only because the tmux version floor cited
  it, and the floor is gone.
- **The published "What setup will create on the host" screenshot** on the Add a remote host page still shows the old
  tmux upload and install steps until the next docs-screenshot refresh; the page text describes the new plan. No
  re-capture was done, as decided.
- **Docs pages changed** (no preview server, as for any unattended run): Get started, Install (the "If the installer
  asks for tmux" section is gone) and Add a remote host; Using Farhelm, Manage hosts, Update and uninstall, and New
  terminals (its "tmux is still needed" section now says nothing needs tmux). Also `docs/install_uninstall.md` and
  `docs/old_readme.md`.
- **Agent instructions changed with the code they describe:** the release procedures (above), the root `AGENTS.md` and
  `.agents/machine.md` (a note about tmux release-asset builds), and the check inventory and narrow-test and Tensorlake
  notes in `.agents/` (`validation.md`, `run-all-tests.md`, `narrow-tests.md`, `tensorlake.md`).
- **Not checked on a Mac.** No Mac was used: the app starting without tmux was checked by the Linux desktop smoke and
  the desktop crate's tests, the installer by its Linux harness with a Darwin shim, and the development build script
  only by `bash -n` and shellcheck.
- **Left for the later plans:** the release gate's pinned test tmux and its build script (still used by the tests that
  build an earlier release's server), the desktop smoke's need for tmux on `PATH`, the brick test's setup notes that
  still expect Farhelm to install tmux on a Linux host (`releasing/mac-vm-test/BRING-UP.md`) and its Mac base to have
  Homebrew tmux (`releasing/mac-vm-test/OPERATIONS.md`), and remaining old-terminal wording in specs, docs and test
  comments.

## Open questions and possible follow-ups

- **When should the "Remove the bundled tmux" step go?** It is needed while Update can reach a host that still carries
  the copy: any release from 0.23.0 (the current upgrade floor) up to the one before this one may have installed it, so
  an Update from any of them can meet it. Nothing tracks its removal yet: the TODO.md entry for that later phase
  ("Remove the startup shutdown of the old private tmux server") does not list it, and agents may not add TODO entries.
  Options: remove the step once the upgrade floor passes this release, or remove it with that later phase, whose own
  timing that entry leaves to be decided when it is picked up; either way a host updated after the step is gone keeps an
  unused copy that still answers `tmux` in its sessions. I recommend adding the step to that TODO entry with the
  condition that it goes only once the upgrade floor passes this release.
- **TODO.md still says "the maintainer's signing of the six payloads"** in a release-promotion entry. It is your file,
  so it was left alone; I recommend rewording it to "the signed checksum list".
- **A queued review finding now cites deleted spec text.**
  `review_feedback_queue/tmux-image-role-recognition-unresolved.md` says cleanup might miss a tmux server a user started
  inside a session when the tmux program has an unusual file name; it rests on a SPEC.md paragraph about which tmux
  programs Farhelm supports, which this stack deleted because Farhelm supports none now. I recommend re-triaging it at
  the next triage, where it can be judged against the current spec; it is not this plan's to remove, and the fourth plan
  keeps it too (it concerns the user's own tmux), so triage is the only place it gets looked at again.
- **Recapture the Add a remote host screenshot** at the next docs-screenshot refresh.

## The PRs

- [#3371](https://github.com/scode/farhelm/pull/3371/changes): stop requiring tmux on the Mac, in `farhelm helm setup`
  and in the installer (breaking: `--tmux` removed).
- [#3373](https://github.com/scode/farhelm/pull/3373/changes): stop installing tmux on remote Linux hosts, remove the
  bundled copy on Add and Update, drop tmux from uninstall, delete the tmux version policy (breaking: `tmux` in sessions
  on such hosts changes).
- [#3374](https://github.com/scode/farhelm/pull/3374/changes): stop publishing tmux builds with releases, with the
  signing membership above.

## Checks run, reused and skipped

The slow end-to-end runs were on Tensorlake sandboxes at your request, through the test recorder like local runs; the
sandboxes' Node is 24.16 against 24.20 locally, which these tests do not depend on. Every sandbox booted systemd, had a
running user manager and working self-ssh; the Rust one also had Podman 4.9.3 with the pinned base image.

At the tip of the stack (#3374, then on main `43175a9d`), from a fresh build:

- The browser provisioning spec, Chromium on one sandbox and WebKit on another: 92 passed on each (`46cf3b03`,
  `88bdeade`).
- The real Podman environment test, from a nextest archive on a sandbox: passed, with no early return (`af25faa0`). It
  ran because #3373 changed what is installed into a container.
- `scripts/test-provision-centos.sh`, which provisions and updates a CentOS Stream 9 container over ssh: passed
  (`f93695cf`). It ran again because #3373's review fixes changed how the remote removal looks the file up.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (no errors; one old warning in the triage archive), `dist generate --check`,
  `cargo check -p farhelm-ui --features desktop` and `cargo check -p farhelm-desktop`: passed.

Per PR, run locally through the recorder unless noted:

- #3371: the `farhelm` binary's tests with the helm's unit and provisioning tests and the supervisor's tmux module, 579
  passed (`2ef5604d`); the desktop crate's tests, 1,159 passed (`47b32a8e`); the setup command-line tests, 2 passed
  (`a71efbba`); the desktop smoke under Xvfb, every leg passed (`32c60e83`); the installer harness, 548 checks passed
  (first without the test recorder, as the check inventory lists it, then through the recorder on the tip, `d59d69fd`).
  Also `cargo check -p farhelm-ui --features desktop`, `cargo check -p farhelm-desktop`, `bash -n` and shellcheck on the
  changed scripts, `sh -n` on the installer, the isolated test-sleep checker and the website build. The review fixes
  changed only text and a development-build prerequisite, rechecked with formatting, `dprint`, `bash -n` and shellcheck.
- #3373: the helm's provisioning and environment tests, 393 passed (`039a3baf`; an earlier run, `19f3c186`, failed one
  test on the new step's missing confirmation text, which was then added); the uninstall tests, 52 passed (`0290d38d`);
  the CentOS test, passed (`130f4172`; an attempt before it, `951f4891`, refused before running anything because the
  pinned nextest was not on `PATH`, and is kept). After the first review's fixes, 394 passed (`9c362812`); after the
  second's, 394 passed (`76777ef4`). Also the test-sleep checker, shellcheck and the website build.
- #3374: the helm's provisioning tests, 359 passed (`bf019818`); the `farhelm` binary's tests, 190 passed (`32b1aa74`);
  `dist plan` (four archives), `scripts/check-static-elf.sh --self-test`, `bash -n` and shellcheck on the changed
  scripts, and the isolated test-sleep checker (no unexplained delays). The review fixes changed only text, rechecked
  with `dprint`, `dist generate --check` and the agents check.

Before delivery the stack was rebased onto main `78e9d68a`, which had gained one change: the installer's update message
and two spec passages say again that sessions keep running across an update. It applied cleanly; the installer message
keeps that sentence, and the installer harness passed again on the rebased tip, 548 checks (`ced291c0`), with `sh -n`,
shellcheck, `dprint check` and the changelog check. The tip runs above are reused for the rebased stack, since that
change touches nothing else they cover.

Reused: #3371's desktop smoke, desktop crate tests and setup command-line tests for the tip. They ran on #3371 before
the installer change was folded in and before its review fixes (`0af44484`); both changed only the installer's scripts,
text and a development-build prerequisite. #3373 and #3374 touch neither setup's command line nor the installer. After
#3371 the app's own code and its startup do not change; #3373 and #3374 change the helm built into it (provisioning and
the release asset table), which the helm's tests at each PR, the CentOS and browser runs and the desktop builds at the
tip cover.

Skipped: the whole browser suite and the whole Rust workspace, since the changes are confined to setup, the desktop
app's startup, provisioning, the installer and release files, each covered by the targeted runs above; anything
macOS-only (no Mac in this run); `actionlint` on the changed workflows (not installed; the edits there remove list
entries, comments and one copy line, and `dist generate --check` covers the generated release workflow); and the README
and docs screenshot capture harnesses, whose staged scenarios lost only the removed "host needs tmux" flag, since
nothing was re-captured, as decided (the provisioning spec above covers the same flag).

## Review gate

Every PR had the required fresh-context review by Opus 5.5 at high effort before it was pushed. Every finding was fixed
except one, explained below.

- #3371: no correctness bugs; two medium leftovers (the implementation spec still said the installer checks tmux, and
  the development build script still required Homebrew tmux for the app's sake), two low (a doc comment promising a
  byte-for-byte check that no longer exists, and the changelog overstating the `PATH` change), all fixed. Not followed:
  the reviewer suggested that writing the supervisor's service should refuse a `farhelm` program given without a
  directory, instead of keeping a guard that leaves such a program's (empty) directory off the service's `PATH`. An
  empty entry there would make the current directory part of `PATH`, so a stray program in whatever directory the
  service starts in could run in place of a real one. Nothing passes such a program today (setup and provisioning always
  pass a full path), the guard and its test say so and why, and a refusal would add code for no caller.
- #3373: nothing blocking; six low and some nits, all fixed: a failed tmux removal failed the whole Add or Update, the
  removal could report a file "already gone" when it could not see it, an ordering test checked a neighbour, the
  changelog entry told users what to do (the release-notes guidance asks entries to describe what changed, not give
  instructions), and sandbox image preparation ran the step needlessly. A second review of those fixes found three more
  low points (the host's error shown twice, "was left in place" when the outcome could be unknown, and no test of two
  warnings in one run), all fixed.
- #3374: no correctness bugs; one medium (the release procedures' hand-off to the signer did not mention the new list),
  three low spec and comment points, and nits, all fixed. The reviewer checked every fixture signature independently.

Commit messages each went through cold reads (three for #3371, two for #3373 and #3374).

### Landing

#### What landed with it

Nothing else landed in this round. Since the stack was delivered, main gained only plan-queue changes. The stack was
already based on the one change that came just before it, which put "Your sessions keep running." back into the
installer's update message and two spec passages; the installer keeps that sentence. A rebase that applies cleanly can
still drop another plan's rules, so one of two independent reviewers checked only the interactions with what is on main,
above all the previous tmux plan (ending old terminals at startup). It found none that break anything:

- the startup step that ends an earlier release's private tmux server finds it by its socket and command line and never
  runs a tmux program, so deleting the bundled tmux cannot make it miss the server; on Update the removal runs after the
  new supervisor has started, so that step has already run;
- the tests that build an earlier release's tmux server still get the pinned test tmux everywhere they run;
- containers and Tensorlake sandboxes run the removal step too, and nothing else on main still uses what the stack
  removed.

The other reviewer read the provisioning, release, desktop and setup code adversarially. Nothing needed a code change:
the removal step deletes only the private directory's `tmux`, removes a symlink rather than what it points to, and
counts the file as already gone only when the host says so; and nothing about release verification was loosened.

#### Fixes made while landing

No behavior changed: docs, spec, changelog entries, a code comment, and the explanation users see under the "Remove the
bundled tmux" step of the plan they approve, each in the PR it corrects:

- **The tmux copy that updates before 0.18.0 left beside `farhelm` is not removed** (#3373). Before 0.18.0, Update put
  Farhelm's tmux next to `farhelm` itself, which on many hosts is a shared folder such as `~/.local/bin`, and 0.18.0
  deliberately left that copy in place, since the file there could be the user's own. This stack rightly leaves it alone
  too, but the spec, two docs pages, the changelog entry and the "Remove the bundled tmux" step's own explanation said
  that afterwards `tmux` in a session is the host's own. On such a host it is still that old copy, because that folder
  comes first on the `PATH` sessions start with. The step's explanation now says only that `tmux` in a session no longer
  runs the removed copy; the spec, the docs and the changelog say the older copy stays. A code comment claiming every
  earlier release put the copy in Farhelm's private folder was corrected.
- **The changelog entry about the supervisor's service** (#3371), which covers both the helm's own machine and remote
  Linux hosts, gained a note for the release-time editor: the supervisor finds `systemd-run`, `systemctl` and `git`
  through the same `PATH` that loses the tmux directory. On a machine that keeps those programs outside
  `/usr/local/bin`, `/usr/bin` and `/bin` (NixOS, say) and whose tmux lived in the same directory, the old service found
  them only through the tmux entry; after the next setup or Update its sessions start without their systemd scope, and
  repository discovery fails. Those programs were reachable that way only by accident, through the tmux folder (hosts
  that used Farhelm's own tmux never had that folder on `PATH`), so I did not change the code; see below.

#### What the report says that is not quite right

- "On a host that had that copy, `tmux` typed in a session now runs the host's own tmux": not on a host that an update
  before 0.18.0 gave a second copy beside `farhelm` (above).

#### Things you should know that the report does not say

- **Machines whose system programs live outside the standard folders** (NixOS and the like), the helm's own or a remote
  Linux host, can lose the supervisor's scope containment and repository discovery after setup or Update, as above,
  whenever their tmux sat in the same folder as `systemd-run`, `systemctl` and `git`, which is the usual NixOS layout.
  Elsewhere nothing changes.
- **On Add, the removal relies on Farhelm's check for an existing supervisor.** When you set up a host, Farhelm offers
  Update if it finds a supervisor there and Add if it finds none. Update restarts the supervisor before the removal, so
  no earlier supervisor can still be using the file. Add only starts the service, which leaves an already running one
  alone, so the guarantee holds there only because that check found none. One the check misses (an earlier supervisor
  run with an unusual state directory) would see its tmux deleted. Its old tmux server keeps running from the deleted
  file, and its old terminals end at its next restart, as they would at that restart anyway since the first tmux plan,
  so the deletion loses nothing extra.

#### Possible follow-ups

- **Decide whether Farhelm supports machines whose system programs live outside `/usr/local/bin`, `/usr/bin` and
  `/bin`.** Options: give the service's `PATH` a deliberate rule for them (for example, the folders the machine's own
  `PATH` names for `systemd-run` and `git` at setup), or declare them unsupported. I recommend the rule if you know of
  users on NixOS; otherwise saying so in the supported host setup is enough.
- The executor's open questions stand: when the "Remove the bundled tmux" step can go, rewording "the six payloads" in
  TODO.md, re-triaging the queued finding about a user's tmux with an unusual program name, and recapturing the Add a
  remote host screenshot.

#### Checks

The slow runs were on boat sandboxes at your request (2026-11-12: boats instead of Tensorlake), through the test
recorder, as on a local machine, on reused 8-core Ubuntu 24.04 machines: the browser suite on seven, the Rust tests on
three. Their Node is 24.22, against 24.20 on the development machine, which these tests do not depend on. Every run
showed systemd's user manager running and working self-ssh, and the Rust runs Podman 4.9.3 with the pinned base image.
Two things about boats that affected the first runs, both fixed in my runner before the reruns below: boat's command
runner gives even its ordinary user the capability to read any file, which made the one test that expects an unreadable
file to be refused fail; and with eight parallel test slots, nextest never started the real Podman test (it needs four)
or 57 other end-to-end tests queued behind it, without reporting a failure. The runner now drops that capability and
uses the four slots the recorder's own nextest mode uses.

On the stack as delivered, restacked onto main:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, `dist generate --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, `cargo check -p farhelm-ui --target wasm32-unknown-unknown`,
  `cargo check -p farhelm-desktop`, `scripts/check-desktop-assets.sh`, `scripts/check-static-elf.sh --self-test`,
  `sh -n scripts/install.sh`, shellcheck on the installer, its harness and the changed scripts, the installer harness
  (548 checks, `84e595ca`) and the website build: passed. shellcheck also reports informational findings in
  `scripts/desktop-smoke.sh` (unreachable-looking trap handlers and the like), of the same kinds as on main; that script
  is not in the shellcheck gate.
- The whole ordinary browser suite on both engines, from a fresh build: 3,309 passed, 3 failed, 14 skipped by the specs'
  own conditions (`d20d38d2`, `a5ae5c0d`, `96825665`, `a87a36b4`, `35480b5a`, `794050ec`, `05d514e2`).
  - Two failures were the boat capability above: the file-refusal test in `terminal-files` on both engines found a
    mode-000 file readable. With the capability dropped, that spec passed on both engines, 7 and 7 (`87bd5b5d`,
    `b7295f63`).
  - The third, Chromium's "bulk Stop carries each target's tab precondition", is the open flake FLAKES.md records
    (2026-11-11): the test reads the session list once, before the stopped session settles. Its two variants run five
    times each per engine failed 5 of 10 on Chromium and passed 10 of 10 on WebKit (`08813e6b`, `77fa629d`). The stack
    touches neither stopping sessions nor the page.
- The whole Rust workspace except the desktop app (`cargo nextest run --workspace --exclude farhelm-desktop`, from a
  nextest archive of the same tree): 5,718 passed in the first runs (`501b9925`, `ec108b53`, `e85dd131`). One failure
  was the test macro's compile test, which builds a small crate offline from the development machine's Cargo download
  cache, absent on the boats; it passed locally (`d1cc7508`). The 58 tests the first run left unstarted were all in the
  end-to-end binary, which then ran whole again on four slots: 433 passed, the real Podman environment scenario included
  (`fcbbe040`, `a1a43071`, `1f1a3fa4`).

After the landing fixes, on the tree that merged:

- The same formatting, dprint, changelog and agents-md checks, `cargo clippy -p farhelm-helm --all-targets`, the helm's
  provisioning unit tests, which pin the changed step text (363 passed, `4a3e9338`), and the website build: passed.

Reused: the executor's desktop smoke, desktop crate tests and CentOS provisioning test (`32c60e83`, `47b32a8e`,
`f93695cf`), and its provisioning browser spec (`46cf3b03`, `88bdeade`); they ran on the delivered stack; since then
main gained only plan-queue changes, and the landing fixes changed only text and the explanation of the "Remove the
bundled tmux" step, so their coverage still applies.

Skipped: anything macOS-only (no Mac here, as in the report).
