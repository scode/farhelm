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

### Landing

#### What landed with it

Main gained nothing but plan bookkeeping since this plan's work started, and nothing else landed in this round. The one
other plan with open PRs, shell-sessions (Bash and Zsh sessions, sent back in the previous round), touches none of the
files this plan changes and adds no helm database version, so the two do not interact. An independent reviewer and I
checked that.

The review also checked what the report does not spell out. The PATH swap happens while the app still runs one thread,
so it cannot race anything; only a lookup that ran out of time starts a thread, and then the PATH is left alone. The
session PATH handoff reaches both a session's agent terminal and its shell tabs and never shows up in a session's own
environment. The helm's notice table rebuild keeps every existing notice, its read mark and the numbering of later
notices.

#### Fixes made while landing

One went in as a third PR, [#4340](https://github.com/scode/farhelm/pull/4340/changes). On a Mac, Restart to update and
the reopen that follows a missing app version start the next copy of the app with the running app's environment, and
that environment now held the login shell's PATH. The new copy then took the login PATH as the PATH it was started with,
and because an update also replaces the local supervisor, it gave that PATH to the new supervisor as the PATH sessions
start from. Every session's login shell would then run its startup files over a PATH another login shell had already
built, which on macOS moves your own directories behind the system ones, the very thing the plan keeps a separate
session PATH to avoid. It would have lasted until the next supervisor replacement, normally the next update, so it would
have come back every time. Both relaunch paths now hand the next copy the PATH this one was started with. A new test
checks the command they build; whether macOS really passes the environment through is not something a Linux test can
see, so the next Mac release test covers it.

The same PR adds a context paragraph to the changelog entry for the release curation (the change applies on Linux too,
and sessions keep their usual starting PATH), one sentence about the relaunch to SPEC_impl.md, and corrects a code
comment that said a reopened app gets no variables from the app that opens it.

#### Checks

The plan changes the desktop app's startup, the supervisor's session environment, the helm's notices and their database
version, and the app's notification panel, so the landing ran the whole workspace suite, the desktop app's tests, the
app's end-to-end smoke and lifecycle runs and the four lint (clippy) runs on the maintainer's own test machine (an arm64
Linux VM that builds and tests the tree itself), and the screenshot comparisons and small checks locally on x86_64.

On the plan as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine
  (`cab5c4d8-3324-4981-aa52-2b5b5a67c5c4`): all 8158 tests passed, with no runtime skips.
- `cargo nextest run -p farhelm-desktop`, on the test machine (`43c233cb-6c32-4425-979f-39c23694ec73`): all 179 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine (`873dbf29-5a8f-499d-8be6-a97e9cf7cf72`): 49 of 50 steps
  passed and the paste step skipped, as it always does on Linux.
- `python3 -B scripts/gui-lifecycle.py`, on the test machine (`8e4bf081-02b2-4aa5-9bad-2c1c9e931da4`): every step
  passed, including relaunches that reuse and replace the supervisor.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's two
  clippy runs (with and without the `agent` feature), on the test machine, not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py`, locally (`17944bea-e75d-4c7a-bae0-3c525968e8d6`): passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4340), which changes only the desktop app's sources, SPEC_impl.md and the changelog fragment:

- `cargo nextest run -p farhelm-desktop`, on the test machine (`dc953136-adc2-4a8a-9be3-e4276aa63ad6`): all 180 passed,
  the new relaunch test among them.
- The desktop app's two clippy runs, on the test machine: passed.
- `cargo fmt --all -- --check`, `dprint check` and `python3 releasing/check-changelog.py format`, locally: passed.

The other runs above were reused for the fix: it touches nothing they cover outside the desktop app's relaunch commands.
A last amendment to #4340 after its review changed only comments, a test's description and the changelog fragment, so
these runs were reused for it too; `cargo fmt --all -- --check`, `dprint check` and the changelog check passed again
locally.

Skipped: the macOS checks (the Dock or Finder launch and the relaunch's environment), which need a Mac; the report asks
the maintainer to confirm the Dock launch on a devbuild.

#### Things you should know that the report does not say

- Every app start now waits for an interactive login shell, up to five seconds, before the window opens. A shell whose
  startup files cannot finish without a terminal (one that starts tmux unconditionally, say) fails the lookup, keeps the
  old PATH and shows the new notice each start; the notice's Send feedback is how you would hear about that.
- The report's Mac check still stands: launch a devbuild from the Dock or Finder and sign in to a host whose ssh
  configuration runs a program from `/usr/local/bin`. Also worth trying once: Restart to update from that devbuild, then
  open a new session and run `echo $PATH`; your own directories should still come before `/usr/bin`.
- Smaller points from the review, left as they are: a failure to wait on the shell is logged as "could not run the
  interactive login shell" though the shell did start; the new SPEC_impl.md paragraph sits under the heading about
  versions inside Farhelm.app although it covers Linux too; and no test checks that the lookup's shell runs without a
  terminal of its own, though the code does so the same way the supervisor's start does.
