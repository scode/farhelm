# The redirect hop bound enforces 4 hops while the policy documents 5

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

No user-visible impact today (real download chains are one hop), but the documented security rule for release downloads
says five redirects while the code allows four — the next reader cannot tell which is intended.

## Details

Source: pre-pr-review-swarm, area provisioning, 2026-09-17. Confidence: definite (correctness-data-flow p1). Coordinator
confirmed the counting against reqwest semantics and the module's own comment.

The release-download redirect policy documents "at most five hops" (payloads.rs:791), on the rationale that "a chain
longer than five is a loop or a game, not a CDN". The decision function refuses at `hops >= 5` (844), where `hops` is
`attempt.previous().len()` (815) — and reqwest's `previous()` includes the originally requested URL (the module's own
comment says `previous()[0]` is the URL farhelm asked for, 809-811; verified against the vendored reqwest source: "The
first URL in the previous is the initial URL and not a redirection"). So the 5th redirect arrives with length 5 and is
refused: at most 4 hops are ever followed.

Suggested fix: move the code (`hops > 5`, recommended — the doc states the security rationale) or correct the doc to
four. Either way, pin the agreed number with a unit test on `release_redirect_decision`, which exists precisely so the
rule is testable.
