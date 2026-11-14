# Creating-verb test documentation still requires a host for every clone

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: the supervisor's request-validation test documentation tells maintainers that every clone must
name a host even though the same change deliberately allows environment clones to omit it. This can misdirect
maintenance of the request boundary and its tests. This is a documentation finding, not a claim that an ordinary clone
silently selects a destination. Exact scope: handlers.rs:7350-7365 describes the two creating verbs' supervisor-doorway
contract universally, explicitly says at 7352 '`Clone` needs a host', and explains at 7358-7360 that 'this doorway
bounds what an agent can push through the two byte-unbounded queues'. It has no ordinary-source-only qualification. That
cannot describe the helm's later source-dependent decision, since this supervisor validator has no source-host
knowledge. The assigned patch changes Clone from HostSelector::Required to host.as_deref at 3924-3929 and removes the
blanket Required(None) rejection from validate_create_fields at 4056-4076. Consequently a shaped clone with source Some
and host None now passes this doorway, as required for environment clones. The adjacent test at 7388-7397 rejects both
source None and host None; missing-source validation explains that assertion and does not prove the universal host
statement. Source/contract refutation of the original runtime concern remains: helm agent_requests.rs:3863-3885 resolves
the source, branches session-owned sources into environment cloning, and rejects every other missing-host clone before
dispatch. Its environment branch at 4313-4335 refuses explicit host/cwd/key overrides and derives

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g122/c1`.

### g122/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435; checked through pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: the supervisor's request-validation test documentation tells maintainers
that every clone must name a host even though the same change deliberately allows environment clones to omit it. This
can misdirect maintenance of the request boundary and its tests. This is a documentation finding, not a claim that an
ordinary clone silently selects a destination. Exact scope: handlers.rs:7350-7365 describes the two creating verbs'
supervisor-doorway contract universally, explicitly says at 7352 '`Clone` needs a host', and explains at 7358-7360 that
'this doorway bounds what an agent can push through the two byte-unbounded queues'. It has no ordinary-source-only
qualification. That cannot describe the helm's later source-dependent decision, since this supervisor validator has no
source-host knowledge. The assigned patch changes Clone from HostSelector::Required to host.as_deref at 3924-3929 and
removes the blanket Required(None) rejection from validate_create_fields at 4056-4076. Consequently a shaped clone with
source Some and host None now passes this doorway, as required for environment clones. The adjacent test at 7388-7397
rejects both source None and host None; missing-source validation explains that assertion and does not prove the
universal host statement. Source/contract refutation of the original runtime concern remains: helm
agent_requests.rs:3863-3885 resolves the source, branches session-owned sources into environment cloning, and rejects
every other missing-host clone before dispatch. Its environment branch at 4313-4335 refuses explicit host/cwd/key
overrides and derives recipe/place from the source host row. SPEC.md, Agents and environments, 4327-4328 says
'`farhelm agent clone` of a sandbox or container session takes no `--host` or `--cwd`; it targets the source's recipe at
the source's place, with the source's launch and repository intent, never a copy of its disk.' That accepted production
behavior exposes the distinct false documentation; it does not refute it. Attribution: the range base
6e423e30068b62c6cfa276c6d720ee35bb70387a retains the same comment at handlers.rs:7321-7336 while the required-host
selector still exists; this in-range commit removes that prerequisite without updating the comment. The pinned tip still
contains the universal statement at 7191, with the same doorway scope at 7197-7204. Suggested fix: document optional
host admission at the supervisor and the helm's ordinary-source host requirement, or limit the statement explicitly to
ordinary-source resolution. Suggested bucket: other (documentation correctness). Confidence: confirmed
pinned-source/contract comparison, no execution. proposed_drop: false for this documentation defect; the earlier
low-substance dismissal is superseded. Original considered unintended-destination consequence remains source-refuted as
above, and is not asserted as a production defect. Could-be-highest: false for the established documentation
discrepancy; no security or loss-of-work route established. possible_cover: parent mentions clone wire comments g090/052
as potentially the same cause; exact supplied neighbour g052 has no retained records and g090 is not in this interface
file, so no duplicate disposition is asserted. No forbidden coverage source searched. Revision reason: separate the
universal testdoc contract from the correct runtime behavior, retain its exact location and in-range attribution, and
replace the prior dismissal with an other-bucket finding. No parent drop or lowering accepted.

Returned proposal (claims-078, event 7): **retain-verified**, suggested bucket **other**.

The test documentation still declares that Clone needs a host at the supervisor boundary, although this change admits a
source-bearing clone without one. Correct helm routing refutes the original unintended-destination suspicion, not the
documentation defect. The earlier dismissal of the stale comment is superseded by the latest record.

Original stronger consequence: Omitting --host might silently select an unintended clone destination. The
ordinary-source branch explicitly refuses that shape; environment clones intentionally derive the target from their
source. No deletion, replacement or security consequence was established.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the in-range documentation defect. Independently check the earlier destination-authorization
refutation conservatively; this consultation does not approve that withdrawal.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g122 record](records/review-through-00de0b6c/groups/g122.json). Record ID: `c1`.
