## What this was about

Farhelm used the same model list for suggestions and for recognizing a typed ID. When a new model replaced Sol or Astra
in that list, Farhelm forgot which agent offered the older ID. Four saved-template names had a special case, so a
template could choose the agent even when typing the same ID left you to choose it yourself.

The catalog now keeps earlier IDs with one latest model per family and agent. Typing an older ID, applying a saved
template or passing it as `--model` can fill in a missing agent when exactly one agent offers it. The launcher and
template editor put the latest choices first and older choices under **Older models**.

## Things you should know

- The older Sol and Astra IDs use the efforts Farhelm accepted in v0.41.0, including Low and Medium for the older Codex
  Astra. If several agents offer exactly the same ID, choose the agent explicitly.
- An older ID can still be used as a custom ID on another explicitly chosen agent. That preserves templates and past
  launches that already used the spelling. If you've used an older model before, search labels that result as a past
  launch so it is distinguishable from the catalog result.
- Headings stay out of the launcher's arrow-key order. The template editor shows older suggestions for its chosen agent.

## Open questions and possible follow-ups

The separate TODO for typing a family name such as Sol can use the family and latest model information in this catalog.

## Pull requests

- Recognize older Sol and Astra IDs across launcher, template and command-line choices.
  [#4149](https://github.com/scode/farhelm/pull/4149/changes)
- Offer older model choices in the launcher and template editor.
  [#4152](https://github.com/scode/farhelm/pull/4152/changes)

## Checks

- The first patch passed 870 selected Linux tests with no runtime skips in recorder run
  `0a7e2633-dd8f-4bf8-ab5a-8b29ff433625`. The recorded nextest command used
  `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-gui --lib` with the proto launcher module, helm
  compiler and CLI flag tests, and desktop launcher and template modules. The test for the catalog route was filtered
  out by its module name. Recorder run `66536a0f-9b31-4f1c-b1d1-7280e4481570` ran and passed it separately, on the same
  command with
  `-E 'package(=farhelm-helm) & test(=sessions::tests::composer_catalog_and_history_routes_serve_helm_owned_choices)'`.
- All 251 selected final desktop tests passed with no runtime skips in recorder run
  `8138cf03-f803-4494-a8ed-dc906a68f2b0`. The recorded command used `cargo nextest run -p farhelm-gui --lib` with the
  launcher model, search, view, folder, recent and environment view tests and template application and editor tests. The
  run covered the rendered headings, filtered rows, keyboard picking and template editor buttons.
- `cargo clippy --locked -p farhelm-proto -p farhelm-helm -p farhelm-gui --all-targets -- -D warnings` passed. The Linux
  GPUI screenshot comparison passed in recorder run `b9a4bfc4-448d-4a87-949a-71c12f4d2fe8` with
  `python3 scripts/check-gui-goldens.py`. The new picker states are covered by the focused desktop tests.
- `cargo fmt --all -- --check`, `dprint check` on changed Markdown, `python -B scripts/check-test-sleeps.py` in its
  isolated environment, `python3 releasing/check-changelog.py format`, and `git diff --check` passed.
- The two initial successful runs used main `e0c6670c` plus the first patch. Those changes were identical after rebasing
  onto newer main. The new base's changes to launcher and editor UI were covered by the final GUI tests. Helm and proto
  source were unaffected by that rebase and the second patch, so their test results still apply.
- Skipped the full workspace suite. The affected behavior is exercised by the catalog, launch and desktop checks.

## Review gate

Both PRs passed independent code review and cold reads of their commit messages. The first review caught a past-launch
selection that could lose its explicitly chosen custom model and an outdated spec sentence. The second caught a missing
test selector and identical labels for history and catalog results. The fixes preserve the custom model, clarify the
rules, and test heading placement and navigation in the launcher and template editor.

### Landing

#### What landed with it

It landed in a round of nine plans: session-group-follow-ups, model-families, session-limit-notice,
reserved-resource-names, secrets-logs-and-screen, checkout-settings-stale-save, concealed-link-target, triage-misc-nov18
and triage-discards-nov18. Since its base, main gained the previous round's plans (submenus that open on hover, Clone
keeping invisible characters, the retired old-terminal announcement, quick switcher screenshot folders, the update
survival wording) and docs screenshot tooling. The previous round's change to the launcher's text fields leaves the
model field alone, and the reviews found no interaction with this plan.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The old special case that
hard-coded four retired Sol and Astra model IDs is gone everywhere it was. The rule that replaces it is a little
asymmetric: with an agent chosen explicitly, a model ID that is another agent's current model is refused, but another
agent's older model ID is still accepted as a custom model. Your requirement that every template that worked in 0.41.0
keeps working needs that. The headings in the launcher's model list use one shared heading that replaced two duplicate
versions. I did not look at the launcher's model list on screen this round; the tests check the headings stay out of the
arrow-key order.

#### Fixes made while landing

- **The changelog fragment claimed too much.** It said a model ID only one agent offers picks that agent for you, as if
  that were new; it already worked for current models. It now says older IDs behave like current ones.
- **Code comments gave the wrong reason.** The new catalog fields' defaults were described as compatibility with older
  helms. The desktop app only ever talks to its own build's helm; the defaults keep older test data valid, which the
  comments now say.

#### Things you should know that the report does not say

- **Family names are not spelled consistently.** Sol, Astra, Luna and Terra use plain names (`sol`), Claude and Muse use
  the full model ID (`claude-fable-5`), and Grok and GLM include the version (`grok-4.5`), as the plan asked. When a
  successor ships, those last ones must be renamed, and the planned "typing a family name picks its latest model"
  follow-up could not resolve `fable`. Nothing reads the family yet, so changing it now costs nothing; that follow-up is
  the natural place to decide.

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
