# Operate the private Tart test pair

For the standard **`brick test release vX.Y.Z`** request, start with [BRICK-TEST.md](BRICK-TEST.md). Its coordinator
discovers private machine bindings and supplies the handles used below. These lower-level helpers also remain useful for
bring-up and reconciliation; they do not by themselves produce a release verdict.

These stdlib Python helpers run on the macOS host and operate only the fresh clones recorded in one private manifest.
The guest-native tester performs GUI checks; the operator judges its retained evidence. Use Python 3.13+ for the guest
tester and Python 3.11+ for these host helpers, installed Tart with guest execution support, two stopped private bases
and an append-only external journal. Bring-up exercised Tart 2.32.1, Python 3.14 on macOS and Python 3.12 inside Linux.
Install guest prerequisites inside the guests; host installation or network changes require the maintainer's
authorization.

## Prepare the bases

Use a recent compatible Cirrus macOS image and ARM64 Ubuntu. Bring-up exercised Tahoe 26.6.2 and Ubuntu 24.04.5 LTS.
Record the exact OCI digests and provisioning privately. The Mac base needs a logged-in desktop, Homebrew tmux in a
prefix Farhelm probes, genuine selected agent CLIs on its login-shell PATH, Python and minisign for verification. The
Linux base needs key-based SSH, a working systemd user manager, lingering and guest privileges for persistent user
services. Bring-up used Homebrew tmux 3.7c locally and removed distro tmux from Linux so Farhelm installed its static
3.7c payload. Removing distro tmux can remove packages depending on it; inspect the package manager's plan first.

Install Codex in Linux too. Prepare the official native Mac CUA helper and guest permissions during setup, then prove
native use and genuine agents in disposable validation clones. Keep both bases credential-free; authorized injection
happens before each test in its working clones. Never collect authentication stores as evidence. Prove Linux's user
manager through both the command channel and an SSH login. Stop the bases with guest shutdown; verify stopped Tart state
and absence of Farhelm binaries/state. Once frozen, create new clones for all tests. Do not start or provision the bases
again to prepare a pass.

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

For coordinator progress, parse `brick_test.py status` as JSON. Lifecycle commands can mix shutdown diagnostics with
their final report, so preserve their stdout without treating the whole stream as JSON. After successful deletion, the
status entry keeps its verdict and evidence but no longer exposes live guest handles. A reporting error after cleanup
does not invalidate completed product checks; inspect the authoritative status before starting another pair.

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

Actual release checks use `brick_test.py test`, which stages an independent guest executor and checkpoint envelope
through this journaled channel. Each checkpoint starts a fresh `codex exec --yolo` process. Its owned process group is
terminated on timeout or completion, including surviving MCP children; Python `waitid`/`WNOWAIT` preserves identity
through teardown. Export includes only bounded executor diagnostics and validated evidence references. A permission
block is retained as automation failure; no host UI rescue or manual approval is allowed during actual testing.

The executor persists native image bytes directly from the private tool event stream, retaining up to ten recent
distinct captures that fit its evidence budget. The tester requests screenshots at acceptance states and writes command
receipts; it must not encode images or run image-saving scripts. The raw `model-result.json` and enriched `result.json`
remain separate so evidence persistence cannot silently rewrite what the model originally observed. Tester
script/evidence faults use `automation-failure`, not a product `fail`.

The staged `fixture_probe.py` performs read-only cleanliness inspection in the Mac and its pinned Linux peer. Use its
recorded helper path and SSH alias from the task context; it writes one private receipt. Do not replace it with guessed
source paths or ad hoc service probes. An empty systemd listing can exit 1 on a clean guest; the helper accepts only
that exact no-output case after positive manager readiness, retaining other failures as unavailable evidence.

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

The Tart input workarounds below are for **fixture setup and historical diagnostics only**. During product tests, native
CUA binds apps directly inside the guest. Keep shared clipboard disabled in either mode.

Bind computer use to Tart's **specific Mac clone window**, checking its title before every interaction. Its
accessibility tree describes the host window, not guest controls. Use screenshots for guest controls and verify each
submission. Bring-up proved guest clicks, keyboard input and screenshot export. Permissions needed by the guest screen
agent and apps require the maintainer's approval; never transfer a guest approval to host permissions.

