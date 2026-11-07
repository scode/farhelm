# Account test does not refresh healthy orphan list

Reviewed commit: 8bd92e1336565dfc263fe5f3fa2fef413a82a87a

## TLDR

After another window changes the account key, Test alone does not refresh a healthy orphan list. Whether the
specification requires that refresh remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-ui/src/tensorlake/orphans.rs:95-101 at 8bd92e1336565dfc263fe5f3fa2fef413a82a87a`.

Source assessment (read with the later corrections below) (review record g074/f11, event 33):

Reviewed commit 8bd92e1336565dfc263fe5f3fa2fef413a82a87a. TLDR: a previously healthy orphan list stays unchanged when
the user presses Test after another tab saves a different key, although the specification names Test as a way for this
tab to observe that change. Whether this is a defect or wording intended only for failed discovery remains unverified
after investigation. Actual source results: tensorlake_settings.rs:100-103 keeps save/test revisions component-local;
another tab's save does not advance them. Successful Test :209-214 increments test_revision and passes it to OrphanList
at :302. OrphanList :97-100 reloads ONLY if its old error is Some; a previously healthy list therefore does not reload.
Backend tensorlake/mod.rs:252-266 does list with the currently saved key and returns only a count; Test endpoint
:401-424 publishes no fleet invalidation that would otherwise update the list. The Test can succeed using the
replacement key while this list still displays the previous successful discovery; no need to assume foreign-project
403/404 behavior. Governing SPEC_impl.md at pinned tip :6156-6158 says both 'A successful Test resumes failed discovery
with one reload' and 'Another tab’s key save is not observed automatically: this tab needs Refresh, Test or reopening
Settings.' The latter appears unconditional, whereas both component module docs (tensorlake_settings.rs:24-28) and
OrphanList :95-96 deliberately restrict Test reload to failed discovery. The earlier withdrawn-candidate event
interpreted the latter sentence contextually; that interpretation is not strong enough to erase the explicit Test
alternative, so this concern is retained with uncertainty rather than dropped. Missing premise is intended scope of that
specification sentence: whether Test must reconcile another tab's key save even if the local list was healthy, or only
release a known failed discovery. Readable code establishes the behavior but cannot settle that product-intent tension.
Settling evidence: maintainer/spec clarification; a narrow browser case with healthy list, external key replacement and
successful Test would verify actual stale UI behavior but not resolve normative intent on its own. Suggested bucket
other, stale-list/recovery convenience; no security or destructive project-key consequence, no claim of actual vendor
response status. Confidence confirmed no-reload code path, likely contract mismatch conditional on the explicit missing
intent premise. Proposed fix if the broader Test alternative is intended: reload once on every successful Test, not
merely when error was already Some; otherwise qualify the spec's final sentence. proposed_drop false; possible_cover
none among selected interfaces. Distinct from f4's cleanup-only HTTP-success error masking, and from supplied
project-key findings' missing persisted project identity/foreign-ID status premise. No dynamic reproduction or
unavailable external policy investigation performed. Discovery pass 3 now one novel source-verified finding f10 and one
investigated-unverified finding f11; lead f11 resolved into this qualified finding.

Later qualification (review record g074/f11, event 35):

Completion evidence for qualified f11, not another count. Reviewed behavior at 8bd92e1336565dfc263fe5f3fa2fef413a82a87a;
original c340166d754d3edb3de7fd538c6408948d3536c6:64-70 already restricted Test reload to error Some, and pinned tip
:95-101 is identical. Tip tensorlake_settings.rs:99-102 still stores local key/test revisions and :208-213 increments
only test_revision after a successful Test. Thus source behavior is confirmed unchanged: healthy orphan discovery does
not refresh from Test alone after another tab saves a key. TLDR: specification's stated Test recovery alternative for
another tab's key change may not match healthy-list behavior. Confidence confirmed source behavior but product-defect
judgment remains unverified after investigation, because SPEC_impl.md:6156-6158 names both failed-discovery-only
recovery and an apparently unconditional Refresh/Test/reopen alternative while code docs explicitly choose failed-only
behavior. Missing premise and settling evidence remain a specification/maintainer clarification, not an external gateway
policy or a vendor 403/404 outcome. Suggested bucket other; no highest claim, no destructive project-key retry route.
proposed_drop false; possible_cover none among supplied interfaces. No dynamic reproduction performed; normal successful
Test still reports the saved key's sandbox count correctly and failed discovery is resumed as specified. Final discovery
pass 3 totals: one novel source-verified ID f10, one novel investigated-unverified ID f11, no new exact duplicates, no
unresolved lead-kind records. The uncertainty is preserved inside f11 rather than left as a lead.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g074/f11`; supporting records
`g060/f17`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
