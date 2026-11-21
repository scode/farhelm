# Status icon hover outlines

## What this was about

Hovering a timestamp or agent icon in the session list made it look clickable. The maintainer wanted those icons to keep
their help and the notch that points to the icon. The outline should signal that the target can be pressed. That applies
in the session list, Hosts, group headers and the host rail.

The desktop app now applies that rule to status marks wherever they appear. The session row still gets its outline when
its summary card shows. Controls such as the status dot, pin grip, notification bell and actions button keep their
outlines.

## Things you should know

One screenshot scene changed, in light and dark. The timestamp's outline is gone and the tooltip stays in the same place
with the same notch and words. The whole-row summary and the rail entry screenshots match their existing references.
Interaction tests cover selected rows and the rail's unavailable status, which have no hovered screenshots in the
catalog.

## Open questions and possible follow-ups

None.

## PRs

- [#4517](https://github.com/scode/farhelm/pull/4517/changes) — remove hover outlines from desktop status icons.

## Checks

- The recorder ran the selection below against `28e6721e`. All 40 passed with no runtime skips (run
  `9f0fe067-58f5-437d-a519-8182d3cf4b40`). The tests check that the tooltip for a selected row's timestamp and the host
  rail's unavailable indicator keeps its notch and loses its outline. They also check that a rail button and a selected
  option row keep their outlines.

  ```sh
  cargo nextest run -p farhelm-gui --lib -E \
    'test(/^(components::tests::status_mark|layers::beside_tests|session_list::summary_tests)::/) | test(a_host_list_never_read_shows_the_unavailable_status) | test(avatar_and_footer_help_follow_the_rails_current_right_edge) | test(selected_option_help_uses_the_list_edge_and_inverse_ring)'
  ```

- Full Linux screenshot update with `python3 -B scripts/check-gui-goldens.py --update` passed at `3de9113b`. It produced
  two changed images (run `56e15dba-8366-4d7c-8ab4-a425a0f0d0c8`). Full Linux reference comparison with
  `python3 -B scripts/check-gui-goldens.py` passed at `6b9030a5` (run `964c5ba7-a22a-4c11-9c73-f831fe8b203b`).
  `python3 -B scripts/test-gui-screenshot-batch.py` passed all 47 scene-list and batch checks at that revision (run
  `02e0880c-71df-40d9-b4f2-f57845752329`).
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and
  `python -B scripts/check-test-sleeps.py` passed at `28e6721e`. The checker used its pinned parser environment. At
  `3de9113b`, `python3 releasing/check-changelog.py format` and
  `dprint check SPEC.md SPEC_impl.md TODO.md docs/gpui/screenshot-scenes.json releasing/changelog.d/status-mark-hover-ring.md`
  passed. `git diff --check` passed at `6b9030a5`.
- The focused Nextest, Clippy, formatter and test-delay checks apply to the pushed revision `14d6e785` because their
  Rust sources and fixtures match `28e6721e`. The targeted formatting and changelog inputs match `3de9113b`. The final
  screenshot comparison, scene-list checks and diff whitespace check apply because the inputs and PR diff match
  `6b9030a5`. The rebase added only queue bookkeeping and other plans' reports to the base.
- Skipped the full Rust workspace suite and desktop end-to-end smoke. The focused tests exercise the component, hover
  placement and representative interaction paths. Skipped native macOS captures because this execution ran on Linux.

## Review gate

The independent source reviewer found no production defect. The review added a rail hover assertion and a selected-row
fixture check, and corrected a stale spec quotation in the summary-card test. Independent Opus visual review passed both
appearances. The reviewer accidentally overwrote two derived review previews. Both were restored and verified against
the intact renders before the references were published.
