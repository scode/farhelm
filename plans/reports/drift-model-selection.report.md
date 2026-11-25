## What this was about

Farhelm's live drift suite could confirm that a vendor CLI started and resumed a conversation while missing a CLI that
silently picked a different model from the one selected in Farhelm. To cover that gap, the maintainer asked for checks
of current catalog models and focused follow-ups when a vendor's startup screen needed more work.

The suite now has separate categories for launch and resume checks and for model selection. Run
`python3 -B scripts/run-harness-drift.py --list` to see the category and agent pairs. A run without a category still
selects the launch and resume checks. A run that selects several categories upgrades each selected agent once by
default.

The model checks launch the choices marked current in Farhelm's catalog and look for them in the CLI's active model
display. Goose, Pi and OMP also start without model or provider flags and must show an active model. The live validation
used the installed versions and checked the startup screens without sending prompts.

## Things you should know

All current model choices were confirmed for Antigravity (four), Claude (one), Muse (one) and OpenCode (seven). Codex
confirmed three of four choices. Goose, Pi and OMP each showed an active model when started without model flags.

The other cases had missing prerequisites or a vendor prompt. The run exited successfully with those cases reported as
`COULD NOT RUN`. Their retained runs name the affected choices.

## Open questions and possible follow-ups

TODO.md records three follow-ups. Cursor stopped at the login screen for both Auto and Composer 2.5. An authenticated
run is needed to identify and verify the active model display. Codex interrupted the GPT-5.6 Terra launch with a choice
between keeping it and switching to its replacement. The test leaves that choice alone. A follow-up can exercise it in
an isolated CLI configuration if Farhelm still offers that model.

The named OpenRouter checks for Goose, Pi and OMP need `OPENROUTER_API_KEY`. Pi may omit the provider in the composer
footer when only one provider is available, so completing that check may need another provider readout. A separate
comparison with the saved provider and model is needed to catch silent fallbacks when launching without flags.

## PRs

- [#4781](https://github.com/scode/farhelm/pull/4781/changes) makes the drift runner category-aware and shares the
  supervised launch and cleanup steps across categories.
- [#4782](https://github.com/scode/farhelm/pull/4782/changes) adds the model selection category and records the
  outstanding verification work in TODO.md.

## Checks run, reused and skipped

- Through the recorder, `cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`
  passed 13 tests on the framework (`cdafc826-f5e7-43f4-9b03-2538997e1f2b`) and 15 on the model changes
  (`3d759eeb-a114-460b-b196-42672b4622fe`). Listing with `--category all` found ten base and nine model cases.
- Through the recorder, `python3 -B scripts/test-run-harness-drift.py` passed 18 stand-in tests on `681a531b`
  (`6966e5d4-f8cb-498d-9d3b-15f90160eaf4`). The runner and its Python tests were unchanged in the final revision.
- The live command was `python3 -B scripts/run-harness-drift.py --no-upgrade --category model_selection`. Final case
  evidence was recorded on `69a6d92c`. The published draft differs only in TODO.md's reproduction commands.

| CLI         | Installed version  | Final live result                                       | Recorder run                           |
| ----------- | ------------------ | ------------------------------------------------------- | -------------------------------------- |
| Antigravity | 1.5.16             | Passed, all four models                                 | `36822094-0aa0-4ad7-a447-b9c74d345e45` |
| Claude      | 2.1.331            | Passed, Fable 5                                         | `9bede34b-7e69-440d-8d6f-392d49b3fecb` |
| Muse        | 1.4.25-R9361.1     | Passed, Spark contributor                               | `c90c1f4c-720f-41a0-81ef-68553e109dde` |
| OpenCode    | 1.19.6             | Passed, all seven models                                | `dd3710ba-1f90-46b5-8edd-b445a2dab460` |
| Codex       | 0.172.0            | Three confirmed, Terra interrupted by retirement prompt | `7de3caf4-7abf-4c7f-8d88-8361024fc858` |
| Cursor      | 2026.11.22-e9042b2 | Login needed                                            | `a9d8900f-3627-4775-82b1-166e196c0239` |
| Goose       | 1.60.2             | No-flag check confirmed, OpenRouter key needed          | `2a3cfe79-f2f3-4ef1-b0fb-324f6ac9146d` |
| OMP         | 19.1.1             | No-flag check confirmed, OpenRouter key needed          | `138a8682-34e4-4279-accc-5954bf746bf0` |
| Pi          | 1.6.0              | No-flag check confirmed, OpenRouter key needed          | `b2eb45f6-8417-4136-adc5-c038d3445dfd` |

- Strict all-target Clippy for `farhelm` and `farhelm-helm`, Rust formatting, Markdown formatting and the test-delay
  checker passed. Instruction link checks found zero errors and one pre-existing archive heading warning. Markdown
  formatting and instruction link checks were rerun after the final TODO edit.
- Vendor upgrades and live base-category turns were skipped. The installed CLI versions were the intended validation
  target, and the model cases need startup only. The workspace battery and desktop checks were outside the scope of the
  test changes.

## Review gate

An independent Sol reviewer checked both PRs, and the executor addressed the findings. This included restricting test
hook consent to the matching harness, confirming isolated hosts stopped before copying configuration updates back, and
making Pi's footer check distinguish the active model from a routed model or an empty choice. The commit and PR titles
passed independent cold reads.
