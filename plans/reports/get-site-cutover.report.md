### What this was about

Three things decided what code ends up running on a user's machine, and an agent's push could reach all three. The
installer asked GitHub which release was the latest and trusted the checksums on the GitHub release. The helm checked
the payloads it downloads against a signature, but CI made that signature with a key stored as a repository secret. The
Mac app's updater asked GitHub which release was the latest, then downloaded the installer from main and ran it without
checking anything. Agents can push to main and run workflows, so each of these let an agent's push run code on users'
machines. You set up get.farhelm.io outside GitHub and moved signing to your own host. The plan switches Farhelm to that
site and makes the release signature what installed software trusts. You decided in planning:

- one origin with a fixed layout;
- installed software trusts a ring of two keys (a primary and a backup) and accepts a signature by either; until you
  swap in the two keys you generate on your host, the ring holds only today's key, the one whose secret CI held;
- CI never signs;
- the updater runs only a signed installer, which it hands the signed checksums;
- no backfill of old releases;
- breaking the old paths is fine;
- the README, the website and the install docs are left out, and switch later (your M7);
- the TODO entry is narrowed to what remains;
- since each release may replace one of the two keys, an app that misses two such rotations can no longer verify
  anything; it must say it needs reinstalling rather than fail silently.

The work is four stacked PRs, called PR 1 to PR 4 below: #1628, #1629, #1630 and #1631. What landed:

- **Release verification accepts any key in a compiled-in ring (PR 1).** The ring is a list in the helm's source, which
  the Mac app shares. It still holds only today's key; your two keys replace it. The version rule is unchanged: the
  signed line that minisign carries beside the signature (its "trusted comment") must name the exact version.
- **The installer and the helm use get.farhelm.io (PR 2).** The installer reads the latest stable release from `/latest`
  (refusing a prerelease or anything malformed). It takes the release's checksums from the site and its archives through
  the site's redirects. A release the site does not have is refused with a message, and nothing falls back to GitHub.
  The helm's default payload location is now `https://get.farhelm.io/v<version>/`. It still checks the signed checksums,
  as before.
- **CI no longer signs (PR 3).** The release workflow's last job still checks the release's payloads and its asset list,
  but it signs and uploads nothing. Its permission drops to read-only. It now refuses a GitHub release that carries a
  `SHA256SUMS` or `SHA256SUMS.minisig`, since those belong only on the site. The release procedure in `releasing/`
  verifies against get.farhelm.io, and its RC install command fetches the tag's `install.sh` from there.
- **The Mac app verifies an update before installing it (PR 4).** It reads `/latest` from the site. It runs a release's
  installer only after the release's signed checksums verify against the app's ring, with that version's trusted
  comment, and the installer matches its signed checksum. It hands those verified checksums to the installer, which
  checks every archive against them instead of fetching its own.
- **A failed verification shows a warning (PR 4).** It shows for any release that fails verification: a signature by no
  key in the app's ring (what an app two rotations behind sees), a signature for another version, or an installer that
  does not match its signed checksum, which also covers tampering or a corrupted download. Nothing is run, and the
  version readout shows a warning mark in the warning colour, also on the red update marker while an update waits. Its
  hover says this Farhelm can no longer verify its updates and gives
  `curl -fsSL https://get.farhelm.io/install.sh | sh`. A download that fails outright never shows it; a download that
  arrives but does not match its signed checksum (an empty installer included) does.
- **The TODO entry "Lock down the release and update trust chain"** is narrowed to the list you gave in planning.

### Things you should know

- **No release can be signed until you swap in your keys.** Today's key's secret is the CI secret, and CI no longer
  signs; and your publishing tooling, as you described it in planning, refuses to sign binaries that do not carry both
  of your keys. Swapping them in is the first item of the narrowed TODO entry.
- **Running main's `install.sh` fails until you publish a release on the site.** Such runs stop with "no release is
  published on get.farhelm.io yet". That is the breakage you accepted. A helm built from these changes likewise
  downloads payloads only for a version the site has.
- **Your publishing tooling must not put `SHA256SUMS` or `SHA256SUMS.minisig` on the GitHub release.** The workflow's
  last job fails the release if it finds either one there.
- **The workflow job is still named `sign-sums`, though it signs nothing.** cargo-dist's generated release workflow
  calls it by that name, and the file's header says so. The unused `MINISIGN_SECRET_KEY` secret is still in the
  repository until you delete it; the TODO entry lists that.
