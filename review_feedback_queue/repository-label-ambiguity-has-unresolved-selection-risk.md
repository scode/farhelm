# Repository-label ambiguity has unresolved selection risk

Reviewed commit: a3650d487b0c461dc0f1e9fee4faca21c087afbb

## TLDR

Malformed repository metadata can produce indistinguishable labels. A harmful decision based on that ambiguity remains
unestablished, despite checks of launch validation and independently bound session actions.

## Details

Suggested bucket: provisionally **highest** for the unresolved harmful-selection interpretation; established display
ambiguity alone is **other**. Confidence: **confirmed presentation ambiguity and operation bindings; consequential human
selection remains a missing causal source premise**, separately from unmeasured browser behavior or frequency.

Ordinary `display_peer` is non-injective (`crates/farhelm-ui/src/peer.rs:159–180`), unlike the distinct identity
spelling at `204–232`. Malformed repository associations can reach labels in `icons.rs:105–129` and
`list/summary_card.rs:56–69`. Preserving raw operational identity does not by itself refute misleading human selection.

The investigated execution bypasses are narrowly refuted. Fresh-create validation rejects these malformed names
(`crates/farhelm-proto/src/github_checkout.rs:203–265`; helm `sessions.rs:1133–1153,1458–1468`). Templates can store
bounded malformed destinations, but applying them reparses the repository before edits
(`crates/farhelm-ui/src/list/create_form.rs:709–725`). Stop/Delete/Replace retain separate session references
(`list/row.rs:1077–1089,2019–2119`; `list/view.rs:1911–1964,2058–2070,2266–2279,2560–2573`), alongside other host, title
and path context. No callback retargets an operation from the repository label.

Independent auditor `g138` could neither establish a concrete harmful authorization caused by the label nor exclude
every such reliance path. Its broad exclusion investigation stopped incomplete. Possible cover: no exact acceptance,
Planned item, known bug, queue item or triage decision found. A rare diagnostic-only filter cannot be generalized to an
established security consequence. Proposed drop: none for the conditional broad concern. Do not present it as a
demonstrated malformed-repository launch bypass.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g084` repository-label candidate and `g138/f6`.
The separate discovery/history parser refutation from `g133/f6` covers those parsed input routes, not every stored
association renderer.
