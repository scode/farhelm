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
