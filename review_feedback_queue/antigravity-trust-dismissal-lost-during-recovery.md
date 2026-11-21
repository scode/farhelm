# Temporary terminal unavailability can erase the local dismissal of a still-current trust question

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A terminal recovery can bring back an Antigravity trust hint the user already dismissed for the same question.

## Details

SPEC.md:1741–1742,1765–1769 and SPEC_impl.md:3507–3512 tie local dismissal to the question leaving its agent screen. The
supported terminal re-adoption path explicitly retries a runtime that became unavailable without replacing the agent's
launch. `crates/farhelm-supervisor/src/service/core/host_handover.rs`:180–201 documents the same-generation re-adoption
and :1672–1705 carries forward the current session, generation and run cell. `hosts/terminal.rs`:840–852 represents the
unavailable interval as unattributed, and `service/status.rs`:189–204 makes it Unknown for an active run.
`service/status.rs`:339–348,417–420 strips `workspace_trust_prompt` from that reply even when the run cell still
contains it. The helm's cached status retention does not retain the trust bool
(farhelm-helm/src/manager.rs:1584–1597,5743–5771). In the GUI, trust_band.rs:213–227 retires the × key on any present
row without the flag. When the same generation is re-adopted and its question evidence returns, :243–249 rereads the
setting and :359–372 can redraw the hint. If the GUI receives the interim row, evidence loss alone cleared its
dismissal. Keep a local dismissal through unavailable or unsampled observations, while continuing to gate drawing on
current evidence, and retire it on a confirmed end or new occurrence.

Confidence: confirmed by source given a delivered unavailable row and a question that survives re-adoption. No runtime
timing was measured.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `agUI/l21`.
