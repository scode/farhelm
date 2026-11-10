# Orphan cleanup promises a provisioning lock it does not hold

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

The implementation specification promises that orphan cleanup checks ownership under the host provisioning lock, but the
endpoint does not hold that lock. No supported destructive adoption race was established.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `a7d3462accd92f85b98b0bb57fdf962be3395f2e`. Original source location: `SPEC_impl.md:6868-6869`.
Locations below use that revision unless another is named; the assessment was checked through the reviewed tip.

The new implementation specification promises an ownership recheck under the host provisioning lock, but the orphan
deletion endpoint takes no such lock. The promise was introduced in a7d3462accd92f85b98b0bb57fdf962be3395f2e; its parent
did not make it. Retain this documentation/implementation mismatch rather than treating it as pre-existing prose.

The stronger newly-owned-work destruction theory was investigated and narrowed with independent agreement. New host rows
reserve their names before billing; an existing same-name sandbox cannot be adopted without a saved ID or pending-create
proof (tensorlake_add.rs:1285-1292). Orphan deletion retains its captured canonical sandbox ID (orphans.rs:233-245).
Those paths do not establish an ordinary new admission acquiring that resource after the orphan check. The verdict does
not cover arbitrary recipe/build or other-helm routes. Correct the stated locking contract or supply its intended lock
without inventing a proven destructive race. Suggested bucket other, confirmed mismatch; g048-006 and g050-L3 are
duplicates.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g049/g049-004`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
