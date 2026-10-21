# Shortcut saves can replace an intervening agent assignment

Reviewed commit: 5f154e79e3b681d8cbe7b6b459737b6131b0a56a

## TLDR

Saving one shortcut can undo an agent's change to another shortcut. After a failed GUI save, even an agent change
completed before the next fresh read can be overwritten. A later direct shortcut may launch the restored template; that
later launch and harmful impact are unverified.

## Details

`crates/farhelm-ui/src/list/templates.rs:1320–1346` reads the complete binding map, changes one assignment, then queues
the whole map. For example, the GUI reads `2=B` while assigning `1=A`; an agent commits `2=C`; the GUI writes `1=A,2=B`.
Final-tip `crates/farhelm-helm/src/preferences.rs:125–150` and `store.rs:5047–5050,5098–5099` replace that field without
an expected revision. A clean refreshed read preserves changes completed before it; a failed-save overlay is an
additional exception described below. The read does not make the update atomic. This residual mechanism predates the
reviewed fix. Its browser test establishes an external edit before interaction, not a write after the fresh GET
(`e2e/tests/templates.spec.ts:297–311`).

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

## Failed-save overlay also replaces pre-read edits

At the same reviewed commit, a failed shortcut write retains ownership of the entire dirty map
(`crates/farhelm-ui/src/api.rs:1058–1069,1299–1310,1321–1340`). A later fresh read overlays that map over the fetched
value (`874–896,1129–1138,1146–1169`). Editing another binding preserves unrelated stale entries and queues the whole
map (`list/templates.rs:33–48,1322–1346`); the helm replaces the complete field
(`crates/farhelm-helm/src/store.rs:5014–5015,5047,5079–5080`). An agent's acknowledged change before that GET can
therefore be lost too. This corrects the unqualified earlier statement that every pre-read edit survives.

Confidence: **confirmed conditional overwrite after a failed save**. Preference loss alone remains **other**; the
existing provisional **highest** bucket preserves the conditional later harmful launch, for which materially different
targets and user-work/security harm remain missing impact premises. No such launch was reproduced.

Possible cover: `SPEC_impl.md:166–170,215–241,1103–1117,5023–5025` and the existing ledger decision strongly support
whole-field local ownership and overlay reuse. They do not clearly reconcile unrelated agent assignments with the
general preservation promise. The queue tests intentionally cover whole-field overlays (`api.rs:6098–6132`), while the
browser regression uses a clean interaction. Independent auditor `g134` returned **ambiguous**, so retain this
additional timing boundary for triage rather than treating those tests as acceptance. Proposed drop: none for the
extension.

Additional provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g062/f4`, independently checked by
`g134`.