- **It is now safe to add your keys.** The old signing job checked its secret against the key it read out of the ring's
  source line, and could only read a one-key ring. PR 3 removed that job, and it landed with the rest.
- **The installer now has a permanent interface.** It accepts two variables, `FARHELM_VERSION` and
  `FARHELM_INSTALL_SUMS_FILE` (a checksum file to use instead of fetching one, which may list more entries than it
  needs). Every app built from PR 4 on passes both, so later installers must keep accepting them. SPEC_impl.md records
  this beside the site layout.
- **Whoever controls get.farhelm.io can still affect installed apps.** A wrong `/latest` cannot make an app run anything
  unsigned. It can hold updates back, or name a release that fails verification and so put up the reinstall warning,
  whose command trusts the site over TLS alone. So a compromised site can get apps to ask their users to run its
  installer unverified. SPEC_impl.md now says this, and it is one more reason the server-side hardening in the TODO
  entry matters.
- **The reinstall warning lasts only as long as the running app.** It is held in memory, as the plan allowed no new
  persistent state. It goes away when a later check verifies and installs, or finds nothing newer, or when a newer
  Farhelm is installed some other way (the user ran the suggested command). After a relaunch it comes back with the
  first check that fails the same way. With automatic updates on, that is the check at startup.
- **No Mac app in users' hands runs the old updater.** The updater that asked GitHub and ran main's installer unchecked
  landed after the last release and never shipped, so no installed app needs a transition.
- **A malformed or prerelease answer from `/latest` counts as an ordinary failed check, not a reinstall warning.** Your
  decision named only bad signatures, a wrong version, or a mismatched installer as the trigger.
- **An unreleased changelog fragment from the original updater change said updates come from GitHub unsigned.** That
  sentence is removed, because these changes would ship in the same release and make it false.

### Open questions and possible follow-ups

- **Two website pages will be wrong once this lands.** "Update and uninstall" and "Security model" still say that
  updates come from GitHub and that no release signature is checked. The plan kept the website out of scope (your M7).
  The narrowed TODO entry only mentions switching the install command, so nothing records that these two pages also need
  rewriting. My recommendation: add them to that TODO item, or handle them in the same docs change as the install
  command.

### PRs

- #1628 `refactor: verify releases against a ring of keys`
- #1629 `feat: install and provision from get.farhelm.io`, with its changelog fragment
- #1630 `ci: stop signing releases`
- #1631 `feat: verify the Mac app's updates before installing them`, with its changelog fragment and the TODO narrowing

### Checks

Run ids are the recorder's retained test runs (`scripts/record-test-run.py`):

- Run now, on the final stack after rebasing onto the latest main:
  - the Mac app's updater and readout tests (31 tests, run 507b04c8), and one test of the readout's marks in the web
    build (run 08da18b9), which compiles the same shared readout code without ever showing updater state;
  - `bash scripts/test-install-sh.sh` (448 checks, 0 failed), plus `sh -n` and `shellcheck`;
  - the hosted macOS uninstall suite (`gh workflow run ci.yml -f suite=uninstall-macos`, run 37268131320), passed;
  - workspace `cargo clippy --all-targets` and `cargo clippy -p farhelm --bins`;
  - the UI crate's clippy with and without the desktop feature;
  - `cargo check -p farhelm-desktop`, `scripts/check-desktop-assets.sh`;
  - `cargo fmt --check`, `dprint check`, the changelog format check, and the test-sleep check.
- Reused:
  - the helm's provisioning tests (192 tests, run 7e82721e, on PR 3's content). Neither PR 4 nor what landed on main
    since touches provisioning;
  - `dist generate --check` with the pinned cargo-dist 0.32.0, on PR 3. Nothing since touched the dist configuration.
- Skipped:
  - the full Rust battery: the changes have direct tests in the crates they touch;
  - browser specs: the only UI change is the updater's part of the version readout, which the web build never shows;
  - the desktop smoke: the app's change is in its updater, which runs only in an installed release bundle and which the
    smoke does not drive; the embedded helm's change (its download location) is covered by the provisioning tests;
  - the CentOS provisioning gate: it hands the helm its payloads directly, which skips the download path these changes
    alter, and a real download from get.farhelm.io is impossible until a release is published there;
  - the release workflow: agents do not run it (your M3).

### Review gate

