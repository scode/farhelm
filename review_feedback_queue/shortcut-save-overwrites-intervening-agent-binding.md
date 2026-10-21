# Shortcut saves can replace an intervening agent assignment

Reviewed commit: 5f154e79e3b681d8cbe7b6b459737b6131b0a56a

## TLDR

Saving one shortcut can undo an agent's change to another shortcut if the agent writes between the GUI's fresh read and
save. A later direct shortcut may then launch the restored template. That later launch and harmful impact are
unverified.

## Details

`crates/farhelm-ui/src/list/templates.rs:1320–1346` reads the complete binding map, changes one assignment, then queues
the whole map. For example, the GUI reads `2=B` while assigning `1=A`; an agent commits `2=C`; the GUI writes `1=A,2=B`.
Final-tip `crates/farhelm-helm/src/preferences.rs:125–150` and `store.rs:5047–5050,5098–5099` replace that field without
an expected revision. The refreshed read preserves changes completed before it; it does not make the update atomic. This
residual mechanism predates the reviewed fix. Its browser test establishes an external edit before interaction, not a
write after the fresh GET (`e2e/tests/templates.spec.ts:297–311`).

Suggested bucket: provisionally **highest** only for a later wrong-target launch with security or user-work
consequences. The established preference loss is readily recreated and would be **other** alone. Confidence: **confirmed
static interleaving; later launch, materially different targets and harmful impact unverified**. The preference write
itself creates or destroys no session. Physical Enter's displayed-name capture is a separate safeguard and is not
bypassed by this proof. A direct shortcut intentionally resolves current preferences, which makes the restored
assignment relevant.

Possible cover: reviewed `SPEC_impl.md:5023–5025` explicitly preserves GUI preference last-write-wins. The completed
`stale-template-dialog-undoes-agent-shortcut.md` decision (final `TRIAGE_OUTCOMES.md:16987–17006`) requires the fresh
read implemented here. Those are strong acceptance evidence. However, the same decision repeats the general preservation
principle now added at reviewed `SPEC.md:3232–3233` (final `3233–3239`), without a post-read exception.
Concurrent-window acceptance at final `SPEC.md:785–790` names windows rather than agents. No exact Planned, BUGS, queue
or filter match was found. The durable replacement and conditional wrong-target effect prevent an unqualified
transient-glitch exclusion.

The independent exclusion verdict is **ambiguous**; retain the contract question before commissioning an atomic-update
redesign. A separate record addresses old full-template drafts, a different write and timing boundary. Provenance:
`template-editing-implementation-1/f1`, contracts `f2`, tests `f2`, and fresh `audit-template-preference-race/a2` in the
trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `template-editing-contracts-1/f2`, `template-editing-implementation-1/f1`,
`template-editing-tests-1/f2`, `audit-template-preference-race/a2`.
