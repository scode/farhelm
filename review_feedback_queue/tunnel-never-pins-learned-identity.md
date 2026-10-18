# A tunnel never pins an identity learned after it opens

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

A tunnel opened before its host reports an installation identity can later forward new browser connections to a
replacement installation. The routing mechanism is established; whether this violates the intended tunnel policy, and
whether an ordinary shipped installation can reach the starting state, remain unresolved.

## Details

`crates/farhelm-helm/src/tunnels.rs:218–223` compares installation identities only when the original listener claim
contained one. The entry and accept loop retain that original claim (`:251–258,346–390`), and reusing the listener does
not update it. Open a tunnel for host H/session S with no identity, reconnect H as installation A with S, use the
existing tunnel, then adopt B advertising S: new connections to the old port pass the identity predicate and resolve B.
A listener originally opened with A would reject B.

Destination edits (`hosts.rs:876–935`; `manager.rs:1491–1692`), adoption (`hosts.rs:1095–1110`; `manager.rs:2555–2661`),
and first identity acquisition (`manager.rs:3737–3821`) neither retire nor update these listeners. Retiring the old
supervisor connection ends its already accepted streams, but does not prevent the listener from admitting a later socket
against the replacement connection.

Suggested bucket: provisionally **highest**, because the unresolved policy question concerns delivery to another
installation. Confidence: **likely**, with the predicate and cross-module lifetime confirmed statically. No runtime
reproduction or ordinary shipped SSH trigger was established. `manager.rs:3733–3740` identifies honest identity-less
peers as test or embedding constructions, not older released supervisors. The replacement must advertise the same
session ID. Ordinary stable-identity reconnects are protected.

Possible cover: SPEC.md:196–205 and the ledger's `stop-approval-retain-targets-owner.md` allow confirmed operations to
follow a logical host across adoption. SPEC_impl.md:1562–1566 specifically requires a tunnel's original host and any
known installation identity to match. Whether “known” includes an identity learned during the listener's lifetime is
ambiguous. The launcher-default identity-less allowance in SPEC.md:763–768 concerns a different feature. The ledger's
`hostnotfound-refresh-keeps-serving.md` also involved an identity-less peer, but its rare host-row deletion and stale
connection consequence do not cover this listener. No exact Planned, BUGS or existing queue coverage was found.

Proposed drop: the producer proposed a conditional policy-based drop; an independent fresh-context Astra/high reviewer
rejected it in disposition review 2. Retain the ambiguity for triage. If lifetime identity binding is intended, pin the
first learned identity or retire the listener when continuity cannot be established.

Provenance: `helm-streams-and-files-1/f1`, latest `turn2.result.md` and `findings.jsonl:3`, corroborated by
`helm-host-lifecycle-1/f6` in its latest `turn2.result.md`; independent `drop-review-2/verdict.md` KEEP. This draft
consolidates that evidence and adds no source-review coverage.
