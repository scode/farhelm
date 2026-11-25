## What this was about

The session launcher offered GPT-5.6 Terra even after Codex began interrupting startup with a retirement prompt. Farhelm
also showed a blanket warning that Goose conversations reopened by ID would never be eligible for Restart. The CLI
treated `-fast` in `--effort -fast` as an unknown flag. The plan removes the retired suggestion, corrects the Goose
guidance, and accepts leading hyphens in custom effort values.

The two OpenCode checks matched workspace paths differently. A symlink could hide the test's own session, and the
initial-prompt check could report that it could not run even after a successful turn. Separately, the fixed list of
instructions checks could silently miss an agent that gains the pointer. Both changes are confined to tests.

## Things you should know

- Saved launches retain their explicit model choice. A template or CLI request that relied on GPT-5.6 Terra to pick the
  agent now needs the intended agent explicitly, such as `--agent codex` for Codex's `gpt-5.6-terra`. Farhelm can pass
  the literal model ID. Codex can still show the retirement prompt, and other providers may refuse the model.
- Reopening a Goose conversation by ID can make **Restart** available after Farhelm accepts Goose's report of the
  reopened conversation's identity. That report needs the reporter saved when Farhelm originally started the
  conversation. Reopening an outside conversation leaves **Restart** unavailable. On **Restart with**, Farhelm clears
  the previous target before launch so a subsequent **Restart** cannot reopen the wrong conversation. If the new agent
  definitively failed to start, Farhelm restores the previous target. An uncertain start leaves **Restart** unavailable
  until Farhelm accepts a report for the new conversation. The **Start a session** and **Stop, restart or resume**
  guides cover named Goose launches.
- Custom effort values such as `--effort -fast` now work. As with `--model`, the next token is the value even if it
  looks like a flag. The helm still validates the value against the selected agent.
- Both OpenCode drift cases identify the test's own workspace through the same lookup, including when the workspace has
  a symlink. Missing and ambiguous matches after a successful turn are failures. The instructions check now derives its
  expected live case list from what the launch actually sends when instructions are toggled.

## Open questions and possible follow-ups

None.

## PRs

