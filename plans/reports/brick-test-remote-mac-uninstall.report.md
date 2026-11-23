## What this was about

The release brick test exercised the remote host's **Uninstall** menu only on Linux. On macOS, Farhelm refuses remote
setup and uninstall on a Mac that has the desktop app installed. The continuity and remote menu tier (P1) now uses a
separate pair of Mac guests to verify uninstall from the app and confirm that the remote Mac keeps its session history.

## Things you should know

- The GUI Mac runs the candidate app. The headless peer starts without Farhelm, uses Remote Login and Homebrew Python,
  and gets no Codex credentials. It defaults to the same base image as the GUI Mac, with an optional dedicated base. If
  the clones inherit identical SSH host keys, the coordinator needs noninteractive sudo in the disposable peer to
  replace its copy before pinning the peer's identity.
- The app adds the peer, starts and stops one plain-command session, and uninstalls from the host menu. The test
  verifies a running supervisor and saved session before allowing the uninstall. Afterward it checks that the launchd
  job, the installed files and Farhelm processes are gone and that the database holds the same session IDs.
- In a full run, the live feedback check (P2) uses the installer pair. The Mac P1 check runs after cleanup of all pairs
  used for the core install, upgrade and supervisor checks (P0). It can still run after a P0 failure once the failed
  pair is released. Bonus work requires passing P0 and waits for the Mac pair to finish. Older saved runs report four P1
  results. Runs that planned the Mac check report it as a fifth result even if it fails or cannot run.

## Open questions and possible follow-ups

The next live release brick test should establish that the two Mac guests boot, establish pinned SSH, and complete
native Uninstall with the session evidence described above. The portable tests simulated the guest and UI boundaries.

## PRs

- Add the headless Mac fixture and pinned SSH setup. [#4700](https://github.com/scode/farhelm/pull/4700/changes)
- Run and report remote Mac Uninstall in P1. [#4702](https://github.com/scode/farhelm/pull/4702/changes)
- Update the operator guide and remove the completed TODO. [#4703](https://github.com/scode/farhelm/pull/4703/changes)

## Checks

- Ran `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` through the test recorder. The
  reviewed fixture revision `321f92e73c06` passed 309 tests (run `635bf752-ea50-44b6-a563-ad95a4beb687`). The reviewed
  coordinator revision `606bafc0e741` passed all 315 tests with no skips (run `21e35ab7-2eac-41dd-935f-56c3e6124e1f`).
  The first full coordinator run (run `02b5dd7d-a8f9-426e-b784-cc9ff9b156cd`) exposed three old expectations about pass
  ordering and saved-run fixtures. The corrected cases passed before the full rerun.
- Ran scoped `dprint check`, `git diff --check`, and `python3 scripts/check-agents-md.py` on the documentation step. The
  instruction-link check reported zero errors and an existing archive citation warning.
- Reused the portable coordinator suite for the final stack because its Python tree matches that tested revision and the
  last PR changes Markdown only. Skipped Tart and native GUI execution on the Linux executor. Rust and JavaScript checks
  had no changed product code to cover.

## Review gate

One fresh independent reviewer checked each PR. The fixture review caught SSH key availability and same-image key
duplication. The coordinator review caught an overly strict P0 gate and a case where an invalid starting sample could
still allow Uninstall. Both reviewers rechecked the fixes. The documentation was checked against the implementation.
