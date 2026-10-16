# Trailing code review checkpoint

Last reviewed commit: `991835f826b25bc93eb088f6d3984477a9dc2fab`

This is the exclusive starting point for the next trailing code review of `main`. Advance it only after completing the
procedure in [AGENTS.md](AGENTS.md#trailing-code-review), together with any resulting queue updates. A proposed
checkpoint reaches `main` only when its review-result PR is merged.

## Completed review

Reviewed range: `75af240c09c7ecd5b81a0fcba9e03f204b0be0a7..991835f826b25bc93eb088f6d3984477a9dc2fab`, 229 commits. The
tip was fetched and the checkout moved onto it at the scheduled start. Fourteen separate adversarial reviewers read
every assigned commit's complete patch, including deleted plans, removed findings, intermediate bookkeeping, tests and
reports. Coverage was checked against the full pinned commit list:

| Scope                                                    | Commits |
| -------------------------------------------------------- | ------: |
| Landed-plan approvals                                    |      17 |
| Review records, triage, specifications and new plans     |      39 |
| Identity, trust, checkout replay, templates and reports  |      60 |
| Terminal file authorization, ntfy and remote uninstall   |       8 |
| Updater, documentation, setup and evidence retention     |      25 |
| Downloads, startup, trash and browser corrections        |      27 |
| Instruction extraction, loader changes and checker       |       3 |
| Rust portability, fixture cleanup and notification focus |      12 |
| Wrapped-link warning                                     |       6 |
| Browser fixture ownership and restoration                |      23 |
| Launcher refresh, retry, draft and scrolling changes     |       9 |

The large instruction-extraction patch was divided at file boundaries among four reviewers; their segments cover its
entire 7,659-line patch without gaps. No assigned patch content remains unread. Unchanged surrounding implementations,
specifications, queue files and the triage ledger were read selectively. Most of the minified terminal bundle was not
read; relevant complete snippets were inspected. Historical private runtime evidence and external review transcripts
were unavailable and were not authenticated by this review.

Eleven new findings were retained. One is conservatively highest priority: a newly created helm registry can reuse a
previous host's checkout-approval namespace. A separate independent reviewer confirmed the static path and agreed to
retain it; exploitation still requires an eligible surviving reservation and knowledge of its original request tuple.
The fresh-registry scenario was not reproduced end to end.

Ten other records cover restored-screen link clicks, repeated-link false warnings, production Git configuration
inheritance, notification fixture cleanup, two trash presentation gaps, two instruction-checker gaps, and two launcher
setup lifecycle gaps. Several are pre-existing adjacent omissions exposed by the fixes under review. Each record keeps
its reviewed revision, provenance, confidence, reproduction limits and possible coverage. The restored-screen filter
question remains explicit for triage rather than being silently settled.

Three fresh independent reviewers agreed to all proposed exclusions involving possible security or user-work loss,
including repaired file authorization, token-origin binding, identity encoding, numeric preservation, Git fixture
isolation, no-clobber publication, refusing CA fallback and corrected security documentation. Their decisions were
limited to each named mechanism. They did not waive the separately queued Claude stale-lock overwrite or the newly
retained fresh-registry replay issue. Model suggestions were checked against their release-owned catalog source rather
than treating the general display helper as injective.

The independent exclusions also checked the explicit acceptances for OS-trusted proxies, deliberately retargeted local
symlinks and reserved sockets, scroll-reachable dialog explanations, the recorded development-build Git decision, the
advisory shield, and the specified limits of whole-tag link comparison. The extracted released-claim reset still forbids
abandoning unpushed work. These acceptances were not generalized to different triggers or cross-host effects. A residual
setup-command quoting concern matched the explicit user-configured-path exception. The native input-method caveat and
unfinished Pi-guide wording remain covered by their exact existing ledger outcomes, not claims of fresh runtime proof.

Review was static. No product runtime suite, real agent turn, live installation action, guest test or destructive
reproduction was run. The review-result change contains records and checkpoint bookkeeping only; no fixes or triage
outcomes were executed. Formatting, queue/index consistency, commit assignment and instruction-pointer checks validate
the result; previous product-suite claims were not reused as current passes.
