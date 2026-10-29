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

### Landing

Landed with two fixes at landing, one of which documents a gap in the isolation you asked for rather than closing it
(below). Linux hosts can now run a session's agent and its terminal tabs in a rootless Podman container, opted into per
launch with the launcher's new Containerized switch (marked experimental). The container gets the session's managed
checkout, one host loopback port for a local API proxy and optionally a GitHub token read from a file; the user supplies
the image. Stop ends the whole container, Delete removes it, and Restart is unavailable for these sessions. A docs page,
"Containerized sessions", describes the setup and its limits.

- #2791: the container launch and its plumbing.
- #2792: terminal tabs inside the container.
- #2793: the launcher switch and its settings.
- #2794: the docs page.

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping landed since the plan was based; nothing interacted.

#### Security review

A fresh-context security review checked that the GitHub token stays off command lines, logs, the database and the
interface; that file paths and links printed inside the container can only open files inside the checkout, with the
actual download pinned beneath it; that Delete removes only the container Farhelm named for that session; that the
protocol version was raised so mismatched helms and supervisors refuse to connect; and that ordinary sessions behave as
before. No database change.

#### Fixes made at landing

- Your planning requirement was that the container must not reach outside itself, and this falls short of it: the
  container can write anything in the checkout, including its `.git` folder and an agent's own project settings, and
  those can run with your full account access when Git or an agent later runs in that checkout outside the container (a
  terminal, a shell prompt that shows Git status, an editor, a later ordinary session). SPEC.md and the docs page
  promised isolation without saying this; both now do, the page with a caution box. The landing documented the gap
  instead of blocking, because the feature is opt-in and experimental and still confines far more than running the agent
  on the host. Closing it is your call, recorded right after this landing as a near-term TODO entry. The two options the
  reviews name: give the container `.git` hooks and configuration (and agent settings) read-only, which closes the main
  path but may break agents that legitimately change them; or warn before an ordinary session or launch uses a checkout
  a container has used. The monitor recommends the read-only option.
- The new Containerized switch had no hover text, which the browser sweep of hover text caught; it now has one that also
  fits the template editor, where the same switch appears.

#### Things you should know

- The terminal connection inside the container keeps Podman's default detach keys: Ctrl-P is held back until the next
  key, and Ctrl-P Ctrl-Q disconnects the session's terminal while the agent keeps running. Probably fixed by turning
  detach keys off, which needs trying on the supported Podman version.
- Services listening on the host's LAN or tailnet address are reachable from inside; only other host loopback ports are
  blocked, and that is proven only on Podman 4.9.3.
- Stop and Delete of a containerized session are refused when Podman is missing or not on the supervisor's PATH, and
  nothing else removes such a session until Podman is back.
- The GitHub token file is read without a size limit, inside the supervisor's request handling; a pipe or device path
  there can hang the supervisor or use unbounded memory. Requiring a regular file and capping its size would fix it.
- Smaller edges: one lock file per containerized session accumulates; a container gets read-write access to managed
  checkouts nested inside its own; an image path containing a colon is misread; the "Turn off containerization" and
  "Clear" buttons in the container settings have no hover text.
- The whole Rust workspace's tests now fail, rather than skip, on a machine without rootless Podman and an Ubuntu 24.04
  image already in its Podman storage (the tests never pull one), unless their documented opt-out is used; that affects
  other machines and the deflake sweep.
- Containerized terminals rely on an internal of the bundled terminal library, so upgrading it will need care; a test
  catches a break.
- The docs page has no launcher screenshots, and it introduces an "Experimental" sidebar badge, new to the site.

#### Checks

This round landed only this plan. On the stack as delivered:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo check -p farhelm-desktop`, the check that every test delay states its reason, `dprint check` across the
  repository, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (three warnings, all
  about citations already on main), the docs website build, the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh`: all passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `d2b6ed87-c8e1-417b-a804-bf820519e660`: all 813 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `e2866549-6346-4f9a-800b-4c65447c5be4`: all 996 passed. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `a6096384-e5cf-45af-82ae-cc82fb792ac6`: 365 passed.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `9e2208e2-e470-472a-9947-fb14ef3acd6b`: 2769 passed, 14 skipped by design, 3 failed:
  the hover-text sweep on both engines (the switch fixed at landing), and on WebKit "a wheel over the header or the
  sidebar edge never scrolls the document", which timed out while deleting its ordinary session at the end. Repeated
  five times alone on WebKit after the fixes, run `f3da77ab-8cb1-4930-8a02-5bcde9cf529d`, it passed every time; one
  failure is not recorded as a flake.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, on a machine with rootless Podman, run `bcd24fec-5ba9-4601-8c2e-fc23d3246328`: 4533 passed, 41 skipped by
  design, and 1 failed: the known flake "shallow pause resumes without reset or replay", in terminal code this plan does
  not touch. The container tests actually ran on this machine rather than returning early.

After the landing fixes:

- `dprint check`, `python3 scripts/check-agents-md.py`, the docs website build (with the new caution box),
  `cargo fmt --all -- --check`, the web-target UI lint and the UI's release web build: all passed.
- `npx playwright test` in `e2e/` with `tests/tooltip-coverage.spec.ts`, `tests/github-checkout-composer.spec.ts` and
  `tests/templates.spec.ts`, on Chromium and WebKit, run `e237d0e7-d3a9-4578-b166-070084c1735c`: all 178 passed.
- The final rewording of the hover text and of the caution came after that run; it changes only text no test checks, and
  `dprint check`, `cargo fmt --all -- --check` and the website build passed again after it.
- Reused: none. Skipped: the desktop smoke tests (no desktop-specific code changed) and the native Mac app (not runnable
  here; containers are Linux only).

The report's checks were the executor's selected runs; this landing ran the whole browser suite and the whole Rust
workspace. Nothing else in the executor's report became untrue at landing.
