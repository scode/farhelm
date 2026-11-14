# Mixed-session regression coverage disappears before legacy serving retires

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: At this intermediate revision, upgrading users can still have old terminals beside new terminals,
but the change deletes the tests that check their combined tab ordering, takeover, Delete and survival when the last old
tab closes.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g153/c3`.

### g153/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: At this intermediate revision, upgrading users can
still have old terminals beside new terminals, but the change deletes the tests that check their combined tab ordering,
takeover, Delete and survival when the last old tab closes. Details: assigned 00790-000.patch:63-385 removes
a_mixed_session_lists_tmux_tabs_first_and_its_delete_ends_its_tab_hosts and
a_host_agent_outlives_its_last_tmux_tab_and_still_opens_tabs, together with the fixtures they depended on; location :406
is the parent-side deletion hunk. Exact governing text, SPEC_impl.md:2370-2376: "every new tab is a terminal host of its
own, whatever its session's agent runs in, so a tmux agent's session holds its old tmux tabs and new host tabs side by
side (a mixed session)" and "A host agent's session has a tmux session only while it has old tmux tabs, and losing the
last of them (tmux ends a session with its last window) touches neither its agent nor its host tabs." Also :2395-2396:
"The tab strip and every tab lookup merge the two kinds: tmux tabs first, in window order, then host tabs by their tags'
creation times"; :2409-2410: "A takeover displaces the previous client's attachments to every terminal of the session,
tmux and host alike". Source checks at the same commit found no replacement mixed-session tests through searches for
mixed-session/last-tmux-tab/lone-tmux-tab/order/migration fixture names across supervisor and integration tests.
host_terminals.rs contains legacy-row evidence and cleanup tests, but its tmux references do not set up the deleted
mixed live tab scenarios. Production core.rs:6428-6433 still adopts an earlier supervisor's tmux server and :14562-14589
still serves/open-gates both kinds, so this is live behavior at the reviewed revision. Suggested fix at that revision:
keep equivalent tests by seeding earlier-release tmux state directly and adopting it, instead of relying on the removed
create switch. Suggested bucket: other; confidence: confirmed by source for loss of these specific regression scenarios,
not an implementation bug. could-be-highest: no established product security or loss-of-work failure; test coverage
only. proposed_drop: false. possible_cover: none. Prior proposed drop preserved: "no implementation regression
established; test-only legacy fixtures were removed with their creation seam". That is insufficient for a test-only
finding and is superseded. Later retirement, distinct from repair: 1ed482dde220452e424973faf38a5e1318986333
SPEC_impl.md:2882-2888 says "This release ends them at startup (below); every terminal runs in a terminal host" and "its
driver of the old terminals is gone". That removes the subject prospectively and does not establish coverage at 647f022.
Parent has accepted no drop. No runtime execution or excluded coverage search.

Returned proposal (claims-098, event 13): **retain-verified**, suggested bucket **other**.

The intermediate revision deletes distinct mixed-session ordering, takeover, Delete and last-old-tab survival oracles
while still adopting and serving old terminals. Later retirement ends the subject prospectively; it does not restore
historical test protection. No production lifecycle defect is established.

Original stronger consequence: The first record considered broken upgraded sessions, then proposed dropping because no
implementation regression was found and creation fixtures were removed. The later records withdraw that basis for the
independent test-only gap. Preserve both the no-product-failure limit and the historical gap.

Unresolved premise or limitation: No missing premise for the historical removal of these specific assertions. A shipped
loss-of-work route would need separate evidence; restoring obsolete tmux cases at current tip is not the proposed fix.

Proposed basis: Retain the intermediate historical test-oracle finding, other priority. The flag preserves independent
review of the earlier dismissal of a possible upgraded-session/process consequence; this consultation does not authorize
that drop.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g153 record](records/review-through-00de0b6c/groups/g153.json). Record ID: `c3`.
