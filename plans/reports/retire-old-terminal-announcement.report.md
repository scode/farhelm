## What this was about

The in-app announcement “Restart old terminals before support ends” told people on older Farhelm releases to restart or
replace their sessions before support for Farhelm’s old tmux terminals ended in v0.39.0. The announcement has been
removed from the release table so later builds stop adding it when people upgrade.

## Things you should know

If your helm already queued the warning, you can still see it after upgrading. Warnings you have already seen remain in
Announcements history.

## Open questions and possible follow-ups

None for this round.

## Pull requests

- Retire the obsolete 0.37.0 announcement and remove the completed TODO entry.
  [#4140](https://github.com/scode/farhelm/pull/4140/changes)

## Checks

- The selected Linux test passed with no runtime skip in recorder run `8eaac0ae-b5d4-4fbf-9e7d-fe20ef3363a1`. The
  recorded command was `cargo nextest run -p farhelm-helm --lib -E 'test(release_announcement_table_is_valid)'`. It ran
  against `f2fc9607` with the two-file patch applied. The draft uses the same patch on newer main. The intervening
  release-note and plan-queue changes affect neither the announcement table nor the helm test.
- `dprint check TODO.md crates/farhelm-helm/src/release_announcements.json` and `git diff --check` passed.
- `git grep -n -F '0.37.0-old-terminal-support'` found only references in the plan after the edit.
- Skipped the full test suite because this change removes one table entry and the focused test reads the compiled table.

## Review gate

Independent code review found no issues. The commit message passed a fresh cold read after clarifying the version and
which copies of the warning are affected.

### Landing

#### What landed with it

It landed third in a round of five plans, after submenus-open-on-hover and clone-keeps-invisible-characters and before
quick-switcher-shot-paths and update-survival-wording. Since its base, main gained the launcher's shared destination
controls, docs screenshot and README hero changes, a docs refresh and bookkeeping; none touches release announcements.

update-survival-wording, in the same round, records that Farhelm's general statements about sessions surviving updates
deliberately carry no exception for the update that ended old terminals. That agrees with this plan: someone updating
straight from an old release still gets each affected session's own notice that its old terminal ended, with its docs
link, and the announcement removed here would only have arrived after that update anyway.

Reviewed with extra skepticism at your request, by me and an independent reviewer: nothing to change. The removed
announcement is the v0.37.0 one asking users to restart old tmux-era terminals before support ends. The table is now
empty, which the helm's loader and its test accept, and nothing else refers to the announcement.

#### Fixes made while landing

None.

#### Checks

This round landed five plans together, so these checks cover all five on one tree: the five stacks one after another on
main. The plans change the desktop app's menus, launcher and quick switcher, the helm's release announcement table,
SPEC.md and documents. The landing ran:

- the lints and source checks;
- the whole test suites of the desktop app's GUI crate and of the helm;
- the app's end-to-end smoke;
- the reference screenshots (goldens);
- a hands-on run of the app, with screenshots of each submenu step (kept out of the repository).

Builds ran locally. The test suites and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered (main had gained only bookkeeping, the docs screenshot tooling, a docs refresh and the
launcher's shared destination controls since their bases):

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`: passed.
- `cargo nextest run -p farhelm-gui -p farhelm-helm`, from a locally built archive, split in three across boats: 1655,
  1603 and 1597 passed, 4855 in all (`1925a98b-8cce-47e4-ad2a-42f7d3d355ce`, `11bd4bb4-ae5f-45e9-a5a9-9be345c3a126`,
  `8a6fa767-830d-4071-a283-f25dea7661a1`). Three helm tests skipped at runtime because the archive does not carry a full
  `farhelm` binary: provisioning over SSH and over the local transport, and the uninstall stop. None touches these
  plans; that coverage is missing from this run.
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 48 of its 49 steps passed and the clipboard
  paste step was skipped, as always on Linux (`9efe756f-4b33-4fe0-bd9a-02bffc4b3bdc`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`061c16ed-d3c3-4664-85d3-1d3340c34326`).

On the fixed tree (comments, SPEC.md, documents and two tests changed):

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 scripts/check-agents-md.py` and `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`: passed.
- `cargo nextest run -p farhelm-gui --lib` for the session menu, submenu, Clone entry and quick switcher tests, locally:
  108 passed (`56e7255b-5924-40ff-bee9-03f8cf0bbe7f`), the two changed tests included.

The boat suites, the smoke and the goldens were reused for the fixed tree: after them only comments, SPEC.md, documents
and two tests changed, which the focused run above covers.

Skipped:

- The rest of the Rust workspace and the desktop process's tests: the plans stay in the GUI crate and the helm.
- The Linux app lifecycle: startup and quit are untouched.
- The Mac-only checks: no Mac-specific code changed. The submenu behavior was checked on Linux only.
