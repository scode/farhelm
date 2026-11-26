# The close question's Discard does nothing visible when a draft changed while it was open

Reviewed commit: aba932657b4f3411be542c841fbde1dda809c44b

## TLDR

When a draft changes while the 'Save changes before closing?' question is open (for example a Codex auth.json chosen
before Close finishes loading, or a custom agent removed from another window), pressing Discard closes the question but
leaves Settings open, with no message and without moving to the page that changed. The user is left guessing why Discard
did not discard and close.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t04a-settings-save-trust` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. The reviewer proposed a possible drop citing SPEC.md's Settings close rules ("New or
changed drafts stay in Settings for a fresh action"); the independent drop check DISAGREED: that text covers the drafts
staying, not a silent no-op. No other coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/settings/editing.rs:728-737 (at aba932657b); editing.rs:804-814 at tip`

At aba932657b, editing.rs:728-737, the Discard alternative now closes only when every live edit is Dirty and permitted
by an approval captured when the question opened; otherwise the closure does nothing. Unlike the Save path, which calls
reveal_save_failure(next.section()) for each unpermitted draft in advance_batch (editing.rs ~617-622 at aba932657b), the
Discard branch neither reveals the changed section nor shows any line. Routes that change a draft while the modal
question is open, all asynchronous: a pending native file choice in the credential editor resolving after Close
(credentials/editor.rs:446-456 replaces the secret input and sets chosen_login, changing EditVersion::Credential; the
commit's own test close_discard_cannot_drop_a_later_codex_file drives exactly this and asserts nothing was sent, but
does not assert any explanation); a custom agent catalog update setting `removed` (custom_agents.rs edit_revision
includes it); an ntfy read changing stored.token_set so token_will_clear flips (ntfy.rs edit_revision includes it). If
the changed draft is on another section than the one shown (the question only reveals a section when exactly one form is
listed), the user sees nothing at all change. Fix: in the Discard closure's refusal branch, reveal the first unpermitted
draft's section (as the Save path does) and, ideally, show a line saying the draft changed after the question opened and
needs a fresh Close. Verify by reading the Discard closure at tip editing.rs:804-814 and comparing with advance_batch's
!permitted branch.

Suggested bucket: other — a confusing no-op with no data loss; the drafts are preserved, which is the safe direction.

Confidence: confirmed (source-established that the branch does nothing visible; that users find it confusing is
judgment).

proposed_drop: possibly — SPEC.md 'The close answers apply to the drafts listed when the question opened. ... New or
changed drafts stay in Settings for a fresh action.' accepts that the drafts stay; it does not say the refusal is
silent, and the Save answer does reveal the affected page, so the two answers behave inconsistently.

possible_cover: SPEC.md Settings paragraph quoted above; later range commit f76edf9e (keep newer Settings confirmations
open) touches the same close path but the tip Discard closure is unchanged.

Revision (turn 2), cross-commit check: a later range commit, b96abfbd1 (explain when a test blocks Settings from
closing), fixed the same class for one cause: a close Save held by a running test used to stall silently and now records
close_save_held and shows how to retry beside Close. Nothing in it covers a Discard (or a Save) that declines because a
draft's approval no longer matches, so the silent-Discard path remains at the tip (editing.rs:804-814). f76edf9e3 also
routes this Discard through finish_close, which now declines while a newer child dialog is open; that refusal is
spec-accepted ('If a dialog or menu opened from Settings still awaits input when the saves finish, both stay open'), and
I checked that the close question itself is already off the layer stack when its answer runs (see l2), so it does not
block Discard. The b96abfbd1 precedent weakens the proposed drop: the project treats a close answer that silently leaves
Settings open as a defect even where the drafts are kept as the spec requires. Bucket unchanged (other).
