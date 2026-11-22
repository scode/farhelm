# Reconnect regression test does not exercise a push waiting for a host slot

Reviewed commit: 666992f3ac8624f7826779bdefedbd78da92fc54

## TLDR

Tests can keep passing if a replacement already queued for a host resumes under the wrong connection lifetime.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:1362`.

The new assertion waits for a push to enter the fake transport before replacing its lease, then checks cancellation,
unchanged operations, bytes and stamp (credential_delivery_tests.rs:1362-1390). It exercises the watch inside push_one,
but would survive dropping generation capture at candidate enumeration and instead taking the latest generation when
push_one starts. Production captures before admitting work to the four-host buffer (credential_delivery.rs:431-469) and
checks it again before assigning an attempt (193-204, 523-535). The adjacent host-lock deadline and no-binding tests do
not turn over a fully bound candidate while it is still outside the buffer (credential_delivery_tests.rs:1307-1355). The
overlapping-lease test starts its push after the first lease drops (1393-1408). SPEC_impl.md:5995-5997 explicitly covers
both queued and running pushes. A separate source-level test assertion should make queue admission and successor binding
observable so old work cannot be mistaken for a successful successor delivery. Bucket: other, regression coverage gap.
Confidence: confirmed by the source and existing assertions. No test execution. Possible coverage: none identified in
assigned assertions.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `access/f1`.
