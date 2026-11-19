## What this was about

The November 18 triage settled nine reports about the terminal, credential entry, template shortcuts and disposable
environments. The maintainer chose to discard these nine reports for the reasons recorded in the triage ledger.

## Things you should know

Each PR removes one report from the active review queue and moves the corresponding decision to its archive. The
archived decisions preserve the assessment, the maintainer's choice and a link to the PR that carried it out. These are
bookkeeping changes.

The active environment-history report remains queued. Both separate reports about confirming a tab's terminal host
during session deletion remain queued. Those three reports were outside the duplicate discards approved here.

## Open questions and possible follow-ups

Some links in the historical review collection still point to the removed queue entries. The original events are still
in the historical collection. Use the archived decisions to find a disposition and its PR. Editing the historical
summaries would change the collected record.

## Pull requests

- Copying a terminal selection can include characters the program concealed. The maintainer accepted that behavior for
  selection, which copies the text stored in the selected cells.
  [#4157](https://github.com/scode/farhelm/pull/4157/changes)
- Operating-system text services can request the contents of a masked field. The maintainer chose to focus this round of
  credential fixes on logs and the app's visible surfaces. [#4158](https://github.com/scode/farhelm/pull/4158/changes)
- Saving a Templates number-key change while the window has an older set of bindings can overwrite an intervening edit
  to another key. Triage accepted last-save-wins for these bindings.
  [#4161](https://github.com/scode/farhelm/pull/4161/changes)
- The report warned that cleanup could choose a different tmux target after the original vanished. Farhelm now cleans up
  through terminal hosts and verifies the process it ends. [#4162](https://github.com/scode/farhelm/pull/4162/changes)
- Removing tests for the old terminal tabs looked like a gap in coverage. Their removal shipped in the same release as
  retirement of the tabs those tests covered. [#4164](https://github.com/scode/farhelm/pull/4164/changes)
- The report raised the possibility that removing a browser clipboard classifier exposed clipboard reads. The current
  terminal emulator drops programs' clipboard read requests. [#4165](https://github.com/scode/farhelm/pull/4165/changes)
- An environment created by an agent can change the launcher's remembered choices and push recent setups out of its
  history. The active environment-history report covers that behavior.
  [#4166](https://github.com/scode/farhelm/pull/4166/changes)
- The earlier Retry label suggested a connection attempt even when the action continued a deletion. The action now says
  “Finish the deletion,” and the helm accepts it after an already confirmed deletion failed.
  [#4167](https://github.com/scode/farhelm/pull/4167/changes)
- The report asked whether deleting a session could leave a terminal tab's host process running after its socket
  disappeared. The investigation found no shipped case, and two active reports still track process confirmation in
  related paths. [#4168](https://github.com/scode/farhelm/pull/4168/changes)

## Checks

- `dprint check TRIAGE_OUTCOMES.md TRIAGE_OUTCOMES_ARCHIVE.md review_feedback_queue/INDEX.md` and `git diff --check`
  passed for each final draft and the combined stack. Every patch removes exactly one finding and its index item, and
  preserves the triage entry apart from the Execution field. `python3 scripts/check-agents-md.py` passed with the
  pre-existing warning about an archived citation. The checked stack tip is `7d444b63`.
- Main advanced to `ffe12567` with two other plan deliveries. Those commits touch the plans queue and reports. The
  checked triage files still match the stack's base.
- Runtime tests and hosted CI were skipped because the drafts touch only the review queue and triage ledger.

## Review gate

Every draft received a fresh independent patch review and a title cold read. The patch reviews confirmed the exact moves
and preservation of related reports. I kept the prescribed scope where review found references to the removed entries in
historical summaries. The final PR numbers, heads, bases and draft states were checked against GitHub.

### Landing

#### What landed with it

It landed in a round of nine plans: session-group-follow-ups, model-families, session-limit-notice,
reserved-resource-names, secrets-logs-and-screen, checkout-settings-stale-save, concealed-link-target, triage-misc-nov18
and triage-discards-nov18. Since its base, main gained the previous round's plans (submenus that open on hover, Clone
keeping invisible characters, the retired old-terminal announcement, quick switcher screenshot folders, the update
survival wording) and docs screenshot tooling. concealed-link-target, in this round, matches the discard about
selections copying concealed text. The triage records came through stacking nine plans that each edit them with nothing
lost or duplicated, which a reviewer checked entry by entry.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The bookkeeping is right: each
discarded entry moved to the archive unchanged apart from its Execution field, and each feedback file and its index line
went in the same commit.

#### Fixes made while landing

Only bookkeeping: while the plan was landing, another triage recording (#4189) reached main and conflicted with the last
PR's change to the triage ledger. I resolved it by keeping both, and checked the ledger and the review queue's index
against the queue's files again before that PR merged.

#### Things you should know that the report does not say

- Two items still waiting in the review feedback queue (on credential editors keeping old secret copies, and on
  secret-handling tests that never check output) say a now-discarded item might cover them. That item is gone, so
  whoever triages them should judge them on their own.
- Because several plans in the round were based before main archived other entries, the archive's newest entries are no
  longer all at its end, though its header says entries are appended in order. Nothing reads that order.

#### Checks

This round landed nine plans together, so these checks cover all nine on one tree: the nine stacks one after another on
main. Between them the plans change the desktop app's GUI crates, the helm and its client, the shared protocol, the
supervisor, the desktop process, SPEC.md, SPEC_impl.md and the triage records. The landing ran the lints, the whole Rust
workspace suite, the desktop process's own tests, the app's end-to-end smoke and the reference screenshots (goldens).
Builds ran locally; the workspace suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `python3 -B scripts/test-gui-e2e-smoke.py`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`, once without and once with `--features agent`: passed.
- `dprint check`: failed on `TRIAGE_OUTCOMES_ARCHIVE.md`, which triage-misc-nov18 wrote unformatted. Fixed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats:
  2744, 2653 and 2625 tests, 8022 in all (`1ffb9472-9f12-4890-b0d5-032dd87718d0`,
  `7c3ef63b-2e2b-4903-acff-22cbe9409601`, `7c539d75-d409-48c7-901b-391169215c11`). One failed: the macro
  compile-contract test, which builds a small crate offline from this machine's cargo cache, which a boat does not have,
  so it fails there every time; it passed locally (below). No runtime skips.
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`59017e5e-7626-4dca-9c45-0b4972a89aca`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed, the new step that
  drags an unpinned session into a group included, and the clipboard paste step was skipped, as always on Linux
  (`b1164f8b-3d55-4cac-9fb1-a76e9a8da60c`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`da0dca91-646f-483a-a436-b10b43ffafe2`), the new groups
  pictures included.

On the fixed tree (comments, SPEC.md, SPEC_impl.md, changelog fragments, the triage archive's formatting, and one
notice's wording changed):

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`: passed.
- The bell notices' tests in the GUI and the helm, locally: 26 passed (`80fb276d-1ba3-4b09-a84a-370aef2b9605`).
- `cargo nextest run -p farhelm-testtrace-macros`, locally: 2 passed, the compile-contract test included
  (`a8a9bf1f-6dc9-4e24-8831-e5d4b7f5dbc0`).

The boat suites, the smoke and the goldens were reused for the fixed tree: after them only comments, documents,
changelog fragments, formatting and one notice's wording changed, which the focused runs above cover.

Skipped:

- The Linux app lifecycle: startup and quit are untouched.
- The Mac-only checks: no Mac-specific code changed.
