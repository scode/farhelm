# Reselecting the current agent erases unfinished permission flags

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

Clicking or pressing Enter on the already selected agent erases custom permission flags containing an unfinished quote
in New, Clone and Replace with.

## Details

The agent-chip callback parses the raw editor and then unconditionally reconstructs it from parsed words
(`crates/farhelm-ui/src/list/create_form.rs:4361–4430`, especially `4415–4421`). For example,
`--permission-mode 'ask me` produces no parsed words and becomes an empty string even though the agent and permission
mode are unchanged. Preserve raw text when both remain unchanged, as the search handlers already do.

Suggested bucket: **other**. Confidence: **confirmed statically**. This loses unsaved form input, which the queue's
priority rule excludes from user-owned-work loss. Invalid Custom flags cannot launch. Restart with has a fixed harness
and its callbacks preserve raw flags, so it is outside this finding. No runtime reproduction.

Possible cover: `TRIAGE_OUTCOMES.md`, `custom-permission-model-choice-erases-draft.md` (`14240–14269`), records the same
wanted behavior and conversion mistake for the two composer-search arms. Its explicit complexity gate limits the fix to
`apply_composer_search_result` and its tests; this chip callback remains separate. The broad completion phrase
“current-harness re-selection” creates possible overlap, so retain this as residual evidence for reopening that issue,
not as a new product-policy question. SPEC_impl.md:2943–2956 explicitly keeps malformed quotes repairable. No current
queue, Planned or BUGS match covers the remaining callback.

Proposed drop: none; duplicate reports are combined here. Provenance: persistent Astra/high review,
`ui-new-session-1/f3` and `ui-launch-workflow-1/f2` (the latter's lead was supplied by the parent, not independently
discovered).
