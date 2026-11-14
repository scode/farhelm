# Removing the legacy container editor is specified retirement

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The old Containerized editor disappears, but this is the intended retirement rather than an
accidental loss of container isolation. Considered consequence: applying a stored setup after editor removal could
silently run previously isolated work on the host (could-be-highest: yes, if that route existed). Source checks: the
entire deleted module was read; at this revision create_form/draft.rs:222-232 preserves the old ContainerConfig and
exposes an explicit refusal, :747-750 refuses structured projection, :1965-1974 and :2650-2656 reject old recent setups
before adopting their destination. list/templates.rs:1237-1239 displays the retirement explanation rather than an
editor. Contract: SPEC.md:4817-4824, Retired containerized sessions, explicitly says: 'Templates, recent setups and
history with that choice still load. Using them drops or refuses the retired choice with an explanation and cannot
create another old-style session. The old launcher switch and account-selection flow are gone.' SPEC_impl.md:8647-8656
explicitly requires removal of launch fields and switches after retirement. Suggested bucket: none, not a defect.
Confidence: confirmed by source and explicit contract; no behavior executed. proposed_drop: editor removal explicitly
specified, and examined stored-choice interfaces preserve/refuse rather than silently projecting the old structured
launch. possible_cover: none; no coverage sources searched. Caveat: this does not independently certify every launcher
path, which is outside this group's deletion scope. Synthesis revision: the refusal is used by actual submit

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g078/c1`.

### g078/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (candidate, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc. TLDR: The old Containerized editor disappears, but this is
the intended retirement rather than an accidental loss of container isolation. Considered consequence: applying a stored
setup after editor removal could silently run previously isolated work on the host (could-be-highest: yes, if that route
existed). Source checks: the entire deleted module was read; at this revision create_form/draft.rs:222-232 preserves the
old ContainerConfig and exposes an explicit refusal, :747-750 refuses structured projection, :1965-1974 and :2650-2656
reject old recent setups before adopting their destination. list/templates.rs:1237-1239 displays the retirement
explanation rather than an editor. Contract: SPEC.md:4817-4824, Retired containerized sessions, explicitly says:
'Templates, recent setups and history with that choice still load. Using them drops or refuses the retired choice with
an explanation and cannot create another old-style session. The old launcher switch and account-selection flow are
gone.' SPEC_impl.md:8647-8656 explicitly requires removal of launch fields and switches after retirement. Suggested
bucket: none, not a defect. Confidence: confirmed by source and explicit contract; no behavior executed. proposed_drop:
editor removal explicitly specified, and examined stored-choice interfaces preserve/refuse rather than silently
projecting the old structured launch. possible_cover: none; no coverage sources searched. Caveat: this does not
independently certify every launcher path, which is outside this group's deletion scope. Synthesis revision: the refusal
is used by actual submit paths, not only a passive editor helper: create_form.rs:5863-5866 refuses before initial
structured launch; :6830-6835 repeats that refusal after asynchronous key minting; :9189-9193 refuses saving it as a new
template. proto launcher.rs:667-669 rejects any template carrying a non-null stored container object before applying any
field. The apparently dropping branch create_form.rs:759-764 only removes a template field matching the already-held
restart value; the held draft still retains its refusal. Reset choices is an explicit user action (:9166-9177); its
implementation clears the selected harness along with the old container choice (draft.rs:1090-1099), requiring a fresh
agent choice, while a restart restores its retained baseline (:2268-2276). This extends the source refutation but does
not certify unexamined launcher paths or remove the original potentially highest consequence. proposed_drop remains
pending independent check; no drop accepted, no execution performed. Discovery pass 1 additional removal checks: re-read
the full deleted module from the parent revision, its module registration diff, and LaunchControls integration diff at
04d85f0c98a9b635709d00b056719a0baa7fd3fc. The deleted module declaration and its prop/control mounting are removed
together (lib.rs:145 vicinity; launch_controls.rs former :130-132 and :653-663). A pinned git grep for
ContainerDraft/ContainerControls/ContainerEdit/container_controls/Containerized/Turn off containerization across UI
source finds only the intentional retired Containerized field label in list/templates.rs:554, not an active old toggle
or handler. No independent defect in retired implementation was sought. No new finding; the existing original
potentially highest host-execution concern and source refutations are preserved, with no accepted drop.

Returned proposal (claims-050, event 11): **covered**, suggested bucket **other**.

Removing the legacy editor is explicitly required. The examined stored-choice paths refuse the retired isolation choice
instead of silently projecting an ordinary agent launch. The original potential isolation-loss consequence still
requires an independent drop check.

Original stronger consequence: Applying a stored setup after editor removal might silently run previously isolated work
on the host, potentially highest if established. Every history event preserves that concern and proposes
retirement/source-refutation only; no drop was accepted.

Unresolved premise or limitation: The stronger consequence requires an unexamined stored-choice path that silently
launches previously isolated agent work on the host. The paths checked here refuse it; that is not a certificate for
every launcher path.

Proposed basis: Propose explicit specification coverage for editor removal and narrow source refutation for the
inspected stored-choice bypass hypothesis, preserving the all-path limitation.

Existing parent disposition (event 11): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-retired-editor-removal-cover-and-bounded-stored-choice-refusal. Full050 explicit
retired supervisor-outside model removes old editor/module/wiring. Actual captured draft retains retired choice/refusal,
recent/template paths reject before destination or field application, structured submit/key-mint/save gates refuse;
explicit Reset choices clears agent with choice, redundant restart-field stripping does not remove held refusal. Cover
editor removal and refute named silent outside-host launch consumers only, not every launcher path. Preserve original
stored isolation/security possibility and explanation requirement for fresh gate.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g078 record](records/review-through-00de0b6c/groups/g078.json). Record ID: `c1`.
