## What this was about

Some SSH servers report success even when a command failed. On a Mac running Tailscale SSH, that hid the result that
Farhelm uses to tell a host with no running supervisor from a failed connection. Add stopped before it could offer
installation. A later failure, such as an uploaded binary failing its checksum, could also be mistaken for success.

## Things you should know

Commands for Add, Update and uninstall now report how they ended. A missing or malformed report fails the step and
preserves the host diagnostic. SSH connection failures still keep their own status. When the first check already gets a
valid reply from a running supervisor, Farhelm can use the reply immediately. This works through the regular SSH client,
including when its server is Tailscale SSH. The live supervisor connection already speaks its own protocol, so a server
that loses exit codes can still give a generic error if that connection fails before it starts.

If a remote command dies from a signal, the shell now reports a number such as 137 for SIGKILL. A command's own exit 255
is distinguishable from a failed SSH login or connection.

## Open questions and possible follow-ups

The real Mac Add remains a post-landing check. From a build containing this change, add an Apple-silicon Mac that runs
Tailscale SSH through the normal Hosts panel. Check that a Mac without a supervisor reaches the install offer, and check
connection and disconnect after installation. Tailscale can require interactive reapproval in check mode, and may leave
sessions behind after disconnect. Those behaviors are separate from exit status. The ignored
`remote_macos_setup_update_crash_recovery_and_uninstall` test also accepts an explicitly owned fixture Mac through
`FARHELM_TEST_SSH_DESTINATION` and a staged Darwin payload.

The real Tensorlake gateway check was skipped because a registered SSH key for that disposable test was unavailable.
Tensorlake's command routing and failure handling ran in the local suite with a stand-in. The gateway follow-up is to
run one successful and one failing setup command in a disposable sandbox and confirm the gateway delivers both results.
The plan allows delivery without that check when a key is unavailable.

## Pull request

