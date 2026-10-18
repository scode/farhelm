# Shortcut approval can succeed after the binding disappears

Reviewed commit: b375716cc08ae5d14931d4f7b56264b3e169cdf4

## TLDR

An agent can request a shortcut that is already assigned, receive success after approval, and find that another client
moved it while the card was waiting.

## Details

When the requested bolt mark or number key already belongs to the template, the resolver treats that binding as
unchanged (`crates/farhelm-helm/src/agent_requests.rs:2206–2215,2240–2250`). The pending approval then uses only a
template precondition (`agent_requests.rs:2393–2410,2419–2428`). A concurrent client can move the binding without
invalidating that precondition, and the agent receives success even though the requested assignment is absent
(`agent_requests.rs:2429–2445`). Keep the requested binding as a post-approval condition even when it was initially
satisfied.

Suggested bucket: other, for a false success reply. Confidence: confirmed by static ordering; the concurrent sequence
was not reproduced. Provenance: trailing review of the stated commit. `SPEC.md` and `SPEC_impl.md` discuss binding
checks for requests that change a binding, but do not expressly accept success when an initially satisfied request stops
being satisfied during approval. The rare-glitch filter excludes success reported for an operation that failed.
