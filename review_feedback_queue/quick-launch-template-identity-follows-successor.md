# Coverage candidate: quick launch can follow a retargeted template-host entry

Independent drop decision: a fresh `gpt-6-astra` high reviewer rejected the exclusion as ambiguous. The general
logical-host rule conflicts with the template-specific promise against silently aiming at a successor. This differs from
removal/default-host fallback: the same registry entry remains while its installation changes. The required
healthy-successor observation without an intervening unavailable snapshot was not reproduced.

Reviewed commit: 4a90caa2f6b3b2e3e082211602d4b9dc82d42287

## TLDR

A pending quick launch may execute on a replacement installation behind the same host entry.

## Details

Suggested bucket: `highest`. Confidence: likely; the required browser observation sequence was not reproduced. Source:
dedicated `gpt-6-astra` high adversarial review. Proposed drop: Proposed covered drop for the same-entry
retarget/adoption variant only; independent drop review remains required and was not performed in this no-delegation
review..

Related to the preceding finding, but distinct scope: after template host application, a host refresh directly observes
a healthy successor installation while preparation remains pending. create_form.rs:2844–2852 updates the binding without
spending automatic intent; :2922–2973 obtains a successor preview; :3973–4032 submits when current readiness succeeds.
This requires no intervening unavailable snapshot to trigger the offline fallback.

Possible cover: SPEC.md:195–204 and TRIAGE_OUTCOMES.md:7791–7806 explicitly accept in-flight creates following the same
logical host entry. However, SPEC.md:614–616 separately promises template host identity will not silently aim at a
successor, and TRIAGE_OUTCOMES.md:6960–6976 enforced that promise for agent template launches. The independent reviewer
must resolve this scope tension before dropping.

Status at frozen endpoint: Mechanism and competing acceptance text remain unchanged at pinned main; no fixing commit.

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
Suggested bucket remains `highest`. Confidence remains `likely`: the healthy-successor observation sequence without an
unavailable snapshot has not been reproduced. Proposed drop: duplicate file only; no specification exclusion. Possible
cover: this item's existing disputed identity/retarget scope.

Number shortcuts at `list/view.rs:3372–3390` enter the same automatic path after resolving a template name. Target
reconciliation at `list/create_form.rs:2914–2923`, preview at `2969–3005` and submission at `4083–4142` therefore expose
the same potential successor substitution. This adds a caller, not stronger evidence for the required timing.

A fresh independent reviewer agreed to retaining the number variant here and explicitly did not agree to dropping the
underlying candidate on specification coverage. The original competing acceptance text and caveats remain unresolved for
human triage.
