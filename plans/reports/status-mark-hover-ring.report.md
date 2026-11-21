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

### Landing

#### What landed with it

The plan's PR merged with a landing fix, #4524, as the last of four plans landed together in this round, after the helm
and Podman triage plans and the exited-terminal scrollback fix. None of those, nor the per-agent Settings and hook setup
plan that landed on main since the executor based this one, touches the sidebar's help or its marks. The only scene this
plan changed (light and dark), the session row's hover help, is not shown by any scene the other plans touch.

An independent reviewer read the PR against the plan and the specs. It found every acceptance criterion met and no
blocker: the rule lives where status marks are drawn, so no place that shows a status mark can turn the outline back on,
and every other kind of help keeps its outline.

#### Fixes made while landing

In [#4524](https://github.com/scode/farhelm/pull/4524/changes), text only:

- **SPEC.md.** The plan left a blank line inside the paragraph on session-row hover help, which split it so that the
  next sentence's "it" no longer pointed at the hover. The paragraph is whole again.
- **Desktop parity notes.** They still named the test the plan replaced and described the session row's picture as
  showing an outline. They now name the two tests that replaced it, one for status marks without an outline and one for
  row controls with one.

#### Checks

The round landed four plans together: the helm triage, the Podman triage, the exited-terminal scrollback fix and the
status-mark hover change. The landing ran their checks once, on all four stacked on main. The Rust workspace suite, the
desktop app's tests, the lint runs and the end-to-end smoke test ran on a separate arm64 Linux test worker. The goldens,
the website build and the small checks ran locally on x86_64. The workspace run used four build jobs, as in the last
round, since runs with more had run out of memory on the worker; this round it had no memory or disk trouble.

On the four plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four build jobs, on the test worker (recorder run
  `8ba93f1d-d933-483b-a90a-eeac88311d3a`): all 8252 passed.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `890a2d33-0c71-410b-9498-3a792708a974`): all
  184 passed.
- `cargo clippy --all-targets -- -D warnings`, on the test worker, not through the recorder: failed on one lint in a new
  Podman test, fixed in #4522.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `c732c6cc-ba70-4b34-853a-fe7961b6d297`): every
  step passed but paste, which skips on Linux as it always does.
- `python3 scripts/check-gui-goldens.py`, the full comparison, locally (recorder run
  `2372fd39-11ef-4468-84d3-f1c70cdf9f7e`): failed only on the two pictures of the recipe Delete anyway question, which
  the Podman plan had captured before main's Settings sidebar changed. Fixed in #4522.
- `python3 -B scripts/test-gui-screenshot-batch.py`, locally: failed because that new scene was missing from the
  screenshot scene list. Fixed in #4522.
- `python3 -B scripts/test-check-gui-goldens.py` (recorder run `952eb6e8-4c50-4a08-bc89-870dd1358b6d`),
  `python3 -B scripts/gui-goldens.py --self-test`, `python3 -B scripts/test-docs-shots.py` (recorder run
  `6c988518-9a04-4428-b87d-f5a1d49ede50`), `python3 -B scripts/test-gui-agent.py` (recorder run
  `430497e6-e6fd-4d35-8648-4082a3b33d33`), the website build (`bun install --frozen-lockfile && bun run build`) with its
  link check, `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its isolated environment),
  `dprint check`, `python3 scripts/check-agents-md.py` and `python3 releasing/check-changelog.py format`, locally:
  passed.

After the landing fixes (#4521 to #4524):

- `python3 scripts/check-gui-goldens.py --update`, locally (recorder run `6d8fe2d8-8886-45e5-a8f7-c1902f41d10c`):
  re-rendered every picture and changed only the two Delete anyway pictures, which were then published.
- `python3 scripts/check-gui-goldens.py --scene 'settings-recipe-*' --scene 'session-row-*'`, locally (recorder run
  `9282ca4e-8230-435c-8d5d-20380af3477c`): passed.
- `cargo nextest run -p farhelm-helm --lib`, on the test worker (recorder run `2b0cfd85-3205-4a06-97c1-d028a7ae46db`):
  all 1801 passed.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and the same with `--features agent`, on the test
  worker: passed. They had not run on the first pass, which stopped at the failing lint run.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `cargo fmt --all -- --check`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

The rest of the first runs still applies after the fixes, which change only a Podman test's lint, a test comment, two
pictures and the scene list, a code comment on the helm's notices table, SPEC.md wording, the desktop parity notes and a
changelog note. Skipped: the Podman suite's opt-in real-engine and real-SSH variants, which neither triage plan needed
beyond the real Podman environment test, which ran in the workspace run on the worker (about two minutes, not skipped);
macOS, since no Mac was part of this landing and the pictures used the Linux renderer only; CI, which runs only on
demand.

#### Things you should know that the report does not say

- The code that turns the outline off for status marks lists the three positions a hover help card can take that draw an
  outline, and ignores every other. A position added later that draws an outline would make status marks show it again
  without any compile error. Listing every position explicitly would make the compiler catch that; it is a small
  follow-up, not needed now.
- The changelog note leads with "Status icons and timestamps keep their hover help", which is not what changed. Release
  curation should lead with the visible change: hovering a status icon or timestamp no longer draws an outline, so
  outlines now mark only things you can press.
