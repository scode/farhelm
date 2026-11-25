## What this was about

When an agent reports its conversation late or stops reporting it, Restart is unavailable and Farhelm cannot reopen that
conversation. A vendor update can also expose hook output in the terminal. The plan called for testing these boundaries
with installed CLIs, including separate runs with hooks off. The new `capture_timing_hooks` category covers the eight
agents that report conversations. The only production change is to the hook log.

## Things you should know

Claude and Goose were resumable before input. Codex, Muse, Grok and Antigravity lacked a Resume offer at the idle
composer and were resumable after submission. Pi and OMP reported pending identities at startup. Restart stayed
unavailable at the idle composer and became available after a turn, with a matching turn-end report. OMP's live file
included a 256-byte title record, which Farhelm's parser accepted.

Claude, Codex, Muse, Grok and Antigravity sent `Stop`. Grok also sent `UserPromptSubmit`. The hook log now names
selected successful events so each remains visible in diagnostics when callbacks replace the same report. The expected
hook output markers were absent from the retained terminal text, including Antigravity's empty JSON response. The
hooks-off launches replied, the supervisor sampled them as settled, and no callback or capture appeared during
observation. Goose received the saved conversation ID on Restart with its reporter still off. Codex still received
`--no-daemon`.

## Open questions and possible follow-ups

No open follow-up from the live checks. An earlier Antigravity run paused at a file-access prompt. The text prompt now
explicitly forbids tools, and both modes passed on rerun. The initial Grok, Pi and OMP check counted pending identities
before input. The revised check measures the actual Restart offer, and all three passed. The Goose fixture now declines
the vendor's offer to switch back to the previous disposable directory. The Stop unit test now supplies its log
destination.

## The PRs

- Check when live agent conversations become resumable. [#4797](https://github.com/scode/farhelm/pull/4797/changes)

## Checks run, reused and skipped

- Compiled the final rebased source and listed all eight cases with
  `python3 -B scripts/run-harness-drift.py --list --category capture_timing_hooks`. The final rebase kept both module
  registrations after main added independent launch-prompt coverage.
- Ran on `cd44c9d1`: format, Markdown, test-delay checker, `cargo clippy -p farhelm --all-targets -- -D warnings`, and
  45 recorded Rust tests selected by
  `test(/^real_agent_capture::drift::capture_timing_hooks::/) | test(/^hook::tests::/)` in the farhelm binary and e2e
  targets (`c3be6574-db34-474c-9b35-99b436d7c2e3`).
- The final prompt was tested with
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category capture_timing_hooks --harness antigravity --harness claude`.
  Antigravity 1.5.16 (`031d47a1-739b-4691-a423-83359324c6ac`) and Claude 2.1.332
  (`ccf3a387-6ca1-479f-bdbe-a012ce546fbe`) passed. Claude was rerun because main added CLI updater suppression for drift
  runs.
- Reused from `34a928e6`: 61 recorded Rust tests with `test(/^real_agent_capture::drift::/) | test(/^hook::tests::/)`
  (`8a7e6ac6-f7f0-4c9c-a8e3-15b6870bcef4`). The full live category ran without upgrades. Six earlier live passes apply
  to the same launch, capture and callback checks before the prompt gained its explicit no-tools instruction: Codex
  0.172.0 (`221daad4-f70a-4dab-97db-f5fc5b83fae9`), Goose 1.60.2 (`d800bc44-c929-4e25-a92c-4cac8c0a2715`), Grok 1.0.92
  (`3eea4ed3-6c6c-4c31-b535-a6844151eeab`), Muse 1.4.25 (`471329a0-f409-4ac8-8045-4c2da804ed4b`), OMP 19.1.1
  (`5bf3ec0e-2a4f-4c16-918b-a57c0a5610fe`), and Pi 1.6.0 (`5059f239-2263-491a-8e00-6c48ea43cc99`).
- Reused 18 passing Python runner tests from `3cee496b`, recorded with `python3 -B scripts/test-run-harness-drift.py`
  (`356e41a6-63c2-4921-b22f-8ec52105737e`). The runner did not change.
- Retained the earlier Goose directory confirmation (`ed60f4b0-9628-4870-9e9b-58b763d77bd6`), Grok, OMP and Pi idle
  checks (`92bd577c-6f18-4fb3-8a90-a77d89639f26`, `6c684f5a-f9c3-4c2c-8da9-d98061f39510`,
  `3cb838bf-91ab-4a5a-96a2-ed4837215b97`), Stop fixture log-path failure (`e6bee496-6bfe-448a-947c-890f85d229ac`), and
  Antigravity file-access prompt (`6a4bc51a-defc-490e-be0b-4146ef071e2f`).
- Skipped the full workspace suite and screenshots because the change is confined to these tests and hook diagnostics.
  The plan called for checking the installed CLI versions without upgrading them.

## Review gate

One independent `gpt-6.2-sol` high-effort source review. Its findings were addressed with fresh callback evidence tied
to the captured conversation, retained terminal text checks, a Goose Restart check against the actual process, a settled
hooks-off turn, and a width check on the same OMP bytes sent to the parser. The commit and PR wording passed a separate
cold read.
