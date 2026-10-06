# Operate the private Tart test pair

For the standard **`brick test release vX.Y.Z`** request, start with [BRICK-TEST.md](BRICK-TEST.md). Its coordinator
discovers private machine bindings and supplies the handles used below. These lower-level helpers also remain useful for
bring-up and reconciliation; they do not by themselves produce a release verdict.

These stdlib Python helpers run on the macOS host and operate only the fresh clones recorded in one private manifest.
GUI checks and the release verdict remain the operator's responsibility. Use Python 3.11 or newer, installed Tart with
guest execution support, two stopped private bases and an append-only external journal. Bring-up exercised Tart 2.32.1,
Python 3.14 on macOS and Python 3.12 inside Linux. Install guest prerequisites inside the guests; host installation or
network changes require the maintainer's authorization.

## Prepare the bases

Use a recent compatible Cirrus macOS image and ARM64 Ubuntu. Bring-up exercised Tahoe 26.6.2 and Ubuntu 24.04.5 LTS.
Record the exact OCI digests and provisioning privately. The Mac base needs a logged-in desktop, Homebrew tmux in a
prefix Farhelm probes, genuine authenticated selected agent CLIs on its login-shell PATH, Python and minisign for
verification. The Linux base needs key-based SSH, a working systemd user manager, lingering and guest privileges for
persistent user services. Bring-up used Homebrew tmux 3.7c locally and removed distro tmux from Linux so Farhelm
installed its static 3.7c payload. Removing distro tmux can remove packages depending on it; inspect the package
manager's plan first.

Prove genuine CLI conversation and resume before freezing the Mac base. Authentication copies require explicit consent
for that credential and destination, restrictive guest file permissions, and a private base/clone policy. Never collect
an authentication directory as evidence. Prove Linux's user manager both through the command channel and an SSH login.
Stop the bases with guest shutdown; verify stopped Tart state and absence of Farhelm binaries/state. Once frozen, create
new clones for all tests. Do not start or provision the bases again to prepare a pass.

## Create and supervise a pass

Run from the source checkout. Set `test_run` to a **new** private directory outside repositories, `journal` to the
operator's external log, and `mac_base`/`linux_base` to the stopped prepared VM names. Keep these inputs out of commits.
Store large VM files in Tart's usual home and measure volume free space before/after cloning: APFS clone sizes overlap.

Define the command helper:

```sh
control() {
  python3 -B releasing/mac-vm-test/control.py --run-dir "$test_run" --journal "$journal" "$@"
}
```

- `control init` creates the directory with mode 0700 and a fresh UUID manifest. Existing directories are refused.
- `control clone mac "$mac_base"` and `control clone linux "$linux_base"` create UUID-named clones with automatic
  pruning disabled. Names and MAC addresses are recorded; collisions and interrupted clone records are refused.
- In a separately supervised foreground terminal/tool session, run `control run mac`. In another, run
  `control run linux --headless`. Keep both sessions alive. The controller records their PIDs and observed running state
  before waiting for their eventual exit. Do not background an untracked process and assume it survived. Each spawned
  handle is retained before startup saves, state probes or result logging; failures in those steps keep the foreground
  owner waiting so another invocation can inspect or cleanly shut down the owned guest.
- `control ready mac` and `control ready linux` use bounded guest probes. An address alone is insufficient. Mac
  readiness requires a virtualized guest and logged-in console user; Linux requires SSH and the systemd user context.
- `control status mac`, `control status linux` and `control ip linux` inspect the recorded guests. Reinspect after any
  interrupted operation before deciding whether to retry.

Read each result before the next mutation. The manifest serializes operations but grants no authority to arbitrary guest
commands: the operator still owns their scope. Journal failure prevents mutation; failed commands remain in the record.
A replaced VM with the same name but a different MAC is refused. This is an operational ownership fence, not protection
against a malicious editor forging both the manifest and VM configuration.

## Guest SSH

Run:

```sh
python3 -B releasing/mac-vm-test/network.py --run-dir "$test_run" --journal "$journal" --linux-user "$linux_user"
```

The helper obtains the Linux host public key through the direct guest channel, creates/reuses a private key only in the
Mac guest, authorizes its public key inside Linux and writes a run-specific SSH alias with strict host-key verification.
It discovers the current clone address and verifies SSH plus the login's systemd user manager. No host SSH configuration
is changed. Read the printed alias or manifest `ssh.alias`; add that destination through Farhelm's ordinary GUI.

On the exercised host, default shared-network peer isolation blocked direct guest-to-guest traffic. The explicitly
selected `--route-via-gateway` option added reciprocal guest-only /32 routes through their existing shared gateway. It
checks that host IPv4 forwarding is already enabled and refuses if it is not; it never enables host forwarding. Derive
routes from the current pair, not saved DHCP addresses. If the current host lacks a usable approved route, resolve that
with the maintainer before continuing. Do not enable bridging or forwarding as an implicit fallback.

Direct Linux `tart exec` does not necessarily inherit an SSH login's runtime environment. For service diagnostics, run
over guest SSH, or set guest `XDG_RUNTIME_DIR=/run/user/$(id -u)` and
`DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus` for that command. A missing bus in the direct channel is not
proof that the provisioned service failed.

## Guest commands and app launch

- `control exec mac -- /bin/sh -c '<guest command>'` runs a bounded command in the owned guest. Arguments are journaled;
  keep secrets out of them and save private output inside a named guest evidence directory.
- For stdin, put the option **before the role**: `control exec --stdin mac -- /opt/homebrew/bin/python3 - < script.py`.
  `exec` treats everything after the role as the guest command, so later controller options become command arguments.
