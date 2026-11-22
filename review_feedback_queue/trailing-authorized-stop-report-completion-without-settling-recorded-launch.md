# Authorized Stop can report completion without settling a recorded launch

Reviewed commit: fc2b0b46e6211c7ca4001d05d90221f35ee5bd97

## TLDR

Stop can acknowledge completion of a retained unconfirmed launch without establishing what its recorded terminal-host
start left running.

## Details

**high.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/handlers.rs:839`.

tip core/host_launch.rs:562-579 writes a first-stage pin and socket before delivering the launch. Tip
core.rs:11492-11523,11651-11673 publishes a terminal-less Launching entry and retains the record after the host cannot
be confirmed ended. The existing E2E assertion in host_creates.rs:1181-1251 checks leave-behind listener, Launching row,
listing and Pending intent for the supported branch using a stand-in. It never calls Stop. status.rs:244-247 and GPUI
stop.rs:65-80,219-235 offer Stop from that Unknown row without the ended-agent restriction.
handlers.rs:839-879,945-954,1046-1054,1080-1133 passes the launch through exit classification and rootless scope/marker
cleanup before its normal success at :1153-1159. With no sentinel, the ObservedExit disposition can also convert
Launching to Exited (store.rs:557-560). Neither check settles the recorded startup evidence. Exact parent
4bd7bf366c56d7552c285e36d4d37fb3c8f54f94 already takes the same route. SPEC_impl.md:2052-2063 and host_launch.rs:945-981
explain why an outstanding recorded start can lack a sweep marker or scope; SPEC.md:2067-2073,2272-2279 requires cleanup
or visible refusal. The grouped accepted crash and two-fault cases in SPEC.md:2284-2298 are not needed to show the
omitted check on the normal retained-failure branch. Settle the recorded launch evidence or refuse with the unsettled
status preserved before claiming success. Bucket: high for false completion over outstanding identifiable launch
evidence. Confidence: source-supported omission and reachable retained-failure route. The stand-in assertion does not
establish a real agent surviving Stop or starting after the reply, and no such runtime outcome is claimed. Exact missing
assertion: authorized Stop on a terminal-less Launching row with outstanding recorded start evidence, checking
refusal/retention or confirmed settlement. Possible coverage: recovery_contract only if it traces this same authorized
Stop path. L8/r6 concern a held child pin, and l9/l10 concern authorization boundaries.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `stop/d2`.
