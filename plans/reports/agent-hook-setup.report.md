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

### Landing

#### What landed with it

The plan's five PRs merged with a landing fix, #4518. Since the executor based the plan, main had gained only the
maintainer-tooling triage plan, which shares no code or behavior with it. The plan moves the helm's database to schema
95, the supervisor's to 37 and the protocol to 83. Main had moved none of the three since, so nothing needed
renumbering.

An independent reviewer read the five PRs against the plan, the specs and the rule that per-agent behavior lives in its
named places. It found no blocker:

- **Plan.** Every acceptance criterion is met, and nothing beyond the plan was built.
- **What it writes.** Farhelm writes only its own hook files: never Antigravity's shared hooks file, and never into a
  plugin folder that links somewhere else. A write is skipped when the file already matches and otherwise replaces the
  file whole.
- **Consent.** Every kind of start carries the user's current choice, and an agent cannot turn the setting on for
  itself.
- **Docs and specs.** The specs and docs pages match the code.

#### Fixes made while landing

All in [#4518](https://github.com/scode/farhelm/pull/4518/changes):

- **Two failing helm tests.** They build an old version of the helm's database by dropping every column added since, and
  compare it with that version's frozen layout. The plan's new columns for the remembered "don't ask again" answers were
  missing from that list, so both tests failed in the workspace suite. The executor ran only targeted tests, so it did
  not see them. The columns are dropped first now.
- **Comments and changelog.** Seven code comments still called the Grok and Antigravity hooks manually configured. And
  the changelog note now warns that turning on Grok setup replaces a hook file you made by hand from the old
  instructions.

#### Checks

The plan changes the desktop app's Settings and launcher, the supervisor's agent launch and hook installation, the
helm's preferences and sessions, the end-to-end tests, the specs and the docs pages. The landing ran the Rust workspace
suite, the desktop app's tests, the lint runs and the end-to-end smoke test on a separate arm64 Linux test worker, and
the goldens, the website build and the small checks locally on x86_64.

The test worker was unreliable this round. The first workspace run (recorder run `e1ca4636-7eae-4e57-adf3-5e81742287e5`)
was killed when the compiler ran out of memory, and a retry with a build-jobs option was refused before it started, so
neither ran tests. Later the worker's disk filled again, as it did in an earlier round today, and its build folder had
to be removed a second time. Runs that died on either problem are not evidence of anything.

On the plan as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four build jobs, on the test worker (recorder run
  `5f90b87d-7d8a-472d-ae39-4ab4d5408482`): 8236 of 8238 passed. The two failures were the helm's downgrade-fixture
  tests, fixed in #4518.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `71a092f5-038d-48f8-a6d5-9ae308338858`): all
  184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's own
  two clippy runs, on the test worker, not through the recorder: passed, with no errors in the output.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `4323e4d3-f4b7-43fb-80ff-51c7602c4b27`): every
  step passed but paste, which skips on Linux as it always does.
- `python3 scripts/check-gui-goldens.py`, the full comparison, locally (recorder run
  `e1ed0cae-d1cf-4a9e-93f3-9c59cfb0bbc2`): passed with the plan's new pictures.
- The website build (`bun install --frozen-lockfile && bun run build`), with its link check, locally: passed.
- `python3 -B scripts/test-check-gui-goldens.py`, `python3 -B scripts/gui-goldens.py --self-test`,
  `python3 -B scripts/test-docs-shots.py`, `python3 -B scripts/test-gui-agent.py`,
  `python3 -B scripts/test-gui-screenshot-batch.py`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py` (in its isolated environment), `dprint check`,
  `python3 scripts/check-agents-md.py` and `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4518):

- `cargo nextest run -p farhelm-helm --lib` from a clean build, on the test worker (recorder run
  `e8dd77a5-8986-4834-ba67-b4e5961aead2`): all 1795 passed, the two fixed tests among them. A first attempt (recorder
  run `4db6a342-1299-49db-9c8b-3fa6df170100`) failed two provisioning tests on the full disk, before the build folder
  was removed.
- `cargo clippy --all-targets -- -D warnings`, on the test worker: passed.
- `cargo fmt --all -- --check`, `dprint check` and `python3 releasing/check-changelog.py format`, locally: passed.

The rest of the workspace run above still applies to the fix, which changes only that test fixture, comments and a
changelog note. Skipped: a test with the real Grok and Antigravity programs, which the plan made conditional and which
spends vendor turns; macOS, since no Mac was part of this landing and the screenshots used the Linux renderer only; CI,
which runs only on demand.

#### Things you should know that the report does not say

- A user who followed the old manual instructions for Grok and then turns setup on gets their own hook file replaced by
  Farhelm's, so any edits they made to it are lost. The changelog note now says so; the report does not.
- On the launcher's question, **Launch without** is the main button and **Set up and launch** the secondary one. That is
  defensible for a question asking consent to change an agent's configuration, but the reverse may be what you expected.
- Settings lists the agent pages in an order that is neither alphabetical nor the launcher's: Cursor, whose page is
  empty, comes first. Using the launcher's chip order would be a small follow-up, but it changes every Settings golden
  picture, so I left it.
- Which agents offer hook setup is answered separately in the desktop app, the helm, the supervisor and the shared
  protocol code. Each is exhaustive, as the code rules ask, but no test checks that they agree. If they drifted, an
  agent's Settings page could promise hook setup and show no switch, or the launcher could offer setup the supervisor
  never performs. A single test would close that.
- Some leftovers of the old Agent trust page remain. Four docs pages still link with the words "agent trust settings",
  and Cursor's page, which has no settings, still says it applies to every host on this helm.
