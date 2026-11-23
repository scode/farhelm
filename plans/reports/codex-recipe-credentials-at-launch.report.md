## What this was about

When Farhelm delivered a Codex recipe endpoint to a sandbox or container, it rewrote Codex's settings file. Delivery
could overwrite an edit saved at the same time. The maintainer chose to supply the endpoint when Farhelm starts or
resumes Codex and to clear the old provider selection and bearer once on hosts where Farhelm recorded an endpoint write.
The provider still uses the same ID so Restart and opening an existing conversation by ID keep working.

## Things you should know

Restart a running Codex session to pick up a replacement endpoint token. Typing `codex` in a terminal tab uses that
shell's usual Codex configuration. A token preconfigured on the `farhelm` provider in a base image can still go to the
recipe's URL even if the recipe's token picker says **None**. Choose **None** as the Codex mode to use the image's
configuration. Codex Subscription still uses `auth.json`.

If Farhelm can't clean up an older settings write, it warns on the host and withholds the new Codex endpoint until a
retry succeeds. Claude's recipe credentials can still reach that host. A host recorded as having no managed Codex
endpoint may have settings the user wrote later. The migration leaves those settings alone.

## Open questions and possible follow-ups

None.

## PRs

- [#4679](https://github.com/scode/farhelm/pull/4679/changes) — Supply Codex recipe endpoints at launch and refresh
  Settings help and screenshots.
- [#4680](https://github.com/scode/farhelm/pull/4680/changes) — Update the specs and the Tensorlake sandbox guide, and
  remove both TODO entries.

## Checks

- 63 targeted Rust tests passed on `54d04289`, covering Codex provider arguments and the private credentials file,
  scoped delivery failures, the protocol bump and recipe editing. Three terminal-host tests passed, including named
  Codex Start and Restart with a rotated token and rejection of a supervisor one protocol version behind. Recorded runs
  `8ae1665c-12f8-4323-8bc4-7389a7f17e40` and `846a6d1d-3d29-430b-a878-38f6b4f030d4`.
- A real local Podman lifecycle test passed with both agents' credentials in the same private file and no Codex settings
  file created. It ran the same-source Linux payload (run `29a30a1e-4abc-45e4-a69d-a2ae149639a9`). Strict all-targets
  Clippy, Rustfmt and the test-delay check passed. The website built and all internal links validated (run
  `536e5718-6696-4b7d-9215-e1fca6910471`).
- A full Linux reference capture changed four expected pictures. Both themes were reviewed, the references were
  published, and the recipe editor scenes matched them (runs `f09e14fa-e80f-4014-9861-937a7164bc0f` and
  `9db05451-b571-431c-95cb-4f4140532279`). Markdown formatting and changelog format passed.
- Reused the targeted Rust and Podman results after rebasing. Main added sandbox ID validation to Tensorlake
  Suspend/Resume and added feedback for Stop and suspend. The credential delivery and launch paths, the Podman test
  actions, and the recipe editor scenes used by the checks were unchanged.
- No live Tensorlake run was needed to exercise delivery. Both providers use the same delivery code, and the real local
  Podman test exercised it with a matching supervisor. Stand-in agents let the launch tests observe exact arguments and
  environment variables without sending vendor requests. The selected suites covered the changed paths, so the full
  workspace suite was skipped. CI runs on demand and was skipped for the draft stack.

## Review gate

Both independent reviews cleared after fixes, with no outstanding findings. The source review found that a historical
no-endpoint record could authorize a settings edit and that the Restart test could mistake replay for fresh readiness.
The fixes preserve user settings after that record and require a distinct readiness signal from the replacement process.
The independent documentation review checked the one-time cleanup, connection timing, tab behavior and image-baked token
caveat. Both PR messages passed separate cold reads.

### Landing

#### What landed with it

The plan's two PRs (#4679, #4680) and one landing fix (#4693) landed first in a round with
explain-stale-launcher-choices, approval-card-program-wording and wayland-clipboard-file-paste, none of which touches
how agents get their credentials. Since the plan's base, main had gained the retirement of the "Agent's own setting"
permission (with its upgrade of saved sessions), Settings keeping its list of leftover Tensorlake sandboxes when a
refresh fails, and changes to the opt-in tests against real agent programs. The rebase met only TODO.md, where this plan
resolves the "Revisit deferring the Codex credential migration" decision: that entry goes, and the decision entry main
added beside it stays. The plan moves the wire protocol from version 86 to 87; nothing else in the round or on main
changed it.

The landing's independent reviewer checked where a recipe's token can go: a tokenless endpoint sends no Authorization
header, the recipe's token takes precedence over a bearer left in Codex's settings, Restart and resuming a conversation
by ID read the same private file, and the token never appears in arguments, the saved launch, logs or the terminal
host's environment. The one-time cleanup removes only what Farhelm wrote. The protocol bump is needed (an older
supervisor would refuse every launch carrying the new field) and complete. On a host whose supervisor predates it, Codex
sessions get no recipe endpoint until the host is updated.

#### Fixes made while landing

- The landing's full test run failed two helm tests of Tensorlake host setup that the plan's own runs had not included.
  A fresh host no longer reads the agent settings during setup, since there is no earlier Farhelm write to clean up, and
  both tests still expected that read: one injected a failure there to check that a credential problem stays a warning,
  the other required it for a recipe whose Codex mode is Subscription or None. #4693 injects the failures where setup
  still touches the host (writing, or for None removing, the private file holding the credentials Farhelm hands agents
  at launch), and expects no Codex cleanup to be recorded. The product code was right.
- #4693 adds two caveats to the release-notes draft that the docs page and SPEC.md already state: typing `codex` by hand
  in a terminal tab no longer reaches the recipe endpoint once Farhelm has removed its old settings entry, and a token
  already configured on the `farhelm` provider in the host's base image can still go to the recipe's URL when the recipe
  picks no token.
- #4693 corrects a comment that gave the wrong reason for leaving a host's Codex settings alone when Farhelm's record
  says it wrote nothing there; behavior is unchanged.

#### Checks

This round landed codex-recipe-credentials-at-launch (how Codex gets a recipe endpoint, in the helm and the supervisor,
with the protocol version bumped), explain-stale-launcher-choices (the desktop launcher), approval-card-program-wording
(the helm's approvals and the desktop's approval cards) and wayland-clipboard-file-paste (the vendored GPUI Linux
crate's clipboard and the terminal's paste) together. Between them they reach every crate except the desktop app's outer
program, so the landing ran the whole workspace suite, the lints (the desktop app's too), the end-to-end smoke test,
every reference picture and the docs site build on the four plans stacked on main, then narrower runs after the landing
fixes. The worker runs were on a separate arm64 Linux test worker with a systemd user manager, rootless Podman and SSH
to itself; the rest ran locally on x86_64 Linux. The first worker attempt (recorder run
`ef6ed206-f59e-44b9-9a19-e73ac2138ffe`) ran out of disk before it compiled anything; the disk was full of this landing's
own earlier build output, which was cleared, and the worker's runners now clean up after themselves (#4701). None of the
runs used a real agent program, a real Tensorlake account or a Wayland file manager: the tests use stand-ins, as the
reports say.

On the four plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `2dd259d9-fdaa-4006-b3ad-bf8181a7df43`): 8435 of 8437 passed. The two failures are Tensorlake setup tests that
  codex-recipe-credentials-at-launch broke, which its landing fix (#4693) corrects; the failed run is kept. One test, a
  check that stopping a session's containers reaches every selected scope, printed a runtime `SKIPPED` because it found
  no usable systemd user manager that time; run again on its own (`cargo nextest run -p farhelm --test e2e` selecting
  only `lifecycle_stops_containers_in_the_selected_scopes_before_signalling`, run
  `1d248544-da0e-41c5-a68b-d4113c9881c1`) it ran fully and passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's two
  `cargo clippy -p farhelm-desktop --all-targets` runs (with and without `--features agent`), on the test worker:
  passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `3b7e19df-96bb-4ec7-854c-83031f92e389`): of
  its 50 steps, 49 passed and one was skipped as always on Linux (paste, which headless Mutter cannot give the app).
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `f5a29c37-701b-4967-b9cd-7517d44f437b`): every picture
  but two matched its reference; the two are the terminal's covered-link scene, light and dark, whose approval card
  approval-card-program-wording rewords, republished by its landing fix (#4694). A full `--update` (recorder run
  `9fe8cb65-be4a-456f-af51-d4310849b6d8`) found those two the only changes.
