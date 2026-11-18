# A folder launch approval can name the previous machine

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A launch approval can still describe the old SSH destination after Farhelm has chosen its replacement. Allow can start
the agent and its wrapper on the replacement machine.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R3 Sol folder create after host replacement (B).

Source references: `crates/farhelm-helm/src/agent_requests.rs:2504–2505, 2632–2725, 2949–2965`,
`crates/farhelm-helm/src/hosts.rs:914–960`.

An agent on one host asks to create an ordinary folder session on an explicit SSH host. The helm resolves that name to a
row and display name, then awaits the wrapper check before sampling the connected installation for the card. A host edit
and accepted replacement can land between these observations. The card uses the old name but the fence uses the
replacement connection. There is a second window after the post-Allow incarnation check, during the awaited YOLO check
and before dispatch reads its client. The ordinary non-workspace-trust route has no composition incarnation to check
there.

Sol rated this source-confirmed and critical. Opus verification B confirmed both waits and rated it noncritical due to
the short helm-local windows. A change during the approval wait itself is fenced. Highest reflects the established
possibility of running an approved wrapper on a different installation. The prior container-place approval finding
concerns a different placement and dispatch path.

Possible cover: SPEC.md lines 419–429 says approved creates follow a logical host entry through retarget or adoption,
which is especially relevant to the post-Allow window. SPEC_impl.md lines 8898–8906 ties launch approval to the machine
the card names. The pre-card mismatch pairs old display information with new authority before the card is ever shown.
The specs leave that mismatch unclear. Keep both windows for triage.

Take the display facts and incarnation together when resolving the target. Carry that incarnation through approval to
the dispatch client check. Test a retarget between resolution and card publication and between Allow and dispatch.
