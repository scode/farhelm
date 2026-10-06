# Test a Mac release

Run this as an agent on the **macOS host**, controlling disposable Tart guests. It requires the two prepared base
images, command and GUI access to the guests, readiness checks, a way to launch the app and capture logs, and lifecycle
scripts established by [BRING-UP.md](BRING-UP.md). If any is missing, do that bring-up first. This document's VM
mechanics are unverified until that work is done; it is an intended procedure, not a record of a successful Mac run.

Each pass gets a fresh macOS clone and a fresh Linux clone. Install and update Farhelm only in the guests. Leave the
host's live installation and prepared bases untouched. The bases and clones contain real credentials and must never be
published. Store private evidence on the host outside repositories; redact any report you share.

## Inputs and readiness

Record the candidate tag (`v<version>`), the previous stable tag, the tag's commit and the SHA-256 of its `SHA256SUMS`,
and [the per-release addendum](#per-release-addendum-template). Before promotion, `/latest` normally gives the previous
stable version. After promotion it gives the candidate: find the preceding stable in the release history, excluding RC
and dev releases. `CHANGELOG.md` has stable `## vX.Y.Z` sections; cross-check the selected tags against published
releases and the site, since notes alone do not prove the bytes are available. Verify the ordering; do not accidentally
test candidate-to-itself. Choose a previous release from v0.23.0 onwards and require the candidate to be newer by SemVer
precedence.

Require the candidate's `install.sh`, `SHA256SUMS`, signature and needed payloads at
`https://get.farhelm.io/v<version>/`. The same applies to the previous release. A candidate already named by `/latest`
can be tested, but that run cannot prevent its initial exposure to users. This recipe does not alter publishing or
promotion. Capture `/latest` at the time the update path is selected, not only when the run starts.

Read the release's changelog fragments and protocol/schema changes to draft the addendum. The baseline below tests one
previous stable; SPEC.md requires clean upgrades from **every** stable release from v0.23.0 onward until upgrade epochs
exist. A passing previous-stable pair does not prove that entire range. Name older-version risks and extra pairs in the
addendum, and report the versions actually covered.

Before each pass, use the bring-up scripts to clone and start both guests, establish Mac desktop and SSH readiness, and
verify the Linux systemd user manager. Record guest OS/image versions privately. Confirm no Farhelm binaries or state
exist in either fresh clone. A broken fixture is “could not run,” not a product pass.

## Pass 1: fresh install

1. Inside the Mac guest, fetch the candidate's public version-path installer and install with
   `FARHELM_VERSION=v<version>`. For example, after replacing the placeholder:
   `curl -fsSL https://get.farhelm.io/v<version>/install.sh | FARHELM_VERSION=v<version> sh`. This is the ordinary
   installer path, not a source build. Save output and check the installed CLI with `farhelm --version` (resolve the
   installed CLI path if the guest's shell has not refreshed PATH).
2. Open the installed `$HOME/Applications/Farhelm.app` in the guest using the verified launch/capture method. Confirm
   the app version, usable session list and connected local host. Save a screenshot and the app log.
3. Add the Linux guest as an SSH host through Farhelm. Confirm the candidate's helm provisions it from scratch and that
   the host becomes connected at the candidate supervisor version. Record the visible result and service evidence.
4. Start a real authenticated Claude session and a real authenticated Codex session on the local host. Give each a small
   conversation and require a meaningful reply, terminal input/output and a captured conversation. On Linux, start a
   plain command session and prove its terminal responds. Mock agents or a CLI `--version` alone do not satisfy this.
5. Run applicable addendum checks. Capture a screenshot per GUI check and command output for command checks. Export
   evidence, then stop/delete only this pass's owned clones with the bring-up scripts.

## Pass 2: upgrade on existing state

1. Start fresh clones again and install the **previous stable** in the Mac guest with its version-path installer and
   `FARHELM_VERSION`. Use the verified bring-up mechanism to disable automatic updates before its first startup check;
   an immediate update would destroy the old-state fixture. Launch it and verify its version and connected local host.
   If that mechanism isolates the guest's network, restore it first; then provision the Linux guest through the old app
   and establish the remote supervisor's old version before proceeding.
2. Create realistic state: local Claude and Codex sessions with conversations, a saved template, and a changed setting
   with an observable value. Save session and conversation identities and screenshots. Confirm automatic app updates
   remain off while seeding state, so `/latest` cannot update the old app before the intended action. Check it is still
   the old version with no candidate already installed. Choose settings/templates supported by that previous release.
3. Start a long-running plain command on Linux that emits an increasing counter and retains evidence of its original
   process identity/start time. Establish that it is alive and advancing before updating. Read its advancing output and
   process identity over SSH from outside Farhelm; a redisplayed terminal buffer is not proof of continued execution.
   Avoid a command that restarts itself and masks interruption. Settle the exact portable command and process oracle
   during bring-up.
4. Select the update path using the next section, perform the install, wait for the old app's readout to show **Restart
   to update**, and use that action. Do not manually launch the new binary to bypass a failed relaunch. Capture the app
   log and readout. Record whether the trigger was automatic or manual for an in-app check; with automatic updates off,
   explicitly ask the app to check.
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

The supervisor is disposable; its sessions live in tmux and must survive app or supervisor restarts. For lifecycle
details, read [SPEC.md](../../SPEC.md), “Durability and resume” and “Lifecycle operations.” Do not reboot the guests
during a continuity check: a host reboot legitimately ends its sessions.

## Select the upgrade path

Apply the first matching rule and record it in the report:

1. `/latest` names the candidate: use the ordinary in-app update. This is an after-the-fact check, including the
   bring-up dry run. It proves neither the override nor the manual candidate path.
2. `/latest` does not name the candidate, the candidate is **stable**, and the previous release contains the override:
   quit the old app, then launch that installed bundle with
   `open --env FARHELM_DESKTOP_UPDATE_LATEST=v<version> "$HOME/Applications/Farhelm.app"` **inside the Mac guest**. This
   command is a recommendation pending bring-up verification of the guest's `open` syntax, environment forwarding and
   log capture. Require the startup override announcement, then use the in-app update. Every existing verification check
   still applies. After Restart to update, ask the new app to Check for updates and require the hover to name the
   version `/latest` currently names, rather than the candidate. With an unpromoted candidate these differ, so this
   observes that the override is gone; save the endpoint observation and hover together. If `/latest` moved to the
   candidate meanwhile, the results no longer distinguish the probes: report that coverage gap, not proof that the
   override was removed. If support in the old release is unknown, establish it from its source before choosing this
   path; for example, `git grep FARHELM_DESKTOP_UPDATE_LATEST v<previous> -- crates/farhelm-ui/src/desktop/updater.rs`
   shows whether the source carries the variable. Do not infer support from a successful ordinary check.
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
```

## Evidence and report

Collect one screenshot per GUI check, the first launch's app log and any relaunch logs the bring-up capture can actually
retain, guest `farhelm --version` output, remote supervisor version and command continuity evidence, and diagnostics for
failures. Copy them to private host storage before teardown. An unavailable relaunch log is a diagnostic coverage gap,
not by itself a failed functional check. Screenshots and logs can reveal account names, credentials, conversations and
addresses; do not commit or publish raw evidence. Shared reports use portable evidence labels and redacted copies, not
hostnames, usernames or private paths.

```text
Verdict: pass | fail | could not run
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

“Pass” requires every required check to run and pass. A product failure makes the verdict fail; a missing test
environment with no demonstrated product failure makes it could not run. Never treat a skipped check or a clean process
exit as a pass. This result covers the actual versions, artifacts and paths exercised, not zero risk for every supported
installation.
