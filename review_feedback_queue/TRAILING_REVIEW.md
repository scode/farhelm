# Trailing code review checkpoint

Last reviewed commit: `afc828776596f2e6639d16d4203ccc8b3d0f5862`

This is the exclusive starting point for the next trailing code review of `main`. Advance it only after completing the
procedure in [AGENTS.md](AGENTS.md#trailing-code-review), together with any resulting queue updates. A proposed
checkpoint reaches `main` only when its review-result PR is merged.

## Completed review

Reviewed range: `991835f826b25bc93eb088f6d3984477a9dc2fab..afc828776596f2e6639d16d4203ccc8b3d0f5862`, 139 commits. The
tip and its saved checkpoint were fetched and pinned before review; the checkpoint was verified as an ancestor. Eight
separate adversarial reviewers read every assigned commit's complete patch, including deleted plans, reports, findings
and release fragments. Assignment counts were checked against the complete pinned commit list:

| Scope                                                     | Commits |
| --------------------------------------------------------- | ------: |
| Earlier landed-plan approvals                             |      28 |
| Later landed-plan approvals                               |      13 |
| Plan creation, lifecycle, reports and review instructions |      59 |
| Documentation, release notes, specifications and triage   |      21 |
| Trash, menu focus, scrolling, IME, downloads and outages  |       7 |
| Registry retry namespace, Git policy and trust startup    |       4 |
| Agent deletion and initial-prompt backend                 |       2 |
| Prompt UI, template names and Replace retries             |       5 |

No assigned patch content remains unread. Unchanged surrounding implementations, specifications and coverage sources
were read selectively. This was not an exhaustive review of those surrounding modules. Private historical runtime
evidence was unavailable and was not authenticated. Installed OMP 18.10.4 argument parsing was inspected statically;
other vendor implementations and native input-method behavior were not independently verified.

Two records were retained. One likely `other` finding concerns outage cards whose unbounded vertical centering can hide
their explanation and remedy above the reachable area. Browser geometry was not reproduced. The other conservatively
retains a potentially highest-priority residual menu-focus case: independent focus during pending arrow navigation can
still be overridden before subsequent activation. The historical plan explicitly accepted this exact limitation, but the
named specification and triage coverage sources do not reflect that narrower acceptance. An independent auditor could
not unambiguously approve exclusion under the recording rules. Its record preserves the exact historical decision and
calls for reconciling it before commissioning a broader design; it does not claim the implementation ignored the
maintainer's direction. Destructive activation was not reproduced.

Three additional independent auditors checked proposed exclusions with possible security or user-work consequences. They
agreed to the scoped refutations for trash-answer identity, registry namespace separation, deletion consent and
connection fencing, prompt placement and replay, old-client prompt decoding, template-name validation, download reply
correlation, terminal recovery ownership, and IME target isolation. They also checked startup ownership across
cancellation, Git repository-environment scrubbing and trust-removal admission. These are static conclusions about the
named mechanisms, not general clean verdicts for their subsystems.

The exclusions for stalled Claude trust publication and residual trust after failed cleanup rest on exact accepted
limits and recorded decisions. A further audit rejected the blanket claim that every failed-create rollback confirms all
work stopped: a failed database confirmation plus a failed process sweep can still lose its session record after tmux
termination. A different independent auditor agreed to exclusion only because the existing ledger records that exact
case and SPEC.md accepts coinciding independent faults leaving work running beside a relaunch. The UI discards the retry
key; another user action is still needed to launch again. This does not accept false refusal proof after a single
ordinary failure, cross-host effects or database corruption.

Review and validation were static. No product runtime suite, browser reproduction, real agent turn or live-installation
action was run, and no previous product-suite results were reused as current passes. Formatting, whitespace, queue/index
consistency, complete commit assignment and instruction-pointer checks validated the records. No fixes or triage
outcomes were executed. The review-result change remains a draft for manual merging.
