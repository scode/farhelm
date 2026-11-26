## What this was about

A recipe's container images are built on each container host where it runs, and Settings → Recipes shows one card per
recipe and host with its build state and its last failure. Rebuild answers right away and runs in the background, and
the host's fresh Podman check (the one that confirms it can run containers and reaches the expected machine and account)
runs only after that answer. When that check failed on a host where the recipe had never been built, the only way to
show the reason was to write it into the recipe's saved build record for that host. That created an empty build record
for a host that never owned anything, which recipe Delete later had to recognize and clear. A second problem sat next to
it: the first time a build saved that it owned a helper container, Farhelm looked the host up again by its internal ID.
If the host's SSH destination or Tailscale setting changed between the check and that save, the build recorded its
helper under settings that were never checked.

In planning you decided to keep Rebuild answering immediately, to keep failures that happen before the check passes in
memory rather than in a new record, to check once when a build or launch preparation starts, and to drop the TODO
entry's second part (rechecking the host before every pull and helper container). That part is not done.

What changed:

- Every build of recipe images on a container host, whether a launch needs it or the user pressed Rebuild, now runs the
  fresh Podman check itself first, before it saves anything. Before, a Rebuild cleared its old failure in the saved
  record before checking, which is what created the empty record.
- A failure is saved only into a build record that already exists. When there is none, Farhelm keeps the failure in
  memory, and the Settings card shows it, as visibly as before, including on hosts the card would otherwise hide because
  the host is currently unusable.
- The build's first save of helper ownership, and that of the helper container that installs Farhelm into the image, are
  tied to the exact host settings and machine-and-account claim that passed the check. If either changed in between, the
  build fails instead of recording ownership under the new settings.

## Things you should know

- A failure kept in memory is gone after Farhelm restarts, and it is cleared when the next build at that host starts or
  when the recipe is deleted. Failures that happen after the build has saved something (a failed package install, say)
  are still saved and survive a restart as before.
- Keeping a failure only in memory applies to launches as well as Rebuild: a launch whose build fails before it has
  saved anything at that host leaves no record either, and the card shows the reason the same way. Your planning
  decision named only Rebuild failures.
- The binding to the checked settings is stricter than the plan's outline. The outline bound only the build's first
  save. During review it turned out that when only the installation step is out of date (after a Farhelm update, for
  example), the installing helper's save is the first one, so that save is bound too. This adds no new check; it only
  decides which host settings those saves must match.
- A build that has already started and then sees the host's settings change fails at its next helper save rather than
  carrying on under the new settings. That only happens if someone edits the host's destination or Tailscale setting
  during a build.

## Open questions and possible follow-ups

- The dropped part of the TODO entry (rechecking the host before each image pull and helper container, and running from
  exactly the route that passed the last check) is not done, as you decided. The entry has been removed from TODO.md
  entirely, so if you want that work later it needs a new entry.

## PRs

- #5094 fix: show container recipe admission failures without an empty record. Code, tests, a SPEC_impl.md paragraph
  next to the Rebuild rules, the changelog fragment, and the TODO entry's removal.

## Checks run, reused and skipped

Run now (recorder runs on Linux):

- `cargo nextest run -p farhelm-helm --lib` filtered to the container build, Podman check, recipe build registry,
  recipe, UI channel and session launch tests: run 9db15e3d, 201 passed, including the three new tests (a first Rebuild
  refused at the check shows its reason with no build record; a host retargeted after the check fails the first
  ownership save with no command run; the Settings recipe places list shows a failure kept only in memory, and recipe
  Delete clears it). Earlier runs 37a5c6da (106 passed) and 5a302acf (105 passed) covered the first version of the
  change before the review fixes.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings` and `cargo clippy -p farhelm-helm --lib -- -D warnings`,
  `cargo fmt --all`, the test sleep checker (0 delays without a reason), `python3 releasing/check-changelog.py format`,
  and `dprint check` on the changed Markdown.

Reused: none.

Skipped:

- The workspace-wide test run and the end-to-end suites. The change is confined to the helm's container recipe builds
  and the Settings recipe places list, which the targeted selection covers; nothing in the supervisor, the desktop app
  or the wire protocol changed.
- A rebase onto the latest main. Main gained an agent Delete notice change and a terminal tab close change since this
  was based. Neither touches recipe builds, and the PR merges cleanly onto main as it is.

## Review gate

A fresh-context Opus 5.5 reviewer at high effort reviewed the PR. Round 1 found four issues, all fixed: a failure
arriving while a recipe was being deleted could have survived the Delete and shown up on a recipe later recreated under
the same name; nothing tested that the Settings card for such a failure actually appears; the preparation helper's save
was not bound to the checked settings (described above); and two test assertions were weaker than they looked. Round 2
confirmed all four fixed and found nothing new.
