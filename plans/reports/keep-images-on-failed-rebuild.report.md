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
