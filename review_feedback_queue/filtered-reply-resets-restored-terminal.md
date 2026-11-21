# An older sidebar reply can reset a restored shell in a two-session fleet

Reviewed commit: 9864b874b3ac36670cb52a566ecbc08610a71430

## TLDR

An older filtered session-list reply can switch a reopened session away from its remembered shell even though the shell
is still available.

## Details

**other**. Confirmed by source and contract inspection.

Source confirmation: filters.rs:278–294 documents this ordering and the shipped controls that can produce it.
Session_pane/carried_tests.rs:269–326 models the two-session sequence, verifies both selection queries, removes the row
in the earlier result, keeps the session selected, and later returns the row outside the filtered order. The test checks
the session and details bar and has no extra terminal. On return the mounted set consumes restore against the old valid
row (set.rs:337–364,527–535,590–602). The intervening omission is treated as an empty roster and drops all extra views
and clears the choice (536,551–588). The later row reaches the same mounted set (session_pane.rs:398–434,449–465) with
restore already consumed. It rebuilds the views without choosing the shell. SPEC.md:2250–2253 promises the remembered
shell or fallback when it is gone or unavailable. Here the shell still exists and the older filtered reply is no such
evidence. SPEC.md:3194–3202 and SPEC_impl.md:400–413 distinguish filtered absence from authoritative disappearance.
Estimate other for wrong terminal staying selected until the user corrects it, and a detached/reopened client view. No
source proof of killed shell processes, unseen input destination, or runtime observation. Possible_cover:
shell-presentation/f13 is the same underlying fallback with a different confirmed trigger. Propose incorporating this
scope into that finding and deduplicating the mechanism. Proposed_drop: none for the supported-size route, and
specifically disagree with a cap-only drop for the combined finding. SPEC.md:2649–2658 at the pinned tip applies to
over-cap fleets, while this established ordering uses two sessions with no truncation. Parent set.rs:485–494,538–543
already collapsed a missing row to an empty tab list, but the reviewed feature adds automatic restoration on returning
without an extra tab click, making the restored choice directly subject to that older reply. Docs follow-up
a315f83db44cf741961ff7ed79712b92755fd0f1 changes no behavior. Pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f
preserves the reconciliation and only changes the main Shell label for shell sessions in
05ae73f23eb2cbb7ab284ca42473d66a771207a9.

The under-cap route from tabs/f11 survives even if the cap-only route is covered. Preserve the shared fallback mechanism
and both triggers in one entry.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-presentation/f13`, `tabs/f11`.
