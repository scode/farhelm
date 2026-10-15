# Removing the template’s host during preparation silently redirects quick launch

Reviewed commit: 4a90caa2f6b3b2e3e082211602d4b9dc82d42287

## TLDR

A quick launch intended for a remote host can create its checkout and start its agent on the default host without
showing the changed destination.

## Details

Suggested bucket: `highest`. Confidence: confirmed static control-flow path; browser interleaving not executed. Source:
dedicated `gpt-6-astra` high adversarial review. Proposed drop: none for host removal and fallback to a different
registry entry.

The new automatic submission at crates/farhelm-ui/src/list/create_form.rs:3973–4032 checks the current target’s
readiness but retains no original destination constraint. Template application resolves its host identity once and
stores the registry ID (:561–571, :669–677). If that host is removed while its managed-checkout preview is pending,
list/shared.rs:473–480 falls back to the ordinary default host. list/view.rs:1193–1211 updates create_target;
create_form.rs:2844–2852 merely retires the old key/error, while :2922–2973 requests a preview for the replacement
destination. Once that preview succeeds, the still-pending automatic launch submits there. The outgoing preview and
connection claims describe the replacement host, so ordinary stale-request guards do not reject it. Concrete scope: a
template explicitly selects remote A; another client removes A during a slow preview; the default host has a usable
checkout root and supports the selected launch. Spend the automatic intent and reveal New when an already-applied
destination disappears.

Possible cover: SPEC.md:195–204 accepts operations following the SAME logical host entry after retarget/adoption; it
does not cover removing that entry and launching on another. SPEC.md:668–674 specifies ordinary default-host fallback,
but quick launch promises template fields override those defaults (:641–648). TRIAGE_OUTCOMES.md 'template-successor.md'
concerns agent-request dispatch, not this new hidden GUI continuation.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703; relevant implementation and
specification are unchanged. No fixing commit.

Review coverage: full commit diff read: true. No commit-diff content unread. The 13,089-line triage ledger was searched
and relevant sections read, not read end-to-end; unrelated queue finding bodies and untouched surrounding modules were
not exhaustively read. One aggregated policy/search response was truncated; BUGS’ tail and relevant ledger
searches/sections were reread. No relied-upon diff or code remained truncated. No Sol reports or working-copy queue
additions read. Highest-candidate independent drop review remains for the collecting reviewer.

Runtime evidence: None, as requested. Static review only; changed tests assessed against the supplied checklist. No
files created, modified or deleted; no fetch, rebase or nested agents.

## Trailing review: number shortcuts

Additional reviewed commit: `4fced22e4d9bc501bbb84948a77cd52ef26203d6`, checked at pinned tip
`507dbcbbcfa00173ca7e4d24c79f3990025963b6`. Source: separate adversarial trailing-review agent, templates scope.
Suggested bucket remains `highest`. Confidence: confirmed static shared path; the browser interleaving was not executed.
Proposed drop: duplicate file only; retain number-key reachability here. Possible cover: this item's existing
removal/default-host trigger and consequence.

The number callback at `list/view.rs:3372–3390` uses the same automatic form path. Template host application at
`list/create_form.rs:572–580` still stores a registry ID; removal can reach default-host fallback at
`list/shared.rs:466–480` and `list/view.rs:1188–1214`. Target changes at `create_form.rs:2914–2923` do not spend
automatic intent. Once the replacement preview at `2969–3005` succeeds, submission at `4083–4142` can proceed on the
default host. The original bug is unchanged; this commit adds number-key admission to it.

A fresh independent reviewer confirmed that this extension retains the whole candidate. Same-entry retarget acceptance
does not exclude fallback to a different host entry.
