# Keyboard overview omits number chords' sequence action

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The keyboard overview calls number chords Launch without explaining that, in a typed sequence, they commit the
unfinished word and queue the binding.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

SPEC.md:913–917 defines the in-sequence action while the overview's action name at 2401 remains narrower. Other docs
explain the behavior, and another reviewer considers physical-key coverage sufficient; retain that interpretation as
possible coverage. No incorrect dispatch or harmful launch is established.

Source locations:
`crates/farhelm-ui/src/keyboard_shortcuts.rs:97-100,110-128; e2e/tests/keyboard-shortcuts.spec.ts:99-114`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d. Preserve prior f4 source/provenance: the generic number chord is
globally displayed as Launch, while the armed producer commits a word then queues a binding; the in-sequence reference
only describes bare digits with no word. No claim that the physical chord is wholly missing, that this plan introduced
the omission (it already exists at d702a9039b55b4612a8b056740d7d0c606187135), or that every help row must enumerate
every effect. SPEC.md:2401-2402 and :913-917 support the context/action interpretation; the website table :63-65
describes that action explicitly. Suggested bucket other; confirmed source/action difference, source/contract coverage
ambiguity preserved. Parent reports that g152 regards the existing physical-key coverage as sufficient. possible_cover
is exactly that physical-key-only reading; there is no universal help-completeness requirement beyond the cited text.
proposed_drop none, per parent retaining OTHER for synthesis. No highest route. Other details and coverage searches
remain in f4 v3.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g153/f4`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
