## What this was about

At startup the Farhelm supervisor now keeps cleaning other repositories' cached checkouts when one repository fails. It
reports the failure after finishing the remaining work, as agreed in the plan. Hosts now refuses setup reruns for a
Tensorlake sandbox it knows is suspended or being deleted. An earlier setup failure also stays visible if the user
confirms from a dialog they opened before that state change. The third PR tests the existing rule that legacy Codex
settings cleanup waits for a compatible supervisor connection.

## Things you should know

- Cleanup stops for a repository when Farhelm cannot verify which checkout to protect or cannot make a required
  filesystem change durable. If it cannot delete one leftover checkout, it still removes other eligible leftovers. When
  an earlier leftover blocks removal of a remaining cache, Farhelm defers removal of that cache.
- Hosts explains the refusal when Farhelm knows a sandbox is suspended, changing state, or being deleted. After a failed
  deletion, **Retry** retries the deletion. Refusal from an already open dialog preserves the earlier setup failure and
  the host's current connection state.
- The test covers the Codex credential choices **Subscription** and **None**. It expects setup to read Claude Code's
  onboarding file and detects unintended legacy agent settings reads on a newly added host. It checks separately that
  the record of Farhelm-managed Codex settings is still unset before the supervisor connects.

## Open questions and possible follow-ups

First setup can fail before Farhelm records the information it uses to refresh sandbox status. If it has yet to see the
suspension, Hosts can still offer **Re-run**. The rerun can then discover the suspension when it checks the sandbox
before installation.

The settings test uses a new host with no record of previously managed Codex settings. A separate existing-host test
with that record would exercise cleanup of an older configuration after a compatible supervisor connects.

## PRs

- Continue sweeping checkout caches after a failure. [#4964](https://github.com/scode/farhelm/pull/4964/changes)
- Refuse Tensorlake setup reruns before replacing earlier progress.
  [#4972](https://github.com/scode/farhelm/pull/4972/changes)
- Check for legacy settings reads during initial Tensorlake setup.
  [#4974](https://github.com/scode/farhelm/pull/4974/changes)

## Checks run, reused and skipped

- All 32 repository-cache and spare-checkout tests passed on `f01605b6`, run `784f2cdc-2c25-4861-8b17-8025f9ebb73f`, and
  again on the reviewed `f84c8b2d`, run `572a5f66-f29e-4fcb-83a8-2785a96da881`. The recorder ran
  `cargo nextest run --locked -p farhelm-supervisor --lib -E 'test(/^repo_cache::/) | test(/^spare_checkouts::/)'`. The
  revised run covers a blocked trash slot with and without a surviving legacy cache.
- All 364 selected Helm Tensorlake Add and GUI Hosts tests passed on `25adb818`, run
  `41e7631b-b272-4616-bb98-7ab18cd7cb25`. The selection covered the probe, the confirmation and the refusal seen in
  Hosts. This was the combined command below with the Codex settings case excluded by
  `- test(codex_subscription_and_none_enter_setup_without_an_endpoint)`.
- The first combined run on `933d4278` had two fixture failures, recorded under `5064c8a6-8c76-4e07-a6f2-18c80d91cab7`.
  An unrelated local host changed the shared feed during the confirmation test. The corrected test parks that host
  before reading the feed revision. The original Codex fixture allowed the fake supervisor to connect and update the
  settings record before the test read it. Run `2df93465-0b66-426d-8985-6bea258fe011` reproduced that failure with
  `cargo nextest run --locked -p farhelm-helm --lib` and an exact filter for
  `provisioning::service::tensorlake_add::tests::codex_subscription_and_none_enter_setup_without_an_endpoint`. The
  corrected fixture holds the host offline from registration and checks the setup's backend events.
- All 365 selected tests passed on `8f7e6665`, run `d88fe1ff-dc42-42c1-a805-a0cd0e3b6eed`, including both Codex modes.
  The recorder ran
  `cargo nextest run --locked -p farhelm-helm -p farhelm-gui --lib -E 'test(tensorlake_add) | test(/^hosts::/)'`. No
  selected test returned early for missing runtime support.
- The initial Clippy run flagged an unnecessary clone in the confirmation test. The final comparison borrows the same
  value. The exact confirmation test passed on `2394b086` in run `fcfda1ee-d95a-491e-b431-8b5206de1565`. It used the
  combined command with an exact filter for
  `provisioning::service::tensorlake_add::tests::a_setup_confirmation_refuses_before_replacing_failed_progress`. The
  broader run still covers the other cases because the rest of the final diff only clarifies a test comment.
- Final tip `2394b086` passed both Clippy configurations, `cargo clippy --locked --all-targets -- -D warnings` and
  `cargo clippy --locked -p farhelm --bins -- -D warnings`. Rust formatting, Markdown formatting, changelog format and
  whitespace checks passed. The delay audit found a rationale for each of the 306 existing delays it inspected.
- Checked main through `821913ed`. The custom-agent and catalog changes affect launch, restart and settings. The
  Tailscale changes affect SSH routing and retire old connections only for Tailscale entries. The Tensorlake setup,
  credential and fixture paths used here and the cache sweep were unaffected, so the existing runtime evidence still
  applies.
- The tests exercise refused operations, previous progress and settings reads through controlled sandbox and host
  fixtures. That coverage was sufficient for this plan's checks. The full workspace suite and live Tensorlake tests were
  skipped, so the results cover simulated sandbox states.

## Review gate

The three PRs passed independent Opus 5.5 source reviews at high effort after findings were addressed. The cache review
added coverage for an earlier leftover blocking removal of a remaining cache. The rerun review added coverage for active
and failed deletion and kept the existing deletion safeguard for first setup. The settings review led to clearer
documentation of the fresh-host assertion. All findings were resolved in the final revisions.
