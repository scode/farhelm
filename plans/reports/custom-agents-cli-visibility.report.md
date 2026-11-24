# Discover registered agents from the Farhelm CLI

## What this was about

An agent inside a Farhelm session had no way to look up the registered custom IDs it could pass to `--agent`. A typo
gave incomplete guidance when starting a session and could be mistaken for a deleted registration when editing a
template. A mistyped built-in reasoning effort also lost the choices a caller could retry with. Finally, an explicit
`--recipe` refused `--conversation`, but reaching the same recipe through a template could get as far as launch
approval.

The maintainer chose to add `farhelm agent agents [--json]`, show both sets of IDs in unknown-agent refusals, restore
built-in effort choices, and apply the same conversation restriction to both ways of selecting a recipe.

## Things you should know

- The listing shows the ID to pass as `--agent`, the name, and whether that type accepts an initial prompt, a
  conversation selector, a model, or an effort value. It reads the registered choices and does no installation check on
  the target host. Sandbox and container sessions can use it too. Both outputs omit the configured command.
- The conversation column describes the ability to start with `--conversation`. A running session reports its own
  readiness for Restart. A launch creating a new sandbox or container refuses `--conversation` before approval. If later
  template or flag choices select an existing host instead, the ordinary conversation rules apply.
- Built-in effort errors use the choices for the resolved model when the helm has them. A template write without a
  resolved model lists the built-in vocabulary. A custom agent still uses its wrapper's own effort spelling.
- Discovery JSON now uses schema 8 for agents, hosts, sessions, and templates. Scripts that check the schema version
  need to accept 8. The existing reply fields keep their shape. The new agent-listing tags advance the helm-supervisor
  protocol to 90, so earlier supervisors need the usual update to connect.

## Open questions and possible follow-ups

An exceptionally large custom catalog can exceed the relay's existing single-reply limit. Discovery then reports that
the reply is too large. I recommend keeping this explicit failure for oversized replies. It preserves existing
registrations and lets a successful listing carry every exact choice.

## PRs

