# Bring up the Mac release test

For an already prepared machine, start with [BRICK-TEST.md](BRICK-TEST.md). It is the fresh-agent entry point for
**`brick test release vX.Y.Z`**. This document prepares the private prerequisites that entry point discovers.

Build a repeatable test of the shipped Mac app coming up on old state and updating the remote supervisors an older
release installed. You are the agent on the **macOS host**: you control disposable Tart VMs from outside them, run
commands in them, and use computer use on the macOS VM's display. The host may run the maintainer's own Farhelm; that
installation, its state and its processes are off-limits. Every Farhelm install or update below happens inside a VM.

The Tart command channel, private bases, guest-to-guest SSH, native app launch and evidence export have now been
exercised on an Apple Silicon host. [OPERATIONS.md](OPERATIONS.md) gives the verified script interface and input
limitations. Keep machine-specific configuration and raw evidence private. Each release still needs its own recipe run;
the bring-up rehearsal cannot establish safety for later published bytes.

Read [RECIPE.md](RECIPE.md), [the historical motivation](../../lore/2026-10-05-release-brick-protection-plan.md), and
the repository's agent instructions. Where the history describes an agent inside the VM, this document's agent on the
host replaces it. Product and implementation contracts live in [SPEC.md](../../SPEC.md) and
[SPEC_impl.md](../../SPEC_impl.md); the release's current publication procedure is [releasing/AGENTS.md](../AGENTS.md).
This work does not change that publication procedure.

## What is already built

The desktop updater accepts `FARHELM_DESKTOP_UPDATE_LATEST=v<stable-version>` at startup. This selects a version before
`https://get.farhelm.io/latest` names it; the real origin, signatures, trusted version comment, installer checksum and
newer-version comparison still apply. Invalid values fail checks rather than falling back. It is captured once for both
automatic and manual checks, kept out of the installer, and removed from the relaunch helper's environment. It works
only when the old app carries the override and the candidate is stable. The override must first ship in a stable
release; older published releases cannot demonstrate it. Record native forwarding as deferred if no carrying release is
available. Removal on restart is checked during the first real staged-candidate run, not the latest-stable dry run.

The candidate must already be signed and published under `https://get.farhelm.io/v<version>/`. A GitHub artifact alone
is insufficient: provisioning and remote updates fetch that version's payloads from the site. Testing before `/latest`
moves can be a gate; testing afterwards is an after-the-fact check. Staging and promotion tooling is outside this work.

## Prepare two private base images

Prepare separate macOS and Linux Tart bases, with **no Farhelm installed or Farhelm state**. The recipe makes new clones
for each pass; never use the bases as the test's working machines. Establish names and prerequisites in this document
when verified. Record the exact image identifiers and guest OS versions privately with the run evidence, and keep
portable recommendations in the repository.

- macOS: pick a clean recent macOS image supported by the host and current Farhelm, for example a suitable Cirrus Labs
  public Tart image. A Cirrus Tahoe base was exercised with macOS 26.6.2. Install the genuine agent CLIs selected with
  the maintainer in the run addendum (the bring-up uses Codex only) and the prerequisites the public installer and
  recipe need. Put the CLIs on the guest user's shell PATH through its startup files. Recommend Homebrew for tmux, in a
  prefix the app probes, at a version both releases accept; verify launches from the app, not only a terminal. Resolve
  authentication with the maintainer. Guest sign-in or an explicitly authorized copy of the selected CLI's
  authentication file can be used; never copy unrelated authentication stores or publish credentials. Verify file
  permissions, CLI login status and a real conversation plus resume. Cloned credentials remain private, and logging out
  a clone may revoke a shared login. Stop the agents before saving the base.
- Linux: recommend an ARM64 Ubuntu 26.04 LTS Tart image, falling back to 24.04 LTS if a suitable 26.04 image is not
  available. The official Cirrus Ubuntu 24.04 image was exercised with Ubuntu 24.04.5 LTS; a suitable official 26.04
  image was not established during bring-up. Give it SSH access from the macOS guest by key, a working systemd user
  manager and the privileges needed to arrange persistent user services. Farhelm provisions its remote supervisor as a
  systemd user service; prove that user manager works before saving the base. No agent credentials are needed on this
  guest: the remote session is a plain command. Recommend a base without tmux, so Farhelm provisions its static tmux
  payload; record whether that path or a guest-provided tmux was exercised.

The macOS base contains real agent credentials. Treat both bases and all clones as private: never push them to a
registry, share them, commit files from them, or put credentials in a report. Resolve how cloned credentials behave with
the maintainer. Respect the host's macOS VM licensing/concurrency constraints; the commonly cited limit of two running
macOS guests must be checked for the actual setup. This recipe needs only one macOS guest at a time.

## Resolve the environment mechanics

Use the verified mechanics in OPERATIONS.md, then resolve any differences on the current host in owned disposable VMs.
The staged-candidate override remains deferred until a carrying stable release is available:

1. Install or confirm Tart on the host and choose the images. Establish clone, start, shutdown and deletion commands.
   Identify owned clones with a fresh run ID, keep an ownership manifest, and never delete a pre-existing VM or base.
2. Establish guest-to-guest networking. Tart's shared network and `tart ip` are likely starting points; prove the Mac
   guest can reach the Linux guest over SSH. Resolve keys, host-key verification and changing clone addresses without
   weakening the host's general SSH configuration.
