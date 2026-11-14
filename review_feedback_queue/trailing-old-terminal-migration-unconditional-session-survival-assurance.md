# Installer survival assurance can lead users to leave work in terminals this update ends

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: someone relying on 'Your sessions keep running' may reopen after updating with work still running
in an old terminal, and the new supervisor deliberately terminates that work. Original concern preserved: the installer
promises session survival during the one update that ends old tmux agents and tabs. This is a process-loss guidance
concern, not merely cosmetic completion wording; the prior suggested 'other for the diagnostic itself' did not resolve
that consequence and no lowering has parent acceptance. Suggested bucket: potentially highest/security or loss of work,
pending the parent's independent judgment; could-be-highest: yes. Confidence: confirmed source-level misleading general
assurance alongside intended terminal termination; actual user reliance and execution have not been observed. Source
references: scripts/install.sh:1287 restores 'Quit and reopen Farhelm to finish updating. Your sessions keep running.'
SPEC.md:3001-3016 and SPEC_impl.md:2784-2834 describe the separately intended retirement. Exact termination acceptance
in SPEC.md: 'This release ends them: the first time a host's supervisor of this release starts, it ends that tmux
server, and with it everything that still ran in an old terminal, without needing a tmux program.' Acceptance of
retirement alone does not settle inaccurate survival guidance. The narrower separate proposed wording acceptance is the
reviewed commit rationale: 'Every other update keeps sessions running, so the general statement comes back without the
caveat, accepting that the installer prints it during that one update too.' proposed_d

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g071/f2`.

### g071/f2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 78e9d68a95119db8452ee139efde13d38546f648. TLDR: someone relying on 'Your sessions keep running' may
reopen after updating with work still running in an old terminal, and the new supervisor deliberately terminates that
work. Original concern preserved: the installer promises session survival during the one update that ends old tmux
agents and tabs. This is a process-loss guidance concern, not merely cosmetic completion wording; the prior suggested
'other for the diagnostic itself' did not resolve that consequence and no lowering has parent acceptance. Suggested
bucket: potentially highest/security or loss of work, pending the parent's independent judgment; could-be-highest: yes.
Confidence: confirmed source-level misleading general assurance alongside intended terminal termination; actual user
reliance and execution have not been observed. Source references: scripts/install.sh:1287 restores 'Quit and reopen
Farhelm to finish updating. Your sessions keep running.' SPEC.md:3001-3016 and SPEC_impl.md:2784-2834 describe the
separately intended retirement. Exact termination acceptance in SPEC.md: 'This release ends them: the first time a
host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old
terminal, without needing a tmux program.' Acceptance of retirement alone does not settle inaccurate survival guidance.
The narrower separate proposed wording acceptance is the reviewed commit rationale: 'Every other update keeps sessions
running, so the general statement comes back without the caveat, accepting that the installer prints it during that one
update too.' proposed_drop: retain that explicit commit-level wording decision for the parent's independent check; no
drop accepted, and no claim that the retirement specification alone authorizes the assurance. possible_cover:
parent-supplied g002/f1 and g004/f1 concern this same restored promise on implementation/specification surfaces; g053/f1
intentional retirement is a distinct claim, and g033 omitted-warning/cleanup-failure residual is separate. No queue,
ledger, filters or plans searched. Checks this synthesis so far: read own complete six-event log/progress and g004/f1's
latest revision; reconcile attribution and caveats without a new finding ID. Next checks: direct pinned installer
report, startup caller and termination helper to verify the path and distinguish source from execution. These are
readily checkable source premises, not a new unresolved lead. Direct checks at 78e9d68a95119db8452ee139efde13d38546f648
now confirm: install.sh:1228-1232 derives updated_installation only from the install result; 1285-1287 prints the
survival sentence for every update, with no old-terminal condition. core.rs:6112-6117 opens and checks the database
first, then 6142-6151 invokes OldTerminals::end_private_server for the state-directory owner before reloading sessions.
old_terminals.rs:176-232 connects to the old socket, verifies same-user peer identity, snapshots pane descendants and
calls terminate_process_within with the server socket predicate and TERM/KILL grace; 247-251 handles Terminated/Killed
as successful retirement. The commit message was re-read directly and contains the exact narrower wording decision
quoted above. This establishes termination as source behavior rather than an observed run, and preserves the possibility
of checks failing or a nonmatching process being left alone. No new finding or new discovery is claimed. Remaining
direct source checks completed at the reviewed revision: desktop.rs:537-586 computes whether the recorded supervisor
started from the current program; plan_local_supervisor at 835-842 selects Replace with wait_first=false when the
program differs. Desktop startup stops that managed predecessor and spawns the new supervisor (585-586), whose command
at 1232-1238 is the selected farhelm supervisor run --state-dir for the same state directory. stop_own_supervisor at
1055-1083 refuses if the predecessor cannot be confirmed stopped. Thus reopening an ordinary updated managed
installation reaches the startup retirement code; a manually started answering supervisor without a matching record is
reused (835-838), a prerequisite failure can refuse before replacement, and failed verification/termination can leave
processes running. hosts/process.rs:476-506 confirms real SIGTERM, SIGCONT and SIGKILL calls after identity checks.
SPEC.md:3001-3016 was re-read at this revision and expressly distinguishes old terminals from terminal-host sessions.
These source caveats do not erase the ordinary supported route or the original potential work-loss guidance concern. The
exact installer-sentence acceptance remains the separately quoted commit rationale, not an inference from terminal
retirement being specified. All readily checkable premises for this guidance claim are settled by source; human reliance
is a hypothetical consequence, not an observed event or a premise requiring reproduction. No open lead remains, and f2
stays an open potentially-highest finding with a proposed drop awaiting independent parent checking. Cross-batch checks
also confirm 17a3fe8 retains the reassurance while deleting only tmux advice; its companion output test still explicitly
expects the reassurance. c1 remains source-refuted and c3 remains specification-accepted; the setup ownership lock and
same-state rendering are unchanged. No new finding IDs or discoveries this synthesis.

Returned proposal (claims-046, event 9): **retain-verified**, suggested bucket **highest**.

Source establishes an ordinary supported update/reopen path from the survival assurance to intentional termination of
still-running old terminals. Human reliance is hypothetical, not a runtime observation. The commit deliberately accepts
this exact wording; that proposed drop needs independent judgment and must not be replaced by acceptance of termination
alone.

Original stronger consequence: Users relying on 'Your sessions keep running' can leave work running in old terminals and
reopen after updating; the new supervisor intentionally ends those processes. Earlier 'other for the diagnostic itself'
wording did not dispose of this work-loss guidance consequence.

Unresolved premise or limitation: No missing source premise for the conditional guidance-to-process-loss route. Actual
reliance and execution were not observed. Whether the explicit commit-level wording decision is sufficient acceptance to
drop the guidance claim remains for the independent disposition.

Proposed basis: Retain the highest consequence pending the parent's independent check of the precise wording acceptance.
No filter matches: this is ordinary migration/update guidance, not an additional rare interruption or harmless
diagnostic. Parent-supplied g002/f1 and g004/f1 may share this root, but they are outside this batch and were not
independently checked as aliases.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-highest-survival-assurance-pending-precise-wording-acceptance. Parent read full046
flagged output. Actual supported installer update prints unconditional survival assurance and reopen can replace
recorded prior supervisor, whose new startup intentionally terminates old terminal work. Prerequisite refusal/unrecorded
supervisor reuse and bounded shutdown failures mean conditional, not every update; actual human reliance unobserved.
Retirement acceptance alone does not accept misleading guidance. Exact78e9 commit deliberately restores statement,
removes SPEC caveats and explicitly accepts printing during this migration: preserve that precise decision for
independent coverage judgment, rather than silently discard or treat a commit rationale as automatically one of the
listed coverage sources. Related002/004 histories need canonical reconciliation. Original highest work-loss guidance
consequence stays pending gate, no rarity/harmless diagnostic filter.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g071 record](records/review-through-00de0b6c/groups/g071.json). Record ID: `f2`.
