# shared gate docs claim hook installations that the module no longer has

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the shared pause-gate helper still tells test maintainers that its module installs two
kinds of hook, although this change removes every local installation.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g155/c8`.

### g155/c8

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 19 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513; checked through tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db.
TLDR: the shared pause-gate helper still tells test maintainers that its module installs two kinds of hook, although
this change removes every local installation. Details: terminal_tabs.rs:162 says 'The two hooks this file installs';
:168 introduces 'The two boundaries in this file' and describes forwarder cleanup publication and tab settle. A
reviewed-revision search of that complete file for notifying_gate calls, forwarder_cleanup_gate and tab_settle_gate
finds only the notifying_gate definition at :178. The remaining known consumer is host_tabs.rs, which imports the helper
at :20 and constructs its settle gate at :417; the assigned patch updates the doc's dead-open reference to that other
module but leaves the local-ownership claims. At the excluded base 6e423e30068b62c6cfa276c6d720ee35bb70387a, both kinds
actually were installed locally (tab_settle_gate :121/:779 and forwarder_cleanup_gate :1051/:1767), so this is not
dismissed because the prose itself predates the assigned commit: removal of the local callers makes it stale within the
range. Tip retains the same claims at :161-172. Rewrite the paragraph around the shared constructor's shape and current
consumer, rather than claiming these removed tests still live here; remove or identify the retired forwarder-gate
example explicitly. Suggested bucket: other, test documentation correctness. Confidence: confirmed by base/reviewed/tip
source and exact local/cross-module caller searches; no execution. proposed_drop: false. possible_cover: related to c6's
stale helper-caller explanation and could share one documentation correction, but c6 concerns a different helper and no
duplicate/drop decision is assumed. Could-be-highest: no security or loss-of-work route. This finding concerns
misleading ownership/scope documentation, not whether runtime gate code works or whether the old pending-cleanup
regression oracle exists (c5). All readily checkable premises settled; c8 is no longer an open lead. No parent drop
accepted.

Returned proposal (claims-099, event 19): **retain-verified**, suggested bucket **other**.

The shared constructor still claims two hooks are installed in its file after every local installation is deleted. The
remaining consumer is in host_tabs. This differs from c6's obsolete conformance-helper rationale.

Original stronger consequence: Initial lead questioned whether any local hook calls remained and whether wording
persisted. Source settles both. It does not assert gate runtime failure or duplicate c5's missing pending-cleanup
oracle.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record the persistent false local-ownership claims on the shared gate constructor; possible joint
documentation fix with c6 is not exact-claim coverage.

Existing parent disposition (event 19): verified. Parent explicitly accepted named retain-verified proposal from
claims-099 after inspecting it. The shared constructor still claims two hooks are installed in its file after every
local installation is deleted. The remaining consumer is in host_tabs. This differs from c6's obsolete
conformance-helper rationale. Record the persistent false local-ownership claims on the shared gate constructor;
possible joint documentation fix with c6 is not exact-claim coverage. Checked source/results:
["647f0227:crates/farhelm/tests/e2e/terminal_tabs.rs:162-173 says 'The two hooks this file installs' and 'two boundaries
in this file'. Local hook search leaves only notifying_gate's definition; no remaining local gate
installation.","6e423e30:terminal_tabs.rs grep finds tab_settle_gate at :121/:779 and forwarder_cleanup_gate at
:1051/:1767, so deletion newly invalidates those ownership statements.","647f0227:host_tabs.rs:417-420 constructs and
installs the shared settle gate. At 00de0b6c the import is :20 and use :407/:410, while terminal_tabs.rs:161-172 still
claims local ownership and names the retired forwarder gate."] Current coverage/results: {"SPEC.md /
SPEC_impl.md":"Host-tab/test-footing and Testing sections do not accept obsolete local-hook ownership
documentation.","TODO.md Planned":"Empty.","plans/queue":"No unlanded entries; retained comment-sweep plan is not a
pending repair.","BUGS.md":"No match.","INDEX.md / matching queue files":"Hook, gate, forwarder, helper documentation
and terminal-test searches find no exact existing item. Generic conformance test coverage is
different.","TRIAGE_OUTCOMES.md":"No matching shared-gate documentation decision.","FILTER.md":"No rare/transient
diagnostic filter applies to this always-stale maintainer description."} Exact cited cover: [] Original stronger
consequence and missing premise remain preserved: "Initial lead questioned whether any local hook calls remained and
whether wording persisted. Source settles both. It does not assert gate runtime failure or duplicate c5's missing
pending-cleanup oracle."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g155 record](records/review-through-00de0b6c/groups/g155.json). Record ID: `c8`.
