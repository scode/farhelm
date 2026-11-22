# A retired lease can still admit a local provisioning command before the next cancellation check

Reviewed commit: 666992f3ac8624f7826779bdefedbd78da92fc54

## TLDR

An outgoing credential replacement can locally start another provisioning operation during retirement. The remote
credential outcome is unverified.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/credential_delivery.rs:533`.

The parent 40d1a549ffe9a1d72100154f59a1c1f9e74bd9ae SPEC_impl.md:5995 says, "Dropping the final lease cancels
in-progress pushes before their next transport poll." The reviewed revision keeps that sentence and adds the
queued/running generation guarantee (SPEC_impl.md:5995-5997), retained at pinned tip
df6199142d6495a95b2d008400ba2686efb775c6 SPEC_impl.md:6008-6010. The biased watch can reject the outgoing generation on
the next outer poll of push_one (credential_delivery.rs:523-541). It is not held through the backend call. The actor can
drop the last lease on a different worker after that watch branch yields Pending but before the backend is polled
(credential_delivery.rs:231-235, 455-476. Crates/farhelm-helm/src/manager.rs:4251-4266.
Crates/farhelm-desktop/src/lib.rs:575-585). The per-host mutex serializes credential operations but is not taken by
final lease Drop. Confirmed path to local transport admission: its settings read calls the provisioning backend
(credential_delivery.rs:643-649. Provisioning/backend.rs:2385-2390), whose read_agent_file invokes run_shell
(provisioning/tensorlake_backend.rs:134-151). Run_shell locally spawns before awaiting capture
(provisioning/backend.rs:909-937). Thus even a read-only backend step has an admitted local command between outer
checks. The transport child's local kill-on-drop governs cancellation of the local wait. It does not establish whether a
remote mutation was started, committed, or survived on the intended host. The source-supported gap applies to the
admission boundary. It does not show secret receipt by a different host or a durable write after retirement. The held
fake-transport test (credential_delivery_tests.rs:1362-1390) yields to the outer poll, verifies an empty cancellation
result and unchanged fake operation list, old bytes and stamp, and provides no witness for same-poll admission or real
remote effects. Keep generation checks for queued work and fence transport admission against retirement, or document the
coarser poll boundary if it is the intended contract. A separate change to delivered data preserving this
lifetime/mutex/warning structure does not resolve the issue. Bucket: other for the proven local admission boundary. A
remote mutation after retirement, its persistence across reconnect, or a wrong-recipient consequence requires separate
evidence of a real backend outcome or shipped recipient path. Confidence: source-supported for local admission. Remote
credential effects are unverified after investigation. The implementation and existing assertions give no cancellation
acknowledgement proving what happened remotely in this window. Retain the narrowed consequence. Possible coverage: none.
Follow-up assertion check: the new test uses the default current-thread test runtime
(crates/farhelm-testtrace-macros/src/lib.rs:247-259) and performs final drop and enter with no intervening await
(credential_delivery_tests.rs:1362-1379). This makes it a source-level oracle for a zero lease count hidden between
outer polls. It cannot place an independent actor's lease Drop between the biased watch and the transport admission
during one worker poll.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `access/l3`.
