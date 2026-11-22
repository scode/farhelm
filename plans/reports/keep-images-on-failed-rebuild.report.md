# Keep working container images when Rebuild fails

## What this was about

When **Rebuild** ran for a container recipe, Farhelm stopped using the host's working images before their replacements
were ready. If a package or installer was unavailable, the recipe could become unable to start new sessions there.

## Things you should know

Rebuild now replaces the software image and the image with Farhelm installed together, after both are ready. If Rebuild
fails, a later launch can use the previous images while the host is usable and the images are still fresh and present.
Settings and the launcher show the Rebuild error and explain when new sessions can use the previous image. As decided in
planning, a launch already waiting for the failed Rebuild receives its error and can be retried.

The usual preparation freshness checks still apply, including the 24-hour limit and matching the Farhelm version.
Ordinary preparation can still reuse a successful software build when it needs to retry installation.

## Open questions and possible follow-ups

None.

## PRs

- Keep the working images when a container recipe's Rebuild fails.
  [#4624](https://github.com/scode/farhelm/pull/4624/changes)

## Checks run, reused and skipped

- **Run.** Three focused tests passed on the final code (run `03c676ec-f745-4ec5-9433-d3626db94397`). The controlled
  engine fixture verified that launches waiting on Rebuild receive its outcome, a failed Rebuild preserves the two
  working images, and the status offers the fallback only for a usable host. The two recipe feed tests verified that the
  desktop app sees jobs start, report progress and finish, and that reading recipes starts no work. Helm formatting,
  Markdown formatting, changelog format, whitespace and test-delay checks also passed.
- **Reused.** All 82 selected helm tests passed on `5f900cfe` (run `fd0194d0-5c71-4f6e-ad51-a94445c8b3df`). The final
  wording change only uses the cached host check when formatting the recipe status. The focused rerun covers the wording
  shown by Settings and the launcher.
- **Reused.** One real local rootless Podman lifecycle passed on `5f900cfe` (run
  `55fd1891-7061-4f05-b43a-1197bf287493`). It installed Farhelm built from that revision in the container and exercised
  preparation, launch, session lifecycle and image cleanup. The controlled engine fixture exercised the failed Rebuild.
  The wording change leaves the real scenario's execution paths and assertions intact.
- **Skipped.** The SSH lifecycle uses the same image selection and Rebuild logic as the local lifecycle. The real run
  covers local Podman. The broader workspace suite would add little coverage of these paths.

## Review gate

The required fresh-context Sol review ran at high effort. It caught ways a launch could miss an ongoing Rebuild during
image cleanup or pick up the result from a finished attempt. It also caught cases where a failed host check could leave
Settings with an outdated or misleading message. The final follow-up passed with no open findings. The commit wording
passed a separate cold read.

### Landing

#### What landed with it

The plan's PR (#4624) landed first in this round, followed by the Restart cleanup fix, the harness drift test's resume
check and the screen fixture scrubber. Since the plan was written, main had gained refactors of the desktop window's
code and a change to how the helm learns whether a host setup command succeeded over SSH. Neither touches recipe builds,
and the rebase conflicted only in TODO.md, where each plan removes its own entry.

An independent reviewer read the change against the plan and found nothing to fix. A failed Rebuild never touches the
two working images (the software image and the image with Farhelm installed): a failure removes only the new images it
had built so far. A successful Rebuild swaps both in one step and then removes the old ones unless a container still
uses them, as before. Tensorlake recipes are unchanged.

#### Fixes made while landing

None.

#### Checks

The round landed changes to the helm's container recipe rebuilds and the supervisor's Restart cleanup, plus two test
tools, so the landing ran the Rust workspace suite, the lint runs and the two tools' own tests, once, on the four plans
stacked together. No desktop code changed (the new Rebuild wording in Settings and the launcher comes from the helm), so
the desktop app's tests, the end-to-end smoke test and the reference screenshots were not run. The worker runs were on a
separate arm64 Linux test worker with a systemd user manager, rootless Podman and SSH to itself; the rest ran locally.

On the four plans as delivered, rebased onto main and stacked:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `03875ce4-e83a-47da-87cb-b3c4ee02da69`): 8349 of 8350 passed, none printed a runtime `SKIPPED`, and the 86 tests
  marked ignored did not run, as on every run. The real Podman recipe build and launch test ran and passed. The one
  failure was the supervisor test that refuses session titles with control characters, which timed out after ten seconds
  waiting for its own create reply while the suite ran around it. It exercises neither supervisor startup nor Restart,
  the parts of the supervisor the Restart cleanup plan changes, it is not a known flake, and it then passed 30 times in
  a row on the same worker, each attempt a separate recorder run inside one worker session (worker session
  `687080cb-021d-499c-a209-af6f89149001`). One timeout is not enough to record it as a flake.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed. Lints are not runtime tests, so they have no recorder run.
- `python3 -B scripts/test-run-harness-drift.py`, `python3 -B scripts/test-capture-agent-screens.py`,
  `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.

Skipped: the real-agent harness drift tests, which spend vendor turns and run only when asked; macOS, since nothing in
the round builds only on a Mac; CI, which runs only on demand.

#### Things you should know that the report does not say

- After a failed Rebuild of a Tensorlake recipe, Settings and the launcher show only the error, while for a Podman
  recipe on a usable host with fresh images they now also say new sessions can use the previous image. A failed
  Tensorlake build already kept its old snapshot, so the two providers now word the same situation differently. The plan
  only covered Podman; a follow-up could give Tensorlake the same explanation.
