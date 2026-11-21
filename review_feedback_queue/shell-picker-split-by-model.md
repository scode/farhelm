# The model field splits the launch-type picker

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

The model field can split the agent and shell choices, moving the Shells group as the selection changes.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed by source for model rows: view/choices.rs:159–176 says the Shells choices go immediately after the agent chips
and pushes those two groups in that order, but view.rs:1807–1819 inserts the model and catalog error at index 1 when the
harness is editable. SPEC.md Creation :1029–1036 describes Shells as the group after the agent chips; :1392–1393
describes tab fields. The undisputed result is a split picker in model-capable drafts. No failure to launch established.
Estimated bucket other. Possible_cover: the product spec says `after the agent chips` without explicitly requiring
adjacency, while the changed picker source explicitly states `immediately after` and the rows control one shared value.
Tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f has no subsequent launcher edits.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-launcher/f4`.
