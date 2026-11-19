# Farhelm's reserved resource names and labels

## What this was about

Three review findings concerned sandboxes or containers created outside Farhelm. Reusing a name Farhelm reserves, or
using a Podman clone with copied Farhelm ownership labels, can put independently created work in scope for cleanup. The
maintainer decided that Farhelm may treat those resources as its own.

## Things you should know

The specification now explains the ownership rules for each operation. Tensorlake leftover deletion asks for
confirmation. During recovery of an outstanding create with no saved sandbox ID, Farhelm can identify the sandbox by its
exact recorded name. Delete of a retired Podman session uses its legacy container name. Podman leftover cleanup checks
both the helm tag and the container role label in the host's Unix account and asks before deleting a leftover. The
change is documentation and bookkeeping. The three related decisions are in the archive with their PR links.

## Open questions and possible follow-ups

None.

## PRs

- [#4148](https://github.com/scode/farhelm/pull/4148/changes) documents the reserved ownership rules and records the
  three completed triage outcomes.

## Checks

- `dprint check SPEC.md TRIAGE_OUTCOMES.md TRIAGE_OUTCOMES_ARCHIVE.md review_feedback_queue/INDEX.md` and
  `git diff --check` passed at `355ccbe9e860`.
- `python3 scripts/check-agents-md.py` reported zero errors and one pre-existing warning about an archived citation to
  the “Bound polling time” heading.
- Used fresh source checks. Skipped runtime tests because the change edits the specification and review bookkeeping.

## Review gate

A fresh gpt-6.2-sol reviewer at high effort read the specification and triage diff. The new paragraph now specifies the
different Tensorlake deletion and recovery conditions, the Podman label pair, and the scope of Delete for a retired
session. The reviewer checked those corrections and the three archived entries. A separate cold reader checked the
commit and PR wording.

### Landing

#### What landed with it

It landed in a round of nine plans: session-group-follow-ups, model-families, session-limit-notice,
reserved-resource-names, secrets-logs-and-screen, checkout-settings-stale-save, concealed-link-target, triage-misc-nov18
and triage-discards-nov18. Since its base, main gained the previous round's plans (submenus that open on hover, Clone
keeping invisible characters, the retired old-terminal announcement, quick switcher screenshot folders, the update
survival wording) and docs screenshot tooling. triage-misc-nov18, in this round, hides containers whose first launch is
still running from the leftovers list; that agrees with what this plan records about Podman leftovers. The reviews found
no other interaction.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The change is documentation and
bookkeeping, as planned; the three triage entries moved to the archive correctly.

#### Fixes made while landing

- **SPEC.md's new paragraph buried the decision.** It opened with "may manage and delete … under the existing cleanup
  rules", went through six sentences of mechanics, and stated your decision (resources that reuse Farhelm's names or
  labels are treated as Farhelm's) only at the end. It also restated the owning sections' conditions in slightly
  different words, and it put "deleting a leftover requires confirmation" next to sandbox recovery, which asks for
  nothing. It now opens with the decision, lists the reserved names and labels, uses the owning sections' wording, and
  says plainly that recovery does not ask.

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
