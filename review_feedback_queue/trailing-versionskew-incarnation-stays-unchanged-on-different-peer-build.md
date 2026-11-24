# Automatic updates stay suppressed when a periodic probe discovers a different older supervisor build or protocol on an already-skewed host

Reviewed commit: 325ae1e1e531bc298b43e7b592de3e6451977c45

## TLDR

If an automatic update fails or is refused on a remote host with an older protocol (for example, because the host's
supervisor is below the upgrade floor) and the host is later upgraded out-of-band to a newer but still incompatible
supervisor build without Farhelm observing it offline in between, Farhelm keeps treating it as the same failed attempt
and does not automatically update it until the user manually clicks Update or Retry or restarts the helm.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/manager.rs:6156`.

In crates/farhelm-helm/src/manager.rs:6156-6165 (inside HostActor::publish_refresh, also unchanged at tip
1b3fe49af86979aebc2719965682a17f8c6366ac), `new_skew` decides whether to mint a fresh incarnation for a VersionSkew
refusal using:

let new_skew = matches!(state, HostState::VersionSkew { .. }) && (!matches!(status.state, HostState::VersionSkew { .. })
|| refusal.as_ref().is_some_and(|refusal| refusal.after_interrupt));

When a host is already in HostState::VersionSkew and a periodic 45-second background probe (`after_interrupt == false`)
observes a DIFFERENT `peer_protocol` or `peer_build` (for example, the host was upgraded out-of-band from a below-floor
build to an above-floor older build, or from one incompatible protocol version to another, and restarted quickly between
probes so no probe observed Unreachable), `status.state != state` holds (triggering `observable_change = true` and
`self.events.bump()`), yet `!matches!(status.state, HostState::VersionSkew { .. })` is false so `new_skew` is false and
`status.incarnation` does not advance. Because `crates/farhelm-helm/src/provisioning/automatic.rs:231,238` suppresses
retries while `host.incarnation` matches `AttemptState::Attempted(incarnation)` or `failed_incarnation`, the automatic
updater skips the changed supervisor even though SPEC.md (lines 195-196) states "Periodic probes that keep seeing the
same protocol mismatch preserve the previous attempt" and SPEC_impl.md (lines 2767-2769) states "An identical background
probe within that state keeps the previous incarnation".

How to verify: In a manager unit test similar to `version_skew_names_both_versions_and_recovers_when_the_host_upgrades`
(crates/farhelm-helm/src/manager.rs:9419), start a host at `PROTOCOL_VERSION - 2` with build `0.15.0`, observe its first
VersionSkew incarnation, edit the script to `PROTOCOL_VERSION - 1` with build `1.9.0` without dropping reachability or
calling `retry_now`, wait for the 45s probe (`refusal_attempt` advances), and observe `status.state` updates while
`status.incarnation` remains unchanged.

Fix: Replace `!matches!(status.state, HostState::VersionSkew { .. })` with `status.state != state` inside `new_skew`
(lines 6156-6160). Since `our_protocol`, `our_build`, and `skew_remediation` (which depends only on `row.destination`,
`peer_protocol`, and `our_protocol`, with destination edits already retargeting via `sync_registry`) are fixed for a
given actor and unchanged row, `status.state != state` is false on every identical background probe and true whenever
the peer enters VersionSkew from another state or reports a different `peer_protocol` or `peer_build`.

Reviewer metadata (g40_host_auto_update_2fa_reconnect_tests/l1): suggested bucket: other (narrow correctness gap against
SPEC.md:195-196 and SPEC_impl.md:2768-2769; requires an out-of-band partial upgrade/change of an already-skewed host
that completes between 45s probes after an earlier automatic failure, and manual Update / Retry still work; no security
or data-loss impact).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g40_host_auto_update_2fa_reconnect_tests/l1`.
