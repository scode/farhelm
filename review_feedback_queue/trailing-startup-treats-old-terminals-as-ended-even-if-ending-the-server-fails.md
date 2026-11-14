# Startup treats old terminals as ended even if ending the server fails

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Startup treats old terminals as ended even if ending the server fails

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g109/c1`.

### g109/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. TLDR and strongest original consequence preserved: an old
agent can remain alive while startup publishes its session as interrupted; if later cleanup cannot see it, Restart can
start a second agent in the same folder, potentially causing conflicting writes and loss of user-owned work. Suggested
bucket remains high/material UX for the established fallback classification; could-be-highest is yes, with the
duplicate/conflicting-write premises explicit rather than an accepted lowering. Confidence: source-confirmed policy, not
observed execution; downstream duplicate/work-loss premises for this ID remain incompletely established. Source:
core.rs:6433-6438 marks the tmux driver absent regardless of termination result; :6965-6969 drops old terminal lookup;
:7100-7114 converts live no-sentinel old rows to Interrupted. old_terminals.rs:179-200 leaves a server alone on failed
connect/peer identification; :67-78 states the fallback. Restart's old unusable-tmux refusal is unreachable
(:12923-12933), but :12985-13010 still performs a recorded-scope/AgentOnly sweep with Refuse before relaunch, so an
ordinary visible/scoped survivor is not automatically ignored. proposed_drop: only explicit fallback acceptance, still
pending independent judgment. Exact SPEC.md:3014-3016: 'A failure to end the server or to stop what ran there is logged
and never stops the supervisor from starting; those sessions are treated as ended all the same, accepting that a Restart
could then start a second agent beside one still running.' Exact SPEC_impl.md:2998-3000: 'A failure anywhere is logged
and never fatal (the maintainer chose to start anyway and treat old sessions as ended, accepting that a Restart could
then start a second agent beside one still running)'. No parent drop or lowering accepted. possible_cover: neighbouring
g005/f1, g007/f1, g112/c1 and g149/c1 address this fallback; parent also reports g097/c1/g099/f1 retirement overlap.
Interface synthesis before discovery: (a) g007/c4 and g128/l1 identify a distinct later consent boundary, not merely the
duplicate-Restart fallback. At dbfc, core.rs:5473-5478 guards only terminal-less Launching, so Interrupted does not
refuse; :13926-14043 obtains host liveness and tmux tab counts, but startup's absent tmux driver makes the latter empty
(tmux.rs:4907-4923). teardown.rs:681-693 still sweeps WholeSession. SPEC.md:2048-2056 requires NothingAlive Delete to
refuse if any agent or tab actually runs, while SPEC_impl.md:2994-2995 leaves startup survivors for Restart or Delete.
Whether that residual waives later no-live consent is a separate independent interpretation; this ID does not propose
dropping those records. (b) g111/l1 identifies a distinct retained-borrower work-loss route. dbfc core.rs:8126-8137 and
:8173-8188 use the now-empty tmux map for claim protection; :8412-8417 then invokes cache/spare sweep. The preserved
borrower path at :12666-12681 does not require the origin's Ready state. repo_cache.rs:120-138 holds only the cache
lock, spare_take.rs:38-40 does not take it, and :156-181 moves children by retained handles then can fail final unlink
after parking. checkout_trash.rs:335-349 traverses an opened child before :374-380 checks its old name; source therefore
supports the neighbour's movement/traversal hazard rather than refuting it through pathname confinement.
SPEC.md:1749-1752 and SPEC_impl.md:4292-4299 specifically retain live/uncertain clone claims.
Failed-termination/duplicate acceptance does not expressly authorize deleting work of another host-backed writer.
Preserve g111/l1's highest route and policy question independently, with introduction at dbfc rather than the later
driver deletion. (c) c2 now separately reconciles g097/l4's conditional wrong-owner host-tab termination; accurate
marker injection is not a universal refutation. These interface checks add no new independent g109 finding. Prior
warning/guidance boundaries remain: g033/r1 warning removal and g004/g071 survival guidance are not settled by this
fallback acceptance. No prohibited coverage sources, runtime tests, reproductions or live-install actions. Revision:
completed the named cross-group synthesis before a new search while preserving the original ID, consequence, acceptance
basis and unresolved authorial interpretations.

Returned proposal (claims-070, event 10): **covered**, suggested bucket **high**.

Source establishes the fallback and the specifications explicitly accept its exact
failed-cleanup/ended-classification/possible-second-agent scope. Propose coverage only for this fallback, preserving the
possible conflicting-write/work-loss consequence and all distinct consent, warnings/guidance and cache-retention
concerns. This is not an independent drop approval.

Original stronger consequence: Old agent remains alive while shown interrupted; Restart may start a second agent in the
same working directory, causing conflicting writes and potentially losing user-owned work. The latest high bucket
reflects established misleading lifecycle state, not an accepted dismissal of the stronger consequence.

Unresolved premise or limitation: The potential highest route still needs a survivor that later AgentOnly/recorded-scope
cleanup misses while succeeding, then a second launch and conflicting writes. Ordinary scoped/marker-visible survivors
are not automatically ignored. No supported completed work-loss sequence for this ID was reproduced or established here.

Proposed basis: Propose specification coverage of the failed-retirement fallback only. Independent fresh-context
agreement is required before dropping or accepting consequence lowering. This does not cover later NothingAlive Delete
consent, wrong-owner host-tab termination, retained-borrower/cache work loss, missing warnings or inadequate survival
guidance discussed in the supplied history.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g109 record](records/review-through-00de0b6c/groups/g109.json). Record ID: `c1`.
