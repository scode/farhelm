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

### Landing

#### What landed with it

Two plans landed this round, both adding a category to the opt-in live drift tests: this one, through
[#4785](https://github.com/scode/farhelm/pull/4785/changes) and the landing fix
[#4791](https://github.com/scode/farhelm/pull/4791/changes), and then drift-permission-modes. They touch the same files
only where each registers its category and records its own follow-ups in TODO.md, so stacking them needed nothing but
keeping both sides. Main also gained three docs changes while they landed (two reorganizations of TODO.md around
Tensorlake, and a triage of the review queue), none of which touches the drift tests.

#### Fixes made while landing

#4791 changes only the drift tests and their docs:

- **Claude's updater is now off in every drift category.** The plan turned off Claude's background updater for its own
  launches only, after Claude updated itself between two launches of one case. The same can happen in any category, and
  a launch and its Restart that straddle an update end up testing two different Claude versions. Claude's updater is now
  turned off for every drift case, the way Muse's and OMP's already were.
- **Per-agent choices now name every agent.** The category decided three things by comparing the agent against one name
  each: that Goose is checked only at launch, since it shows no effort on screen; that Muse gets a wider terminal; and
  what text means Claude moved the turn to another model. The repository's code rules forbid that, because an agent
  added later silently gets the default. Each now lists every agent.
- **The drift docs now describe the category.** They say which efforts it picks, that a Goose pass cannot confirm the
  effort the provider applied, and that Goose, Pi and OMP need OpenRouter credentials for the whole case.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. Both plans landed this
round (drift-reasoning-effort and drift-permission-modes) change only the opt-in live drift tests and their docs, so the
landing ran the drift tests that need no vendor CLI and the checks that inspect the changed files, once on the two plans
stacked together with their landing fixes, which is what main now holds.

On the stacked plans with the landing fixes (`3ac3fd12`):

- `cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`, on the test worker
  (recorder run `b68d5437-426c-4166-a747-29c27d4df8a8`): all 20 passed, with no runtime skip. They include each new
  category's checks that every harness offering the feature has a case.
- `python3 -B scripts/run-harness-drift.py --list --category all`, on the test worker: it built the drift tests and
  listed ten base, nine model selection, eight permission modes and seven reasoning effort cases, starting no agent.
- `cargo clippy --all-targets -- -D warnings`, on the test worker: passed.
- `python3 -B scripts/test-run-harness-drift.py`, locally (recorder run `148edde1-d9c4-45da-a873-6d2ee6294170`): all 18
  passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 scripts/check-agents-md.py` (no errors; one warning already on main), locally: passed.

The same set had also passed on an earlier revision of the landing fixes (`441426b3`, drift tests in recorder run
`c4b82231-1bd2-43e9-a2dd-8c2647f4cff1`).

Reused: the executors' live runs of the new categories against the installed agents, recorded in the reports. The
landing fixes change how the per-agent choices are written, not what any agent is launched with, with one exception:
Claude now starts with its background updater turned off in every category, not only the reasoning effort one. That was
already how the reasoning effort runs in the report launched Claude.

Skipped: the live drift cases themselves, since they spend vendor startups or turns, need the vendor logins and are
opt-in by design. That leaves Claude's updater setting unexercised live in the other categories; the launch with it is
the same one the reasoning effort runs used, and the drift tests above check how the launch command is built. Also
skipped: the workspace-wide Rust suite and the desktop checks, since no shipped code changed, and CI, which runs only on
demand.

#### What the report says that is no longer true

Only one detail: the report says the test disables Claude's background updates "for its own invocation". After the
landing fix, every drift category does that, as described above.

#### Things you should know that the report does not say

- **Goose's readiness check has never seen a real Goose screen.** It waits for Goose's bare `>` prompt, a check the
  executor added after its last live run, and every Goose run stopped earlier for lack of an OpenRouter key. The first
  run with the key may report a working Goose as FAILED if its prompt looks different. Running the report's follow-up
  with the key will exercise it, though that TODO entry does not warn about it.
- **Off and Minimal are still unverified on any agent.** Only Goose, Pi and OMP offer them, and those three need the
  OpenRouter key, as the report says.
