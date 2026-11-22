# Re-adoption can publish a missing terminal without invalidating the session list

Reviewed commit: 063203912059cb23101d9a458d8821f02f1bc18f

## TLDR

If re-adoption finds that an unreadable main terminal has gone, the recovery card can wait for a later refresh.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/host_handover.rs:1743`.

host_handover.rs:1680-1705 replaces the unavailable handle with None for RowHost::Absent and :1743-1749 only logs it.
:1553-1563, :1526-1530 and :1394-1419 neither send a hint nor publish a new runtime on this outcome.
status.rs:319-323,345-347 changes terminal_missing from false to true. The first unavailability event hints through
ticker.rs:937-951,1009-1018, but the Absent result runs in a later retry and emits no second runtime event. An
already-terminal durable outcome needs no later outcome transition. SPEC_impl.md:3072-3088 explicitly requires a hint on
changes to listed values and describes the three-second poll as a backstop. SPEC.md:2344-2347 gives this bit its
recovery action. Hint after publishing an Absent result, independently of re-adoption's successful-runtime return or a
durable outcome change. Bucket: other for missed invalidation with a periodic refresh backstop. Confidence:
source-confirmed omission, with no observed notification or display timing. Possible coverage: recovery_contract. This
is separate from r10's identity preservation and r16's directly notified Gone transition. Existing tip
ticker.rs:2930-2963 confirms the direct runtime wake hints. The checked host_terminals.rs tests cover readoption
fencing, backoff, and successful adoption with replayed exit, but do not assert listing invalidation when an
already-unavailable terminal produces Absent on a retry. The periodic list refresh still reports the missing value
correctly. Comparison from ownership review: host_tabs.rs:1173-1206 explicitly hints after a successful tab retry
install, including Gone. The main-terminal Absent install has no equivalent. Do not count this sibling check as another
invalidation finding.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery/r19`.
