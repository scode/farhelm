# Unmarked tab-shell cleanup loses its integrated regression oracle

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: deleting the regression removes the integrated check that Delete kills a tab shell and its
background job when neither carries a readable session marker and no system scope can clean them up. This narrows the
original missing-assurance claim; no shipped cleanup malfunction is established.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g103/f1`.

### g103/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 70bc66a9f285cc2a9984924f389834d118b7cfdc; checked through pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: deleting the regression removes the integrated check that Delete kills a
tab shell and its background job when neither carries a readable session marker and no system scope can clean them up.
This narrows the original missing-assurance claim; no shipped cleanup malfunction is established. Details: deleted
parent core.rs:21490-21690 execs env -i in the tab's own pane, asserts shell and job both lack session markers, confirms
both are live and HUP-resistant with no usable scope, then requires both gone after successful Delete. At this
intermediate revision old tmux tabs are still supported; SPEC_impl.md:2970-3005 describes retained old-terminal
operation and service/teardown.rs:181-210 requires other_pane_roots precisely because macOS can hide the tab shell's
environment. Its marker-root alternative remains concrete in sweep.rs:550-625: marked processes become PPID-closure
roots before expansion. The commit-cited replacement host_tabs.rs:1374-1452 (tip :972-1043) scrubs only a background job
and leaves the shell marked, so it cannot distinguish explicit tab-child rooting from marker-root expansion. Existing
host_terminals.rs:2264-2313 (tip :2240-2289), delete_roots_children_after_failed_agent_and_tab_reconnects, does
meaningfully assert Delete kills an unmarked real stand-in child through a recovered placeholder runtime, before cleanup
and despite later host-end refusal. Its child is a directly spawned sleep, not the removed shell/job composition, and
its root collection is the host path in session_hosts.rs:510-548. sweep.rs:2159-2190 separately checks transitive
closure over synthetic process identities. These layered tests narrow the gap but neither exercises the old tmux
other_pane_roots caller nor establishes the combined unmarked-shell/HUP-immune-job/scopes-off/successful-Delete oracle.
They therefore do not refute that specific lost regression coverage. Suggested fix: retain the old fixture while the
legacy branch is supported, or migrate its complete premise and both-process death assertions into an appropriate
retained fixture. Suggested bucket: other, test coverage. Confidence: confirmed source/test-contract comparison, no
execution. Could-be-highest: no established security or loss-of-work route; future undetected cleanup failures are
hypothetical and do not elevate this test defect. proposed_drop: false for the remaining narrow integrated-oracle gap.
Earlier proposed-drop basis is preserved: the independent host pinned-root test plus pure transitive-closure test refute
a broad claim that explicit roots or PPID traversal have no tests; they do not replace this specific oracle. Later
retirement of the tmux branch in 1ed482dde220452e424973faf38a5e1318986333 makes that legacy branch obsolete at tip but
does not erase its supported intermediate gap. No parent drop/lowering was accepted. possible_cover: g107/g107-c4 and
g108/c2 concern lone legacy-tab discovery/listing, Delete guard and Stop after agent loss, a different trigger and
oracle; do not merge this marker-invisibility gap into those. All neighbour records read from discovery-neighbours.json
only, not prohibited coverage sources. Material caveats: production roots remain correct in inspected code; this is not
an actual process leak, not a demand to restore retired tmux machinery at tip, and not a newly discovered issue in this
discovery turn. This revision resolves the caller's pending reconciliation before the separate new search.

Returned proposal (claims-067, event 10): **retain-verified**, suggested bucket **other**.

The deleted test excluded both competing cleanup mechanisms: readable shell markers and system scopes. The cited
replacement leaves its shell marked, while the independent root test uses a direct sleep and refuses final host
shutdown. Neither preserves the integrated shell-plus-background-job/successful-Delete oracle.

Original stronger consequence: Initially claimed explicit unmarked tab roots had no equivalent regression coverage.
History later proposed dropping that broad claim because the existing failed-reconnect child test plus PPID test supply
layered coverage. Latest claim retains only the lost combined unmarked-shell/HUP-immune-job/scopes-off/successful-Delete
oracle, including the old tmux other_pane_roots caller while supported. Production roots remain correct in inspected
source; no current process leak is alleged.

Unresolved premise or limitation: None for the narrow source-level coverage loss. Actual cleanup leakage, runtime false
pass/failure and frequency are not established.

Proposed basis: Retain the narrow test-gap finding, other priority. Preserve the earlier proposed withdrawal and its
narrower valid evidence for parent reconciliation; do not silently turn retirement or production correctness into a
test-equivalence proof.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: preserve-narrow-intermediate-integrated-unmarked-tab-delete-test-gap. Full067 separates
actual removed unmarked-shell plus HUP-resistant child plus scopes-off successful Delete oracle from narrower
replacement marked-shell and failed-host-end pinned-child tests. Those and synthetic PPID tests refute broad
no-root-coverage claim, not integrated equivalence. Intermediate supported tmux gap is real; later1ed retires exact
subject. Preserve both historical gap and earlier proposed withdrawal, then reconcile with established
historical-retirement disposition rather than demand obsolete tip test or allege current production leak. No highest
consequence established for this maintainer test; narrower other claim retained in provenance.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g103 record](records/review-through-00de0b6c/groups/g103.json). Record ID: `f1`.
