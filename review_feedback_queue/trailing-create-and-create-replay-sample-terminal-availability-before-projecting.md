# Create and create replay sample terminal availability before projecting checkout details

Reviewed commit: 063203912059cb23101d9a458d8821f02f1bc18f

## TLDR

A create or a keyed replay can answer that its session has a terminal after that host has disappeared.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core.rs:10304`.

core.rs:10304-10319 reads the new bit for replay, but :10353 awaits with_checkout_metadata without re-reading it. Fresh
create takes the sample while publishing at :11905-11912, then awaits notification reload and projection at
:11954-11986. with_checkout_metadata awaits the registry at :11991-11996. A Gone transition in either window is visible
to a new listing through status.rs:345-347, and ticker.rs:937-951,1009-1018 hints listings on that transition without
waiting for an outcome change. The reply still carries the earlier value. SPEC_impl.md:8379-8383 defines the bit as the
supervisor's missing-terminal report and SPEC.md:2344-2347 uses it to decide whether to attach. Re-sample at the
successful create/replay reply boundary, after metadata. Bucket: other for established stale response data. Confidence:
source confirms the ordering, with no observed loss, attach timing, or UI duration. Possible coverage:
recovery_contract. Creates and keyed replays are separate producer routes from r8's Restart with settings write. A
common final-projection implementation could cover both. Interface verification: handlers.rs:480 sends SessionCreated
from that value. The pinned helm accepts and caches the whole SessionInfo via sessions.rs:2261-2273 and :2714-2731.
Session_cache.rs:162-172,187-192 leaves terminal_missing as supplied, so the stale false value can overwrite the missing
bit in an already-fresher listing. Runtime display delay remains unverified. store.rs:3673-3718 confirms checkout
projection updates only repository and membership metadata. Existing tip core.rs:34694-34761 asserts registry
projections on create and replay, and :35494-35523 checks a replay that already lacks a terminal before constructing the
reply. Neither assertion covers disappearance during projection.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery/r17`.
