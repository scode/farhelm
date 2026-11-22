## What this was about

Farhelm used the user's SSH host-key settings when adding a machine reached through Tailscale. A first check could ask
the user to confirm the host's key. **Tailscale host** in Add host tells Farhelm to resolve the node on the tailnet and
verify the key Tailscale supplies without asking the user to confirm it. With the switch off, Farhelm uses the usual SSH
host-key settings. The switch in **Host settings → Connection** reconnects an existing SSH host using the selected
policy. An ensure-hosts file can set `tailscale: true` when it first adds a host.

The agreed design keeps system SSH and shared connections. During implementation, a fresh Tailscale lookup could
identify a different address, key or network path from the one an open connection used. Farhelm closes the actual shared
connections for the old route when changing the setting.

## Things you should know

- Tailscale needs to be running on the machine where the helm opens SSH connections, and the peer must advertise the key
  its SSH server presents. A node that only runs its own SSH server may omit that key. Farhelm explains why it could not
  verify the host. It keeps its own copies of the advertised public host keys for marked hosts, leaving the user's
  known-hosts files untouched.
- Sign-in, setup and sessions use the same connection policy. Farhelm closes the previous shared SSH connections when it
  changes that policy. It can look up and close existing Tailscale connections even with the daemon stopped. Farhelm
  refuses a route change during SSH checks or setup, and keeps the route of a host with recorded Podman resources.
- If two destinations resolve to the same Tailscale machine, each destination has its own shared connections. That lets
  changing one host close only its connections. If a destination is forgotten and immediately added again, a check
  racing key-file cleanup can fail safely and be retried.
- Add a remote host and Manage hosts now show where to find the option and what it requires.

## Open questions and possible follow-ups

No open decisions or proposed follow-ups.

## Pull requests

- [#4637](https://github.com/scode/farhelm/pull/4637/changes) prepares SSH connections with Tailscale's advertised keys.
- [#4638](https://github.com/scode/farhelm/pull/4638/changes) saves the SSH mode and applies it across host operations.
- [#4639](https://github.com/scode/farhelm/pull/4639/changes) adds the desktop controls, screenshots and docs.

## Checks

- **Run.** `cargo nextest run -p farhelm-gui --lib -E <filter>` passed the Escape regression
  (`762e264c-27b5-41b8-be01-2ef70e096096`) and all 130 Add host, Host settings, setup, approval and launcher tests
  (`bda18a4b-5703-4256-a516-e2c07be7c548`). `python3 scripts/check-gui-goldens.py --update` rendered 406 Linux pictures
  (`bc54343e-b3a4-4295-bc43-53144776d7b1`). I reviewed and published the eight changed pictures across both themes.
  All-target Clippy, Rust formatting, the test-delay checker, changelog formatting and the docs build passed. The docs
  build validated all internal links.
- **Reused.** `cargo nextest run -p farhelm-helm -p farhelm-proto --lib -E <filter>` passed 99 cases
  (`39ebb1a4-700c-407c-bf56-33e92785cf58`) against the working tree snapshotted as `26be22ca` on parent `95dcac2b`. They
  covered SSH preparation, host registration and migration, setup plans, mode switching and Podman routing. The
  published Helm revision `6db72594` changes only how a diagnostic string is passed to a function, leaving the text and
  behavior intact. Scoped all-target Clippy passed on that revision.
- **Skipped.** Local Linux checks used scripted Tailscale status and a local OpenSSH configuration probe. A live tailnet
  and a macOS runtime were outside this fixture setup. The targeted checks exercised the changed paths, so the full
  release battery was skipped.

## Review gate

The SSH preparation review verified fixes for reuse across key and route changes. The helm review verified the route and
request-lifetime fixes. I also addressed its remaining compiler, socket-fixture and removal-order findings, then ran the
focused suite. The final desktop review found no remaining blocking issues. All three slices had independent source
review and cold reads of the commit wording. I checked the changed screenshots in both themes.