- List registered IDs in launch and template refusals, restore effort guidance, and clarify recipe placement.
  [#4752](https://github.com/scode/farhelm/pull/4752/changes)
- Add `farhelm agent agents` and the custom-agent guide entry.
  [#4755](https://github.com/scode/farhelm/pull/4755/changes)

## Checks run, reused and skipped

- On PR 1 tree `25bcab88`, pinned `cargo nextest run -p farhelm-proto -p farhelm-helm --lib -E <selection>` passed 193
  tests selected from `launcher`, `agent_launch`, `agent_requests` and `custom_agents` (run
  `db6a9aca-42e5-4340-8256-02ecdbf13803`). After `cargo build --locked -p farhelm-fixtures`,
  `cargo nextest run -p farhelm --bin farhelm --test e2e -E <selection>` passed nine tests from
  `agent_listing_real_stack` and the `recipe_flags_send_the_environment_verb_and_refuse_placement_mixes` parser case
  (run `9fb6a49c-3910-4dbb-9bca-736497bd8e8a`).
- On PR 2 tree `3249f6e2`, pinned
  `cargo nextest run -p farhelm-proto -p farhelm-supervisor -p farhelm-helm --lib -E <selection>` passed 550 tests. The
  selection covered the proto library and the `agent_kind`, `agent_requests`, `custom_agents`, and `agent_verb` cases
  (run `aa720109-8572-4b6e-adde-b1efb90339a6`). After the matching fixture build,
  `cargo nextest run -p farhelm --bin farhelm --test agent_cli --test e2e -E <selection>` passed 80 tests covering the
  `agent_cli` binary, `agent_listing_real_stack`, `agent_instructions`, `agent_client`, and `render` (run
  `a54a5808-d6dc-4ec8-879a-fe4fbe5beaeb`). Both stages used the recorder and reported no runtime skips.
- Reused the PR 2 broad checks and `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings` from `3249f6e2`. The changes at `0a856b0e` were a stronger
  confined-origin test and documentation. On that tree, the recorded
  `cargo nextest run -p farhelm-helm --lib -E 'test(=agent_requests::tests::confined_agents_reach_only_their_own_environment)'`
  passed for both provider fixtures (run `c73f5552-ea58-43c7-a502-3906c51c6241`).
- `cargo fmt --all -- --check`, `dprint check` on both specs, TODO, the changelog fragment and the **Custom agents**
  guide, `python3 releasing/check-changelog.py format`, and `git diff --check` passed. The isolated
  `python -B scripts/check-test-sleeps.py` on `3249f6e2` inspected 310 delays with no missing annotations. The added
  test setup awaits registration and includes no delays. The final `bun run build` in `website` succeeded with 41 pages
  and validated internal links, including the changed guide.
- Main advanced during validation with the sidebar update reminder and recipe credential delivery fixes. The discovery
  request reads only the helm's agent catalog. The recipe conversation refusal occurs before approval and provisioning.
  The stack kept the tested base.
- Skipped the full workspace and desktop runtime suites. The selected protocol, relay and real-stack CLI tests exercise
  the changed behavior. The desktop product names come from the same values moved to the shared agent-type method.

## Review gate

Both drafts passed source review and a separate commit-message read. The first review found that uppercase effort typos
were still intercepted before the model was known. The final checks verify that Codex reports its known choices and a
custom wrapper still rejects the same value. The second review strengthened discovery coverage for both sandbox and
container sessions and clarified the command-omission wording.

### Landing

#### What landed with it

This was the only plan landed this time: its two PRs ([#4752](https://github.com/scode/farhelm/pull/4752/changes),
[#4755](https://github.com/scode/farhelm/pull/4755/changes)) plus one landing fix,
[#4758](https://github.com/scode/farhelm/pull/4758/changes). While the plan was being built, main gained the desktop
sidebar's update-pill colors (light-mode-upgrade-pill-pulse) and the helm's handling of unreadable recipe credentials
(preserve-creds-on-lookup-failure), plus plan bookkeeping. Neither touches the agent catalog, the `farhelm agent`
commands or the wire protocol, so nothing interacted with this plan. The plan moves the helm-supervisor protocol from 89
to 90, and main was still at 89, so no renumbering was needed. The rebase met one conflict, in TODO.md, where this plan
and the two plans that reached main meanwhile each removed their own entry; all three removals were kept.

#### Fixes made while landing

- **A value that is not even an agent ID got no pointer to the new listing.** The CLI itself refuses an `--agent` value
  that does not fit the custom ID format, such as a display name typed in place of an ID (`--agent "Acme Code"`, which
  the listing's new name column makes more likely), before the helm sees it. That refusal listed `custom` as if it were
  a built-in type and said nothing about how to find registered IDs. #4758 lists only the real built-in types and names
  `farhelm agent agents`.
- **The changelog fragment did not mention the JSON schema change.** The report says scripts that check the `--json`
  schema version need to accept 8, but the fragment did not; the earlier bumps to 6 and 7 were announced in the
  changelog. It now says so.
- Doc comments that had gone stale or were missing were fixed: the agent type's two display names (a full product name
  for listings, a short name for session wording) now say why they differ, a supervisor comment now names where an
  agent's full product name is defined, and the listing entry's fields are documented.

#### Checks

The plan moves the helm-supervisor protocol to 90 and changes the agent catalog's refusals and discovery across the
protocol crate, the helm, the supervisor, the CLI and the desktop app's agent names, plus a docs page. So the landing
ran the whole Rust workspace suite, clippy with warnings as errors over the workspace and over the `farhelm` CLI binary,
every desktop screenshot against its reference picture, the docs website build, and the formatting checks (`cargo fmt`,
`dprint`), the test-sleep check and the changelog format check. Worker runs were on a separate arm64 Linux test worker
(Ubuntu 24.04, with a working systemd user manager, Podman, tmux and self-ssh); the rest ran locally on x86_64 Linux.
One test failed once, as described below; no other check failed.

On the plan's PRs rebased onto main, before the landing fix:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `d9292233-66c8-4f92-b418-2499e43d9894`): 8539 passed, one failed and 86 were skipped as ignored, and no test silently
  skipped for a missing systemd, ssh or Podman setup. The failing test,
  `codex_identity::nested_native_codex_reports_cannot_replace_the_foreground_conversation`, failed because the worker's
  systemd user manager refused a Restart; the run is kept. That test then passed 3 of 3 alone on the worker
  (`b09d3291-d48d-440e-898e-4a5e3d5c7f0c`).
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `b916b858-09d6-46fd-8f40-5ff0c86da4aa`): every
  screenshot matched its reference.
- `cd website && bun install --frozen-lockfile && bun run build`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4758):

- `cargo nextest run -p farhelm -p farhelm-proto --bin farhelm --test agent_cli --lib`, on the test worker (recorder run
  `8deebdbf-62f6-4d13-bde9-4c2664e0cb79`): all 488 passed, including the test of the CLI's refusal, none of them
  silently skipped.
- Both clippy runs on the test worker, and the formatter, `dprint check` and the changelog format check locally: passed.
  The test-sleep check was not rerun: the fix only tightens one unit test's assertions and adds no delay.

Reused for the fix: the rest of the workspace suite, the screenshots and the website build above, since the fix changes
only the CLI's own refusal text, doc comments and the changelog fragment. Skipped: the desktop app's own tests, since
the app's crate did not change (the agent names it shows moved inside the shared GUI library, which the workspace run
and the screenshots covered); the end-to-end desktop smoke, whose steps do not use the agent catalog's refusals or
listing; macOS, since nothing platform-specific changed and no Mac was used; and CI, which runs only on demand.

#### Things you should know that the report does not say

- The worker's full workspace run had one failure that is not this plan's: a test of restarting a Codex session failed
  because the worker's systemd user manager refused the Restart at that moment. The plan changes nothing about Restart
  or process cleanup, and the test passed three times alone right after. A single failure of the machine's own service
  manager is treated as an environment event rather than a flake, so it is not recorded in FLAKES.md.
- When the helm has no model catalog for an agent (for example right after connecting), a mistyped effort reads "is not
  accepted by codex for this model", which sounds as if Codex takes no effort at all. Small and cosmetic; a follow-up
  could list the general effort vocabulary there instead, as template writes already do.
- The end-to-end listing test pins all twelve built-in agent IDs, so adding a harness will need that test updated too.
  That is intended (it proves the listing shows every built-in type), so no action is needed.

#### Possible follow-ups

- The cosmetic effort wording above, if it bothers you. I suggest folding it into the next change that touches effort
  validation rather than a plan of its own.
