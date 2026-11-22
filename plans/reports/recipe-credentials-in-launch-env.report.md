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

### Landing

#### What landed with it

The plan's two PRs (#4558, #4593) landed first in this round, with one landing fix (#4600); the desktop startup helpers
landed after it. Since the plan's work was written, main had gained only desktop app work elsewhere, and the rebase had
no conflicts. The supervisor protocol goes from 84 to 85, the only bump in the round.

An independent reviewer read the stack against the plan and its two Decisions (keep Codex's `farhelm` provider name;
move only Claude's credentials and leave Codex as it was). It found no blocker:

- **As decided.** Only Claude moved: Codex's settings writes, provider name and cleanup behave as before. The Codex
  migration is in TODO.md's "Plan follow-up". The one-time cleanup of Claude's settings runs only on hosts where Farhelm
  recorded writing to them, and only once the host's supervisor speaks protocol 85.
- **Credentials.** The supervisor reads the credential file once for each agent start, Restart and terminal start; a
  corrupt file is refused before anything is written; the credentials reach only the new process's environment, and
  error messages name no token. Values that carry a secret can no longer be printed in debug output.

#### Fixes made while landing

All in [#4600](https://github.com/scode/farhelm/pull/4600/changes), docs only:

- **Settings already in an image win.** Claude gives its own settings precedence over the environment, and the plan's
  change leaves alone a settings file Farhelm never wrote. So a base image whose Claude settings already set a base URL
  wins over the recipe, and Claude then sends the recipe's token to the image's URL. The Tensorlake sandboxes docs page
  and the changelog fragment now say so and suggest setting the recipe's Claude credentials to **None** for such an
  image. The plan required leaving such files alone; warning about it at delivery would be a new mechanism, left as a
  possible follow-up.
- **Who gets the credentials.** The changelog draft and the Tensorlake docs page now say that every session and terminal
  Farhelm starts on the host gets the credentials, and the page no longer says Farhelm writes settings for both Claude
  Code and Codex.

#### Checks

This round landed two plans stacked in this order on main: recipe credentials in the launch environment (protocol 85)
and the desktop startup helpers. The checks ran on the combined result; the worker runs were on a separate arm64 Linux
test worker, the rest locally on x86_64.

On both plans as delivered, stacked onto main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four nextest slots, no retries and four build jobs, on
  the test worker (recorder run `0533a2d1-482b-4855-8997-6048700f9783`): all 8318 passed, the real Podman environment
  test among them.
- The four clippy runs (`--all-targets`, `-p farhelm --bins`, and the desktop app with and without `--features agent`),
  on the test worker, not through the recorder: passed.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `e684d4f8-2f9e-4959-a29b-b54a9d260b29`): all
  185 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `f73f7f73-6517-4404-ac56-ac4d02ee04f8`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 -B scripts/gui-lifecycle.py`, the Linux app lifecycle test (startup, quitting, the saved window frame), on
  the test worker (recorder run `198b3e2a-a687-4689-8fd2-9d0bba408994`): every step passed.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `993f1622-0462-4776-95eb-e35602d8230d`):
  passed.
- The docs website build, `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its isolated
  environment), `dprint check` and `python3 releasing/check-changelog.py format`, locally: passed.

After the recipe credentials' landing fix (#4600), which changes only Markdown: `dprint check`, the website build and
`python3 releasing/check-changelog.py format`, locally: passed. The runs above still apply.

Skipped: a launch on a real Tensorlake sandbox with Claude recipe credentials, which spends cloud resources (the real
Podman environment test in the workspace run delivers the credentials to a container); macOS, since no Mac was part of
this landing; CI, which runs only on demand.

#### Things you should know that the report does not say

- Claude settings baked into a base image can send the recipe's token to the image's base URL; only the docs warn about
  it.
- The deferred Codex migration will need its own protocol bump: a protocol-85 supervisor refuses a credential file with
  fields it does not know, so new Codex fields would break every launch on it otherwise.
- The recipe editor's Claude help says credentials go to sessions "on this host", though it is shown in Settings →
  Recipes, where a recipe serves many hosts; changing it means re-rendering four reference pictures.
- SPEC_impl.md now limits a protocol bump to what the helm relies on from the supervisor, but the code comment beside
  the protocol version still says "a communicating peer"; the two should be made to agree.
- An end-to-end test named only for the managed GitHub token now also checks recipe credentials, so its name undersells
  what it covers.
