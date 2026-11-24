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
