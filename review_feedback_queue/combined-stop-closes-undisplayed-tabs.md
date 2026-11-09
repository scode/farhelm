# Combined Stop confirmation can close additional unseen tabs

Reviewed commit: d726df7d5b89ab95665b9e622b6b4310947072da

## TLDR

Confirming Stop for the agent and a displayed number of terminal tabs can also end a tab opened afterward by another
client, including its work. Whether the specification intends to accept that combined-prompt case is unresolved.

## Details

Suggested bucket: **highest**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

SPEC.md:1810-1811 explicitly accepts additional tabs only for a tabs-only prompt. Independent check agrees that narrower
exclusion and retains this combined-prompt remainder. SPEC_impl.md:4995-5001 prescribes boolean guards for all Stop
prompts, while SPEC.md:1794-1811 limits destructive consent to displayed consequences. Record this contract tension for
triage; neither rarity nor implementation prescription was assumed to settle it. No queue item, Planned entry, unlanded
plan or ledger decision was found fully covering this remainder.

## Recorded source assessment

At the reviewed revision, `list/row.rs:1240-1241` reduces the displayed snapshot to two independent booleans; the
combined question at `:2263-2303` displays a numeric tab consequence but sends those booleans. `list/lifecycle.rs:61-78`
does the same for bulk Stop. `api.rs:3414-3430` sends no displayed tab count or identities. The supervisor checks tabs
only when `refuse_if_tabs_open` is true (`service/handlers.rs:819-844`), then closes the execution-time set
(`service/core.rs:15152-15184`). Its lifecycle claim prevents opens during execution, but cannot prevent a tab opening
between the rendered question and acquisition of that claim. The source-confirmed loss route is that additional tab's
processes being ended; the unresolved issue is whether the product contract intends to authorize it.

Reviewed d726df7d5b89ab95665b9e622b6b4310947072da. Original owner remains g017/f1. New durable interface evidence
corroborates, without creating new roots: g015/s5 confirms boolean-only HTTP transport; g016/f5 confirms lifecycle
locking does not exclude tabs opened before claim acquisition and execution closes the current set; g018/s1 confirms the
same combined numeric header prompt; g019/f1 identifies assigned test stop-tabs.spec.ts:122-164 explicitly expecting the
ended=false combined one-tab prompt to remain unchanged while a second tab opens and then expecting both tabs closed.
These are owner-attributed source reports, not a claimed runtime reproduction by this reviewer. TLDR retained: combined
Stop can end an additional terminal's user-owned processes/work beyond its drawn count. Confidence confirmed source
route; highest potential work-loss consequence preserved. proposed_drop remains ONLY for tabs-only prompts under
SPEC.md:1810-1811; combined remainder has proposed_drop false and remains open. Exact uncertainty remains contract
interpretation: SPEC_impl.md:4995-5001 prescribes independent booleans for all UI Stop while SPEC.md:1794-1805 limits
destructive consent to displayed consequences; no exact combined-prompt additional-tab acceptance has been supplied.
Settling evidence remains authoritative combined-prompt policy clarification or an implementation count/identity guard.
possible_cover only the unresolved implementation prescription; no acceptance inferred from the new test's expectation
or from analogous tabs-only acceptance. No consequence reduction, withdrawal or new runtime claim.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g017/f1`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
