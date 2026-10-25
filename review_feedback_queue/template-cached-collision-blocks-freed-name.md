# Cached template collisions block a name that became free

Reviewed commit: 8b859b08299df1f9466b92fd7b65968a8bb154dc

## TLDR

After the CLI or an agent deletes a template, repeated GUI saves under its now-free name can still report a collision
without doing the fresh check. Reopening refreshes the list; the gate predates the patch.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/templates.rs:1791-1796`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit 8b859b08299df1f9466b92fd7b65968a8bb154dc; persists at pinned tip
463cac1528b5c5e259404975380d321a3fff0412 (templates.rs:1805-1810). TLDR: a template deleted through the CLI or an agent
can keep blocking a new template or rename in an already-open Templates dialog; clicking Save again never checks whether
its name is now free. Details: the resource reads when the dialog mounts (templates.rs:1603-1607); name_refusal at
1560-1572 consults that resource and save_name_refusal at 887-904 rejects a cached match. The save handler returns at
1791-1796 before its fresh fetch at 1808-1809. Error handling does not refresh the resource; retry is shown only for a
failed list resource (2258-2261), not this successful stale list. Closing and reopening or completing another local
mutation can refresh the list, but abandoning a dirty draft or doing an unrelated write should not be needed to save
under a free name. Suggested bucket other: incorrect refusal and extra user work, no data/credential/process loss or
wrong-target mutation. Confidence confirmed source mechanism; runtime occurrence unmeasured. Caveats: the cached
preflight predates the reviewed patch; only CLI/agent deletion is claimed, not unsupported consistency across multiple
GUIs. Coverage search at pinned tip checked SPEC.md, SPEC_impl.md, TODO Planned (empty), BUGS.md, queue INDEX plus
template/name searches over queue files, TRIAGE_OUTCOMES.md and relevant queued-plan search. No exact coverage found.
possible_cover: SPEC.md:3405-3416 covers other-GUI stale views only; 3418-3424 expressly supports concurrent CLI/agent
operations. TRIAGE_OUTCOMES.md:18343-18360 covers the opposite collision direction (a newly taken name being
overwritten), not a freed name repeatedly refused. Ledger template-clobber.md covers failed/pending lists allowing
overwrites, and template-catalog-error.md covers hidden read failures; neither covers this successful cached list.
template-panel-escape.md covers focus/Escape after refused launcher saves, not Templates name freshness. proposed_drop:
none. FILTER rare self-correcting glitches does not fully match a deliberate supported external deletion whose false
refusal persists on Save retries. No reproduction performed.

Provenance: `a057/f3`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
