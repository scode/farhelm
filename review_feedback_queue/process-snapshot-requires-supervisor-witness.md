# Process cleanup accepts an empty process table without a self-witness

Reviewed commit: 1ec60cc06baff3442bcdd73760a9ebdc8b08c49c

## TLDR

Stop, Restart, Delete, or tab cleanup can report success without checking any processes when process enumeration returns
an empty table that should have been treated as broken.

## Details

Found by pre-pr-review-swarm run `20261001-1343-1ec60cc-5337` as `F18 / COR-PROCESS-WITNESS`, tagged **definite**. The
same issue is already named in `TODO.md` as “Snapshot self-witness”; this queue item records the current review evidence
until that planned work is executed.

At `crates/farhelm-supervisor/src/procs.rs:124`, the shared snapshot wrapper accepts an empty result from the Linux or
macOS process enumerator. The cleanup sweep treats the empty map as a successful enumeration with no descendants and
never verifies that the still-running supervisor appears in it. An empty `/proc` directory or empty `KERN_PROC_ALL`
response can therefore make cleanup claim success while detached processes remain.

Reject any successful process table that lacks the current supervisor process. Put the check in the shared wrapper and
cover empty/incomplete tables with an injected or pure validation test.
