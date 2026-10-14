# Live-sibling preservation test finishes before automatic cleanup completes

Reviewed commit: 059de70910e6eb78d32985b1a1ca90c07788ed4a

## TLDR

The regression test can pass before cleanup finishes, weakening its assurance that another terminal survives.

## Details

Suggested bucket: `other`. Confidence: confirmed: missing completion boundary is established statically; no runtime
failure reproduced. Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop: none.

crates/farhelm-supervisor/src/service/ticker.rs:2611–2637 stops waiting when both dead panes disappear, immediately
checks the surviving tab, and returns. After this commit, pane removal precedes the background close's second process
sweep and attachment cleanup (core.rs:12829–12859). Consequently, the final sweep can still be running when the survival
assertion passes and StateDir destroys the fixture's tmux server (core.rs:15189–15211). The new wait_for_reaped_panes
helper explicitly distinguishes these boundaries at ticker.rs:2199–2237, but this changed test omits it. Await cleanup
completion before taking the final survivor snapshot. The pane-death test at ticker.rs:2751–2768 also relies on pane
disappearance plus ticker shutdown, which no longer joins closes.

Possible cover: No matching Planned item, queue entry, or ledger decision found. This is a fixture-lifetime and
assertion-ordering issue under the supplied test checklist.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703; ticker.rs is unchanged from the
reviewed commit.

Review coverage: full commit diff read: true. No commit-diff content unread. Specifications and the large triage ledger
were read in relevant excerpts, not in full. Unrelated open queue finding bodies and unrelated intervening main changes
were not reviewed. Truncated relevant output was reopened before use. No Sol reports or working-copy queue additions
read.

Runtime evidence: None, as requested. Static review only; no checkout changes, scratch files, fetches, or helper agents.
