# Filter escape test accepts earlier query

Reviewed commit: a657a1aa9e26c103704196819f2d6056dad655dc

## TLDR

The removed-host filter test can credit an earlier unfiltered request instead of proving that clearing the filter sent a
new one.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/tests/filters.spec.ts:169 at a657a1aa9e26c103704196819f2d6056dad655dc`.

Source assessment (read with the later corrections below) (review record g015/l7, event 12):

Reviewed commit: a657a1aa9e26c103704196819f2d6056dad655dc. TLDR: the test can claim clearing a removed-host filter
fetched all hosts without proving that the clear action sent that query. Details: watchListingReads at lines 26-34
appends every session GET to an array. The removed-host case installs it at line 152, before selecting the remote host
and its feed-driven refresh, then after chip.press(Space) at line 165 asserts reads.some(no host query) at line 169 with
no baseline or clearing. An earlier unfiltered request observed during startup/feed refresh can satisfy this check. The
other assertions prove focus, selected control and chip disappearance, not the server request, which the file's own
module comment expressly distinguishes from client-only filtering. Save a read count or clear the array immediately
before the escape, then require a new unfiltered request and its accepted result. The report also notes the long-alias
case at lines 221-228 does not assert the remote-empty fixture before selecting it (unlike lines 104 and 142); its later
message check is fail-closed, so that narrower issue is diagnostic quality rather than another product finding.
Suggested bucket other, confidence confirmed for the unscoped assertion; no highest route. proposed_drop: false.
possible_cover: host-filter browser test interface; parent checks coverage. Caveat: not evidence that the shipped clear
button fails, and not a claim that every run observes an earlier unfiltered request.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g015/l7`; supporting records
`g090/d7`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
