# Legacy Muse hook preservation has an unresolved source gap

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A broad legacy hook-preservation claim remains uncertain for an unrecognized override spelling and destructive vendor
replacement behavior.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Recognized explicit override spellings preserve their narrow independent source-refutation. The broader claim still
needs a supported Legacy Muse producer, vendor acceptance of the uncaught -c= spelling and destructive replacement
semantics. Actual source/documentation checks did not establish that combination. Do not promote it to an observed lost
hook or conflate it with current duplicate-hook behavior.

Source locations:
`crates/farhelm-supervisor/src/agent_kind/muse.rs:47-64; crates/farhelm-proto/src/session_launch.rs:268-300,412-447`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. Original provenance is now mapped to repaired g094/f7 from
immutable g094/turn3-result.txt:21, preserving the earlier reply-line identity. This is a refinement of the existing
audit limitation, not a new product finding. Independent verdict remains bounded agree for the recognized forms; no
proposed drop for a universal helper-preservation claim. Exact established source: -c=hooks={} does not match this
helper because stripping -c leaves =hooks={}, whereas the six tested separate/attached forms refuse injection and
preserve original argv. Suspected consequence, only if all missing premises hold, is replacement of a caller-owned
security hook. Focused producer checks found historical Muse mapped to Generic, legacy migration preserves the old kind
or successfully converts the row to Agent with an explicit placeholder, and new create rejects Legacy. Current
Agent/Command spawns choose Expand rather than Legacy. Additional store constructor search showed store.rs:2678 is a
pattern in decode_session_row, not another constructor; :2656-2685 decodes an already stored tagged launch and validates
it. The exclusive-base AgentKind enum had no Muse variant. Thus no supported route producing a current Legacy/Muse row
was established by the inspected normal creation/migration/readback paths; this is source-specific evidence, not an
assertion that every conceivable stored state is impossible. Other missing premises are vendor acceptance of an uncaught
spelling and destructive replacement rather than rejection/merge. The readable repository sources and synthetic helper
tests do not include the closed vendor parser. Earlier local read-only inspection found a forwarding launcher and
stripped native runtime, no readable implementation in inspected conventional locations; no vendor code, user settings
or credentials were accessed. Source-only authorization cannot settle vendor behavior through an experiment. Evidence
that would settle a harmful allegation is a supported LegacyMuse-producing history, version-matched parser
documentation/source or real accepted argv, and hook-resolution semantics showing loss of the caller-owned hook. No
current security impact is claimed. Suggested bucket other for the established limitation; potentially highest only with
that full route. Confidence confirmed helper/migration/decoder facts; consumer and harmful producer premises unverified.
proposed_drop: recognized-form source refutation only, none for universal preservation or any demonstrated security-hook
loss. possible_cover: SPEC.md:762-773 governs explicit structured placeholders; SPEC_impl.md:3470-3476 requires
legacy-hook preservation but does not accept its failure. Codex-only ledger :8425-8435 is not Muse coverage. No filter
exclusion used. Parent retains any broader allegation as this investigated conditional uncertainty rather than treating
missing vendor evidence as proof of safety.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a001/a001-f9`, `g094/f7`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
