# Stop agent-started Podman containers through Podman

## What this was about

On Linux, Stop, Restart, closing a terminal tab and Delete could kill a Podman container's monitor while its workload or
Podman record survived. Podman could keep listing it as running or stopping, even when it was started with `--rm`.

Farhelm now gives Podman a chance to stop containers whose main monitors are still in the scopes the action is cleaning
up. A container started with `--rm` is removed. As decided in planning, deleting a session keeps its ordinary containers
in the exited state. If Delete destroys the whole environment, its contents go with it. A container started outside
Farhelm stays running when the agent only entered it through `podman exec`.

## Things you should know

- Podman 4.9 can report a successful stop before its monitor finishes automatic removal. Farhelm confirms removal before
  sending scope signals by asking Podman to remove the stopped `--rm` container without force.
- If Podman cannot confirm cleanup, Farhelm continues its normal process cleanup. The app reports the affected
  containers after Stop, Restart or tab close. If a later cleanup on the same launch reports different containers, the
  updated warning is unread even if the earlier one was cleared. Delete returns the warning with its result unless it
  destroys the whole environment.
- Farhelm uses the selected systemd scopes to attribute containers. Podman can place a monitor in its own scope
  depending on how it was launched. Containers with monitors in separate scopes, Docker, `sudo podman`, Podman machines
  on macOS and launches without systemd scopes are outside this cleanup. Pods use individual stop attempts. A pod's
  infrastructure container can still be left behind if Podman refuses to stop it.
- The container work for one scope cleanup has a shared deadline of about 15 seconds. Delete shares that deadline across
  the session's scopes. A Stop that closes tabs can spend up to about 15 seconds on Podman for each tab it closes. When
  Farhelm starts and cleans up old terminals, it gives Podman a shorter deadline so ordinary process cleanup still has
  time to run within the existing startup budget.

## Open questions and possible follow-ups

None.

## Pull requests

