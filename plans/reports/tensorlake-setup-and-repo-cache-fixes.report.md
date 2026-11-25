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

### Landing

#### What landed with it

Only this plan landed this round, through its three PRs. Since the plan's PRs were written, main gained the plans landed
in the previous rounds, among them the environment triage, which made Setup and Update refuse a host whose environment
is being deleted, and the sandbox workspace trust plan, which added a Claude first-run step to Tensorlake setup. This
plan's refusal of a setup rerun builds on the environment triage's refusal and agrees with it: a sandbox being deleted
is refused by both. They differ only in wording: this plan's refusal suggests Retry only once a deletion has actually
failed, while the environment triage's says the host's environment is being deleted. The landing's review found no other
interaction.

The review also checked what the plan changed about the order in which a setup rerun takes its locks, since that order
moved: there is no deadlock, a refusal now happens before the earlier failed run's progress is touched, so nothing is
left half replaced for Hosts to recover from, and a new sandbox's setup waits for its own new host entry exactly as
before. The repository cache cleanup still stops for a repository whenever continuing could touch a checkout in use;
only removals of leftovers already set aside continue past an error. The review counted no blockers, no should-fix items
and five small points: the startup warning and the four in the last bullet of the last section.

#### Fixes made while landing

- None.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PRs rebased onto main as it stood at the start of the landing (`508a81df`); the rebase conflicted only in
TODO.md, where both sides removed entries and I kept both, and every other file's change is exactly what the executor
pushed. The rebased stack's top commit was `da1caf4a`, and that tree is what main now holds outside the plans queue,
apart from a review queue triage record (#4978) that landed on main while this landing ran and touches no code.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `d88239f2-8794-460e-bcea-f2aab455c115`): all 8708 tests passed, with no runtime skip. The plan changed the helm's
  Tensorlake setup, the supervisor's repository cache cleanup and one GUI Hosts setup test; the whole suite was the
  narrowest run that covered them and their callers.
- The Tensorlake setup test `codex_subscription_and_none_enter_setup_without_an_endpoint`, run alone ten times on the
  test worker (recorder runs under the selection "tensorlake_add
  codex_subscription_and_none_enter_setup_without_an_endpoint, attempt N on the rebased stack" in the recorder, for
  example `7ce3f031-d3b3-47ea-87fd-37e35d51c3ec`): passed all ten. The same loop on main as it stood in the previous
  round (`455d5f00`, without this plan) failed 3 of 10, for example recorder run `74684414-d0a4-4527-844f-2df823127ddd`;
  the last section says why that matters.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, `python -B scripts/check-test-sleeps.py`
  and `dprint check`, locally: passed. `python3 scripts/check-agents-md.py`, locally: failed, on an error already on
  main that this plan does not touch (the last section says more).

Reused: none. The executor's runs were focused selections on an older base, and everything they covered ran again above.

Skipped: the desktop app's tests, screenshot comparison and end-to-end smoke, since the plan changed nothing the desktop
app draws or does (its one GUI change is a test, which the workspace suite ran); a live Tensorlake sandbox, which needs
a provider account; the workspace doctests, since no documentation example changed; a Mac run, since nothing changed is
Mac-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **This plan fixes the flaky Tensorlake setup test from draft PR
  [#4757](https://github.com/scode/farhelm/pull/4757/changes).** Its third PR keeps the test's new host offline from
  registration, so the credential delivery that runs once a host connects can no longer change the record before the
  test reads it, which was that draft's suspected cause. The test passed ten of ten runs on this stack, against three
  failures in ten on main just before. The draft, which records the test as an open flake, is now out of date. I
  recommend turning it into a record of a flake closed by this plan (in the flake archive), so the flake's history is
  kept; closing it unmerged is the alternative. Say which you prefer.
- **Main fails `python3 scripts/check-agents-md.py` already.** A review finding recorded by an earlier trailing code
  review (#4923) cites `docs/harness-drift-tests.md` under a heading that page does not have (the words are a sentence
  in it, not a heading). This plan does not touch either file. The fix is a one-line wording change in that finding, but
  nothing will make it on its own, and every instruction change runs that check meanwhile. I recommend a small PR for
  it; say if you want me to open one.
- **(Small) Startup can log a second warning about spare checkout cleanup.** A spare checkout is the managed checkout
  Farhelm keeps ready for the next session. A failure to clean up its record used to be logged and ignored; now the
  cache sweep still finishes the remaining work but also reports itself as failed at the end, so a recurring problem
  there prints two warning lines at every start. It is harmless; no follow-up unless you see it.
- **(Small) Four more small points the review noticed.** The code comment on the new setup refusal says the marker
  Farhelm sets while a sandbox is being deleted means a failed deletion, though it means any deletion in progress; a
  rerun reads the host entry twice, the first time redundantly; the "being deleted" case of the new confirmation test
  sets up a state a real removal cannot produce at that moment (a real removal holds the host's lock, so it is refused
  as busy first), though the case still covers the refusal it is named for; and the plan's SPEC_impl.md amendment, which
  lists the sandbox states that refuse a setup rerun (suspended, changing state, being deleted), leaves out "stopped",
  which the code also refuses, though Tensorlake hosts do not appear to reach that state. None needs a follow-up on its
  own; they would fit a small cleanup PR.