Each PR was reviewed by two fresh-context reviewers, Claude Opus 5.5 and gpt-6-astra, both at high effort. PR 4 had a
second round on its fixes.

- **PR 1:** the helm's own downloads now go through the same verification entry point the app uses. A test checks the
  ring in reverse order.
- **PR 2:**
  - the installer settles which release to install and fetches its checksums before creating anything on disk;
  - "nothing published yet" and "this release is not published" give separate messages;
  - a multi-line `/latest` is refused;
  - spec passages that still said the Mac app updated from get.farhelm.io (true only after PR 4) were corrected.
- **PR 3:** leftover comments and docs that still said CI signs were corrected, and the job now prints the installer's
  checksum with the payloads'.
- **PR 4:**
  - an empty downloaded installer now triggers the reinstall warning like any other mismatch;
  - the warning shows on the update marker while an update waits, with a test that would catch it going missing;
  - the TLS-trust and "lasting" wording in the specs was corrected;
  - the stale changelog sentence was removed;
  - the warning clears once a newer Farhelm is installed from a terminal;
  - a release signed by a key outside the app's ring is tested;
  - a failed download no longer claims an installer ran.

Every finding was fixed except one, declined as out of scope: the two website pages raised above. The second round's
findings on PR 4 were small (test coverage and wording) and are all fixed.

### Landing

Landed on 2026-10-05 (UTC) as four squash commits on main, in stack order: #1628 (verification against a ring of keys),
#1629 (installer and helm downloads from get.farhelm.io), #1630 (CI stops signing) and #1631 (the Mac app verifies an
update before installing it). Nothing else reached main while they merged. Since #1629 is on main, running main's
`install.sh` (the README's install command) fails with "no release is published on get.farhelm.io yet" until you publish
a release there, as the report says and as you accepted in planning.

#### What else was on main

Nothing that could interact. The stack was built on main right after the previous plan landed (the one that matches an
agent's retried create by its request), and between then and the landing main gained only the planning queue's own
bookkeeping. No other plan landed alongside it. One merge waited about two minutes because GitHub took that long to work
out whether #1630 could merge; nothing changed in between.

A separate reviewer that had not seen the work checked the stack before anything merged. It found nothing broken, and
confirmed:

- Every caller of the changed verification code and of the helm's default download location is updated inside the PRs.
- Every check of a signature goes through the ring, and a signature still has to name the exact version in its signed
  comment, whichever key made it. No code checks against a single key any more.
- None of the test and capture scripts outside the PRs (CentOS provisioning, the desktop smoke, the browser tests, the
  README image, the demo video, the docs screenshots) uses the default download location, GitHub release URLs or a
  checksum file on the GitHub release.
- The setting the installer tests use to point the installer at a stand-in site now takes the whole site rather than one
  release's folder; only the two installer test scripts use it, and both are updated.
- The release workflow stays consistent with the configuration it is generated from: the only generated change is the
  signing job's permission dropping to read-only, and no other workflow refers to signing or its secret.

#### Out-of-date text the review found

Beyond the two website pages the report already raises ("Update and uninstall" and "Security model"), the review found
more text that still describes GitHub as where Farhelm comes from. None of it was changed during the landing, and
nothing tracks it apart from this note. It would fit the same docs change that switches the install command:

- The website's "Add a remote host" page says the Mac downloads Farhelm for a host from GitHub and needs to reach it.
- `docs/install_uninstall.md` says the app checks GitHub for updates, and shows installing version 0.2.1 with
  `FARHELM_VERSION`, which main's installer now refuses because old releases are not on get.farhelm.io.
- The helm's `--payload-dir` help text, which users see in `--help`, says it is for tests that would rather not reach
  GitHub.
- A comment in the workspace's `Cargo.toml` says a release build downloads its payloads from the GitHub release.
- A few comments touched by the stack are mis-indented or left over-long (in `scripts/install.sh`,
  `.github/dist-build-setup.yml`, `dist-workspace.toml`, `scripts/check-static-elf.sh` and the helm's provisioning
  assets). The formatters pass on them; they do not check comment layout.

#### Checks

- Reused: the report's checks, which ran on the final stack. The code on main after the last merge is identical to that
  final stack, and the only other commits since the stack was based are the planning queue's bookkeeping.
- Skipped: running anything again during the landing, for the same reason.

Nothing in the report above was made untrue by the landing.