- Stop and finish cleanup through Podman before signalling the scopes.
  [#4281](https://github.com/scode/farhelm/pull/4281/changes)
- Show failed Podman cleanup in the app. [#4283](https://github.com/scode/farhelm/pull/4283/changes)
- Exercise real rootless Podman in private storage. [#4287](https://github.com/scode/farhelm/pull/4287/changes)

## Checks

- The focused Linux suite covered container discovery, the bounded cleanup commands, the startup fallback, the lifecycle
  notices and stored notification behavior after rebasing onto main. It exercised 15 cases, including the real Podman
  4.9.3 test (recorder `5fece198-b095-4618-902a-7fd5c1080509`). A Restart and Delete test skipped when its systemd probe
  did not answer. The narrow rerun exercised that case and passed (recorder `c5f66102-5717-47e2-82a3-33d726b7b5b0`).
- The same real test passed on Podman 5.4.2 (recorder `f4447676-4fdc-4007-bcc6-9beaa5173959`). On both versions the
  auto-remove container disappeared, the ordinary container remained exited, and the external container survived the end
  of the agent's exec session.
- The rebased 4.9.3 run applies to the final heads because the later changes in its tree only reflowed Markdown. Rust
  formatting, changed Markdown formatting, whitespace, changelog format and the test sleep audit passed. The instruction
  check found four existing reference warnings and no errors. The full workspace suite was skipped because the change is
  confined to Linux scope cleanup and notification handling.

## Review gate

Each PR passed its independent GPT-6.2 Sol high review after fixes. The fixes kept Podman inside the startup deadline,
restricted which storage options the supervisor accepts, and kept blocked process reads off the async worker. The notice
review corrected repeated failures and the recorded Restart generation. The test review moved the production probe into
private storage and made the monitor placement, workload prerequisites and guard explicit. The real test exposed the
Podman 4.9 removal race. The stack was rebased over Antigravity's notification changes and the focused coverage ran
again.

### Landing

#### What landed with it

It landed alone in its round. Since its base, main gained persist-environment-seal (the helm remembering environment
deletions), docs-shot-desktop-fixes and emacs-text-field-keys (both in the desktop app); antigravity-support was already
in its base. The independent reviewer checked them against this plan and found no interaction:

- The helm's Podman destruction path and the probe this plan's test reuses are separate.
- The app shows Delete's notice generically, so the new Podman text needs no app change.
- The notification list holds both the Antigravity and the new Podman kind, each handled everywhere.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The reviewer checked that a
container started outside Farhelm can only be stopped if something in the session started it. Containers are found only
through the operation's own scopes, and each is re-checked for owner, start time and cgroup before Podman is asked.

The reviewer also raised a possible hazard in the real Podman test. Its private temporary directory makes Podman run its
first-start refresh, which the reviewer recalled as freeing every Podman lock of the user account, the live install's
containers' included. I checked Podman's source for 4.9.3 and 5.4.2: the refresh frees no locks, and each container
retrieves its own lock again. So the test does not touch other stores' containers, and nothing was changed.

#### Fixes made while landing

- **The failure notice was vague.** It read "Farhelm could not finish Podman cleanup for …. Check the containers on
  their host with podman ps -a using their original storage settings. …", which says neither which host nor what state
  the containers may be in. It now reads "Farhelm could not confirm that Podman stopped …, so they may still be running.
  Check them with podman ps -a on the session's host, adding the storage options the agent used if it chose its own, and
  remove any you no longer need with podman rm -f and the container's ID." The end-to-end tests that find the notice by
  its text were updated with it.

#### Checks

The plan changes the supervisor's scope cleanup, notifications and store, a helm Podman probe, the end-to-end harness
and a real Podman test, so the landing ran the whole Rust workspace suite, the real Podman test and clippy. Those ran on
the maintainer's own test machine, an arm64 Linux VM that builds and tests the tree itself, with Podman 4.9.3, a systemd
user manager, tmux and self-ssh in place. Formatting and the small checks ran locally on x86_64. The desktop app is
untouched, so neither the reference screenshots nor the app's end-to-end smoke ran.

On the plan as delivered, rebased on main:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine
  (`130cc072-f422-4596-a508-dc8af5e9027b`): all 8108 passed, with no runtime skips.
- The real Podman test, `cargo nextest run -p farhelm --test e2e --run-ignored only -E 'test(/^scoped_podman_real::/)'`,
  on the test machine (`c30437d0-eb6b-4afd-ae4f-aa852a34f7b3`): passed, with no runtime skip.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test machine:
  passed.

On the fixed tree, on the test machine: the supervisor's sweep and notification tests and the scoped Podman end-to-end
tests,
`cargo nextest run --workspace --exclude farhelm-desktop -E 'package(farhelm-supervisor) & test(/sweep|notification/) | test(/^scoped_podman::/)'`
(`401b5945-f511-4805-bb2b-30cf3c0ea2a1`): all 64 passed. A first run of the same selection
(`728ec14c-229f-4021-854f-6d16f00d43d5`) failed two end-to-end tests that still looked for the notice's old words; the
fix above updated them. `cargo fmt --all -- --check` passed locally. The rest of the suite, the real Podman test and
clippy were reused, since only the notice's text and the tests' text match changed after them.

Skipped:

- The runtime suites on x86_64 Linux and anything on a Mac: they ran on arm64 only this round. The release gate still
  runs them on x86_64.
- Podman 5: the test machine has 4.9.3. The plan's own run covered 5.4.2.

#### Things you should know that the report does not say

- **Startup cleanup is best-effort for stubborn containers.** When Farhelm starts and ends old terminals, Podman gets
  about 3 seconds per stop, but the stop itself waits 5 seconds before force. A container whose main process ignores
  SIGTERM (a shell, `sleep`) then always fails that stop, and only a warning is logged; ordinary process cleanup still
  runs. This applies only to the old-terminal startup pass.
- **Long Podman waits can outlast the app's 20-second request limit.** Podman's cleanup can take up to about 15 seconds
  for each scope. A Delete or a Stop that closes several tabs can then exceed the helm's 20-second wait for the host's
  reply. The operation still finishes, but the app says the host did not answer in time, and a Delete's Podman notice is
  lost with the reply.
- **A second failure replaces the first warning's names.** Stop cleans the agent's scope and then each tab's, so a Stop
  where several fail ends with a warning naming only the last scope's containers.
- **This relies on Podman leaving its monitor in the session's scope.** Podman does this when it runs under a systemd
  unit, which Farhelm's environment provides today. If Farhelm ever stopped passing that environment to agents, this
  cleanup would quietly find nothing. Nothing documents that dependency yet; the reviewer recalled the Podman detail
  from memory, and I have not checked it against Podman's source.
- **Toolbox and distrobox:** entering a stopped toolbox from a Farhelm tab starts it there, so closing the tab now stops
  that toolbox, as SPEC.md's rule "whoever starts it owns it" says. Users may not expect it.
