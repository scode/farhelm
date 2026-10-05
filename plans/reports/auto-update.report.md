### What this was about

The Mac app never updated itself: a user only got a new release by remembering to run the installer again. You asked for
Chrome-style updates: check for a new stable release at startup and about once a day, install it in the background with
the same installer a user runs by hand, show that an update is ready, and offer Restart to update, with one setting to
turn the automatic part off (on by default). You also asked for the local machine's Update item and the `?` menu to
check on demand, and for the spec's "automatic updates are out of scope" and notification non-goals to go.

What landed for the user, in the installed Mac app only (`~/Applications/Farhelm.app`, a release build):

- Shortly after it starts and then once a day of wall-clock time, the app asks GitHub which stable release is the
  latest. When that is newer than the installed version, it downloads the current installer script from the main branch
  (the one the README pipes to `sh`, not a copy from the release) and runs it in the background, pinned to that release.
  Nothing on screen changes while it works.
- Once a newer version is installed, by the app or by a terminal install (noticed within a minute), the version number
  at the top of the sidebar turns red with an up-arrow. Its hover names the waiting version. Selecting it opens a menu:
  **restart to update** (quits and reopens Farhelm on the new version; sessions keep running) and **what's new** (the
  GitHub releases page).
- **check for updates** in the `?` menu and **update** in the local host's row menu check right away and install if
  there is a newer release. The version number's hover says how such a check went: checking, installing, up to date, or
  failed with a reason.
- The settings dialog gains **install updates automatically**, a setting of this app installation (kept in the app's own
  state file, not the helm). Off stops the automatic checks; the on-demand checks and the red marker still work.
- The web UI, builds from main, Linux helms and remote hosts behave as before.

### Things you should know

- **Nothing Mac-specific could be run here**: this run had only a Linux machine, as you accepted when planning. The
  checks owed on a real Mac are below. Until they pass, treat the real install, the relaunch and the Mac's permission
  behavior as unverified.
- **The on-demand checks landed with the updater** rather than in a PR of their own, so that no PR added code nothing
  called.
