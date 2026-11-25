## What this was about

Farhelm offers reasoning effort when launching a supported agent. The live checks previously started most agents at one
familiar setting and could miss a vendor change that rejects or resets another setting. The maintainer asked for live
coverage of boundary values such as Off, Minimal, Xhigh and Max, with one real turn at an enabled effort per agent and
focused follow-ups when the installed CLI was hard to verify.

The new `reasoning_effort` category selects a current Farhelm catalog model that supports each chosen effort. It checks
the level shown by the agent and verifies an actual reply at one enabled level. Goose 1.60.2 has no active effort
readout in its terminal status. Its case checks that Farhelm passes the chosen effort to Goose, Goose opens its input
prompt, and Goose completes a turn at one chosen setting. The provider's effective effort remains unverified.

## Things you should know

Four agents confirmed the selected values and completed a real turn. Claude confirmed Xhigh and Max, Codex confirmed
High, and Antigravity and Muse confirmed Xhigh. The Claude turns in earlier runs switched to Opus 4.8 and kept Xhigh.
The final run completed without a reported model switch. Claude updated itself between launches in the first run, so the
test now disables background updates for its own invocation.

The three OpenRouter cases reported `COULD NOT RUN` before launch because this executor had no `OPENROUTER_API_KEY`. Off
and Minimal are offered only by those three harnesses, so the run confirmed neither level. The category's aggregate exit
was zero with four passes and three unavailable cases.

## Open questions and possible follow-ups

TODO.md records the credentialed run for Goose, Pi and OMP. Pi and OMP need live display evidence for Off, Minimal,
Xhigh and Max and a reply at one enabled setting. Goose needs authenticated startup for Off and Max and a reply for Max.
Verifying Goose's effective provider effort would need a source of evidence beyond the displayed terminal status.

## PRs

- [#4785](https://github.com/scode/farhelm/pull/4785/changes) adds the live reasoning-effort category and the
  credentialed follow-up in TODO.md.

## Checks run, reused and skipped

- Through the recorder, `cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`
  passed all 18 selected tests at `79697847` (`033da070-3cd5-4b30-8be0-67c774f18b55`).
  `python3 -B scripts/run-harness-drift.py --list --category reasoning_effort` found all seven cases.
- Through the recorder, `python3 -B scripts/test-run-harness-drift.py` passed 18 stand-in checks at `8cfc7161`
  (`f639bf20-f833-4ebd-b987-3b1734013bec`). The Python runner and case registrations did not change after that run.
- `python3 -B scripts/run-harness-drift.py --no-upgrade --category reasoning_effort` ran at `79697847`, the same tree as
  the draft PR.

| CLI         | Installed version | Final live result                   | Recorder run                           |
| ----------- | ----------------- | ----------------------------------- | -------------------------------------- |
| Antigravity | 1.5.16            | Hydrogen Xhigh, reply confirmed     | `0f4ee728-d7c2-4ea1-8869-1317df5ad1aa` |
| Claude      | 2.1.332           | Fable Xhigh and Max, reply on Xhigh | `bb3c0a07-14e7-45a1-b119-a709a67864f8` |
| Codex       | 0.172.0           | Luna High, reply confirmed          | `b9bf5ae9-de00-4ca1-898a-67174324cda0` |
| Muse        | 1.4.25-R9361.1    | Spark Xhigh, reply confirmed        | `103fe033-34d1-4799-9237-9589c1793859` |
| Goose       | 1.60.2            | OpenRouter key needed               | `ab76abfa-8259-486e-bdb9-e0574a94a510` |
| OMP         | 19.1.1            | OpenRouter key needed               | `893da1ab-80e6-4613-be51-c083aa7c7756` |
| Pi          | 1.6.0             | OpenRouter key needed               | `540d0aaf-26fb-4a27-80f2-fdaffe24995a` |

- `cargo clippy --locked -p farhelm --all-targets -- -D warnings` and `cargo fmt --all -- --check` passed on `79697847`.
  The Markdown, test-delay and instruction link checks passed on `407efc79`. The final changes affect only the Goose
  prompt check and the test prompt wording. The instruction checker still reports one pre-existing archive heading
  warning.
- Vendor upgrades by the runner, the live base category and the full workspace suite were skipped. This plan called for
  installed-version evidence and changed only the opt-in effort tests and TODO.md.

## Review gate

The independent Sol review and its follow-up identified coverage defects. The checks now wait for Goose's input prompt,
prefer the current Codex status over the startup banner, exclude warning text from Claude's effort readings and
recognize OMP's glyph alongside the effort word. A separate scope reassessment supported keeping these changes within
this category. The findings were addressed, and the draft title passed cold review.
