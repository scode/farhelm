## What this was about

Only some model families had shortcuts in the desktop launcher. Picking a model the selected agent did not offer could
also produce a long message that listed every agent offering it. You chose versionless family names as shortcuts and a
shorter refusal that names the other agent when there is only one.

## Things you should know

Type a family name in the Model field to highlight its current model. Enter applies the full model ID. For example,
`grok` picks Grok 4.6 under an agent that offers it, and Grok 4.5 remains available by its exact ID under **Older
models**. With no agent selected, bare `grok` in the general launcher search still highlights the Grok agent. Use
`model:grok` there to search for the model family.

If Codex is selected, typing `fable` in Model now tells you to choose Claude. If Claude is selected and you type `sol`,
the launcher asks you to choose an agent that offers it. It leaves the current agent selected in either case.

## Open questions and possible follow-ups

None.

## PR

- Uses model family names as launcher shortcuts and shortens refusals for models offered by other agents.
  [#4353](https://github.com/scode/farhelm/pull/4353/changes)

## Checks

All 901 selected Linux tests passed with no runtime skips (recorder `0c7c8712-517a-4dc4-8d46-a9ef53d13c29`). The tested
tree matches #4353.

```sh
cargo nextest run --locked -p farhelm-proto -p farhelm-helm -p farhelm-gui --lib \
  -E 'test(launcher::) | test(launches::tests::) | test(composer_catalog_and_history_routes_serve_helm_owned_choices) | test(templates::)'
```

These checks also passed against the submitted code.

```sh
cargo clippy --locked -p farhelm-proto -p farhelm-helm -p farhelm-gui --lib --tests -- -D warnings
cargo fmt --all -- --check
dprint check SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/launcher-model-family-words.md
python -B scripts/check-test-sleeps.py
python3 releasing/check-changelog.py format
git diff --check origin/main...HEAD
```

All evidence comes from this execution. Main moved only for queue claims and a plan report.

The full workspace suite was skipped because the focused suites exercise the catalog contract, search, model field and
template behavior. Screenshot comparisons were skipped because the affected query and refusal are absent from the
existing scenes. The model field tests use the real launcher view and assert its text and applied model.

## Review

The source review passed after two search fixes. Unrelated exact model IDs keep their existing match priority. With an
agent selected, typing a bare family name stays within that agent's models even when it offers no match.
