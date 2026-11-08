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
