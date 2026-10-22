# Changed-host warning flattens compared identities

Reviewed commit: be353c3b315956e716ea461871cc5b80e6d1d0a0

## TLDR

Different installation identities can look identical in the changed-host warning, and its values lack separate direction
isolation. Harmful reliance on that warning remains unestablished.

## Details

Suggested bucket: provisionally **highest** for an unresolved security or wrong-target decision; the established
diagnostic ambiguity alone is **other**. Confidence: **confirmed rendering mismatch; harmful reliance is a missing
causal source premise**. Browser reordering, occurrence frequency and actual harm were not measured.

`crates/farhelm-ui/src/provisioning.rs:382–403` combines raw identities into a string; `3503–3507` renders it as one
ordinary peer part. `peer.rs:159–180` can render actual U+202E and literal `<U+202E>` identically. The distinct identity
formatter at `204–232` avoids that collision; separate peer runs at `235–246,304–323` supply per-value isolation.
Combining the sentence loses both properties required by `SPEC.md:3144–3148` at the pinned tip.

The warning contains recorded registry identity, not an unaccepted mismatch identity. Adoption separately displays and
submits captured reported bytes; retry consumes a raw request and checks its binding
(`hosts.rs:1486–1516,3207–3210,3668–3705`; `provisioning.rs:2670–2687,2719–2723`). Observed mismatches and frozen
identity decisions are refused by the backend. These checks refute direct implicit adoption from the warning. They do
not prove every retry verifies identity: protocol-skewed or absent-supervisor paths deliberately proceed with incomplete
evidence (`crates/farhelm-helm/src/provisioning/service.rs:1050–1078,1796–1824`). The warning itself recommends checking
the host version and updating again (`provisioning.rs:410–411`).

Independent auditor `g139` found no concrete harmful authorization caused by the warning despite the separate evidence,
and could not exclude that reliance generally. Its causal investigation stopped incomplete. Possible cover: the
rare-diagnostic filter covers only the diagnostic-only branch. Existing identity decisions address other surfaces, and
the destination-whitespace item has different values and renderers. Proposed drop: none for the conditional concern.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g086/f1`, independently checked by `g139`.
