# Orphan cleanup overlay forgets failed discovery

Reviewed commit: c340166d754d3edb3de7fd538c6408948d3536c6

## TLDR

When discovery fails while cleanup remains unfinished, earlier orphan rows disappear and unrelated fleet updates keep
retrying the failed read.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: high.

Source location: `crates/farhelm-helm/src/tensorlake/orphans.rs:86-105 at c340166d754d3edb3de7fd538c6408948d3536c6`.

Source assessment (read with the later corrections below) (review record g041/l3, event 21):

Synthesis rechecked at pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b, retaining reviewed
c340166d754d3edb3de7fd538c6408948d3536c6 backend and 8bd92e1336565dfc263fe5f3fa2fef413a82a87a UI. TLDR: a vendor list
failure while any explicit cleanup remains causes other previously listed sandboxes to disappear and fleet notices keep
retrying the failed discovery. Backend :94-105 returns Ok(unfinished) for the failed vendor read and rewrites each
cleanup row state with the visible failure; endpoint :263-265 sends that as normal JSON success. UI orphans.rs:68-86
sees Ok, waits for local time bridge, replaces rows with only cleanup rows, clears error and advances checked_at; feed
:102-105 gates retries only on error None. Exact spec consequences at pinned tip: SPEC.md:393-400 says failed reads show
reason and keep earlier rows. SPEC_impl.md:6153-6158 says failed discovery pauses automatic reads, unrelated fleet
changes must not resend rejected key; :6160-6166 says failed reads keep earlier rows and existing error remains during
explicit recovery. Returning partial success preserves unfinished cleanup visibility, which is intended, but does not
satisfy the separate discovery error contract. Qualification: an error IS visible inside each cleanup row; no false None
found message is displayed because cleanup rows are nonempty (UI :137-139). checked_at is incorrectly advanced
internally but not shown as an empty-success label on this branch. Repeated requests require unrelated fleet notices,
not a standalone timer; no automatic destructive cleanup is claimed. Suggested bucket high for hiding still-billing
resources and persistent failed cloud reads in normal Settings operation; no established highest consequence. Confidence
confirmed source mechanism, no runtime/cloud experiment. proposed_drop false; possible_cover adjacent orphan API/UI
owners, parent coverage not searched. Suggested fix: carry discovery failure/status separately alongside cleanup
overlays, retaining prior discovery rows and pause state while still exposing cleanup retries. No repair in the pinned
source.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g041/l3`; supporting records
`g073/f5`, `g074/f4`, `g040/g041:l3`, `g049/iface_g073_f5`. The retained private log preserves earlier wording and its
corrections. This record does not claim runtime reproduction.
