# The pre-client output test connects during its premise wait

Reviewed commit: d5a7f8de5ea949628ce146dcd7944852baa40db1

## TLDR

A test meant to protect output produced before the first client can connect before that output is produced. It can
therefore pass without exercising its intended boundary.

## Details

`crates/terminal-host/tests/host_lifecycle.rs:31–35` immediately negotiates clients while polling for the child to exit.
Startup readiness proves successful exec, not completed output. If the first Hello precedes printing, a regression that
loses output before the first negotiated client can pass unnoticed.

Observe finished output independently before opening that client. The startup helper's raw credential probe is a
separate earlier socket connection: a test claiming literally no socket connection must account for it too. Snapshot and
slow-client tests do not supply the same premise; their clients are already connected.

Suggested bucket: `other`. Confidence: confirmed ordering gap; no missing source premise. No product output-loss
regression or runtime frequency is claimed. No exact coverage or applicable filter was found.

Provenance: g038/f3; Codex gpt-6.1-sol, high reasoning. The distinction between the raw probe and a negotiated client
was retained during synthesis.
