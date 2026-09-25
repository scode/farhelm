# The redirect limit allows four hops, not five

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A release mirror that redirects five times is refused as "too many redirects".

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F27 / COR-REDIRECT-HOPS`, tagged **definite**. Anchors and title: `provisioning/payloads.rs:844`,
`provisioning/payloads.rs:815` — The redirect limit allows four hops, not the documented five

The release download client follows HTTP redirects (GitHub answers every asset URL with one), and its policy documents
"at most five hops" (payloads.rs:791). The pure decision function (payloads.rs:844) refuses when `hops >= 5`, where
`hops` is reqwest's `attempt.previous().len()` (payloads.rs:815). That list already includes the original URL — the
policy's own comment relies on `previous()[0]` being the URL farhelm asked for — so it has 1 entry at the first
redirect, and the fifth redirect sees 5 entries and is refused. Only four redirects are actually followed.

This is harmless for GitHub's single hop; it is a disagreement between the documented boundary and the behaviour.
Suggested change: use `> 5` (or change the documentation to four) and pin the boundary with a test.

User-visible consequence: a release mirror that redirects five times is refused as "too many redirects".
