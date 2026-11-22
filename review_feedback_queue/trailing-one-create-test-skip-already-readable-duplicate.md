# One-create test can skip an already-readable duplicate

Reviewed commit: 8986f1fda1b8889ad7be3a2874f1a88180b6d609

## TLDR

The test can pass with a duplicate request already waiting on the scripted supervisor stream when the successful launch
returns.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/sessions_tests.rs:483`.

sessions_tests.rs:482–486 races completion and the control frame with no priority. Each create increments the counter
only after the frame branch wins (lines 526–536). The test signals completion as soon as the first successful create
returns and asserts the drained count (lines 585–603). When both branches are ready, completion can win and omit the
duplicate from that count. This weakens the one-create assertion for the exact-preview flow required by
SPEC.md:1820–1826. Order the final drain to give ready frames priority as the Replace responder at
sessions_tests.rs:698–702 does, or tie the observation window to a completed dispatch boundary. This is a test oracle
issue. Bucket: other. Confidence: confirmed by source for already-ready duplicate frames. Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `checkout/l5`.
