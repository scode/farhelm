# Brick-test cleanup refuses an untested remote Mac pair after a stop-after-P0 request

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

During a release brick test, if the maintainer asks to stop after P0 checks after the dedicated remote-Mac test machines
were created but before they were tested, ordinary cleanup of those machines is refused. The run cannot finish cleanup
without stopping to ask the maintainer about keeping machines that hold no test evidence at all, and then using the
discard override. Nothing is lost; the stop request just stalls on an unnecessary question. Whether a stop-after-P0
request ever arrives in that window is not established.

## Details

Concerns eafc3e7ffd. Main reference `releasing/mac-vm-test/brick_test.py:1884-1895` (cleanup's tiered `complete`
expression, now including `self.native_preflight_retained(entry)`).

The remote Mac pass (`REMOTE_MAC_PASS`) has only P1 checkpoints (`brick_test.py:280-286`), so `p0['required']` is empty
for it. `require_priority` lets it be prepared only with P1 in scope (`:1631-1636`), but `limit_scope` (`:1399-1418`)
can narrow the run to p0-only at any time afterwards, and `releasing/mac-vm-test/BRICK-TEST.md:142-146` directs a
mid-run stop-after-P0 to "finish only the remaining P0 work and cleanup". Once narrowed, `test` refuses the pair
(`require_priority`: "outside this run scope"), so it never gets a native preflight.

Before this commit, cleanup's tiered gate for that pair was all-true (no P0 required, no automation failure, `in_scope`
empty, no results), so ordinary cleanup released the untested guests. eafc3e7ffd added
`self.native_preflight_retained(entry)` to the gate unconditionally; under the guest-native backend it returns False
when `native_preflight` is absent (`:1675-1680`), so cleanup raises "pass is incomplete or failed; ask about retention
before --discard-failed". `BRICK-TEST.md:715-716` says "On the dedicated Mac pair, ordinary cleanup requires a result
for every in-scope P1 checkpoint. Out-of-scope tiers need no result." The new gate contradicts that for an untested
pair. The commit's own test (`test_native_coordinator.py`) covers only a pair that has recorded results and then lost
its preflight, which is the case the check was meant for.

Verify by reading cleanup at `:1884-1895` together with `native_preflight_retained` at `:1675-1680` and the preflight
sequencing in `test_owned` at `:937-947`. Fix: require the retained preflight only when the pair has recorded any result
or retry (or when `native_preflight` is present), for example
`(not entry['results'] and not entry.get('native_preflight') and not entry.get('tester')) or self.native_preflight_retained(entry)`,
and add a test that a prepared, never-tested remote Mac pair under stop-after-P0 cleans up normally.

Bucket: other (a maintainer-tooling stall that asks one extra retention question; no user data or security consequence).
Not highest: the harm is confined to release tooling and loses nothing.

Confidence: likely. Source shows the route (prepare under P1 scope, `limit_scope` to p0-only, test refused, cleanup
refused); not established whether operators ever issue stop-after-P0 in the window between remote-Mac preparation and
its first test.

Possible cover: none known; "Out-of-scope tiers need no result" (`BRICK-TEST.md:716`) is the contract it breaks.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g08 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
