# Sentence-case sweep leaves authored lowercase text

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Several ordinary refusal messages and the no-alias label retain lowercase wording after the sentence-case change.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Retained sites are Restart search refusal formatting, uninstall-plan decode notices, the authored no-alias fallback and
file/link download/open refusals. Dynamic peer text is a different case. The errors' rarity is not established for all
these ordinary paths, so the rare-diagnostic filter is not an exact whole-finding match. This is presentation
consistency, without an authorization consequence.

Source locations: `crates/farhelm-ui/src/list/create_form/draft.rs:2536-2539`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis complete. Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d; assigned change
08aaac8a4a5bcf91726a1d3bfcfc980de4116971, where the same omission is at draft.rs:2471-2474. TLDR: Restart search still
displays lowercase notices while adjacent static refusals now use sentence case. The direct search branch at pinned
draft.rs:2519-2540 maps Harness or incompatible Model to 'agent type', Host to 'host', folder/path/repository/recent
results to 'destination', and Name to 'session name', then inserts that authored lowercase label at the beginning of a
standalone sentence. The ordinary host:local search route remains established by
create_form.rs:4274-4295,6283-6312,6529-6533,4415-4427 and launch_composer.rs:1044-1072. create_form.rs:6270-6271
renders the refusal through display_peer without capitalization. The original sentence-case contract, read at pre-change
full commit cc734789f2ce77155ad2838eea7771bd52fe240c:plans/queue/steel-sentence-case.md:163-168,184-187, explicitly
includes authored notices and preserves user/protocol values. The same contract remains at the tip. This is one producer
omission covering the four authored field labels, not a separate defect per field. Shared template producers are
correctly excluded: draft.rs:744-783 constructs TemplateRefusal; proto launcher.rs:586-605 starts the complete sentence
with 'Template', and its generic field/reason clauses (631-905, including the fixed-host refusal at881-895) are inside
that sentence. create_form.rs:704-719 preserves the complete sentence, including its separate capitalized
GitHub-destination wrapper. This distinction also holds in the assigned commit's proto launcher.rs:562-580. Template
tests' lowercase substring assertions at draft.rs:3322,3392,3429 concern inner clauses and do not establish another
casing omission or test defect. FILTER.md:28-34 requires both a rare trigger and a listed consequence. Its trigger
requirement is false here: an enabled shipped search supplies the input, with no crash, interruption or timing window.
The diagnostic is accurate and complete; the demonstrated defect is authored sentence casing, not imprecision,
misleading detail or loss. No filter match is established. SPEC.md:1373-1393 and SPEC_impl.md:687-704 accept refusing
held edits but provide no casing exemption. Coverage searches of these specs, TODO Planned (empty at23-25), BUGS.md,
TRIAGE_OUTCOMES.md, queue/index and plans found no matching known omission, accepted exception or unlanded repair; the
sentence-case plan landed in the review range. Suggested bucket other; confidence confirmed from source; proposed_drop
false; possible_cover none. The refusal remains a refusal: no changed target, permission semantics, action consent,
security harm or loss of user work is alleged. No tests, builds, experiments, images or private historical runtime
evidence were independently authenticated in this source-only review. A focused direct Restart-search notice assertion
would distinguish this omitted standalone producer from the correctly capitalized template wrapper.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g025/f1`, `g043/f1`, `g046/f1`, `g063/f1`.
These are original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and
independent verdicts remain in the private review evidence. No fix or triage decision was executed.
