# Stop comment incorrectly says full-screen final frames are never retained

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the internal explanation of Stop tells future developers that a full-screen program's
final frame must not remain after exit, although the product deliberately keeps it when the program does not leave its
alternate screen.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g125/c3`.

### g125/c3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: the internal explanation of Stop tells future
developers that a full-screen program's final frame must not remain after exit, although the product deliberately keeps
it when the program does not leave its alternate screen. Details: changed handlers.rs lines 944-947 say SPEC.md states
'a full-screen program's last frame is not retained after it exits and no snapshot of it is taken or stored.' The first
half is false for the host-only terminal model this patch introduces. SPEC.md Terminal experience, lines 2897-2901,
explicitly says a program that does not leave its full-screen display, 'because it was killed, say', leaves that frame
showing in the same terminal and keeps it across supervisor restarts. SPEC_impl.md Launch failure classification, lines
2703-2708, distinguishes retaining the original terminal from taking a snapshot and painting it into a restarted
terminal. The executable handler does not capture a snapshot and correctly leaves the original host and attachment
intact; lines 1144-1149 even explain that the host keeps the final screen. The concrete misinformation is a nonexistent
blanket ban on retaining the existing terminal's last frame, which could misdirect lifecycle maintenance or tests toward
clearing a frame the product promises to keep. Suggested bucket: other (documentation correctness). Could-be-highest: no
established security or loss-of-work route from this comment. Confidence: confirmed source/contract discrepancy; no
execution. proposed_drop: false; lack of an executable behavior defect does not refute this bounded documentation issue.
possible_cover: parent supplied g117c3 as an analogous stale lifecycle explanation; no coverage sources searched and no
duplicate disposition asserted. Caveats: this is not a claim that stopped frames currently disappear. The comment's
separate rule against taking/storing a pre-stop snapshot remains correct. The old comment also carried the blanket
non-retention claim, but the reviewed patch rewrites and retains it while removing its final qualification that the dead
pane shows whatever tmux still holds. This finding revision supersedes the prior proposed drop under the same ID and
corrects the exact source span; no parent drop accepted.

Returned proposal (claims-079, event 10): **retain-verified**, suggested bucket **other**.

The changed comment attributes a blanket final-frame non-retention rule to the specification, which explicitly preserves
the original terminal's frame when a program does not leave its alternate screen. This is a documentation defect;
correct runtime retention does not refute it.

Original stronger consequence: Misdirecting future lifecycle maintenance or tests into clearing a frame the product
promises to keep. Lines 4 and 6 proposed dropping the bounded comment issue for lack of runtime impact; lines 8 and 10
explicitly supersede that drop. The no-snapshot/no-repaint part remains correct.

Unresolved premise or limitation: None for the source/contract discrepancy. No current disappearing frame, security
consequence or user-work-loss route is established.

Proposed basis: Retain the bounded internal documentation finding at other. Do not describe it as a runtime frame-loss
bug or merge it with unspecified analogous lifecycle comments.

Existing parent disposition (event 10): verified. Parent explicitly accepted named retain-verified proposal from
claims-079 after inspecting it. The changed comment attributes a blanket final-frame non-retention rule to the
specification, which explicitly preserves the original terminal's frame when a program does not leave its alternate
screen. This is a documentation defect; correct runtime retention does not refute it. Retain the bounded internal
documentation finding at other. Do not describe it as a runtime frame-loss bug or merge it with unspecified analogous
lifecycle comments. Checked source/results: ["1ed482d:service/handlers.rs:944-947 says a full-screen program's last
frame is not retained after exit. Its :1144-1149 instead correctly says the host keeps the final screen and leaves the
attachment intact. The inaccurate comment remains at pinned tip :944-947.","1ed482d:SPEC.md:2897-2901 preserves the
frame of a program killed without leaving its full-screen display, across supervisor restarts. SPEC_impl.md:2703-2708
distinguishes keeping the original screen from capturing and repainting a snapshot in another terminal.","The parent
comment at handlers.rs:1013-1024 already includes the blanket claim, but qualifies it with whatever tmux itself still
holds. The assigned diff rewrites that explanation for hosts and removes the qualification. Attribution is the newly
rewritten, unqualified host-lifecycle explanation, not first invention of the blanket phrase."] Current
coverage/results: ["Current SPEC.md:2889-2898 retains the original-screen versus no-painted-snapshot distinction. It
contradicts this comment, rather than accepting it.","Searched both specifications, empty TODO.md Planned, plans/queue,
BUGS.md, INDEX.md and active TRIAGE_OUTCOMES.md for full-screen frame retention, stopped-host final screens and
lifecycle comment drift. No exact queued/decided cover. tmux-drop-from-docs's broad comment audit is already landed
(plans/REPORTS.md:32), so it is not an unlanded cover. No FILTER.md boundary matches ordinary source-comment
misinformation."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved:
"Misdirecting future lifecycle maintenance or tests into clearing a frame the product promises to keep. Lines 4 and 6
proposed dropping the bounded comment issue for lack of runtime impact; lines 8 and 10 explicitly supersede that drop.
The no-snapshot/no-repaint part remains correct."; "None for the source/contract discrepancy. No current disappearing
frame, security consequence or user-work-loss route is established.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g125 record](records/review-through-00de0b6c/groups/g125.json). Record ID: `c3`.
