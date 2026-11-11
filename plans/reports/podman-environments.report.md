# Podman environments

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

Current targeted checks cover the retirement boundary and interactions with newly landed SSH identity, Trash updates,
host-add focus handling and approval geometry. The stack was carefully rebased onto current main, preserving those
contracts. Earlier successful checks are reused per command where the intervening changes preserve their coverage.

- `cargo nextest run` on the retired-session, cleanup, SSH-route and Delete/Trash selections passed 69 tests in
  `abc2f359-d40d-4016-8801-1f3972f3cf02`. Four legacy-session cases use stored rows, and the prompt-drift retry
  regression uses ordinary admission. The setup step is counted separately from the 69 tests.
- Both-engine browser runs passed the exact two retirement cases in `652e3fba-668e-424b-88cc-cefaca2f0ea9` and the
  remaining 18 targeted cases in `3dfaad15-2e0c-4dfa-8cb8-545217ae43b2`. The first attempt
  `2d03b916-3460-4fc8-abbc-2d4f79e04784` exposed an incorrect new-test Restart selector and exceeded the policy-report
  size limit with its long evidence path. The selector and evidence location were corrected, and that failed attempt
  remains retained.
- Asset JavaScript passed 37 terminal-file cases in `d779f73e-74dc-401b-91e3-501cf883039c`. The archive configuration
  checks passed six cases in `20c19f26-2767-4277-b2a1-b29aa8ae5b23`. Imported sandbox harness checks passed five cases
  in `fe3ef151-ce9d-463c-87d2-fccdc454b2ed`.
- Local real acceptance and the three existing boot-classification cases passed in
  `1072cb0c-9842-4553-91bb-fdd5fd1e2125`, four actual tests and two separate setup steps. The real case took 225
  seconds. The recorded command selected the real local lifecycle case and the three ordinary boot cases with
  `cargo nextest run --locked -p farhelm -p farhelm-supervisor --lib --test e2e -E <four-test-selection>`.
- The archive worker selected the exact ignored SSH case with
  `nextest run --archive-file <archive> --partition hash:1/1 --test-threads 4 --retries 0 --run-ignored only -E 'test(/^podman_environments::real_ssh_environment_managed_checkout_and_ordinary_features$/)'`.
  Worker run `61f3c0fd-04de-4adf-a5a2-991f7a1c7f7b` passed one actual test in 96 seconds and one separate payload setup
  step. Its generic recorder has no normalized nextest counts, so raw JUnit, console and summary were checked together.
  Source was clean at `a77c1394`, strict localhost SSH succeeded, the required base was present and there were no
  runtime skips. The imported evidence includes every current run record, JUnit, console, summary and configuration.
  Successful default-root trace slimming had already removed its detailed traces. The parent client record is
  `643bb8a9-f8af-4964-8e24-811de1e97271`.
- The first real local attempt `e42111b2-a5e3-40ff-a6ad-342c52c661ae` found a launch race between the container reaching
  Running and its user bus becoming available. Launch now waits with the existing bounded, read-only readiness script.
  Twenty-four focused readiness, launch-ordering, failure-cleanup and shared-preparation tests passed in
  `d49963ab-2c01-4bc7-8778-abb30ea4e4c7`. The second real attempt `e96663ec-4075-4453-bc4f-7e6c149474b9` reached the
  running agent and managed checkout, then found a test assertion passing a full Git origin URL to the owner/repository
  identifier parser. The assertion now checks the canonical clone URL. Both whole failed attempts remain retained.
- The third local attempt `dcc39971-f51d-491e-895b-e9c32c0779b8` passed the ordinary feature protocols, pause/Resume,
  explicit Stop and conversation Restart, helm reconnection and the automatic container start. It then exposed that
  incorrect Interrupted assertion. Its whole failed record remains retained. The separate owned probe
  `def1c682-63b2-49ee-898c-618a1e33a244` confirmed that a container stop/start leaves the host kernel boot unchanged and
  removed its probe container after checking absence.
