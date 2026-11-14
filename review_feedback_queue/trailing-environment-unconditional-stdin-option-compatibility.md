# Unconditional environment idle limit is specified

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: considered whether replacing the Tensorlake helper capability probe with an unconditional
idle-limit option prevents an old sandbox helper from connecting. Source: transport.rs:191-201 now obtains stdio argv
from HostAccess; access.rs:25-33 and 145-157 always append --stdin-idle-limit-secs 120 for Tensorlake and Podman routes.
The parent ssh.rs:460-481 probed --help and omitted this option for older helpers. Contract acceptance: SPEC_impl.md,
One route for transport and provisioning, at this commit explicitly says: 'All environment stdio routes pass
`--stdin-idle-limit-secs 120` unconditionally and emit the existing 30-second heartbeat. The supported environment
helper has that option.' Suggested bucket: high for incompatibility with an affected old helper; confidence: confirmed
intentional contract change, no shipped unsupported helper route established; proposed_drop: true, exact specification
acceptance above; possible_cover: none, coverage sources not consulted. Could-be-highest: no, established potential
consequence is connection unavailability rather than lost work or credentials. Caveat: this is specification acceptance,
not execution evidence that all deployed helpers implement the option. No execution performed.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g056/c2`.

### g056/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 2 (candidate, open):

Reviewed commit: 1038bb1f4549f6896fa6e064fecf540f4548b4da. TLDR: considered whether replacing the Tensorlake helper
capability probe with an unconditional idle-limit option prevents an old sandbox helper from connecting. Source:
transport.rs:191-201 now obtains stdio argv from HostAccess; access.rs:25-33 and 145-157 always append
--stdin-idle-limit-secs 120 for Tensorlake and Podman routes. The parent ssh.rs:460-481 probed --help and omitted this
option for older helpers. Contract acceptance: SPEC_impl.md, One route for transport and provisioning, at this commit
explicitly says: 'All environment stdio routes pass `--stdin-idle-limit-secs 120` unconditionally and emit the existing
30-second heartbeat. The supported environment helper has that option.' Suggested bucket: high for incompatibility with
an affected old helper; confidence: confirmed intentional contract change, no shipped unsupported helper route
established; proposed_drop: true, exact specification acceptance above; possible_cover: none, coverage sources not
consulted. Could-be-highest: no, established potential consequence is connection unavailability rather than lost work or
credentials. Caveat: this is specification acceptance, not execution evidence that all deployed helpers implement the
option. No execution performed.

Returned proposal (claims-035, event 2): **duplicate**, suggested bucket **other**.

Exact same old-helper option-compatibility hypothesis and specification acceptance as g055/l1; consolidate its caveat
there.

Original stronger consequence: An old sandbox helper could reject the new unconditional option before hello, causing
high connection unavailability; no user-work-loss or security consequence established. Reviewer proposed explicit
specification acceptance.

Unresolved premise or limitation: No supported helper lacking the option established; this does not prove all deployed
helper binaries implement it.

Proposed basis: Exact alias of g055/l1 and the same explicit specification acceptance; preserve its deployment-evidence
caveat.

Existing parent disposition (event 2): duplicate. Parent read both complete claims-035 entries and accepts the exact
same old-helper/no-option hypothesis as g055/l1. Reviewed1038bb1 transport.rs:190-201 and5665983 access.rs:30-47,234-247
invoke the environment option; reviewed/current SPEC_impl.md explicitly states it shipped with Tensorlake support and
every supported environment helper has it. Canonicalg055/l1 already has the narrow specification-acceptance decision.
Preserve hypothetical old-helper connection failure, deployment-evidence caveat and absence of executed helper
validation. This says nothing about actual deployed binary inventories, remote orphan lifetime, missing provisioning
watchdog invocation g055/l5 or deadline exit flushing. No highest effect was alleged.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g056 record](records/review-through-00de0b6c/groups/g056.json). Record ID: `c2`.
