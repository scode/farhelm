# Podman environments

## Since the last review

Nothing from this stack had landed when the monitor returned it. The same nine draft PRs remain; #3325, #3328, #3329 and
#3330 changed. The earlier five retain their reviewed content after rebase. No PR was added or dropped.

The returned failures were diagnosed individually. Two old-database tests failed because their rewind fixture retained
new columns and history in a database labelled as an old release; the fixture now removes them. The template flag test
used the deliberately retired setting and now uses a supported template while retaining its precedence assertions. The
toolbar still locked all four actions, but its test missed two controls inside conditional fragments; it now checks the
named rendered buttons. These were test corrections, with no change to their intended product behavior.

Tensorlake's changed suspension wording was a product regression. Its host row, session card, external 30-minute idle
explanation and “resume the sandbox first” refusal are restored. Containers have their own accurate process and memory
wording. Add host's selected recipe was also a product bug: remounting native options after Back displayed the first
recipe instead. Explicit option selection now preserves the choice, with a competing earlier recipe in the test. The
shared recipe editor again explains the retained snapshot or container image as appropriate.

The Settings inventory and launcher Tab-order tests were stale: Recipes and Containers are intentional new sections, and
the three Runs on buttons intentionally precede Host. Credentials testing still navigated to Tensorlake to edit a
recipe; it now opens Recipes. The WebKit Claude-hint timeout occurred in an extra response-body completion wait after
the held clicks and successful bodyless response. The test now requires both the actual backend commit and browser
response, retaining its serialization and readback checks. The precise WebKit notification cause remains an inference.

The other returned findings are fixed. Agent refusal uses the required sandbox-and-container wording. A separate
breaking-change entry explains Delete-only legacy sessions, their still-running agent, container destruction and removal
of an old template setting. Proxy values travel through private input and a private file, rather than command arguments.
A definitively refused container create with confirmed absence settles immediately, so cleanup and recipe builds do not
wait fifteen minutes. Automatic closed-tab cleanup skips legacy containerized sessions before consuming its budget;
explicit Delete retains its cleanup.

Fresh source review found and corrected five further defects: stale successful launches could rewrite history after
identity adoption and suggestion eviction; a newly cloned supervisor could retain preparation-time proxies; package
installation lost proxies at sudo; restored placement fields could display a different recipe or host than they would
submit; and a background history refresh could replace a focused Recent setup. Old launch completions now preserve their
successful result without changing newer choices. Initial setup refreshes the supervisor's proxies before admitting a
session, package commands retain the supported proxies, and visible launch choices stay stable until a deliberate
interaction.

Broader validation also exposed three latent fixture defects. A corrupt-sentinel test had not established that its
terminal had ended; a busy-row menu retry could spend most of its budget on one dismissed flyout; and a late container
probe could scroll the launcher while a permission tooltip was pending. Each fixture is corrected without relaxing its
behavior assertions. The failed broad runs and separate corrected evidence remain distinguished below.

## What this was about

Container sessions now get their own disposable Linux machine, with Farhelm's supervisor, checkout and agent inside it.
They use the same recipes and ordinary session features as Tensorlake sandbox sessions. The earlier experimental
containerized-session mode kept the supervisor outside. Existing sessions of that kind now offer Delete only.

The launcher, templates, Clone and recent setups carry the recipe and container host. Settings has shared Recipes and a
Containers section showing host readiness and cleanup leftovers. The provider and connection abstractions encode the
shared behavior, with exhaustive decisions for Tensorlake and Podman.

## Things you should know

- Containers require Linux, rootless Podman 4.9.3 or newer, cgroup v2, the systemd cgroup manager and working user
  scopes. A recipe builds on its first launch at a container host and its preparation is refreshed when stale.
- The checkout lives inside the container. Suspend freezes processes while retaining memory. A stopped container is
  started once when its host is connected. Session outcomes follow the ordinary reboot and exit rules, and captured
  resumable conversations offer Restart.
- User-confirmed Delete destroys the whole environment, every session and process in it, and its files. Failed Replace
  preserves the existing environment and files. These follow the latest sandbox ownership Decisions, which supersede
  older planner examples about counting sessions before deletion.
