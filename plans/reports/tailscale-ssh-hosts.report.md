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

### Landing

#### What landed with it

The plan's three PRs (#4637, #4638, #4639) landed alone in this round, followed by one landing fix (#4640). Nothing but
queue bookkeeping reached main between the plan's base and its landing, so the rebase was trivial. The plan builds on
the container host identity check that landed just before it (#4632, #4635, #4636): when a host's Tailscale setting
changes, it forgets the machine and account Farhelm had recorded for new containers there, so the next check records
them again over the new route, and it refuses the change while the host still runs environments or holds environment
images built there.

An independent reviewer found no way for the option to weaken host-key checking. A host not marked as a Tailscale host
gets no extra host-key options, so the user's own SSH settings apply as before. A marked host is checked strictly
against a private file built only from the keys Tailscale advertises, with the user's and the system's known-hosts files
and DNS-based key checks switched off, and a shared connection opened under one set of keys or one network path is never
reused under another. Existing hosts all come through the database upgrade as not Tailscale hosts.

#### Fixes made while landing

- The landing's full test run failed three tests the plan broke; its own runs were limited to the code it changed and
  missed them. Two compare a database turned back into the frozen version-26 schema with that schema, and the helper
  doing the rewind had not been taught to drop the new Tailscale column. The third opens Add host, types a destination
  and presses Tab once to reach the alias, but the new Tailscale host checkbox now sits between them. #4640 fixes the
  tests; the product code was right.
- How the helm picks the way to dial a host matched container hosts and sent every other kind of host through a
  catch-all branch, which the repository's code rules forbid in shared code, since a kind of host added later would take
  that branch without anyone deciding it should. #4640 names every kind; nothing changes in behavior.
- The plan's additions to SPEC.md and SPEC_impl.md were not wrapped the way the formatter requires, which fails its
  check; #4640 rewraps them, with no change to the wording.
- #4640 adds caveats to the plan's changelog draft: Tailscale has to be running on the computer running Farhelm, the
  node has to advertise its SSH host key, and changing the setting reconnects the host and is refused while it is being
  checked or set up, or still runs environments or holds environment images Farhelm built there.

#### Checks

The plan changes how the helm opens SSH connections, the hosts table, the desktop app's Add host and Host settings,
their reference screenshots, and two docs pages, so the landing ran the Rust workspace suite (including the real Podman
environment test), the desktop app shell's own tests, the end-to-end smoke test, every reference screenshot, the docs
site build and the lint runs on the plan as delivered, then the helm's and the desktop interface's library suites again
after the landing fix. The worker runs were on a separate arm64 Linux test worker with a systemd user manager, rootless
Podman and SSH to itself; the rest ran locally on x86_64 Linux. None of them reached a real tailnet: the plan's tests
use a scripted Tailscale status, as the report says.

On the plan as delivered, rebased onto main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `9fb706e0-ade0-4584-8d6b-9afec610f5af`): 8384 of 8387 passed, none printed a runtime `SKIPPED`, and the real Podman
  environment test ran and passed. The three failures are the tests the landing fix corrects; the failed run is kept.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app
  shell's two `cargo clippy -p farhelm-desktop --all-targets` runs (with and without `--features agent`), on the test
  worker: passed.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `2f83878f-5836-44ce-bd28-e5d91c418fa0`): all
  185 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `c6db90e8-af98-4aa8-8a55-d476d8bbca70`): all
  50 steps passed, one skipped as always on Linux (paste, which headless Mutter cannot give the app).
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `6fc22f73-dc4e-4eda-8f24-c21376eb3801`): every picture
  matched its reference, the plan's eight republished ones included.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`
  and the docs site build (`bun run build` in `website/`), locally: passed. `dprint check` failed on the plan's SPEC.md
  and SPEC_impl.md passages, which the landing fix rewraps.

After the landing fix (#4640):

- `cargo nextest run -p farhelm-helm -p farhelm-gui --lib`, on the test worker (recorder run
  `59aed754-5002-4276-ac7c-4d233a4c2e59`): all 5163 passed, among them the three tests the fix corrects and the rest of
  the helm's database, SSH, Tailscale and host tests and the desktop interface's Add host and Host settings tests.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

Reused: the workspace run above for everything outside the helm's and the desktop interface's libraries, since the fix
changes only three tests, one helm function's structure (no behavior), spec wrapping and the changelog draft; likewise
the desktop app shell's own tests, the smoke test and the reference screenshots, which the fix does not touch. Skipped:
a live tailnet, which no test setup here has; macOS, where nothing compiles differently, though two Mac-only paths have
not run (see below); CI, which runs only on demand.

#### Things you should know that the report does not say

- Host keys are matched by the node's Tailscale address only, rather than by the address, the full Tailscale name and
  the name typed, as the plan suggested. That is enough, because Farhelm always connects to that address, and since the
  stored keys are tied to the address and not to a port, a node whose SSH server listens on another port (set in the
  user's SSH config) is checked the same way.
- Removing a host now waits while a sign-in question for that host's Set up, Update or Uninstall is open, up to about
  two minutes per question (each new question restarts the wait), since both take the same new per-host lock. This
  applies to every SSH host, not only Tailscale ones. It cannot deadlock, but Remove looks stuck meanwhile. A follow-up
  could refuse Remove as busy instead, or cancel the sign-in check; it is worth asking whether the lock is needed at
  all, since editing a destination changes the route just as much without it.
- A destination that works without Tailscale but cannot be used as a Tailscale node name is refused with "is not a
  usable ssh destination" when the option is turned on in Host settings, although the host works fine without it. That
  happens for a destination with a port, an `ssh://` address, or a name with characters a Tailscale node name cannot
  have, such as an underscore. Add host's check already says "Enter a Tailscale node name or IP"; the setting could
  reuse that wording.
- One new helm test of the setting's change gives itself a fixed five seconds, which could fail on a heavily loaded
  machine. It passed in every run here.
- For a Tailscale host, Farhelm sets the connection's proxy itself, so a jump host or proxy command in the user's SSH
  config for that destination is not used.
- Nothing in this plan has run on a Mac. Two paths only a Mac takes have never run anywhere: finding the Tailscale
  command inside the Tailscale app when it is not on the PATH, and keeping shared-connection socket names within the
  Mac's shorter limit. A first try on a Mac with Tailscale running is the real test of both.
