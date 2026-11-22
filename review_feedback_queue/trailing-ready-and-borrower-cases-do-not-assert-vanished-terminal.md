# Ready and borrower cases do not assert that the vanished terminal becomes Exited

Reviewed commit: 85cf6b2e7715c5dce446bcc95170662c1fbc8bb8

## TLDR

The test documents ordinary exit for a stopped Ready or borrower session, but can pass if the observer proposes nothing
and reload keeps the old Running outcome.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core.rs:37232`.

core.rs:37099-37109 states the intended classification; :37163-37208 validates an accepted, initially running terminal,
removes the unconsumed spec, establishes no sentinel and confirms the terminal host ended. In the Ready and borrower
cases the observer checks only that sentinel.is_some is false (:37209-37216), though status.rs:571-591,684-696 and
:435-475 propose ObservedExit for an absent terminal and an owned Running row. SPEC_impl.md:2234-2241 says a confirmed
absent host is reconciled as a vanished terminal. Same-boot reload likewise proposes ObservedExit with no exit code for
that Running row (core.rs:6889-6962,6989-7020). Yet :37231-37240 asserts only not Error and equality with storage. A
skipped reconciliation can thus leave both projections Running. Assert the observer's exit proposal and the reloaded and
stored ordinary Exited result, without inventing an exit code for the absent host. This is separate from f16's
checkout-membership premise and f7's origin runtime handoff. Bucket: other. Confidence: source-confirmed test-result
gap. The reviewed production code still performs the transitions. Possible coverage: recovery for general vanished-host
classification. Exact coverage of these two preparation cases unverified.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_unit/f18`.
