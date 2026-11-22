# A retry can delete the ambiguous Restart record before its scope is confirmed

Reviewed commit: 063203912059cb23101d9a458d8821f02f1bc18f

## TLDR

If a failed Restart leaves an unresolved scope, another refused Restart can erase the evidence that blocks startup from
reopening the older terminal.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/host_restart.rs:593`.

host_restart.rs:764-783 keeps the failed generation's record when its named scope cannot be confirmed.
Core.rs:12929-12955,13001-13006,13053-13085 keep the Launching generation, its scope and no published main terminal. A
retry with no held terminal processes all agent records (host_restart.rs:531-568) and deletes a Gone socket's record at
:588-600. core.rs:12526-12528 does that before the :12585-12598 scope check, so a still-unusable manager refuses the
retry after the record was erased. On supervisor startup, host_handover.rs:1190-1200 and :763-777 treat missing address
and record as NoHostStarted; :907-941 may recover the earlier still-readable host. SPEC_impl.md:2090-2099 requires the
new record to stay until sweep and scope settle; :2108-2117 and :2125-2130 reserve the older-host fallback for a
clean/no-record window and describe why the ambiguous case keeps the earlier host out of publication. Delay deleting the
current unresolved generation's record until scope cleanup is confirmed, or retain durable evidence that distinguishes
this state from never started. Bucket: other for loss of recovery classification and possible older-terminal
publication. The stored scope selection still independently blocks an unconfirmed relaunch. Neither an overlapping
process nor lost user work is claimed. Confidence: source confirms removal and the subsequent decision conditions. No
observed startup result. Possible coverage: lifecycle_e2e, recovery_contract. Parent
fc2b0b46e6211c7ca4001d05d90221f35ee5bd97 host_restart.rs:759-783 removed the record before the initial sweep and treated
scope errors as warnings. #4527 gives the initial unwind a stricter fence. The unchanged retry path can undo it.
Existing host_restart.rs:899-910 asserts the first unwind retains the record, with no public retry or reload. The tip
host_relaunches.rs claim-window cases check the intended no-record fallback. The ambiguous replacement case continues to
Delete, not a repeated scope refusal. On adoption of a readable older host, core.rs:7035-7059 can also store its address
under the current generation when startup sees the processed exit.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery/r18`.
