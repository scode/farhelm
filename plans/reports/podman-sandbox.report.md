## What this was about

Farhelm's ordinary process cleanup does not isolate an agent from the host account. This adds optional, experimental
Linux sessions in rootless Podman: the agent and its terminal tabs share one container, with the managed checkout, one
selected host loopback API-proxy port, and optionally a GitHub token. The launcher, templates, file links and lifecycle
operations all preserve that choice. The feature is subject to rapid change.

## Things you should know

- You supply an OCI image archive on the session host. The image needs a uid/gid-1000 user, Bash, sleep, the agent and
  its API-proxy configuration; GitHub use also needs Git/gh configured for GH_TOKEN. Farhelm builds or downloads no
  image and bypasses its usual startup command. Subscription logins are unsupported.
- Only managed checkouts and Agent launches are supported. The whole verified checkout is shared, including launches
  from a subdirectory or through a symlink alias. A copied existing managed directory remains reusable on the same host.
  Fresh launchers start with containerization off and empty fields; templates, Recent setups, Clone and Replace with
  carry the complete choice.
- There is no Farhelm program, credential, socket, browser fallback or conversation integration inside the container.
  Status still comes from the agent's screen. Restart and Restart with are unavailable; Replace starts fresh. After a
  host reboot, replace or delete the session.
- Stop ends the whole container, including tabs and detached work. The container's files remain, and opening a tab
  starts that existing container again. Closing one tab ends its shell but can leave work it detached. The container
  also stays running after the agent exits on its own.
- Delete keeps its ordinary read-only preflights and confirmation. A refused preflight leaves the container intact;
  confirmed cleanup uses Podman's actual state. Delete can remove detached work even after the displayed agent and tabs
  have ended. An image import can delay Stop/Delete: after 30 seconds waiting, the operation refuses without signalling
  processes and asks for a retry after preparation finishes.
- Both printed file paths and explicit terminal file hyperlinks are restricted to readable files inside the shared
  checkout, including symlink and dot-dot checks. The actual download checks independently. Hyperlinks retain their
  labels and support filenames with spaces. Localhost tunnels still reach the host, not a server inside the container.
- The optional token file is read afresh for the agent and each tab. Farhelm does not save its contents or put them on a
  command line; the user's own Podman tools can inspect the receiving exec session's environment.
- This adds no CPU or memory limit. Internet, LAN, tailnet and cloud-metadata access remain open. The user's Podman
  configuration still applies, including extra mounts or environment settings they deliberately configure.
- Files written as the image's uid-1000 user belong to the host user. Container-root files can need `podman unshare` to
  edit or remove; root-created directories can also prevent Empty Trash. Delete removes the container but leaves
  imported images in Podman's storage until the user prunes them.

The public setup page is `docs/agents/containerized-sessions/`, beside Agent wrappers. It has an Experimental badge and
describes these prerequisites and limitations. The existing source TODO is narrowed to the work that remains, with its
plan reference retained.

The first two proposed PRs were combined so no intermediate revision could accept container intent without its runtime.
Creation and cleanup share a per-session gate so a delayed image import cannot recreate a container after Stop or
Delete. An early terminal draft disabled explicit file hyperlinks; that would omit required file-link behavior, so the
final version preserves their labels and filenames while checking that each target stays in the shared checkout. No
requested isolation guarantee was weakened.

## Open questions and possible follow-ups

The narrowed source TODO retains macOS desktop support, image assistance and stronger isolation. Existing separate TODO
entries retain plain-directory launches, CLI flags, fuller Farhelm integration, restart support, uid/path matching and
container-aware localhost tunnels or file access outside the checkout. These were deliberately outside this plan.

No maintainer decision remains unresolved for this implementation. The isolation and networking promises depend on the
documented rootless Podman setup; the runtime evidence is from Linux with Podman 4.9.3 and the offline Ubuntu 24.04
image, not a claim about every image or host distribution.

## The PRs

