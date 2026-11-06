# Mixed setup plans escape cache bound

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313

## TLDR

A mixture of setup-plan kinds can accumulate retained plan entries beyond the intended cache bound.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/provisioning/service.rs:55,60-65,789-795,1016-1028 at ca28b604b876770b1ab757783416f83ad1709313`.

Source assessment (read with the later corrections below) (review record g048/l2, event 15):

Synthesis recheck: confirmed, suggested bucket other; proposed_drop false; possible_cover unknown. Reviewed commit
ca28b604b876770b1ab757783416f83ad1709313. TLDR: Abandoned Tensorlake creation offers can remain in helm memory beyond
the 64-offer limit when SSH offers evict them, retaining their frozen recipe data until consumption or process exit.
This does not establish unauthorized use or multiple execution.

Exact contract: SPEC_impl.md Adding Tensorlake hosts:5738-5739 requires a bounded one-use outer plan;
Provisioning:6818-6825 calls confirmation ids opaque and one-use. service.rs:55 sets MAX_PENDING_PLANS=64, and :60
describes bounded process-local state. The two plan maps at :63-65 share plan_order. tensorlake_add.rs:242-249 and
service.rs retain_plan:1819-1826 remove evicted ids from both maps. Ordinary SSH Add:789-795 instead pops an id and
removes only memory.plans. Therefore prepare one Tensorlake offer then 64 ordinary absent-supervisor SSH offers: the
Tensorlake id falls out of plan_order but remains in tensorlake_plans. Repeat interleaved kinds and the map grows
without the intended bound, because later queue evictions cannot find the orphaned id. New-host Tensorlake plans have
host=None (tensorlake_add.rs:231-239); forget_host_memory's retain by existing host does not remove them.

Important limit: start_add:1016-1026 removes the Tensorlake plan from its map under the memory mutex before starting
work. The same id remains single-use even after its eviction queue entry is lost. There is no proof of double creation,
guessable ids, an unauthenticated submission, a GUI submitting a cancelled offer, or an attacker obtaining a retained
id. Being still executable after intended eviction is a cache lifetime defect, not by itself unauthorized creation.
Endpoint-token retention in memory does not itself disclose the token. No expiry duration/TTL is specified; the concrete
violated bound is queue capacity, not an invented time-based expiry. Stronger consequences are not claimed.

The omission remains at inclusive pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b: service.rs:792-795 evicts only
plans; :1019-1029 still consumes tensorlake_plans directly. This corrects the earlier event's tip line numbers, which
had not accounted for three inserted lines. Parent will carry this finding to the add-flow interface reviewer. Fix by
centralizing insertion/eviction or removing both maps' entries in the ordinary Add loop; a bounded interleaving test
should assert the old id is rejected and combined map sizes stay bounded. Source-only validation; no runtime tests run.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g048/l2`; supporting records
`g048/i2`, `g071/c7`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
