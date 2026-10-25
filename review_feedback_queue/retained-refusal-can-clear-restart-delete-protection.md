# Retained refusal has a conditional Delete-protection gap

Reviewed commit: e36b2e90f1984d0233b01e6f314d0a85c1eec8eb

## TLDR

Retained-refusal settlement lacks a generation check and could replace terminal-less Launching with Error, weakening
Delete protection. A compatible supported validation-failure trigger across concurrent Restart remains missing; no
unconfirmed process kill is claimed.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `crates/farhelm-supervisor/src/store.rs:3609`. Locations and revisions in the assessment below identify
the reviewed source; later main is outside this review's pinned range.

FOCUSED SYNTHESIS STOPPED INCOMPLETE at reviewed commit e36b2e90f1984d0233b01e6f314d0a85c1eec8eb. The exact unresolved
prerequisite is an ordinary supported non-corruption state transition producing RetainedCreateRefusal after a
generation-zero/evidence-absent decision while a concurrent Restart can pass admission, advance generation and later
remain Launching/terminal-less. No such producer was established. New allocation-source evidence: store.rs:2911-2976
commits reservation, session, plan, fresh_checkout_id and membership together; retry takeover preserves
fresh_checkout_id in the same transaction (3952-3967). read_origin_working_copy (2789-2805) returns None/None for
consistent ordinary sessions; mismatch is not an identified product transition under held directory admission
(core.rs:1974-1983). Current-config changes do not replace the stored origin snapshot/launch, changed agent resolutions
fail before retained wrapping, and malformed named starts/prompts fail handler validation before admission. Fresh
preparation begins NotStarted, becomes Ready only in the shim after preparation; Restart rejects Planned and non-Ready
states. This does not establish impossibility across all historical server-loss scenarios, and no broad refutation of
generation-zero evidence absence is claimed. The retained UPDATE's generation omission and its conditional downstream
Delete-protection loss remain as previously cited; do not present the whole sequence as confirmed or substitute an
arbitrary transient database/filesystem error for its missing source premise. Suggested bucket OTHER provisionally,
POTENTIALLY HIGHEST only if the supported prerequisite is established. Confidence: unresolved source lead, not merely
unmeasured runtime. possible_cover: pinned TRIAGE_OUTCOMES.md:17626-17643 is ordinary rollback;18290-18304 is successful
takeover. Exact retained-refusal remainder is neither accepted nor implemented by those entries, so neither is blanket
exclusion. proposed_drop false pending independent source audit; no unilateral withdrawal. Preserve original f4 highest
assessment and every revision, plus f1-f3 unchanged. No new discovery is claimed. No reproduction, tests,
process/filesystem experiments or network access.

The retained branch rereads the current row under lifecycle ownership at supervisor service/core.rs:7766–7779, but
store.rs:3609–3611 tests only outcome, not generation. An ambiguous fresh-terminal Restart can remain Launching without
a terminal at core.rs:11734–11739,11772–11789; retained-refusal publication changes it to Error with no terminal at
14930–14941. The terminal-less check protects Launching only at 4742–4747 and 12759–12767; guarded Delete proceeds
through service/handlers.rs:1159–1182 and service/teardown.rs:435–444,579–630. Those are conditional downstream
mechanics. The supported retained-validation-failure prerequisite remains missing, as the assessment states.

Provenance: `g268/f4`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
