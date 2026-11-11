## What this was about

Tensorlake builds and sandbox connections could offer a configured personal certificate or hardware-token key before
Farhelm's registered key, even with SSH-agent use disabled. A gateway that refuses the first unfamiliar key could then
refuse that connection. The maintainer asked to close the documented exceptions while preserving the user's proxy and
jump configuration and the OpenSSH 7.6 minimum.

The shared Tensorlake options now restrict accepted public-key types to plain ed25519 and disable token providers.
Farhelm's generated key is the first public-key login candidate. Other configured file keys may follow it. The same
options cover snapshot commands, preparation, host setup, credential delivery and host connections. The named TODO entry
is removed, and the existing unreleased changelog fragment is updated.

## Things you should know

- A real local OpenSSH 9.6p1 experiment proved the ordering change. Its control connection actually offered the
  configured certificate before Farhelm's stand-in key. With the new policy, the certificate was filtered and Farhelm's
  key was the first actual offer. Both connections authenticated and completed their command. An extra plain key
  remained eligible after Farhelm. The fixture's server and children were stopped and reaped.
- OpenSSH 7.6–7.9 compatibility was checked against upstream source. Those clients treat the disabled provider value as
  a library name and can print a nonfatal load error, as the maintainer explicitly accepted. Runtime verification used
  9.6p1. The older option spelling produced no deprecation warning there.
- The experiment used a configured nonexistent provider to distinguish override behavior. Actual hardware-token loading
  was assessed from upstream source. Tensorlake's live gateway and account were outside the experiment.
- The production and test/spec diff is 149 inserted and deleted lines, excluding the TODO removal and fragment, within
  the plan's approximate 150-line gate.

## Open questions and possible follow-ups

User configurations that disable public-key login, or enable a separate method such as hostbased authentication, retain
those authentication choices. The ordering promise concerns public-key user identities with Farhelm's generated pair
available. On old OpenSSH, an additional private-only key can still be tried later if Farhelm's key is refused. The live
gateway's behavior on the first unfamiliar key remains an assumption rather than a new observation.

## The PRs

- [#3312](https://github.com/scode/farhelm/pull/3312/changes), one draft, `plan/tensorlake-key-order/01-key-types`.

## Checks run, reused and skipped

- Passed seven focused Rust tests in run `063afbf0-d365-49bb-9dff-0c626b2b4e44`, with pinned nextest and tmux, four
  slots and zero retries. The selection covered the shared Tensorlake options, caller argv, ordinary SSH exclusion,
  shared-master teardown and the generated-key/accepted-type coupling. The command was
  `cargo nextest run -p farhelm-helm --lib -E 'test(/^tensorlake::ssh::/) | test(tensorlake_access_adds_its_options_before_the_terminator) | test(a_tensorlake_row_is_dialed_with_farhelms_key) | test(releasing_a_sandbox_exits_both_shared_masters) | test(the_ssh_key_is_created_only_on_request_with_private_permissions)'`.
  Another 1,669 tests were filtered out and two ignored tests were left out. JUnit and retained output are complete,
  with 215,398 bytes, observed EOF, zero omissions, truncation or console loss, and no forced cleanup or runtime SKIPPED
  message. Source identity matches the local experiment below. Later comment-only corrections preserve this run's
  coverage.
- Passed `cargo clippy -p farhelm-helm --all-targets -- -D warnings` on the final tree.
- Passed current `cargo fmt --all -- --check`, targeted `dprint check`, `python3 releasing/check-changelog.py format`
  (41 fragments), and `python -B scripts/check-test-sleeps.py` (429 delays, zero missing rationale). The first format
  check found one new assertion's wrapping, which was corrected.
- Passed the local SSH experiment through the recorder, run `21cbea8a-a77d-4832-b3d6-246dfb2704bc`, using
  `deno run -A <local-fixture-script>` with one private loopback sshd and sequential control/policy clients. Source and
  retained output are complete, with 44,454 bytes, observed EOF, zero omissions/truncation, zero console loss and no
  forced cleanup. The working tree identity was `c7de66dd38a6c54aa8ad1c594e5005a28cc2fd96` plus the implementation,
  fingerprint `578dc8dc576e11a35b522f449f41eea3180230ce723ca457eea1fba4ac7502d0`. Later edits only corrected comments.
- Four retained fixture-development failures preceded success, all accounted for. `dd925cb1-3037-4192-bea6-78668c188ec2`
  assumed ssh-G printed a disabled provider instead of omitting it. `5ee2d2b5-cdf1-4641-aef8-cc9ab3956760` and
  `de451425-0ab8-49da-b5d0-f71254f7e418` used unsupported helper redirection forms and started no server.
  `bec958d1-6747-4081-b91a-e6edecd71805` used buffered helper output for readiness, then stopped and reaped its server
  without making a client connection. The final fixture observed sshd's own direct log. Seven helper commands passed
  across the eleven retained attempts, including the final two parser/redirection checks in
  `381edea5-724e-4157-b35c-66f38bdf8515`. These were fixture setup corrections, with no product-policy failure or
  latent-flake claim.
- Skipped browser, desktop, full-workspace and live-account tests because this changes the shared SSH options and
  explanatory text. The focused caller tests and local authentication experiment cover that behavior. Old-version
  runtime and real-token checks were replaced by primary upstream source inspection, not claimed as runtime passes.

## Review gate's outcome

The required fresh gpt-6.1-sol reviewer at high effort found no actionable issues in the final diff, regression tests,
source compatibility analysis and retained local authentication evidence. The commit-message cold read passed. The first
cold reader disclosed reading instructions too early and its assessment was discarded. A fresh reader identified an
unclear compatibility sentence, the body was rewritten, and the final fresh reader understood every intended point.
