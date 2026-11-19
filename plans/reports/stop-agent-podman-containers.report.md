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
