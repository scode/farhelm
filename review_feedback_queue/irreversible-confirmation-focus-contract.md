# Permanent-cleanup focus has two competing defaults

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain other, contract ambiguity only.
`SPEC.md:3276–3282` still expressly lists permanent trash deletion under Confirm-first and then gives risky irreversible
actions Cancel-first; `1815–1818` still describes permanent uncommitted/unpushed loss and no restore. No exact current
coverage or later clarification resolves which category controls that example. No executable focus change or current
destructive bug is asserted.

## TLDR

The new focus rules say permanent trash deletion starts on its confirming answer, then give risky irreversible actions a
Cancel-first exception without defining which rule wins. This is a specification ambiguity, not a demonstrated change to
current deletion behavior.

## Details

Bucket: **other**. Confidence: **confirmed conflicting or ambiguous contract wording**; original suggestion other. No
new executable route to unwanted deletion, security harm or loss of work is claimed.

Introducing specification change: `6e423e30068b62c6cfa276c6d720ee35bb70387a`. `SPEC.md:3249-3252` puts explicitly opened
confirmations on their enabled confirming button and expressly includes “permanent trash deletion”. The next sentence
says a “risky action that cannot be undone” starts on Cancel instead, using YOLO as its example. “Risky” has no defined
boundary excluding the named permanent deletion. The managed-checkout contract at 1758-1760 says uncommitted or unpushed
work is permanently lost and there is no restore; 1786-1799 describes recursive permanent cleanup.

The parent specification named YOLO and Restart with as the Cancel exceptions, without this general irreversible-action
category. Existing `SPEC_impl.md:1062-1065` and `crates/farhelm-ui/src/list/trash.rs:685-690` still choose the
confirming answer. They follow one explicit product rule; this record does not label that implementation a confirmed
destructive bug or say its default changed.

Clarify either the intended irreversible-action exception or the named permanent-cleanup default so an implementer has
one acceptance target. Reading the named control as an exception to the generic sentence preserves existing behavior,
but the paragraph does not state that precedence. Source and specification checks establish ambiguity without needing a
runtime experiment; none ran.

The existing Trash-question reopening item is about a replacement token skipping mount-only focus, not which initial
answer is intended. No current ledger decision, Planned item, unlanded plan, accepted bug or whole-finding filter
settles this new wording. No existing queue extension was identified.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `irreversible-confirmation-focus-contract`;
`g001/g001-l4`, including event17 and its original lead in `prep/groups/g001/findings.jsonl`.
