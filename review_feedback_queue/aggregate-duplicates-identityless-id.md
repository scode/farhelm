# Identity-less live rows are not deduplicated against cached ones

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In an unusual setup, one session shows twice and neither copy can be operated on.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F23 / COR-AGGREGATE-DUP-LIVE-ID`, tagged **possible**. Anchors and title: `aggregate.rs:567` — An identity-less host's
in-memory sessions are not deduplicated against cached claims

The merged session list (`aggregate.rs`) first adds every cached row from `helm.db`, then appends each identity-less
host's in-memory rows (`aggregate.rs:562-570`). There is no check that an in-memory row's id is not already in the list.
If an identity-less host reports an id that another host's cache already holds, the session appears twice and is counted
twice in `total`. Routing correctly refuses both copies, because `resolve_owner` sees the cached owner and the live
owner disagree. The display is still wrong: SPEC_impl (line 1990) says the first claim holds and the later claimant's
row is dropped "so the LIST stays coherent". That rule is enforced for cached rows inside `replace_host_sessions`, but
not on this in-memory path.

Suggested fix: when appending in-memory rows, skip ids already present from the cached rows (and from earlier
identity-less hosts).

User-visible consequence: in an unusual setup, one session shows twice, and neither copy can be operated on.
