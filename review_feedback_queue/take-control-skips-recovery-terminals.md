# Take control skips terminals waiting for reconnection

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Taking control in a new tab during a build mismatch can leave existing terminals in Recovery until Reconnect now is
used.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The inspected conditional route skips retained controllers that are already waiting for manual reconnection. Excluding
the same-session target does not prove the entire terminal set reattached. This is pre-existing source behavior with
ambiguous reconnect policy; no session process or terminal content is lost.

Source locations: `crates/farhelm-ui/assets/terminal.js:3076-3095,2915-2938`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Owner a065/f1, reviewed d6645f771bee8ed03a66486ef8755834937d759d. Preserve the previous detailed conditional mechanism,
producer, tests and filter caveats. Provenance check: this is PRE-EXISTING at exclusive base
da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c, not introduced by the assigned late patches. Base terminal.js:3050-3089
already documents full-desired-set reclaim and iterates islands/pendings plus tombstones without retiring reconnects;
base symbol search locates the controller skip at 2927. Current discovery confirms partial reattachment under
manual-only capability, with the separate Reconnect now remedy. reconnect.rs:153-163 explicitly documents that manual
per-terminal remedy; it does not say Take control may omit other wanted terminals. Suggested bucket other; confidence
source-confirmed conditional behavior, no runtime reproduction. proposed_drop:false; possible_cover remains ambiguous
diagnostic/retry filtering and partial old-release-upgrade filtering, not blanket acceptance. No highest consequence
established. No additional defect found in the checked host-refusal dispatch, upload-probe/abort or capture cleanup
branches.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a065/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
