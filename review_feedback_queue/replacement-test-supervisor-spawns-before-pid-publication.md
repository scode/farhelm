# Replacement test supervisor spawns before cleanup registration

Reviewed commit: 1f991980581df1304ce955e40b5bdee34942a7f3

## TLDR

Interrupting a browser worker during supervisor restoration can leave its replacement supervisor running after stack
cleanup, because the worker has not yet published the process identity needed to stop it.

## Details

Suggested bucket: **other**, for an owned test-fixture leak. Confidence: **confirmed conditional source mechanism**;
interruption timing and leak duration were not measured. Unrelated processes are outside this finding.

`e2e/tests/terminal-multihost.spec.ts:744–752` spawns detached, then writes the cleanup PID at `759`. Worker death
between those operations leaves no usable identity for `e2e/start-stack.sh:277–288`, which removes state at `297`. This
launch supplies no stdin lifetime tether (`crates/farhelm/src/main.rs:1608–1616`), and the supervisor does not exit
merely for idleness (`crates/farhelm-supervisor/src/service/core.rs:7004–7046`). A subsequent worker can observe the
untracked supervisor serving and leave it alive.

Normal successful publication has cleanup. Guards against spawning over a serving supervisor and the later three-stack
isolation change do not close this per-stack publication gap. Keep cleanup ownership continuous across spawn and
registration, for example through an owner tether or a parent-owned launch-and-registration mechanism.

Possible cover: no exact match found. Healthy-filesystem failure acceptance excludes ordinary cancellation;
single-user/PID tooling conventions do not accept a lost lifecycle owner. The shell-trap-deferral TODO and the ledger's
failed-second-spawn case are different mechanisms. A persistent process leak does not fully match the rare-diagnostic
filter. Proposed drop: none.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g072/f1`; `g071` independently reconciled the
interrupted-publication gap with its normal-publication check.
