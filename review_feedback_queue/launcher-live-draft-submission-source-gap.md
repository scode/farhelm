# Pending Resume can launch edited permission flags

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

While a resumed host's checkout preview loads, editing Custom permission words can automatically launch the edited agent
without a new Launch press. Supported edits can change guarded vendor settings into permission bypass. This specific
source-confirmed route makes the item highest priority; the older generic target/container concerns below remain
unverified after investigation.

## Details

Source coordinates from the original records: `crates/farhelm-ui/src/list/create_form.rs:3689-3715,6373-6415`;
`SPEC_impl.md:6191-6192; crates/farhelm-ui/src/list/create_form.rs:3689-3715,6373-6415`. References below use these
paths' basenames unless another path or revision is stated.

### Confirmed pending Resume case

Additional reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

While a resumed host's checkout preview loads, editing Custom permission words can automatically launch the edited agent
without a new Launch press. Supported edits can change an agent from its guarded vendor mode to bypassing permission
checks.

This specific permission-widening route is **source-confirmed and highest**; the earlier generic live-draft concern
remains **unverified after investigation** under its original scope. The new route supplies a compatible editor, waiting
boundary, accepted request and permission consequence for this Resume case. It does not retroactively establish every
earlier suspected target or container sequence.

Completed later-turn synthesis. Concrete supported route: choose Claude/Custom with --permission-mode auto, press Launch
for a fresh checkout on a suspended host, then after Resume releases its operation claim but before the fresh preview
finishes, replace the Custom text with --dangerously-skip-permissions. Both are permission-only words.
LaunchHarness::guarded_flags at proto/launch.rs:350-365 identifies the former as selecting the automatic guarded vendor
mode; helm/launches.rs:444-458 uses the latter as Claude's shipped YOLO flag. offers_permission at
proto/launch.rs:454-460 permits Custom for Claude.

The parser at proto/launch.rs:830-859 checks quoting, nonempty words, NUL and the Custom/vector relationship, not
meaning. helm/launches.rs:724-738 additionally refuses reserved placeholders, absent in both vectors. Its 433-470
compiler applies no Guarded mode in Custom and appends the exact supplied vector; 792-798 confirms the harness offers
the choice. SPEC.md:812-820 explicitly delegates permission flag choice and defines Guarded as a safety mechanism versus
YOLO's absence of checks. SPEC_impl.md:5174-5185 confirms that custom words occupy the permission slot and are not
interpreted by Farhelm. This example stays within that contract; it does not depend on abusing Custom to inject another
command.

The accepted press binds create_form.rs:5350-5355/5384-5385 to preview_agent_now, whose 3689-3715 fingerprint omits
custom words. Resume drops the operation claim at 5397. The editable Custom input at launch_controls.rs:704-709 reaches
draft.rs:1518-1519; apply_edit at 1420-1430 admits it once the claim ends, retiring only intent_key. On Ready, 6373-6415
still accepts the stamp and ordinary submit snapshots the edited draft at 5206. SPEC_impl.md:6191-6192 says draft
changes cancel this pending press.

The supervisor handoff preserves the compiled permission vector: service/core.rs:2012-2026 selects launch.start_argv;
prepare_launch keeps non-wrapper argv as the executable vector at 16924-16925 and publishes it unchanged in
LaunchSpec.argv at 16988-17008. No late Custom-policy guard restores the pressed draft. Reserved-placeholder and
shell-literal protections remain intact, so this is a temporal consent/agent-permission boundary bug, not shell
injection.

Both Custom choices count as YOLO to Farhelm; that conservative classification does not authorize changing the exact
words after the press. On new Tensorlake hosts the host-level Start YOLO sessions without asking setting is on
(SPEC.md:320-321), so no later host YOLO question closes this route. On a host that still asks, that independent
confirmation remains a boundary and the scenario must account for it; the established default-Tensorlake route needs
none.

Suggested bucket: highest. Source proves a supported edit can replace an accepted guarded vendor mode with the
permission-bypass mode across the required fresh-launch consent boundary; no hypothetical later data destruction is
needed to establish the security consequence. Confidence: confirmed from source through the actual command handoff; no
runtime/vendor/endpoint experiment performed. proposed_drop: false. possible_cover: none. Fix by including effective
custom words in the pending submission identity (or retiring the pending press on semantic Custom edits), retaining the
intentional naming/configuration allowances.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g006/g006-l1`,
`g008/d2`. The retained append-only histories preserve all earlier claims and corrections. Reviewed source references
name their own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no
product runtime experiments or live-install changes.

### Earlier generic live-draft investigation

The following earlier record retains its original reviewed revisions, other-priority assessment and missing supported
input sequence. Those qualifications describe that broader investigation, not the newly established pending-Resume
permission route above.

Reviewed commit: da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c

#### TLDR

The launcher reads current choices when submission runs. A delayed action might therefore use choices different from
those the user answered, but no complete shipped input sequence producing that contradictory accepted launch was
established. Choosing a value and immediately pressing Enter is explicitly supported and is not itself a defect.

#### Details

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

#### Additional review at d6645f77

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
