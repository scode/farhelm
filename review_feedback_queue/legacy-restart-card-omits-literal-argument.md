# An older session's Restart card can omit an executed argument

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

A Restart approval for an older session can omit a literal word that its saved resume command still executes. The card
then describes different arguments from those being approved. Whether a released older writer could create the exact
unusual command, and whether the omission could cause a harmful approval, remain unverified.

## Details

Found by persistent reviewer `A21-card-meaning`, finding `A21-F3`; independently assessed for the proposed drop and
checked against source by the recording agent. Suggested bucket: other for the established disclosure mismatch,
potentially highest if the omitted word changes security-relevant execution. Retained provisionally in highest because
that boundary is unresolved. Confidence: confirmed static omission and execution retention, likely reachability with the
historical-writer, usable-conversation and harmful-effect premises unverified. No runtime work was performed.

`crates/farhelm-ui/src/approvals.rs:990-994` filters every whole argv element equal to `{farhelm_args}` from the shown
resume command. Both Agent and Legacy branches use it (`1060-1068` for Legacy). Agent treats this as an integration
slot; Legacy treats it as an ordinary literal word.

Old non-structured rows retain their resume vector through `SessionLaunch::from_pre_launch_kinds`
(`crates/farhelm-proto/src/session_launch.rs:245-277`) and the schema migration
(`crates/farhelm-supervisor/src/store.rs:2294-2355`). `resume_argv` returns that vector unchanged
(`session_launch.rs:437-447`). Legacy load checks (`store.rs:2635-2705`) call executable and resume validation
(`agent_kind/mod.rs:2305-2320,2375-2392`), neither of which reserves this literal. New Legacy creates are refused
(`session_launch.rs:402-406`), so this finding requires old carried state.

Restart fills the conversation slot while retaining other arguments (`agent_kind/mod.rs:1931-1968`) and chooses Legacy
injection (`service/core.rs:2638-2646,2683-2702,11935-11938`). Claude's old tail-injection path preserves the vector and
may append hooks (`agent_kind/mod.rs:712-753,1242-1243`). The helm's card and expected-launch precondition share the
stored launch (`crates/farhelm-helm/src/agent_requests.rs:569-624`); equality prevents later substitution, but cannot
recover a word the renderer removed. The narrow correction is to remove the integration slot only for Agent and display
Legacy argv literally.

An omitted literal can change positional-argument meaning. Current Legacy validation even permits this unusual word as
the executable, which the card would hide. Neither a reachable program with that name nor a harmful command was
established. Ordinary generated resume commands are not claimed to contain it. The user must approve Restart; there is
no new automatic execution capability or same-account isolation claim. Cross-host reliance on misleading consent is the
unresolved security concern.

Possible cover: FILTER.md, "Upgrades from older releases combined with another rare condition", covers the ordinary
old-state disclosure mismatch and even session-local work loss, but excludes security/trust-boundary consequences. The
independent reviewer did not agree those consequences were refuted, so the candidate is retained for triage.
SPEC.md:2330-2346 and SPEC_impl.md:5192-5234 require preserving legacy commands; neither accepts omitting literal argv
on today's Restart card. SPEC_impl.md:5626-5641 requires the card to show the stored launch.

Ledger `yolo-guard-skips-resume-template.md` (`4971-4990`) documents historical hand-written profiles and raw API resume
vectors, but proves neither admission of this exact token nor acceptance of this omission. The old hook-collision
discard (`8425-8435`) is a different consequence. `legacy-launch-validation-could-prevent-startup.md` (`12353-12363`)
concerned a state combination known to have been refused by its writer; the exact writer premise here remains open. No
matching Planned item, BUGS entry or queue item was found. Proposed drop: not agreed by the independent reviewer; retain
the unresolved consequential approval gap with these limits.
