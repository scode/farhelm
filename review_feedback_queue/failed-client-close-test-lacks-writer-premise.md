# The failed-client close test can run before its writer

Reviewed commit: d5a7f8de5ea949628ce146dcd7944852baa40db1

## TLDR

A regression test can pass without exercising the continuously sending peer it claims to cover. No production
unbounded-read defect was established.

## Details

`crates/terminal-host/src/host.rs:3733–3737` starts a writer thread, immediately steps the host, and checks connection
closure. The host can run before the writer, close after its failed reply, and encounter an empty socket during its
final receive. The writer then fails its first write. That schedule still passes if the close path stops supplying a
finite final-read budget.

The separate helper-limit regression at `3931–3985` checks a supplied limit, not the budget supplied by `close_client`
at `742–744`. The production call currently supplies a finite budget. Establish the adversarial receive premise or
observe the close path's requested budget directly. A writer-start barrier alone does not prove this boundary was
exercised.

Suggested bucket: `other`. Confidence: confirmed missing fixture witness. Runtime frequency and mutation survival were
not measured. No exact coverage or applicable filter was found. The terminal-host specification's bounded-close contract
explains why this distinction matters; it does not accept a test that misses it.

Provenance: g034/f1; Codex gpt-6.1-sol, high reasoning. No runtime reproduction or mutation ran.
