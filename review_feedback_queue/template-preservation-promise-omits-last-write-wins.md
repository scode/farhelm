# The agent-edit preservation promise omits template exceptions

Reviewed commit: 5f154e79e3b681d8cbe7b6b459737b6131b0a56a

## TLDR

The new promise that GUI saves preserve agent edits conflicts with accepted template behavior. A user relying on it can
save an old GUI draft over an agent's saved command or prompt. Loss of nontrivial work from that reliance is
conditional.

## Details

Added `SPEC.md:3232–3233` states the preservation rule without an exception. The same revision's `SPEC.md:60–63` and
`SPEC_impl.md:5008–5014,5023–5025` explicitly accept full-template last-write-wins and unconditional GUI writes.
`crates/farhelm-ui/src/list/templates.rs:1432,1456` captures and saves the full earlier draft; `src/api.rs:3945–3962`
serializes it, and `crates/farhelm-helm/src/templates.rs:38–56,68–84,118–142` uses unconditional replacement. Open a
template, complete an approved agent command edit, then save an unrelated GUI field: the old command is sent again. This
needs no narrow race. The implementation is pre-existing and expressly accepted; the new defect is contradictory
wording. State the intended exception or settle the policy deliberately, rather than treating this record as an
instruction to redesign accepted runtime behavior.

Suggested bucket: provisionally **highest** for a preservation claim that could lead to loss of nontrivial saved command
or prompt content. Confidence: **confirmed contract conflict; consequential reliance and unrecoverable nontrivial
content unverified**. Trivial or independently retained edits do not establish that consequence; the wording conflict
alone is **other**. No automatic execution, approval bypass or running-session loss was established.

Possible cover: the explicit template exceptions above do cover the runtime overwrite. The fresh independent
`audit-template-field-contract` agrees with excluding that runtime complaint but disagrees with dropping the conflicting
promise. Final `TRIAGE_OUTCOMES.md:16987–17006` concretely concerns fresh shortcut reads, not template-field merging.
`template-clobber.md:6944–6958` concerns unavailable name lists, a different trigger. No exact Planned, BUGS, queue or
filter match settles the wording. The separate post-GET shortcut record retains its own mechanism and ambiguity.

Provenance: contracts `template-editing-contracts-1/f1`, implementation `f2`, tests `f1`, and the independent
field-contract audit in the trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction
was performed.

Original finding IDs: `template-editing-contracts-1/f1`, `template-editing-implementation-1/f2`,
`template-editing-tests-1/f1`, `audit-template-preference-race/a1`, `audit-template-field-contract/f1`.
