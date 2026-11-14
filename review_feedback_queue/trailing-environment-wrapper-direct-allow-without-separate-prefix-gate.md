# Environment wrapper bytes are shown and the separate prefix gate does not apply

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: investigated whether omitting the ordinary card's separate 'agent supplied wrapper' block hides a
required authorization decision on new environment cards. The entire wrapped start and resume command is shown; the
separate prefix gate is specific to destinations that ask before YOLO, and these new environment hosts start with YOLO
allowed. Checks/results: approvals.rs:1051-1056 and :528-531 limit provenance-specific block/note to
LaunchWithAgentWrapper, while :1118 calls launch_rows. launch_rows at :1361-1399 retains raw wrapper words in the resume
display and displays the complete start via display_command. agent_requests.rs:4108-4110 retains agent_wrapper_supplied;
:4256-4266 populates it from resolution and :4477-4483 carries it into the request. SPEC.md:4123-4129 states exactly 'on
a host that asks before YOLO launches, it needs an Allow or Deny card even when ordinary agent commands from that host
run without asking' and 'A host allowed to start YOLO sessions without asking has no prefix approval gate.'
SPEC.md:4340-4342 says the new environment defaults allow YOLO; SPEC_impl.md:6687-6690 says exactly 'the direct-Allow
card already covers an agent-supplied wrapper.' Thus 'otherwise shows what any launch card shows' does not require these
cards to emulate a separate prefix-gate presentation for a host where that gate does not apply. Suggested bucket: other,
no established defect. Confidence: confirmed by source and scoped contract analysis, no runtime observation.
proposed_drop: complete command visibility plus the explicit scope of the separate prefix gate refute the s

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g041/l3`.

### g041/l3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (candidate, open):

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435. TLDR: investigated whether omitting the ordinary card's
separate 'agent supplied wrapper' block hides a required authorization decision on new environment cards. The entire
wrapped start and resume command is shown; the separate prefix gate is specific to destinations that ask before YOLO,
and these new environment hosts start with YOLO allowed. Checks/results: approvals.rs:1051-1056 and :528-531 limit
provenance-specific block/note to LaunchWithAgentWrapper, while :1118 calls launch_rows. launch_rows at :1361-1399
retains raw wrapper words in the resume display and displays the complete start via display_command.
agent_requests.rs:4108-4110 retains agent_wrapper_supplied; :4256-4266 populates it from resolution and :4477-4483
carries it into the request. SPEC.md:4123-4129 states exactly 'on a host that asks before YOLO launches, it needs an
Allow or Deny card even when ordinary agent commands from that host run without asking' and 'A host allowed to start
YOLO sessions without asking has no prefix approval gate.' SPEC.md:4340-4342 says the new environment defaults allow
YOLO; SPEC_impl.md:6687-6690 says exactly 'the direct-Allow card already covers an agent-supplied wrapper.' Thus
'otherwise shows what any launch card shows' does not require these cards to emulate a separate prefix-gate presentation
for a host where that gate does not apply. Suggested bucket: other, no established defect. Confidence: confirmed by
source and scoped contract analysis, no runtime observation. proposed_drop: complete command visibility plus the
explicit scope of the separate prefix gate refute the suspected missing authorization decision; this is a proposed
candidate disposition, not a parent-accepted drop. possible_cover: none in supplied neighbour records. Could-be-highest:
no established route to unseen execution or loss of work; all executed wrapper bytes remain in the command display and
direct Allow is required. Original concern preserved: the new card does omit the separate agent-provenance label, but
that alone does not establish violation of this scoped gate. No missing source premise or open investigation remains.

Returned proposal (claims-025, event 11): **covered**, suggested bucket **other**.

The environment card omits the ordinary separate agent-provenance label, but displays the complete wrapped start and
resume commands and always requires direct Allow. The specification scopes the separate prefix gate to hosts that ask
before YOLO and expressly says the environment card already covers an agent-supplied wrapper.

Original stronger consequence: Line 10 investigated whether omitting the separate agent-supplied-wrapper provenance
block hides a required authorization decision. It already noted that full command bytes are shown, so unseen execution
was not established. Line 11 proposes a contract-based drop while preserving the actual label omission. There is no
established route to unseen execution or loss of work.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Propose narrowly spec-covered absence of a separate prefix gate/provenance block on an
always-direct-Allow new-environment card whose entire command is shown. Because this is wrapper authorization, include
the proposed dismissal in the independent drop check despite the latest other bucket.

Existing parent disposition (event 11): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-direct-Allow-complete-wrapper-without-separate-prefix-gate. Parent read full025
plusactualSPEC4080-4100/4282-4316andimpl6188-6202.
Separateprefixprovenancelabelomittedbutcompletewrappedstart/resumecommandshownanddirectAllowalwaysrequired;
newhostdefaultsYOLOallowedandexplicitimplsaysdirectcardcoversagentwrapper.
Proposedabsenceofseparateprefixgate/labelacceptedforthiscompletecommandcardonly.
Originalpotentialunseenauthorizationconcernremainsforfreshgate; noproofallcommandrenderingsorassistivetechnologycorrect,
notrarelabeldiagnosticfilter.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g041 record](records/review-through-00de0b6c/groups/g041.json). Record ID: `l3`.
