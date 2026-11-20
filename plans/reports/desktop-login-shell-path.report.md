## What this was about

A Mac launch from the Dock or Finder could give Farhelm a PATH that left out tools its SSH configuration needed. A
ProxyCommand could name a locally installed helper under `usr/local` and leave only “Connection closed” when Farhelm
could not find it. You chose to use the interactive login shell's PATH for the desktop's own tools while keeping each
session's usual starting PATH. On macOS and Linux, the app, its embedded helm, and a local supervisor it starts now use
the login PATH for programs they look up by name.

## Things you should know

Sessions still start from the same PATH they did before, so their login shell does not have to process a PATH that the
shell has already built. An already-running local supervisor keeps its old PATH until the app starts it again.

If the shell cannot supply a PATH within five seconds, startup uses its inherited PATH. The log records the cause, and
the rail bell explains the fallback and offers **Send feedback**. A later successful lookup resolves the notice. Another
failure reopens a resolved notice quietly if it is still in the bell, or creates a fresh unread notice if it was
cleared. Existing notices and their read marks carry over when upgrading.

## Open questions and possible follow-ups

Launch a macOS devbuild from the Dock or Finder and connect to a host whose SSH configuration calls the locally
installed helper by name. The connection should open successfully. A separate follow-up could improve the bare
“Connection closed” error by retaining the SSH diagnostic that explains why the connection failed.

## PRs

- Reads the login shell's PATH and uses it for the desktop's tools while preserving the starting PATH for sessions.
  [#4338](https://github.com/scode/farhelm/pull/4338/changes)
- Shows the fallback in the rail bell and preserves notice history across launches and upgrades.
  [#4339](https://github.com/scode/farhelm/pull/4339/changes)

## Checks

The focused Linux nextest run passed 33 tests with no runtime skips. It covered the startup lookup, notice history and
upgrade, and the rail's feedback action and focus return. Recorder `b7459f02-2444-47df-b758-b29680d18f06` captured the
`78ad241c` source tree, which matches the production code in #4339.

```sh
cargo nextest run --locked -p farhelm-helm -p farhelm-gui -p farhelm-desktop --lib \
  -E 'test(notices::tests::) | test(desktop_login_path_notice_tracks_startup_and_resolution) | test(login_path::tests::) | test(a_helm_started_with_an_ensure_hosts_file_lists_its_hosts)'
```

The focused rerun of the amended startup test passed with no runtime skip (recorder
`b1ea872f-5bba-42aa-916f-69e7bc44d4ad`). It adds a second failure after a quiet reopen and checks that the bell keeps
the same notice and read mark. This test-only amendment preserves the wider runtime evidence.

```sh
cargo nextest run --locked -p farhelm-helm --lib \
  -E 'test(=tests::desktop_login_path_notice_tracks_startup_and_resolution)'
```

Reused the 10 passing cases from the PR #4338 source review revision `50d0a7dd` (recorder
`8abc261c-654a-4758-81ae-abf85f9f4989`). They exercise the shell output and deadline, and the supervisor's session PATH
handoff. The PR only reworded the commit after this run. PR #4339 leaves these launch paths unchanged.

```sh
cargo nextest run --locked -p farhelm-desktop -p farhelm-supervisor --lib \
  -E 'test(login_path::tests::) | test(service::core::host_launch::tests::) | test(only_the_startup_constructor_waits_for_the_state_dir)'
```

`python3 -B scripts/test-gui-agent.py` passed all 93 cases with no skips, including the tests with a private SSH server
(recorder `041f955e-761b-4744-adef-90ba0cbbc6d7`).

`python3 -B scripts/gui-lifecycle.py` passed all 12 steps on Linux, including forwarding a second launch and keeping
sessions through supervisor replacement (recorder `1fe145dd-fb3f-4a40-a10f-f9f821b6d15b`). The staged README scenario
ran the desktop against its private SSH server and all seven fake-agent sessions reached their expected states across
three hosts (recorder `bda61c3a-5462-44b5-8428-c6ad813b46e3`).

```sh
python3 -B scripts/gui-agent.py <run-dir> start --no-build --scenario docs/readme-hero/scenario.json5
python3 -B scripts/gui-agent.py <run-dir> stop
```

Rust and Markdown formatting, diff whitespace, the Rust test delay checker, crate layering, changelog format, the gated
dependency notices check, pinned `cargo-deny --locked check licenses sources` and focused Clippy passed.

The full workspace suite and general GUI smoke suite were skipped because the focused checks exercise the changed
startup, session environment and notice paths directly. Skipped the rail snapshots because their scenes contain neither
feedback notice. The focused rail test covers the card text, action and focus. Skipped the Dock or Finder acceptance
check because it needs macOS.

## Review

Both changes passed separate fresh-context source reviews. The first review caught startup shells configured to refuse
overwriting the result file and prompted a correction to how an overdue shell is reaped. The second found no product
correctness issues.