- `python3 -B scripts/test-gui-screenshot-batch.py` (the screenshot scene list's own checks, locally and not through the
  recorder, so it has no run id), `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`,
  `dprint check`, `python3 releasing/check-changelog.py format` and the docs site build (`bun run build` in `website/`),
  locally: passed.

After the landing fixes (#4693 and #4694):

- `cargo nextest run -p farhelm-helm -E 'test(/tensorlake_add::tests::|credential_delivery/)'`, on the test worker
  (recorder run `1974b03e-abe5-4a18-b7c7-9716db0f0872`): all 65 passed, the two corrected tests among them.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker,
  with #4693 on main and the other three plans on top: passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

Reused: the workspace run and the smoke test above for everything else, since #4693 changes only those two tests, a
comment and a release-notes draft, and #4694 only two reference pictures' manifest entries. Skipped: the tests of the
desktop app's outer program (the part that starts the window and the embedded helm), which nothing here reaches; macOS,
where nothing compiles differently and the Wayland change does not apply; real agent programs, a real Tensorlake account
and a real Wayland file manager; CI, which runs only on demand.

#### Things you should know that the report does not say

- Before this plan, a credential lookup that failed left the host's private credentials file alone. Now, if the lookup
  for one agent fails while delivering credentials (any credential push can trigger one, a GitHub token replacement
  included), the file is rewritten without that agent's still-valid entry, or deleted if nothing else was in it, and
  nothing retries until the host's next connection. A lookup only fails transiently in practice, since a credential in
  use cannot be deleted, so the cost is a session that misses its credentials until then, not a leak. When both agents'
  lookups fail, the warning repeats "could not resolve the recipe's endpoint credentials." without naming either agent.
  A follow-up could keep the file as it was when a lookup fails and name the agent.
