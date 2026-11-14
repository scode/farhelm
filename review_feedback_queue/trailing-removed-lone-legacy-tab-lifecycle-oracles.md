# Deleted lone legacy-tab lifecycle regression oracle overlaps g108/c2

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Deleted lone legacy-tab lifecycle regression oracle overlaps g108/c2

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g107/g107-c4`.

### g107/g107-c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR and original consequence preserved: the intermediate
change removes the integrated regression test proving a surviving old terminal tab remains listed and blocks unconfirmed
Delete after the agent disappears, while those legacy tabs are still served. A future guard regression could terminate
user tab work, but correct current production code refutes an actual established consent bug; the verified issue is test
coverage. Prior full source basis, replacement comparisons and caveats in this ID's preceding finding remain applicable:
parent core.rs:23163–23268 is deleted at reviewed :21323; retained host lifecycle tests and pure control_codec tests do
not exercise old tmux's lone-window discovery/terminal-loss boundary; SPEC_impl.md:2956–3002 keeps adoption and marker
rediscovery supported. Interface synthesis before discovery: exact neighbour record g108/c2 independently identifies
this same deleted guard oracle and a second deleted Stop oracle (its patch:182–247, parent core.rs:23929 onward) for the
same lone surviving legacy tab with no agent terminal. Treat them as one lost legacy-tab lifecycle-oracle cause,
preserving both g107-c4 and g108/c2 provenance and both consequences (unconsented Delete if guard regresses; Stop
leaving tab work alive if inventory regresses). g108 reports production guard and Stop correct at this intermediate
revision, forged-marker tests retained, and this specific legacy test need obsolete at tip after
dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 retires old terminals and 1ed482dde220452e424973faf38a5e1318986333 removes the
driver. Those tip/Stop details are received interface evidence, not fresh source reads by g107, and do not erase the
intermediate defect. The old tab was window-marked by open_tab; no unmarked-shell cleanup premise is introduced.
Suggested bucket: other, regression coverage. Could-be-highest: original hypothetical user-work loss remains explicit;
no shipped destructive route is established and no parent lowering/drop is assumed accepted. Confidence: confirmed
source comparison for g107 guard/listing oracle; independent interface agreement for added Stop boundary. proposed_drop:
false; coordinate duplicate recording with g108/c2, retaining both IDs rather than treating overlap as dismissal.
possible_cover: g108/c2 is the same cause plus the Stop location; no prohibited coverage sources searched. No runtime
execution or reproduction. This revision adds reconciliation only; it is not a newly discovered issue.

Returned proposal (claims-069, event 8): **retain-verified**, suggested bucket **other**.

At the intermediate revision, legacy tabs remain supported while their integrated absent-agent listing/Delete oracle is
deleted. Retained host tests and pure marker-query tests do not exercise that legacy boundary. Later retirement makes
the old fixture obsolete but does not erase the intermediate deletion.

Original stronger consequence: Unconfirmed Delete might terminate user work in a lone surviving tab after the agent
disappears; the later synthesis also preserves Stop leaving tab work alive if inventory regresses. Current production
checks refute an actual removed-guard bug; the established issue is missing regression protection.

Unresolved premise or limitation: No shipped unauthorized Delete or Stop regression is established. The Stop-oracle
alias synthesis was reported by the original reviewer, and requires reconciliation with the separately owned g108/c2
record before combining files.

Proposed basis: Retain the verified intermediate test defect, other priority. Independent review must judge the
consequence lowering from possible user-work loss. Proposed same-root alias reported in the history: g108/c2; do not
discard its Stop scope when consolidating.

Existing parent disposition (event 8): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: preserve-narrow-intermediate-lone-legacy-tab-listing-delete-and-stop-oracles. Full069
production marker inventory still refuses live tabs after agent disappears, refuting actual removed consent guard, while
intermediate supported lone legacy-window listing/absent-terminal/Delete and Stop integrated oracles were removed.
Retained host/unreadable-tab tests differ. Preserve source-confirmed other test gap and original possible work-loss
allegation; later1ed retires exact subject, to reconcile with historical-retirement decisions, not demand obsolete
current fixture. Reported108c2 overlap needs actual alias join preserving Stop scope.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g107 record](records/review-through-00de0b6c/groups/g107.json). Record ID: `g107-c4`.
