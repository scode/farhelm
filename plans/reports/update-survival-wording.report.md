# Session survival wording during the old-terminal update

## What this was about

Farhelm's usual update message says sessions keep running. The one-time migration that retires old tmux terminals ends
work still running in those terminals. Three review findings flagged the risk of leaving work open based on the usual
reassurance. The decision to keep the usual session-survival message was recorded in the commit that restored it.

## Things you should know

The maintainer chose to describe how the product works in general and record narrow exceptions where they apply. The
specification now states that principle under Documentation scope. In the same specification, the Old terminals bullet
under Terminal experience records the choice to use the usual survival wording during this migration, including the
installer message. The three related triage decisions are now in the archive with the link to their implementation.

## Open questions and possible follow-ups

None.

## PRs

- [#4145](https://github.com/scode/farhelm/pull/4145/changes) documents the wording decision and records the three
  completed triage outcomes.

## Checks

- `dprint check SPEC.md TRIAGE_OUTCOMES.md TRIAGE_OUTCOMES_ARCHIVE.md review_feedback_queue/INDEX.md` and
  `git diff --check` passed at `ccf054e60b9d`.
- `python3 scripts/check-agents-md.py` reported zero errors and one pre-existing warning about an archived citation to
  the “Bound polling time” heading.
- Used fresh source checks. Skipped runtime tests because this change only edits the specification and review
  bookkeeping.

## Review gate

A fresh gpt-6.2-sol reviewer at high effort passed the final draft with no findings. The reviewer checked both additions
to the specification and verified that the three archived entries preserved the original text except for their Execution
fields. A separate cold reader checked the commit and PR wording.

### Landing

#### What landed with it

It landed fourth in a round of five plans, after submenus-open-on-hover, clone-keeps-invisible-characters and
retire-old-terminal-announcement; quick-switcher-shot-paths landed after it, once a conflict with a screenshot tooling
change that reached main during the round (#4141) was resolved. Two of those also edit SPEC.md, in other sections (the
menu paragraphs and "Command launch"); the three edits do not contradict each other. retire-old-terminal-announcement
removes the announcement that warned about ending old terminals, which agrees with what this plan records. Since its
base, main gained only work that does not touch these statements.

Reviewed with extra skepticism at your request, by me and an independent reviewer. The bookkeeping is right: the three
triage entries moved to the archive unchanged apart from their Execution fields, and their feedback files and index
lines are gone.

#### Fixes made while landing

The plan's two SPEC.md sentences were weaker than your decision:

- **The principle.** It read only that general statements "keep their ordinary wording". It lost your point: a narrow,
  separately specified exception does not make them wrong and is not a reason to qualify them everywhere. That is
  exactly the misreading behind the three review findings, which asked for those statements to carry an exception for
  the update that ended old terminals. It now says that, and quotes you as the paragraph beside it does: "These are
  general principles and statements about how the product operates, not a lawyer's contract."
- **The note in "Old terminals".** It said Farhelm "deliberately keeps the general statements", without saying which
  statements or that they carry no exception for this update. It now names them and says so: SPEC.md's "Restart to
  update" and "Durability and resume" sections, and the installer's closing line after an update, which the installer
  also prints during that update.

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
