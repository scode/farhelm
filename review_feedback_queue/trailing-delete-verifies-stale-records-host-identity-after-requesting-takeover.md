# Delete verifies a stale record's host identity after requesting takeover

Reviewed commit: 85cf6b2e7715c5dce446bcc95170662c1fbc8bb8

## TLDR

If a retained launch record names a socket serving another session whose host already has a client, Delete can close
that owning connection before refusing.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs:1275`.

SPEC_impl.md:2134-2158 promises a no-touch refusal for another tag. service/session_hosts.rs:225-287 discovers this
record when the target row has no runtime holding its socket (an absent terminal or placeholder suffices).
service/core/host_launch.rs:976-1013 connects and obtains Info before service/session_hosts.rs:404-421 verifies the tag.
hosts/client.rs:626-646 and :1280 use the takeover handshake. crates/terminal-host/SPEC.md:277-322 specifies that it
closes the prior host connection and drops its queued, pending or unacted input. SPEC_impl.md:2367-2376 describes
input-epoch failure after lost input, ending the affected attachment and requiring reattachment. A stale record on its
own has no such effect: it must outlive its original socket, that pathname must now serve a differently tagged host, and
that host must have an owning connection to displace. terminal-host/src/host.rs:256-271 can unlink a host socket without
editing the supervisor record, and service/launch_artifacts.rs:744-775 retains agent host records. Pinned source allows
reallocation of a free pathname, but the review establishes the ordering and the conditional consequence, without a
runtime observation of pathname reuse, user-visible duration, or loss of durable user work. The recorded-host settlement
and tag check were already present at the exclusive base and are unchanged in this range. The test manufactures the
mismatch via record_s1 and a terminal-less s1 row (host_terminals.rs:1083-1116,1280-1285). These premises are unchanged
by the assigned commits. #4543 adds only a protocol Shutdown receipt check, and #4546 joins/unlinks the fixture listener
and verifies directory removal. Neither attaches an owning client or asserts that its input attachment survives. Bucket:
other for the demonstrated conditional connection and input effect and fixture gap, with no established termination or
durable work loss. Confidence: source and contract support the ordering and conditional effect. Possible coverage:
recovery, recovery_contract.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_unit/f8`.
