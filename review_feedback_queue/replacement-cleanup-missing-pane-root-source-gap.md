# Replacement cleanup without a pane root remains unresolved

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Failed replacement cleanup without a pane root has a conditional orphaning concern, but a supported uncovered producer
remains missing.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The review checked managerless and pane/root ownership paths and the exact compound-fault exceptions. It did not
establish a complete supported history outside those exceptions that leaves the suspected replacement work unowned.
Existing optional-PID, vanished/foreign target and startup-interval refutations remain bounded. No process or work loss
was observed.

Source locations: `crates/farhelm-supervisor/src/service/core.rs:12942-12961`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Follow-up to original discovery g180/f5, same reviewed tip d6645f771bee8ed03a66486ef8755834937d759d. Additional
accessible checks: core.rs:3058-3079 explicitly retains no terminal after an ambiguous fresh-terminal relaunch;
:6250-6311 recovers existing ordinary marked panes during reload; :15635-15702 limits retained-create terminal-less
publication to confirmed-not-launched failures or fallback snapshots and protects an already newer entry. These identify
candidate terminal-less histories but do not establish a supported uncovered one-failure replacement-cleanup sequence. A
prior ambiguous fresh-terminal relaunch followed by a new failed replacement adds an earlier failure and may fall under
the exact independent-two-fault acceptance rather than prove an uncovered route. Read queue files
startup-surviving-tab-adoption-source-gap.md:1-42 and retained-refusal-can-clear-restart-delete-protection.md:1-48: both
retain their own missing supported state producers; neither supplies the missing producer here, and neither covers the
different rootless-cleanup consequence. The queue reports are context, not independently authenticated runtime evidence.
No current capture rollback/wrong-Resume allegation is made. The conditional no-root/HUP-resistant/unmarked tree
mechanism remains as originally logged. Exact remaining premise is a supported uncovered producer of that replacement
state with only one ordinary failure, or a concrete in-scope history outside the accepted compound-fault cases. This
cannot be settled by asserting arbitrary tmux error-after-effect timing or out-of-band pane removal; it needs the actual
shipped source sequence, or later permitted runtime evidence establishing it. Suggested bucket OTHER conditional source
lead, no separate highest route established, confidence unverified after investigation, proposed_drop=false pending
parent synthesis. possible_cover stays SPEC.md:3912-3923 for direct private-server variants and :1515-1527 /
TRIAGE_OUTCOMES.md:7948-7964 for independently compound-fault variants, not blanket coverage. No new runtime work, no
new ID, and no erasure of original premise or investigation history.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g180/f5`, `a056/g180-f5`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
