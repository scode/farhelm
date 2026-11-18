# Connection-refusal tests release their port too soon

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Two tests can reach a different process if it starts listening after the test gives up the port it reserved.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

Both tests release the kernel-assigned port before the client connects. A concurrent process can listen there, so keep
the port reserved with a bound, non-listening socket until each client observes the refusal.

Reviewed locations: `crates/farhelm-desktop/src/updater/transport_tests.rs:167-182`,
`crates/farhelm-helm-client/src/transfers/tests.rs:351-361`. Haiku out-T1.md §3. Suggested bucket **other**.

The root AGENTS.md shared-machine rules require per-fixture isolation. A kernel-selected port stops being private when
the listener is dropped. This is source-confirmed and was not reproduced with a competing listener.
