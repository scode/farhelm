# Tensorlake Rust runner parses completed test count from head-60-truncated summary.txt

Reviewed commit: ced3e3e2263fa4ceadab6ab747252e505f68bc7a

## TLDR

The Tensorlake Rust test runner can falsely fail a passing shard and leave its test sandbox running when a shard emits
60 or more skipped-test lines.

## Details

**other.** Source inspection. Main reference `scripts/tensorlake/rust-tests.ts:165`.

Commit `ced3e3e2` added `completedTests(summary)` at `scripts/tensorlake/rust-tests.ts:115-120` and calls it at line 165
on `summary.txt` (before setting `collected = true` at line 166). However, `scripts/tensorlake/run-rust-shard.sh:123`
builds `summary.txt` with `grep -E '^\s+Summary |^\s+FAIL |SKIPPED' "$OUT/console.log" | sort -u | head -60`. Because
`.config/nextest.toml:13` sets `success-output = "immediate"`, `SKIPPED` lines printed by passing tests are written to
`$OUT/console.log`. In standard UTF-8 locales on Ubuntu (`en_US.UTF-8`), `SKIPPED...` (`sk...`) sorts alphabetically
before `    Summary [...]` (`su...`) — and in `C` locale, any `>4`-space-indented `SKIPPED`/`FAIL` line sorts before
`    Summary [...]`. Verified via `LC_ALL=en_US.UTF-8 sort -u` and `LC_ALL=C sort -u`. When a passing shard
produces >=60 unique matching lines that sort before `Summary`, `head -60` evicts nextest's `Summary [...] N tests run:`
line from `summary.txt`. `completedTests(summary)` then throws `shard finished without nextest's completed test count`
while `collected` is still `false`, failing the run (`SOME SHARDS FAILED`, exit 1) and recording `retained_sandbox` in
`ledger.jsonl` so the automatic sweep keeps the billed sandbox alive. `rust-tests.ts:152` already copies the complete,
untruncated `${dir}/console.log` locally for every non-setup-failed shard (on both new and reused `--staged` snapshots).
Fix direction: parse `completedTests()` from `${dir}/console.log` (or emit the `Summary` line in `run-rust-shard.sh`
outside the `sort -u | head -60` cap, falling back to `console.log` for reused staging snapshots).

Reviewer metadata (g08-tensorlake-specs-and-tests/l1): suggested bucket: other (cannot be highest: maintainer
test-runner tooling only; can fail passing shards and keep a test sandbox retained until explicit sweep); confidence:
confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g08-tensorlake-specs-and-tests/l1`.
