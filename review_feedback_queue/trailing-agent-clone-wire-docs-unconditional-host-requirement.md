# Clone wire documentation still forbids the new environment clone form

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Clone wire documentation still forbids the new environment clone form

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g090/c3`.

### g090/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (finding, open):

# Clone wire documentation still forbids the new environment clone form

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435.

## TLDR

Developers following the agent protocol documentation are told that clone always requires a target host and that an
omitted host is accepted only to return an error. This release introduces environment clones that require the host to be
omitted. The stale contract describes a valid request as invalid and hides the source-dependent routing rule.

## Details

At the reviewed revision, crates/farhelm-proto/src/lib.rs:1911–1914 says '`Clone` requires it (an absent one is decoded
only to be refused)'. The Clone.host field at :2077–2078 repeats: 'The target host\'s display NAME. `None` is retained
only so an old wire shape can be decoded and refused.' The variant introduction at :2064–2066 also describes only
copying to an explicitly named host. These statements must distinguish ordinary sources from environment sources.

Semantic introduction is established across the full pinned range, not inferred from unchanged comment lines: at
exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a, agent_requests.rs:1203–1215 requires
NonEmptyString::required(host, "--host") while parsing every Clone; the base tests at :4271–4278 explicitly require
refusal of None. The immediate parent of 190d844e has the same required-host parser at :1321–1323 and refusal tests at
:4495–4502. Commit 190d844e changes that parser to optional_host(host) at :1353, changes CloneRequest.host to
Option<String>, and adds the environment-source branch in clone_for_agent at :3872–3880 before ordinary-host admission.
clone_environment_for_agent at :4316–4323 rejects an explicitly supplied host, then derives the recipe/place from the
source environment row. The assigned CLI patch changes AgentCmd::Clone.host to Option<String> and sends it unchanged,
enabling this exact valid None form. SPEC.md:4326–4327 says it 'takes no `--host` or `--cwd`; it targets the source\'s
recipe at the source\'s place'. Thus the old wording was true at the exclusive base and parent, and becomes false with
the in-range feature, despite its lines remaining unchanged. At pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, the
same claims remain at lib.rs:1789–1791 and :1954–1955; no later in-range repair.

Suggested bucket: other, documentation correctness. Confidence: confirmed by pinned source, feature diff,
exclusive-base/parent comparison and governing contract; no runtime observation. proposed_drop: false for this
documentation defect. The earlier proposal to dismiss it as outside main.rs or too minor is superseded; production
correctness does not refute wrong interface documentation. possible_cover: none in supplied interface records; parent
owns coverage.

Original c3 consequence preserved: a hostless clone silently placing copied commands on another ordinary host could be
highest/security or loss of work, but that runtime concern remains source-refuted. Ordinary clone_for_agent still
refuses None after excluding environment sources; environment clones derive the source target, reject host/cwd/key
overrides, require direct approval and revalidate the source owner. No unintended-host runtime execution or work-loss
route is asserted by this documentation finding. Could-be-highest: original runtime concern yes, refuted; distinct
documentation defect has no established security or work-loss route and is other. No parent drop or lowering has been
accepted. This revision closes c3's attribution lead and retains the separate factual documentation defect under its
original ID.

Returned proposal (claims-059, event 15): **retain-verified**, suggested bucket **other**.

The in-range environment-clone feature makes the wire documentation's unconditional host requirement false. Preserve
this documentation finding separately from the source-refuted unintended-host concern. The earlier suggestion to dismiss
it as outside the assigned file or too minor was explicitly superseded.

Original stronger consequence: A hostless clone silently chooses another ordinary host and runs copied commands against
its work (highest/security or work loss). Lines 4, 9 and 13 refute that route. Lines 14–15 retain a distinct
other-priority documentation defect; no parent lowering or drop was accepted.

Unresolved premise or limitation: None for the documentation contradiction. Unintended-host execution remains
unestablished and is directly refused for ordinary clones.

Proposed basis: Retain wrong protocol documentation with exact in-range semantic attribution. Independently check the
proposed refutation of the earlier highest runtime concern.

Existing parent disposition (event 15): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-wire-Clone-host-documentation-with-original-runtime-refutation-gate. Full059 new190d
hostless environment Clone derives source recipe/place and rejects host/cwd/key overrides before approval/revalidation;
ordinary clones still require host. This refutes original ordinary host fallback/security/work route but makes unchanged
unconditional wire host-required/None-only-refused prose newly false, persisting to tip. Retain distinct other
documentation defect; earlier assigned-file/too-minor dismissal superseded. Preserve both paths and original stronger
consequence for fresh gate, not treat correct runtime as prose refutation.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g090 record](records/review-through-00de0b6c/groups/g090.json). Record ID: `c3`.