- Session-owned hosts remain visible in Hosts and the ordinary attention and update surfaces. Their permanent rail icons
  and ordinary destination choices are omitted. Agent requests from environments remain refused at the helm. Agent
  launch approval cards and environment-aware CLI operations belong to the dependent plan.
- SSH environments share the container host's SSH connection. The usual ten-channel server limit can reduce capacity,
  especially when another login requires interactive approval. Ordinary OpenSSH fallback is retained.
- Ambiguous image commits retain their recorded cleanup identity. An unresolved image tag can remain recorded
  indefinitely until a positive ownership observation permits cleanup. Cleanup errors remain visible for explicit retry.
- Real acceptance uses a Claude-shaped stand-in with the actual injected hook and conversation format. It exercises
  ordinary terminal, tab, file, transfer, credential and framed TCP-relay protocols. Native desktop opening and browser
  file-hover/opening gestures rely on source review, desktop compilation and asset JavaScript evidence. Their full
  end-to-end gestures remain unverified by these real-container runs.
- Real local acceptance passed, including public checkout, delivered credentials, ordinary feature protocols, lifecycle
  recovery, confirmed Delete and nonforcing cleanup of recipe images.
- SSH acceptance passed on one Tensorlake test sandbox used as a plain SSH container host. Exactly one sandbox was
  created for the plan. Its evidence was copied back before termination, and its automatic suspend snapshot was removed.
  Independent inventories confirmed the sandbox and every snapshot attributed to it were gone.
- Real recovery validation stops the container externally and verifies one automatic start. An actual container-host
  reboot was not performed. The existing supervisor boot-classification tests cover interruption of sessions last known
  live across a changed boot and preservation of known exits. A same-host container stop returns ordinary exited
  sessions. The original fixture incorrectly demanded Interrupted for that incident, and the new specifications and
  guide were qualified to preserve Farhelm's existing durability contract after an independent scope assessment.
- Changing or removing a container host's proxies after preparation is covered by production control-flow tests, and the
  package commands have an actual child-environment test with a strict sudo stand-in. A changed proxy reaching an actual
  session through a real systemd user manager while reusing the same prepared image remains unverified.

The earlier Podman review findings were resolved as follows.

- Podman's detach keys could leave an agent running while Farhelm showed it as exited, allowing Delete without the
  live-agent warning. Agents and tabs now use the inside supervisor's ordinary terminals, removing that interactive
  Podman attachment path.
- A token file inside a writable checkout could be replaced with a symlink, causing a readable host file to be supplied
  as the GitHub token. Container recipes now select named catalog credentials and deliver them inside the container.
- Proxy inheritance could silently include credentials from the host's environment. The agreed behavior retains
  inheritance, now deterministically from the host's systemd user manager, including credentials in configured proxy
  URLs.
- Retrying an earlier experimental creation after a crash could delete an initialized container and its files. Those
  retired creation requests now refuse while preserving existing containers for explicit Delete. New environment
  launches record ownership before creation and retain failed cleanup for explicit retry.
- Inspecting a checkpoint image could restore its old workload. Images carrying checkpointed process state are refused
  as recipe bases.
- Agent launch approval cards still need to show the recipe and container host and prevent older clients from treating
  those requests as ordinary launches. Agent-initiated environment operations remain refused, so those cards and the
  environment-aware CLI belong to the dependent agent-operations plan.

## Open questions and possible follow-ups

Ordinary SSH hosts still require their agent credentials to be configured on the host. Selecting a named entry from
Settings → Credentials for automatic delivery to an ordinary SSH host remains future work. The agreed scope covered the
container half of that work through recipes, while leaving ordinary SSH hosts for a later change. The requested future
macOS-container and agent-started-container entries were reworded for the new model. Documentation covers the retained
container-guide URL and fourteen linked maintained pages, including session launch, lifecycle, templates, hosts,
credentials, security and uninstall behavior.

The frozen Overview page still mirrors older README claims. Its three proposed follow-ups are retained for review.

- In “Agents start sessions”, change “Tell your agent to” to “On an ordinary host, tell your agent”.
- In “Version control”, replace “a fresh clone for each session, moved to the trash when you are done with it” with “a
  fresh clone for each session, moved to the trash when you are done with it on an ordinary host. In a disposable
  container or sandbox, Delete permanently removes it with the environment.”
