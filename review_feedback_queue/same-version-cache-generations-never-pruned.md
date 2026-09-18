# Same-version payload cache generations from other base URLs are never pruned

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Every time the download server setting is switched (release server to mirror, mirror to test fixture), tens of megabytes
of downloaded files are abandoned in the cache forever — disk usage grows with each switch and nothing ever cleans it
up.

## Details

Source: pre-pr-review-swarm, area provisioning, 2026-09-17. Confidence: definite (correctness-state-lifecycle p3).
Coordinator confirmed the prune predicate, the keeping comment, and the contradicting single-process guarantee.

Payloads cache under `<state>/payloads/v{version}-{urlhash12}` per version+base-URL. `housekeeping`
(release_payloads.rs:964) prunes other-version generations, but `is_stale_generation` returns false for any generation
whose version parses equal regardless of URL hash (1059-1084). The keeping comment justifies this as "other base URLs a
running helm may still be using" (942-944) — but the same module documents "a helm is one process per state directory
(plan Step 3), so there is no second writer to coordinate with" (400-402), which is what justifies the in-process locks.
No concurrent helm shares the cache root, so nothing can be using abandoned generations. Cost: ~50–100 MB per abandoned
URL, no TTL, LRU, or manual path.

Suggested fix: in `housekeeping`, also prune same-version generations not matching the current source's own generation
(safe under the single-process guarantee), keeping only the current `cache_dir`; or bound them by age. Update the
keeping comment to match.
