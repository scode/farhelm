## What this was about

This plan resolves 33 review decisions about the helm, the CLI and desktop startup. It fixes host label formatting,
automatic update admission, remote setup confirmation, tunnel and credential delivery lifetimes, late Delete results,
and agent launch guidance. It also corrects source documentation and tightens focused tests.

## Things you should know

- An automatic update that finds another operation already working on the host gives back its attempt. The helm tries
  again after that operation releases the host. Hosts already reporting a development build stay out of the automatic
  update scan.
- Remote Mac setup checks again before starting a standalone install if the old plan found no supervisor to adopt. If
  the desktop app has appeared or the target login or architecture has changed, setup asks for a fresh plan. An already
  installed supervisor still takes the ordinary adoption path.
- An environment check that stalls while receiving its input now gives up at the same deadline as the rest of
  provisioning. Removing a host also retires its finished setup tasks, even when one finishes during removal.
- A temporary failure accepting a localhost tunnel client leaves the port open and retries. A quick reconnect cancels
  obsolete credential transport work even if the new connection arrives before the old delivery sees the disconnect.
- If a session Delete outlasts its reply and leaves cleanup details, the host notices bell shows them after the row is
  gone. This includes the path and any reason when the checkout was left behind. It also applies when deleting one
  session from an environment that stays. Reading this one-time notice makes it available to Clear resolved. An archive
  from either Delete or Replace wakes Trash even if the session was already gone from the list.
- Agent launch refusals quote the **Start YOLO sessions without asking** switch. Agents on ordinary hosts get the
  `farhelm agent create --template` hint when they ask to manage a recipe template. Recipe launches use the new
  environment's home by default, and the create help explains the folder rule.
- The checkout-notice display required one addition in the desktop notices panel, the area of the session-list triage
  plan. The setup cleanup fix covers two places that track Tensorlake setup work. The retained-environment Delete and
  cache-survival checks use the existing scripted environment fixtures.

## Open questions and possible follow-ups

- The two startup fixtures still check their existing outcomes. One checks whether the desktop PATH warning is stored.
  The other checks the update preference saved by a devbuild. Establishing first-client notice delivery or proving the
  automatic update scan sees the seeded choice would need new startup fixtures. The approved test scope was limited to
  small fixes, so those two decisions closed without a test change.

## Pull requests

