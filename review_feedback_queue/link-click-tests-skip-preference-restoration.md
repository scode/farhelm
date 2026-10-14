# Terminal-link tests can leave shared link opening disabled after failure

Reviewed commit: cfb54c4c4f7e0c7018b55f802fb1add03ad0a577

## TLDR

A failed test can leave “program only” enabled for the shared test helm, causing subsequent link-opening tests to fail
misleadingly. No shipped-product regression was confirmed.

## Details

Suggested bucket: `other`. Confidence: confirmed by control-flow inspection; not reproduced at runtime. Source:
dedicated `gpt-6-astra` high adversarial review. Proposed drop: none.

e2e/tests/terminal-link-clicks.spec.ts:359–361 saves terminal_link_click='program' and creates the session before
entering try/finally. If session creation fails, restoration at 381 never runs. Independently, lines 379–381 and 212–214
put restoration after cleanupSession; that helper can throw on transport failures or non-404 refusals
(e2e/tests/helpers/fleet.ts:520–529), skipping restoration. The suite shares one helm across tests/projects
(e2e/playwright.config.ts:83–100). Neither the evidence fixture nor terminal-suite reset restores preferences, and
terminal-links.spec.ts:55–95 assumes Farhelm opens links without explicitly seeding this setting. Arm preference
restoration before the first mutation and execute it independently of session cleanup. Scope: test isolation and
cascading false failures.

Possible cover: No matching original queue entry, Planned item, BUGS entry, or searched ledger decision found.
FILTER.md's rare-glitch filter excludes durably recorded wrong state; this preference survives browser/worker
replacement.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: terminal-link-clicks.spec.ts and the
relevant shared-stack/evidence lifecycle are unchanged. No fixing commit identified.

Review coverage: full commit diff read: true. No commit-diff content unread. Initial aggregated output truncations were
repaired by narrower reads. Most of the 13,089-line triage ledger was searched rather than read sequentially; original
queue finding bodies were not opened because the frozen index identified no matching item. Unchanged surrounding code
outside the listed ranges was not exhaustively reviewed. No Sol reports or working-copy queue finding contents were
read; a git-status call incidentally displayed working-copy filenames.

Runtime evidence: None, as requested by default. Applied the supplied test-authoring checklist by source inspection. No
checkout files or scratch files were created, modified or deleted; no fetch, rebase or helper agents.
