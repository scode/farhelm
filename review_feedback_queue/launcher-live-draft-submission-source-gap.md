# Delayed launcher submission has an unresolved target-binding premise

Reviewed commit: da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c

## TLDR

The launcher reads current choices when submission runs. A delayed action might therefore use choices different from
those the user answered, but no complete shipped input sequence producing that contradictory accepted launch was
established. Choosing a value and immediately pressing Enter is explicitly supported and is not itself a defect.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Live reads are confirmed; an unintended
accepted target, permission bypass or work-loss consequence is not. The latter would justify `highest` only after its
complete route is established. This is context adjacent to launcher redesign commit
`09b806f7f08f436afacf0ac1f7cb867c8a7ef126`, not an established regression introduced by that change.

At the reviewed tip, `crates/farhelm-ui/src/list/create_form.rs:4830–4844` captures the destructive precondition,
restart-stop flag and launcher opening. The generic submission then reads the live draft at `4900`, live target and host
choice at `4949–4959`, and current request projection at `5051–5059`. The displayed target comes from
`5910–5923,7447–7453,7554`. The admission checks compare current target and host selection rather than comparing the
whole request with the displayed draft. That observation alone does not demonstrate an action exceeding consent.

The investigation traced actual producers and guards. Choice-and-Enter applies the focused choice before submission
(`src/launch_controls.rs:41–65`, `src/list/create_form.rs:5820–5841,6520–6543`). Host-picker selection changes the draft
without launching (`src/list/host_picker.rs:119–135,187–198`), updates placement authority synchronously
(`src/list/create_form/draft.rs:1517–1543`), and submission refuses an outdated target. Default-folder seeding requires
matching authority and preserves explicit folders (`src/list/create_form.rs:3268–3304`); pending defaults block
submission. Ordinary edits refuse while busy and the host freezes after key minting. Replace retains its source-host
check. Guarded and YOLO answers separately preserve their question, opening, request-key authority and captured
destructive precondition; their correctness does not prove whole-draft binding for generic submission.

The missing premise is a complete supported same-opening sequence: the displayed action and target, an intervening
producer, delivery of the older activation, survival of all admission checks, the accepted request, and a consequence
outside the explicitly accepted input window. Source inspection found no such sequence. A source-complete dispatch
argument or an event/paint trace with its admitted request would settle it. No runtime reproduction ran. Do not infer a
wrong-host operation from a live read or require a universal proof that arbitrary event schedules are impossible.

Possible cover: `SPEC.md:684–691` requires fresh same-event choices. Its `3580–3583` acceptance permits sub-second
pre-redraw actions as delivered while preserving named guards. The exact decisions at
`TRIAGE_OUTCOMES.md:8562–8586,16402–16417,18840–18854` cover particular delivered-input variants; they do not clearly
settle every automatic mutation or delayed confirmed action. `SPEC_impl.md:918–947` requires answered controls to retain
what was drawn. These rules can coexist; no contradiction between the specifications is asserted. The existing
source-supersession, initial-host-fallback and template-batch records have different triggers. Planned is empty, the
unlanded sentence-case plan does not change this authority, and BUGS supplies no coverage. Proposed drop: none for the
conditional remainder; independent auditors rejected a blanket exclusion. If confirmed, preserve required fresh
choice-and-Enter behavior while binding or refusing the contradictory action.

Provenance: trailing-review run `2b7a8eaf-c4e9-42ab-be93-1acf29106132`, `g11/f2`, `g02/f2`, `g03/f2`, `g04/f3`,
`g05/f5`, `g13/f6`, and the scoped `g14/f2` interface correction; independent checks `a01/f4`, `a02/f1` and `a03/f18`.
These are one investigated concern, not multiple defects.

## Additional review at d6645f77

Additional reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

The current container-choice fields pass through the same live draft projection. Independent review found no distinct
complete shipped input sequence establishing a contradictory accepted container launch, so this adds provenance rather
than a new defect count. Invalid enabled-state projection, dormant Command badge and Restart container-edit routes
retain their specific source refutations; those do not prove whole-draft render binding. The existing missing compatible
activation/admission premise, other priority and investigated-uncertainty status remain unchanged.

Source and provenance, run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`:

- `g023/f1`: `crates/farhelm-ui/src/list/create_form.rs:4985-5084`.
- `g036/f2`: `crates/farhelm-ui/src/list/create_form.rs:4915-4985,5050-5061,5152-5160`.
- `g120/f1`: `crates/farhelm-ui/src/list/create_form.rs:4985-4997,5572-5588`.
- `g123/f3`: `crates/farhelm-ui/src/list/create_form.rs:4981-4997,5572-5588`.
- `a002/f5`: `crates/farhelm-ui/src/list/create_form.rs:4985-5027,5572-5629`.
- `a005/f4`: `crates/farhelm-ui/src/list/create_form.rs:4981-4997,5572-5588`.
- `a025/f2`: `crates/farhelm-ui/src/list/create_form.rs:4905-4985,5050-5119,5152-5161`.

Proposed drop: none for the retained remainder. No runtime reproduction, fix or triage decision was performed.
