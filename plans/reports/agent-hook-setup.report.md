# Per-agent Settings and opt-in conversation hooks

## What this was about

People looking for their agent in Settings had to know about the old Agent trust page. Agents with no settings were
missing from the list, which made it hard to tell if Farhelm supported them. Every agent now has a page under Settings →
Agents. Each trust switch and its hint are on that agent’s page. Credentials is under General.

Restart used to stay greyed out in Grok and Antigravity until the user installed hooks by hand. The maintainer chose an
all-hosts switch for each agent, off by default, because enabling it changes the agent’s saved configuration. The
launcher offers setup when the user picks either agent with setup off. The session hint offers the same setting in a
session started without setup. After a successful install, send the first prompt in the session’s terminal so the agent
can report the conversation. **Restart** can then resume it and hover help explains why it is unavailable while Farhelm
waits, when setup was skipped, or when setup failed.

## Things you should know

- With setup enabled, Farhelm installs or updates its own hook files on every host, container or sandbox where it starts
  that agent and the host’s hook policy permits it. A host policy skip is explained at Restart. A failed write also
  produces a session notification. The agent starts in either case.
- **Launch without** leaves the offer available for the next launcher. **Don’t ask again** hides both the launcher
  question and the in-session hint for that agent and leaves the draft open. **Show the hook setup prompt and hint** on
  that agent’s Settings page restores both. Templates and recent setups use the current switch without opening the
  question. Their sessions can still show the hint if setup was skipped.
- The hooks stay in the agent’s configuration after setup is turned off. Settings names what to delete. Grok uses the
  custom home when the supervisor has `GROK_HOME` in its environment. A value set only in a shell startup file does not
  change where Farhelm installs the file. Antigravity loads Farhelm’s own plugin. Users who followed the old manual
  instructions can remove the old `farhelm` entry from their shared hooks file independently.
- The callbacks also run in sessions outside Farhelm. Grok’s is quiet, and Antigravity’s returns the empty reply its CLI
  expects. Neither adds the line that teaches an agent to interpret `$farhelm` requests.

## Open questions and possible follow-ups

- Farhelm writes the hook files in the agent’s own configuration area. They can outlive a Farhelm uninstall and then
  point at an executable that is gone. A disposable install would let us test how real Grok and Antigravity sessions
  started outside Farhelm handle the missing command. The current validation uses isolated directories and stand-in
  agents, so vendor behavior after uninstall remains unverified.

## Pull requests

- Per-agent Settings pages and Credentials under General. [#4468](https://github.com/scode/farhelm/pull/4468/changes)
- Install Farhelm’s hook files before an opted-in launch. [#4483](https://github.com/scode/farhelm/pull/4483/changes)
- Save and apply the separate all-hosts choices in the helm. [#4495](https://github.com/scode/farhelm/pull/4495/changes)
- Settings, launcher, session hints and Restart explanations.
  [#4513](https://github.com/scode/farhelm/pull/4513/changes)
- Update the guides and specs and remove both TODO entries. [#4514](https://github.com/scode/farhelm/pull/4514/changes)

## Checks

At the rebased tip `4df4c62c`:

- `cargo nextest run -p farhelm-gui -p farhelm -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --lib --bin farhelm --test e2e -E <hook setup and Restart selection>`.
  All 31 selected tests passed, including capture and Restart with a stand-in Antigravity process. The terminal change
  that landed on main made Restart stop waiting for history after a terminal runtime exits. Its regression test passed
  with the new hook launch path in the same build. Recorder `c3f97103-2c43-4f3c-a64e-7e9937093529`.
- `python3 scripts/check-gui-goldens.py` and `python3 -B scripts/test-gui-screenshot-batch.py`. Full Linux comparison
  and all 47 screenshot scene-list checks passed. Recorders `0229d319-02d6-42de-b7bc-c66cae2224c4` and
  `a229a77e-173f-45d5-9894-51d9d261d65d`. Visually reviewed six new and two updated light/dark screenshots from the full
  `python3 scripts/check-gui-goldens.py --update` at `cd90bffe`, recorder `1a6fe3be-51e2-47a1-84e9-d63e784b5615`.
- `cargo clippy -p farhelm-gui -p farhelm -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` and `dprint check <changed docs and screenshot files>` all passed.
- `bun run build` in the website directory built the site and validated every internal link. Recorder
  `fc5150ca-d88c-455c-a7b6-b38e06b5562f`.

Reused pre-rebase tests where the tested behavior carried through unchanged:

- `cargo nextest run -p farhelm-gui --lib -E <Settings and trust>` at `84c90a88` passed 275 tests, recorder
  `f4521065-e3d8-4707-b028-5ed474ebe5d0`. The Settings layout commit rebased without a patch change. The final
  screenshot comparison and hook-focused tests cover the later additions to those pages.
- `cargo nextest run -p farhelm-proto -p farhelm-supervisor -p farhelm --lib --test e2e -E <protocol, hook and capture tests>`
  at `0f245003` passed 291 cases and failed three invalid launch fixtures. The corrected fixtures then passed with
  `cargo nextest run -p farhelm-supervisor --lib -E <three persistent hook launch tests>` against the same revision plus
  those fixture fixes. Recorders `4b7bffc8-ed77-4c34-9c70-a33e35395332` and `005e1d60-ac8a-47a0-a3ee-09ea23694c6e`. The
  final run rechecked the hook launch and Restart behavior with main’s terminal changes.
- `cargo nextest run -p farhelm-helm -p farhelm-proto -p farhelm-supervisor --lib -E <hook consent and migrations>` at
  `ecd89c76` passed 106 cases, including every selected helm test, and failed three supervisor schema parity tests.
  Those passed with `cargo nextest run -p farhelm-supervisor --lib -E <three schema parity tests>` at `7eba20c6`.
  Recorders `2b3d16f9-c8cc-4cfb-bb11-207e9c69f2c5` and `c431b61d-c879-4e50-b7ac-c162cb0900e9`. Main’s subsequent changes
  left the migrations and helm dispatch intact.
- `cargo nextest run -p farhelm-gui -p farhelm -p farhelm-proto -p farhelm-supervisor --lib --bin farhelm -E <launcher, Settings, hints and Restart wording>`
  at `650396a6` passed 436 cases and failed two fixtures. Both corrected fixtures passed at `cd90bffe` with
  `cargo nextest run -p farhelm-gui --lib -E <two corrected fixtures>`. Recorders `453ea239-fce9-46ee-8b1d-df9a4b93e50b`
  and `26a7f811-6bd4-4e65-856e-703cceac97bf`. The final run reran the new hook screens and launcher paths after the
  rebase.

Full workspace tests were skipped in favor of the affected selections. Screenshot checks used the available Linux
renderer. Live vendor sessions were skipped to keep vendor credentials and the real agent configurations untouched.

## Review gate

Each draft passed its fresh-context source review and commit wording read. The launcher review found two ways a prompt
answer could fail to launch. Both were corrected and covered by UI tests. The docs review corrected the remedy for a
host policy skip and clarified what installed callbacks do.
