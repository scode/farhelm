# Orphan cleanup start does not invalidate feed

Reviewed commit: c340166d754d3edb3de7fd538c6408948d3536c6

## TLDR

Starting orphan cleanup does not notify the live feed, so other views may miss its new in-progress state.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/tensorlake/orphans.rs:192-196,270-273 at c340166d754d3edb3de7fd538c6408948d3536c6`.

Source assessment (read with the later corrections below) (review record g073/f3, event 7):

Reviewed commit: c340166d754d3edb3de7fd538c6408948d3536c6. TLDR: Sandbox cleanup keeps showing Delete/Retry cleanup
instead of Cleaning up until it finishes, the user refreshes, or an unrelated fleet change occurs. Details: This event
corrects the prior f3 source line numbers after checking numbered pinned source. orphans.rs:192-194 records Running;
line 196 awaits the vendor sweep; lines 270-273 notify the fleet only after delete_orphan returns. UI orphans.rs:74-78
reads on feed changes and lines 123-133 set busy without changing rows or reloading at admission. Its Cleaning up label
at line 111 depends solely on the returned row, so the initiating client also retains its old label. feed.rs documents
notification-driven reads with polling only while the feed is unhealthy. SPEC.md Topology and SPEC_impl.md Tensorlake
recipe tunnels and unowned sandboxes require visible running cleanup. Publish an invalidation upon admission outside the
mutex and update the initiating view. Suggested bucket: other; progress and stale controls, with duplicate destruction
refused by orphans.rs:186-190. Confidence: confirmed from source. proposed_drop: false. possible_cover: unknown;
coverage sources not searched. No later repair through 8a8c9579e04299410652f0e4681975a7198dfc7b: the sole later
assigned-file edit, 8bd92e1336565dfc263fe5f3fa2fef413a82a87a, adds refresh feedback but leaves admission unchanged.
Caveat: unrelated notifications/manual refresh mask the issue; no runtime tests run.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g073/f3`; supporting records
`g074/f7`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
