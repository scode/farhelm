# Tailscale SSH on a Mac reports exit status 0 for every command

NOTE: This is a record of what was found on 2026-11-17 while diagnosing a failed host add, and of the decision that came
out of it. It is not documentation of shipped behavior. The upstream status below is as read from GitHub that day; the
issue had about 190 hidden timeline items that were not read, so the summary of the discussion there is partial.

## Short version

When a Mac runs Tailscale SSH (`tailscale set --ssh=true`), every command run on it over ssh comes back with exit status
0, whatever it actually exited with. This is a known, open Tailscale bug,
[tailscale/tailscale#18256](https://github.com/tailscale/tailscale/issues/18256), and it affects only macOS servers: a
Linux machine with Tailscale SSH on reports exit statuses correctly. Farhelm's provisioning reads remote exit statuses
throughout, so adding such a Mac fails at the very first step, and if it had got further, a failed install step could
have read as success.

The maintainer decided to stop relying on ssh's exit status for the commands Farhelm runs on a host and report each
command's status in its own output instead. Detecting Tailscale and refusing the host was considered and not chosen.

## How it showed up

Adding a Mac mini as an ssh host failed within seconds, before any installation step, with:

```
the supervisor probe closed before hello completion with exit status exit status: 0:
host stderr "farhelm-probe-command-started-v1\n"
```

The maintainer believed the Mac was reached through its ordinary ssh server, with Tailscale only providing the network.
That turned out to be wrong: Tailscale SSH answers port 22 on a Mac's Tailscale address when it is enabled, so a plain
`ssh <host>` to that address lands on Tailscale's server, not on macOS Remote Login.

The probe is the first thing the add flow runs on a host. It is a small shell script that prints a start marker to
stderr, then either finds a `farhelm` binary (prints a second marker naming it and `exec`s its stdio proxy) or exits 75,
which the helm reads as "Farhelm is not installed here, offer to install it". Exit 0 with only the start marker and no
protocol output is impossible for that script under an honest ssh server, so the helm correctly refused to treat it as
either outcome.

## What was measured

A read-only diagnostic script, run from the maintainer's laptop (OpenSSH 10.3p1 on macOS, Farhelm 0.40.0-rc.6), replayed
the exact probe script the helm sends (`sh -c '<probe>'`) in five variants: stdin from `/dev/null`, stdin held open by a
pipe, traced with `sh -x`, and twice through a fresh ssh connection-sharing master with the options the helm uses for
provisioning (`ControlMaster=auto`, `ControlPersist=60`, `BatchMode=yes`).

- All five variants exited 0, printed only `farhelm-probe-command-started-v1` on stderr, and wrote nothing to stdout.
- The `sh -x` trace showed the script running to its end: `command -v farhelm` found nothing, both standard install
  paths failed the `-x` test, and the last traced command was `exit 75`.
- `ssh -vv` showed `remote software version Tailscale` and authentication with method `none`, which is Tailscale SSH.
- The Mac ran macOS 27.2.1 with zsh as the login shell and `/bin/sh` as bash 3.2. It had no `~/.ssh/rc`, no
  `/etc/ssh/sshrc`, no `authorized_keys`, and no `ForceCommand` in its sshd configuration, so nothing on the Mac's side
  of ssh itself was rewriting the command.
- The Mac's Tailscale version was not recorded.

From a different machine on the same tailnet, `tailscale ping` reached the Mac but port 22 timed out, both with plain ssh
and with `tailscale ssh`. That machine is presumably not allowed ssh to the Mac by the tailnet's access rules; this was
not checked. As a result, whether `tailscale ssh` (the client wrapper) behaves differently was not tested. It should not:
the wrapper runs the system `ssh` against the same server, and the server is what sends the exit status.

## Upstream status as of 2026-11-17

[tailscale/tailscale#18256](https://github.com/tailscale/tailscale/issues/18256), "Exit status not propagated to ssh
client from macOS", was opened 2025-12-19 (Tailscale 1.90.9, macOS 15.5). Its reproduction: with Tailscale SSH on,
`ssh mac exit 5` returns 0; with it off, 5; a Linux node with it on returns 5.

The cause, as diagnosed by commenters and in [v1k45/bigtty#5](https://github.com/v1k45/bigtty/pull/5): on macOS,
Tailscale SSH runs every command, including non-interactive ones without a terminal, through `/usr/bin/login -f -p ...
$SHELL -c <cmd>`, and macOS `login` exits 0 regardless of the command's status. `ssh -v` shows the server itself sending
exit-status 0, so nothing on the client side is losing it.

What has happened to it:

- Tailscale staff (raggi in January 2026, kradalby in April) pushed several commits against the issue that fixed
  exit-status ordering and session shutdown in general, but not the macOS case.
- #19573 merged and shipped in 1.104.1. Users reported the bug still reproducing on 1.104.1, and a Tailscale staff
  member reopened the issue on 2026-10-17.
- [#20626](https://github.com/tailscale/tailscale/pull/20626), by kradalby, is the direct fix: use `login` only for
  interactive shells with a terminal. Its own commit message notes the tradeoff, that commands bypassing `login` get no
  PAM "remote" session and no utmpx entry (they do not show in `who` or `last`). It never got a review. raggi closed it
  on 2026-10-29 as superseded: "Motivating issue #18256 fixed by #19573 ... Close as superseded (rebase only if the
  darwin login restriction is still wanted)." That contradicts the reports on 1.104.1.
- [#22066](https://github.com/tailscale/tailscale/pull/22066), "bound session teardown so shutdown can't wedge on it",
  merged 2026-10-29. It does not mention the exit status. It was linked as closing #18256 on 2026-11-05, by the staff
  member who had earlier linked another change as a fix and then retracted ("I don't think my fix actually propagates
  the exit status like you expect").
- The latest visible report, 2026-11-05, reproduces on 1.104.5 (the latest release then) on macOS 27.0.1.

No Tailscale staff comment visible on the issue objects to fixing it or defends the behavior. The read at the time was
that it is neglect plus a mistaken belief that it was already fixed, not policy.

Other projects hitting the same thing: Zed Remote Development (its check for its server binary always looks successful,
so it never installs it and fails with "Client exited with exit_code 0", per a comment on the issue),
[dennys246/Maxim#1612](https://github.com/dennys246/Maxim/issues/1612) (which also hit Tailscale SSH's "check mode"
asking for re-approval), and [odysseus0/nix-config#6](https://github.com/odysseus0/nix-config/pull/6). The workarounds
people use are the two below: turn Tailscale SSH off and use Remote Login, or stop trusting ssh's exit status and have
each remote command report its status in-band.

An older, unrelated bug, [tailscale/tailscale#6054](https://github.com/tailscale/tailscale/issues/6054) (2022, every
failure returned 1), was fixed in late 2022 and is not this.

## Where Farhelm reads remote exit statuses

As of main at `4dd1b812c`, in the helm:

- Provisioning (add, install, update, and the host checks they run) is where nearly all the exposure is. About 40 call
  sites run a Farhelm-authored shell script on the host and check its exit status, mostly as 0 or not 0. A few codes
  carry meaning: 75 (Farhelm not installed, the probe), 44 (the inspected file does not exist), 76 (an uploaded
  payload's checksum does not match) and 79 (the destination changed underneath the upload). The checksum one is the
  dangerous case: over Tailscale SSH on a Mac, a corrupted upload would be accepted.
- All of those go through one function that builds the remote command,
  `SystemBackend::remote_command` in `crates/farhelm-helm/src/provisioning/backend.rs` (used by the shell runner,
  the payload upload and the probe), and their statuses are collected in `finish_child_output` (two callers) and in the
  probe's own handshake handling. Container hosts reached over ssh use the same function.
- The live connection to a host runs `farhelm internal stdio` over ssh and speaks Farhelm's own protocol, which is
  already in-band. Its exit status is used only to explain a failed connection (127 reads as "Farhelm is not on that
  host").
- Exit 255 is produced by the local ssh client itself when ssh fails (cannot connect, authentication fails, a server
  refuses another session). The Tailscale bug cannot affect it, and it should keep being used.
- Tensorlake hosts run their own commands through Tensorlake's gateway; they are Linux and unaffected.

Nothing found needs ssh's exit status in a way an in-band status cannot replace, because every remote command in
provisioning is a script Farhelm writes itself.

## Options considered

**Detect and refuse.** Make the probe print an explicit "absent" marker before `exit 75`. If the helm sees that marker
but a status other than 75, the exit status was provably lost. On that failure path only, run a verbose ssh to the same
destination with connection sharing off (a shared connection never sees the server's greeting) and look for
`remote software version Tailscale`, so the error can name Tailscale SSH and the fix (turn it off on the Mac and use
Remote Login). The greeting line is OpenSSH debug output, long stable but not a contract; if parsing it fails, the
generic "this ssh server does not report exit statuses" error still stands. Small and contained, but the host stays
unusable until the user changes their Mac's setup.

**Report status in-band.** Wrap every provisioning command in `remote_command` so that it ends by printing its real
status as a final record carrying a per-invocation random tag (so command output cannot forge it), read the status from
that record when the results are collected, and treat a missing or malformed record as failure. Call sites that check
for 0 or 44 keep working unchanged because the code they see comes from the record. ssh's own 255 stays an exit status.
This is stricter than trusting exit statuses: a server that lies, or a connection that dies mid-command, cannot pass as
success. The probe needs its own small change, since it `exec`s the stdio proxy on the success path.

The first instinct was that in-band status would mean touching every remote command and risk missing one, which would
turn a failed step into a silent success. Reading the code showed the single funnel above, which makes it a contained
change.

## Decision

In-band status, decided by the maintainer on 2026-11-17, recorded as a Near term TODO entry. It should make a Mac with
Tailscale SSH work as a host as far as exit statuses go. Two things were explicitly left open:

- Tailscale SSH on macOS has other open problems, for example sessions left behind on disconnect
  ([tailscale/tailscale#21150](https://github.com/tailscale/tailscale/issues/21150)), and its "check mode" can ask for
  an interactive re-approval that a non-interactive ssh cannot answer. The diagnostic run did not hit either. So the
  change needs to finish with a real end-to-end add of a Mac with Tailscale SSH on, not just the probe.
- A clear error for a missing or mismatched status record is still wanted, since it covers any other transport that
  misbehaves the same way.

A separate, orthogonal TODO entry was recorded the same day: use the `tailscale ssh` wrapper for Tailscale hosts so
Farhelm does not depend on the user having the host's key in ssh's known hosts already.