- The default timeout is 300 seconds; `control exec --timeout 60 linux -- <command>` makes a narrower limit explicit.

Before the previous app's first launch, create its guest-local `~/.local/state/farhelm/desktop-client.json` only when no
Farhelm state exists. Set `install_updates_automatically` to `false`. Verify the setting and old installed version
before seeding state and again before selecting the update path. The installed record contains `0.23.0`, without the
tag's `v`. For an existing fixture, preserve the file's other fields; never overwrite real state to force a test
premise.

Inside the Mac guest, launch the ordinary installed bundle with:

```sh
open -n --stdout "$guest_evidence/launch.stdout" --stderr "$guest_evidence/launch.stderr" "$HOME/Applications/Farhelm.app"
```

Both releases exercised the real logged-in GUI session and retained the first launch's stderr, including old updater
activity. For ordinary updates, use **? → check for updates**, then select the changed version readout and its **restart
to update** item. Its helper's candidate relaunch had stderr routed to `/dev/null` on the exercised guest; report that
diagnostic gap. Do not manually launch the candidate to conceal a failed helper. A later deliberate quit and reopen may
use the verified launch command with new evidence filenames.

Native override forwarding remains unverified until an installed stable release carries the override. Check its source
at the actual old tag and follow RECIPE.md's path selection. A successful ordinary updater run proves neither forwarding
nor removal of a staged-candidate override.

## Computer use and continuity

Bind computer use to Tart's **specific Mac clone window**, checking its title before every interaction. Its
accessibility tree describes the host window, not guest controls. Use screenshots for guest controls and verify each
submission. Bring-up proved guest clicks, keyboard input and screenshot export. Permissions needed by the guest screen
agent and apps require the maintainer's approval; never transfer a guest approval to host permissions.

Text delivery was inconsistent: `typeText` could display text without committing a reactive guest form value. Physical
`pressKey` character events worked in the launcher, including its host search and command field. Verify complete text
before submission. Native destination-menu selection and modifier shortcuts were unreliable; supported host search
worked. Wheel scrolling did not move the guest dialog, while dragging its visible scrollbar did. These are measured
input limitations; their cause was not isolated as a product defect.

Install `continuity.py` inside Linux before freezing the base or copy it into the owned working clone. Launch this as
Farhelm's **plain command**, with agent type none:

```sh
python3 /path/to/continuity.py run /path/to/private-counter-directory
```

The workload exclusively creates its identity/counter files and never restarts itself. Observe through independent SSH:

```sh
python3 /path/to/continuity.py observe /path/to/private-counter-directory
python3 /path/to/continuity.py observe /path/to/private-counter-directory --before /path/to/earlier-observation.json
```

Save stdout as each observation receipt. `--before` requires unchanged PID, kernel start ticks and Linux boot ID plus a
larger counter. Prove advancement before upgrading, after the app update, after remote supervisor update and after final
reopen. A terminal replay alone is insufficient; stopped/replaced-process tests exercise the oracle's refusal paths. Do
not reboot the guest or restart the workload during this check.

## Signed inputs, evidence and cleanup

Inside a Mac guest checkout with the previous tag available and minisign on PATH, run:

```sh
python3 -B releasing/mac-vm-test/verify_release.py --previous "$previous" --version "$candidate" --output "$verification_directory"
```

The fresh output directory retains bounded version-path downloads, signature, checksums, installer and a receipt. It
verifies against the previous tag's compiled key ring, exact trusted version comment and signed installer checksum. It
does not install anything or replace the old updater's own verification. Keep its source checkout and scratch evidence
out of the frozen clean base after exporting them.

- Save GUI evidence with approved guest `screencapture -x <file>` and command/service output in explicit guest evidence
  directories. Do not collect home directories, credentials or session-token database columns.
- `control collect mac "$mac_guest_evidence" mac-evidence` and the Linux equivalent stream private tar archives to the
  run directory, bounded to 128 MiB and 60 seconds. Existing labels are refused; failed partial exports are retained.
- Inspect archive contents and actual required evidence before teardown. macOS tar may include AppleDouble `._*`
  metadata; distinguish those from the actual PNGs. A completed copy alone is insufficient evidence of a passing check.
- `control shutdown mac` and `control shutdown linux` request clean guest shutdown and require live stopped Tart state.
  macOS can close the execution transport before replying; the failed command stays journaled, and only independent
  stopped state satisfies shutdown. A running guest is retained for diagnosis; there is no forced-stop fallback.
- After verified export, `control delete mac` and `control delete linux` require unchanged ownership and stopped state,
  then verify absence. They cannot delete bases, other runs or pre-existing VMs. Preserve failed clones only by
  agreement.

Keep the manifest, journal, archive hashes and explicit evidence directory for later cleanup. Base images contain real
credentials; never push them or export raw evidence to GitHub. Set a disk/free-space budget and failure-retention policy
with the maintainer before starting another run.

## Harness validation

The stdlib unittest suite injects command runners for lifecycle fences and runs real disposable Linux processes for
continuity. It changes no test-process environment variables. Run it through the repository's recorder inside a guest
source checkout, including the `scripts/` dependencies:

```sh
python3 -B scripts/record-test-run.py --kind development --selection 'portable Tart harness' --concurrency 'one unittest process' --tmux none --timeout 30 --output-root "$guest_test_evidence" -- python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v
```

The historical 27-test suite passed on real Linux during bring-up; profile/coordinator tests extend that suite. Linux
process tests skip on macOS, so a Mac-only run cannot claim those tests ran. BRICK-TEST documents a separate
command-only real-guest smoke for coordinator changes. Guest product checks still need the actual native bundle and GUI
flow described in RECIPE.md.
