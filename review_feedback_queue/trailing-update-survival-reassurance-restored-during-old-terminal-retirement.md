# Update-survival promise omits the explicitly accepted old-terminal migration

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Someone trusting the restored statement that sessions keep running across an update could leave
running work in an old terminal that this migration ends. Source consequence and attribution: assigned SPEC_impl.md
patch removes the explicit migration caveat and restores '; sessions keep running.' at line 7781;
scripts/install.sh:1285-1287 prints 'Quit and reopen Farhelm to finish updating. Your sessions keep running.' whenever
updated_installation is 1. Direct synthesis checks at the same revision establish the separate termination route:
service/core.rs:6142-6151 calls OldTerminals::end_private_server only for the state-directory owner and before session
reload; service/core/old_terminals.rs:164-232 reaches a verified same-user private server and calls
terminate_process_within, within the migration deadline. The reassurance-restoration commit does not introduce that
termination: it restores general wording after the earlier retirement change. Exact general retirement acceptance,
SPEC.md:3004-3006: 'This release ends them: the first time a host's supervisor of this release starts, it ends that tmux
server, and with it everything that still ran in an old terminal, without needing a tmux program.' That retirement rule
alone does NOT accept inaccurate assurance. Separate exact stated wording decision in this reviewed commit's rationale:
'Every other update keeps sessions running, so the general statement comes back without the caveat, accepting that the
installer prints it during that one update too.' Preserve this narrower decision for independent checking. SPEC.md:3012
separat

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g004/f1`.

### g004/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 78e9d68a95119db8452ee139efde13d38546f648. TLDR: Someone trusting the restored statement that sessions
keep running across an update could leave running work in an old terminal that this migration ends. Source consequence
and attribution: assigned SPEC_impl.md patch removes the explicit migration caveat and restores '; sessions keep
running.' at line 7781; scripts/install.sh:1285-1287 prints 'Quit and reopen Farhelm to finish updating. Your sessions
keep running.' whenever updated_installation is 1. Direct synthesis checks at the same revision establish the separate
termination route: service/core.rs:6142-6151 calls OldTerminals::end_private_server only for the state-directory owner
and before session reload; service/core/old_terminals.rs:164-232 reaches a verified same-user private server and calls
terminate_process_within, within the migration deadline. The reassurance-restoration commit does not introduce that
termination: it restores general wording after the earlier retirement change. Exact general retirement acceptance,
SPEC.md:3004-3006: 'This release ends them: the first time a host's supervisor of this release starts, it ends that tmux
server, and with it everything that still ran in an old terminal, without needing a tmux program.' That retirement rule
alone does NOT accept inaccurate assurance. Separate exact stated wording decision in this reviewed commit's rationale:
'Every other update keeps sessions running, so the general statement comes back without the caveat, accepting that the
installer prints it during that one update too.' Preserve this narrower decision for independent checking. SPEC.md:3012
separately says 'A session whose agent or tabs already run in terminal hosts keeps them.' Suggested bucket remains
potentially highest because misleading update guidance could induce a loss-of-work mistake; could-be-highest: yes.
Confirmed source consequences are the misleading general reassurance and the independently implemented one-time
termination route. No reliance event or process termination was observed during execution; no runtime tests or
reproductions were run. Confidence: confirmed source-level mismatch with an explicit deliberate wording decision.
proposed_drop: the exact commit-rationale decision separately accepts printing the assurance during the migration; this
remains only a proposal, with NO parent drop or lowering accepted. possible_cover: parent-supplied g002/f1, this g004/f1
and g071/f2 concern this same reassurance restoration at the same revision; g071/f2 supplies the installer surface, now
checked directly. These are manifestations of one underlying claim, not new discoveries. g033's
omitted-warning/cleanup-failure residual is distinct: neither a post-termination notice nor assumptions that cleanup
always succeeds are used to refute this reassurance concern, and this record makes no claim about g033's residual.
Revision adds directly checked shipped premises and attribution, preserves the original potentially highest consequence,
and leaves the record open for independent judgment. No queue, ledger or plan coverage searches.

Returned proposal (claims-002, event 7): **needs-parent**, suggested bucket **highest**.

The misleading general assurance and the intended one-time termination are established. The commit rationale explicitly
accepts printing the assurance during that migration, but that is a separate decision source from specification
acceptance of termination. Parent must adjudicate that proposed exclusion through the independent check; do not silently
treat migration acceptance as wording acceptance.

Original stronger consequence: A user relying on the assurance could leave running work in an old terminal that this
migration ends. Keep potentially highest until independently adjudicated; do not conflate this with omitted
post-termination warnings or residual failed-cleanup behavior.

Unresolved premise or limitation: No actual reliance event was observed. Parent must decide whether the explicit
commit-rationale wording decision is sufficient accepted cover under this recording flow; absent that decision, retain
the contract mismatch.

Proposed basis: Potential explicit wording acceptance in the reviewed commit rationale. Supplied g002/f1 and g071/f2 are
same-root provenance, not independently read named records in this batch; reconcile them by underlying_key.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: duplicate-retained-highest-g002/f1. Full002same78e9
installer/SPECunconditionalsurvivalassuranceduringordinarymigrationthatterminatesoldagents/tabs.
Preserveexplicitcommitdeliberate-wordingdecision andbothcontradictorycontractclauses;
bareterminationacceptancedoesnotacceptassurance. Canonical002f1 retains highestconditionalworkloss,
nofiles/currenthostharm. Dedup exactsameassurance/rootwith004SPEC_implwordingprovenance andlater071f2, do not
silentlydiscardtheoriginalreviewerintent-baseddropproposal.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g004 record](records/review-through-00de0b6c/groups/g004.json). Record ID: `f1`.
