# Abbreviated IPv4 short labels lose address components

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A valid abbreviated IPv4 destination can lose address components in its short label; whether such forms count as IP
literals under the display contract is unresolved.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Primary OpenSSH/libc source establishes a supported resolution route. The short-label helper's parsing then treats the
text like a dotted hostname. Operation identity and dispatch are unchanged. Product wording about IP addresses/literals
leaves the display-policy scope ambiguous; no wrong-host action is claimed.

Source locations: `crates/farhelm-proto/src/host_name.rs:39-48`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full commit d6645f771bee8ed03a66486ef8755834937d759d; introduced by 993cd95e8af93f50bbf5797e299409397ed5010b.
TLDR: an unaliased host registered using an SSH-supported IPv4 shorthand such as 10.1 receives the short caption 10
instead of retaining its address spelling. This is confirmed from source, not runtime observation. Farhelm's registry
admits the spelling (store.rs:1824-1825) and passes it unchanged to SSH (ssh.rs:209-223). Rust 1.90.0
library/core/src/net/parser.rs:160-174 requires four decimal IPv4 components and rejects zero-prefixed components
(109-170); FromStr<IpAddr> uses this parser (244-246,295-320). Hence Farhelm's strict IpAddr check fails and
host_name.rs:46-48 chooses the first dotted component. Supported OpenSSH Portable V_9_6_P1 ssh.c:295-311 classifies
digit/dot inputs as addresses; 1226-1235 canonicalizes them regardless of CanonicalizeHostname; 432-445 and 342-350 use
getaddrinfo(AI_NUMERICHOST, SOCK_STREAM). glibc 2.39 nss/getaddrinfo.c:881-910 and 1168-1171 accept __inet_aton_exact
numeric results before any DNS lookup. resolv/inet_addr.c:106-193 explicitly accepts a.b with a 24-bit final component
and a.b.c with a 16-bit final component, so 10.1 means 10.0.0.1. glibc getaddrinfo.c:388-429,1052-1127 produces a single
TCP sockaddr for this request; OpenSSH ssh.c:1634-1636 passes it to ssh_connect, and sshconnect.c:447-508,545-553 uses
the returned sockaddr for the direct connection. No external parsing or scheduling premise remains. Suggested bucket:
other. The effect is limited to short display text: current consumers still select/resolve by registry ID and retain
full names in tooltips; no security, wrong-action-target or user-work-loss route is established. Confidence: confirmed
behavior and supported source route; specification compliance remains ambiguous. possible_cover: SPEC.md:216-222
promises whole IP addresses but may have intended canonical literals only; plans/queue/host-short-names.md:130-132
explicitly selects core::net parsing without discussing shortened or octal numeric forms. This is ambiguous, not exact
accepted coverage. proposed_drop: null. Coverage searches across SPEC.md, SPEC_impl.md, empty TODO Planned, all
plans/queue, BUGS.md, INDEX.md and TRIAGE_OUTCOMES.md found no exact acceptance or planned fix; repeated
numeric-shorthand searches in this synthesis pass found no matches. Remaining question is solely product-contract scope.
Primary release-tag source snapshots and SHA-256 digests are retained in this group's scratch; URLs and exact ranges are
in progress.md. No tests, resolver/SSH experiments or live-install actions were performed. Earlier f1 records claiming
the external source premise was unresolved are superseded by this source investigation, not erased.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g143/f1`, `a027/f4`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