- Two short-host-label fixes cover valid SSH URI variants and abbreviated IPv4 addresses.
  [#4493](https://github.com/scode/farhelm/pull/4493/changes)
- Two Automatic Update fixes cover Busy results and development-build exemptions.
  [#4497](https://github.com/scode/farhelm/pull/4497/changes)
- Three setup fixes cover Mac setup confirmation, environment probe deadlines, and task registration when a host can be
  removed. [#4500](https://github.com/scode/farhelm/pull/4500/changes)
- Two fixes keep a tunnel listening after a temporary accept error and cancel obsolete credential deliveries after
  reconnect. [#4505](https://github.com/scode/farhelm/pull/4505/changes)
- Two Delete and Trash fixes cover a notice that arrives after the reply deadline and a Trash refresh after archiving.
  [#4507](https://github.com/scode/farhelm/pull/4507/changes)
- Four wording changes cover the YOLO switch label, template-management refusal, the launch wait comment, and recipe
  folder help. [#4508](https://github.com/scode/farhelm/pull/4508/changes)
- Six documentation decisions cover two provisioning tests, the current-connection history check, the placement
  resolver, and two reports of the same hostless Clone contract.
  [#4509](https://github.com/scode/farhelm/pull/4509/changes)
- Ten tests cover the Mac cleanup budget, retained terminal hosts, the upload pipe holder, manifest contents, Automatic
  Update wake, connection refusals, host phase names, the Trash wake, environment heartbeats, and the other session's
  cache entry after Delete. The PATH first-read and devbuild first-scan outcomes close without a test change.
  [#4510](https://github.com/scode/farhelm/pull/4510/changes)

## Checks run, reused and skipped

All runtime commands used the recorder. The earlier runs tested the respective draft's working changes before their PR
links and the rebase. The recorder preserves those changes, their parent revisions and the exact selections.

- `cargo nextest run -p farhelm-proto --lib`, selecting `host_name::tests`, passed three tests over parent `e4e3642a` in
  run `def59a78-be93-4fe2-a5e9-f765fb4b40de`.
- `cargo nextest run -p farhelm-helm --lib`, selecting the automatic update tests and cancellation and eligibility
  cases, passed eight tests over parent `fb669476` in `08270821-b902-4fd8-9394-c0f78813350b`.
- The same helm command scoped to Add/setup confirmation, the environment probe and session launch failure passed 16
  tests over parent `771d3582` in `e4dc5690-dcd0-4d5f-9fb3-f493dc7ac46c`. Five setup confirmation and probe cases passed
  again on the final provisioning changes over that parent in `9714f1df-bef0-493a-a9e9-3bd17d4c6fb0`. The subsequent
  source changes from the first run affected those targeted cases.
- The helm command scoped to `credential_delivery::tests` and `tunnels::tests` passed 48 tests over parent `f0a68950` in
  `601964db-eeca-4f5a-bb30-e1ed54ee9017`. This includes recovering on the same port after one injected accept error and
  retiring a held credential delivery across reconnect.
- The helm command scoped to notice tests, ordinary Delete replies, late ordinary and environment Delete, and fresh
  schema creation passed 19 tests over parent `2d1342ff` in `edcfbab1-c614-4cc2-a34c-616b9412d5e2`. On the same source,
  `python3 scripts/check-gui-goldens.py --scene 'notices-*' -j 4` matched the three existing notices scenes against
  their references in `e44c0757-7c28-4abc-b3b2-93b86caea2ac`. The new notice has focused REST and persistence tests. The
  screenshot comparison covers existing scenes.
- The final Linux selection ran
  `cargo nextest run -p farhelm-helm -p farhelm-helm-client -p farhelm-desktop -p farhelm --lib --bin farhelm` at
  `4d948279`, scoped to the agent refusals and parser help, the changed regression fixtures, and both late Delete paths.
  All 20 passed in `cd303f98-524e-48cc-913e-873b8e846324`, with zero runtime skips. The later formatter change only
  rewrapped a test closure.
- The rebases onto `98f9c0b8` and `7261f00f` preserved the product diff from the reviewed drafts. The GUI-session and
  terminal-host changes affect separate code paths and spec sections. The later tooling changes affect separate fixtures
  and how the recorder saves their evidence. The earlier selected checks still cover the preserved behavior, including
  at the final stack tip `4d72ce30`. Each draft's archive was checked against its new parent.
- Runtime testing for the comment-only PR was skipped. The real Mac opt-in cases compile in the selected helm test
  binary. Their native runtime behavior requires a Mac, so the local run provides no native execution evidence.
- The full battery was unnecessary because the selected suites cover the changed behavior and the rebase preserved the
  product changes. Static validation covered the Rust and Markdown formatters, whitespace, changelog format, the
  test-delay checker and instruction references. The instruction checker reported one existing archive citation warning.
- The first Automatic Update and tunnel test builds are retained as `0b93b356-8e77-4a96-811f-26e3ae8dc309` and
  `870bc9b6-87b6-444e-8db5-c98b44ad33de`. Corrected a test import and test assertions, respectively, before the
  successful selected runs above.

## Review gate

All eight drafts passed the required independent source review and cold wording read. The retained-environment Delete
coverage, setup target recheck, tunnel recovery fixture and test readiness checks incorporate review findings. The
reviews left no unresolved issue in the changes.

### Landing

#### What landed with it

The plan's eight PRs merged with a landing fix, #4521, as the first of four plans landed together in this round; the
Podman triage, the exited-terminal scrollback fix and the status-mark hover change landed right after it. Since the
executor based the plan, main had gained the per-agent Settings and hook setup plan, which also added a step to the
helm's database. While rebasing, the landing gave this plan's step for late Delete notices the next number after that
one, so existing databases run both upgrades in order and new databases get both changes. Nothing else that landed
touches the plan's code: the other three plans in the round share no source files with it.

The 33 triage outcomes were graduated by the plan's own commits. The review queue, its index and the outcome ledger
conflicted with the other triage plan's edits to the same files; the landing kept both sides by replaying each commit's
own change onto the combined files, and a script then confirmed that every queue file has its index line, every open
ledger entry has its queue file, and nothing this round archived is still in the queue or the open ledger.

An independent reviewer read the eight PRs against the plan, the triage outcomes and the specs. It found no blocker:

- **Outcomes.** All 33 were carried out as decided, with nothing beyond them except the small extras the report names.
- **Correctness.** It found no bug in the late Delete notice, the credential delivery cancellation, the automatic update
  retry or the tunnel's accept loop, and checked the new lock orders for deadlocks.
- **Specs and docs.** SPEC.md, SPEC_impl.md, the CLI help and the protocol docs match the code.

#### Fixes made while landing

In [#4521](https://github.com/scode/farhelm/pull/4521/changes): the code comment that explains the notices table (how
notices go away with their host, the uniqueness rule, why notice ids are never reused) had ended up on a frozen copy of
the table that an older upgrade step needs. It is back on the current table, and the frozen copy says what it is for. No
behavior changed.

#### Checks

The round landed four plans together: the helm triage, the Podman triage, the exited-terminal scrollback fix and the
status-mark hover change. The landing ran their checks once, on all four stacked on main. The Rust workspace suite, the
desktop app's tests, the lint runs and the end-to-end smoke test ran on a separate arm64 Linux test worker. The goldens,
the website build and the small checks ran locally on x86_64. The workspace run used four build jobs, as in the last
round, since runs with more had run out of memory on the worker; this round it had no memory or disk trouble.

On the four plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four build jobs, on the test worker (recorder run
  `8ba93f1d-d933-483b-a90a-eeac88311d3a`): all 8252 passed.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `890a2d33-0c71-410b-9498-3a792708a974`): all
  184 passed.
- `cargo clippy --all-targets -- -D warnings`, on the test worker, not through the recorder: failed on one lint in a new
  Podman test, fixed in #4522.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `c732c6cc-ba70-4b34-853a-fe7961b6d297`): every
  step passed but paste, which skips on Linux as it always does.
- `python3 scripts/check-gui-goldens.py`, the full comparison, locally (recorder run
  `2372fd39-11ef-4468-84d3-f1c70cdf9f7e`): failed only on the two pictures of the recipe Delete anyway question, which
  the Podman plan had captured before main's Settings sidebar changed. Fixed in #4522.
- `python3 -B scripts/test-gui-screenshot-batch.py`, locally: failed because that new scene was missing from the
  screenshot scene list. Fixed in #4522.
- `python3 -B scripts/test-check-gui-goldens.py` (recorder run `952eb6e8-4c50-4a08-bc89-870dd1358b6d`),
  `python3 -B scripts/gui-goldens.py --self-test`, `python3 -B scripts/test-docs-shots.py` (recorder run
  `6c988518-9a04-4428-b87d-f5a1d49ede50`), `python3 -B scripts/test-gui-agent.py` (recorder run
  `430497e6-e6fd-4d35-8648-4082a3b33d33`), the website build (`bun install --frozen-lockfile && bun run build`) with its
  link check, `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its isolated environment),
  `dprint check`, `python3 scripts/check-agents-md.py` and `python3 releasing/check-changelog.py format`, locally:
  passed.

After the landing fixes (#4521 to #4524):

- `python3 scripts/check-gui-goldens.py --update`, locally (recorder run `6d8fe2d8-8886-45e5-a8f7-c1902f41d10c`):
  re-rendered every picture and changed only the two Delete anyway pictures, which were then published.
- `python3 scripts/check-gui-goldens.py --scene 'settings-recipe-*' --scene 'session-row-*'`, locally (recorder run
  `9282ca4e-8230-435c-8d5d-20380af3477c`): passed.
- `cargo nextest run -p farhelm-helm --lib`, on the test worker (recorder run `2b0cfd85-3205-4a06-97c1-d028a7ae46db`):
  all 1801 passed.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and the same with `--features agent`, on the test
  worker: passed. They had not run on the first pass, which stopped at the failing lint run.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `cargo fmt --all -- --check`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

The rest of the first runs still applies after the fixes, which change only a Podman test's lint, a test comment, two
pictures and the scene list, a code comment on the helm's notices table, SPEC.md wording, the desktop parity notes and a
changelog note. Skipped: the Podman suite's opt-in real-engine and real-SSH variants, which neither triage plan needed
beyond the real Podman environment test, which ran in the workspace run on the worker (about two minutes, not skipped);
macOS, since no Mac was part of this landing and the pictures used the Linux renderer only; CI, which runs only on
demand.

#### Things you should know that the report does not say

- The new "Session deleted on <host>" entry in the notices panel has no golden picture, and once read it only greys its
  text, while the other notice kinds show a bold "Resolved" line. SPEC.md's "stay greyed" is met. A picture and the
  matching "Resolved" line would be a small follow-up if you want that entry to look like the others.
- One test helper's comment in the helm's agent request code still quotes the retired YOLO label, "start YOLO sessions
  here without asking". It was outside the decided scope and changes nothing for users.
- One graduated entry in the outcome archive has an Execution field with a sentence starting in lowercase ("Corrected.
  the helm and CLI refusals ..."). I left it, since the archive is history.
