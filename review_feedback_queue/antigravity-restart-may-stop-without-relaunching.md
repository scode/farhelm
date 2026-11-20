# A conversation update can leave Antigravity stopped after Restart

Reviewed commit: fdd74ae5e9a612ec89a9d0aaddfee8e08a99695c

## TLDR

If Antigravity reports a conversation change during Restart, Farhelm can stop the current agent and return a refusal
before opening its replacement. The session remains available for another Restart.

## Details

The supervisor snapshots the captured conversation before it stops the current agent. An ordinary callback can advance
capture before the transaction that opens the new launch generation. The transaction then refuses because the offer
changed. Plain Restart depends on the captured offer and needs that check before the old agent is stopped.

At `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see `crates/farhelm-supervisor/src/service/core.rs:12266`,
`crates/farhelm-supervisor/src/service/core.rs:12465` and `crates/farhelm-supervisor/src/store.rs:4639`. The transaction
test checks that a refusal leaves its input row alone. It does not exercise the earlier service stop.

Suggested bucket **high** for the interruption and required retry. Source-confirmed from trailing reviewer g003/l9. The
record of the conversation remains recoverable.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g003/l9.
