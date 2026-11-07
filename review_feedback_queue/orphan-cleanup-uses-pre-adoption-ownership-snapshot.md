# Orphan cleanup uses pre adoption ownership snapshot

Reviewed commit: c340166d754d3edb3de7fd538c6408948d3536c6

## TLDR

Orphan cleanup can use an old ownership snapshot to terminate a sandbox after Add has adopted it and ordinary user work
has begun.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Source location:
`crates/farhelm-helm/src/tensorlake/orphans.rs:67-80,175-196,226-241 at c340166d754d3edb3de7fd538c6408948d3536c6`.

Source assessment (read with the later corrections below) (review record g073/f1, event 41):

Reviewed commit: c340166d754d3edb3de7fd538c6408948d3536c6. TLDR: An orphan cleanup can terminate a sandbox after this
helm has adopted it and a user has started new work there, destroying that new work using stale ownership information.
Independent consultation drop05, verdict g073/f1 DISAGREE, is accepted after personally rereading the exact orphan
snapshot/admission/termination and add registration/adoption/setup source. Corrected source-level ordering:
known_sandboxes first captures list_hosts at :69-76, then awaits recipe rows at :77. The orphan task may remain unpolled
after that capture while a different shipped task registers a generated-name collision
(provisioning/tensorlake_add.rs:273-282), adopts the SAME canonical sandbox ID (:523-527), persists it (:578-580), and
completes setup/admission/hold release on a suitable sandbox (:480-503). Shipped user create_session resolves a
connected target and dispatches the session (sessions.rs:1496-1499,1620-1724; personally read in the prior synthesis,
independently rechecked by drop05). When the old orphan task resumes, its saved name set still excludes that host
(:175-178), and it sweeps the canonical ID (:196,226-241) without a fresh host ownership check or the first-add per-host
lock. This ordering can finish registration/setup/user work BEFORE the orphan resumes at all; it requires no favorable
vendor termination delay or measured vendor-latency premise. My earlier assertion that newly created work depended on
such an unresolved vendor-ordering premise was too strong and is superseded. Complete application interleaving with
actual work producers is source-established conditional on a generated-name collision and a sandbox suitable for setup.
Suggested bucket highest, confidence confirmed source-permitted race and destructive consequence; no dynamic
reproduction or observed incident asserted. Preserve qualifications: copied supervisor state can refuse setup in some
cases (:480-485), and no frequency, practical scheduling window or particular lost file is measured. Those conditions do
not refute the suitable-sandbox case or lower the established destructive boundary for rarity. Distinct from g071:l5's
standalone collision/adoption credential overwrite and from f9's retry name-reservation policy; same f1 root/history
retained. Fix needs shared ownership/admission exclusion or a revalidation bound to destruction so a formerly unowned
snapshot cannot remain deletion authority after adoption. possible_cover g071:l5 shares collision provenance but does
not replace this orphan race; parent consolidates as appropriate. proposed_drop false; no lowering or withdrawal. drop05
path: <private review run>/independent/drop05/verdicts.json, g073/f1 only; no judgment about its other records.
Unchanged assigned orphan implementation through pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. No
runtime/process/cloud experiment.

Recording judgment: Retain highest on drop05 DISAGREE; reconcile complete source-permitted task interleaving in original
reviewer, with no invented vendor delay prerequisite.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g073/f1`; supporting records
`g032/g073:f1`, `g071/c11`. The retained private log preserves earlier wording and its corrections. This record does not
claim runtime reproduction.
