# Tensorlake test harness still allocates the sandbox before recording ownership in ledger.jsonl

Reviewed commit: ced3e3e2263fa4ceadab6ab747252e505f68bc7a

## TLDR

The Tensorlake test harness still creates a worker sandbox before recording it in its cleanup ledger, so an interruption
or ledger write failure during creation can leave an untracked sandbox running.

## Details

**other.** Source inspection. Main reference `scripts/tensorlake/sandbox-tests.ts:33`.

In `scripts/tensorlake/sandbox-tests.ts:33-40`, `create()` invokes
`tl sbx create ${fromSnapshot} --timeout 60 --disk_mb ${disk} ${name}` (line 35) and only appends
`{ name, id, disk_mb, ... }` to `${scratch}/ledger.jsonl` afterward (line 38). Commit `ced3e3e2` moved
`await Deno.mkdir(dir)` ahead of `create()` in `scripts/tensorlake/rust-tests.ts:137-138` (preventing sandbox allocation
when `${out}/shard${k}` already exists) and listed `worker-allocation-precedes-ownership-ledger.md` under Review
outcomes, but left `sandbox-tests.ts:33-40` unchanged. If the process is interrupted after `tl sbx create` creates the
sandbox on Tensorlake and before line 38 finishes, or if `Deno.writeTextFile` at line 38 fails, `create()` throws before
`rust-tests.ts:141`'s `try ... finally` block is entered and no record of `name` or `id` exists in `ledger.jsonl`, so
neither the shard's `finally` nor `sweep()` terminates the sandbox or deletes its snapshots. Fix direction: append a
pending ownership entry carrying `name` to `ledger.jsonl` before invoking `tl sbx create`, update/append the canonical
`id` once created, and teach `sweep()` to reconcile pending names against `tl sbx ls`.

Reviewer metadata (g08-tensorlake-specs-and-tests/f2): suggested bucket: other (cannot be highest: maintainer
test-runner tooling only; a crash or write failure during create can leak a test sandbox outside the ledger);
confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g08-tensorlake-specs-and-tests/f2`.
