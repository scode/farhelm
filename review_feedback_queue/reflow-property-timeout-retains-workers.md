# Shrinking can keep timed-out allocating workers alive

Reviewed commit: a98147fb2eaaefb448473107529ada69114d7b03

## TLDR

If an allocating terminal regression returns, its property test can keep consuming memory after the advertised
ten-second timeout and start more workers while shrinking. No present emulator regression or measured memory exhaustion
is claimed.

## Details

`crates/farhelm-terminal-emulator/tests/narrow.rs:39–53` detaches its worker and panics when the receive times out. Its
containment argument relies on the test process exiting after that panic. The property at `146–167` calls this helper
inside proptest, which catches the panic and continues shrinking. The original worker remains alive; additional timeouts
can accumulate more workers. The ordinary nextest configuration supplies only a five-minute outer timeout.

The reviewer checked the pinned proptest 1.12.0 runner's panic handling and shrinking control flow, not a different
version or a runtime mutation. Use a killable child, terminate the test process on timeout, or prevent shrinking from
catching this timeout so that the intended process-exit boundary actually occurs.

Suggested bucket: `other`, confined to test containment on the established evidence. Confidence: confirmed control-flow
and worker-lifetime defect. Memory exhaustion additionally requires an allocating regression; catastrophic collateral
harm was not established. No exact coverage was found, and accumulating allocating workers do not match the rare safe
failure filter.

Provenance: g049/f1; Codex gpt-6.1-sol, high reasoning. No allocating regression was reproduced, no mutation test ran,
and no upstream-library defect is asserted by this record.
