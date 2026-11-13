# Prepared-snapshot cleanup explicitly excludes bundled-tmux removal

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Prepared-snapshot cleanup explicitly excludes bundled-tmux removal

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g045/l2`.

### g045/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (candidate, open):

# Preparation's bundled-tmux omission is specified

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d

## TLDR

Original consequence retained: if a custom base retains ~/.local/lib/farhelm/tmux from an earlier installation but has no supervisor database or setup-managed unit, prepared environments can inherit that binary and user tmux commands can resolve it. This is a conditional source route, not an observed deployed defect or evidence that a default base carries the file.

## Details

Synthesis corrects the initial likely code-defect report. The exact specific contract in SPEC_impl.md, Terminal substrate: private tmux server, lines 2868-2871 is: "Snapshot preparation skips the step, as it skips attachment and lingering: the sandbox image it installs into has no Farhelm installation (preparation refuses setup-managed installs and copied state), so it is not expected to carry an earlier tmux." This explicitly prescribes the skip. It qualifies the general Linux Add/Update removal promise in SPEC.md:182-189; SPEC.md:385-391 likewise distinguishes cold prepared clones from ordinary rerun/Update installation. The implementation follows that explicit preparation rule at snapshot_preparer.rs:109-119, so propose dropping l2 as a code-defect report. The rationale's expectation is weaker than proven absence; this revision does not claim the two checks prove lib/tmux absent.

Actual supported-input checks: read complete shared/custom recipe, build and prepared-snapshot contracts (SPEC.md:450-609; SPEC_impl.md:5980-6263,6447-6634) and complete terminal-substrate exception (2784-2876). Custom bases are accepted if they meet systemd, passwordless sudo, Ubuntu/Debian with apt, bash/curl/git requirements (SPEC.md:483-486); arbitrary image names and user snapshot IDs are public choices. check_recipe validates image/snapshot spelling, not contents (farhelm-proto/src/recipes.rs:735-787). Helm recipe admission refuses only current/retired Farhelm-owned snapshot IDs for this issue (tensorlake/recipes.rs:158-195). create_builder passes the selected image/snapshot directly as SandboxSource (tensorlake/build.rs:1335-1375). Build readiness, packages, agent installers and final apt/machine-id/log cleanup do not remove lib/tmux (build_script.rs:52-215). Preparation refuses setup-owned units (snapshot_preparer.rs:75-90), then copied-state guard only checks state/supervisor.db absence (tensorlake_backend.rs:188-198); no private-binary absence guard exists. The prepared-clone path with saved facts then admits, delivers credentials and attaches without the ordinary install (tensorlake_add.rs:1415-1503). These checks establish that an externally supplied base with the stated files is admitted by the shipped path; they do not establish an actual vendor/user image with those contents, and no such deployment is claimed. Nothing found promises removal of deliberately retained arbitrary base programs beyond ordinary setup; the explicit preparation skip is dispositive here.

Suggested bucket of the original conditional consequence: other (unexpected inherited executable selection). Could-be-highest: no established vulnerability, credential leak or work-loss route; an obsolete executable alone proves none. Confidence: confirmed specification acceptance and conditional source path; deployed affected-base premise remains unverified and is not needed for the proposed-drop basis. proposed_drop: exact explicit preparation exception quoted above, not an assertion that the custom input is impossible. possible_cover: SPEC_impl.md Terminal substrate: private tmux server 2868-2871. Caveats: parent has accepted no drop; original consequence is preserved for its decision. g072/l1 is the same conditional route, not a new finding; g048/l1 and g057/c4 concern ordinary Add adoption/replacement of an answering supervisor and remain distinct. No runtime test or reproduction performed. No readily checkable premise remains open for this specification-based resolution.

Returned proposal (claims-028, event 15): **covered**, suggested bucket **other**.

The conditional inherited-binary route remains possible. The specific preparation contract nevertheless explicitly requires skipping bundled-tmux removal, qualifying the ordinary Linux setup/Update rule.

Original stronger consequence: A custom base retaining the earlier private lib/tmux without the database/setup-managed unit can propagate it into prepared clones, where an explicit user/agent tmux command may select it before the host's tmux. The earlier likely code-defect finding was revised to proposed specification coverage; the conditional source behavior was not withdrawn.

Unresolved premise or limitation: An actually deployed affected base is unverified. The specification's expectation is weaker than proven executable absence; absence is not the proposed-drop basis.

Proposed basis: Propose exact specification coverage of the omission, preserving the admitted custom-input route and earlier withdrawal history. Conservative independent check requested for dismissal of an inherited executable-selection concern; no actual vulnerability or user-work-loss route is established. No cross-batch alias was independently checked.

Existing parent disposition (event 15): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: explicit-preparation-skip-contract-pending-inherited-private-binary-gate. Full028 custombasecanretainprivate lib/tmuxwithoutmanagedunit/database; finalcleanupdoesnotprovebinaryabsence. Farhelmdoesnotinvokeitbutexplicituser/agentcommandmayselectoldprivatebinary. Sourceconditionalnotrefutednorrequiringolderliveupgrade. Proposedexactimplsnapshot-preparationskipqualifiesordinarysetup/Updatecleanup, weak'notexpected'claimNOTproofabsence. Freshgatecheckgoverningproduct/implscopeandpreserveoriginallikelycodefinding→speccoverrevision/custominputlimits; noconcretevulnerability/worklossestablished.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g045 record](records/review-through-00de0b6c/groups/g045.json). Record ID: `l2`.

