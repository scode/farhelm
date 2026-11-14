# Old tmux terminal access is deliberately removed on upgrade

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: upgrading ends agents and shell tabs still in old tmux terminals and loses their terminal
screen/history; normal supervisor paths no longer support accessing or closing those terminals. This is an explicitly
specified migration consequence proposed for dropping, not a confirmed implementation defect.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g113/f2`.

### g113/f2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: upgrading ends agents and shell tabs still in old tmux
terminals and loses their terminal screen/history; normal supervisor paths no longer support accessing or closing those
terminals. This is an explicitly specified migration consequence proposed for dropping, not a confirmed implementation
defect. Details: assigned patch core.rs:13750–13768 replaces tmux tab-open liveness with host liveness; 14050–14060
removes tmux tab rediscovery/close; 15799–15813 refuses report anchors for stored tmux rows. Constructor 6142–6152
invokes OldTerminals::end_private_server before reload; reload contract 6372–6392 leaves old-kind rows with no terminal
while preserving their recorded address as evidence of a prior launch. Suggested bucket: highest if treated as a defect,
because running processes and terminal history are user-owned work. Confidence: confirmed source/contract behavior, not
runtime-observed. Could-be-highest: yes. proposed_drop: SPEC.md, Terminal experience, 3003–3016 explicitly accepts this
process/history loss. Exact text (line wrapping normalized): "This release ends them: the first time a host's supervisor
of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without
needing a tmux program." Exact screen/tab-loss acceptance: "one whose agent had already ended keeps its outcome, but its
terminal's last screen, which lived in the server, is gone; old tabs are closed." Exact failure acceptance: "A failure
to end the server or to stop what ran there is logged and never stops the supervisor from starting; those sessions are
treated as ended all the same, accepting that a Restart could then start a second agent beside one still running."
SPEC_impl.md, Terminal substrate: private tmux server, 2882–2930 describes startup cleanup and conversion.
possible_cover: the parent reports likely duplicate scope in g099/f1, g110/f1 and g053/f1; those logs were not read here
and semantic deduplication remains the parent's. Material caveats: independent proposed-drop review has not accepted a
drop, so this record remains open with its original highest consequence. This acceptance applies only to ending old tmux
terminals, their process/history loss, and removal of their operational access paths. It does not settle generic
survival guidance, a missing migration warning, or a distinct implementation error in cleanup/report handling. Revision
preserves exact reviewed source and acceptance text and adds the caller-supplied possible-duplicate references and
acceptance limits; no original consequence or severity has been lowered.

Returned proposal (claims-073, event 7): **covered**, suggested bucket **highest**.

The specifications explicitly accept ending old terminals, their programs and terminal history, and removing their
operational access. This is a proposed specification-coverage disposition requiring the independent drop check. It does
not cover misleading survival guidance, missing warnings or a distinct cleanup/reporting implementation error.

Original stronger consequence: Upgrade ends running old agents and tab shells and loses their terminal screens/history.
These are user-owned processes and work, so highest is preserved even though the migration is explicitly accepted.

Unresolved premise or limitation: Independent fresh-context agreement is still required before the parent drops this
highest-consequence record.

Proposed basis: Explicit specification acceptance of old-terminal retirement only; no accepted drop is inferred.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g113 record](records/review-through-00de0b6c/groups/g113.json). Record ID: `f2`.
