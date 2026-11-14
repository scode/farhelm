# Intentional legacy terminal retirement remains a specification-based drop proposal

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The first startup of this version terminates user agents and shell work still in legacy terminals
and destroys the terminal server's retained screen/history, including final screens for already-ended agents. This is
actual source-implied process/history loss, not merely a hypothetical failed cleanup; no execution was performed.
Suggested bucket: highest on that original strongest consequence; could-be-highest: yes. Confidence: confirmed source
and contract. hosts/process.rs:476-506 supplies the verified SIGTERM/SIGCONT/SIGKILL operation used by
service/core/old_terminals.rs:223-257; removal of the server destroys its in-memory terminals. proposed_drop: exact
specification acceptance of this intentional retirement only, pending the caller's independent check; no parent drop has
been accepted. SPEC_impl.md, Terminal substrate: private tmux server, 2956-2958: "This section covers the old terminals:
the private tmux server releases before terminal hosts ran every terminal in, which later releases kept running beside
terminal hosts. This release ends them at startup (below); every terminal runs in a terminal host". SPEC.md, Terminal
experience, explicitly says the first startup ends that server and everything still running in an old terminal, and an
already-ended agent keeps its outcome but loses the last screen held in the server. Distinct failure contract, not a
universal refutation: SPEC_impl.md:2998-3000 says "A failure anywhere is logged and never fatal (the maintainer chose to
start anyway and treat old sessions as ended, accepting that a Restart could then start a second

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g097/c1`.

### g097/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 18 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. Synthesis revision. TLDR: The first startup of this version
terminates user agents and shell work still in legacy terminals and destroys the terminal server's retained
screen/history, including final screens for already-ended agents. This is actual source-implied process/history loss,
not merely a hypothetical failed cleanup; no execution was performed. Suggested bucket: highest on that original
strongest consequence; could-be-highest: yes. Confidence: confirmed source and contract. hosts/process.rs:476-506
supplies the verified SIGTERM/SIGCONT/SIGKILL operation used by service/core/old_terminals.rs:223-257; removal of the
server destroys its in-memory terminals. proposed_drop: exact specification acceptance of this intentional retirement
only, pending the caller's independent check; no parent drop has been accepted. SPEC_impl.md, Terminal substrate:
private tmux server, 2956-2958: "This section covers the old terminals: the private tmux server releases before terminal
hosts ran every terminal in, which later releases kept running beside terminal hosts. This release ends them at startup
(below); every terminal runs in a terminal host". SPEC.md, Terminal experience, explicitly says the first startup ends
that server and everything still running in an old terminal, and an already-ended agent keeps its outcome but loses the
last screen held in the server. Distinct failure contract, not a universal refutation: SPEC_impl.md:2998-3000 says "A
failure anywhere is logged and never fatal (the maintainer chose to start anyway and treat old sessions as ended,
accepting that a Restart could then start a second agent beside one still running)". That clause specifies proceeding
after failure; it does not establish correct process selection, sufficient identification, valid attribution, or
adequate warning presentation. Parent's g004/g071 general survival guidance and g033 hidden failure-warning concern are
separate causes and are not disposed of by c1. possible_cover: parent reports g099/g110 independently describe the same
intentional terminal/history retirement, likely duplicates; no queue coverage sources searched. Caveats: source-only
review, and the parent owns reconciliation of those interface findings and independent drop approval. Discovery turn 3
normalization: kind is now finding, preserving the full original consequence and exact proposed_drop citations above.
Parent has accepted no drop or lowering. Neighbour records g099/f1 and g110/f1 describe the same intentional
retirement/history consequence, with g008/f1, g113/f2 and several driver-deletion groups also retaining provenance;
overlap is not a disposition. Distinct surviving-agent, hidden-tab consent, spare-cleanup, warning and guidance findings
remain outside this proposed drop.

Returned proposal (claims-064, event 18): **covered**, suggested bucket **highest**.

Source establishes the intentional ending of legacy agents, tabs and retained screens. The specifications explicitly
accept that exact consequence. This does not cover selecting a wrong process, mistaken pane attribution, inadequate
warnings or surviving-agent guidance. The parent-reported aliases are outside this batch and were not independently
matched.

Original stronger consequence: First startup terminates user agents and shell work in legacy terminals and destroys
their retained screen/history, including final screens of ended agents. Preserve highest until the independent drop
decision.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact specification acceptance of intentional retirement only; this consultation does not authorize the
drop.

Existing parent disposition (event 18): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-accepted-intentional-old-terminal-process-and-screen-retirement. Parent read
full064 in two bounded halves. Explicit startup retirement terminates old agents/tabs and retained final screens,
exactly accepted by product/impl, including separately logged nonfatal cleanup failure. Highest actual process/content
consequence preserved until gate. Scope only intended old resources, not wrong server/pane attribution, inadequate
warnings or survivor assurance; aliases need actual join. No ordinary-upgrade rarity filter or Mid-term TODO cover.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g097 record](records/review-through-00de0b6c/groups/g097.json). Record ID: `c1`.
