# Retired tmux status and tab discovery follows explicitly accepted process loss

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Retiring private-tmux terminals can terminate running agents, close old tabs and discard terminal
screens/history. This release deliberately treats those terminals as ended even if teardown fails, so Restart can create
a second agent beside one that survived. Could-be-highest: yes, established process/history loss. Suggested bucket:
highest if explicit acceptance is rejected. Confidence: confirmed source behavior and specification acceptance,
source-only. proposed_drop: SPEC.md:3006–3010 says "This release ends them: the first time a host's supervisor of this
release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a
tmux program" and "its terminal's last screen, which lived in the server, is gone; old tabs are closed".
SPEC.md:3013–3016 says "A session whose agent or tabs already run in terminal hosts keeps them" and explicitly accepts
Restart beside a surviving old agent. SPEC_impl.md:2891–2927 specifies retirement before reload, terminal-less old rows,
Interrupted conversion for active outcomes, and preservation of ended outcomes. Source: service/core.rs:6142–6184 ends
old terminals before reload; :6675–6679 never resolves old pane addresses; :6769–6782 reads direct generation-scoped
sentinel evidence before :6805–6822 converts remaining active old rows to Interrupted. :6635–6645 separately adopts host
tabs for every session, including old-kind agent rows, so old-agent conversion does not itself discard host tabs.
Removing pane_evidence and old tab discovery from status.rs follows that replacement contract. possi

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g130/c1`.

### g130/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 5 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Retiring private-tmux terminals can terminate running
agents, close old tabs and discard terminal screens/history. This release deliberately treats those terminals as ended
even if teardown fails, so Restart can create a second agent beside one that survived. Could-be-highest: yes,
established process/history loss. Suggested bucket: highest if explicit acceptance is rejected. Confidence: confirmed
source behavior and specification acceptance, source-only. proposed_drop: SPEC.md:3006–3010 says "This release ends
them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that
still ran in an old terminal, without needing a tmux program" and "its terminal's last screen, which lived in the
server, is gone; old tabs are closed". SPEC.md:3013–3016 says "A session whose agent or tabs already run in terminal
hosts keeps them" and explicitly accepts Restart beside a surviving old agent. SPEC_impl.md:2891–2927 specifies
retirement before reload, terminal-less old rows, Interrupted conversion for active outcomes, and preservation of ended
outcomes. Source: service/core.rs:6142–6184 ends old terminals before reload; :6675–6679 never resolves old pane
addresses; :6769–6782 reads direct generation-scoped sentinel evidence before :6805–6822 converts remaining active old
rows to Interrupted. :6635–6645 separately adopts host tabs for every session, including old-kind agent rows, so
old-agent conversion does not itself discard host tabs. Removing pane_evidence and old tab discovery from status.rs
follows that replacement contract. possible_cover: caller reports likely duplicate coverage in g099/g110/g113/g118; no
coverage sources were read here. The parent has accepted no drop; retain this as a potentially highest finding proposed
for drop and independent checking. No implementation violation is established. Original strongest consequences
preserved: terminating user processes, losing terminal content, and possible second agent after failed teardown. No
runtime execution or unresolved source premise.

Returned proposal (claims-082, event 5): **covered**, suggested bucket **highest**.

The specification explicitly accepts ending all old terminals, losing their screens/tabs, and treating them ended even
after teardown failure. The source status cleanup follows that policy. This proposed dismissal requires the independent
drop check.

Original stronger consequence: Terminating still-running user agents, closing old tabs, losing terminal screens/history,
and permitting a second agent after failed teardown. The earlier records proposed specification acceptance; none was a
parent-approved drop. All are preserved here.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact specification acceptance, not rarity, a filter, or an assertion that process/history loss is
harmless. Parent must submit this proposed covered disposition to the independent drop check before dismissal.

Existing parent disposition (event 5): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g130 record](records/review-through-00de0b6c/groups/g130.json). Record ID: `c1`.
