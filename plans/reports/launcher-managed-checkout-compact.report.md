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