- **The help menu fix that landed on main during this run (#1598) shaped the menus.** That fix moved the `?` menu's
  panel out of the sidebar's sticky top bar, because WebKit clips it there. This plan's shared menu component first put
  the panels back inside the bar, which would have re-broken the help menu on the Mac and hidden the new update menu
  too. It was redesigned on rebase: both menus keep their panels outside the bar, the way #1598 requires, so the update
  menu depends on the same fix (its TODO entry was dropped by #1611 while this ran).
- **The `?` menu's CSS class names and data attributes were renamed** from `help-menu` to `bar-menu`, the name of the
  menu component both sidebar menus now share, with the two browser specs that select them updated. Nothing outside the
  UI and those specs used them.
- **#1598's new browser test was flaky; this stack fixes it.** The test closed the help menu with Escape right after the
  page loaded. When the session's terminal finished connecting just after the menu opened, it took the focus, and the
  Escape went to the terminal. It failed in 2 of 4 runs here, once on each engine. The test now waits for the terminal
  to take focus first, and passed 4 of 4 runs on both engines afterwards. The failure was not reproduced on unmodified
  main; main's test runs the same steps against the same terminal behavior. FLAKES.md has the entry.
- **No signature check, by your decision.** The updater trusts GitHub over TLS, like the hand-run installer. Because it
  runs main's installer script, a change to `install.sh` on main reaches every installed Mac app at its next check,
  whether or not a release ships; that is the same trust a first install gives main, and the TODO entry about locking
  down the release and update chain, which stays, covers tightening it. SPEC_impl.md has a rule that a signing-key
  rotation must ship before any feature that checks the next release's signature with the key it carries; it now says
  this updater checks no signature, so the rule does not apply to it yet, and still applies to any future updater that
  does.
- **A pinned version gets replaced while automatic updates are on.** If you install an older release with
  `FARHELM_VERSION`, the next automatic check, including the one at startup, moves you to the latest stable release. A
  pinned prerelease newer than the latest stable release is kept. `docs/install_uninstall.md` says this.
- **The installer survives a quit mid-install.** It runs in its own process group and writes its output to a file. A
  pipe into the app would break when the app quits, and `install.sh` would then die without its cleanup and leave its
  lock behind, refusing every later install. The app copies the output into its log after the installer exits.
- **Restart to update runs `open -n`, after the old process has exited.** macOS can keep listing an app as running for a
  moment after its process exits, and a plain `open` would then only try to bring that old instance forward. This is
  inferred, not observed; see the checks owed.
- **Two small keyboard gaps in the update menu, accepted for now.** If the red version number turns grey while focus is
  inside the update menu, focus drops to the page instead of returning to a control (the menu closes). Tab from the red
  version number while its menu is open goes to the settings gear, because the menu's panel comes later in the page.
- **The update page on the website has no screenshots.** The screenshot pipeline captures the web UI, which has none of
  these controls, so the page explains them in words. The docs Overview page does not mention updates and was left
  alone.

### Checks owed on a real Mac

1. Install the previous stable release with the installer (`FARHELM_VERSION=<previous>`), then open Farhelm with
   automatic updates on. Expected: within a minute or two the version number turns red with an up-arrow, and its hover
   names the latest release. The app's log shows lines starting `updater:`, including the installer's output.
2. Select the red version number, then **restart to update**. Expected: Farhelm quits and reopens on the new version
   within a few seconds, sessions are still running, and the version number is grey again. If the app quits and does not
   come back, the relaunch helper gave up (it waits up to about a minute) or `open -n` misbehaved.
3. Watch for a macOS prompt while the app installs over its own bundle (for example "Farhelm wants to modify apps" under
   App Management), and check that the new version opens without a Gatekeeper warning. The installer downloads with
   curl, which sets no quarantine attribute, so no warning is expected.
4. With no update waiting, choose **check for updates** in the `?` menu, and then **update** in the local host's menu.
   Expected: the hover says checking, then "Farhelm is up to date (…)". Offline, it should say the check failed.
5. Reinstall the previous release with the installer, untick **install updates automatically**, then quit and reopen.
   Expected: no automatic check (no `updater: installing` line), while **check for updates** still installs the latest.
   Tick it again and the box stays ticked after reopening.
6. Open the update menu and the `?` menu and confirm each shows in full. Keyboard: ArrowDown on the red version number
   opens the update menu with focus on its first item, and Escape returns focus to it.

- **Main moved twice during the run.** Both rebases were checked against what landed: the help menu fix (above) and the
  CLI permission prompts stack (a per-host "run farhelm commands from this host without asking" setting and approval
  cards). The latter touched the same files only in neighbouring text: the host settings dialog it extends is not the
  app settings dialog this stack adds a checkbox to, and its spec changes are in other paragraphs. The only conflict was
  a module list.

### Open questions and possible follow-ups

- A failed automatic check waits a full day before the next try. On a laptop, the due check tends to run right after
  waking, often before Wi-Fi is back, so one failure can cost a day. This matches your "retried at the next check".
  Options: leave it, or retry a failed automatic check a few times a few minutes apart. Recommendation: add a TODO entry
  for the short retry; it is small and keeps "about daily" true on laptops.
- When a session's terminal finishes connecting, it takes the keyboard focus, even from a sidebar menu the user just
  opened; the menu then stops responding to Escape and arrow keys until clicked again. It is rare in practice (it needs
  a menu opened in the second or two while a session connects) and is what made #1598's test flaky. Options: leave it,
  or have the terminal not take focus while a sidebar menu is open. Recommendation: leave it unless you have seen it in
  use.

### The PRs

- #1599 — docs: specify automatic updates for the Mac app (SPEC.md and SPEC_impl.md).
- #1600 — feat: keep the Mac app up to date automatically (the updater, red marker, on-demand checks, install doc,
  changelog fragment).
- #1604 — feat: restart the Mac app into an installed update (shared bar menu, update menu, relaunch, settings checkbox,
  the flaky test fix and its FLAKES.md entry).
- #1613 — docs: document the Mac app's automatic updates (website pages, README, removes the "Auto-update the Mac app"
  TODO entry).

### Checks run, reused and skipped

Run on the final stack (on main `a01d53a2`), all passing:

- Rust unit tests through the recorder, nextest, desktop feature, selection
  `test(/updater::/) | test(/desktop::state::/) | test(/update_menu_item/) | test(/app_bar::/) | test(/menu_panel::/)`:
  run `9a96c1fd`, 49 passed (on main `84c10b3b`); then every desktop-feature UI unit test on the final base, run
  `6381e6f2`, 507 passed.
