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

### Landing

#### What landed with it

This was the only plan landed this time: its two PRs, [#4781](https://github.com/scode/farhelm/pull/4781/changes) and
[#4782](https://github.com/scode/farhelm/pull/4782/changes), plus one landing fix,
[#4783](https://github.com/scode/farhelm/pull/4783/changes). Main gained only plan bookkeeping between the plan's base
and the landing, and, while it landed, a review queue update (#4776) that touches no code or test. Nothing could
interact with it.

#### Fixes made while landing

#4783 changes only the opt-in drift tests, their docs and the agent instructions that describe them:

- **A refreshed vendor login could be lost without a word; now it is at least named.** For Antigravity and Grok, the
  drift cases run the agent against a private copy of its configuration directory, so that Farhelm's hook setup never
  touches your real hook files. When the vendor replaces a file in that copy, such as a refreshed login, the case copies
  it back afterwards. This plan made that copy-back wait until the agent's processes are confirmed ended. When they
  could not be confirmed ended, the replaced files were deleted with the copy and nothing said so. A vendor that rotates
  refresh tokens would then leave you logged out of it with no hint why. The plan also moved the check of your real hook
  files ahead of the copy-back, so a failed check skipped the copy-back too, as did any single file whose copy-back
  failed. Now the copy-back runs first, and a copy-back cut short leaves no partial file beside yours. When the
  processes cannot be confirmed ended, the replaced files are still not copied back, so a refreshed login can still be
  lost, but each one is now named in the case's output. A new test checks that a changed hook file fails the case only
  after the copy-back has run.
- **"Run all tests" would have skipped the new category.** That flow exists to run everything, but the plan pinned it to
  the default `base` category (the launch and resume checks the report describes). It now runs every category, so the
  seven drift plans queued behind this one are covered there too as they land.
- **The validation list recommended skipping the agent upgrade.** It gave `--no-upgrade` as the standard way to run the
  model checks, which contradicts the drift docs' rule that a drift result only means something on the latest vendor
  release. It now says `--no-upgrade` is only for reproducing a result on the installed versions.
- **Two per-agent choices in the new category were written as defaults.** Its terminal width and its handling of Codex's
  model retirement screen each named one agent and gave every other agent a default, which the repository's code rules
  forbid because a newly added agent would silently get the default. Each now lists every agent.
- **The docs now say what COULD NOT RUN covers in the new category.** It also covers a vendor prompt the test leaves
  unanswered (Codex's retirement screen), and a model case reports it when any one of its models could not run, even if
  the others were confirmed. That includes Goose, Pi and OMP without an OpenRouter key, whose check without model flags
  can pass while the case as a whole still reads COULD NOT RUN.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. The plan changes only the
opt-in live drift tests, their runner and docs, and makes two helm functions callable from those tests alone, so the
landing ran the drift tests that need no vendor CLI and the checks that inspect the changed files.

On the plan's PRs rebased onto main (`aa562a84`), before the landing fix:

- `cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`, on the test worker
  (recorder run `0680a77a-03d1-418a-b5d5-6982585ecbaf`): all 15 passed, with no runtime skip.
- `python3 -B scripts/run-harness-drift.py --list --category all`, on the test worker: it built the drift tests and
  listed the ten base and nine model selection cases the report names, starting no agent.
- `cargo clippy --all-targets -- -D warnings`, on the test worker: passed.
- `python3 -B scripts/test-run-harness-drift.py`, locally (recorder run `51ded389-12a4-4997-bb3f-9666cff9e89f`): all 18
  passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check`,
  `python3 scripts/check-agents-md.py` (no errors; one warning already on main) and
  `python3 releasing/check-changelog.py format`, locally: passed.

On the landing fix (`68b821b4`): the same drift tests on the test worker (recorder run
`5260db8c-0f04-41aa-8f8a-9dbb3a501fb1`, all 16 passed, the new one included), clippy on the test worker, the runner's
tests locally (recorder run `80560754-6608-4b1d-893e-1d36c2f5829c`, all 18 passed), the formatter, the sleep check,
`dprint check` and `check-agents-md`: passed. An earlier revision of the fix failed clippy on one line of its new test,
fixed before it landed.

Reused: the executor's live run of the model selection category against the installed agents, recorded in the report.
The landing fix does not change how a model is launched or recognized, only the cleanup after a case and how two
per-agent choices are written (each now lists every agent).

Skipped: the live drift cases themselves, since they spend vendor startups or turns, need the vendor logins and are
opt-in by design; the workspace-wide Rust suite and the desktop checks, since no shipped code changed beyond those two
helm functions, which only these tests call and clippy compiled; and CI, which runs only on demand.

#### What the report says that is no longer true

Nothing. The review found the report accurate. What it does not mention follows below.

#### Things you should know that the report does not say

- **A Codex model you list as current is being retired.** Codex 0.172.0 interrupts a GPT-5.6 Terra launch with a choice
  between keeping it and switching to its replacement, and the test reports that model as COULD NOT RUN rather than
  FAILED. The plan chose that, and the report's follow-up covers it. Since Farhelm's catalog still marks Terra as
  current, though, the catalog likely needs your decision on whether to keep offering it.
- **A Cursor login screen now reads COULD NOT RUN in the base category too.** The plan added Cursor's "Press any key to
  log in" screen to the login markers both categories share. A base Cursor case that reaches that screen now reports the
  missing login. Before, it could pass if the screen also matched the cue it waits for, which was a false pass, so the
  change is for the better.
- **This plan also fixes two queued review items, which are still in the review queue.** One, on agent teardown hiding a
  test failure, asks that a cleanup error fail the case while keeping the check's own result; the drift cases now do.
  The other, on the drift cases skipping their configuration cleanup when a session fails to start, asks that the
  cleanup run on every path and that the copy-back wait until the agent's processes are gone; both now hold.
- **Some of what the plan added has no user yet.** As the plan asked, it exports the helm's named conversation start and
  adds a way for a case to fail on a trust prompt and to use another agent's private configuration, all for the seven
  drift plans queued behind it. If any of those plans is abandoned, the code it planned to use stays unused.

#### Possible follow-ups

- Deciding whether the catalog keeps GPT-5.6 Terra as a current Codex model.
- Closing the two review queue items above, so that triage does not schedule work that has landed.
- When the agent's processes cannot be confirmed ended, the case now names a dropped login file but still deletes it.
  Keeping the private copy for you to recover it by hand would avoid the lost login entirely.
