# The all-phase fixture does not compare the manager's phase strings

Reviewed commit: 019741b9a5a1b6dee57369aadf6ff66ab48d973a

## TLDR

The strengthened fixture catches client/wire phase drift. A manager-only rename for a phase can still pass this fixture.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/hosts.rs:1689`.

crates/farhelm-proto/src/ui_channel.rs:983–987 and :1114–1128 specify one vocabulary for the manager accessor, client
view accessor and wire tag. SPEC_impl.md:5370–5385 describes the manager phases. The new Stopped and Deleting cases in
hosts.rs:1683–1694 compare a client view with serialized JSON. Neither creates a manager state. The same limitation
applies to the rest of this table at :1620–1682. There is separate partial coverage: the Tensorlake transition assertion
at :1702–1715 compares a manager state with its view, and the existing unreachable case at :2067–2078 checks the manager
state and returned tag against the same literal. This finding is about the all-phase fixture, not the absence of manager
tests elsewhere or a current mismatch. An all-phase fixture could start from manager states, project through
hosts.rs:72–174, and compare the three strings. Bucket: other, maintainer test coverage. Confidence: confirmed by pinned
test assertions. Possible coverage: trailing-host-phase-vocabulary-test-omits-stopped.md, named in the reviewed commit.
Individual scenario assertions cover some phases.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `status/f3`.
