# Bring up the Mac release test

For an already prepared machine, start with [BRICK-TEST.md](BRICK-TEST.md). It is the fresh-agent entry point for
**`brick test release vX.Y.Z`**. This document prepares the private prerequisites that entry point discovers.

Build a repeatable test of the shipped Mac app coming up on old state and updating the remote supervisors an older
release installed. The setup agent on the **macOS host** prepares private Tart fixtures and native permissions. During
actual tests, an independent Codex process inside the Mac guest drives computer use directly; the host coordinates
lifecycle and evidence. The host may run the maintainer's own Farhelm; that installation, its state and its processes
are off-limits. Every Farhelm install or update below happens inside a VM.

The Tart command channel, private bases, guest-to-guest SSH, native app launch and evidence export have now been
exercised on an Apple Silicon host. [OPERATIONS.md](OPERATIONS.md) gives the verified script interface and input
limitations. Keep machine-specific configuration and raw evidence private. Each release still needs its own recipe run;
the bring-up rehearsal cannot establish safety for later published bytes.

Read [RECIPE.md](RECIPE.md), [the historical motivation](../../lore/2026-10-05-release-brick-protection-plan.md), and
the repository's agent instructions. Product and implementation contracts live in [SPEC.md](../../SPEC.md) and
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
  authentication file can be used in disposable validation clones. Install the official desktop bundle containing the
  native CUA runtime, and Python 3.13+ for the tester. Record explicit guest runtime paths in the format-two profile.
  Through the guest's normal Settings UI, grant Accessibility and Screen Recording to the official helper. Host computer
  use is allowed for this setup only. Resolve administrator or interactive sign-in steps before freezing. Never copy
  unrelated authentication stores or publish credentials. Stop agents before saving the base.
- Linux: recommend an ARM64 Ubuntu 26.04 LTS Tart image, falling back to 24.04 LTS if a suitable 26.04 image is not
  available. The official Cirrus Ubuntu 24.04 image was exercised with Ubuntu 24.04.5 LTS; a suitable official 26.04
  image was not established during bring-up. Give it SSH access from the macOS guest by key, a working systemd user
  manager and the privileges needed to arrange persistent user services. Farhelm provisions its remote supervisor as a
  systemd user service; prove that user manager works before saving the base. Install genuine Codex on the SSH
  login-shell PATH: P0 requires a successful remote agent. Recommend a base without tmux, so Farhelm provisions its
  static tmux payload; record whether that path or a guest-provided tmux was exercised.

Both reusable bases must contain **no agent authentication**. Remove inherited authentication files locally during
setup; logging out a clone may revoke a shared login. Obtain explicit standing credential-copy consent for this
machine's disposable Mac and Linux test clones (or the exact run scope). Inject that named file via stdin with mode 0600
before testing. Treat fixtures and clones as private: never publish them or raw evidence. Respect the host's macOS VM
licensing/concurrency constraints; the commonly cited limit of two running macOS guests must be checked for the actual
setup. This recipe needs only one macOS guest at a time.

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
4. Establish native computer use inside the Mac guest through the official provider with Codex `--yolo`. In a disposable
   authenticated validation clone, prove actual returned screenshots, clicks and exact nonsecret text entry. Prove the
   tester survives Farhelm quit/reopen. Repeat native use and a genuine product agent in a fresh clone of the
   credential-free prepared image, without host UI input or permission grants. An inherited signed-in desktop app does
   not establish that clean images work with only the authorized CLI authentication injection.
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
guest SSH, `fixture_probe.py` performs read-only paired cleanliness inspection, `continuity.py` observes one original
Linux process, and `verify_release.py` checks published installer inputs against the previous tag's key ring.
OPERATIONS.md documents the interface. Keep machine names, usernames, credentials and local absolute paths out of
source; take local configuration as inputs and keep it private. The controller refuses mutations of unrecorded clones or
replacements with a different hardware identity.

The public coordinator is `brick_test.py`; `machine_profile.py` validates the private interface. Record the frozen
bases' names and MAC identities, Linux account, Tart storage, journal, evidence root, policy, budgets and explicit route
choice in the private profile described in BRICK-TEST. A local control project may install that profile with its own
adapter; neither its path nor credentials belong in Farhelm. Agree the conventional profile's host write during setup.
Use the command-only smoke to prove discovery, readiness, guest SSH, export and owned cleanup before a release run.

## Certify the native fixture

Save private mode-0600 consent and fixture records referenced by `guest_tester` in BRICK-TEST's format-two profile. The
consent record uses `format: 1`, `approved: true`, `scope: "disposable-test-clones"`, `destinations: ["mac", "linux"]`
and `auth_source` matching the profile's normalized path. Record the maintainer's actual response and scope privately;
this is authorization metadata, never an authentication file.

Full/P1-only runs also need separately recorded `feedback_send_approved: true` in that authorization record, covering
only RECIPE's fixed message through the disposable candidate app to `farhelm.io` and its private inbox with blank
contact. Obtain it during setup when that scope is wanted. P0-only validation needs no feedback approval and sends none.

The fixture record uses `format: 1`, `bases` containing the exact `mac` and `linux` name/MAC bindings, and `settings`
matching the profile's guest runtime/model configuration. Its `checks` object must attest all six obligations:
`native-screenshot-click-type`, `native-after-reopen`, `clone-permission-carryover`, `auth-not-in-bases`,
`farhelm-clean` and `linux-codex-ready`. Retain actual validation evidence privately and reference it in the record.
These are operator attestations; never fill them from a successful install command alone. A new base identity or changed
runtime requires new validation. Each actual test also probes its own clone before installing Farhelm.

Save credential-free builders as new stopped bases, preserving old bases and the old profile for historical cleanup.
Verify no Farhelm installation/state or Codex authentication remains. Update the conventional profile only after clean
disposable-clone validation succeeds. Do not refresh authentication in a reusable builder to get past a setup failure. A
permission block in a release test fails that run; fix setup and start a new run instead of manually rescuing it.

## Prove the setup and open a PR

Run RECIPE.md's five P0 checks on its three independent pairs with the latest stable release as the candidate and the
preceding stable release as the previous version. Use releases from v0.23.0 onwards for the upgrade contract. This dry
run exercises the ordinary `/latest` updater path only; it does not prove a staged stable candidate override or an RC's
manual path. Record those coverage gaps explicitly. A future release needs its own run against its actual published
bytes.

Fix scripts and instructions where the dry run exposes wrong assumptions. Keep unresolved mechanics marked open, and ask
the maintainer instead of turning a guess into a claim of success. Read the recipe and this entry point together one
final time: a new host agent must be able to start from this file alone.

Open a draft PR with the portable scripts and corrected recipe. Follow the repository's Conventional Commit, wording
review and validation rules. Keep machine bindings, setup and dry-run reports, runtime versions, screenshots and raw
logs in private evidence storage. Report actual coverage and remaining gaps to the maintainer; successful
fixture-validation agent replies do not establish supervisor-upgrade coverage, and a later pass does not erase an
earlier failed attempt. This is the bring-up deliverable, not a release promotion or permission to change the host's
live Farhelm.
