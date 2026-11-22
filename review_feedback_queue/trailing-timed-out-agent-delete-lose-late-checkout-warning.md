# A timed-out agent Delete can lose its late checkout warning

Reviewed commit: df6199142d6495a95b2d008400ba2686efb775c6

## TLDR

An approved agent Delete can leave a checkout behind after its relay has timed out, with no warning in the app's notice
history.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/agent_requests.rs:2828`.

SPEC.md:4332–4338 gives agent Delete the UI's teardown and checkout notices while specifying an unknown outcome for a
lost reply. The rail notice is how the shared contract surfaces a known cleanup problem after the reply bound
(SPEC.md:2012–2018, :3211–3212). The helm's agent_delete goes directly through do_delete_session or
delete_session_within_environment and returns AgentReply::Deleted (agent_requests.rs:2824–2835), bypassing the HTTP
collector (sessions.rs:4035ff). The relay budgets the call from transport queueing, ordinarily 30 seconds plus approval
wait for Delete (service/core.rs:313, service/agent_relay.rs:315–334, :685–707, proto/lib.rs:2281–2298). Agent Delete
refuses the target's occupied agent fence, but subsequent admission, directory and lifecycle waits in
service/handlers.rs:1224–1243 are unbounded and can run past the relay wait. The timed-out mutation retains the relay
fence until answer or connection loss, and the retention task discards the returned outcome (agent_relay.rs:342–362).
The timely CLI path prints the notice (farhelm/src/main.rs:2444–2453). An eventual successful teardown with an archive
warning is therefore only available in internal supervisor diagnostics, not a notice the user can read. Suggested
direction: make the relay's late-action result carry the same checkout notice ownership as a timed-out GUI Delete while
retaining the unknown-outcome reply semantics. Bucket: other for this narrow timing and loss of UI diagnostics.
Confidence: confirmed by source. No runtime observation. Possible coverage: l7 is the ordinary HTTP Replace path and
does not handle agent relay replies. agent_delete_preserves_the_card_guard_and_checkout_notice asserts timely return,
not a late notice.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `notices/l9`.
