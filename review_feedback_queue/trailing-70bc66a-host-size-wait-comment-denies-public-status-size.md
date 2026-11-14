# Resize wait comment gives a false synchronization rationale

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the new resize-wait helper tells future maintainers that it must periodically recheck
terminal size because size is not in runtime status. In fact the public runtime status already carries that exact size
and notifies subscribers when a resize is processed. This is a narrow documentation defect in the stated reason for the
wait's shape; no wrong runtime outcome is alleged. Source: terminal_tabs.rs:2164-2170 says the wait is rechecked every
100 ms 'since the size is the emulator’s and not a status field of its own'. At that same commit
hosts/terminal.rs:644-692 defines HostStatus, with public size: Size at :688-691 explicitly documented as the emulator's
size and the same value as HostTerminal::size. HostTerminal::subscribe at :2116-2118 returns a watch receiver of that
HostStatus. A processed resize at :3754-3766 calls after_record; :3875-3886 calls publish; :3890-3906 updates
status.size through send_modify. Thus size changes do notify the very subscription this helper awaits; the comment
describes a limitation the API does not have. Governing implementation contract: complete SPEC_impl.md 'Recovery and
checkpoints' at 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513:2200-2205 says 'The status carries the program’s modes and the
terminal’s size as of each change'; this public contract is already implemented at the reviewed revision, and the
HostStatus field documents it directly. Attribution: the assigned 70bc66a9 patch adds this helper and its rationale; git
diff against its parent shows no HostStatus/runtime change, so this is not a later API change that invalidated an
originall

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g151/f5`.

### g151/f5

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 70bc66a9f285cc2a9984924f389834d118b7cfdc. TLDR: the new resize-wait helper tells future maintainers
that it must periodically recheck terminal size because size is not in runtime status. In fact the public runtime status
already carries that exact size and notifies subscribers when a resize is processed. This is a narrow documentation
defect in the stated reason for the wait's shape; no wrong runtime outcome is alleged. Source:
terminal_tabs.rs:2164-2170 says the wait is rechecked every 100 ms 'since the size is the emulator’s and not a status
field of its own'. At that same commit hosts/terminal.rs:644-692 defines HostStatus, with public size: Size at :688-691
explicitly documented as the emulator's size and the same value as HostTerminal::size. HostTerminal::subscribe at
:2116-2118 returns a watch receiver of that HostStatus. A processed resize at :3754-3766 calls after_record; :3875-3886
calls publish; :3890-3906 updates status.size through send_modify. Thus size changes do notify the very subscription
this helper awaits; the comment describes a limitation the API does not have. Governing implementation contract:
complete SPEC_impl.md 'Recovery and checkpoints' at 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513:2200-2205 says 'The status
carries the program’s modes and the terminal’s size as of each change'; this public contract is already implemented at
the reviewed revision, and the HostStatus field documents it directly. Attribution: the assigned 70bc66a9 patch adds
this helper and its rationale; git diff against its parent shows no HostStatus/runtime change, so this is not a later
API change that invalidated an originally true comment. Later applicability: the pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the same comment at terminal_tabs.rs:1300-1306 and the same public
status.size contract at hosts/terminal.rs:688-691. Fix direction: correct the helper's rationale to acknowledge resize
status notifications; any periodic fallback retained needs its actual reason, not the nonexistent absence of a size
field. Suggested bucket: other (low-impact test documentation). Confidence: confirmed source/contract analysis; no
execution. proposed_drop: false; correct runtime behavior does not refute inaccurate documentation of its
synchronization contract. Could-be-highest: no established or suspected security/work-loss route. possible_cover: none
in the supplied exact interface findings; forbidden coverage sources were not searched. Material caveat: polling is
bounded and the implementation can still correctly observe a processed resize; this finding does not claim that polling
itself is unsafe or that changing executable behavior is necessary. This resolves f5's HostStatus/history premise.

Returned proposal (claims-097, event 12): **retain-verified**, suggested bucket **other**.

The helper's new rationale says size is not a status field, but the same revision exposes precisely that size in its
public status and notifies subscribers after processed resizes. The comment remains at the review tip.

Original stronger consequence: Future maintainers receive the wrong synchronization model. The lead's
public-status/history premise is resolved; no stronger runtime or user-work consequence was claimed.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record a narrow test-documentation defect, other priority. Correct behavior of the size wait does not
refute the comment defect.

Existing parent disposition (event 12): verified. Parent explicitly accepted named retain-verified proposal from
claims-097 after inspecting it. The helper's new rationale says size is not a status field, but the same revision
exposes precisely that size in its public status and notifies subscribers after processed resizes. The comment remains
at the review tip. Record a narrow test-documentation defect, other priority. Correct behavior of the size wait does not
refute the comment defect. Checked source/results: ["70bc66a9f285cc2a9984924f389834d118b7cfdc terminal_tabs.rs:2164-2170
adds the false rationale; git diff confirms this helper is new and no hosts/terminal.rs change in that commit explains
it as later API drift.","Same revision hosts/terminal.rs:688-691 defines public HostStatus.size as the emulator's size,
:2116-2118 exposes its watch subscription, and :2240-2253 defines HostTerminal::size as applied emulator state.","Same
revision :3754-3766 on_resized calls after_record; :3875-3886 publishes; :3890-3906 writes status.size with send_modify.
Resizes therefore notify the subscription already used by the helper.","Tip 00de0b6 terminal_tabs.rs:1300-1306 retains
the rationale. No wrong runtime result, unsafe polling, test failure or need to change executable behavior was
established."] Current coverage/results: ["At the reviewed revision SPEC_impl.md:2204-2205 says status carries terminal
size as of each change; current Recovery and checkpoints (:2074-2075) preserves this. This contradicts the comment
rather than accepting it. SPEC.md dimension ownership supplies no exception.","Searched TODO Planned, plans/queue,
BUGS.md, queue INDEX/matching terms and active TRIAGE_OUTCOMES.md for helper/status-size/resize-rationale claims. No
exact coverage found; Planned is empty. The broad comment sweep in tmux-drop-from-docs is recorded landed, and its
subject is tmux-specific rationale, not this status claim.","No FILTER.md boundary matches: the inaccurate rationale is
present in normal source reading, without a rare trigger, and is not a product diagnostic."] Exact cited cover: []
Original stronger consequence and missing premise remain preserved: "Future maintainers receive the wrong
synchronization model. The lead's public-status/history premise is resolved; no stronger runtime or user-work
consequence was claimed."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g151 record](records/review-through-00de0b6c/groups/g151.json). Record ID: `f5`.
