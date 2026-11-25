# Give What's new the session launcher's look

## What this was about

The upgrade dialog previously grouped highlights under category headings and showed one count across all releases. The
maintainer chose the launcher sibling design and asked to keep every entry in full. The new dialog shows the upgrade
range, highlights by category, and the date and total changes for each release. Each entry has its category mark and the
full text, with a separate headline when release curation supplies one.

## Things you should know

Release curation now proposes a headline for each highlight. For wording already supplied by the maintainer, the release
draft keeps that wording while the headline awaits approval. The format checker requires headlines only in new finalized
sections. Before landing the first PR, confirm the cutoff still matches the newest section on main, including any
in-progress draft. At delivery it is v0.45.0.

The Screenshot link stays on a separate line at the end of its entry in both appearances, so it remains independently
focusable when the paragraph wraps. The version pill uses the launcher's terminal font in both appearances. The visual
review found these details fit the written design and accepted both images. The dialog continues to open and dismiss
under the same startup, focus and saved-preference rules.

## Open questions and possible follow-ups

For a future planning-system change, the planning questions page could propose a headline when it proposes a plan's
release-note entry. The existing release review is where that suggestion is made today.

## Pull requests

- Curated headlines for new finalized highlights. [#5004](https://github.com/scode/farhelm/pull/5004/changes)
- The redesigned What's new dialog and both appearance references.
  [#5008](https://github.com/scode/farhelm/pull/5008/changes)

## Checks run, reused and skipped

- `python3 releasing/check-changelog.py --self-test` through the recorder passed, including draft and historical
  compatibility and the finalized headline rule. Run `703e39d7-de20-49c7-a88f-c7411a39db1e`.
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and `git diff --check` passed.
  The instructions check reported one pre-existing archive citation warning.
- `cargo nextest run --locked -p farhelm-gui --lib -E 'test(=assets::tests::every_icon_resolves_to_bundled_svg_bytes)'`
  passed through the recorder, run `320c9af6-10b0-40d0-a3d2-96f22658ed7b`. The pinned nextest selection
  `cargo nextest run --locked -p farhelm-gui --lib -E 'test(/whats_new::|session_list::rename|session_list::quick_switcher|launcher::view_tests::/)'`
  passed 153 cases, run `9ccbbbc0-17a1-4172-83b5-1f910d003ce1`. Both used the repository's four-slot budget and no
  retries.
- `python3 scripts/check-gui-goldens.py --update --scene whats-new-dialog --jobs 1` and the full
  `python3 scripts/check-gui-goldens.py --update` both ran through the recorder, runs
  `2b912038-23e4-44bc-b49d-206a5b6ddd1d` and `c020a9d6-1829-4886-9843-714565223621`. All 540 Linux reference images were
  accounted for. A byte-level comparison with the previous manifest found exactly the two reviewed What's new changes,
  and the full run produced the same bytes as the selected captures.
- `cargo fmt --all -- --check`, touched-file `dprint check`, and `python -B scripts/check-test-sleeps.py` with the
  isolated interpreter passed. Whole-tree dprint flagged the untouched triage archive.
- Focused and full render checks cover window revision `2e81b8c1f26ee594e204bd74c52e9ee8ee530c9c`. The later reference
  manifest update preserves their coverage. The release checker and window fixtures cover the changed paths, so broader
  workspace, native Mac updater, and live release tests were skipped.

## Review gate

Independent Opus 5.5 high source reviews covered both steps. The window review led to one shared Enter-key chip for the
primary actions and to restored heading navigation, per-release change totals and spoken version ranges. A separate Opus
5.5 high visual review checked the light and dark dialog against direction 2 and accepted both.
