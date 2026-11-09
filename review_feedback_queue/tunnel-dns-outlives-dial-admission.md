# Timed-out tunnel dials retain resolver work

Reviewed commit: 906793ea4fbfba9bc9e19b9a1b5a7864d089ad87

## TLDR

Unverified after investigation: stalled hostname resolution can outlive recipe tunnel timeouts and admission limits,
potentially accumulating work or delaying helm shutdown. An affected native resolver execution has not been
demonstrated.

## Details

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. The original reviewer's
confidence, qualifications and severity history are retained below; source-confirmed conditional behavior is not a
runtime observation.

The tunnel contract requires bounded destination work; no explicit resolver exception was found. BUGS.md's local
SSH-helper output hang is a different producer and lifetime. The hung-tmux/systemd filter does not cover native name
resolution or helm-wide shutdown.

## Recorded source assessment

Reviewed commit 906793ea4fbfba9bc9e19b9a1b5a7864d089ad87. Discovery-boundary clarification, preserving all prior
evidence and other bucket: user-visible failure is unverified after investigation. Source-confirmed facts: Tokio 1.53.2
net/addr.rs:189-222 creates blocking system resolution outside the event permit; task/blocking.rs:106-119 makes started
work non-abortable; tunnels.rs:268-272 drops only the async connect future at timeout; client/tcp_relays.rs:57-68
eventually returns the event permit independently; lib.rs:2282-2306 drops the owned runtime without shutdown_timeout.
Therefore resolver completion is not bounded by tunnel admission or its timeout. Unmeasured native scheduling/system
behavior: whether and for how long the supported host's NSS/getaddrinfo blocks, whether enough repeated attempts
accumulate to materially degrade service, and actual shutdown delay. These are not established merely by Tokio's API
contract. Actual investigation read the version-matched installed dependency, pinned Cargo.lock and runtime owner at
reviewed hash; no missing dependency source remains. Missing premise is an affected native resolver execution,
unavailable under this source-only review. Settling evidence is controlled blocked hostname resolution followed by
repeated timeout/admission and orderly runtime shutdown observations. Suggested bucket other; confidence likely for
conditional impact, source-confirmed for lifetime escape. proposed_drop false. possible_cover: no explicit spec
acceptance; g007/l1, g009/s1 and g010/s1 corroborate original g012/l1, not separate defects. No consequence reduction or
withdrawal introduced.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g012/l1`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
