## What this was about

Selecting Managed checkout made the launcher's Destination card tall. The path appeared twice, and the explanation
wrapped down a narrow middle column. The approved layout puts Host and Repository together on an installed host. The
Folder path and lifetime note use the full width below the address and optional Name.

## Things you should know

- Folder now shows the resolved path, the wait for a current preview, or the host's refusal in the same place. The
  plain-text Folder line replaces the old selectable, focusable path field, as agreed. A suspended host still explains
  that it picks the folder when ready. The existing suspended-host screenshots were added after planning, so this change
  updates them as well as adding the two requested scenes.
- For a new environment, Recipe stays in the address and Repository has its own row below the placement controls. The
  Folder line reads “Cloned inside the new environment once it starts”. Its lifetime note reads “New folder named after
  the session, inside the new environment; deleting its last session removes it permanently.” This layout and wording
  came from the planner and are covered by interaction tests. The screenshot review covered installed hosts.
- The checkout setup field still precedes the repository because the host needs a checkout root before previewing. The
  docs image showing that field, `checkout-setup`, will change at the next docs refresh.

## Open questions and possible follow-ups

None.

## PRs

- [#4603](https://github.com/scode/farhelm/pull/4603/changes) Compact the managed-checkout destination in the launcher.

## Checks

- Ran 245 focused GUI tests with `cargo nextest run --locked -p farhelm-gui --lib` after rebasing, covering the checkout
  preview, repository suggestions, host wake, environment destination, the rest of the launcher form, template save and
  host-supplied text. All passed with no runtime skips (recorder `2910b243-34b6-41fa-97c7-d3c7def2be42`).
- Ran the full Linux screenshot update with `scripts/check-gui-goldens.py --update --jobs 4` (recorder
  `7f902468-0094-44fe-9a93-6c34ad89d7f8`). All 404 rendered. The four new pictures and two updated suspended-host
  pictures match what Opus reviewed byte for byte. Published those six references.
- Ran locked Clippy on all GUI targets, scoped Rust and document format checks, changelog and whitespace checks, and the
  test sleep checker. The sleep checker found zero unexplained delays.
- Ran crate layering and its 12 checks (recorder `dad28053-e8fa-420b-a5d5-aec38d47a640`), the license and source policy,
  and the dependency notice check and its 17 tests (recorder `38afce68-62c4-4d57-8774-789a47dd20ff`). The published
  license inventory is current.
- Ran all 47 screenshot catalog and batch-harness tests with `python3 -B scripts/test-gui-screenshot-batch.py` against
  the updated manifest (recorder `3e71cdeb-ca07-4440-9da9-db8d20c782da`).
- Reused nextest and the full screenshot run at `77eac204` after renaming a private method to clear Clippy. Its
  implementation and only caller behave the same.
- The checks used the Linux renderer and fake Helm. The checkout tests verified accessible status and detail metadata,
  and source review checked the pinned Linux and macOS adapter paths. Native screen-reader speech, macOS screenshots and
  real remote launches were outside this layout check.

## Review gate

Sol reviewed the implementation and the behavioral follow-ups, including preview authority and polite Folder
announcements. Opus reviewed all three affected scenes in light and dark. Its second review confirmed the branch mark
alignment and approved the result.

### Landing

#### What landed with it

No other plan landed in this round. The plan's PR, #4603, landed together with a landing fix, #4609. Since the plan's
work was written, main had gained recipe credentials in the launch environment and the desktop startup helper refactors;
neither touches the launcher, and the rebase had no conflicts.

An independent reviewer read the change against the plan's acceptance criteria and Decisions. It found no blocker:

- **As decided.** All nine acceptance criteria are met. The two changed suspended-host screenshots go beyond the
  acceptance criterion that no existing reference screenshot changes, but that scene shows a managed checkout and was
  added after planning; the report says so.
- **Behavior.** The Folder line and the Launch button use the same folder preview, so the line cannot show a stale path
  from an earlier connection; the suspended-host message carried over; with the folder path now plain text instead of a
  focusable field, the setup field comes before Host in tab order, which matches what is drawn.

#### Fixes made while landing

All in [#4609](https://github.com/scode/farhelm/pull/4609/changes):

- **No new dependency.** The plan made the GUI crate depend on AccessKit directly, to mark the Folder line so screen
  readers announce its updates without interrupting. GPUI already provides the AccessKit it uses, so the launcher now
  takes it from there, and the GUI crate's dependencies and the lockfile are back to what main had.
- **Two screenshot titles** in the GPUI scene list still described the old read-only folder field.

#### Checks

The plan changes the GUI launcher's layout, its reference screenshots and, as delivered, the GUI crate's dependencies,
so the landing ran the GUI crate's full test suite (3312 tests, wider than the report's 245 focused tests), the lint
runs, the end-to-end smoke test, every reference screenshot and the dependency checks. The worker runs were on a
separate arm64 Linux test worker, the rest locally on x86_64.

On the plan as delivered, rebased onto main:

- `cargo nextest run -p farhelm-gui`, on the test worker (recorder run `733036ee-e419-41cc-b016-49d9ff11a19f`): all 3312
  passed.
- `cargo clippy --all-targets -- -D warnings` and both desktop app clippy runs (with and without `--features agent`), on
  the test worker, not through the recorder: passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `4229eba2-c39b-4481-929d-df4fa01a99f9`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `ccc16a5a-3f07-4d2c-a4cb-b450c6a7f71a`):
  passed, the new and updated launcher scenes included.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `python3 -B scripts/check-crate-layering.py`,
  `cargo-deny --locked check licenses sources`,
  `python3 -B scripts/dependency-notices.py --check --only-if-inputs-changed-since origin/main`,
  `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its isolated environment), `dprint check`
  and `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4609):

- `cargo nextest run -p farhelm-gui`, on the test worker (recorder run `0b76e4a9-fc76-4b98-b533-a8593cac9052`): all 3312
  passed.
- `cargo clippy --all-targets -- -D warnings` and both desktop app clippy runs, on the test worker: passed.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `cargo fmt --all -- --check` and `dprint check`, locally: passed.

The fix only changes how one line names AccessKit and two scene titles, so the smoke test, the reference screenshots and
the dependency checks above still apply; the GUI crate's dependencies now match main's.

Skipped: the Rust workspace suite outside the GUI library, since nothing else changed; the desktop app's own tests,
since the desktop app's process is untouched; macOS and native screen readers, since no Mac was part of this landing;
CI, which runs only on demand.

#### Things you should know that the report does not say

- The report quotes the new-environment Folder wording, but not that the same wording now also replaces "Select a
  repository" before any repository is chosen. That case came with the planner's layout, not a decision of yours, and
  the interaction tests do not cover it; a follow-up could show "Select a repository" again until one is picked.
- Other status lines in the app that change as you fill in a form still lack the marking that makes screen readers
  announce updates without interrupting, which the Folder line now has; the review queue already tracks this.
