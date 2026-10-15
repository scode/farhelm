# A mark acknowledgment can make quick launch run the previous template

Reviewed commit: 4a90caa2f6b3b2e3e082211602d4b9dc82d42287

## TLDR

After marking template B, quick launch can execute template A instead, including A’s command and destination.

## Details

Suggested bucket: `highest`. Confidence: confirmed by static ordering analysis; not reproduced at runtime. Source:
dedicated `gpt-6-astra` high adversarial review. Proposed drop: none.

crates/farhelm-ui/src/list/view.rs:3352–3354 reads preferences, awaits templates, then overlays only currently dirty
local intent. api.rs:795–801 marks a successful PUT acknowledged; :856–860 and :880–883 consequently stop overlaying it.
Trigger: A is stored; the user marks B and invokes quick launch while B’s PUT is pending; GET preferences returns A; B’s
PUT succeeds before the template read finishes. The continuation now treats the older A snapshot as authoritative and
automatically launches A (:3369–3381). No second client is required. The held-write test at
e2e/tests/quick-launch.spec.ts:280–340 keeps the write pending through launch, missing acknowledgment during the reads.
Preserve the local choice across the read’s lifetime, or invalidate/refetch a snapshot overtaken by a local write.

Possible cover: None found. SPEC_impl.md:853–863 expressly says a pending local mark cannot silently launch the previous
mark. FILTER.md’s timing filters exclude wrong-target actions and security consequences.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703; relevant files are unchanged. No fixing
commit.

Review coverage: full commit diff read: true. No commit-diff content unread. The 13,089-line triage ledger was searched
and relevant sections read, not read end-to-end; unrelated queue finding bodies and untouched surrounding modules were
not exhaustively read. One aggregated policy/search response was truncated; BUGS’ tail and relevant ledger
searches/sections were reread. No relied-upon diff or code remained truncated. No Sol reports or working-copy queue
additions read. Highest-candidate independent drop review remains for the collecting reviewer.

Runtime evidence: None, as requested. Static review only; changed tests assessed against the supplied checklist. No
files created, modified or deleted; no fetch, rebase or nested agents.

## Trailing review: number-key bindings

Additional reviewed commit: `4fced22e4d9bc501bbb84948a77cd52ef26203d6`, checked at pinned tip
`507dbcbbcfa00173ca7e4d24c79f3990025963b6`. Source: separate adversarial trailing-review agent, templates scope.
Suggested bucket remains `highest`. Confidence: confirmed static ordering, without a browser reproduction. Proposed
drop: duplicate file only; this extension retains the new field and entry path. Possible cover: the original bolt-mark
finding above covers the shared mechanism, but previously omitted number bindings.

The same race now affects `template_shortcuts`. Key 1 names A; assigning it to B sends a pending preference PUT. Press
the key while that PUT is pending. The shortcut's preferences GET can return A before the PUT succeeds, then await the
templates GET. On acknowledgment, `api.rs:835–850` marks the map clean, so `893–906` no longer overlays B. The shared
callback at `list/view.rs:3354–3357,3372–3390` resolves A and enables automatic launch. Binding writes originate in
`list/templates.rs:1164–1179`. A second client is unnecessary. Hold templates, acknowledge the PUT after the preferences
snapshot, then release templates to exercise the missing interleaving.

A fresh independent reviewer agreed that extending this item is the appropriate deduplication; none of its wrong-target
consequence is excluded. The original provenance and confidence remain unchanged.
