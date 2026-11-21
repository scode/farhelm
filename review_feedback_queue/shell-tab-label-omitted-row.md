# The shell tab can still say Agent when the pane has no session row

Reviewed commit: 05ae73f23eb2cbb7ab284ca42473d66a771207a9

## TLDR

An open shell can show an Agent tab when the session list omits its row.

## Details

**other**. Confirmed by source and contract inspection.

A Bash or Zsh pane can still show Agent and agent-specific help when the sidebar has no row for it. Source-confirmed,
estimated other. Terminal/set.rs:692–712 selects Shell and its help only from the current SessionRow, otherwise the pair
is “Agent” and “Agent: show the session's agent terminal” at :704. The same projection causes the three checked cases.
(1) A hidden shell can open immediately from quick switching before its row arrives (quick_switcher.rs:808, test source
quick_switcher/tests.rs:1273–1308). A refused naming query can keep that live visit rowless
(filters.rs:215–221,264–274,312–334,390–453 and helm ui_channel.rs:887–906. Tabs/f2 is the separate upload notice
effect). (2) shell-contract/f15 is the same fallback and belongs in this item, but in that route Replace has already
removed the source. Session_list.rs:569–577 preserves its selection until the reply,
launcher/replace_with_tests.rs:377–399 documents and checks the push-before-reply ordering without executing it,
session_pane.rs:398–434 retains an existing mount, and card.rs:119–130 and header.rs:561–569,617–623 leave its strip
drawn. It is a transient caption regression on an already identified source that has been removed, not a never-delivered
row or still-running source. (3) shell-contract/f8 also confirms a retained selection at the supervisor's listing cap.
Ordinary failed refresh preserves rows (helm ui_channel.rs:468–484). The caption itself changes neither terminal choice,
target nor attach availability. F13 covers extra-tab view continuity separately. SPEC.md:48–53 distinguishes a shell
from an agent and :1978–1981 preserves source selection until Replace settles. Possible_cover: :2259–2262 describes
initial tab-list loading, which does not fit Replace's known source or persistent refused naming. Proposed_drop: at most
exclude the cap-only example under SPEC.md:2649–2657, where count warning and notice are “the whole of the answer to
it”. Do not drop the combined item or make a second item for shell-contract/f15. The pinned tip
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f contains no later fix.

The label can also change while Replace leaves the previous selection mounted after the previous row was removed. This
uses the same missing-row fallback as the cap and the quick-switcher paths.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-contract/f8`,
`shell-contract/f15`, `shell-presentation/f2`.
