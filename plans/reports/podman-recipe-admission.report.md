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

### Landing

#### What landed with it

This round landed this plan, then podman-proxy-free-images (host proxy settings kept out of captured recipe images).
Both change the helm's container recipe builds, in the same files, so an independent reviewer and I read them together.
They do not conflict in behavior; stacked, they left one variable this plan introduced unused, which the second plan's
landing fixed (see its notes). Since this plan was built, main gained seven unrelated changes (#5079, #5087, #5088,
#5090, #5091, #5092, #5093). Two of them touch the same part of the helm as this plan: reporting a launch's late wake
failure right away (#5092), and showing a tagged resume's outcome in the hosts list (#5079). They change different code
and nothing interacted. This plan adds no new way of reaching a container host without the proof that it is the expected
machine, which landed earlier with podman-host-verification (#5084).

The reviewer confirmed what the report says: a refused check creates no empty record and its reason still shows in
Settings, cleaning up after a failed build cannot create a build record either, Delete clears the failure kept in
memory, and nothing changed in how Farhelm recovers when it cannot tell whether Podman received a request to create a
helper container. The real Podman scenario on the test worker passed with this plan's checks in place (see Checks).

#### Fixes made while landing

None for this plan. The unused variable mentioned above was fixed as part of landing podman-proxy-free-images.

#### Worth knowing

- The report's follow-up stands: rechecking the host before each image pull and each helper container was dropped by
  your decision, and its TODO entry is gone, so that work needs a new entry if you want it.
- When preparing images, the helm looks the host up twice where once would do. The reviewer noted it as a tidy-up; I did
  not check whether a host change between the two lookups could matter. No TODO entry proposed.

#### Checks

Run once for the whole round, on podman-recipe-admission and podman-proxy-free-images stacked in that order on main as
it stood when the landing started; what reached main after each plan was built is in each plan's notes. The test worker
is an aarch64 Linux machine with rootless Podman and a systemd user manager:

- On the test worker, `cargo clippy --all-targets -- -D warnings` failed on the stacked tree with an unused variable,
  which the landing of podman-proxy-free-images fixed. After the fix, both it and
  `cargo clippy -p farhelm --bins -- -D warnings` were clean.
- On the test worker, `cargo nextest run -p farhelm-helm -p farhelm-supervisor`, recorded run
  247588ce-13dc-4361-ae76-c7976490acd4: 3,616 passed, 12 skipped (tests marked ignored). That is every helm and
  supervisor test on the stacked tree, which neither executor ran with both plans together. After the landing fix, the
  helm's Podman provider tests again (`cargo nextest run -p farhelm-helm` filtered to the Podman provider, recipe
  builds, recipe places and launch failures), recorded run cf3a17cc-3a34-4e82-9cbf-38b1876cddfa: 130 passed.
- On the test worker, the end-to-end tests that use a stand-in for Podman and every one whose name mentions Podman,
  recipes or containers, apart from the real scenario below (`cargo nextest run -p farhelm --test e2e` filtered to
  them), recorded run edfb4e85-0d09-4d75-bce9-ad0b35147956: 9 passed. The worker's systemd user manager was not usable
  for two of them this time, so they ran only their fallback path, and the variant where the container is stopped inside
  its own systemd scope went untested. Both check the warning shown when stopping a container fails, on Stop and on tab
  close, which neither plan changes.
- On the test worker, the real Podman environment scenario
  (`cargo nextest run -p farhelm --test e2e -E 'test(/^podman_environments::/)'`), recorded run
  07049c94-e3e1-4467-8d5e-21ecab2af868: passed, and it did not skip. It builds a real recipe image with the production
  Podman provider on a real host, so it ran the fresh Podman check that now starts every build, and captured an image
  after the new step that strips the host's proxy settings from it. Neither executor ran it.
- Locally, `cargo fmt --all -- --check` (again after the fix), the test-sleep check,
  `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown: passed, and `dprint check`
  on SPEC.md again after the landing's wording fix there (in podman-proxy-free-images).

Reused: nothing. Skipped: the rest of the end-to-end suite and the other crates' tests, since both plans change only the
helm's container recipe builds; and the desktop app's tests, the screenshot comparison and the end-to-end smoke test,
since nothing in the app changed.
