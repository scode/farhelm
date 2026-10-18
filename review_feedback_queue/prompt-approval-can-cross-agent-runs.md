# A prompt approval can apply to a different agent run

Reviewed commit: 13fa3c9a0d4c469d575315e3737b1ee454168d9c

## TLDR

A prompt approved for one agent run can be delivered to a replacement run in the same session after Restart or Replace
changes its permissions.

## Details

The approval card captures the target's launch and permission shield, but after approval the helm checks only host
incarnation and connection before sending by session ID (`crates/farhelm-helm/src/agent_requests.rs:1911–1965`). Restart
or Replace on that connection can change the run before the prompt is delivered. The supervisor checks the current run's
eligibility (`crates/farhelm-supervisor/src/service/core.rs:10560–10675`), rather than proving it is the run and shield
the person approved.

Suggested bucket: highest, for a prompt crossing a changed permission boundary. Confidence: likely; the same-connection
replacement while a card waits was not reproduced. Provenance: trailing review of the stated commit. `SPEC.md`,
“Agent-spawned sessions,” calls the attachment notice a snapshot but does not explicitly accept run or permission-shield
drift for an approved prompt. Bind the approval to the displayed run and shield, and refuse or ask again when either
changes.
