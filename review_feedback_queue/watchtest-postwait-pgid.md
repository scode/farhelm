# Watcher stop test can kill a group created after its fixture ended

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Watcher stop test can kill a group created after its fixture ended.

## Details

`F44 / COR-WATCHTEST-POSTWAIT-PGID` — **possible** — `scripts/test-plans-watch.sh:460` — Watcher stop test can kill a
group created after its fixture ended

The watcher stop test checks that stopping a watcher leaves no children behind. It starts the fixture in a separate
process group, records that group number, sends TERM to the watcher alone, waits for the watcher to exit, and allows
another 0.3 seconds for leftover children to become visible. It then searches the recorded group number; on failure it
also kills processes in that group.

If the original group has disappeared and its number is reused after the leader is reaped, the test could classify
unrelated processes as leaked fixture children and kill them during failure cleanup. Neither disappearance followed by
reuse nor a wrong-group kill was reproduced. Keep an owned leader or other verified identities through the observation
and cleanup period, rather than treating a historical group number as proof of ownership. This is a test-tool risk.
Proposed bucket: highest. No possible cover was identified.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `auto_security p1`, `auto_secrets p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Unverified original group disappearance and reuse, testtool only.

## Filed reviewer metadata

- `auto_security p1`: confidence as filed: possible / likely with the unverified premise that the original group
  disappears and its PGID is reused during the post-wait observation/diagnostic window. No reproduction. Suggested
  bucket as filed: highest. Confidence: possible / likely with the unverified premise that the original group disappears
  and its PGID is reused during the post-wait observation/diagnostic window. No reproduction.
- `auto_secrets p1`: confidence as filed: possible; unverified premise that the fixture group disappears and its numeric
  group ID is reused during the post-wait observation window. Suggested bucket as filed: highest.
