# A remote host can block other hosts' sessions by listing their ids

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

One compromised remote machine can make every session on your other machines refuse to open, stop, take input or be
deleted ("ambiguous owner") until you remove that machine.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F28 / SEC-CONTESTED-ID-DOS`, tagged **possible**. Anchors and title: `sessions.rs:733-749`, `store.rs:4490`,
`manager.rs:2266` — Any remote host can make other hosts' sessions unusable by listing their ids

Suppose host B's refresh lists a session id that host A's cache already holds. `replace_host_sessions` drops B's row and
reports the id as **contested** (`store.rs:4490`), and B's actor publishes that set. `resolve_owner`
(`sessions.rs:733-749`) checks `contested_claimants` (`manager.rs:2266`). As long as any host other than the cached
owner is contesting the id, it refuses every operation on that session with `SessionOwnerAmbiguous`. That covers stop,
restart, rename, delete, replace, terminal attach, uploads, mark-seen and even the detail read.

B can learn every session id in the fleet. The agent relay's `sessions` verb returns a fleet-wide listing, and a hostile
supervisor can send that upcall itself on behalf of one of its sessions. An identity-less B achieves the same through
its in-memory list, because `resolve_owner` refuses when the cached owner and the live owner differ. After A's cache is
purged (by remove-and-re-add or by adoption), B can even list A's ids before A's first refresh and become the cached
owner itself.

SPEC.md ("Remote input, session defaults, and availability") says malicious behaviour from a remote host "must not
disrupt unrelated hosts or ordinary helm/GUI controls". SPEC_impl (line 1990) deliberately chose "while both hosts keep
reporting the id, ROUTING fails closed naming both", but does not reconcile that with the availability rule. This is a
spec conflict to put to the maintainer, who may consider it accepted.

Suggested fix: when the cached owner is connected, has a verified identity, and its own latest drain still lists the id,
route to the owner and flag the contesting host. Fail closed only when the owner no longer lists the id or has no
verified identity. Alternatively, record the tradeoff explicitly in SPEC.md.

User-visible consequence: one compromised remote machine can make every session on your other machines refuse to open,
stop, take input or be deleted ("ambiguous owner") until you remove that machine.