Check leftover brick-test pairs before starting a new one, following BRICK-TEST.md's startup reconciliation procedure.
Separate Tart processes share the same application identity; binding by bundle ID or app path can select an earlier
guest even after the intended window is brought forward. This was resolved by cleanly shutting down the abandoned
earlier pair, preserving its disks and evidence, then resetting computer use and binding to the remaining Mac clone.
Verify the returned window title before product setup. Do not shut down another active operator's pair to obtain GUI
access, and do not treat foreground placement as proof that the tool selected that guest.

If computer use returns `noWindowsAvailable` or `cgWindowNotFound`, inspect the coordinator status and live Tart state
before doing anything to the guests. In one run, both owned guests were still running and command-ready even though
binding by bundle ID, installed app path and display name, and a clean computer-use reset, could not recover the Mac
window. Those errors alone do not prove a guest or Farhelm crash. Keep the foreground owner alive and ask the maintainer
to restore access to the visible Mac clone window; its cause and a reliable recovery were not established. Do not
restart guests, start duplicate Tart owners or substitute host Farhelm to get past the missing GUI prerequisite.

Text delivery was inconsistent: `typeText` could display text without committing a reactive guest form value. Physical
`pressKey` character events worked in the launcher, including its host search and command field. Verify complete text
before submission. Native destination-menu selection and modifier shortcuts were unreliable; supported host search
worked. Wheel scrolling did not move the guest dialog, while dragging its visible scrollbar did. These are measured
input limitations; their cause was not isolated as a product defect.

In a later release test, rapid physical key sequences also dropped characters. Per-character `pressKey` with
`durationMs: 100` delivered the complete SSH alias. For replacing a field, triple-click with `durationMs: 100`, inspect
the actual selection highlight in a separate screenshot, then type; a click attempt alone did not establish selection.
Modifier chords were received without their modifier: `super+a` inserted `a`, and both a `colon` key name and explicit
Shift-semicolon chords inserted `;`. Do not submit `tl;` as a template search or assume select-all worked. The tool
refuses standalone modifier presses, so trying to hold Shift in a separate call is not a supported fallback.

The verified workaround for exact text, including punctuation, is the guest's own clipboard and keyboard navigation of
its native Edit menu. Prepare the text through the journaled controller, for example:

```sh
control exec mac -- /bin/sh -c 'printf %s "tl:example" | /usr/bin/pbcopy'
```

Focus the intended guest field; for replacement, verify its selection highlight first. Click the **guest** Edit menu
through computer use, then use `Down` with `durationMs: 100` until **Paste** is visibly highlighted, and press Return
with the same duration. In the exercised Farhelm menu, five Down presses from the newly opened menu selected Paste;
inspect the actual menu rather than assuming that count applies elsewhere. Direct mouse clicks on native guest menu
items, including a continuous drag, dismissed the menu and reached the underlying window instead. Arrow navigation and
Return pasted the exact `tl:` query and produced the expected template match. `pbcopy` needed no sudo or bootstrap
context change; such a change did not solve the mouse-menu issue. Keep the coordinator's `--no-clipboard` isolation and
the host clipboard untouched. Verify the full text and reactive form result before submitting. Record newly verified
workarounds here and in the release-test section of AGENTS.md as they are found.

For terminal-file downloads, first clear any terminal selection with an explicit single click in blank terminal space.
Click the path with `clickCount: 1`, inspect the hover's ready metadata, then click the ready link with `clickCount: 1`
again (a different point within the same path worked). An earlier repeated click selected text instead; Farhelm
correctly refused to download that selection gesture. Require both the actual saved-file message and an independent
check of the resulting file's bytes, not just the hover tooltip.

A copied Codex login can become stale when shared refresh tokens rotate. File presence and `codex login status` do not
prove that a genuine conversation can run; a sign-in request or `refresh_token_reused` is a fixture-authentication
failure. Keep credential contents out of diagnostics and evidence. Reusable bases must remain credential-free. The named
authorized file enters working clones before testing; any later authentication failure stops the run as a harness
failure. Diagnose and repair setup separately, without mid-test refresh or session replacement. Never export
authentication stores or log out a clone to clean it up. Do not infer credential-copy permission from a brick-test
request alone. A dedicated test login is preferable when available. Standing consent authorizes the copy, not a
guarantee that the vendor accepts concurrent use.

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

- Save actual native CUA-returned PNG/JPEG evidence and command/service output in explicit guest evidence directories.
  Do not collect home directories, credentials or session-token database columns.
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

Keep the manifest, journal, archive hashes and explicit evidence directory for later cleanup. Working clones contain
real credentials; reusable bases must contain none. Never publish either or export raw evidence to GitHub. Set a
disk/free-space budget and failure-retention policy with the maintainer before starting another run.

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
