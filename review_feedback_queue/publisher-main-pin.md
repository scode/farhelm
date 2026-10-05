# Publishing from an older checkout may stop retaining main’s pinned images

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Publishing from an older checkout may stop retaining main’s pinned images.

## Details

`F48 / COR-PUBLISHER-MAIN-PIN` — **possible** — `scripts/publish-docs-shots.sh:314` — Publishing from an older checkout
may stop retaining main’s pinned images

Published docs images live in snapshot commits retained by a dedicated Git reference. Older snapshots can be discarded
six weeks after replacement, but the snapshot referenced by main's image manifest must always remain available. The
publisher's exemption comes from the manifest in its local checkout; it fetches the remote image keeper, but does not
read main's current manifest before deciding which older snapshots to retain.

If publication runs from an older checkout, the local exemption can differ from the snapshot main still references. Once
that snapshot's replacement is older than six weeks, a later publication could omit it from the keeper. GitHub could
then collect the images even though the published docs still use them. This requires an aged publication sequence that
was not reproduced; losing reachability does not itself prove immediate image loss. Read and retain main's fetched pin,
or enforce a current-main publication premise before pruning.

The proposed bucket is highest if published assets count as user work; otherwise it is other. Possible cover: the
screenshot SPEC requires capture from a checkout based on latest main and prescribes capture, review, then publication.
It does not clearly settle freshness at the later publication step.

Restater note: The input records an independent D48 assessment of ambiguity. The source uses only the local pin, but
whether the supported publication procedure permits the stale-checkout sequence needs the coordinator's verdict.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `auto_general p1`.

Possible cover recorded during collection: docs/docs-shots/SPEC latestmain capture requirement;
immediatepublicationfreshness unclear..

Collection caveats: Agedpublicationsequence unverified, suggestedhighest ifpublishedassets userwork/otherwiseother;
independentD48 ambiguousrecord.

Coordinator confirmation: independent audit D48 found the latest-main capture requirement did not establish freshness at
publication. Keep the stale-pin sequence possible.

## Filed reviewer metadata

- `auto_general p1`: confidence as filed: possible; premise is publishing from an old checkout while remote main pins an
  older retained snapshot. Suggested bucket: highest if published image assets count as user-owned work; otherwise
  other. Severity: asset loss. Suggested bucket as filed: highest if published image assets count as user-owned work;
  otherwise other. Severity: asset loss.
