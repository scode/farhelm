# Host-scan lock map forbids the terminal calls its fence must span

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The rewritten lock-order map tells maintainers not to make terminal calls while holding
the host-scan fence, although that fence must stay held while terminal hosts start and are adopted. This is an incorrect
internal locking contract, not an observed runtime locking failure.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g110/l1`.

### g110/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: The rewritten lock-order map tells maintainers not to
make terminal calls while holding the host-scan fence, although that fence must stay held while terminal hosts start and
are adopted. This is an incorrect internal locking contract, not an observed runtime locking failure. Details: assigned
patch line 982 (original core.rs:4581–4583) changes the old prohibition on acquiring 'a tmux call' under host_scan to 'a
terminal call'. The new blanket prohibition contradicts the intentional host-only synchronization.
core/host_launch.rs:536 takes the shared spawning guard, :537–579 starts and finishes the host, and :609 calls
HostTerminal::adopt with that same guard still live. core/host_handover.rs:722–728 takes the exclusive guard expressly
through adoption, :732 scans host connections and :786–790 invokes HostTerminal::adopt_found under it. core.rs:4794–4817
explains why this is required: scans take over a host's connection and must not race a spawn before its publication.
SPEC_impl.md:2302–2306 likewise requires host_scan to span host starts until publication and directory scans/removals to
hold it exclusively. The predecessor wording prohibited tmux calls, which are outside the host lifecycle; replacing that
with all terminal calls newly makes the documented rule contradict the required operations. A maintainer following the
prohibition could release the fence too early; no such current implementation defect or work-loss consequence is
asserted. Suggested fix: remove or scope the obsolete external-call restriction, retaining the actual lock acquisition
order and explicitly allowing the host operations the fence protects. Suggested bucket: other/documentation correctness.
Could-be-highest: no established security or loss-of-work route from the text; the established issue is a false
safety/locking explanation. Confidence: confirmed source and contract analysis; no runtime execution. proposed_drop:
false. possible_cover: no matching record in the supplied complete interface snapshot; no prohibited coverage sources
searched. Caveats: correct current runtime code does not refute this documentation defect. No separate broad lock-order
audit is claimed. This revision closes the readily source-checkable lead; later-revision persistence is confirmed as
described below. Attribution/persistence check: git log -S for the exact phrase over the complete pinned range
identifies only 1ed482dde220452e424973faf38a5e1318986333. The tip core.rs:4580–4582 retains the same prohibition; the
later scoped diff contains no repair to it. Reviewed host_launch.rs:639–645 returns the still-live spawning guard with
the new runtime, proving the local guard was not implicitly released before adoption. No narrower definition of terminal
call was found in the surrounding lock map or host-scan field contract. No open source premise remains.

Returned proposal (claims-071, event 10): **retain-verified**, suggested bucket **other**.

The changed lock-order sentence forbids terminal calls under a fence whose stated purpose and implementation require
host startup and adoption under that fence. This is a documentation contract defect; no current runtime failure is
asserted.

Original stronger consequence: A maintainer following the false prohibition could release the scan fence prematurely. No
actual early release, work loss or current runtime locking failure was established; the confirmed consequence is
incorrect safety documentation.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record the false host_scan contract. Parent may extend the adjacent lock-table item with this additional
distinct clause, explicitly preserving both lock identities and causes; it is not exact coverage.

Existing parent disposition (event 10): verified. Parent read full claims071 and complete existing lock-table item.
New1ed482 core4560-4583's host_scan prohibition contradicts its intended and implemented startup/adoption I/O under the
guard; tip4580-4582 retains it. Existing item covers adjacent host_adoption no-connection clause, a different
lock/cause. Extend that maintenance lock-map item with the distinct host_scan clause, preserving both guard names, valid
re-adoption qualification versus startup/scan publication span, and both source paths. This is useful additional
documented invariant scope, not an exact cover or drop. No runtime early-release or work-loss route established. Current
coverage checks found no acceptance/pending repair/filter.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g110 record](records/review-through-00de0b6c/groups/g110.json). Record ID: `l1`.