3. Establish command execution in the Mac guest (Tart's guest-execution support if available, or SSH) and in Linux.
   Probe actual readiness: a guest address alone does not prove SSH, the user manager or the desktop session is ready.
   Use bounded waits with diagnostics, not fixed sleeps as a substitute for readiness.
4. Establish computer use on the macOS guest's GUI, through its VM window or a suitable remote display. Prove clicks,
   keyboard input and screenshots affect the guest, and that the agent cannot accidentally install on the host.
5. Check whether a published stable release carries the override, using the source lookup in RECIPE.md. If none does,
   leave its native demonstration explicitly deferred; this does not stop the ordinary latest-stable dry run. Once a
   carrying stable release R is available, install R in an owned guest, quit the app, and test
   `open -n --env FARHELM_DESKTOP_UPDATE_LATEST=v<R> "$HOME/Applications/Farhelm.app"`. Consult local `open` help and
   require the captured startup announcement and a Check for updates hover naming R. When `/latest` also names R, the
   hover alone does not prove forwarding; without that first-launch log, leave forwarding unverified. An optional
   malformed-value launch must fail the check. This demonstrates forwarding without needing a newer candidate. Prove the
   command channel launches into the guest's logged-in GUI session. An existing app must exit before launching: `-n`
   follows the helper's handling of macOS briefly listing a quit app as running. During the first real staged-candidate
   run, use RECIPE's visible check to prove the restarted candidate's updater returns to the `/latest` probe; the app
   stays on the candidate. A latest-stable dry run cannot distinguish those probes.
6. Establish app-log capture for the original launch and relaunch. [Desktop triage](../../docs/desktop-web-triage.md)
   identifies the desktop process's stderr as the general log source; it does not promise a universal Finder-launch
   file. Pick and verify a capture method that retains the real bundle and updater activation. Establish collection of
   Linux supervisor service logs and guest command output too. The app's helper discards its own standard streams and
   invokes `open -n` without redirecting the new app's output: that relaunch's stderr is outside the agent's launch
   configuration. Discover whether the guest retains it anywhere; unavailable relaunch logs are a reported diagnostic
   coverage gap, not invented evidence or by themselves a failed functional check.
7. Establish candidate checksum/signature verification for manual installs. If minisign is available, fetch the
   candidate's `SHA256SUMS.minisig` and verify against the **previous release's** compiled-in key ring, not main's or
   the candidate's: otherwise a newly trusted key could hide the old app's refusal. Find `RELEASE_KEY_RING` in
   `crates/farhelm-helm/src/provisioning/release_payloads.rs` at that tag (for example,
   `git show v<previous>:crates/farhelm-helm/src/provisioning/release_payloads.rs`). Its base64 public keys are accepted
   by `minisign -V -P <key> -m SHA256SUMS`. Require a signature by a key in that ring and a trusted comment exactly
   `farhelm v<version>`, then check `install.sh` against its signed checksum. Record which checks actually ran. Resolve
   missing tooling with the maintainer; never describe an unsigned/manual path as the old updater's verification.
8. Choose private host-side evidence storage outside repositories and a way to copy evidence out before deleting VMs.
   Settle resource budgets, failure retention and cleanup with the maintainer. Preserve failed clones only by agreement.
9. Establish how to disable automatic updates before the old app's first startup check. The desktop preference is
   `install_updates_automatically` in `desktop-client.json` (SPEC_impl.md, "The desktop app's updater"); resolve its
   guest-local path and initialization safely, or verify temporary guest network isolation until the setting is off.
   Clicking quickly after launch is not a readiness oracle. Restore networking before agents or provisioning need it.
10. Settle the remote continuity command and its oracle. Recommend a plain command appending a timestamp and increasing
    counter to a file on Linux, recording its PID and start time, and reading both process identity and advancing output
    over SSH from outside Farhelm (for example `ps -o pid=,lstart= -p <pid>`). Prove the oracle detects a stopped
    process; a redisplayed buffer or a replacement process must not satisfy continuity.

## Write the host-side scripts

The stdlib Python scripts are now present: `control.py` owns lifecycle/readiness/export, `network.py` configures pinned
guest SSH, `continuity.py` observes one original Linux process, and `verify_release.py` checks published installer
inputs against the previous tag's key ring. OPERATIONS.md documents the interface. Keep machine names, usernames,
credentials and local absolute paths out of source; take local configuration as inputs and keep it private. The
controller refuses mutations of unrecorded clones or replacements with a different hardware identity.

The public coordinator is `brick_test.py`; `machine_profile.py` validates the private interface. Record the frozen
bases' names and MAC identities, Linux account, Tart storage, journal, evidence root, policy, budgets and explicit route
choice in the private profile described in BRICK-TEST. A local control project may install that profile with its own
adapter; neither its path nor credentials belong in Farhelm. Agree the conventional profile's host write during setup.
Use the command-only smoke to prove discovery, readiness, guest SSH, export and owned cleanup before a release run.

## Prove the setup and open a PR

Run both passes of RECIPE.md with the latest stable release as the candidate and the preceding stable release as the
previous version. Use releases from v0.23.0 onwards for the upgrade contract. This dry run exercises the ordinary
`/latest` updater path only; it does not prove a staged stable candidate override or an RC's manual path. Record those
coverage gaps explicitly. A future release needs its own run against its actual published bytes.

Fix scripts and instructions where the dry run exposes wrong assumptions. Keep unresolved mechanics marked open, and ask
the maintainer instead of turning a guess into a claim of success. Read the recipe and this entry point together one
final time: a new host agent must be able to start from this file alone.

Open a draft PR with the scripts, the verified setup details, corrected recipe and a short portable dry-run result.
Follow the repository's Conventional Commit, wording review and validation rules. Keep screenshots and raw logs in
private evidence storage; commit only a redacted result, versions, actual coverage and remaining gaps. This is the
bring-up deliverable, not a release promotion or permission to change the host's live Farhelm.
