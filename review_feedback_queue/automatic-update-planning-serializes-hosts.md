# Automatic Update planning serializes unrelated hosts

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

One slow host's planning can delay automatic updates for later hosts even though each individual wait is bounded.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The automatic worker plans remote targets serially. Per-host timeouts bound each step but accumulate across the fleet.
The mechanism is confirmed; whether the manual Update-all responsiveness contract also governs this automatic path
remains ambiguous. No fleet latency or runtime prevalence was measured.

Source locations: `crates/farhelm-helm/src/provisioning/automatic.rs:109-151`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis at reviewed d6645f771bee8ed03a66486ef8755834937d759d, introduced04e8b3f97901f79e07e2fc7cb95a211f8cb60335.
Retain, narrowed to background producer isolation. TLDR: later healthy hosts wait for earlier hosts' slow SSH/systemd
inspection before their automatic updates can even begin; several such hosts accumulate delay. scan processes the
ID-sorted snapshots serially and awaits plan_update plus admission per host(automatic.rs109-151;manager1880-1904).
Individual waits are bounded: handshake15s, inspection shell30s plus bounded drain, not an infinite
hang(backend17-29,1936-1940,657-658,2189,2242-2244). With several hosts each consuming the15s probe budget, later target
selection accumulates those budgets; no precise runtime measurement or unbounded starvation is claimed. A
source-grounded trigger is a still-published older supervisor whose new planning SSH connection is slow, or a live
supervisor whose user-manager environment query stalls while its session connection remains usable. The existing four
planning permits are not saturated by this producer: it starts only one plan at a time. Fleet events cannot run a
concurrent scan(automatic50-61). The precise product contract is isolation, not a numerical deadline or guaranteed start
order: SPEC.md3480-3486 says other hosts must not wait on host Update or other slow work;3499-3511 allows the slow
host's own work to be slow while forbidding it from stopping service to other hosts. Automatic per-host Update work is
expressly requested by the opt-in(SPEC144-147); those rules contain no foreground/background exception. My finding does
not extend the Update-all-specific failure wording137-140 into a general SLA. No terminal, GUI-control, manual-action,
normal session-list blockage or starvation was established. Bucket other, confirmed source mechanism and literal
isolation-contract reading. proposed_drop:false. possible_cover: if the maintainer intends serving other hosts to mean
only on-demand service and allows this background producer to serialize bounded remote waits, that narrower scope is not
explicit in current text; parent should preserve that interpretation question. FILTER Rare self-correcting glitches is
not an exact match: ordinary per-host latency is enough and no action fails safely for the later host; it simply is not
dispatched. FILTER hung user manager requires the whole consequence stay on one host(lines52-59), whereas here the delay
reaches other automatic targets. SPEC.md3895's malicious/arbitrarily-broken-host DoS acceptance is not necessary to the
ordinary-slow-host trigger. No matching Planned entry, unlanded fix, BUGS entry, queue item or TRIAGE decision was
found. No tests run; assigned tests have only one remote host, so provide no fairness evidence.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g139/f2`, `g140/f3`, `g107/f4`, `g108/f3`,
`a029/a029-v6`, `a030/g140-f3`, `a054/g140-f3`, `a058/g108-sequential`. These are original reviewer identities for this
concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the private review
evidence. No fix or triage decision was executed.
