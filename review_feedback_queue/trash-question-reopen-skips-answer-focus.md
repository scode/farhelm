# Reopening a Trash question skips answer focus

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Reopening the same visible Trash question can leave keyboard focus on its opener, so Enter reopens it again instead of
reaching an answer.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The opener creates a new token while the unkeyed confirmation remains mounted; mount-only focus/reveal work does not run
again. Matching Dioxus source confirms the lifecycle. Answers remain reachable through keyboard navigation, and the
behavior predates the reviewed change. No unseen destructive answer was established.

Source locations: `crates/farhelm-ui/src/list/trash.rs:511,1085-1106`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d. Preserve the complete confirmed source route, coverage checks and
suggested correction in the prior f5 event. Attribution check: immutable exclusive-base
da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c already has the same onmounted-only answer focus at trash.rs:474-516 (focus at
509), always-active global opener at 964, and unkeyed already-present ConfirmBand at 985-986. Therefore f5 is a
pre-existing lifecycle defect still present in the assigned footer path, not introduced by
8996f7213020f4fd10e7c5074c7e6eeadd57649c. Current regression test trash-dialog.spec.ts:832-899 proves initial opening
and cancel/unmount/reopen; after it Tabs to Delete all at 871 it manually focuses Cancel at 874 rather than activating
Delete all, so it does not exercise same-mounted-band reopening. Source-only confidence remains confirmed for absent
focus/reveal on a replacement token; runtime behavior was not executed. TLDR: activating Delete all again while the
question is open leaves keyboard focus on that opener, and fresh Enter repeats it instead of confirming; answers remain
reachable with ordinary Tab/Shift+Tab. Suggested bucket OTHER, no security, wrong-target consent or work-loss route
alleged. proposed_drop: none. possible_cover: none exact found; existing native session-flyout reopening and
large-warning visibility records differ in trigger and consequence. Keep separate from f1/f2 and filtered f3; no new
unfinished premise.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g091/f5`, `a074/g091/f5`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
