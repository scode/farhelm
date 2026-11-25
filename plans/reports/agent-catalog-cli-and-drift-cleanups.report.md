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

### Landing

#### What landed with it

Three plans landed this round, in this order: the custom agents triage (28 PRs and the landing fix #4969), this one
through its five PRs, and then private Tailscale SSH socket folders (#4957, #4962). This plan and the custom agents
triage both touch the launcher catalog and the CLI; the landing's review read them against each other and against what
main gained since this plan was written (the environment triage, the sandbox workspace trust plan and other launcher
work) and found no interaction: retiring GPT-5.6 Terra leaves only deliberate mentions of it, and none of the plans
changes the protocol or a database schema. Its rebase conflicted only where conflicts below it were carried up, in
TODO.md and the custom agents plan's files, which were resolved there; every file of this plan's own change is exactly
what the executor pushed.

#### Fixes made while landing

- None.

#### Checks

This round landed three plans in queue order: the custom agents triage, the agent catalog and drift test cleanups, and
private Tailscale SSH socket folders. All the checks below ran once on the three plans' PRs stacked in that order on
main as it stood at the start of the landing (`f6ec0f51`), with the conflict resolutions described in the Landing
section of the custom agents triage report; the stack's top commit was `30ae8ef5`. Worker runs were on a separate arm64
Linux test worker; the rest ran locally on x86_64 Linux. The only later change is the landing fix
[#4969](https://github.com/scode/farhelm/pull/4969/changes), inserted after the custom agents plan's last PR, which adds
24 screenshot scene list entries and reformats two triage ledgers and one call in the launcher's golden scene setup;
after it I ran the screenshot batch tool's test (recorder run `18834801-a217-426a-aea5-61f735104753`),
`cargo fmt --all -- --check`, `dprint check` and the changelog check again, and all passed. That final tree is what main
now holds outside the plans queue, apart from what main gained while the custom agents plan was merging: two TODO.md
entries, a SPEC_impl.md note and new review queue findings, none of which touches code the checks covered.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `40997676-1b25-42dd-a283-44987367a22e`): all 8701 tests passed, with no runtime skip. The plans changed the launcher,
  Settings, templates, the quick switcher and Restart in the GUI, the helm's custom agent registry, templates and
  launches, the CLI, the supervisor's Goose restart, the drift tests, and the helm's Tailscale SSH connections; the
  whole suite was the narrowest run that covered them and their callers.
- The desktop app's tests, `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run
  `c1000071-3994-42e2-ad73-d85b8866e537`): all 184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `9e8e8c24-edac-4e59-9429-c8ab9f3211cb`): 50 steps passed, one skipped by design on Linux (paste).
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `4fc1cb5d-72d7-4edd-870c-909f63840e93`): passed, including the custom agents plan's 24 new scenes and main's sandbox
  scenes whose setup the landing merged with the custom agents plan's.
- The screenshot batch tool's test, `python3 -B scripts/test-gui-screenshot-batch.py`, locally: failed on the stack
  (recorder run `8e6013bf-03ee-48d3-9544-f2e4cb90d9eb`) because the 24 new scenes were not in its scene list, and passed
  with the landing fix.
- The docs website build, `bun install --frozen-lockfile && bun run build` in `website/`, locally: passed. Two docs
  pages changed (Start a session, and Stop, restart and resume), both in the agent catalog plan.
- `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; one warning, about an
  old citation in the triage archive, already on main) and `python -B scripts/check-test-sleeps.py`, locally: passed.
  `cargo fmt --all -- --check` and `dprint check`, locally: failed on the stack as described in the custom agents triage
  report's Landing section, and passed with the landing fix.
- `bash -n scripts/test-install-sh.sh` and `shellcheck scripts/install.sh scripts/test-install-sh.sh`, locally: passed.
  The custom agents plan added cases to the installer test.

Reused: the custom agents plan's run of the installer test,
`bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` (recorder run `242fd8fd`, at the executor's revision
`cb9cef98`), which covers the cases the plan added for a custom conversation report while an update holds the installed
version. Nothing it exercises changed since: main changed neither the installer nor the CLI since the custom agents
plan's base; that plan's later CLI change is in template flags, and the agent catalog plan's only CLI change is how
`--effort` reads its value, neither of which the installer test's conversation report path uses. Also reused: the agent
catalog plan's live Goose named Create and Restart drift run at its executor's revision (recorder run `5e4a1a9a`), which
spends vendor turns and was not repeated; nothing that landed since that plan's base touches Goose's support code. The
executors' other runs were focused selections that the runs above cover again.

Skipped: real agent and drift runs, which spend vendor turns and run only when asked; a real Tailscale network, which
the tests replace with stand-ins; the workspace doctests, since no documentation example changed; a Mac run, so the
macOS limit on socket path length (which the Tailscale plan's socket folder names are sized for) and the macOS installer
are covered only by tests that run on Linux; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **`--effort` now takes a following flag as its value.** For example, `--effort --yolo` launches a custom agent with
  the literal effort `--yolo`, where it used to fail with a missing value. That is how `--model` already behaves and
  what the specification accepts, so it is consistent, but for a custom agent a mistyped command now launches with the
  wrong effort instead of failing; for a built-in agent the helm still refuses an effort it does not know. Say if you
  want a TODO entry to refuse a value that looks like a flag.
- **Reopening a Goose conversation Farhelm did not start loses plain Restart without notice.** When you use Restart with
  to reopen, by ID, a Goose conversation Farhelm did not start, the launcher shows no warning now that the blanket
  warning is gone. Farhelm still clears the session's previous Restart target, and the reopened conversation never
  becomes eligible, so plain Restart becomes unavailable. The specification accepts this; whether to warn is a product
  call. Say if you want a TODO entry.