- Browser specs through the recorder, Chromium and WebKit: `feedback.spec.ts` (including #1598's structure test),
  `tooltip.spec.ts`, `tooltip-coverage.spec.ts`: run `497884e7`, 34 passed (and `4bba7c49` before the last rebase). The
  two flaky failures described above are retained as runs `4c88e5c5` and `a959a160`; all attempts are listed in
  FLAKES.md.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D
  warnings`,
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`, and
  `cargo check -p
  farhelm-ui --features web --target wasm32-unknown-unknown`.
- `scripts/check-desktop-assets.sh` (21 assets, requested set equals bundled set),
  `python -B
  scripts/check-test-sleeps.py` (0 unannotated), `python3 releasing/check-changelog.py format`,
  `dprint check` on changed Markdown, and the website build (`bun run build`, all internal links valid).

Skipped: the full Rust battery, other browser specs, the desktop smoke, the installer and uninstall suites, and the
CentOS gate. The change is confined to the desktop UI crate (updater, app bar, settings, host row), plus two helpers in
the helm crate that were only made public, unchanged: the version comparison the host list already uses to call a build
newer, and the test for a development build. The updater stays inactive outside the installed bundle, which also keeps
it inactive in the desktop smoke. Nothing in the installer, the helm's other code or the supervisor changed.

### Review gate

Each PR got the two fresh-context reviewers you asked for, Claude Opus 5.5 at high effort and gpt-6-astra at high
effort. The third PR got a second round after its menus were redesigned on rebase. Every finding was addressed except
these, each a recorded decision:

- No short retry after a failed automatic check: the spec says "retried at the next check"; it is a follow-up question
  above.
- No second check of which version is installed between downloading the installer and starting it: a terminal install
  landing in those few seconds is still kept apart by the installer's own lock. The app does re-check after asking
  GitHub which release is latest, because that request is the slow step.
- The two update-menu keyboard gaps above.

### Landing

Landed on 2026-10-05 (UTC) as four squash commits on main, in stack order: #1599 (the spec), #1600 (the updater and the
red version marker), #1604 (restart to update, the shared sidebar menu and the settings checkbox) and #1613 (the
website, the README's install section and the TODO entry's removal).

#### What else was on main

Between the commit the stack was built on and the landing, one change reached main besides the planning queue's own
files: the fix that makes remote uninstall refuse a host whose program directory is a symbolic link (#1614). It edits
SPEC.md, SPEC_impl.md and TODO.md, and the helm's remote uninstall code. A separate reviewer that had not seen the work
read both changes against each other before anything merged and found no interaction. They edit different paragraphs of
SPEC.md (remote uninstall there; installation and updates, and the non-goals, here) and different sections of
SPEC_impl.md; each removes a different TODO entry, and neither refers to the other's; and the two version helpers this
stack makes public in the helm are ones the uninstall fix does not touch. The rebase applied without conflict. #1614 was
another plan's landing and finished before this one began; nothing else reached main while these four merged.

The same reviewer also searched the rest of the repository for anything the stack's renaming of the help menu's CSS
classes and data attributes (from `help-menu` to `bar-menu`) would break, including the docs screenshot, README image
and demo video harnesses, and found nothing: every use was updated inside #1604, and the help button's own class, which
other tests and code use, was kept. It confirmed the stack adds no files to the desktop app's bundled assets, and that
it leaves the docs Overview page and the README's introduction alone.

#### Checks

- Run now, on the final stack after rebasing onto the latest main: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features web --target wasm32-unknown-unknown`, `dprint check`,
  `python -B scripts/check-test-sleeps.py` (no unannotated delays) and `python3 releasing/check-changelog.py format`.
  All clean.
- Reused: the test runs in the report's "Checks run, reused and skipped" section, which ran on the earlier base
  (`a01d53a2`). The only code that reached main since then is the uninstall fix in the helm's provisioning code, which
  this stack does not use, and the rebase onto it changed none of the stack's code.
- Skipped: running Rust or browser tests again, for the same reason; and every check on a real Mac, which still needs a
  Mac (the report's "Checks owed on a real Mac" stand as written).

Apart from the stack's base, which moved past `a01d53a2` to include #1614, nothing in the report above was made untrue
by the landing.
