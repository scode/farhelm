# lone surviving old tab loses its guard and Stop regression oracles

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: lone surviving old tab loses its guard and Stop regression oracles

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g108/c2`.

### g108/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (finding, open):

# retain the lone-tab lifecycle oracle while old terminals are still served

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513

## TLDR
This intermediate commit still serves old tmux terminals, but deletes the tests proving that a surviving shell tab is found and protected after the agent's window/terminal has disappeared. General host-tab tests exercise the consent rule without the legacy discovery boundary that originally hid such tabs. No actual destructive regression is established; the issue is removing that meaningful regression oracle while the behavior is still required.

## Details
Assigned patch lines 1–97 deletes the_delete_guard_sees_a_lone_tab_after_the_agent_window_is_gone, and lines around 182–247 delete stop_closes_a_lone_tmux_tab_without_an_agent_terminal (original core.rs:23929 onward). The deleted fixtures made a real old agent and tab, removed the agent pane, established that the tab alone kept its tmux session alive, and then checked authoritative listing, NothingAlive refusal, AgentEnded/Unconditional allowance, the terminal-less entry case, and Stop closing the actual surviving tab. These are distinct from a merely stopped agent whose final-screen terminal remains.
The surrounding interface makes this boundary material: tmux.rs:4859–4947 conditionally omits the separate marker query, and :4953–4972 defines the unconditional-marker variant specifically for a surviving lone tab. still_alive_for_delete and tmux_tabs_for_stop must choose the authoritative inventory, including without an agent terminal. Current core.rs:13964–14095 and :14214–14239 do choose it correctly; that is source refutation of an actual destructive bug, not replacement test evidence. In tmux.rs tests, searches for pane_states_with_markers/pane_states calls find no corresponding successful lone-tab guard/Stop fixture. control-codec/pure tab-selection tests can check decoded fixed inputs but not this caller/query/terminal-loss interaction.
Meaningful retained general coverage was checked: e2e/session_lifecycle.rs:3646 onward stops a host agent, then opens a host tab and verifies NothingAlive refusal and later deletion; :3710 onward verifies AgentEnded allows closing tabs. core/tests/host_terminals.rs:333–381 makes a terminal-less entry with a host tab that becomes unreadable and checks uncertainty refusal. These protect the generic consent contract but cannot catch the old tmux marker-query selection regression. terminals.rs:3897–3946 does meaningfully cover forged-marker exclusion from a recorded agent window; ticker.rs:3889 onward also retains an automatic-reap case. Thus the strongest remaining c2 coverage issue is the deleted lone-tab inventory/lifecycle boundary; no separate new forged-marker finding is asserted.
At reviewed revision SPEC_impl.md:2954–2981 expressly still adopts/serves old terminals and SPEC.md:2063–2067 forbids deleting live work beyond the confirmation. Later dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 ends old terminals at startup and 1ed482dde220452e424973faf38a5e1318986333 removes the driver; pinned tip SPEC_impl.md:1888–1909 says every served terminal is a host and old terminals are ended at startup. Therefore this particular legacy discovery oracle is obsolete at tip, but was not obsolete at the reviewed intermediate commit; review charter includes such intermediate changes. The replacement should have kept a driver-built/adopted old-session fixture until old terminals stopped being served, or recorded this as an explicit transitional test coverage loss.
Suggested bucket: other (regression coverage). Confidence: confirmed source comparison only; no runtime test or reproduction. proposed_drop: false; previous proposed drop based only on retained correct guards is superseded. possible_cover: g107/c4, which the parent reports separately considers this exact deleted lone-tab guard test; coordinate/deduplicate with that record, do not silently dismiss the concern. Could-be-highest: preserve the original possible consequences of an unconfirmed Delete killing a live tab and Stop falsely leaving its work alive; no shipped failure route has been established here, so the finding remains test quality rather than actual security/loss of work. Caveats: this is an intermediate-only legacy-test gap; no broad absence of host-tab consent coverage is claimed. Pure forged-marker coverage remains and was actually read. Parent has accepted no drop; keep the strongest original concern for independent adjudication.

Returned proposal (claims-070, event 15): **retain-verified**, suggested bucket **other**.

The intermediate revision deletes a meaningful lone-surviving-tab discovery oracle while still serving adopted tmux terminals. Correct retained guards refute an established production regression, not the test-coverage finding. The mechanism is obsolete at the reviewed tip; preserve that historical limitation. The earlier proposed destructive-regression drop was superseded in the record.

Original stronger consequence: Unconfirmed Delete killing a live tab, forged tab identity killing the agent, or Stop leaving live tab work behind. No such production route was established; forged-marker production/pure-test protection survives and the latest record narrows to the lone-tab oracle.

Unresolved premise or limitation: No missing premise for the bounded deleted-oracle comparison. No shipped destructive failure is established. Parent-reported g107/c4 may alias the Delete portion, but it is outside this batch and was not independently read; do not infer deduplication here.

Proposed basis: Retain the historical test-quality finding, with current obsolescence stated. Independent judgment is still needed for the earlier destructive-route refutation/consequence narrowing; this consultation does not authorize it.

Existing parent disposition (event 15): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g108 record](records/review-through-00de0b6c/groups/g108.json). Record ID: `c2`.

