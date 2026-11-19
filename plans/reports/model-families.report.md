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