- Formatting, touched-crate all-target Clippy, shipped-binary Clippy, desktop compilation, the pinned web build, the
  36-page website build and internal-link validation, instruction lint, test-delay lint, shell syntax and ShellCheck,
  Deno checks, generated release-workflow parity and changelog format passed. The static provisioning CLI and pinned
  tmux both passed static-ELF checks. The test-delay check inspected 424 delays with zero missing rationales.
- Earlier command identities and tested source snapshots are listed below. Those commands ran with tracked changes on
  the named HEAD, so the fingerprint identifies the tested snapshot including its delta. Hashes are abbreviated. Prior
  failed commands remain failed observations, with corrected cases verified by separate retained retries.
- The full workspace and browser batteries, hosted CI, installer suites and native desktop runtime were skipped.
  Targeted checks cover the changed behavior and rebase interactions. No live installation was used. Published release
  shots and historical material retain their existing source.

### Reused command identities

All rows passed with complete source and output records. Each command's `-E` argument is its recorded selection.

| Command                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Recorder run                                      | Tested source                               |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- | ------------------------------------------- |
| `cargo nextest run -p farhelm-helm --lib -E 'test(the_environment_gate_decides_every_status) \| test(tensorlake_status_translation_preserves_every_state) \| test(an_unanswered_status_check_dials_anyway) \| test(a_suspended_sandbox_is_never_dialed_and_resumes_by_itself) \| test(a_deleted_sandbox_is_unreachable_with_the_remedy) \| test(the_store_refuses_to_lose_a_sandbox)'`                                                                                                                                           | `b6f49cee-12a7-41e0-9467-eb9c0fb9d0e9`, 6 tests   | HEAD `253639d3`, fingerprint `9fe7eb009668` |
| `cargo nextest run -p farhelm-helm --lib -E 'test(/^tensorlake::hosts::tests::/) \| test(recipe_session_) \| test(route_replacement_covers_the_hook_publication_gap_and_reports_failures) \| test(actor_hold_cancels_the_real_hook_before_acknowledging_parking)'`                                                                                                                                                                                                                                                               | `3faa7ae1-dd9a-443d-a8a2-7eb29cc29531`, 36 tests  | HEAD `253639d3`, fingerprint `a4d58cd08b2f` |
| `cargo nextest run -p farhelm-helm --lib -E 'test(/^access::/) \| test(/^ssh::/) \| test(/^transport::/) \| test(/^provisioning::/) \| test(/^tensorlake::shell::/) \| test(/^manager::tests::ssh_/) \| test(local_container_exit_does_not_claim_an_ssh_failure) \| test(nested_container_channel_refusal_names_the_place_host)'`                                                                                                                                                                                                | `996c006c-afba-41f7-87e8-60e8e33e7489`, 400 tests | HEAD `6d2e38aa`, fingerprint `f75fa1f58e1c` |
| `cargo nextest run -p farhelm-helm --lib -E 'test(/^access::/) \| test(/^transport::/) \| test(/^manager::tests::ssh_/) \| test(local_container_exit_does_not_claim_an_ssh_failure) \| test(nested_container_channel_refusal_names_the_place_host)'`                                                                                                                                                                                                                                                                             | `fbb65d0f-e934-438e-8eb1-71d67df4b629`, 24 tests  | HEAD `a06d463b`, fingerprint `6335899c5453` |
| `cargo nextest run -p farhelm-helm --lib -E 'test(local_container_exit_does_not_claim_an_ssh_failure) \| test(nested_container_channel_refusal_names_the_place_host) \| test(session_refusal_survives_later_fallback_diagnostics)'`                                                                                                                                                                                                                                                                                              | `f4b45c6e-210d-4052-adff-d1e2c99d519e`, 3 tests   | HEAD `a06d463b`, fingerprint `3606dced2401` |
| `cargo nextest run -p farhelm-helm -p farhelm-proto -p farhelm-ui --lib -E 'test(/^recipes::tests::/) \| test(/^environment::recipes::tests::/) \| test(/^tensorlake::recipes::tests::/) \| test(/^tensorlake::build::tests::/) \| test(/^tensorlake::prepared::tests::/) \| test(/^provisioning::tensorlake_add_tests::/) \| test(/^tensorlake::recipe_editor::tests::/)'`                                                                                                                                                      | `0898d080-5533-47e2-8e49-47b3107c9bbe`, 95 tests  | HEAD `ef3a8194`, fingerprint `0041ede355d0` |
| `cargo nextest run -p farhelm-helm --lib -E 'test(tensorlake_add::tests::)'`                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `c7a814b8-4ef7-4535-aed8-a0430ae813c5`, 25 tests  | HEAD `ef3a8194`, fingerprint `0041ede355d0` |
| `cargo nextest run -p farhelm-helm --lib -E 'test(=store::tests::history_schema_migrations_discard_schema_22_history_while_preserving_the_host_registry) \| test(=store::tests::schema_28_preserves_history_without_inventing_repo_intent) \| test(=store::tests::schema_82_adds_ownership_without_cascading_host_rows) \| test(unknown_provider_refuses_discovered_ssh_convergence_without_mutation) \| test(update_refuses_unfinished_container_before_probing) \| test(copied_supervisor_state_refuses_before_installation)'` | `343459cf-1942-477f-98b3-0fd8526ceacd`, 6 tests   | HEAD `122ebdbf`, fingerprint `1527ee303b3c` |
| `cargo nextest run -p farhelm-helm --lib -E 'test(environment_history) \| test(schema_66_starts_empty_visits_and_preserves_legacy_preferences)'`                                                                                                                                                                                                                                                                                                                                                                                 | `b8c9959b-ab6f-4f51-af2f-58976283c72d`, 9 tests   | HEAD `ed4c15b2`, fingerprint `ef8f0d11ba20` |
| `cargo nextest run -p farhelm-helm --lib -E 'test(=environment::recipes::tests::podman_places_round_trip_independently_of_tensorlake_state) \| test(=environment::recipes::tests::every_owned_podman_resource_blocks_recipe_removal) \| test(=provisioning::service::tensorlake_add::tests::an_unrequested_name_collision_refuses_setup_and_delete_leaves_it_alone)'`                                                                                                                                                            | `a8827950-615b-4f2a-b1c0-df720cc17f07`, 3 tests   | HEAD `b725cbeb`, fingerprint `53289bf8515d` |
| `cargo nextest run -p farhelm-ui --lib -E 'test(=hosts::tests::a_tensorlake_host_is_deleted_not_removed)'`                                                                                                                                                                                                                                                                                                                                                                                                                       | `35d1e34b-36b8-42b5-8130-67996f4c2132`, 1 test    | HEAD `47a026a1`, fingerprint `c248ae62ddd8` |
| `cargo nextest run -p farhelm-ui --lib`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `b7d6f858-191b-4e05-9e30-70af0df7029c`, 964 tests | HEAD `47a026a1`, fingerprint `d66378556b51` |

The provider and Tensorlake lifecycle rows retain their delegated status, destruction and actor behavior. The routing
rows cover the shared connection paths, followed by the two narrower commands for their corrections. The later 69-test
command covers the newly landed Tensorlake SSH identity restriction and retirement interactions. Recipe and Tensorlake
creation rows retain their shared catalog and installation behavior. Storage's migration was mechanically renumbered on
rebase while preserving its SQL operations and historical-data assertions. Intent-history behavior is preserved. These
review units had empty per-unit interdiffs after rebase. The later container boot wait is separately covered by the
24-test command and both real runs.

The exact host-Delete correction and full UI command cover the preserved launcher, template and Settings logic. Later
targeted Rust and both-engine browser commands cover retirement, approval geometry and presentation changes. The full UI
run is reused for that preserved logic, with the later commands supplying evidence for the changed portions.

## Review gate outcome

Every PR passed its required single fresh-context Sol-high source review, including the rebase and subsequent narrow
corrections. Accepted findings were fixed and checked again by the same reviewer. Required Opus consultations covered
the significant abstraction proposals. The independent scope audits accepted the reduced lifecycle contract, the
existing-handler UI corrections and the ordinary reboot/exit classification correction. All nine PRs were independently
verified open and draft at their expected heads and bases.
