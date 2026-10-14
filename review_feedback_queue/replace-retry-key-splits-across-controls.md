# Replace retries use separate keys in the sidebar and header

Reviewed commit: fcbcbe26286604c115fd2a7e64a85f34dcb4ac9d

## TLDR

Retrying an unanswered Replace from the other control can start a second agent in the same folder.

## Details

Suggested bucket: `high`. Confidence: confirmed static trace; lost-response duplication was not reproduced. Source:
dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none. Duplicate launches are established by inspection;
conflicting writes or lost work are not established.

The sidebar retains its own replacement key (`crates/farhelm-ui/src/list/view.rs:2213–2217,2259–2264`), while the header
and interrupted card have a separate key (`crates/farhelm-ui/src/session_view.rs:635–640,1500–1506`). If the first
Replace creates a replacement, source deletion fails and its reply is lost, retrying that source through the other
control mints a different key. The helm passes it through (`crates/farhelm-helm/src/sessions.rs:3215–3252`), and the
supervisor allocates a new session for the unseen key (`crates/farhelm-supervisor/src/service/core.rs:7133–7150`). Both
directions are affected. The shared operation gate prevents simultaneous actions but releases after failure and does not
share retry identity.

Possible cover: TRIAGE_OUTCOMES.md, `plain-replace-loses-retry-identity.md`, marks the original defect fixed. Its
completion criterion requires unresolved retries to reuse their key and does not accept this remaining cross-control
case. SPEC.md:653–655 requires one intended create to yield one session. This is an incomplete fix of that defect.

Current-main status: persists at `4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703`; separate keys remain at
`list/view.rs:2232,2274–2279` and `session_view.rs:648,1508–1514`. Share unresolved intent state for the same source
across its controls. A controlled regression should commit the first replacement, fail its deletion/report, then retry
through the other control and observe exactly one replacement.
