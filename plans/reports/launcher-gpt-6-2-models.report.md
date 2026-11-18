## Since the last review

Draft PR #4099 now includes the saved-template compatibility decision recorded after the first review. The review caught
a case where the desktop launcher dropped the inferred agent. The updated draft applies both the agent and the saved
model, and includes a desktop regression test.

## What this was about

The session launcher replaces its `gpt-6.1-sol` and `gpt-6-astra` suggestions, including provider-prefixed forms, with
GPT-6.2 Sol and Astra in Codex, Goose, Pi, OMP and OpenCode. In Codex, the new Astra offers Low, Medium and High effort.

## Things you should know

Older templates that named only a model relied on the suggestions to supply an agent. When the template and launcher
both omit the agent, bare `gpt-6.1-sol` and `gpt-6-astra` still pick Codex and `opencode/gpt-6.1-sol` and
`opencode/gpt-6-astra` still pick OpenCode. Farhelm passes along the saved model ID as a custom model. This works in the
desktop launcher and through the CLI's template option. Recent launches and templates that already specify an agent
continue using their saved model through the custom-model path.

## Open questions and possible follow-ups

Actual availability of the new OpenRouter and OpenCode Zen IDs depends on the provider. As decided during planning,
validation covered Farhelm's suggestions and launch handling. It did not make vendor requests.

## PRs

- #4099 — Update the launcher model suggestions and preserve saved templates.

## Checks

- Ran the selected helm launch, session-route and shared launcher tests through nextest. All 218 passed, including the
  catalog route and the shared saved-template tests. Recorder run `f2491515-c59f-4743-8329-b1817581c2dc` covered
  snapshot `50e9180f`. The final changes after that snapshot touched only spec prose and desktop code, so this evidence
  still covers the helm and shared launcher behavior.
  `cargo nextest run --locked -p farhelm-helm -p farhelm-proto --lib -E 'test(launches::tests::) | test(sessions::tests::) | test(/^launcher::/)'`
- Ran the desktop template application tests through nextest. All 39 passed, including the regression for bare Codex and
  OpenCode Zen IDs. Recorder run `af56420b-875b-4bbd-8794-8ac9d7d3272e` covered the pushed source tree.
  `cargo nextest run --locked -p farhelm-gui --lib -E 'test(templates::apply_tests::)'`
- Ran Clippy for `farhelm-helm`, `farhelm-proto` and `farhelm-gui`, all targets with warnings denied. Passed at
  `b4f1d53e`, recorder run `e9943477-07a3-423d-bc06-5c1a7d8db894`.
  `cargo clippy --locked -p farhelm-helm -p farhelm-proto -p farhelm-gui --all-targets -- -D warnings`
- Ran Rust formatting, dprint on changed Markdown, the changelog format check, the Rust test-sleep checker and the diff
  whitespace check. All passed. The full workspace runtime suite and screenshot goldens were outside the affected
  behavior. The template tests cover the change to saved choices in the existing controls.

## Review gate

The required gpt-6.2-sol review at high effort found that desktop replay could drop the agent inferred by template
application. The draft now applies the resolved agent with the saved model and has direct desktop coverage. The reviewer
confirmed the fix and reported no remaining findings. A cold reader reviewed the commit title.
