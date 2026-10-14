# Folder history changes the highlighted destination

Reviewed commit: 294c8499399e2ed2f1f22fea2f8cfa9e615778d8

## TLDR

Enter can choose a recent folder instead of “local home”, leaving a subsequent launch aimed at the remote host.

## Details

Suggested bucket: `highest`. Confidence: confirmed by static state-flow inspection; browser sequence not reproduced.
Highest classification is conservative because the substituted reset can leave a subsequent command launch on the wrong
machine. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

At crates/farhelm-ui/src/list/create_form.rs:2946–2959, keyboard selection retains only an index into recents followed
by Home and LocalHome. Before the initial history response, ArrowUp highlights LocalHome at index 1. The first response
is promoted automatically at 2545–2549, inserting recent folders before the resets without clearing or remapping
folder_active. With at least two recents, index 1 now identifies a recent folder; Enter applies that different choice at
5542–5546. With a remote host selected, the intended local-host reset never happens. The choice itself only changes the
draft, but a subsequent Launch uses the remote destination. Preserve choice identity across list changes or invalidate
keyboard selection when its entries change. The added browser test waits for eight recents at
e2e/tests/sidebar.spec.ts:5935 before navigating, so it misses this boundary.

Possible cover: No matching Planned item, BUGS entry, queue item or ledger decision found. SPEC.md:613–618 describes
deliberate keyboard selection and local-home pinning, without accepting selection substitution. The sub-second-race
filter does not fully cover this: initial-response latency is unbounded, and the consequence can include a subsequent
operation on the wrong machine.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: create_form.rs:2599–2603 still automatically
installs initial history, 3003–3016 retains index-only selection, and 5768–5772 activates the reinterpreted index. No
fixing commit found.

Review coverage: the full commit diff and relevant failure paths were read. No runtime reproduction was run; unchanged
surrounding code was inspected only where relevant.

## Additional affected scope

Second-pass classification: this is a repeat of the same mechanism, with no new affected scope. Astra suggests `other`
because selecting the folder changes only the draft and another explicit Launch is required. Sol's conservative
`highest` bucket includes that later wrong-machine consequence. Both suggestions are preserved for human triage; this
repeat adds no new distinct finding.

Reviewed commit: 294c8499399e2ed2f1f22fea2f8cfa9e615778d8

## TLDR

While folder history loads, a highlighted home reset can become a recent folder. Enter then fills a different
destination from the one the user navigated to.

## Details

Suggested bucket: `other`. Confidence: confirmed by code inspection; no browser reproduction. Source: dedicated
`gpt-6-astra` high adversarial review. Proposed drop: none.

In crates/farhelm-ui/src/list/create_form.rs:2946–2959, the highlighted choice is retained only as an index into recents
followed by two resets. Before the initial history response, ArrowUp highlights local home at index 1. The initial
response is automatically offered at 2545–2550 without resetting or reconciling folder_active. If it contains two or
more folders, index 1 now identifies the second recent folder; Enter at 5542–5546 applies that folder instead of local
home. This requires history to arrive between navigation and acceptance, but does not require acceptance within a
sub-second window. It changes the draft only; another explicit launch is needed to start a session. Preserve the
selected choice's identity across initial population. Existing tests populate history before navigating and miss this
transition.

Possible cover: FILTER.md's rare, self-correcting glitches filter may merit triage consideration, but accepting the
shifted selection changes the destination draft rather than merely its display. SPEC.md:613–618 specifies arrow-selected
choices and does not explicitly accept this remapping. No matching Planned item, BUGS entry, queue item, or searched
ledger decision found.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: create_form.rs:2599–2603 automatically
installs initial history, 3003–3016 retains the positional highlight, and 5768–5772 accepts the remapped choice. No
fixing commit identified.

Review coverage: full commit diff read: true. No commit diff hunks unread. Surrounding unchanged modules and unrelated
ledger sections were not read exhaustively. Initial aggregate output truncation was recovered for the relevant diff,
Planned bucket, and ledger search results; unrelated TODO content was not relied upon. No Sol reports or working-copy
queue additions read. Rendered browser behavior remains unverified.

Runtime evidence: None, as requested. Static review only; no checkout or scratch files created or modified.
