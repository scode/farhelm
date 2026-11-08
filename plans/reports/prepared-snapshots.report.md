### What this was about

New Tensorlake hosts used to install Farhelm each time. They now boot an installation shared through a recipe's prepared
snapshot. The first new host prepares it; concurrent requests join. It becomes stale only after more than 24 hours, when
the helm's Farhelm version changes, or when the recipe's built snapshot changes. Preparation stays lazy, with no button
or background renewal. The recipe row and setup progress explain that first-time wait and show readiness, staleness,
failure or interruption.

### Things you should know

Preparation bills a short-lived sandbox. It installs the enabled supervisor unit and lingering, stops the supervisor,
removes and verifies its entire state directory, and clears the machine ID before checkpointing. Two real hosts reused
one prepared snapshot and remained ordinary connected hosts together, with distinct supervisor and machine identities.
Both had active boot-enabled units and lingering, independently selected Claude Code, Codex and GitHub credentials in
private files, and working recipe tunnels. Neither clone ran ordinary Farhelm installation.

The agreed custom-base boundary is preserved: authentication already in a custom base may remain and be inherited.
Farhelm delivers none of its own credentials, GitHub configuration or tunnels to the preparer. Rerun and Update on
existing hosts keep ordinary installation; recovery of an interrupted prepared create keeps that create's saved
installation facts. The Phase 2 TODO remains for sandbox sessions.

Measured on one CPU, 1 GB memory and 10 GB disk, with fictional endpoint credentials and a GitHub token:

| Host   | Create to Running | Admission, including SSH readiness | Credentials |   Attach |     Total |
| ------ | ----------------: | ---------------------------------: | ----------: | -------: | --------: |
| First  |            899 ms |                             974 ms |   18,614 ms | 1,167 ms | 21,655 ms |
| Second |            923 ms |                           1,028 ms |   18,025 ms | 1,198 ms | 21,175 ms |

These phases start after preparation. The first whole Add took about 109.1 seconds including building and preparing the
recipe; the second took about 23.6 seconds using the cache. Those whole-operation observations include polling overhead.
There is no ordinary-install baseline, statistical comparison or hard latency target. The development payload matched
Ubuntu's glibc; this was not a static-musl release-artifact check. Measurements are retained in
`lore/2026-11-08-prepared-snapshot-host-timings.md`.

A real attempt found that ordinary installation's unprivileged linger action could fail through masked polkit even after
sandbox preparation had enabled lingering with sudo. The preparer now omits that redundant action; its final guard still
independently requires actual lingering and persistent boot enablement. The successful real run used six actual
sandboxes plan-wide, against eight conservative reservations and the authorized cap of 100. All six sandboxes, three
snapshots and two registered test SSH keys were confirmed removed; no images were created. Temporary helm state was
removed after evidence extraction. No live installation was used.

### Open questions and possible follow-ups

No decision is needed to land this plan. Credential delivery accounted for roughly 18 seconds of each cached spawn;
reducing that cost is a possible follow-up. Per-session sandbox launch remains the dependent plan's work.

### The PRs

