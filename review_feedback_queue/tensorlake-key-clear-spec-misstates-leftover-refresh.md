# The Tensorlake leftovers specification misstates what clearing the API key does

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The implementation spec says clearing the Tensorlake API key refreshes the leftovers list. The app clears and
invalidates that list instead.

## Details

SPEC_impl.md:6891–6894 includes Clear in the list of read triggers, then :6898–6902 separately says a Clear ends the
round. crates/gui/farhelm-gui/src/settings/tensorlake_key.rs:453–480 passes a successful Clear answer to the account as
a key change; :192–204 publishes the returned account settings. The leftovers view at
crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs:147–168 calls read only if the returned account still has a
saved key. With no key it advances the round, clears rows and the read failure, and resets the timestamp. :193–208
discards an answer from the prior round. The separate deletion error can remain visible (:332–339), so the whole section
need not disappear. No later change in the review range corrected the documented read trigger. Describe Save as a read
trigger and Clear as local list invalidation without a fresh list request, so future clients and tests preserve the
actual behavior.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `host/l13`.
