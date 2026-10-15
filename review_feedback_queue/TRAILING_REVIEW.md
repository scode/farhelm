# Trailing code review checkpoint

Last reviewed commit: `75af240c09c7ecd5b81a0fcba9e03f204b0be0a7`

This is the exclusive starting point for the next trailing code review of `main`. Advance it only after completing the
procedure in [AGENTS.md](AGENTS.md#trailing-code-review), together with any resulting queue updates. A proposed
checkpoint reaches `main` only when its review-result PR is merged.

## Completed review

Reviewed range: `507dbcbbcfa00173ca7e4d24c79f3990025963b6..75af240c09c7ecd5b81a0fcba9e03f204b0be0a7`, 111 commits. Seven
separate adversarial reviewers read every assigned commit's complete changes, including deleted findings, transient
plans, reports, and intermediate implementations. Coverage was checked against the full pinned commit list:

| Scope                                                    | Commits |
| -------------------------------------------------------- | ------: |
| Guest-native release-test executor                       |       1 |
| Terminal file opening and development builds             |       6 |
| Permission and command-edit safeguards                   |       5 |
| Session pins and live permission display                 |       2 |
| Save ordering, host Settings, restart tests, and watcher |       5 |
| Managed-checkout Claude trust                            |       1 |
| Documentation and bookkeeping                            |      91 |

No assigned patch content remains unread. Unchanged surrounding code and specifications were read selectively. The
triage ledger was searched with relevant sections read rather than read end-to-end. External prior-review transcripts
and private runtime evidence were unavailable; report claims were checked for internal integrity, not independently
authenticated.

Nine new findings were retained: three conservatively highest-priority records for destructive Git redirection,
unrelated numeric mutation, and publication after vendor-lock takeover; one high-priority startup-delay record; and five
other records covering stale Settings, test portability, browser fixture cleanup, and inaccurate Resume docs. Each
record preserves confidence and unverified premises. The lock-takeover candidate was retained after an independent
reviewer rejected its proposed concurrent-writer exclusion.

Four existing file-link findings were extended to preserve the native-opening consequence, with the oversized-path
record narrowed for the new filename-first layout. Fresh independent review required those extensions before
deduplication. The custom-permission fixture-cleanup record was extended to the permission-switch test. The older folder
save record was annotated with evidence that its ordinary successful-save trigger is fixed; this recording-only review
made no triage decision or ledger change.

Fresh independent reviews agreed to narrow security-related exclusions for Settings dismissal losing a failed
confirmation-restoration warning; unknown remote outcomes after abandoned configuration saves; advisory live permission
readings retaining launch-based authority; malicious same-user guest testers being outside the executor's sandbox claim;
and the already accepted incomplete-checkout reuse case after killed preparation. The orphan-Git lock lifetime concern
was not generalized into acceptance of harm to completed checkouts or active sibling work.

The native-file review independently confirmed the historical symlink redirect was fixed within the range, and the
explicit acceptances for authenticated native opening, absent quarantine, real-state development builds, and
operator-trusted payloads. No Linux MIME-sniffing bypass was established in the bounded local probes; other MIME
databases and default applications were not verified. This is uncertainty, not a universal safety claim.

For Claude trust, independent review accepted the expressly best-effort failed-removal/path-reuse case, the unlocked
exit-save race, and deliberate managed-path interference. It checked the Ready-restart, Fresh-only, archive-selection,
and worker-ownership refutations. Vendor lock-alias behavior was checked against the project's recorded observation, not
a fresh vendor-binary inspection. The locking-writer takeover race and deterministic numeric rewrite remain separate
retained findings.

Review was primarily static. Isolated scratch Git commands reproduced destruction in a foreign scratch checkout; Linux
GIO probes did not reproduce the proposed extension-capitalization bypass. A standalone serde_json probe and Node
comparison confirmed a distinct binary64 value changing during serialization. No live installation, real vendor
configuration, or guest was changed. Queue formatting, index consistency, and complete commit assignment were checked.
No product runtime suite was run for this documentation-only result; earlier product-suite claims were not reused as
current runtime passes. No fixes or triage outcomes were executed.