- [#3203: reuse prepared snapshots for Tensorlake hosts](https://github.com/scode/farhelm/pull/3203/changes), draft.

### Checks run, reused and skipped

- Passed on the final behavior:
  `cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::snapshot_preparer::tests::)'` as part of run
  `833c774e-25ed-443a-b9b8-82aee6c83d4b` (two tests), and
  `cargo nextest run -p farhelm-helm --lib -E 'test(snapshot_cleanup_)'`, run `179d9cfa-92f9-4b5e-8b47-37de2d11a69d`
  (four tests). These cover the linger regression and the actual cleanup script's boot, state and link guards. Pinned
  nextest, four slots, zero retries and verified pinned tmux; no runtime skips.
- Passed: `npx playwright test 'tensorlake-(hosts|recipes)\.spec\.ts'`, run `9a686a0e-1d23-46af-bcaa-7c6124f04106`: all
  52 cases, 26 per engine, no skips, three workers with one stack each and zero retries. Matching CLI/web builds were
  prepared first. The later production change only omits the preparer's redundant linger action; the simulated backend
  returns success for that action, and browser rendering is unchanged.
- Passed: the isolated real two-clone check, run `9cf2b160-5387-4f04-b5b6-86f95e0c9ec8`, using the corrected CLI and
  matching staged payloads. It exercises the production API, installer, SSH, supervisor connection, per-host credential
  delivery, tunnels and cleanup. Its bounded private harness also passed three pure checks, run
  `84391ccb-7e93-4ca0-a0ca-0a6c41182641`. All listed final records have complete source, output and cleanup evidence.
- Passed: `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, matching CLI build, `cargo fmt --all -- --check`,
  the test-delay source check (423 delays, none missing a reason), changed specification/fragment/CSS `dprint check`,
  changelog formatting and the instruction-reference check (zero errors, one pre-existing triage citation warning).
- Reused: the successful 49-test prepared-engine, Add/recovery, backend and UI-progress selection from `4efc518e`, run
  `dc6ea102-ff1a-47c3-a18d-688d605f8001`, plus the six-case fixture retry `b2659cc9-67dc-4445-9765-e7c5143d0f2f`.
  Subsequent changes were three mechanical lint fixes, the separately tested linger correction, browser fixture
  corrections and prose. The earlier all-targets Clippy pass for helm, UI and proto on `98b76388` is reused for
  unchanged UI/proto code. The rebase onto `5331b984` preserved the feature patch apart from a SPEC blob index and line
  offset. Incoming test-only tmux observation, independent session-row geometry and validation-policy changes were
  inspected; they do not change the prepared-host contracts. No runtime rerun was needed for that rebase.
- Retained failures: the initial Rust attempt `506633b4-03e3-4607-b0c3-359711a89caf` stopped at test compilation;
  `ea27aab9-582b-4970-9b7e-1c0809b1b1d1` exposed six fixture/observation failures, corrected and covered by the
  successful six-case retry and 49-test selection. Initial browser run `d28e55ea-ce4c-4aab-91be-102c678c52e9` had ten
  failures from stale step/view expectations and an unestablished folding premise; the corrected exact cases passed in
  `e0735088-5cb4-433b-940f-71f7088efe96`, then all 52 passed. Real attempts `96e4ee56-3c10-4933-b111-47e0d4dfe984` and
  `8a7b9d37-8695-4357-8d63-1f450118f531` found the harness socket-path limit and redundant linger issue respectively. A
  conservative sweep initially treated an uncreated host reservation as unsettled
  (`59861ac3-18de-4e2a-ad6e-4b96944c1080`); persisted ownership and completed host deletion settled it, and explicit
  sweep `a5e2ce52-89bb-4330-a205-65f9d5ba4ea2` confirmed cleanup. None of these failed attempts is a pass or a newly
  established latent flake.
- Skipped: the full workspace and browser batteries, native desktop, release packaging and installer-distribution gates.
  The affected provisioning/recipe paths, browser integration and real clone behavior have targeted coverage; no
  desktop-only, release or cross-distribution behavior changed. No CI dispatch was needed; CI is manual-only.

### The review gate's outcome

One fresh-context Opus 5.5 reviewer at high effort reviewed the coherent change, then verified corrections in the same
session. Four behavior findings were fixed: clone SSH readiness before admission, interrupted-create recovery planning,
keeping ordinary confirmation details out of live progress, and a concurrent preparation waiter race. The real-run
linger correction and browser fixture fixes also passed that reviewer. Ready wording deliberately says when freshness
was last checked and that it expires after 24 hours; renewal remains lazy. No replacement reviewer or review swarm was
used. The same reviewer checked the measurements and confirmed the rebase preserved the feature diff; its one minor
document finding was fixed by removing an unreachable pre-rebase commit reference from the historical measurement note.

### Landing

Landed with fixes made at landing: one that the landing's checks required, and wording corrections from the landing
review. What reached main is what the report describes: new Tensorlake hosts start from a recipe's prepared snapshot,
with Farhelm already installed, instead of installing it on each host.

- #3203: the whole change, one PR, with its tests, specification, a measurement note under `lore/` and a changelog
  entry.

#### The block answered on your behalf

The plan blocked once, on whether a prepared snapshot may keep logins that a custom base already contains. The landing
monitor answered it for you under your instruction to unblock minor, reversible questions: yes, the snapshot keeps
whatever the custom base holds, Farhelm adds none of its own to it, and the base's author is responsible for that
content. The alternative was to refuse such bases or clear their logins before taking the snapshot. Where the report
calls this the "agreed" boundary, that agreement is this answer, not one you gave. TODO.md's "Plan decisions to revisit"
has an entry for it.

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping landed on main after this stack's base before the round started, and this round landed no
other plan. Two planning PRs merged while it ran (#3204, a rail notification bell and three smaller follow-ups, and
#3212, upgrade epochs and five smaller entries); they change only TODO.md and plan files, so they cannot interact.

The landing monitor and, independently, a fresh-context review read the change against the earlier Tensorlake plans it
builds on, and found nothing that needs changing:

- recipe-bases (#3178), which made every recipe build a snapshot: preparation starts from that built snapshot, and
  rebuilding it makes the prepared one stale, so the next new host prepares again. A prepared snapshot cannot be chosen
  as a recipe's base, as built ones cannot.
- the leftover-sandbox fixes (#3186, #3187): the preparation sandboxes, named `farhelm-prepare-...`, are recognized as
  Farhelm's own, so a leftover one appears under **Sandboxes not on this helm** like a leftover builder.
- name ownership (#3188 to #3190): a preparation sandbox is cleaned up from its own record, as builders now are, and an
  interrupted host create from a prepared snapshot recovers from the installation details it saved itself.
- credential delivery (#3173): preparation delivers no credentials or GitHub setup, opens no tunnels and never connects
  to the prepared supervisor; each new host gets its credentials separately, as before.

The whole supervisor state folder is removed and checked gone, and the machine ID is emptied, before the snapshot is
taken, so each new host gets its own identity.

#### Fixes made at landing

- **Migration test fixtures.** The landing's full run of the helm crate's tests failed four migration tests, which the
  executor's narrower selection had not included. They rebuild a current database as an older one through three shared
  fixtures (two of the tests use the same fixture), and none of the three removed the plan's new column for prepared
  installation details. All three now do, and all four tests pass. The product's own database upgrade code was right and
  did not change.
- **Recipe row cleanup message.** The recipe row's "could not delete" line promised that the next build would retry any
  leftover. A build only retries what a build left; a leftover preparation sandbox is retried by the next preparation or
  by deleting the recipe. The line now says the next build or preparation, whichever left it.
- **Changelog entry.** It read as if the snapshot renews in the background, and it did not say what the first host
  costs. It now says the first new host waits while the snapshot is prepared and briefly bills a preparation sandbox,
  and that nothing renews it in the background.
- **Specification wording.** SPEC.md said copied supervisor state in a custom base is refused before preparation; it is
  refused during preparation, so the recipe row shows a preparation failure, and the text now says so. SPEC.md's
  sentence about what the UI calls snapshots had become garbled ("templates means launch templates") and now reads that
  the UI calls them snapshots, never templates, a word kept for launch templates. SPEC_impl.md introduced the conditions
  that make a snapshot stale as the definition of "freshness"; they are now introduced as what makes it stale.
  SPEC_impl.md also said a host is admitted once it reaches Running; it now says admission also waits until the host
  answers over SSH, which is what the code does.
- **A flake the report did not mention.** The plan also changed an older credential-delivery test,
  `actor_hold_cancels_the_real_hook_before_acknowledging_parking`, and the report does not say why. It had failed once
  in the executor's own run: it waited to see the connection drop, but the reconnect took well under a millisecond, so
  it never saw that moment. The report says the run found no latent flake; the landing disagrees. This was a latent
  flake that predates the plan, the plan's change fixes it, and it is now recorded in FLAKES.md as fixed. It passed in
  the landing's run.

#### Things you should know

- **New hosts from one recipe wait for each other.** While a new host from a recipe is being created, up to the moment
  Tensorlake reports it running, other new hosts from that recipe, a rebuild replacing its snapshot, and deleting the
  recipe wait. Creation took under a second when measured. If Tensorlake keeps a sandbox waiting for capacity, though,
  the others wait until that create succeeds or gives up, which happens after 15 minutes. Host setups are rare, so this
  is acceptable for hosts. The dependent plan for sandbox sessions launches far more often and would inherit it; letting
  creates that only read the snapshot run together is a possible follow-up there.
- **For the sandbox sessions plan.** Asking for a prepared snapshot first builds the recipe's snapshot when it has none,
  which session launch must not do. The waiting explanation shows only for the **Prepare snapshot** step, so session
  launch progress needs its own wording.
- **Recipe row wording.** The ready line reads "Ready for new hosts when last checked · prepared … for Farhelm … ·
  expires after 24 h; checked before use", rather than the "renews after 24 h" in the plan's example wording.
- **Deleted source snapshot.** After a rebuild deletes the old built snapshot, the prepared snapshot cloned from it
  stays recorded, stale and unused, until the next preparation replaces it. That a clone does not depend on its deleted
  source is the same assumption Farhelm already makes for built snapshots. The real two-host check did not test it.

#### Checks

These ran on the PR as delivered, rebased onto main `ef2eef28` (its base plus plan bookkeeping only):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`, the check that every test delay states
  its reason (`python -B scripts/check-test-sleeps.py`, 423 delays, none without a reason), `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; one older warning
  about a citation in the triage record), `cargo build` and the UI's release web build
  (`dx build --package farhelm-ui --platform web --release`): all passed.
- The browser specs that add Tensorlake hosts, build and prepare recipe snapshots, deliver credentials, open recipe
  tunnels, show setup progress and update hosts (`tensorlake-hosts`, `tensorlake-recipes`, `tensorlake-lifecycle`,
  `tensorlake-tunnels`, `credential-delivery`, `provisioning`, `host-updates`), on Chromium and WebKit, through the test
  recorder (`python3 scripts/record-test-run.py --runner playwright --kind development --tmux required ...`,
  `npx playwright test` with those seven files, three workers, no retries), run `5d40e052-9ca2-4ac5-ba9b-97aabcca1401`:
  all 316 passed.
- The protocol and helm crates' tests (`cargo nextest run -p farhelm-proto -p farhelm-helm`, pinned nextest 0.9.148 and
  tmux 3.7c, four parallel test slots, no retries), run `de3c2c8a-7885-42df-b5c0-c1714bb9ba80`: 1839 of 1843 passed. The
  four failures were the migration fixtures fixed at landing; that run is kept as their evidence.
- The UI crate's tests in its desktop build (`cargo nextest run -p farhelm-ui --features desktop`), run
  `069b0592-a0bc-45e1-9f72-4055f6de76fd`: all 1104 passed.

After the landing fixes, the landed tree:

- `cargo fmt --all -- --check`, `dprint check`, the changelog check and
  `cargo clippy -p farhelm-helm -p farhelm-ui --all-targets -- -D warnings`: all passed.
- Every helm store and checkout-config test, including the four that had failed
  (`cargo nextest run -p farhelm-helm --lib -E 'test(/^store::tests::/) | test(/^checkout_config::tests::/)'`), run
  `9c82f649-c4e9-4c3b-8532-38d61a0447de`: all 212 passed.
- The UI's Tensorlake recipe tests (`cargo nextest run -p farhelm-ui --lib -E 'test(/^tensorlake::/)'`), run
  `11340499-e368-4545-951d-188b7ebd3697`: all 14 passed.
- The merge rebased the PR onto the two planning PRs named above. After it, main outside `plans/` matches the tested
  tree apart from their TODO.md entries.
- Reused for the landed tree: the browser, the rest of the helm and the UI runs above. The fixes after them change only
  test fixtures, one status line that no test or spec checks, and documentation.
- Reused from the executor: its real two-host Tensorlake check, which the landing did not repeat. The landing fixes
  change nothing it exercised.
- Not run: the rest of the browser suite and the other Rust crates, and the native Mac app. Nothing else depends on the
  code this changes beyond what the runs above cover.

The executor's report is otherwise still accurate, apart from the PR's draft state; it is merged.
