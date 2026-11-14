# proposed drop: retirement permits residual agents beside later runs

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The update treats old terminals as ended despite an unconfirmed stop. Its explicit contract
accepts a later Restart starting a second agent beside one still running, which could let concurrent agents modify the
user's work. The original possibly-highest loss-of-work concern remains intact for independent checking; neither a drop
nor a parent lowering has been accepted. Exact accepted residual/proposed_drop basis: SPEC.md:3014-3016 says 'A failure
to end the server or to stop what ran there is logged and never stops the supervisor from starting; those sessions are
treated as ended all the same, accepting that a Restart could then start a second agent beside one still running.'
SPEC_impl.md:2994-3003 says the conversion commits despite an exhausted sweep budget and repeats that same
duplicate-agent acceptance. Only that explicit product choice is the proposed drop basis; it is not a general license to
ignore unrelated cleanup bugs. Intentional old-terminal termination and last-screen loss are also explicit, rather than
silently introduced. Synthesis source checks: old_terminals.rs:290-355 still performs the conversion candidates' agent
and tab sweeps with ScopeKillFailure::Warn under the shared fifteen-second retirement budget. core.rs:7337-7382 then
commits interrupted outcomes and removes launch artifacts despite an unsuccessful sweep. The prior narrowing stands:
Restart at core.rs:12985-13010 takes a fresh AgentOnly sweep with ScopeKillFailure::Refuse, and :7436-7439 derives its
retained scope from the same stored launch generation and launch_scoped fact. Conversion did no

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g005/f1`.

### g005/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4

TLDR: The update treats old terminals as ended despite an unconfirmed stop. Its explicit contract accepts a later
Restart starting a second agent beside one still running, which could let concurrent agents modify the user's work. The
original possibly-highest loss-of-work concern remains intact for independent checking; neither a drop nor a parent
lowering has been accepted.

Exact accepted residual/proposed_drop basis: SPEC.md:3014-3016 says 'A failure to end the server or to stop what ran
there is logged and never stops the supervisor from starting; those sessions are treated as ended all the same,
accepting that a Restart could then start a second agent beside one still running.' SPEC_impl.md:2994-3003 says the
conversion commits despite an exhausted sweep budget and repeats that same duplicate-agent acceptance. Only that
explicit product choice is the proposed drop basis; it is not a general license to ignore unrelated cleanup bugs.
Intentional old-terminal termination and last-screen loss are also explicit, rather than silently introduced.

Synthesis source checks: old_terminals.rs:290-355 still performs the conversion candidates' agent and tab sweeps with
ScopeKillFailure::Warn under the shared fifteen-second retirement budget. core.rs:7337-7382 then commits interrupted
outcomes and removes launch artifacts despite an unsuccessful sweep. The prior narrowing stands: Restart at
core.rs:12985-13010 takes a fresh AgentOnly sweep with ScopeKillFailure::Refuse, and :7436-7439 derives its retained
scope from the same stored launch generation and launch_scoped fact. Conversion did not remove that scope identity. Thus
an ordinary persistent failure to kill a known scope does not alone establish duplicate agents: Restart refuses. The
user-visible interrupted status after failed retirement is established from source; a full shipped
duplicate-agent/work-corruption route has not been shown.

Other accepted boundaries are precise: SPEC.md:2088-2099 accepts detached unscoped processes whose environment markers
can no longer be read, and :2102-2110 says processes Farhelm has no way to identify do not make a cleanup operation
fail. A survivor outside those identifiers or another separately accepted boundary would be needed to get past the later
fresh sweep. No specific agent-survivor sequence establishing the missing premise was found in this source-only contract
review, so do not upgrade speculation into an observed or established concurrent-agent route. What would establish it is
a shipped source path showing an old agent can survive retirement, lose both usable process-tree and marker attribution,
and pass the fresh sweep while a replacement agent runs in its working directory. No reproduction requested or
performed.

Suggested bucket: preserve possibly-highest (security/loss-of-work) assessment for independent adjudication; source has
established failed-retirement classification, but not the complete loss-of-work route. Earlier 'other for established
specification acceptance' wording was a reviewer's provisional assessment of established impact, not an accepted parent
lowering and not grounds to erase the original concern. Confidence: confirmed source classification and exact
specification acceptance; duplicate-agent consequence remains possible with the named missing premise. proposed_drop:
exact quoted specification acceptance, pending independent check. possible_cover: caller reports the same concern at
g007/f1; provenance only, that log was not read and no coverage search was performed. This record remains g005/f1 and is
not withdrawn. No open uninvestigated lead remains.

Returned proposal (claims-002, event 8): **covered**, suggested bucket **other**.

Failed-retirement classification is source-established and exactly accepted. The stronger
duplicate-agent/work-corruption route remains missing a survivor that escapes Restart's fresh refusal-grade sweep. Both
the proposed drop and the earlier provisional consequence lowering need independent adjudication.

Original stronger consequence: Concurrent old/new agents could modify or corrupt the user's work. Earlier other wording
was provisional, not an accepted parent lowering. Keep this potentially highest route and the missing premise in the
independent register; established classification alone is other under the route-established priority rule.

Unresolved premise or limitation: A shipped old-agent survivor that loses usable tree and marker attribution, survives
retirement, passes the fresh Restart sweep, and runs beside the replacement in the same work. Ordinary persistent
known-scope failure does not supply this.

Proposed basis: Exact specification acceptance of failed retirement and the possible duplicate-agent residual. Supplied
g007/f1 is provenance for cross-batch reconciliation, not a separately checked alias here.

Existing parent disposition (event 8): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: covered-including-stated-worst-consequence.
Full002sourceconfirmsstartupWarn-sweepfailurestillcommitsInterrupted;
exactSPEC3008-3010/SPEC_impl2815-2821explicitlyacceptthis ANDpossibleRestartsecondagentbesidestillrunning.
Fullcoverofstatedmigrationfailureconsequence, notgeneralcleanupwaiver.
PreservefreshRefuse-gradeRestartsweepandmissingconcretesurvivorthatpassesit; do not
inferduplicateagentsfromknownscopekillfailurealone or requireabsenceproof.
Originalpossibleconcurrent-workcorruption/highestretainedinregister; freshcheckexactacceptedfailed-retirementroute, not
raritylowering.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g005 record](records/review-through-00de0b6c/groups/g005.json). Record ID: `f1`.
