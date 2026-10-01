# One hostile host can hide every other host's sessions

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

A compromised remote host can make every other host's sessions disappear from the sidebar (and from agents' session
listings) by reporting 500 sessions that sort first, leaving only a generic "could not read to the end" notice.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F58 / SEC-LIST-CROWDING`, tagged **definite**. Anchor and title: `crates/farhelm-helm/src/aggregate.rs:387` —
a remote host can push every other host's sessions out of the merged session list.

The sidebar's session list is built by the helm, not by each host: the helm merges every host's cached sessions, applies
any filter, sorts them, and cuts the result to 500 rows (`assemble` in `crates/farhelm-helm/src/aggregate.rs`, cut at
lines 387-388). The same merged list answers the agent-facing session listing. When a host's session list comes in, the
helm checks it in `drain_sessions` (`crates/farhelm-helm/src/manager.rs:731-765`) only for three things: at most 500
rows, session ids within a size cap, and no duplicate ids. It does not check any of the timestamps or titles that drive
the sort.

So a compromised or hostile supervisor can claim the top 500 positions under every sort the sidebar offers: with
creation times in the future for the default newest-first order (`sort_rows`, line 332), with sessions reported as
running and the latest work-start time for the activity order, or with titles that sort first for the title order. Every
other host's sessions then fall past the 500-row cut. The user sees only a generic "could not read to the end" notice,
and the other hosts' sessions reappear only when filtering by host, because the filter is applied before the cut.

SPEC.md ("Remote input, session defaults, and availability") says that failures or malicious behavior from a remote host
must not disrupt unrelated hosts or ordinary helm/GUI controls. Here one hostile supervisor hides every other host's
sessions from the normal sidebar and from agents' listings for as long as it keeps answering. This is not the tracked
`list-ingress-id-validation-gap.md` (which is about what session ids may contain), nor the accepted rule about two hosts
claiming the same session id.

The suggested fix is to bound each host's share of the merged list (a per-host fair share before merging, or always
keeping each host's newest rows and cutting only the remainder), and/or to clamp creation and work-start/activity times
at ingress in `drain_sessions` so none is later than the time the helm received the list. Add a test in which one host
lists 500 future-dated rows and another host's sessions still appear under each sort.
