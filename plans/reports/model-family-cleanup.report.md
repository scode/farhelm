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

### Landing

#### What landed with it

Two other plans landed in the same round, one just before this one and one just after: antigravity-workspace-trust (an
Antigravity workspace trust switch and Waiting for its trust question, protocol 82) and template-placeholder-validation
(launch templates refuse misplaced command placeholders when saved). Since this plan's work started, main had gained
only the Add host takeover wording fix and two TODO entries. An independent reviewer and I read the three plans' changes
against each other. The other two change different parts of the same launch code, and nothing they do depends on the
model family names. This plan stopped sending a separate shortcut word with each model in the launcher's list; that list
goes only from the helm to the app built with it, so dropping the word needed no protocol change.

#### Fixes made while landing

One went in as a second PR, [#4363](https://github.com/scode/farhelm/pull/4363/changes). The docs page "Start a session"
listed the Model field's shortcuts as exactly seven words. Every family's name works now, so the page gives examples
instead of a list.

#### Checks

This round landed three plans together: antigravity-workspace-trust, model-family-cleanup and
template-placeholder-validation. Between them they change the protocol (to 82), the helm's database (to version 93), how
the supervisor reads Antigravity's screen, the helm's launches, templates and preferences, the desktop app (Settings,
the note above the terminal, the launcher's model field and search, the template editor), screenshots, the docs website,
SPEC.md and SPEC_impl.md. So the landing ran the whole workspace suite, the desktop app's tests, the app's end-to-end
smoke test and the main lint (clippy) on a separate arm64 Linux test machine, and the screenshot comparisons, the
website build and the small checks locally on x86_64. All of these ran on the three plans stacked together on main.

On the plans as delivered:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine
  (`8bcf674c-ac40-4153-932e-8c1088275db5`): all 8182 tests passed, and no test reported skipping itself because the
  machine lacked something it needs (a systemd user session, ssh to itself, Podman).
- `cargo nextest run -p farhelm-desktop`, on the test machine (`6780e911-a7be-4097-ba35-431188b6f40b`): all 180 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine (`084e7104-a303-472d-8b1b-04d8906d56b6`): every step passed
  but paste, which skips on Linux as it always does.
- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py` (the screenshot comparisons), locally (`2980a7fb-c943-402e-99cf-5ac085118d90`):
  passed, with Antigravity's two new Settings pictures.
- `python3 -B scripts/test-gui-screenshot-batch.py` (which validates the screenshot list), the website build
  (`bun run build`), `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format` (formatting, test and changelog lints), locally: passed.

After the landing fixes, #4362 for Antigravity (one function in the app's Settings code, which still shows the same help
text, plus SPEC.md, SPEC_impl.md and a changelog fragment) and #4363 for model-family-cleanup (a docs page):

- `cargo nextest run -p farhelm-gui`, on the test machine (`c609aff2-d2f1-4346-9cb8-16f37e4a1266`): all 3226 passed.
- `cargo clippy --all-targets -- -D warnings`, on the test machine: passed.
- The website build, `cargo fmt --all -- --check`, `dprint check` and `python3 releasing/check-changelog.py format`,
  locally: passed.

Reused for the fixes: the workspace suite, the desktop tests, the smoke test and the screenshot comparisons, since the
Settings help shows the same text and nothing else that runs changed. Skipped: Mac runs, since no macOS-specific code
changed; the Antigravity note's devbuild check covers the Mac by hand.

#### Things you should know that the report does not say

With an agent already chosen, typing `grok` in the launcher's search box now looks only for Grok models on that agent:
the plan required that a word naming both an agent and a model family mean the family once an agent is chosen. Before
this plan, `grok` offered the Grok agent whatever was chosen. Grok models are offered by agents such as Pi, where `grok`
picks Grok 4.6 as the report says. But with Codex or Claude chosen, which offer no Grok model, `grok` shows no row, only
the warning that no model matches and that you can enter a custom model ID in the model field. And with the Grok agent
itself chosen, which has no model choice at all, the same warning points at a model field it does not have.
`harness:grok` still finds the agent. The landing left this as the plan specifies. A follow-up could keep model-only
search for agents that offer the family and let the agent row win otherwise, and fix the warning for agents without a
model field.

#### What you should still do

Nothing beyond the report. If the `grok` behavior above is not what you want, ask for that follow-up.