- Replace “The helm reaches each supervisor over SSH.” with “The helm reaches remote hosts over SSH. Experimental
  container sessions run their own supervisor and checkout inside rootless Podman on a Linux host, reached through that
  host.” Link “container sessions” to the existing container guide.

## The PRs

- [#3305](https://github.com/scode/farhelm/pull/3305/changes) establishes the specifications and provider encoding rule.
- [#3307](https://github.com/scode/farhelm/pull/3307/changes) introduces the shared environment provider contract.
- [#3311](https://github.com/scode/farhelm/pull/3311/changes) shares remote connection and provisioning routes.
- [#3314](https://github.com/scode/farhelm/pull/3314/changes) generalizes recipes and preparation by place.
- [#3321](https://github.com/scode/farhelm/pull/3321/changes) records container environments and host readiness.
- [#3325](https://github.com/scode/farhelm/pull/3325/changes) preserves reusable environment launch history.
- [#3328](https://github.com/scode/farhelm/pull/3328/changes) launches and manages container sessions through the helm.
- [#3329](https://github.com/scode/farhelm/pull/3329/changes) adds launcher, template and Settings controls.
- [#3330](https://github.com/scode/farhelm/pull/3330/changes) retires the earlier containerized mode and updates the
  guides and TODO dispositions.

## Checks run, reused and skipped

The returned plan required full workspace Rust tests and both engines' affected-screen browser specs. The Rust run
finished with one existing fixture failure and 6,067 other passes; the eight browser batches finished with two fixture
failures, 3,282 passes and 14 configured skips. Each failure was retained, diagnosed and corrected, with separate narrow
passing evidence below. Those original failed commands remain failed observations.

- `cargo nextest run --workspace --exclude farhelm-desktop`, run `a6a1f51c-6706-4333-a9af-bbdbb8d1504d`, executed 6,068
  tests: 6,067 passed, one failed and 72 were ignored. Its two setup steps passed separately. Clean source
  `b1640e841c311adb3b630ac378f28ebcc593dfab`, fingerprint
  `74268f54cf7fc698f3e56fdbe6ce18471059a9a537e27adaf1436937d75f58be`. Pinned nextest 0.9.148, four slots, zero retries
  and pinned tmux 3.7c. Complete report/output, no forwarding loss and no runtime substrate skips. The real local Podman
  case passed in 221.592 seconds, including checkout, ordinary features, lifecycle and cleanup.
- That run's corrupt-sentinel test treated an unavailable terminal as proof it had ended, allowing its runtime to stop
  before reconciliation. The fixture now shuts down its owning runtime and requires confirmed absence before planting
  the corrupt evidence, preserving all original durability and repair assertions. The exact corrected case passed in
  `40c6da7b-f7f8-4631-9d6b-7b1beecc9f82`, one actual test in 0.543 seconds plus separate setup, clean `28961fd9`,
  fingerprint `cc09eb103e30d13175ef96491fac684b3dd0e625eabb80c25160a5c4640c90a7`. Six complete isolated old-source
  passes remain classification evidence; one run with incomplete console forwarding is excluded.
- Subsequent Rust changes only remove needless returns in thirteen test-fake match arms. Their eleven failure-model
  cases passed in `141ae218-24ed-4956-8978-2703ee4b3ba9`, using the workspace graph with
  `-E 'test(environment::podman::failure_tests::)'`; no setup step or runtime skip. Clean `d10f932f`, fingerprint
  `4aac83272c0cf621abbc4433b60782f495bcc0685b14c4680b2de831f505867d`. Both all-target and shipped-binary Clippy commands
  passed in `ccac7213-1e29-46b5-be32-a2f2169f60e4` on that same source. The initial Clippy failure remains retained. No
  production Rust changed after the full run.
- Eight recorded `npx playwright test <whole-spec-file-patterns>` batches covered all 130 ordinary specs, once per
  engine: 3,298 listed cases, three workers with one owned stack each and zero retries. All eight reports, source
  records, output and forwarding were complete. The portable summary explicitly discovered all eight runs. Chromium had
  1,646 passes, two skips and one failure; WebKit had 1,636 passes, twelve skips and one failure.

| Batch | Recorder run                           | Actual result                                    |
| ----- | -------------------------------------- | ------------------------------------------------ |
| 1     | `e6093065-7afe-4ffa-97af-8daf4602895e` | 414 passed                                       |
| 2     | `1cd61d98-e960-43ea-b40c-985de6c586c7` | 411 passed, one WebKit busy-row failure          |
| 3     | `fe2fe85d-4ae6-44f5-933f-63bdd6bacc94` | 403 passed, nine WebKit clipboard skips          |
| 4     | `28315abf-d086-4c99-9404-79e0eb4df54a` | 412 passed, including all Podman specs           |
| 5     | `2a5835a5-ec22-42b9-8c30-62af9e59cfe7` | 411 passed, one WebKit visual-matrix skip        |
| 6     | `041d85b0-29c6-4d7c-bda4-ff4d441c44d5` | 410 passed, two vendor-agent skips               |
| 7     | `b20e250a-70fd-44eb-82ae-5efd362202ef` | 410 passed, two vendor-agent skips               |
| 8     | `96c9c47c-734d-4578-9e50-92a7353a1c6a` | 411 passed, one Chromium permission-help failure |

Batches 1–2 used clean `fe085427ccfd53c2c27371f5fb5f49b9c24126ac`, fingerprint
`c18bdd4c05b26e2e004a0e0c9a342696554048795642efff9b53347eb8ee609b`; batches 3–8 used clean
`31449dd259cd7f73c988df12aa4af9422c0cd9d6`, fingerprint
`e85a68b9b3a665e98ca5f2370774de26f03fe8b39ab5eeeee3a92f1050b86fbf`. The intervening browser-helper correction preserves
every other caller's defaults; the later batches exercise those callers again. All skip annotations were read: WebKit
cannot grant the clipboard permissions used by nine cases, one visual matrix is intentionally Chromium-only, and four
cases require actual vendor turns. These are missing coverage, not passes.

- The busy-row failure spent five seconds of an eight-second retry waiting for an already dismissed flyout. The next
  attempt reached the correct disabled Settings item, but the outer deadline expired just before its assertion passed.
  That case alone now opts into one-second inner waits, retaining native input, measured-menu readiness, all state
  assertions and the outer bound. The exact corrected case passed on both engines in
  `d1fab191-7f8e-4a7b-85cb-05d4d273f55a`, two actual tests at clean `d10f932f` and fingerprint `4aac83272c0c…`. Three
  old-source WebKit repetitions also passed. The original flyout dismissal's precise trigger remains unknown.
- The permission-help capture showed a late usable container-host probe adding Runs on. Browser scroll anchoring
  scrolled its form 99.3 milliseconds after Guarded pointer entry, before the tooltip's 300-millisecond rest delay;
  ancestor scrolling intentionally cancels pending help. This ordinary-host fixture now supplies an empty environment
  catalog, preserving every original native-hover, phone, mode, flags and paint assertion and its timeouts. The
  temporary observer and unproven tall-viewport experiments were removed. Exact noninstrumented run
  `ee434afd-f6b1-441e-8c4d-b48419e091f1` passed twenty repetitions, ten per engine, with complete generic console
  counts. Controlled whole-spec run `957ecd13-c918-4a37-973a-e64926d8e237` passed four actual cases, two per engine,
  with no skips and a complete report. Both used clean `cb6e71ca64d82e9757c00c86273bb303f94c171e`, fingerprint
  `58087882d456e380b94f6281593c45d292f9dc7d34a95a833099c451629dd603`; later changes only append and format its flake
  record. Diagnostic `6051ecc3-f3ba-42dd-a4f2-ab063ac8a542` stopped at one failure with one pass, two interrupted and
  sixteen unstarted planned cases; it establishes the captured sequence, not a validation pass. Earlier failed
  tall-viewport experiments remain retained, and their same-session fixture errors are not latent-flake entries.
- `cargo check -p farhelm-ui --features desktop` and `cargo check -p farhelm-desktop` passed in
  `41a5e342-b111-4eb6-955d-4bcda0092754` at clean `31449dd2`, fingerprint `e85a68b9b3a6…`.
  `cargo nextest run -p farhelm-ui --features desktop` passed all 1,156 tests, with no skips, in
  `bcf0fc14-f477-4c9c-9a70-b1717a4fa15b` at clean `8f9910a6`, fingerprint
  `709d6b02a6ac678d8a3a4c4286cc40594ae80356907a9c843c0ef16bafd13278`. Later changes are browser fixtures and prose only;
  these desktop checks retain their coverage. They do not establish native GUI gestures.
- Workspace doctest discovery/compilation and the separate desktop-feature variant completed successfully in
  `c975a13c-b533-4978-97c6-517433a7409b` at clean `e11458a0`, fingerprint
  `9a6d3093e0b8c3032ddef116cf3f479526bf92190afbebb6e7f43067aa1b72da`. Commands were
  `cargo test --locked --doc --workspace --exclude farhelm-desktop -- --show-output --test-threads=4` and
  `cargo test --locked --doc -p farhelm-ui --features desktop -- --show-output --test-threads=4`, serially. All nine
  workspace targets and the desktop variant contained zero actual doctests; this is compilation/discovery evidence, not
  executed doctest coverage. Later changes only wrap the new flake entry.
- The terminal-file asset's 37 JavaScript tests passed in `7783f601-cf5d-4cc9-8bc1-3d9af5810b9e` at clean `b1640e84` and
  fingerprint `74268f54cf7f…`. The normal CLI/fixture build and pinned release web build also passed there. Their
  production inputs are unchanged by the later Rust-test, browser-fixture and prose corrections, so the browser runs
  reuse those builds.
- SSH real acceptance reuses worker run `61f3c0fd-04de-4adf-a5a2-991f7a1c7f7b`, one actual pass in 96 seconds plus
  payload setup at clean original `a77c1394`. The archive selected the exact ignored
  `podman_environments::real_ssh_environment_managed_checkout_and_ordinary_features` case with four slots and zero
  retries. Strict localhost SSH, raw JUnit, console and summary were checked together before the sandbox was terminated.
  Its generic recorder supplies no normalized nextest counts. Parent record `643bb8a9-f8af-4964-8e24-811de1e97271`
  retained imported evidence; successful default-root slimming had already removed detailed traces. Later fixes have
  current local runtime and source evidence, but this is not a new real-SSH pass for those fixes. The one-sandbox total
  budget is consumed and its cleanup is complete.
- `deno test -A scripts/tensorlake/rust-tests.ts` passed six archive-configuration tests in
  `db4e4279-0cde-427e-a06c-ca237adb4302`, complete source/output and no forwarding loss, at clean
  `bb138fa025a4e421c377f19a6651d59d99b32992`, fingerprint
  `768bb166d34c8126a5e0b452e55114c94fe4283c7ce6eb8d616b9a6b01a42684`. This starts no cloud machine. A fresh
  `bun run build` at that same revision built all 36 website pages and validated every internal link.
- Current `cargo fmt --all -- --check`, `git diff --check`, full `dprint check`, instruction lint, changelog format,
  Bash syntax/ShellCheck for the three changed shell scripts, and Deno type-check passed. The test-delay check inspected
  424 delays with zero missing rationales. Instruction lint reports one pre-existing archived-heading warning and zero
  errors; changelog format covers 45 fragments. Generated release-workflow parity passed using the required dist 0.32.0;
  the ambient 0.34.0 invocation refused the version mismatch and is not a pass. The final documentation pass covered
  each corrected source and test file.
- Hosted CI, another cloud allocation, installer/provisioning platform suites, native GUI smoke and actual vendor turns
  were skipped: they do not add required coverage within this plan's substrate and budget. No live installation was
  changed. Published release shots and historical material were left alone. Unaffected full-suite commands were not
  repeated after test-only corrections; each correction has focused evidence above.

## Review gate outcome

Every PR passed its required single fresh-context Sol-high source review. Accepted findings were fixed and checked again
by the same reviewer. Required Opus consultations covered the significant abstraction proposals. Independent scope
audits accepted the reduced lifecycle contract, existing-handler UI corrections, ordinary reboot/exit classification,
captured-claim history guard, narrow proxy refresh and stable Recent choices. The final hover scope audit rejected
speculative whole-dialog fit and accepted bounded diagnosis; the same source reviewer then accepted the fixed catalog
and the flake record. All nine PRs were independently verified open and draft at the expected pushed heads and linear
bases. Nothing from this stack was merged by the executor.
