# Sandbox workspace trust and Claude Code first-run setup

## What this was about

A recipe host could have valid Claude Code credentials and still stop at the theme chooser. A new sandbox or container
session could also stop at the agent's workspace trust question unless that agent's Settings switch was on. The plan
chose to fill in Claude's missing first-run settings and apply workspace trust automatically on supported launches into
a session's own sandbox or container.

## Things you should know

Farhelm fills in Claude's first-run setting when preparing a recipe host. It also seeds trust for the home folder when
preparing a host owned by one session. Existing choices and credentials take precedence. The isolated Claude Code
2.1.332 probe needed only `hasCompletedOnboarding` to pass the theme chooser. Once home was trusted, Claude started both
there and in a checkout below it. An alternate Claude config can still prompt, as can an explicit choice in the default
file. If Farhelm cannot safely update the default file, for example if it is a symbolic link, it reports that first-run
setup is pending.

Claude Code, Codex, Muse Code, Pi and Antigravity receive workspace trust automatically when Farhelm composes a launch
into a sandbox or container owned by that session. Installed hosts and persistent recipe hosts use the Settings switch.
Antigravity's explicit `--settings` takes precedence. Restart and Replace using the original launch keep its saved
choices. Claude's separate setting for applying trust after a managed clone also remains in force.

When starting Claude, Farhelm protects the host's home folder and its parents from new trust entries. If recipe setup
already trusted the exact home folder, Farhelm can acknowledge that existing trust without writing it again or showing a
trust warning for that launch.

## Open questions and possible follow-ups

The installed Claude Code probe used a loopback endpoint because there was no usable subscription login. The first
launch with a live Claude subscription remains unverified.

## Pull requests

- Fill in Claude Code's first-run settings and seed home trust for disposable sessions.
  [#4943](https://github.com/scode/farhelm/pull/4943/changes)
- Apply automatic workspace trust to launches in session-owned sandboxes and containers.
  [#4955](https://github.com/scode/farhelm/pull/4955/changes)

## Checks

- Isolated Claude Code 2.1.332 startup probe `0d64422a-4aae-4892-9665-0a347f08c70f` used nine private homes and a
  loopback endpoint. The empty and theme-only cases opened the theme chooser. Onboarding alone reached the trust
  question. With home trust seeded and no theme key, Claude reached its prompt in both home and a child directory. The
  probe exercised startup screens without submitting a prompt.
- Reused pinned nextest checks from step 01 at `94f2473a9884`. The 44-test `farhelm-helm --lib` run selected
  `test(credential_delivery) | test(/tensorlake_add_tests::/) | test(/agent_endpoints::/) | test(conditional_settings_install)`
  (`5d7845cb-8ea6-43c8-9fc7-a18752facac7`). The 35-test `farhelm-helm --lib` run selected
  `test(/tensorlake_add::tests::/)` (`e06d5d8a-db6b-44b3-8665-882a44a432b0`). The 20-test `farhelm-gui --lib` run
  selected `test(recipes)` (`6b6da0a7-13ee-4cfb-a3c2-19dd93c5150f`). The draft retained the tested patch through both
  rebases. The final launch gate covered the integrated paths after the changes to main.
- `python3 scripts/check-gui-goldens.py --update` passed for both steps (`d9ba19eb-a21a-4d69-a22f-26f727a516d6` and
  `02a3c682-d39b-4138-88af-770eb7b423eb`). Reviewed and published the light and dark recipe summary in step 01, then
  both appearances of the five agent Settings pages in step 02. The full catalog captures showed only those intended
  differences.
- On the rebased stack, pinned `cargo nextest run -p farhelm-helm -p farhelm-supervisor --lib` passed all 394 selected
  tests (`45fc7f2c-3fb1-4ba7-92e9-a24f044309e8`). The selection covered the launch policy, provider routes, credential
  delivery, and the exact-home Claude trust guard. Pinned
  `cargo nextest run -p farhelm-gui --lib -E "test(/settings::/)"` passed all 261 selected tests
  (`5c9610a5-b1e2-4a4d-9157-4661f8247af0`). Those two runs covered the final source tree before publication updated the
  ten screenshot references. Both `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings` passed.
- The docs site built and checked links. Rust formatting, the Rust test-delay check, changed Markdown formatting,
  changelog format and whitespace checks passed. The full runtime battery and live provider launches were skipped
  because the changed launch paths are covered by focused local tests and scripted provider fixtures.

## Review

The independent source review gate passed with no remaining findings. The reviews confirmed the minimal Claude settings
write, how it preserves existing choices and the destination-based trust rule. The first review found a missing fixture
for routing settings differently on persistent and disposable hosts and Settings copy whose scope was too broad. The
second review verified the expanded fixtures, revised wording and the rebase over managed checkout history.
