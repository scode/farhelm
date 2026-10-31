# Recent setup descriptions omit the stored wrapper prefix

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Different saved setups can look identical even though Enter restores and launches different wrapper prefixes.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The recent-selection summary, tooltip and accessible name omit wrapper_prefix, but equality/ranking retain it and direct
Enter restores the captured full selection. Ordinary GUI launches can create the distinct recents. This is inherited
behavior at the exclusive base. SPEC_impl.md:6233–6239 requires restoring the prefix and does not explicitly accept
hiding it from the choice description. A harmful prefix/mistaken-launch route was not established, so the confirmed
description mismatch remains other.

Source locations:
`crates/farhelm-ui/src/launch_composer.rs:746-845; crates/farhelm-ui/src/list/create_form.rs:6612-6669`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d. TLDR: Saved setups differing only in their wrapper prefix can
appear as identical recent rows, including identical tooltip and screen-reader names. Enter restores and launches the
chosen row's undisclosed prefix without a second action. Evidence: selection_summary and its tail at
launch_composer.rs:746-780 render harness, model, effort, permissions and custom permission flags, but never
wrapper_prefix. Compact detail at 828-845 omits it too. normalized_selection at 267-271 clones the complete selection
and only normalizes permission/clears trust. LaunchSelection derives full-field Eq at
farhelm-proto/src/launch.rs:572-603, including wrapper_prefix. ranked_recents at launch_composer.rs:1403-1444 therefore
retains different prefixes as distinct choices; matches_filter at 1380-1393 does not filter prefixes away. The row's
tooltip and aria label use the incomplete summary at create_form.rs:6612-6637, and its Enter handler applies the
captured entry and submits at 6645-6669. select_recent at launch_composer.rs:1548-1559 clones the selection and clears
trust only. Controller accept_recent at list/create_form/draft.rs:1874-1949 restores that selection, and restore_launch
at 1075-1095 copies wrapper_prefix into the raw wrapper draft. Submitted selections preserve it (draft.rs:788-801;
create_form.rs:517-546). This is not a live-state target race: the selected entry is correctly captured, but its
distinguishing field is absent from the control's description. Shipped producer established: the Wrapper prefix editor
at launch_controls.rs:629-651 accepts user input; ordinary successful GUI creates capture the complete selection at helm
sessions.rs:1682-1690 and pass it through accept_created_session at 1730-1740,1753-1809. store.rs:210-250 serializes
that selection without stripping prefixes, records it per session, and launch_history at 7843-7916 decodes it back into
the API result; sessions.rs:573-604 exposes those rows. Two successful ordinary launches for the same folder, harness,
model, effort and permission, with prefixes such as env PROJECT_MODE=one and env PROJECT_MODE=two, create distinct rows
whose descriptions are identical. With otherwise default values both compact rows can say Defaults, even though Enter
chooses different prefixes. No timing race or hostile host is needed for this indistinguishability. The assigned
explicit-choice and Enter tests at sidebar.spec.ts:6910-6955 and 6803-6869 use no wrapper, so they do not establish the
promised complete description for that field. Suggested correction: include the stored prefix in the complete
tooltip/accessibility description and provide a distinguishable visible indication, preserving existing Enter behavior;
no new confirmation dialog is implied. Historical provenance: omission and direct-Enter submission already exist at
exclusive base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c, launch_composer.rs:712-798 and create_form.rs:6486-6543. This
is inherited behavior discovered from the assigned tests' completeness contracts, not introduced by casing commit
08aaac8a4a5bcf91726a1d3bfcfc980de4116971. Suggested bucket: other, from the established inability to distinguish recent
configurations and inspect their prefix before direct Enter launch. Confidence: confirmed source path; no missing
factual premise for the display/application mismatch. No browser or runtime reproduction was performed. A stronger
security, isolation, wrong-machine or user-work-loss consequence is not established: it would require a supported
consequential prefix and a complete mistaken-launch/harm route beyond the ordinary environment-prefix example. Preserve
that limitation if reassessing priority; no potentially higher consequence is being proposed for exclusion.
proposed_drop: none. possible_cover: SPEC_impl.md:6233-6239 explicitly requires recent setups to use their stored
prefix, which explains restoration but does not accept omitting it from descriptions. SPEC.md:947-949 authorizes direct
Enter on a recent; it does not make different prefixes distinguishable. TRIAGE_OUTCOMES.md
template-wrapper-follows-different-agent.md at 15792-15815 accepts user-saved wrapper/agent pairing and lack of a
separate agent-wrapper approval, not indistinguishable GUI recent rows. The command-shell-meaning plan changes wrapper
interpretation, not recent descriptions. Required searches across both specs, TODO Planned (empty), BUGS.md, queue
index/relevant queue searches, plans/queue and TRIAGE_OUTCOMES.md found no exact cover. No FILTER.md rule covers this
ordinary static choice-description mismatch. Provenance: new g039 second separate discovery pass beyond f1/f2 and prior
casing/cleanup exclusions; recorded immediately upon establishing source mismatch, then finalized after
producer/history/coverage checks.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g039/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
