### What this was about

When Replace with started keeping the source session's own folder, and the helm later began refusing any Replace that
would start a fresh managed checkout, the desktop launcher lost every way to start a managed checkout inside a sandbox
or container session that already exists. Some launcher code written for that case stayed behind. The main piece was the
note in the Destination card, below the checkout's folder, warning that such a folder is removed permanently, not moved
to the trash, when its last session is deleted. The others were three spots in Replace with's handling that looked for a
checkout of its own. None of it could run any more, and no test exercised it, so a later change bringing the path back
could have relied on wording and code nobody had checked. A trailing code review had recorded the leftover note as a
review-feedback item. You chose a full cleanup that also closes that item, with no release note.

What was built, in one PR:

- The launcher no longer has the existing-environment checkout note or the code that picked it. The note in the
  Destination card now says one of two things, as it already did in practice: permanent removal for a checkout inside a
  new sandbox or container the launch creates, and the trash for a checkout on an ordinary host. The code's comments say
  why there is no third case, and that a future path starting a checkout inside an existing environment would need its
  own note.
- The three Replace with spots that looked for a checkout of its own are gone. Replace with behaves exactly as before,
  including the line that says the replacement keeps working in the source's managed checkout.
- The review-feedback item is closed, and the TODO.md entry for this cleanup is removed.

Nothing changes for anyone running Farhelm.

### Things you should know

- **SPEC.md needed no change.** The review-feedback item also asked to remove a SPEC.md sentence promising the note for
  a checkout inside an existing environment. The helm Replace change had already removed it, so "Managed checkouts"
  matched the code before this PR.
- **No changelog fragment.** The PR is a `refactor:` with no effect anyone can see, and the unreleased changelog draft
  the TODO entry mentioned had already gone out in v0.46.0's release notes.

### Open questions and possible follow-ups

None.

### The PRs

- #5160 `refactor: remove dead Replace checkout code from the launcher`

### Checks

- The launcher's and launch templates' unit tests in the desktop app, through the test recorder
  (`cargo nextest run -p farhelm-gui --lib -E 'test(/launcher::/) | test(/templates::/)'`): 963 passed, none failed (run
  0caca2f6).
- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-gui --all-targets -- -D warnings` (no unused code or dead-code
  warnings left), and `dprint check` on TODO.md and the review-feedback index: clean.

The tests, clippy and dprint ran on the change before review. The review's fixes changed only comments in the code
(rewording, and one comment rewrapped) and left TODO.md and the index alone, so those results still cover the change
that landed. `cargo fmt --all -- --check` ran again after the fixes, clean.

Skipped: screenshot comparisons, since none of the remaining drawing changed (the removed note had no screenshot left);
every test outside the launcher and templates, since only unreachable launcher code was removed.

### Review gate

A fresh-context Opus 5.5 agent at high effort reviewed the PR, then verified the fixes; the gate passed. It found no
bugs. It traced every way a Replace with draft or request could have carried a checkout and confirmed that none does,
that the helm itself refuses one, and that the live Replace with behavior is untouched. Its three findings were wording:
a comment that implied the wrong condition, a comment that relied on an unstated rule about its callers, and the commit
message's accuracy. All three were fixed, and the verification found nothing else.

### Landing

#### What landed with it

Nothing outside the queue's own bookkeeping reached main between the executor's last rebase of this plan and its
landing, and it was the only plan in this round. So no other change was landed or reviewed together with it.

An independent reviewer and I read the change and found nothing to fix. The reviewer traced every way a launch could end
up with a managed checkout on a host that belongs to an existing sandbox or container session. New, Clone, Replace with,
Restart with, templates, search, recents and the restore of a waiting launch were all covered, and none can reach that
case. The launcher hands the YOLO question the folder of the checkout a launch starts, and for a Replace with it now
hands none. A Replace with request never carried a checkout folder, so that was already empty for it before.

#### Fixes made while landing

None.

#### What the landing made untrue in the report

Nothing. The report's checks ran only the launcher's and templates' tests and clippy for the desktop app's GUI crate.
The landing added the GUI crate's whole test suite, the screenshot comparison and the desktop app's end-to-end smoke
test, and they found nothing.

#### Worth knowing

- **Main fails `cargo clippy -- -D warnings`, and not because of this plan.** Since the change that made the helm poll
  each host every 30 seconds (#5120), the helm's ntfy settings code keeps a save method that only tests call. Clippy
  reports it as dead code, and with `-D warnings` the helm and everything built on it, the desktop app included, fail to
  lint. Clippy with warnings not made fatal shows that method as the only warning in the workspace's default build and
  in the desktop app. Every executor's clippy gate will hit this until it is fixed. Possible follow-up, recommended now:
  keep the method for tests only, or have the tests call the method the helm uses.

#### Checks

Run once, on the plan's one PR rebased onto main as it stood when the landing started. Main had gained nothing outside
the queue's bookkeeping since the executor's rebase, so this is the code that landed. The change removes code from the
desktop app's launcher only. So the checks cover the desktop app's GUI crate, its screenshots and its end-to-end smoke
test, plus clippy over everything. The SSH test worker was unreachable, so the smoke ran on a boat sandbox (an x86_64
Ubuntu 24.04 VM), and the rest ran locally:

- `cargo clippy --all-targets -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets` without and with
  `--features agent` (the build with the desktop app's test-automation hooks), each with `-D warnings`: all three failed
  on the same pre-existing dead-code warning in the helm's ntfy settings code. That warning has been on main since #5120
  and is outside this plan (see "Worth knowing"). Run again without `-D warnings`, all three reported that warning and
  no other. `cargo clippy -p farhelm-gui --all-targets -- -D warnings` (the GUI crate, which does not build on the
  helm): clean.
- The GUI crate's whole default test suite (`cargo nextest run -p farhelm-gui`), recorded run
  bc87602d-b7f3-4804-8431-4249560a66ea: 3,589 passed, none failed, and it skipped its 4 opt-in tests, among them the
  screenshot test the next item runs.
- The full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run 14684436-a078-42f4-86d8-2200cd6d6779:
  passed.
- On a boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  38640382-d621-4474-ac13-a762a04a8c3d: 48 of its 49 steps passed, and the paste step was skipped, as on every Linux
  run. Its steps open the launcher, including in Replace with, and run a Replace from the session row that asks about
  YOLO first. None of them sends a Replace with from the launcher, so the smoke does not reach the one line this plan
  changed in a launch request; only the landing's two readings of the change (mine and the independent reviewer's) cover
  it.
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format` and `dprint check` on TODO.md and the
  review-feedback index: passed.

Reused: nothing. Skipped: the rest of the Rust workspace's tests, since nothing outside the launcher changed; the opt-in
tests; and any run on a Mac, since none was available to the landing.
