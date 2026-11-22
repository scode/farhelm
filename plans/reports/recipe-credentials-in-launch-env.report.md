# Claude recipe credentials at launch

## What this was about

Farhelm delivered recipe credentials by editing Claude Code's settings on a Tensorlake or Podman host. A setup,
reconnect or token replacement could overwrite a save Claude Code or the user made to that file at the same time. Every
agent session and terminal Farhelm starts on a recipe host now gets the selected Claude endpoint or subscription
credentials in its environment. Farhelm owns the private file that each new launch reads.

## Things you should know

Restart a running session to pick up a replacement. New terminal tabs get the current value when they open. Claude
started in a shell outside Farhelm uses that shell's own configuration.

An upgraded host clears the three keys that Farhelm previously managed in Claude's settings, even if those same keys
were edited after the recorded delivery. Other keys are preserved. Hosts with no record of earlier delivery keep their
settings file. Cleanup must succeed before Farhelm publishes the new credentials because Claude can prefer a settings
URL over the inherited one. If cleanup fails, a new Claude launch uses the authentication available in its existing
configuration and can prompt for a login if none works. The host's details explain why the new credentials are pending,
and Codex and GitHub delivery can continue. Farhelm retries a failed upload after successful cleanup without reopening
the settings file.

In an isolated loopback test with Claude Code 2.1.330 and synthetic tokens, the settings token took precedence over the
process environment. A running Claude kept using its original token for two prompts after settings cleanup. A new Claude
process with the cleared settings used its environment token. This observation covers that version and fixture.

The helm refuses an older supervisor before it could launch Claude without the recipe credentials after cleanup. This
uses protocol 85. Protocol 84 remains the workspace-trust change.

## Follow-up

Codex keeps its existing settings and subscription delivery. Moving Codex's endpoint credentials has its own Plan
follow-up in TODO.md. The provider ID must stay `farhelm` for saved conversations. In isolated Codex 0.171.0 runs,
omitting the token option sent a stale settings bearer to a tokenless endpoint. Setting the field to empty sent an empty
Authorization header. The follow-up records both results and their recorder runs.

## Pull requests

- [#4558](https://github.com/scode/farhelm/pull/4558/changes) gives each new agent and terminal one fresh credential
  snapshot and keeps it out of the persisted launch and the terminal host's own environment.
- [#4593](https://github.com/scode/farhelm/pull/4593/changes) delivers Claude credentials through that snapshot, retires
  previously managed settings keys, and documents the new behavior.

## Checks

- On rebased revision `4638115b5185`,
  `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm --lib --test e2e -E …` passed
  seven selected cases. The selection was the protocol 85 pin, refusal of 84, the stamped migration regression, Claude
  trust before exec and its home/root exclusions, malformed credential file redaction, and real agent, tab and Restart
  credential delivery. Recorder `199e9679-f5e9-47a5-9fef-9aaf2deff511`.
- Reused `cargo nextest run -p farhelm-helm -p farhelm-proto --lib -E …` from `f783f4ea412d`. All 70 selected delivery,
  endpoint, Tensorlake Add and then-current protocol cases passed.
  `cargo clippy -p farhelm -p farhelm-helm -p farhelm-proto --all-targets -- -D warnings` passed separately. The rebase
  kept the delivery code, and the seven-case run checks the compatibility interactions again. Recorder
  `05138bc6-4dfe-4126-b483-d1ff48478cc1` covers nextest.
- Reused `cargo nextest run -p farhelm --test e2e -E …` from `e0bf92d810d3` for the real local rootless Podman
  lifecycle. It proved the ordinary installer delivered a 0600 file and independently observed the running child's
  endpoint and token digest. The review repair changes stamped upgrades. A fresh host still gets the same file before
  enablement, which the later focused tests also check. Recorder `05910aa4-179b-4295-bc9a-6fc36c74b067`.
- `python3 scripts/check-gui-goldens.py --update` rendered the full desktop set on `e0bf92d810d3`. After publishing four
  expected recipe editor references,
  `python3 scripts/check-gui-goldens.py --scene settings-recipe-editor-middle --scene settings-recipe-editor-end`
  compared them pixel for pixel on `f783f4ea412d`. The rebase added reference entries for other scenes and kept this
  editor's inputs. Recorders `4641b70f-8293-45d8-9b0c-a5771bede41a` and `ccea4c95-f993-4413-a27f-891bbd65630c`.
- The Claude behavior probe used `scripts/agent-tui.py start -- … claude` with isolated settings. Recorder identity is
  unavailable for that probe. The two Codex cases used
  `scripts/agent-tui.py start -- env -i … codex --no-daemon -c model_provider=farhelm -c 'model_providers.farhelm={…}'`.
  Recorder `c235a277-a9fb-497d-a323-ebb0538a8976` covered the omitted field and `59601200-f69b-4179-99a6-a94f4a685ec5`
  covered the empty value.
- Rustfmt, Markdown formatting, changelog format and the test-delay checker passed. The full workspace battery and
  separate live Tensorlake and remote Podman runs were skipped. The local Podman run covers the launch contract and file
  permissions. The focused tests cover Tensorlake setup ordering and stamped upgrades.

## Review

The independent source gate passed both changes. It caught two issues in the delivery patch. Publishing before cleanup
could combine an old settings URL with a new token, and a cleanup error could prevent otherwise deliverable Codex or
GitHub updates. The same reviewer confirmed both fixes, including the focused migration test's assertions. During the
final rebase I checked that the first patch was identical, kept the workspace-trust history at 84, and moved the
credential contract and its test to 85. The seven-case run exercised the rebased flow.