- Recover remote setup results from command output. [#4617](https://github.com/scode/farhelm/pull/4617/changes)

## Checks

- `cargo nextest run --locked -p farhelm-helm --lib -E 'test(/^provisioning::/)'` initially ran 387 tests at
  `ba4c8d31efaf`. Five fixtures misread the bound of a quoted script in the wrapper and failed to run their planned
  substitutions. The result is retained (recorder `dd491f6c-7603-44ed-8f0a-27316110553a`).
- `cargo nextest run --locked -p farhelm-helm --lib -E 'test(/remote_scripts_have_the_runtime_fallback_and_prepare_parses|conditional_settings_install_preserves_a_concurrent_save|agent_upload_temporary_is_private_before_finishing|failed_remote_upload_removes_partial_temporary|remote_upload_launches_no_other_command_until_transfer_exits|ssh_transport_255_takes_precedence_over_a_recorded_status|tagged_shell_keeps_the_full_stderr_diagnostic_budget|remote_put_fails_when_ssh_hides_the_upload_error|remote_install_rejects_tampering_after_upload/)'`
  passed all nine corrected or targeted cases at `b242f426cf1f` (recorder `574259a6-5384-4c6a-80e7-c36307ca31a4`).
- `cargo nextest run --locked -p farhelm-helm --lib -E 'test(/^provisioning::/)'` then passed all 390 selected tests at
  `b242f426cf1f` (recorder `3a5b8a71-a537-42e0-bb5b-05d4829d2d6b`). No selected tests returned an early runtime skip.
  The real localhost SSH case installed a supervisor, updated it while a session stayed usable, and uninstalled it.
- `cargo clippy --locked -p farhelm-helm --all-targets -- -D warnings` and `python -B scripts/check-test-sleeps.py`
  passed at `b242f426cf1f`. Formatting used `cargo fmt -p farhelm-helm` and
  `dprint fmt SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/host-setup-command-status.md`.
- The stack rebased onto `a75511f5b44d`. Main added terminal grid protocol 86 and compact checkout rendering. The
  provisioning implementation and fixtures are byte-identical to the validated revision. The framed hello and version
  refusal paths retain their contracts. Reused the passing nextest, Clippy and test-delay commands from `b242f426cf1f`.
- At the rebased draft `9f87c63b1e68`, `cargo fmt -p farhelm-helm --check`,
  `dprint check SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/host-setup-command-status.md`, `git diff --check` and
  `python3 releasing/check-changelog.py format` passed on the same tree before the push.
- Skipped a full workspace sweep because the change is confined to the helm's provisioning route. Mac and real
  Tensorlake coverage are described above. No desktop rendering changed.

## Review

The independent Sol high source gate found two issues that are fixed in the change. The status record now has its own
allowance inside the bounded stderr capture, so it cannot displace the host's diagnostic. The wrapper separates a
command's 255 from a real SSH transport 255. A new test checks that a zero-reporting SSH transport cannot hide a failed
upload. The corrected fixtures and all three review cases passed. The commit message was cold-read separately.

### Landing

#### What landed with it

The plan's PR (#4617) landed last in this round, after the two helper refactors of the desktop window, followed by one
landing fix (#4629). It changes the helm's host setup and SSH code, which nothing else in the round or on main since its
base touched, so the rebase was clean.

An independent reviewer traced every place the helm reads how a setup command ended and found no blocker: when the
status line a command now prints at its end is missing or garbled, the step fails with "the host did not report how the
command ended" instead of passing, and a command's own exit 255 stays apart from an SSH connection failure.

#### Fixes made while landing

- #4629 adds a context paragraph to the plan's changelog draft for the release curator. The draft said adding a host
  whose SSH server reports success for failed commands can now work, but not that no real Tailscale SSH Mac and no real
  Tensorlake sandbox took part in testing, that only host setup commands changed, or that Tailscale SSH's check mode and
  leftover sessions are untouched. The entry text itself is unchanged.

#### Checks

The round landed two refactors of the desktop window's code and a change to how the helm learns whether a host setup
command succeeded, so the landing ran the whole Rust workspace, the desktop app's tests, the lint runs, the end-to-end
smoke test and every reference screenshot, once, on the three plans stacked together. The worker runs were on a separate
arm64 Linux test worker with a systemd user manager and SSH to itself; the rest ran locally.

On the three plans as delivered, rebased onto main and stacked:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `068b2cdd-019d-458c-8d42-853d8baa039b`): all 8342 passed, and none printed a runtime `SKIPPED`, so the tests that need
  the systemd user manager, Podman or SSH ran rather than returning early. The 86 tests marked ignored (benchmarks and
  opt-in real-host and real-Mac tests among them) did not run, as on every run.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and both desktop app
  clippy runs (with and without `--features agent`), on the test worker: passed. Lints are not runtime tests, so they
  have no recorder run.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `d7399edb-dd5b-4c27-bdff-c8afce3a87d9`): all
  185 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `dbfad242-d312-4bbc-8951-85c55a6a19f7`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `87d4fc87-f934-4921-977d-06508c7234bb`):
  every reference screenshot matched.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 -B scripts/test-gui-screenshot-batch.py` (the screenshot
  comparison page's own tests), locally: passed.

After the changelog fix on the host command status plan (#4629), which touches only a release-notes draft:
`python3 releasing/check-changelog.py format` and `dprint check`, locally: passed. Everything above still applies.

Skipped: macOS, since nothing in the round builds only on a Mac; for the host command status plan, a real Tailscale SSH
Mac and the real Tensorlake gateway, which its report names as post-landing checks; CI, which runs only on demand.

#### Things you should know that the report does not say

- The new status line goes to the command's error output. If the Tensorlake gateway or a Tailscale SSH Mac merged error
  output into normal output, Add or Update would fail with "the host did not report how the command ended" rather than
  report a false success. Nothing suggests either does this, but the local tests use a stand-in that keeps the two
  apart, so only the real-host checks the report names would show it.
- The tests check that the status line is removed from what the install, update and uninstall steps hand on, but not
  from the output of Add's first check, which goes through its own path. A follow-up test would close that gap.
- Whether an exit 255 means "SSH failed" depends on the command's route going over SSH (plain SSH, or Podman reached
  over SSH), and the helm decides that from a fixed list of route kinds. A new SSH-based route added later without
  updating that list would have its connection failures reported as the command's own exit 255. Worth remembering when a
  route is added; no change needed now.
- SPEC_impl.md does not mention that when a command itself exits 255, the helm's wrapper exits 254 so SSH's own status
  never looks like a connection failure, while the status line still carries the real 255. A one-sentence docs
  follow-up.
- TODO.md's "Use the Tailscale SSH wrapper for Tailscale hosts" entry still calls itself separate from "the in-band
  status entry above", which this plan removed. Harmless; the next edit of that entry can drop the reference.