- [#2791](https://github.com/scode/farhelm/pull/2791/changes): launch/template intent, protocol 66, container creation,
  isolated agent execution, Stop/Delete, isolation proof and experimental changelog entry.
- [#2792](https://github.com/scode/farhelm/pull/2792/changes): terminal tabs in the retained container and
  checkout-confined file links/downloads.
- [#2793](https://github.com/scode/farhelm/pull/2793/changes): launcher controls, validation, copied setups, templates
  and source TODO narrowing.
- [#2794](https://github.com/scode/farhelm/pull/2794/changes): user-facing setup and limitations page.

## Checks run, reused and skipped

Run for this stack:

All runtime commands below used `python3 scripts/record-test-run.py`. Nextest ran with four global slots, zero retries
and required pinned tmux; each real Podman selection contains one product case plus fixture setup. Browser commands ran
from `e2e/` on both engines with three workers, one owned stack per worker, zero retries and required pinned tmux. The
JavaScript command ran from `crates/farhelm-ui/js-tests/` with Node's default within-file concurrency and no tmux.

- `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --lib -E 'test(container) or test(flags_edit_the_templates_result_and_win_over_it)'`:
  13 passes, run `f84474ae-4a82-4e7c-a40c-0fbb250ae120`. This checks the new launch/template and creation-fence
  boundaries; one selected existing repository test is not new feature coverage.
- `cargo nextest run -p farhelm --test e2e -E 'test(=containerized_sessions::podman_isolation_and_stop_delete_cover_scoped_and_unscoped_launches)'`:
  run `943bd078-465f-48b9-babf-da6d6671a224`, one product case plus fixture setup, passed. It exercises actual scoped
  and unscoped launch, host-file isolation, selected proxy forwarding and refusal of another host loopback port, uid
  mapping, token delivery, marker-free container helpers, Stop and Delete.
- `cargo nextest run -p farhelm --test e2e -E 'test(=containerized_sessions::podman_tabs_and_downloads_share_the_existing_container)'`:
  run `71e9b24b-b534-4d71-977d-b037b3111a28`, one product case plus setup, passed. Both scope modes ran, including the
  dedicated helper-scope assertion. It covers fresh tokens, host-startup bypass, canonical cwd, natural agent exit,
  detached work surviving tab close, whole Stop, retained-container reopen, missing-container refusal, actual
  multi-frame downloads and outside/symlink escapes. No runtime prerequisite skip.
- `cargo nextest run -p farhelm-ui -p farhelm-supervisor --lib -E '(package(farhelm-ui) and (test(container) or test(create_form::) or test(save_template::) or test(templates::) or test(session_view::) or test(launch_composer::))) or (package(farhelm-supervisor) and test(service::downloads::tests::))'`:
  238 passes, run `0f67951e-16f3-470b-8076-9aa4295dd529`. Reused after the later renderer text-helper/style correction
  because the tested unit logic is unchanged; that correction has separate browser evidence below.
- `node --test terminal-files.test.js`: 39 passes, run `cd8081c5-b5f1-4a83-8a99-5d575c11aaac`. Reused unchanged source
  through the final split; includes the actual shipped terminal bundle's native file-hyperlink provider, stale replies,
  disposal and independent link checks.
- `npx playwright test 'github-checkout-composer\.spec\.ts' -g 'container choices require a managed checkout'`: two
  passes, Chromium and WebKit, run `ddeb3b78-60d6-4095-8987-f43b2abc1596`. Includes empty inputs, request contents and
  actual control geometry at 1100px and 390px. The retained screenshots were inspected on both engines.
- `npx playwright test 'github-checkout-composer\.spec\.ts|templates\.spec\.ts' -g 'container templates|save as template captures|a template fills the command tab|tl: applies templates|legacy switches and agent choices|Replace with accepts its own template host|destination type keeps unselected repositories|manual checkout needs selection'`:
  18 passes, nine per engine, run `65f36483-f7e8-46a4-90a7-c0f288eb1c2f`. Covers complete container-template
  edits/apply/save/clear, template stacking, ordinary Agent and Command saves, Replace-with host checks and pending
  checkout choices. Both engines' template screenshots at 1100px and 390px were inspected. All browser reports are
  complete, without skips or retries.
- `cargo clippy` with all targets for the five affected packages, and a separate `cargo clippy -p farhelm --bins`, both
  with warnings denied. After the UI correction, the UI package's all-target Clippy passed again. The second command
  checks the shipped binary without integration-test seams.
- Actual CLI build and web release build passed; the web build was repeated after the renderer/style correction.
  `cargo fmt --all -- --check`, dprint on changed Markdown, changelog format, the test-delay source check (327 delays,
  zero unjustified) and the instruction check passed. The instruction check retains three pre-existing citation warnings
  and no errors. The final website build produced 33 pages with all internal links valid.

Earlier failed and interrupted development attempts are retained in the plan's working log and private recorder data;
they are not counted as passes. The tab case initially exposed fixture mistakes about real detachment, viewer input
ownership and the in-process harness's absent cleanup ticker. The first browser run
`70d4a141-52d7-4c5e-bd98-716e806633f6` exposed the real empty-editor placeholder bug on both engines (the template case
passed); the corrected exact run above passes. Same-session issues were fixed here, not recorded as latent flakes.

Reused earlier author evidence: the inspected 263-case backend compatibility selection
`c44a8fdf-ee98-4426-b729-fa5cfd05863c` and seven-case container/ordinary-lifecycle selection
`6bb379ac-c4ab-40b6-b4e7-0f121ea1e5e7` cover the integrated backend source. The executor also reran the focused units
and actual Podman cases above after integration. Review and commit metadata changes preserve those tested code paths.

The author commands, under the same recorded nextest policy, were:

- `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --lib -E 'package(farhelm-proto) | (package(farhelm-helm) & (test(launches::) | test(template))) | test(container::tests::) | test(container_restart_validation_tests::)'`
  (263-case run `c44a8fdf-ee98-4426-b729-fa5cfd05863c`).
- `cargo nextest run -p farhelm --test e2e -E 'test(containerized_sessions::) | test(=session_lifecycle::stop_then_delete_both_succeed) | test(=restart_with_resume::restarting_a_live_session_stops_its_tree_and_reuses_the_terminal) | test(=terminal_tabs::stopping_the_agent_leaves_a_tabs_shell_and_its_daemonized_child_running) | test(=create_idempotency::a_retried_create_returns_the_same_session_across_a_supervisor_restart) | test(=launch_sentinel_error_status::stop_before_any_list_on_an_exec_failed_session_still_ends_error) | test(=structured_launches::structured_launches_forward_to_ready_processes_and_survive_a_fresh_generation)'`
  (seven-case run `6bb379ac-c4ab-40b6-b4e7-0f121ea1e5e7`).

These checks ran on working trees, so HEAD alone would misidentify the tested revision. The recorder binds HEAD and
working-source contents; the reused evidence has these abbreviated identities:

| Run                            | Recorded HEAD  | Working-source SHA256 prefix |
| ------------------------------ | -------------- | ---------------------------- |
| 238 units, `0f67951e`          | `11d659a1c5bc` | `c7de1ffddf496b58`           |
| 39 JS cases, `cd8081c5`        | `11d659a1c5bc` | `041df42e6c05bd4c`           |
| 263 author cases, `c44a8fdf`   | `9a3a8ceae731` | `6bcf04868bc075ff`           |
| Seven author cases, `6bb379ac` | `9a3a8ceae731` | `8eac7c7305d1f143`           |

The final stack tip `a9fb201c3c52` has the same complete source tree as pre-split `24b4a78d`; the split changed commit
boundaries only. The integrated backend in `11d659a1c5bc` preserves the author's tested backend paths. Later changes
leave the reused unit logic and JavaScript unchanged; the renderer and style corrections were rebuilt and checked by the
browser runs above. Main remains the original base `9a3a8ceae731`, so no upstream rebase changes this comparison.

Skipped the whole workspace/browser batteries, installer, provisioning and native desktop runtime checks: they add no
specific coverage for the remaining risk after the selected lifecycle, filesystem, renderer and browser checks. WebKit
covers the shared UI's engine family; this is not a claim of native macOS container support. No test opted out of
Podman. No live installation was changed. No preview server or release publication was started; documentation was built
locally.

## Review gate outcome

Each PR received its required single fresh-context `gpt-6.1-sol` review at high effort, with no substitute or swarm. The
backend had no actionable findings. The terminal reviewer identified the absent-ticker fixture assumption; the UI
reviewer identified placeholder text and switch/input layout defects; the docs reviewer requested resource-limit and
image-startup caveats. All were corrected and reconciled, with no open findings. Commit/PR wording passed independent
cold reads. The plan report received its separate required cold read before delivery.