- Remove GPT-5.6 Terra suggestions. [#4945](https://github.com/scode/farhelm/pull/4945/changes)
- Correct named Goose Restart guidance and verify recall. [#4951](https://github.com/scode/farhelm/pull/4951/changes)
- Accept hyphen-prefixed custom effort values. [#4954](https://github.com/scode/farhelm/pull/4954/changes)
- Share the OpenCode drift session lookup. [#4956](https://github.com/scode/farhelm/pull/4956/changes)
- Derive the instructions-delivery test inventory. [#4959](https://github.com/scode/farhelm/pull/4959/changes)

## Checks run, reused and skipped

- Catalog, launcher and managed checkout suites passed 85 selected cases at `61179383`, run
  `298cef1b-4767-41dc-a717-877f0383a489`. The command was
  `cargo nextest run --locked -p farhelm-helm -p farhelm-proto -p farhelm --lib --test e2e -E 'test(/^launches::/) | test(/^launcher::/) | test(/^github_checkouts::/)'`
  through the recorder. The review follow-up only changed the release note and the historical-template test. That test
  passed on `535b2714` with
  `cargo nextest run --locked -p farhelm-helm --lib -E 'test(=launches::tests::a_retired_catalog_model_requires_an_explicit_agent)'`,
  run `b80b075f-a031-491b-90fd-4f6c0d63f745`.
- The corrected Goose offline suite passed all 157 selected cases at `b8179c3e`, run
  `6559b53b-5bdf-4b27-9d10-1608e1850e6a`. The recorder ran
  `cargo nextest run --locked -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm-gui -p farhelm --lib --test e2e -E 'test(/^launches::/) | test(/^launch::/) | test(named_conversation) | test(named_goose_restart) | test(prompt_and_conversation_rows_follow_the_agents_capabilities)'`.
  The first candidate's e2e test used the wrong error type and did not compile. That failing run is
  `861d28d8-8e9a-430a-a7a4-89df252b9b67`.
- Goose 1.60.2 passed the live named Create and named Restart checks at `b8179c3e`, run
  `5e4a1a9a-6979-4552-831f-451781f66175`, via
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category named_conversation --harness goose`. Both fresh
  replies and accepted reports matched the reopened conversation. The offline supervisor case verifies when the old
  capture is cleared.
- The CLI passed both selected create-intent tests at `2dcfdff6`, run `16e2c31d-2836-4555-8ddf-33c53ff68146`. The
  recorder ran
  `cargo nextest run --locked -p farhelm --bin farhelm --test agent_cli -E 'test(custom_create_) | test(effort)'`. The
  review follow-up only edited source comments.
- The 33 selected offline drift cases passed at `7bf0b424`, run `b582fa41-858e-48ea-a987-0272e8a87a37`, via
  `cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`. Both workspace lookup
  cases passed again after the error messages were revised at `a509abf6`, run `74280ff8-fd68-4312-a6bc-21c0a64536c9`,
  with
  `cargo nextest run --locked -p farhelm --test e2e -E 'test(opencode_matches_a_symlinked_test_workspace_from_either_side) | test(opencode_only_resumes_a_session_in_the_test_workspace)'`.
- The instructions-delivery pin passed on rebased `8b89763d` in the standalone supervisor suite without the `test-seams`
  feature, run `640f76ba-d9a1-472a-8354-c28261674f7c`. The recorder ran
  `cargo nextest run --locked -p farhelm-supervisor --no-default-features --lib -E 'test(=agent_kind::tests::instruction_delivery_follows_the_start_expansion)'`.
- Rust formatting, Markdown formatting on the changed documents, changelog formatting, the test delay check and diff
  whitespace checks passed on the rebased stack. `cargo clippy --locked --all-targets -- -D warnings` and
  `cargo clippy --locked -p farhelm --bins -- -D warnings` both passed.
- Rebased onto `66c6f4aa` after repository suggestions moved to the helm's checkout history. All five patches applied
  without changes. Run `aec1e43b-a06e-4de0-a738-bf766823e9e4` passed all 37 selected tests on `8b89763d`, including the
  real clone with history, replay after a lost response, named Goose Restart, the launcher warning, and the offline
  drift cases. The recorder ran
  `cargo nextest run --locked -p farhelm -p farhelm-supervisor -p farhelm-gui --lib --test e2e -E 'test(a_raw_http_fresh_checkout_completes_the_real_clone_hook_and_agent) | test(lost_fresh_checkout_success_replays_after_settings_change_and_helm_restart) | test(named_goose_restart) | test(prompt_and_conversation_rows_follow_the_agents_capabilities) | test(/^real_agent_capture::drift::/)'`.
  The earlier live Goose run and CLI parse tests still apply because the rebase preserved hook startup and effort
  parsing. Main then added Claude onboarding on recipe hosts and automatic workspace trust for disposable sessions. The
  checkout cases use a local installed host, where trust still follows its existing settings, so the focused evidence
  still applies. The updates to trust and its approval cards also leave named Goose capture and the instructions pointer
  unchanged.
- Skipped the workspace runtime battery. The targeted runs cover the changed launch and drift contracts. The live Codex
  retirement probe was conditional on Farhelm still offering the model. Skipped the live OpenCode cases because the
  offline tests exercise the shared workspace lookup. Skipped the six live instructions cases because the change
  measures coverage from the existing launch behavior. The launcher warning was checked in the GUI fixture.

## Review gate

All five PRs passed the Opus 5.5 source review gate at high effort. The Goose follow-up confirmed the fixes for the
compile error, stale help and reused reply token. The final coverage fix lets the capability test run in a standalone
supervisor suite and reports a fixture error if either launch expansion refuses its inputs. No source review findings
remain open.
