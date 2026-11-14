# Approved command authority is preserved; agent history effects are confirmed elsewhere

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The agent's approved command and trust survive provisioning. Agent-created environments
nevertheless change the GUI's container-host default and age its recent setups; the earlier broad default-history
refutation was incorrect. This history defect is canonical g050/l1, independently identified by g024/c3, rather than a
new discovery here.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g027/c1`.

### g027/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (candidate, open):

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435

TLDR: The agent's approved command and trust survive provisioning. Agent-created environments nevertheless change the
GUI's container-host default and age its recent setups; the earlier broad default-history refutation was incorrect. This
history defect is canonical g050/l1, independently identified by g024/c3, rather than a new discovery here.

Details: Preserve the command/trust authority refutation: private TrustResolvedLaunch moves in memory through
SessionLaunch and create_in_session_sandbox into create_session_request; agent origin avoids launch recompilation/trust
resolution and records no explicit GUI launch selection. The original possible highest consequence, silently changing an
approved command/security policy, is refuted by those checked paths. That does not prove absence of GUI history effects.
Direct source checks this turn confirm record_created_history (sessions.rs:2174-2242) destructures environment_target
independently of explicit_selection and sends both to record_create_history_with_environment_if_current. Its
current-claim closure admits a session-owned host when an environment target is present.
store/environment_history.rs:49-99 requires only target, admits NULL selection, writes preferences.last_container_host
for every novel container target and evicts older entries from the shared bounded window. The reader at 111-114 hides
NULL-selection rows but cannot undo their eviction effects. Agent origin does not reach that storage function. This
matches the complete parent-supplied g050/l1 record and preserves its attribution to integrating commit 190d844e over
the earlier history API at 52cc5d7250d262f93b730d91a07c240d33c2ea5f.

Contract: SPEC.md:5158-5163 at this revision excludes anything an agent creates from GUI future defaults/suggestions and
names recent setups. The per-launch Allow is not explicit GUI launch-choice provenance. Suggested bucket: highest for
the separately source-refuted command/trust mutation route; other for the confirmed history effects as canonical g050/l1
recommends. Could-be-highest: yes for the original authority concern, refuted only on that precise basis; no new highest
route established for remembered choices. Confidence: confirmed source checks for both command authority preservation
and the distinct history defect, no execution. proposed_drop: source refutation ONLY of changed approved command/trust;
the broader GUI-default-history refutation is withdrawn. History effects are linked as a duplicate of canonical g050/l1
and independent g024/c3, not rejected. possible_cover: those exact parent-supplied IDs, no prohibited coverage search.
Caveat: correctness of command authority does not refute distinct history, documentation or test-oracle defects.
Revision: completed direct source validation of the caller/storage provenance mismatch; no new local finding ID or
new-discovery credit claimed.

Returned proposal (claims-015, event 13): **retain-verified**, suggested bucket **other**.

Approved command/trust survives the in-memory handoff, while an agent-origin environment target independently reaches
GUI-history storage. Retain the confirmed history effect and preserve the separate proposed security-consequence
refutation for independent checking.

Original stronger consequence: Originally considered a silently changed approved command/security policy (highest), and
treating agent choices as GUI defaults. Later records correctly withdraw the broad history refutation while retaining a
narrow command/trust refutation.

Unresolved premise or limitation: No missing source premise for the history-storage effect. Actual loss of a visible
recent setup requires enough novel launches to age it out. No changed approved command/security-policy route is
established by this handoff.

Proposed basis: Record/consolidate the history defect under its shared origin-admission root. Independently check only
the proposed rejection of command/trust substitution; do not let that rejection erase the confirmed history effect.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-agent-origin-history-with-narrow-command-trust-refutation. Parent read full015.
Approved resolved command/trust is fixed before card and moved unchanged through environment-target handoff, home and
supplied resolved-mode paths, refuting only proposed silently changed approved launch. Independent NULL GUI-selection
does not prevent target-history admission, last-container place update or bounded-recents aging. Same retained020l5
canonical environment-history-admits-agent-origin and050l1 alias; preserve027/024 supporting paths. Ordinary successful
current approved agent creates contradict SPEC explicit agent exclusion, no rarity filter. Original highest
command/security possibility still needs fresh narrow source-refutation verdict; do not let it erase history defect or
merge with reminted connection claim.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g027 record](records/review-through-00de0b6c/groups/g027.json). Record ID: `c1`.
