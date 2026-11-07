# Redirect regression omits location header

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246

## TLDR

The redirect regression does not supply a redirect destination, so it cannot prove that Farhelm refuses to follow one.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/tests.rs:359 at da24fb6822d289a144e18677cdd248cc35093246`.

Source assessment (read with the later corrections below) (review record g051/l1, event 11):

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246. TLDR: The test guarding Tensorlake's no-redirect policy can
pass if that policy is accidentally removed. Exact references corrected against numbered pinned source: Reply at
tests.rs:49-53 only models status/content_type/body; Stub::start at 118-123 emits Content-Type and no Location.
failures_are_classified_with_the_servers_message scripts the 307 at 359-363, asserts the 307 at 405-411, and claims no
redirect was followed at 413-414 by counting nine requests. A client allowed to follow redirects also stops at a 307
without Location, so these observations cannot distinguish the policies. Supplied test-authoring contract requires an
observable distinguishing competing mechanisms. Production http.rs:128-133 explicitly applies Policy::none(); no shipped
redirect or credential exposure bug is established. Fix: allow a Location header pointing at a separate route on the
owned stub; assert that route is not called and the original 307 is returned. Removing Policy::none() should then fail
the test. Suggested bucket other, confined to regression coverage. Confidence confirmed by source inspection; no runtime
execution permitted. proposed_drop false; possible_cover unknown (parent handles coverage). Temporal check: complete
path-limited diff of tests.rs/http.rs from reviewed commit to 8a8c9579e04299410652f0e4681975a7198dfc7b shows no changes
to the fixture or this test, and no http.rs changes. The gap remains at the pinned tip. Caveat: the production redirect
policy itself is correct.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g051/l1`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
